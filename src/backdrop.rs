//! Fundo das arenas.
//!
//! O fundo nao e um cartao parado atras da briga. Ele tem tres coisas que um
//! cartao nao tem:
//!
//! - **profundidade**: cada peca vive num plano, e planos distantes deslizam
//!   junto com a briga em vez de ficarem colados no mundo. E parallax de
//!   verdade, mesmo com a camera fixa -- quem se move e a atencao, nao a lente.
//! - **clima**: cinza, chuva, fuligem e petala caem sem parar, e nunca nascem
//!   nem morrem: a mesma particula da a volta pela moldura.
//! - **acontecimento**: o vulcao acorda de tempos em tempos, cospe fumaca,
//!   bomba de lava e sacode a tela.
//!
//! Nada aqui colide, machuca ou entra na fisica. O arquivo inteiro pode ser
//! reescrito sem que uma regra de jogo mude -- e por isso ele pode ser
//! exagerado a vontade.

use bevy::prelude::*;

use crate::actor::Player;
use crate::ascii::{Accent, AsciiArt, AsciiSprite, CELL, Layer, palette};
use crate::combat::Lifetime;
use crate::fx::Shake;
use crate::level::{ARENA_HALF_H, ARENA_HALF_W, LevelGeometry};
use crate::state::{AppSet, arena_live};

/// Um bloco de predio ao fundo: `(x, y, colunas, linhas)`.
pub type Building = (f32, f32, u16, u16);
/// Um letreiro: `(texto, y, cor acesa, fase do piscar)`.
pub type Sign = (&'static str, f32, Color, f32);

/// Direcao visual do fundo. A geometria continua obedecendo ao mesmo contrato.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Theme {
    #[default]
    City,
    Volcano,
    Industrial,
    Oriental,
}

/// Composicao visual de uma arena.
///
/// `Theme` ainda agrupa clima e linguagem; `Scene` escolhe o lugar concreto.
/// Os tres mapas de um tema deixam de ser a mesma pintura com plataformas
/// diferentes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Scene {
    #[default]
    City,
    Caldera,
    MagmaBridge,
    ForgeCore,
    AcidWorks,
    Reactor,
    Drainage,
    RedGate,
    SunsetPagoda,
    DragonGarden,
}

impl Scene {
    pub const fn theme(self) -> Theme {
        match self {
            Self::City => Theme::City,
            Self::Caldera | Self::MagmaBridge | Self::ForgeCore => Theme::Volcano,
            Self::AcidWorks | Self::Reactor | Self::Drainage => Theme::Industrial,
            Self::RedGate | Self::SunsetPagoda | Self::DragonGarden => Theme::Oriental,
        }
    }
}

// --- profundidade -----------------------------------------------------------

/// Profundidade de um plano: quanto ele acompanha a briga.
///
/// `0` e o plano do jogo -- fica parado no mundo, como o chao. `1` seria o
/// infinito, que acompanha a atencao inteira. Entre os dois esta o parallax:
/// como a camera nao anda, o que desliza sao os planos, e distante desliza
/// mais.
const SKY: f32 = 0.30;
const FAR: f32 = 0.21;
const MID: f32 = 0.13;
const NEAR: f32 = 0.06;
/// O plano do proprio jogo: nao desliza. E onde fica a moldura da arena, que
/// nao e paisagem -- e a borda do ringue.
const WORLD: f32 = 0.0;

/// O quanto o eixo vertical conta na profundidade.
///
/// Menos que o horizontal de proposito: o fundo tem chao e horizonte, e um pulo
/// nao pode levantar a serra do lugar.
const VERTICAL_BITE: f32 = 0.32;

/// Ate onde a atencao chega. Alem disto o fundo para de seguir.
///
/// Sem o limite, quem cai no vao arrasta o ceu inteiro junto na descida.
const REACH: Vec2 = Vec2::new(ARENA_HALF_W * 0.75, ARENA_HALF_H * 0.9);

/// A linha do chao das arenas.
///
/// Todo fundo se apoia nela. O terreno desenha na frente, entao uma peca
/// colocada abaixo desta altura existe, custa entidade e nunca aparece --
/// enterrar cenario e o erro mais barato de cometer aqui, e o mais dificil de
/// notar, porque nada quebra.
const GROUND: f32 = -170.0;

/// Largura, em colunas, de um plano que atravessa a tela.
///
/// A tela inteira mais o deslize do plano mais fundo, com sobra. Um cenario com
/// a largura exata da tela abre um buraco de ceu vazio no canto assim que a
/// briga anda para o outro lado -- e ela anda justamente quando os dois estao
/// no mesmo canto, que e quando alguem esta olhando para la.
const SPAN: u16 = 200;

/// Uma peca que vive num plano do fundo.
///
/// `home` e o lugar dela no plano; quem escreve a `Transform` e so
/// [`drift_planes`]. Fumaca, bomba de lava e clima mexem em `home` e continuam
/// respeitando a profundidade de graca -- se cada um escrevesse a `Transform`
/// direto, o parallax valeria so para o que esta parado.
#[derive(Component)]
struct Parallax {
    home: Vec2,
    depth: f32,
}

/// Bamboleio proprio, por cima do plano: brasa que sobe, lanterna que balanca.
#[derive(Component)]
struct Sway {
    phase: f32,
    speed: f32,
    travel: Vec2,
}

/// Onde a briga esta acontecendo, suavizado.
#[derive(Resource, Default)]
struct Focus(Vec2);

/// Segue o centro de massa dos lutadores.
///
/// Suavizado porque o alvo pula: alguem que morre e renasce do outro lado da
/// arena moveria a media de uma vez, e o ceu inteiro daria um tranco.
fn track_focus(
    time: Res<Time>,
    mut focus: ResMut<Focus>,
    players: Query<&Transform, With<Player>>,
) {
    let mut sum = Vec2::ZERO;
    let mut count = 0.0;
    for transform in &players {
        sum += transform.translation.truncate();
        count += 1.0;
    }
    let target = if count > 0.0 {
        (sum / count).clamp(-REACH, REACH)
    } else {
        Vec2::ZERO
    };
    focus.0 = focus.0.lerp(target, 1.0 - (-time.delta_secs() * 2.2).exp());
}

/// Poe cada peca no lugar: casa + bamboleio + deslize do plano.
///
/// Tambem decide o Z: plano mais fundo desenha antes, senao duas pecas na mesma
/// camada disputam quem cobre quem e a serra pode nascer na frente do vulcao.
fn drift_planes(
    time: Res<Time>,
    focus: Res<Focus>,
    mut planes: Query<(&Parallax, Option<&Sway>, &mut Transform)>,
) {
    let now = time.elapsed_secs();
    let drift = Vec2::new(focus.0.x, focus.0.y * VERTICAL_BITE);

    for (plane, sway, mut transform) in &mut planes {
        let wobble = sway.map_or(Vec2::ZERO, |sway| {
            let wave = now * sway.speed + sway.phase;
            Vec2::new(
                wave.sin() * sway.travel.x,
                (wave * 0.73).cos() * sway.travel.y,
            )
        });
        let at = plane.home + wobble + drift * plane.depth;
        transform.translation.x = at.x;
        transform.translation.y = at.y;
        transform.translation.z = -plane.depth;
    }
}

// --- pecas paradas ----------------------------------------------------------

/// Uma peca parada do fundo, ja resolvida em arte, lugar e profundidade.
///
/// O fundo se descreve antes de existir: o tema devolve uma lista destas e so
/// depois alguem as vira entidade. E o que permite conferir a composicao inteira
/// num teste -- sem `App`, sem GPU, sem janela -- em vez de descobrir na tela
/// que a cratera saiu do topo do cone.
struct Panel {
    art: AsciiArt,
    at: Vec2,
    depth: f32,
    /// Ancorada na base em vez do centro: o normal para o que pisa no fundo.
    foot: bool,
}

impl Panel {
    /// Peca centrada em `at`.
    fn new(art: AsciiArt, at: Vec2, depth: f32) -> Self {
        Self {
            art,
            at,
            depth,
            foot: false,
        }
    }

    /// Peca apoiada em `at`, que passa a ser o pe dela.
    fn footed(art: AsciiArt, at: Vec2, depth: f32) -> Self {
        Self {
            foot: true,
            ..Self::new(art, at, depth)
        }
    }
}

fn raise(commands: &mut Commands, panel: Panel) {
    let sprite = if panel.foot {
        AsciiSprite::footed(panel.art)
    } else {
        AsciiSprite::new(panel.art)
    };
    commands.spawn((
        LevelGeometry,
        Parallax {
            home: panel.at,
            depth: panel.depth,
        },
        sprite,
        Layer::Background,
        Transform::from_translation(panel.at.extend(-panel.depth)),
    ));
}

/// Brasa, faisca ou lanterna: um glifo que balanca no lugar.
fn spark(commands: &mut Commands, glyph: char, at: Vec2, color: Color, sway: Sway, depth: f32) {
    commands.spawn((
        LevelGeometry,
        Parallax { home: at, depth },
        sway,
        AsciiSprite::new(AsciiArt::glyph(glyph, color)),
        Layer::Background,
        Transform::from_translation(at.extend(-depth)),
    ));
}

// --- serra ------------------------------------------------------------------

/// Monta a silhueta de uma serra a partir de alturas de controle.
///
/// Gerada, e nao desenhada a mao: uma cordilheira de cento e tantas colunas
/// escrita caractere a caractere sai com degrau flutuando e encosta furada, e
/// nada no jogo reclama. Aqui a encosta e reta entre dois controles por
/// construcao, e a altura fracionaria vira meio bloco (`▄`) -- resolucao de meia
/// celula de graca, que e o que tira o serrilhado da crista.
fn ridge(controls: &[u16], span: u16, color: Color) -> AsciiArt {
    let rows = controls.iter().copied().max().unwrap_or(1).max(1);
    let last = controls.len() - 1;

    // A altura de cada coluna, com casa decimal: e ela que vira meio bloco.
    let crest: Vec<f32> = (0..span)
        .map(|col| {
            let t = col as f32 * last as f32 / (span.max(2) - 1) as f32;
            let low = (t.floor() as usize).min(last);
            let high = (low + 1).min(last);
            controls[low] as f32 + (controls[high] as f32 - controls[low] as f32) * t.fract()
        })
        .collect();

    let text = (0..rows)
        .map(|row| {
            // Quantas linhas de encosta esta altura precisa ter para chegar aqui.
            let needed = (rows - row) as f32;
            crest
                .iter()
                .map(|height| match *height {
                    h if h >= needed => '█',
                    h if h >= needed - 0.55 => '▄',
                    _ => ' ',
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    AsciiArt::solid(&text, color)
}

/// Teto de fumaca esfarrapado, pendurado no alto do ceu.
///
/// E onde a coluna do vulcao vai dar. Sem ele a fumaca sobe e some no preto, e
/// a erupcao perde o teto que a faz parecer uma erupcao e nao uma fogueira.
///
/// A densidade rareia para baixo e serpenteia com a coluna, entao a borda de
/// baixo nunca sai reta -- que e o que denuncia faixa desenhada com `fill`.
fn smog(span: u16, rows: u16, color: Color) -> AsciiArt {
    let text = (0..rows)
        .map(|row| {
            (0..span)
                .map(|col| {
                    let wave = ((col as f32 * 0.21).sin() + (col as f32 * 0.07).cos()) * 0.45;
                    match 1.15 - row as f32 / rows as f32 + wave {
                        d if d > 1.05 => '▓',
                        d if d > 0.75 => '▒',
                        d if d > 0.45 => '░',
                        _ => ' ',
                    }
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    AsciiArt::solid(&text, color)
}

// --- o vulcao ---------------------------------------------------------------

/// Altura do cone, em linhas.
const CONE_ROWS: u16 = 13;
/// Largura do topo do cone, em colunas.
const CONE_TOP: u16 = 14;
/// Quanto ele alarga por linha, em colunas por lado.
const CONE_FLARE: u16 = 2;
/// Largura da boca da cratera.
const CRATER_COLS: u16 = 6;
/// Quantas linhas do topo ficam abertas.
const CRATER_ROWS: u16 = 2;

/// Onde o vulcao se apoia. Fora do eixo de proposito: montanha centrada na tela
/// vira logotipo, e o mapa e assimetrico.
const VOLCANO_FOOT: Vec2 = Vec2::new(120.0, GROUND);

/// A boca da cratera, em coordenadas de mundo.
///
/// Sai da propria arte -- base mais altura, menos a linha aberta -- e nao de um
/// numero escrito a mao. Fumaca, brilho e bomba de lava saem daqui: mexer na
/// altura do cone leva os tres junto.
const CRATER: Vec2 = Vec2::new(
    VOLCANO_FOOT.x,
    VOLCANO_FOOT.y + (CONE_ROWS as f32 - CRATER_ROWS as f32 * 0.5) * CELL.y,
);

/// Os glifos por onde a lava desce. Sao eles que o [`Accent`] acende: a
/// montanha inteira e uma arte so, e a cor separa rocha de brasa.
const LAVA: &str = "≈~";

/// Onde os dois veios passam nesta linha, medidos do meio para fora.
///
/// Fracao da meia-largura, e nao coluna fixa: o veio desce pela encosta, que
/// abre a cada linha, entao ele tem que abrir junto. Reto para baixo, ele leria
/// como cano descendo por dentro da montanha.
const VEINS: [f32; 2] = [-0.42, 0.30];

/// Um caractere da encosta: rocha, lava ou vazio.
///
/// Os tres saem da mesma funcao porque so assim a lava cabe exatamente onde a
/// rocha nao esta. Enquanto foram duas artes carimbadas uma sobre a outra, cada
/// celula de brasa desenhava em cima de uma pedra -- duas celulas no mesmo
/// lugar, no mesmo Z, e quem aparecia era sorteio da ordem de spawn.
fn face(col: u16, width: u16, row: u16) -> char {
    // Bordo duro dos dois lados: e ele que recorta a silhueta contra o ceu.
    if col == 0 || col + 1 == width {
        return '█';
    }
    if col == 1 || col + 2 == width {
        return '▓';
    }
    // A boca fica aberta: o brilho da lava entra por ela, por baixo.
    let mouth = (width - CRATER_COLS) / 2;
    if row < CRATER_ROWS && col >= mouth && col < mouth + CRATER_COLS {
        return ' ';
    }
    let middle = (width as f32 - 1.0) * 0.5;
    let meander = (row as f32 * 1.3).sin() * 1.6;
    for lean in VEINS {
        let channel = (middle * (1.0 + lean) + meander).round();
        if col as f32 == channel {
            return if row.is_multiple_of(2) { '≈' } else { '~' };
        }
    }
    // Motivo curto que anda com a linha, entao a textura nunca se repete na
    // mesma coluna e a encosta nao vira papel de parede listrado.
    const ROCK: [char; 6] = ['▒', '▒', '░', '▓', '▒', '▓'];
    ROCK[(col as usize * 3 + row as usize * 5) % ROCK.len()]
}

/// A montanha inteira, linha a linha.
fn cone(stone: Color, lava: Color) -> AsciiArt {
    let base = CONE_TOP + (CONE_ROWS - 1) * CONE_FLARE * 2;
    let mut text = String::new();
    for row in 0..CONE_ROWS {
        let width = CONE_TOP + row * CONE_FLARE * 2;
        text.push_str(&" ".repeat(((base - width) / 2) as usize));
        text.extend((0..width).map(|col| face(col, width, row)));
        text.push('\n');
    }
    AsciiArt::build(
        &text,
        &Accent {
            base: stone,
            accent: lava,
            on: LAVA,
        },
    )
}

/// A lava vista pela boca da cratera.
const CRATER_GLOW: &str = "░▒▓▓▒░
▒▓██▓▒";

/// O clarao do instante em que ela estoura.
const BLAST_FLASH: &str = " ▄▄▄▄
▄████▄
██████
▀████▀";

// --- o resto do repertorio --------------------------------------------------

/// Lua cheia com mar de sombra.
const MOON: &str = "  ▄▄▄▄▄▄
 ████████
██▓▓▒████
██▓░▒▓███
 ███▓████
  ▀▀▀▀▀▀";

/// Antena de radio da cidade: a coisa mais alta do horizonte.
const MAST: &str = "  ▄
 ▄█▄
  █
 ▄█▄
▄▀█▀▄
  █
 ▄█▄
▄███▄";

/// Caixa d'agua no telhado.
const TANK: &str = "▄████▄
██████
▀█▀▀█▀
 █  █";

/// Um vao da trelica de aco. O portico e este desenho repetido.
const BAY: [&str; 6] = [
    "o===============",
    "|\\             /",
    "| \\           / ",
    "|  o=========o  ",
    "| /           \\ ",
    "o===============",
];

/// O portico que atravessa a fabrica, vao a vao.
///
/// Gerado porque trelica e repeticao pura: doze vaos escritos a mao sao seis
/// linhas de duzentos caracteres cada, e basta um traco a menos em uma delas
/// para a estrutura nascer torta.
fn gantry(bays: usize, color: Color) -> AsciiArt {
    let text = BAY
        .iter()
        .map(|row| {
            // A coluna que fecha o ultimo vao e a primeira do proprio desenho:
            // montante vira montante, junta vira junta.
            let mut line = row.repeat(bays);
            line.push(row.chars().next().unwrap_or(' '));
            line
        })
        .collect::<Vec<_>>()
        .join("\n");
    AsciiArt::solid(&text, color)
}

/// Cano de escoamento correndo na linha do chao, com as saidas em pe.
fn drains(span: u16, color: Color) -> AsciiArt {
    let vent = |col: u16| col % 17 == 5;
    let cano: String = (0..span)
        .map(|col| if vent(col) { '╦' } else { '═' })
        .collect();
    let saida: String = (0..span)
        .map(|col| if vent(col) { '╨' } else { ' ' })
        .collect();
    AsciiArt::solid(&format!("{cano}\n{saida}"), color)
}

/// Onde a refinaria se apoia.
const REFINERY_AT: Vec2 = Vec2::new(-30.0, GROUND + 110.0);

/// As tres chamines, medidas do meio da arte da refinaria.
///
/// Onze colunas separam um cano do seguinte no desenho. As bocas de fumaca sao
/// entidades a parte da arte; enquanto os dois foram numeros escritos em
/// lugares diferentes, o vapor subia de tres pontos do ceu ao lado dos canos e
/// a arte continuava intacta -- nada para um teste pegar, e obvio na tela.
const STACKS: [f32; 3] = [-11.0 * CELL.x, 0.0, 11.0 * CELL.x];

/// Tanques de pressao, com as tres chamines em cima.
const REFINERY: &str = "   ▄▄▄        ▄▄▄        ▄▄▄
   ███        ███        ███
 ▄▄███▄▄    ▄▄███▄▄    ▄▄███▄▄
 ▐█████▌    ▐█████▌    ▐█████▌
 ▐█████▌    ▐█████▌    ▐█████▌
▄███████▄▄▄▄███████▄▄▄▄███████▄";

/// Carcaca da usina: vasos, passarelas e a placa de risco.
const PLANT: &str = "    ____             .---------.             ____
 __/____\\__          |  TOX-03  |          __/____\\__
|  PRESSURE |========|    !!    |========| CORROSIVE |
|___________|        '---------'         |___________|
     ||                  ||                   ||
=====OO==================OO===================OO=====";

/// Pagode de tres andares.
const PAGODA: &str = "        ▄▄▄
    ▄▄▄█████▄▄▄
 ▄▄███████████████▄▄
    ██│╬╪╫│██
  ▄▄███████████▄▄
 ▄███████████████▄
    ██│ ╬ │██
  ▄▄███████████▄▄
    ███████████
    ██│  │██│█";

/// Torii: o portao vermelho, sempre no primeiro plano.
const TORII: &str = "▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄
 ▀▀██▀▀▀▀▀▀▀██▀▀
▄▄▄██▄▄▄▄▄▄▄██▄▄▄
   ██       ██
   ██       ██
   ██       ██";

/// Galho de cerejeira entrando pelo canto de cima.
const BRANCH: &str = "▄▄▄▄▄▄▄▄▄▄
 ░ ▀▀▄▄ ░ ▀▀▄▄▄
░ ░  ░ ▀▄  ░  ▀▀▄
      ░  ░ ░";

/// Lanterna de papel, com o miolo vazado: a brasa que mora ali dentro e uma
/// entidade a parte, que balanca sozinha.
const LANTERN: &str = "┌─┐\n│ │\n└─┘";

/// Sol baixo do poente.
const SUN: &str = "   ▄██████▄
 ▄██████████▄
▄██████████████▄
████████████████
▀██████████████▀
 ▀██████████▀
   ▀██████▀";

/// Ponte de basalto suspensa sobre duas quedas de magma.
const BASALT_BRIDGE: &str = r"███████████████████████████████████████████████
██▓▓▒▒░       o=================o       ░▒▒▓▓██
██▓▒░        /|                 |\        ░▒▓██
██▓▒        / |                 | \        ▒▓██
██▓▒   ~~  /  |                 |  \  ~~   ▒▓██
██▓▒   ≈≈ /   |                 |   \ ≈≈   ▒▓██
███████████████████████████████████████████████";

/// Boca mecanica da forja: massa escura, dentes e nucleo quente.
const FORGE_MOUTH: &str = "        ▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄
     ▄██▓▓▒▒▒▒▒▒▒▒▒▓▓██▄
   ▄██▓▒   ╔═══════╗   ▒▓██▄
  ██▓▒    ║ ≈≈≈≈≈ ║    ▒▓██
  ██▓▒    ║≈█████≈║    ▒▓██
  ██▓▒    ║ ≈≈≈≈≈ ║    ▒▓██
  ████▄▄▄▄╩═══════╝▄▄▄▄████
     ███████████████████";

/// Anel do reator, com nucleo vazado para manter espaco negativo.
const REACTOR_CORE: &str = r"       .----[ 02 ]----.
    .-'      ____      '-.
   /      .-'    '-.      \
  |      /  ░▒▓▒░  \      |
==O=====|  ▒▓█▓▒  |=====O==
  |      \  ░▒▓▒░  /      |
   \      '-.____.-'      /
    '-.      ||      .-'
       '----=OO=----'";

/// Galeria baixa da drenagem; arcos repetidos leem como subsolo, nao skyline.
const SEWER: &str = r"▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄
██▓▓▒▒░   .--------.   ░▒▒▓▓██▓▓▒▒░   .--------.   ░▒▒▓▓██
██▓▒     /          \     ▒▓██▓▒     /          \     ▒▓██
██▓     /            \     ▓██▓     /            \     ▓██
██▓    |              |    ▓██▓    |              |    ▓██
███▄▄▄▄|__≈≈≈≈≈≈≈≈__|▄▄▄▄█████▄▄▄▄|__≈≈≈≈≈≈≈≈__|▄▄▄▄███";

/// Lua vazada do jardim, bem menos luminosa que o sol do pagode.
const MOON_GATE: &str = "    ▄▄▄▄▄
  ▄██▀▀▀██▄
 ██▀       ▀██
██           ██
██           ██
 ██▄       ▄██
  ▀██▄▄▄██▀
    ▀▀▀▀▀";

/// Dragao de pedra horizontal: uma unica linha de gesto e poucos detalhes.
const STONE_DRAGON: &str = r"        __      /\                 _
  _____/  \____/  \__       ___.-' '-.
<____  o     _   _   '-----'  _      /
     \______/ \_/ \__________/ \____/
        /_/                 \_\";

const BAMBOO: &str = " |   |      |   |
-O-  |     -O-  |
 |  -O-     |  -O-
-O-  |     -O-  |
 |   |      |   |";

// --- a composicao de cada tema ----------------------------------------------

/// Serra distante do vulcao. Os numeros sao alturas em linhas.
const VOLCANO_RIDGE: [u16; 15] = [2, 5, 3, 8, 4, 6, 3, 9, 5, 3, 7, 4, 8, 3, 2];
/// Serra do tema oriental: mais baixa e mais longa, para o sol ficar por cima.
const EAST_RIDGE: [u16; 13] = [2, 4, 7, 3, 5, 9, 4, 6, 3, 8, 4, 5, 2];
/// Torres distantes da fabrica.
const STACK_RIDGE: [u16; 11] = [3, 9, 4, 10, 3, 6, 11, 4, 9, 3, 5];
/// Morros atras da cidade: baixos, para aparecerem so entre os predios.
const CITY_RIDGE: [u16; 12] = [2, 4, 3, 6, 2, 5, 3, 6, 2, 4, 5, 2];

/// Predios distantes da cidade: `(x, y, colunas, linhas)`, ja no plano de tras.
const CITY_FAR: [Building; 9] = [
    (-600.0, -30.0, 9, 12),
    (-450.0, -10.0, 7, 16),
    (-300.0, -40.0, 11, 10),
    (-160.0, 0.0, 8, 18),
    (-20.0, -25.0, 12, 13),
    (140.0, -5.0, 7, 17),
    (290.0, -35.0, 10, 11),
    (450.0, -12.0, 8, 15),
    (600.0, -32.0, 11, 12),
];

/// A composicao parada do tema.
///
/// De tras para frente, e tudo apoiado em [`GROUND`]: cenario nasce no chao da
/// arena, nao numa altura escrita a mao que ninguem sabe conferir.
fn panels(scene: Scene) -> Vec<Panel> {
    match scene {
        Scene::City => vec![
            // Morros baixos atras da cidade: eles so aparecem nos vaos entre um
            // predio e outro, e e isso que da fundo ao horizonte.
            Panel::footed(
                ridge(&CITY_RIDGE, SPAN, palette::COAL),
                Vec2::new(0.0, GROUND),
                SKY,
            ),
            Panel::new(
                AsciiArt::solid(MOON, palette::ASH),
                Vec2::new(-420.0, 150.0),
                SKY,
            ),
            Panel::footed(
                AsciiArt::solid(MAST, palette::IRON),
                Vec2::new(470.0, GROUND + 230.0),
                FAR,
            ),
            Panel::footed(
                AsciiArt::solid(TANK, palette::IRON),
                Vec2::new(-250.0, GROUND + 240.0),
                FAR,
            ),
            // A moldura da arena: uma viga em cima e dois pilares nas pontas.
            // Ela nao e paisagem, e a borda do ringue -- por isso fica no plano
            // do jogo e nao desliza com o resto.
            Panel::new(
                AsciiArt::fill('═', SPAN, 1, palette::IRON),
                Vec2::new(0.0, 205.0),
                WORLD,
            ),
            Panel::new(
                AsciiArt::fill('║', 1, 25, palette::IRON),
                Vec2::new(-590.0, 5.0),
                WORLD,
            ),
            Panel::new(
                AsciiArt::fill('║', 1, 25, palette::IRON),
                Vec2::new(590.0, 5.0),
                WORLD,
            ),
        ],
        // A cratera nao esta aqui: quem a desenha e a propria boca, em
        // `vents`, porque a cor dela e o aviso de que a montanha vai estourar.
        Scene::Caldera => vec![
            Panel::new(
                smog(SPAN, 5, palette::COAL),
                Vec2::new(0.0, ARENA_HALF_H - 24.0),
                SKY,
            ),
            Panel::footed(
                ridge(&VOLCANO_RIDGE, SPAN, palette::COAL),
                Vec2::new(0.0, GROUND),
                SKY,
            ),
            Panel::footed(cone(palette::IRON, palette::SCENE_FIRE), VOLCANO_FOOT, MID),
            // Chao derretido correndo na linha do horizonte, na frente da
            // montanha: e o que deixa claro que a arena inteira esta dentro da
            // caldeira, e nao so olhando para ela de longe.
            Panel::footed(
                AsciiArt::fill('▄', SPAN, 1, palette::SCENE_RED).stamp(
                    &AsciiArt::fill('▀', SPAN, 1, palette::SCENE_FIRE),
                    0,
                    1,
                ),
                Vec2::new(0.0, GROUND),
                NEAR,
            ),
        ],
        Scene::MagmaBridge => vec![
            Panel::new(
                smog(SPAN, 3, palette::COAL),
                Vec2::new(0.0, ARENA_HALF_H - 22.0),
                SKY,
            ),
            Panel::footed(
                ridge(&[3, 7, 4, 11, 6, 2, 8, 3, 9, 4, 3], SPAN, palette::COAL),
                Vec2::new(0.0, GROUND),
                SKY,
            ),
            Panel::footed(
                AsciiArt::build(
                    BASALT_BRIDGE,
                    &Accent {
                        base: palette::IRON,
                        accent: palette::SCENE_FIRE,
                        on: LAVA,
                    },
                ),
                Vec2::new(0.0, GROUND + 18.0),
                MID,
            ),
        ],
        Scene::ForgeCore => vec![
            Panel::new(
                smog(SPAN, 6, palette::COAL),
                Vec2::new(0.0, ARENA_HALF_H - 28.0),
                SKY,
            ),
            Panel::footed(
                gantry(12, palette::COAL),
                Vec2::new(0.0, GROUND + 250.0),
                FAR,
            ),
            Panel::footed(
                AsciiArt::build(
                    FORGE_MOUTH,
                    &Accent {
                        base: palette::IRON,
                        accent: palette::SCENE_FIRE,
                        on: LAVA,
                    },
                ),
                Vec2::new(170.0, GROUND + 12.0),
                MID,
            ),
            Panel::footed(
                AsciiArt::fill('║', 1, 24, palette::IRON),
                Vec2::new(-470.0, GROUND),
                NEAR,
            ),
            Panel::footed(
                AsciiArt::fill('║', 1, 24, palette::IRON),
                Vec2::new(470.0, GROUND),
                NEAR,
            ),
        ],
        Scene::AcidWorks => vec![
            Panel::new(
                smog(SPAN, 4, palette::COAL),
                Vec2::new(0.0, ARENA_HALF_H - 20.0),
                SKY,
            ),
            Panel::footed(
                ridge(&STACK_RIDGE, SPAN, palette::COAL),
                Vec2::new(0.0, GROUND),
                SKY,
            ),
            Panel::footed(AsciiArt::solid(REFINERY, palette::IRON), REFINERY_AT, MID),
            Panel::footed(
                AsciiArt::solid(PLANT, palette::SCENE_TOXIC),
                Vec2::new(0.0, GROUND + 10.0),
                MID,
            ),
        ],
        Scene::Reactor => vec![
            Panel::footed(
                gantry(12, palette::IRON),
                Vec2::new(0.0, GROUND + 250.0),
                FAR,
            ),
            Panel::new(
                AsciiArt::solid(REACTOR_CORE, palette::SCENE_BLUE),
                Vec2::new(0.0, 58.0),
                MID,
            ),
            Panel::footed(drains(SPAN, palette::COAL), Vec2::new(0.0, GROUND), NEAR),
        ],
        Scene::Drainage => vec![
            Panel::footed(
                AsciiArt::solid(SEWER, palette::IRON),
                Vec2::new(0.0, GROUND + 8.0),
                MID,
            ),
            Panel::footed(
                AsciiArt::fill('~', SPAN, 1, palette::SCENE_TOXIC),
                Vec2::new(0.0, GROUND),
                NEAR,
            ),
        ],
        Scene::RedGate => vec![
            Panel::footed(
                ridge(&[2, 3, 5, 4, 6, 3, 5, 2], SPAN, palette::SCENE_HAZE),
                Vec2::new(0.0, GROUND),
                SKY,
            ),
            Panel::footed(
                AsciiArt::solid(TORII, palette::SCENE_RED),
                Vec2::new(0.0, GROUND + 12.0),
                MID,
            ),
            Panel::new(
                AsciiArt::solid(BRANCH, palette::COAL),
                Vec2::new(-450.0, 185.0),
                NEAR,
            ),
        ],
        Scene::SunsetPagoda => vec![
            Panel::new(
                AsciiArt::solid(SUN, palette::SCENE_RED),
                Vec2::new(-315.0, 95.0),
                SKY,
            ),
            Panel::new(
                smog(SPAN, 3, palette::SCENE_HAZE),
                Vec2::new(0.0, ARENA_HALF_H - 16.0),
                FAR,
            ),
            Panel::footed(
                ridge(&EAST_RIDGE, SPAN, palette::SCENE_HAZE),
                Vec2::new(0.0, GROUND),
                FAR,
            ),
            Panel::footed(
                AsciiArt::solid(PAGODA, palette::SCENE_GOLD),
                Vec2::new(250.0, GROUND + 48.0),
                MID,
            ),
        ],
        Scene::DragonGarden => vec![
            Panel::new(
                AsciiArt::solid(MOON_GATE, palette::IRON),
                Vec2::new(320.0, 92.0),
                SKY,
            ),
            Panel::footed(
                ridge(&[2, 5, 3, 4, 2, 6, 3, 5, 2], SPAN, palette::COAL),
                Vec2::new(0.0, GROUND),
                FAR,
            ),
            Panel::footed(
                AsciiArt::solid(STONE_DRAGON, palette::ASH),
                Vec2::new(0.0, GROUND + 56.0),
                MID,
            ),
            Panel::new(
                AsciiArt::solid(BAMBOO, palette::SCENE_TOXIC),
                Vec2::new(-470.0, 42.0),
                NEAR,
            ),
        ],
    }
}

// --- clima ------------------------------------------------------------------

/// O que cai do ceu num tema, e como.
///
/// Uma tabela e nao quatro sistemas: chuva, cinza, fuligem e petala so diferem
/// em glifo, cor e ritmo. Enquanto foram codigo separado, so a que alguem estava
/// olhando ganhava conserto.
struct Weather {
    glyphs: &'static [char],
    colors: &'static [Color],
    count: usize,
    /// Queda por segundo, ja negativa.
    fall: f32,
    /// Deriva lateral constante -- vento.
    slant: f32,
    /// Amplitude do bamboleio de quem cai devagar.
    sway: f32,
    depth: f32,
}

/// Uma particula de clima. Ela nunca morre: ao sair da moldura volta pelo topo.
///
/// Sem isto, chuva e cinza seriam spawn e despawn a cada quadro -- centenas de
/// entidades nascendo e morrendo por segundo para desenhar o que nunca muda.
#[derive(Component)]
struct Drift {
    fall: f32,
    slant: f32,
    sway: f32,
    phase: f32,
}

fn weather(theme: Theme) -> Weather {
    match theme {
        Theme::City => Weather {
            glyphs: &['│', '!', '\'', '│'],
            colors: &[palette::IRON, palette::ASH],
            count: 54,
            fall: -520.0,
            slant: -70.0,
            sway: 0.0,
            depth: NEAR,
        },
        Theme::Volcano => Weather {
            glyphs: &['·', '°', '∙', ','],
            colors: &[palette::ASH, palette::IRON, palette::SCENE_FIRE],
            count: 46,
            fall: -46.0,
            slant: -14.0,
            sway: 22.0,
            depth: MID,
        },
        Theme::Industrial => Weather {
            glyphs: &['·', '.', '∙'],
            colors: &[palette::IRON, palette::COAL, palette::SCENE_TOXIC],
            count: 34,
            fall: -62.0,
            slant: 18.0,
            sway: 14.0,
            depth: MID,
        },
        Theme::Oriental => Weather {
            glyphs: &['*', ',', '°', '·'],
            colors: &[palette::SCENE_RED, palette::SCENE_GOLD, palette::SCENE_HAZE],
            count: 40,
            fall: -54.0,
            slant: -26.0,
            sway: 30.0,
            depth: MID,
        },
    }
}

/// Semeia o clima ja espalhado pela tela inteira.
///
/// Nascer tudo no topo faria a primeira leva descer em bloco, como uma cortina.
fn seed_weather(commands: &mut Commands, sky: &Weather) {
    for i in 0..sky.count {
        let at = Vec2::new(
            (fastrand::f32() - 0.5) * (ARENA_HALF_W * 2.0 + 120.0),
            (fastrand::f32() - 0.5) * (ARENA_HALF_H * 2.0 + 80.0),
        );
        commands.spawn((
            LevelGeometry,
            Parallax {
                home: at,
                depth: sky.depth,
            },
            Drift {
                fall: sky.fall * (0.75 + fastrand::f32() * 0.5),
                slant: sky.slant,
                sway: sky.sway,
                phase: i as f32 * 0.77,
            },
            AsciiSprite::new(AsciiArt::glyph(
                sky.glyphs[i % sky.glyphs.len()],
                sky.colors[i % sky.colors.len()],
            )),
            Layer::Background,
            Transform::from_translation(at.extend(-sky.depth)),
        ));
    }
}

/// Faz o clima cair, e o devolve ao topo quando ele sai por baixo.
fn blow_weather(time: Res<Time>, mut drops: Query<(&Drift, &mut Parallax)>) {
    let dt = time.delta_secs();
    let now = time.elapsed_secs();
    let edge = ARENA_HALF_W + 80.0;

    for (drift, mut plane) in &mut drops {
        plane.home.y += drift.fall * dt;
        plane.home.x += (drift.slant + (now * 0.9 + drift.phase).sin() * drift.sway) * dt;

        if plane.home.y < -ARENA_HALF_H - 40.0 {
            plane.home.y = ARENA_HALF_H + 30.0;
            plane.home.x = (fastrand::f32() - 0.5) * (edge * 2.0);
        }
        if plane.home.x < -edge {
            plane.home.x = edge;
        } else if plane.home.x > edge {
            plane.home.x = -edge;
        }
    }
}

// --- fumaca e erupcao -------------------------------------------------------

/// Uma boca que solta fumaca.
///
/// A chamine da fabrica e a cratera do vulcao sao a mesma coisa com numeros
/// diferentes: uma so fumega, a outra tambem explode de tempos em tempos. Como e
/// um componente so, a fumaca das duas envelhece pela mesma regra -- enquanto
/// foram dois sistemas, so a que alguem estava olhando ficava bonita.
#[derive(Component)]
struct Vent {
    /// Intervalo entre baforadas em repouso.
    puff: Timer,
    /// Fumaca quente de rocha derretida, ou vapor de agua.
    hot: bool,
    /// Forca da coluna: quanto a baforada sobe e quanto ela dura.
    power: f32,
    /// Segundos restantes de erupcao.
    blast: f32,
    /// Conta para a proxima erupcao. `None` numa chamine, que so fumega.
    erupt: Option<Timer>,
    /// Tom aceso agora, para o brilho nao ser reconstruido a cada quadro.
    tone: usize,
}

/// Quanto tempo a boca cospe depois de estourar.
const BLAST_TIME: f32 = 3.4;
/// Quantas vezes ela cospe mais rapido enquanto isso.
const BLAST_RUSH: f32 = 5.0;

/// Tons da cratera, do descanso ao estouro.
const HEAT: [Color; 3] = [palette::SCENE_RED, palette::SCENE_FIRE, palette::SCENE_GOLD];

/// Uma baforada, do nascimento a dissipacao.
const PUFF: [&str; 6] = [
    "∙",
    "░▒░",
    "▒▓▒\n░▒░",
    " ▒▓▓▒\n▒▓██▓\n ░▒▒░",
    "░▒▓▓▒░\n▒▓▓██▓▒\n ░▒▓▒░",
    "░ ▒ ░ \n ░ ▒ ░\n░  ░  ",
];

/// Cor de cada quadro da fumaca quente: sai brasa e vira cinza.
const SOOT: [Color; 6] = [
    palette::SCENE_FIRE,
    palette::SCENE_RED,
    palette::IRON,
    palette::IRON,
    palette::COAL,
    palette::COAL,
];
/// Cor de cada quadro do vapor.
const STEAM: [Color; 6] = [
    palette::ASH,
    palette::ASH,
    palette::ASH,
    palette::IRON,
    palette::IRON,
    palette::COAL,
];

/// Uma baforada solta no ar.
#[derive(Component)]
struct Smoke {
    age: f32,
    life: f32,
    rise: f32,
    drift: f32,
    frame: usize,
    hot: bool,
}

/// Pedra derretida cuspida pela cratera. Vive no plano do fundo: nao colide com
/// nada, so risca o ceu e apaga.
#[derive(Component)]
struct LavaBomb {
    velocity: Vec2,
    trail: Timer,
}

/// Gravidade das bombas. Menor que a do jogo de proposito -- o arco tem que
/// durar o suficiente para ser visto do outro lado da arena.
const BOMB_GRAVITY: f32 = 300.0;

fn vents(commands: &mut Commands, scene: Scene) {
    match scene {
        Scene::Caldera => {
            commands.spawn((
                LevelGeometry,
                Parallax {
                    home: CRATER,
                    depth: MID,
                },
                Vent {
                    puff: Timer::from_seconds(0.42, TimerMode::Repeating),
                    hot: true,
                    power: 1.0,
                    blast: 0.0,
                    erupt: Some(Timer::from_seconds(11.0, TimerMode::Repeating)),
                    tone: 1,
                },
                AsciiSprite::new(AsciiArt::solid(CRATER_GLOW, HEAT[1])),
                Layer::Background,
                Transform::from_translation(CRATER.extend(-MID)),
            ));
        }
        Scene::MagmaBridge => {
            for x in [-310.0, 310.0] {
                let at = Vec2::new(x, GROUND + 115.0);
                commands.spawn((
                    LevelGeometry,
                    Parallax {
                        home: at,
                        depth: MID,
                    },
                    Vent {
                        puff: Timer::from_seconds(0.9, TimerMode::Repeating),
                        hot: true,
                        power: 0.55,
                        blast: 0.0,
                        erupt: None,
                        tone: 0,
                    },
                    Transform::from_translation(at.extend(-MID)),
                ));
            }
        }
        Scene::ForgeCore => {
            let at = Vec2::new(170.0, GROUND + 150.0);
            commands.spawn((
                LevelGeometry,
                Parallax {
                    home: at,
                    depth: MID,
                },
                Vent {
                    puff: Timer::from_seconds(0.58, TimerMode::Repeating),
                    hot: true,
                    power: 0.72,
                    blast: 0.0,
                    erupt: None,
                    tone: 0,
                },
                Transform::from_translation(at.extend(-MID)),
            ));
        }
        Scene::AcidWorks => {
            // Em cima dos canos, que e o topo da arte da refinaria.
            let top = REFINERY_AT.y + REFINERY.lines().count() as f32 * CELL.y;
            for (i, dx) in STACKS.into_iter().enumerate() {
                let at = Vec2::new(REFINERY_AT.x + dx, top);
                commands.spawn((
                    LevelGeometry,
                    Parallax {
                        home: at,
                        depth: MID,
                    },
                    Vent {
                        puff: Timer::from_seconds(0.7 + i as f32 * 0.19, TimerMode::Repeating),
                        hot: false,
                        power: 0.62,
                        blast: 0.0,
                        erupt: None,
                        tone: 0,
                    },
                    Transform::from_translation(at.extend(-MID)),
                ));
            }
        }
        Scene::Reactor => {
            for x in [-190.0, 190.0] {
                let at = Vec2::new(x, GROUND + 180.0);
                commands.spawn((
                    LevelGeometry,
                    Parallax {
                        home: at,
                        depth: FAR,
                    },
                    Vent {
                        puff: Timer::from_seconds(1.15, TimerMode::Repeating),
                        hot: false,
                        power: 0.48,
                        blast: 0.0,
                        erupt: None,
                        tone: 0,
                    },
                    Transform::from_translation(at.extend(-FAR)),
                ));
            }
        }
        Scene::City
        | Scene::Drainage
        | Scene::RedGate
        | Scene::SunsetPagoda
        | Scene::DragonGarden => {}
    }
}

fn puff(commands: &mut Commands, at: Vec2, depth: f32, power: f32, hot: bool) {
    let life = (1.9 + fastrand::f32() * 1.4) * power.max(0.4);
    commands.spawn((
        LevelGeometry,
        Parallax { home: at, depth },
        Smoke {
            age: 0.0,
            life,
            rise: (34.0 + fastrand::f32() * 42.0) * power,
            drift: (fastrand::f32() - 0.35) * 26.0,
            frame: 0,
            hot,
        },
        AsciiSprite::new(AsciiArt::solid(
            PUFF[0],
            if hot { SOOT[0] } else { STEAM[0] },
        )),
        Layer::Background,
        Transform::from_translation(at.extend(-depth)),
    ));
}

/// Toca as bocas: baforada, contagem para a erupcao e o brilho da cratera.
fn run_vents(
    time: Res<Time>,
    mut commands: Commands,
    mut shake: MessageWriter<Shake>,
    mut vents: Query<(&mut Vent, &Parallax, Option<&mut AsciiSprite>)>,
) {
    let delta = time.delta();
    let dt = time.delta_secs();
    let now = time.elapsed_secs();

    for (mut vent, plane, glow) in &mut vents {
        vent.blast = (vent.blast - dt).max(0.0);

        // --- a erupcao ---
        let mut warning = 0.0;
        if let Some(clock) = vent.erupt.as_mut() {
            warning = clock.fraction().powi(6);
            if clock.tick(delta).just_finished() {
                vent.blast = BLAST_TIME;
                shake.write(Shake(0.55));
                blast(&mut commands, plane.home, plane.depth);
            }
        }

        // --- a fumaca ---
        let erupting = vent.blast > 0.0;
        let rush = if erupting { BLAST_RUSH } else { 1.0 };
        if vent.puff.tick(delta.mul_f32(rush)).just_finished() {
            let force = vent.power * if erupting { 2.6 } else { 1.0 };
            let spread = if erupting { 46.0 } else { 12.0 };
            let from = plane.home + Vec2::new((fastrand::f32() - 0.5) * spread, 8.0);
            puff(&mut commands, from, plane.depth, force, vent.hot);
        }

        // --- o brilho ---
        let Some(mut sprite) = glow else {
            continue;
        };
        let pulse = (now * 2.6).sin() * 0.5 + 0.5;
        let heat = (warning + vent.blast.min(1.0) + pulse * 0.35).min(1.0);
        let tone = ((heat * HEAT.len() as f32) as usize).min(HEAT.len() - 1);
        if tone != vent.tone {
            vent.tone = tone;
            sprite.art = sprite.art.recolored(HEAT[tone]);
        }
    }
}

/// O estouro: clarao na boca e um leque de bombas de lava.
fn blast(commands: &mut Commands, crater: Vec2, depth: f32) {
    commands.spawn((
        LevelGeometry,
        Parallax {
            home: crater,
            depth,
        },
        AsciiSprite::new(AsciiArt::solid(BLAST_FLASH, palette::MAGMA)),
        Layer::Background,
        Transform::from_translation(crater.extend(-depth)),
        Lifetime(Timer::from_seconds(0.22, TimerMode::Once)),
    ));

    for i in 0..9 {
        // Leque aberto em torno da vertical: nenhuma bomba sai rasante, senao
        // ela cruza a arena na altura da briga e vira ruido no meio do jogo.
        let angle = std::f32::consts::FRAC_PI_2 + (i as f32 / 8.0 - 0.5) * 1.7;
        let speed = 190.0 + fastrand::f32() * 170.0;
        let at = crater + Vec2::new((fastrand::f32() - 0.5) * 30.0, 10.0);
        commands.spawn((
            LevelGeometry,
            Parallax { home: at, depth },
            LavaBomb {
                velocity: Vec2::from_angle(angle) * speed,
                trail: Timer::from_seconds(0.06, TimerMode::Repeating),
            },
            AsciiSprite::new(AsciiArt::glyph(
                if i % 2 == 0 { '*' } else { '°' },
                palette::MAGMA,
            )),
            Layer::Background,
            Transform::from_translation(at.extend(-depth)),
        ));
    }
}

/// Envelhece a fumaca: ela sobe perdendo forca, incha e apaga.
fn drift_smoke(
    time: Res<Time>,
    mut commands: Commands,
    mut puffs: Query<(Entity, &mut Smoke, &mut Parallax, &mut AsciiSprite)>,
) {
    let dt = time.delta_secs();
    for (entity, mut smoke, mut plane, mut sprite) in &mut puffs {
        smoke.age += dt;
        if smoke.age >= smoke.life {
            commands.entity(entity).despawn();
            continue;
        }
        plane.home.y += smoke.rise * dt;
        plane.home.x += smoke.drift * dt;
        // Fumaca que esfria perde o empuxo e passa a so vagar com o vento.
        smoke.rise *= 1.0 - dt * 0.6;
        smoke.drift += dt * 9.0;

        let frame = ((smoke.age / smoke.life * PUFF.len() as f32) as usize).min(PUFF.len() - 1);
        if frame != smoke.frame {
            smoke.frame = frame;
            let tone = if smoke.hot { SOOT[frame] } else { STEAM[frame] };
            sprite.art = AsciiArt::solid(PUFF[frame], tone);
        }
    }
}

/// Voa as bombas de lava e deixa rastro de brasa.
fn fly_bombs(
    time: Res<Time>,
    mut commands: Commands,
    mut bombs: Query<(Entity, &mut LavaBomb, &mut Parallax)>,
) {
    let dt = time.delta_secs();
    for (entity, mut bomb, mut plane) in &mut bombs {
        bomb.velocity.y -= BOMB_GRAVITY * dt;
        plane.home += bomb.velocity * dt;

        if plane.home.y < -ARENA_HALF_H + 40.0 || plane.home.x.abs() > ARENA_HALF_W + 100.0 {
            commands.entity(entity).despawn();
            continue;
        }
        if bomb.trail.tick(time.delta()).just_finished() {
            ember(&mut commands, plane.home, plane.depth);
        }
    }
}

/// Uma brasa parada que apaga sozinha -- o rastro das bombas.
fn ember(commands: &mut Commands, at: Vec2, depth: f32) {
    commands.spawn((
        LevelGeometry,
        Parallax { home: at, depth },
        AsciiSprite::new(AsciiArt::glyph(
            if fastrand::bool() { '·' } else { '°' },
            palette::EMBER,
        )),
        Layer::Background,
        Transform::from_translation(at.extend(-depth)),
        Lifetime(Timer::from_seconds(0.55, TimerMode::Once)),
    ));
}

// --- letreiros --------------------------------------------------------------

/// Letreiro de fundo que pulsa como neon velho.
#[derive(Component)]
struct NeonSign {
    bright: Color,
    dim: Color,
    speed: f32,
    phase: f32,
    lit: bool,
}

fn flicker_neon(time: Res<Time>, mut signs: Query<(&mut NeonSign, &mut AsciiSprite)>) {
    for (mut sign, mut sprite) in &mut signs {
        let wave = (time.elapsed_secs() * sign.speed + sign.phase).sin();
        let lit = wave > -0.82;
        if lit != sign.lit {
            sign.lit = lit;
            sprite.art = sprite
                .art
                .recolored(if lit { sign.bright } else { sign.dim });
        }
    }
}

// --- montagem ---------------------------------------------------------------

/// Predios do tema, com janelas acesas.
fn towers(commands: &mut Commands, skyline: &[Building], scene: Scene) {
    let window = match scene {
        Scene::City => palette::IRON,
        Scene::AcidWorks => palette::SCENE_TOXIC,
        Scene::Reactor => palette::SCENE_BLUE,
        _ => return,
    };

    // Os distantes nao tem janela acesa: e a falta dela, mais que o tamanho,
    // que os empurra para longe.
    if scene == Scene::City {
        for &(x, y, cols, rows) in &CITY_FAR {
            raise(
                commands,
                Panel::footed(
                    AsciiArt::fill('▓', cols, rows, palette::COAL),
                    Vec2::new(x, y + GROUND),
                    FAR,
                ),
            );
        }
    }

    for &(x, y, cols, rows) in skyline {
        let mut art = AsciiArt::fill('▓', cols, rows, palette::COAL);
        for row in (1..rows.saturating_sub(1)).step_by(2) {
            for col in (2..cols.saturating_sub(1)).step_by(4) {
                art = art.stamp(&AsciiArt::solid("·", window), col, row);
            }
        }
        raise(commands, Panel::footed(art, Vec2::new(x, y + GROUND), MID));
    }
}

/// O que balanca no fundo do tema: brasa, faisca, lanterna.
fn ambience(commands: &mut Commands, scene: Scene) {
    match scene {
        Scene::City => {
            // Luz de aeronave piscando sobre a antena mais alta.
            for (i, at) in [Vec2::new(470.0, 190.0), Vec2::new(-250.0, 145.0)]
                .into_iter()
                .enumerate()
            {
                spark(
                    commands,
                    '°',
                    at,
                    palette::BLOOD,
                    Sway {
                        phase: i as f32 * 2.1,
                        speed: 1.4,
                        travel: Vec2::new(0.0, 3.0),
                    },
                    FAR,
                );
            }
        }
        Scene::Caldera => {
            // Brasas subindo em torno da montanha.
            for i in 0..16 {
                let hot = i % 3 == 0;
                spark(
                    commands,
                    if hot { '*' } else { '·' },
                    Vec2::new(-560.0 + i as f32 * 74.0, -30.0 + (i % 5) as f32 * 44.0),
                    if hot {
                        palette::SCENE_FIRE
                    } else {
                        palette::ASH
                    },
                    Sway {
                        phase: i as f32 * 0.73,
                        speed: 0.45 + (i % 4) as f32 * 0.08,
                        travel: Vec2::new(20.0, 14.0),
                    },
                    MID,
                );
            }
        }
        Scene::MagmaBridge | Scene::ForgeCore => {
            for i in 0..12 {
                spark(
                    commands,
                    if i % 4 == 0 { '*' } else { '·' },
                    Vec2::new(-500.0 + i as f32 * 92.0, -80.0 + (i % 4) as f32 * 48.0),
                    palette::SCENE_FIRE,
                    Sway {
                        phase: i as f32 * 0.61,
                        speed: 0.62,
                        travel: Vec2::new(14.0, 24.0),
                    },
                    MID,
                );
            }
        }
        Scene::AcidWorks | Scene::Reactor => {
            for (i, x) in [-430.0, -115.0, 215.0, 470.0].into_iter().enumerate() {
                spark(
                    commands,
                    if i % 2 == 0 { 'o' } else { '°' },
                    Vec2::new(x, 34.0 + i as f32 * 15.0),
                    palette::ASH,
                    Sway {
                        phase: i as f32 * 1.4,
                        speed: 0.7 + i as f32 * 0.08,
                        travel: Vec2::new(10.0, 18.0),
                    },
                    FAR,
                );
            }
        }
        Scene::Drainage => {
            for (i, x) in [-420.0, -210.0, 15.0, 250.0, 460.0].into_iter().enumerate() {
                spark(
                    commands,
                    if i % 2 == 0 { '·' } else { '\'' },
                    Vec2::new(x, 155.0 - (i % 3) as f32 * 22.0),
                    palette::SCENE_BLUE,
                    Sway {
                        phase: i as f32,
                        speed: 0.36,
                        travel: Vec2::new(2.0, 18.0),
                    },
                    MID,
                );
            }
        }
        Scene::RedGate | Scene::SunsetPagoda => {
            // Lanternas penduradas, cada uma com a propria brasa dentro.
            for (i, x) in [-260.0, -120.0, 255.0, 405.0].into_iter().enumerate() {
                let at = Vec2::new(x, 22.0 + (i % 2) as f32 * 18.0);
                raise(
                    commands,
                    Panel::new(AsciiArt::solid(LANTERN, palette::BLOOD), at, MID),
                );
                spark(
                    commands,
                    if i % 2 == 0 { '•' } else { '·' },
                    at,
                    palette::SCENE_GOLD,
                    Sway {
                        phase: i as f32,
                        speed: 0.55,
                        travel: Vec2::new(6.0, 4.0),
                    },
                    MID,
                );
            }
        }
        Scene::DragonGarden => {
            for (i, x) in [-390.0, -180.0, 120.0, 360.0].into_iter().enumerate() {
                spark(
                    commands,
                    '·',
                    Vec2::new(x, 70.0 + (i % 2) as f32 * 45.0),
                    palette::SCENE_GOLD,
                    Sway {
                        phase: i as f32 * 1.7,
                        speed: 0.38,
                        travel: Vec2::new(22.0, 10.0),
                    },
                    FAR,
                );
            }
        }
    }
}

/// Levanta o fundo inteiro do tema.
///
/// Um lugar so monta tudo, na ordem de tras para frente. As fases nao spawnam
/// fundo por conta propria, entao nao existe mapa com uma serra na frente do
/// jogador porque alguem escreveu o `z` errado.
pub fn build(commands: &mut Commands, skyline: &[Building], signs: &[Sign], scene: Scene) {
    let theme = scene.theme();
    for panel in panels(scene) {
        raise(commands, panel);
    }
    towers(commands, skyline, scene);
    ambience(commands, scene);
    vents(commands, scene);
    seed_weather(commands, &weather(theme));

    for &(text, y, bright, phase) in signs {
        let at = Vec2::new(0.0, y);
        commands.spawn((
            LevelGeometry,
            Parallax {
                home: at,
                depth: NEAR,
            },
            NeonSign {
                bright,
                dim: palette::IRON,
                speed: 3.0,
                phase,
                lit: true,
            },
            AsciiSprite::new(AsciiArt::solid(text, bright)),
            Layer::Background,
            Transform::from_translation(at.extend(-NEAR)),
        ));
    }
}

/// Anima o fundo: profundidade, clima, fumaca e erupcao.
pub struct BackdropPlugin;

impl Plugin for BackdropPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Focus>().add_systems(
            Update,
            // Encadeados de proposito: tudo mexe em `Parallax::home`, e
            // `drift_planes` -- o unico que escreve `Transform` -- roda por
            // ultimo, com o quadro ja fechado.
            (
                track_focus,
                blow_weather,
                run_vents,
                drift_smoke,
                fly_bombs,
                flicker_neon,
                drift_planes,
            )
                .chain()
                .in_set(AppSet::Animate)
                .run_if(arena_live),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const THEMES: [Theme; 4] = [
        Theme::City,
        Theme::Volcano,
        Theme::Industrial,
        Theme::Oriental,
    ];
    const SCENES: [Scene; 10] = [
        Scene::City,
        Scene::Caldera,
        Scene::MagmaBridge,
        Scene::ForgeCore,
        Scene::AcidWorks,
        Scene::Reactor,
        Scene::Drainage,
        Scene::RedGate,
        Scene::SunsetPagoda,
        Scene::DragonGarden,
    ];

    /// A arte que nasce durante a partida, e por isso nao passa por [`scene`].
    fn loose_art() -> Vec<&'static str> {
        vec![CRATER_GLOW, BLAST_FLASH, LANTERN]
            .into_iter()
            .chain(PUFF)
            .collect()
    }

    /// Desenha a composicao parada de um tema num grid de texto.
    ///
    /// E o unico jeito de conferir fundo sem abrir o jogo: um `Panel` fora do
    /// lugar nao quebra nada, nao falha teste nenhum, e so aparece como uma
    /// montanha atravessada na tela.
    fn preview(scene: Scene) -> String {
        const COLS: usize = 160;
        const ROWS: usize = 30;
        let mut grid = vec![vec![' '; COLS]; ROWS];

        for panel in panels(scene) {
            let size = panel.art.size();
            let left = panel.at.x - size.x * 0.5;
            let top = if panel.foot {
                panel.at.y + size.y
            } else {
                panel.at.y + size.y * 0.5
            };
            for cell in &panel.art.cells {
                let x = ((left + cell.col as f32 * CELL.x + ARENA_HALF_W) / CELL.x).round();
                let y = ((ARENA_HALF_H - top + cell.row as f32 * CELL.y) / CELL.y).round();
                if x < 0.0 || y < 0.0 || x >= COLS as f32 || y >= ROWS as f32 {
                    continue;
                }
                grid[y as usize][x as usize] = '#';
            }
        }

        grid.into_iter()
            .map(|row| row.into_iter().collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// A silhueta do cone tem que ser um cone.
    ///
    /// Ela e gerada justamente porque desenhar treze linhas que alargam sempre
    /// o mesmo tanto e o tipo de coisa que sai com um degrau no meio -- e um
    /// degrau numa encosta le como erro de desenho, nao como rocha.
    #[test]
    fn o_cone_do_vulcao_alarga_sempre_igual() {
        let art = cone(palette::IRON, palette::EMBER);
        assert_eq!(art.rows, CONE_ROWS);

        let mut anterior = None;
        for row in 0..CONE_ROWS {
            let linha: Vec<u16> = art
                .cells
                .iter()
                .filter(|c| c.row == row)
                .map(|c| c.col)
                .collect();
            let largura = linha.iter().max().unwrap() - linha.iter().min().unwrap() + 1;
            let esperada = CONE_TOP + row * CONE_FLARE * 2;
            assert_eq!(largura, esperada, "linha {row} saiu do compasso");
            if let Some(antes) = anterior {
                assert!(largura > antes, "linha {row} nao alargou");
            }
            anterior = Some(largura);
        }
    }

    /// A boca tem que ficar aberta, e o brilho tem que caber exatamente nela.
    ///
    /// Uma celula de rocha sobrando ali desenha por cima da lava, e a montanha
    /// apaga sozinha.
    #[test]
    fn a_cratera_e_um_buraco_do_tamanho_do_brilho() {
        let art = cone(palette::IRON, palette::EMBER);
        let glow = AsciiArt::solid(CRATER_GLOW, palette::MAGMA);
        assert_eq!(glow.cols, CRATER_COLS, "o brilho nao cabe na boca");
        assert_eq!(glow.rows, CRATER_ROWS);

        for row in 0..CRATER_ROWS {
            let largura = CONE_TOP + row * CONE_FLARE * 2;
            let mouth = (largura - CRATER_COLS) / 2;
            let lead = (art.cols - largura) / 2;
            for col in mouth..mouth + CRATER_COLS {
                assert!(
                    !art.cells
                        .iter()
                        .any(|c| c.row == row && c.col == lead + col),
                    "a cratera esta tapada em {row},{col}"
                );
            }
        }
    }

    /// Duas celulas no mesmo lugar sao um carimbo fora do lugar.
    ///
    /// Elas desenham uma por cima da outra, no mesmo Z: o que aparece e sorteio
    /// da ordem de spawn, e o desenho muda sozinho entre uma partida e outra.
    #[test]
    fn nenhum_carimbo_pisa_em_cima_de_outro() {
        for scene in SCENES {
            for panel in panels(scene) {
                let mut lugares: Vec<(u16, u16)> =
                    panel.art.cells.iter().map(|c| (c.col, c.row)).collect();
                lugares.sort_unstable();
                let antes = lugares.len();
                lugares.dedup();
                assert_eq!(antes, lugares.len(), "{scene:?}: carimbo sobreposto");
            }
        }
    }

    /// Nada do fundo pode nascer enterrado.
    ///
    /// O terreno desenha na frente, entao uma peca inteira abaixo da linha do
    /// chao custa entidade e nunca aparece. Nenhum teste pegava isso porque
    /// nada quebra: o cano de escoamento da fabrica e o rio de lava do vulcao
    /// passaram a primeira versao inteira debaixo do piso.
    #[test]
    fn nenhuma_peca_nasce_enterrada() {
        for scene in SCENES {
            for panel in panels(scene) {
                let size = panel.art.size();
                let top = panel.at.y + if panel.foot { size.y } else { size.y * 0.5 };
                assert!(
                    top > GROUND,
                    "{scene:?}: peca de {size:?} em {:?} fica toda embaixo do chao",
                    panel.at
                );
            }
        }
    }

    /// O fundo nao pode nascer fora da tela.
    ///
    /// Uma peca centrada onde ninguem ve e trabalho jogado fora que nada
    /// denuncia -- ela simplesmente nao aparece.
    #[test]
    fn todo_plano_encosta_na_tela() {
        for scene in SCENES {
            for panel in panels(scene) {
                let size = panel.art.size();
                let low = panel.at.x - size.x * 0.5;
                let high = panel.at.x + size.x * 0.5;
                assert!(
                    high > -ARENA_HALF_W && low < ARENA_HALF_W,
                    "{scene:?}: peca de {size:?} em {:?} esta fora da tela",
                    panel.at
                );
            }
        }
    }

    /// Um plano largo tem que cobrir a tela mesmo no deslize maximo.
    ///
    /// Quem atravessa a tela e cenario de ponta a ponta: serra, viga, cano,
    /// chao derretido. Se ele tiver so a largura da tela, a briga andar para um
    /// canto abre um buraco de vazio no outro -- e isso acontece exatamente
    /// quando os dois jogadores estao no mesmo lado, que e quando alguem esta
    /// olhando para la.
    #[test]
    fn o_cenario_largo_cobre_a_tela_no_deslize_maximo() {
        for scene in SCENES {
            for panel in panels(scene) {
                let half = panel.art.size().x * 0.5;
                if half < ARENA_HALF_W * 0.5 {
                    continue;
                }
                let drift = REACH.x * panel.depth;
                assert!(
                    panel.at.x - half + drift <= -ARENA_HALF_W
                        && panel.at.x + half - drift >= ARENA_HALF_W,
                    "{scene:?}: plano de {} colunas em {:?} descobre a borda",
                    panel.art.cols,
                    panel.at
                );
            }
        }
    }

    /// Cada chamine tem que ter cano embaixo dela.
    #[test]
    fn a_fumaca_da_fabrica_sai_de_cima_dos_canos() {
        let art = AsciiArt::solid(REFINERY, palette::IRON);
        for dx in STACKS {
            let col = (dx / CELL.x + (art.cols as f32 - 1.0) * 0.5).round() as u16;
            assert!(
                art.cells.iter().any(|c| c.row == 0 && c.col == col),
                "a boca em {dx} fumega ao lado do cano, na coluna {col}"
            );
        }
    }

    /// Arte fora da CP437 vira interrogacao na tela, e o jogo roda assim mesmo.
    ///
    /// A varredura passa pela composicao ja montada, e nao pela lista de
    /// strings: assim ela cobre tambem o que e gerado -- serra, cone, predio --
    /// que e justamente o que ninguem pensa em conferir. Foi ela que pegou o
    /// `▟` da refinaria, um glifo que a IBM nunca desenhou.
    #[test]
    fn a_arte_do_fundo_cabe_na_cp437() {
        use crate::ascii::cp437::glyph_index;
        let fallback = glyph_index('?') as u8;

        for scene in SCENES {
            for panel in panels(scene) {
                assert!(
                    panel.art.cells.iter().all(|cell| cell.glyph != fallback),
                    "{scene:?}: peca em {:?} usa glifo fora da pagina",
                    panel.at
                );
            }
        }

        let solto: String = loose_art()
            .join("")
            .chars()
            .chain(weather_glyphs())
            .collect();
        for ch in solto.chars().filter(|c| *c != '\n') {
            assert_ne!(
                glyph_index(ch) as u8,
                fallback,
                "U+{:04X} {ch:?} nao existe na pagina",
                ch as u32
            );
        }
    }

    /// Fundo usa uma gama propria: as cores de jogador/perigo ficam livres
    /// para continuar legiveis mesmo quando a silhueta cruza um landmark.
    #[test]
    fn o_fundo_nao_rouba_as_cores_de_gameplay() {
        let reserved = [
            palette::BONE,
            palette::P1,
            palette::P2,
            palette::P3,
            palette::P4,
            palette::BLOOD,
            palette::GOLD,
            palette::MAGMA,
            palette::EMBER,
            palette::TOXIC,
            palette::MOSS,
        ];

        for scene in SCENES {
            for panel in panels(scene) {
                assert!(
                    panel
                        .art
                        .cells
                        .iter()
                        .all(|cell| !reserved.contains(&cell.color)),
                    "{scene:?}: landmark usa cor reservada ao gameplay"
                );
            }
        }
        for theme in THEMES {
            assert!(
                weather(theme)
                    .colors
                    .iter()
                    .all(|color| !reserved.contains(color)),
                "{theme:?}: clima usa cor reservada ao gameplay"
            );
        }
    }

    /// Os glifos de todo clima do jogo.
    fn weather_glyphs() -> Vec<char> {
        THEMES
            .into_iter()
            .flat_map(|theme| weather(theme).glyphs.to_vec())
            .collect()
    }

    /// Os dois veios de lava tem que descer inteiros.
    ///
    /// Eles saem de uma conta sobre a largura da linha, e uma conta que caia no
    /// bordo do cone perde a celula para a pedra: o veio some no meio da
    /// encosta e volta embaixo, como se a montanha piscasse.
    #[test]
    fn a_lava_desce_a_encosta_inteira() {
        let art = cone(palette::IRON, palette::EMBER);
        for row in CRATER_ROWS..CONE_ROWS {
            let brasas = art
                .cells
                .iter()
                .filter(|cell| cell.row == row && cell.color == palette::EMBER)
                .count();
            assert_eq!(brasas, VEINS.len(), "a lava sumiu na linha {row}");
        }
    }

    /// A serra desce ate a base: pico solto no ar nao e montanha.
    #[test]
    fn a_serra_e_macica_por_baixo() {
        let art = ridge(&[2, 5, 3], 24, palette::COAL);
        for col in 0..art.cols {
            let coluna: Vec<u16> = art
                .cells
                .iter()
                .filter(|c| c.col == col)
                .map(|c| c.row)
                .collect();
            if coluna.is_empty() {
                continue;
            }
            let topo = *coluna.iter().min().unwrap();
            assert_eq!(
                coluna.len() as u16,
                art.rows - topo,
                "a coluna {col} tem buraco"
            );
            assert!(coluna.contains(&(art.rows - 1)), "a coluna {col} flutua");
        }
    }

    /// A montanha tem que estourar sozinha, e o estouro tem que sair da boca.
    ///
    /// Composicao parada e o que os outros testes conferem; a erupcao e a
    /// unica coisa aqui que so existe em movimento. Sem este teste ela podia
    /// parar de acontecer -- um timer que nunca vira, uma bomba que nasce e
    /// morre no mesmo quadro -- com todos os outros verdes.
    #[test]
    fn o_vulcao_estoura_sozinho() {
        let mut app = App::new();
        app.init_resource::<Time>()
            .add_message::<Shake>()
            .add_systems(Startup, |mut commands: Commands| {
                vents(&mut commands, Scene::Caldera)
            })
            .add_systems(Update, (run_vents, drift_smoke, fly_bombs).chain());
        app.update();

        // Doze segundos em passos de um quadro: mais que o relogio da erupcao,
        // e sem um salto unico que faria a bomba nascer e sumir junto.
        for _ in 0..200 {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_millis(60));
            app.update();
        }

        let world = app.world_mut();
        let fumaca: Vec<Vec2> = world
            .query::<(&Smoke, &Parallax)>()
            .iter(world)
            .map(|(_, plane)| plane.home)
            .collect();
        assert!(!fumaca.is_empty(), "a cratera parou de fumegar");
        assert!(
            fumaca.iter().any(|at| at.y > CRATER.y + CELL.y),
            "nenhuma baforada subiu: a coluna nasce e fica parada na boca"
        );

        let world = app.world_mut();
        let bombas = world.query::<&LavaBomb>().iter(world).count();
        assert!(bombas > 0, "doze segundos e o vulcao nao cuspiu nada");
    }

    /// Um olho no fundo montado, quando alguem quiser conferir a composicao:
    /// `cargo test olhar_o_fundo -- --nocapture --ignored`.
    #[test]
    #[ignore = "so para olhar"]
    fn olhar_o_fundo() {
        for scene in SCENES {
            println!("\n=== {scene:?} ===\n{}", preview(scene));
        }
    }
}
