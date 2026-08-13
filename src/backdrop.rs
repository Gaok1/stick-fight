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

/// Uma peca solta que balanca no lugar: grou em voo, folha, faisca.
fn adrift(commands: &mut Commands, art: AsciiArt, at: Vec2, sway: Sway, depth: f32) {
    commands.spawn((
        LevelGeometry,
        Parallax { home: at, depth },
        sway,
        AsciiSprite::new(art),
        Layer::Background,
        Transform::from_translation(at.extend(-depth)),
    ));
}

/// Brasa, faisca ou lanterna: um glifo que balanca no lugar.
fn spark(commands: &mut Commands, glyph: char, at: Vec2, color: Color, sway: Sway, depth: f32) {
    adrift(commands, AsciiArt::glyph(glyph, color), at, sway, depth);
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

// --- geradores de cenario ---------------------------------------------------
//
// Tudo aqui e desenhado por conta, e nao escrito a mao, pelo mesmo motivo que a
// serra e o cone ja eram: as formas grandes do fundo sao repeticao com variacao
// -- coluna de basalto, andar de pagode, arco de galeria, escama de dragao. Uma
// parede de vinte linhas por quarenta colunas escrita caractere a caractere sai
// com uma junta a menos no meio, e o que denuncia nao e a junta: e a linha da
// crista, que deixa de ser uma curva so.

/// Rampa de densidade padrao, do vazio ao macico.
///
/// A ordem importa: os geradores escolhem por indice, entao trocar a rampa
/// muda o material inteiro de uma vez.
const RAMP: [char; 4] = ['░', '▒', '▓', '█'];

/// Escolhe o degrau da rampa para uma densidade em `0..1`.
fn shade(density: f32) -> char {
    match density {
        d if d >= 0.80 => RAMP[3],
        d if d >= 0.58 => RAMP[2],
        d if d >= 0.36 => RAMP[1],
        d if d >= 0.16 => RAMP[0],
        _ => ' ',
    }
}

/// Monta uma arte celula a celula a partir de uma funcao `(col, row) -> char`.
///
/// Todo gerador daqui repetia as mesmas oito linhas de `map`/`collect`/`join`,
/// e era nelas -- nao no desenho -- que nascia o erro de uma coluna a mais.
fn draw(
    cols: u16,
    rows: u16,
    map: &'static [(char, Color)],
    fallback: Color,
    mut cell: impl FnMut(u16, u16) -> char,
) -> AsciiArt {
    let text = (0..rows)
        .map(|row| (0..cols).map(|col| cell(col, row)).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    AsciiArt::tinted(&text, map, fallback)
}

/// Uma cortina de liquido despencando, num quadro do ciclo.
///
/// Os veios saem por coluna e o padrao anda para baixo com o quadro: e o que
/// faz a queda escorrer em vez de piscar. O pe se desmancha em respingo, porque
/// queda com a base reta le como fita colada na parede.
fn cascade(cols: u16, rows: u16, frame: u16, map: &'static [(char, Color)]) -> AsciiArt {
    draw(cols, rows, map, palette::COAL, |col, row| {
        let vein = (col as f32 * 1.9).sin() * 0.32 + 0.62;
        let flow = ((row + frame) as f32 * 0.85 + col as f32 * 2.4).sin() * 0.28;
        let spray = 1.0 - (row as f32 / rows.max(2) as f32).powi(3) * 0.85;
        shade((vein + flow) * spray)
    })
}

/// Parede de basalto: colunas verticais com junta, fratura e topo quebrado.
fn basalt(cols: u16, rows: u16, map: &'static [(char, Color)]) -> AsciiArt {
    draw(cols, rows, map, palette::COAL, |col, row| {
        // A parede nao termina numa reta: a crista sobe e desce com a coluna.
        let crest = (col as f32 * 0.53).sin() * 1.3 + (col as f32 * 0.17).cos() * 1.7 + 2.6;
        let depth = row as f32 - crest;
        match depth {
            d if d < -0.5 => ' ',
            d if d < 0.5 => '▄',
            _ if col.is_multiple_of(5) => '▓',
            _ if (row * 7 + col * 3) % 23 == 0 => '▒',
            _ => '█',
        }
    })
}

/// Ponte pensil arruinada: torres, cabo que peca e tabuado partido no meio.
///
/// O vao no meio nao e enfeite -- e o que diz que essa travessia ja caiu, e
/// que a que os jogadores estao pisando e a ultima que sobrou.
fn suspension(cols: u16, gap: u16, map: &'static [(char, Color)]) -> AsciiArt {
    const ROWS: u16 = 9;
    let deck = ROWS - 1;
    let (left, right) = (3u16, cols.saturating_sub(4));
    let void = |col: u16| (col as i32 - cols as i32 / 2).unsigned_abs() as u16 * 2 < gap;

    draw(cols, ROWS, map, palette::IRON, |col, row| {
        if col == left || col == right {
            return if row == 0 { '╥' } else { '║' };
        }
        if void(col) {
            return ' ';
        }
        if row == deck {
            return if col < left || col > right {
                ' '
            } else {
                '═'
            };
        }
        if col < left || col > right {
            return ' ';
        }
        // Cabo: parabola presa no topo das duas torres, pecando no meio do vao.
        let u = (col - left) as f32 / (right - left).max(1) as f32;
        let sag = 1.0 + (deck - 3) as f32 * 4.0 * u * (1.0 - u);
        if row == sag.round() as u16 {
            return match u {
                u if u < 0.45 => '\\',
                u if u > 0.55 => '/',
                _ => '_',
            };
        }
        // Pendural: so entre o cabo e o tabuado, senao ele fura a ponte.
        if (col - left) % 4 == 2 && row as f32 > sag && row < deck {
            return '│';
        }
        ' '
    })
}

/// Tanque de pressao: casco abaulado, cintas e visor de nivel.
fn vat(cols: u16, rows: u16, map: &'static [(char, Color)]) -> AsciiArt {
    let glass = cols / 2;
    draw(cols, rows, map, palette::IRON, |col, row| {
        // O casco fecha em abobada: duas linhas de recuo no topo.
        let inset = match row {
            0 => 2,
            1 => 1,
            _ => 0,
        };
        if col < inset || col + inset >= cols {
            return ' ';
        }
        if row == 0 {
            return '▄';
        }
        if col == glass && row > 1 {
            // Visor: a coluna de liquido some antes do topo, entao o tanque
            // le como meio cheio em vez de solido.
            return if row * 3 > rows * 2 { '▒' } else { '░' };
        }
        if row % 4 == 3 {
            return '═';
        }
        if col == inset || col + inset + 1 == cols {
            return '▓';
        }
        '█'
    })
}

/// Corrida de canos com valvulas, manometros e pernas de descida.
fn pipeline(span: u16, map: &'static [(char, Color)]) -> AsciiArt {
    draw(span, 3, map, palette::IRON, |col, row| {
        let (valve, leg) = (col % 13 == 4, col % 13 == 10);
        match row {
            0 if valve => '○',
            1 if valve => '╪',
            1 if leg => '╤',
            1 => '═',
            2 if leg => '║',
            _ => ' ',
        }
    })
}

/// Galeria de arcos: o subsolo lido de uma vez so.
///
/// O arco e meia-circunferencia de verdade, e nao dois riscos e um traco: e a
/// curva que separa galeria de porta quadrada.
fn arcade(bays: u16, bay: u16, rows: u16, map: &'static [(char, Color)]) -> AsciiArt {
    let peak = (rows as f32 - 1.5).max(1.0);
    draw(bays * bay, rows, map, palette::IRON, |col, row| {
        let u = (col % bay) as f32 / (bay - 1).max(1) as f32 * 2.0 - 1.0;
        // Fora do vao e pilar macico; dentro, a altura do arco vem do circulo.
        let open = if u.abs() < 0.68 {
            peak * (1.0 - (u / 0.68).powi(2)).max(0.0).sqrt()
        } else {
            0.0
        };
        let mouth = rows as f32 - 1.0 - open;
        match row as f32 {
            r if r > mouth => ' ',
            r if r > mouth - 1.0 => '▓',
            0.0 => '▄',
            _ if (row * 5 + col * 3) % 19 == 0 => '▒',
            _ => '█',
        }
    })
}

/// Pagode de `tiers` andares: telhado que abre, corpo que estreita.
///
/// Cada andar sao tres linhas -- ponta de beiral, telha e corpo -- e a de cima
/// e sempre a mais estreita. Escrito a mao, o quarto telhado sai uma coluna
/// menor que o terceiro e o predio inteiro entorta sem ninguem saber por que.
fn pagoda(tiers: u16, map: &'static [(char, Color)]) -> AsciiArt {
    const TOP_ROOF: u16 = 11;
    const ROOF_STEP: u16 = 5;
    const BODY_INSET: u16 = 3;

    let widest = TOP_ROOF + (tiers - 1) * ROOF_STEP;
    let mut text = String::new();
    let pad = |n: u16| " ".repeat(n as usize);

    // Pinaculo: a joia e a haste que fecham o predio por cima.
    text.push_str(&format!("{}○\n{}║\n", pad(widest / 2), pad(widest / 2)));

    for tier in 0..tiers {
        let roof = TOP_ROOF + tier * ROOF_STEP;
        let body = roof - BODY_INSET * 2;
        let side = (widest - roof) / 2;

        // Beiral virado na propria linha da telha: `▀` desenha na metade de
        // cima da celula e `▄` na de baixo, entao a ponta sobe meia celula sem
        // gastar linha nenhuma. Numa linha propria acima, a ponta descolava do
        // telhado e virava dois riscos soltos no ceu.
        text.push_str(&format!(
            "{}▀{}▀\n",
            pad(side),
            "▄".repeat(roof.saturating_sub(2) as usize)
        ));
        // Os andares de baixo sao mais altos: pagode com todos os corpos da
        // mesma altura le como bolo de casamento, nao como torre.
        for _ in 0..1 + tier / 2 {
            text.push_str(&format!(
                "{}{}\n",
                pad(side + BODY_INSET),
                (0..body)
                    .map(|col| match col as i32 - body as i32 / 2 {
                        0 => '╬',
                        -1 | 1 => '│',
                        _ => '█',
                    })
                    .collect::<String>()
            ));
        }
    }
    AsciiArt::tinted(text.trim_end(), map, palette::IRON)
}

/// Portao de dois pilares e duas travessas, com a de cima virada nas pontas.
fn gate(cols: u16, rows: u16, map: &'static [(char, Color)]) -> AsciiArt {
    let pillar = |col: u16| {
        let inner = cols / 5;
        (inner..inner + 3).contains(&col) || (cols - inner - 3..cols - inner).contains(&col)
    };
    draw(cols, rows, map, palette::SCENE_RED, |col, row| match row {
        // Kasagi: a viga de cima, com as duas pontas levantadas.
        0 => {
            if col < 3 || col + 3 >= cols {
                '▀'
            } else {
                '▄'
            }
        }
        1 => '█',
        // Shimaki: a segunda viga, recuada, que da espessura ao topo.
        2 => {
            if col < 4 || col + 4 >= cols {
                ' '
            } else {
                '▄'
            }
        }
        // Nuki: a travessa que amarra os pilares.
        4 => {
            let inner = cols / 5;
            if (inner..cols - inner).contains(&col) {
                '▄'
            } else {
                ' '
            }
        }
        // Gakuzuka: o pilarete curto entre as duas travessas.
        3 if col == cols / 2 => '║',
        _ if pillar(col) => '█',
        _ => ' ',
    })
}

/// Pilar de jade com dragao em relevo: a espiral sobe uma coluna por linha.
fn jade_pillar(rows: u16, map: &'static [(char, Color)]) -> AsciiArt {
    const COLS: u16 = 5;
    draw(COLS, rows, map, palette::SCENE_JADE, |col, row| {
        if row == 0 || row + 1 == rows {
            return '▄';
        }
        if col == 0 || col + 1 == COLS {
            return '▓';
        }
        // O relevo anda de lado a cada linha: e a espiral que sobe o fuste.
        if col == 1 + (row % 3) {
            return '▒';
        }
        '█'
    })
}

/// Disco ou anel desenhado no grid retangular da fonte.
///
/// A celula e 8x16: um circulo escrito a mao com o mesmo numero de colunas e
/// linhas sai um ovo deitado, e nao ha correcao possivel depois -- ou se conta
/// a proporcao ou se desenha errado. Aqui o raio vertical ja entra corrigido,
/// entao a lua e redonda na tela, nao no arquivo.
fn disc(radius: u16, hollow: bool, map: &'static [(char, Color)]) -> AsciiArt {
    let squash = CELL.y / CELL.x;
    let ry = (radius as f32 / squash).round().max(1.0);
    let (cols, rows) = (radius * 2 + 1, ry as u16 * 2 + 1);

    draw(cols, rows, map, palette::ASH, |col, row| {
        let dx = col as f32 - radius as f32;
        let dy = (row as f32 - ry) * squash;
        let d = dx.hypot(dy) / radius as f32;
        match (hollow, d) {
            (_, d) if d > 1.04 => ' ',
            // Anel: so a casca, e ela engrossa para dentro.
            (true, d) if d < 0.70 => ' ',
            (true, d) => shade(1.0 - (d - 0.87).abs() * 3.4),
            // Disco: denso no meio, esfumado na borda -- volume, nao recorte.
            // A rampa tem que cruzar os quatro degraus dentro do raio, senao o
            // sol sai chapado em dois tons e vira um circulo pintado.
            (false, d) => shade(1.62 - d * 1.32),
        }
    })
}

/// Faixa de liquido correndo na horizontal, num quadro do ciclo.
///
/// Duas frequencias incomensuraveis, e nao uma: com uma so, o rio atravessa a
/// tela repetindo o mesmo desenho a cada quinze colunas, e listra regular numa
/// faixa de mil e seiscentas unidades e a primeira coisa que o olho pega.
fn current(span: u16, rows: u16, frame: u16, map: &'static [(char, Color)]) -> AsciiArt {
    draw(span, rows, map, palette::COAL, |col, row| {
        let t = (col + frame * 2) as f32;
        let wave = (t * 0.42 + row as f32 * 2.1).sin() * 0.68 + (t * 0.17).sin() * 0.32;
        match wave {
            w if w > 0.62 => '≈',
            w if w > 0.05 => '~',
            w if w > -0.55 => '-',
            _ => '_',
        }
    })
}

/// Alto-forno: massa que abre para baixo, com a boca em arco e o nucleo aceso.
fn furnace(cols: u16, rows: u16, map: &'static [(char, Color)]) -> AsciiArt {
    let mouth_w = cols as f32 * 0.26;
    let mouth_h = (rows as f32 - 2.0) * 0.55;

    draw(cols, rows, map, palette::IRON, |col, row| {
        // O corpo estreita para cima: forno com a parede reta le como caixa.
        let inset = ((rows - 1 - row) as f32 * 0.5).round() as u16;
        if col < inset || col + inset >= cols {
            return ' ';
        }
        // A boca e o mesmo arco da galeria, so que cheia de fogo.
        let u = (col as f32 - (cols as f32 - 1.0) * 0.5) / mouth_w;
        let arch = if u.abs() < 1.0 {
            mouth_h * (1.0 - u * u).sqrt()
        } else {
            0.0
        };
        let lip = rows as f32 - 1.0 - arch;
        match row as f32 {
            r if r > lip => {
                if row.is_multiple_of(2) {
                    '≈'
                } else {
                    '~'
                }
            }
            r if r > lip - 1.0 => '▓',
            0.0 => '▄',
            _ if (row * 5 + col * 7) % 17 == 0 => '▒',
            _ => '█',
        }
    })
}

/// O corpo do dragao: uma serpente que ondula, desenhada por espinha e volume.
///
/// Gerada, e nao escrita: o que denuncia um dragao ASCII feito a mao nao e a
/// escama, e a espinha -- basta uma linha fora da curva para o bicho deixar de
/// ser um corpo so e virar tres pedacos empilhados. Aqui a espinha e uma
/// senoide por construcao, e o volume sai da distancia ate ela.
fn serpent(span: u16, rows: u16, map: &'static [(char, Color)]) -> AsciiArt {
    let middle = (rows as f32 - 1.0) * 0.5;
    draw(span, rows, map, palette::SCENE_JADE, |col, row| {
        let t = col as f32 / (span - 1).max(1) as f32;
        // Duas ondas somadas: uma so daria uma cobra de brinquedo, sempre
        // igual de um lado ao outro.
        let spine = middle
            + (t * 7.4).sin() * (rows as f32 * 0.26)
            + (t * 3.1 + 1.2).sin() * (rows as f32 * 0.12);
        // Engrossa ate a ponta em que a cabeca encaixa, e afina ate o rabo. O
        // lado grosso e `t = 1` porque e ali que o pescoco entra.
        let girth = 2.9 * t.powf(0.55) + 0.45;
        let reach = (row as f32 - spine).abs() / girth;

        // Crista dorsal: fica logo acima do lombo, entao ela recorta a
        // silhueta contra o ceu em vez de sumir dentro do corpo.
        if col % 3 == 1 && (row as f32 - (spine - girth - 0.7)).abs() < 0.5 {
            return '▲';
        }
        match reach {
            r if r < 0.34 => '█',
            r if r < 0.64 => '▓',
            r if r < 0.87 => '▒',
            r if r <= 1.0 => '░',
            _ => ' ',
        }
    })
}

/// Onde os tres tanques da fabrica se apoiam.
const VAT_FOOT: f32 = GROUND + 30.0;
/// Largura de um tanque de processo, em celulas.
const VAT_COLS: u16 = 13;

/// Eixo e altura de cada tanque -- e, por tabela, de cada chamine.
///
/// Alturas diferentes de proposito: tres tanques do mesmo tamanho, igualmente
/// espacados, leem como icone repetido tres vezes, nao como patio.
///
/// As bocas de fumaca sao entidades a parte da arte. Enquanto os dois foram
/// numeros escritos em lugares diferentes, o vapor subia de tres pontos do ceu
/// ao lado dos canos e a arte continuava intacta: nada para um teste pegar, e
/// obvio na tela.
const STACKS: [(f32, u16); 3] = [(-190.0, 12), (0.0, 10), (190.0, 14)];

/// Altura da boca de uma chamine, tirada da propria altura do tanque.
fn stack_top(rows: u16) -> f32 {
    VAT_FOOT + rows as f32 * CELL.y
}

/// Placa de risco da fabrica: o unico texto do cenario, e por isso ele conta.
const PLACARD: &str = "╔════════════╗
║  TOX-03 !! ║
║ CORROSIVE  ║
╚════════════╝";

// --- materiais --------------------------------------------------------------
//
// Cada tabela e um material: a mesma rampa de densidade lida como basalto,
// como jade ou como acido dependendo so de qual delas pinta. E o que deixa um
// gerador servir tres temas sem virar tres geradores.

/// Rocha fria: basalto, penhasco, muro de pedra.
const STONE: [(char, Color); 5] = [
    ('▄', palette::ASH),
    ('█', palette::IRON),
    ('▓', palette::COAL),
    ('▒', palette::COAL),
    ('░', palette::COAL),
];
/// Aco de fabrica, com a ferrugem nas juntas.
const STEEL: [(char, Color); 10] = [
    ('▄', palette::ASH),
    ('█', palette::IRON),
    ('▓', palette::COAL),
    ('▒', palette::SCENE_RUST),
    ('░', palette::COAL),
    ('═', palette::SCENE_RUST),
    ('║', palette::IRON),
    ('╪', palette::ASH),
    ('╤', palette::ASH),
    ('○', palette::SCENE_TOXIC),
];
/// Queda de magma, do nucleo claro a borda apagada.
const MAGMA_FALL: [(char, Color); 4] = [
    ('█', palette::SCENE_GOLD),
    ('▓', palette::SCENE_FIRE),
    ('▒', palette::SCENE_RED),
    ('░', palette::SCENE_CINDER),
];
/// Rio de lava correndo na linha do horizonte.
const MAGMA_FLOW: [(char, Color); 4] = [
    ('≈', palette::SCENE_GOLD),
    ('~', palette::SCENE_FIRE),
    ('-', palette::SCENE_RED),
    ('_', palette::SCENE_CINDER),
];
/// Rocha do forno: massa fria com a boca acesa.
const FURNACE: [(char, Color); 6] = [
    ('▄', palette::ASH),
    ('█', palette::IRON),
    ('▓', palette::SCENE_CINDER),
    ('▒', palette::COAL),
    ('≈', palette::SCENE_GOLD),
    ('~', palette::SCENE_FIRE),
];
/// Queda de acido.
const ACID_FALL: [(char, Color); 4] = [
    ('█', palette::SCENE_ACID),
    ('▓', palette::SCENE_TOXIC),
    ('▒', palette::SCENE_JADE),
    ('░', palette::COAL),
];
/// Canal de esgoto correndo no fundo da galeria.
const ACID_FLOW: [(char, Color); 4] = [
    ('≈', palette::SCENE_ACID),
    ('~', palette::SCENE_TOXIC),
    ('-', palette::SCENE_JADE),
    ('_', palette::COAL),
];
/// Queda de refrigerante do reator: fria onde a de magma e quente.
const COOLANT_FALL: [(char, Color); 4] = [
    ('█', palette::ASH),
    ('▓', palette::SCENE_BLUE),
    ('▒', palette::IRON),
    ('░', palette::COAL),
];
/// Casco de tanque, com o visor de nivel esverdeado.
const TANK_SKIN: [(char, Color); 6] = [
    ('▄', palette::ASH),
    ('█', palette::IRON),
    ('▓', palette::COAL),
    ('═', palette::SCENE_RUST),
    ('▒', palette::SCENE_ACID),
    ('░', palette::SCENE_TOXIC),
];
/// Jade: massa profunda com a borda acesa, que e como pedra translucida le.
///
/// A rampa e invertida de proposito. Em rocha opaca o miolo e o que pega luz;
/// no jade e a casca fina que acende, e e essa inversao -- e so ela -- que
/// separa uma escultura de jade de uma escultura de granito pintada de verde.
const JADE_SKIN: [(char, Color); 11] = [
    ('█', palette::SCENE_JADE),
    ('▓', palette::SCENE_JADE),
    ('▒', palette::SCENE_JADE_LIT),
    ('░', palette::SCENE_JADE_LIT),
    ('▄', palette::SCENE_JADE_LIT),
    ('▀', palette::SCENE_JADE_LIT),
    ('▲', palette::SCENE_GOLD),
    ('•', palette::SCENE_GOLD),
    ('~', palette::SCENE_JADE_LIT),
    ('\\', palette::SCENE_GOLD),
    ('/', palette::SCENE_GOLD),
];
/// Telhado de telha e madeira lacada do pagode.
const TIMBER: [(char, Color); 7] = [
    ('▄', palette::SCENE_RED),
    ('▀', palette::SCENE_RED),
    ('█', palette::SCENE_GOLD),
    ('│', palette::SCENE_FIRE),
    ('╬', palette::SCENE_FIRE),
    ('○', palette::SCENE_GOLD),
    ('║', palette::SCENE_GOLD),
];
/// Laca vermelha do portao, com as ferragens douradas.
const LACQUER: [(char, Color); 4] = [
    ('█', palette::SCENE_RED),
    ('▄', palette::SCENE_RED),
    ('▀', palette::SCENE_FIRE),
    ('║', palette::SCENE_GOLD),
];
/// Sol baixo do poente.
const SUNSET: [(char, Color); 4] = [
    ('█', palette::SCENE_RED),
    ('▓', palette::SCENE_RED),
    ('▒', palette::SCENE_FIRE),
    ('░', palette::SCENE_FIRE),
];
/// Lua: pedra fria, com o mar de sombra nos degraus de baixo.
const LUNAR: [(char, Color); 4] = [
    ('█', palette::ASH),
    ('▓', palette::IRON),
    ('▒', palette::IRON),
    ('░', palette::COAL),
];
/// Anel de contencao do reator.
const CONTAINMENT: [(char, Color); 4] = [
    ('█', palette::IRON),
    ('▓', palette::SCENE_BLUE),
    ('▒', palette::SCENE_BLUE),
    ('░', palette::COAL),
];
/// Nucleo do reator, do repouso ao pico de carga.
const CORE_HEAT: [(char, Color); 4] = [
    ('█', palette::SCENE_BLUE),
    ('▓', palette::SCENE_TOXIC),
    ('▒', palette::SCENE_ACID),
    ('░', palette::SCENE_BLUE),
];

/// Galho de cerejeira entrando pelo canto de cima.
const BRANCH: &str = "▄▄▄▄▄▄▄▄▄▄
 ░ ▀▀▄▄ ░ ▀▀▄▄▄
░ ░  ░ ▀▄  ░  ▀▀▄
      ░  ░ ░";

/// Lanterna de papel, com o miolo vazado: a brasa que mora ali dentro e uma
/// entidade a parte, que balanca sozinha.
const LANTERN: &str = "┌─┐\n│ │\n└─┘";

/// Escadaria de pedra: degraus que abrem para baixo.
fn terrace(steps: u16, top: u16, spread: u16, map: &'static [(char, Color)]) -> AsciiArt {
    let widest = top + (steps - 1) * spread * 2;
    draw(widest, steps, map, palette::IRON, |col, row| {
        let half = (top + row * spread * 2) / 2;
        let middle = (widest - 1) / 2;
        if col.abs_diff(middle) > half {
            ' '
        } else if row == 0 || col.abs_diff(middle) > (top + (row - 1) * spread * 2) / 2 {
            '▄'
        } else {
            '█'
        }
    })
}

/// Bambuzal: colmos com no, alturas diferentes e folhagem nos nos.
fn bamboo(stalks: u16, rows: u16, map: &'static [(char, Color)]) -> AsciiArt {
    let head = |stalk: u16| (stalk * 5 % 4) + 1;
    // Folha so nos nos, e so em um no a cada dois: em todos, o bambuzal fecha
    // numa cerca viva e os colmos somem dentro dela.
    let leafy = |stalk: u16, row: u16| {
        row >= head(stalk) && (row - head(stalk)).is_multiple_of(4) && (row / 4).is_multiple_of(2)
    };
    draw(stalks * 4, rows, map, palette::SCENE_JADE, |col, row| {
        let stalk = col / 4;
        match col % 4 {
            // O colmo: no a cada quatro linhas, que e o que separa bambu de
            // vareta.
            1 if row >= head(stalk) => {
                if (row - head(stalk)).is_multiple_of(4) {
                    '╪'
                } else {
                    '║'
                }
            }
            0 if leafy(stalk, row) => '\\',
            2 if leafy(stalk, row) => '/',
            _ => ' ',
        }
    })
}

/// Teto de galeria: laje macica com dente pendurado.
///
/// A borda de baixo nunca sai reta -- e ela, e nao a laje, que faz o teto ler
/// como pedra escavada em vez de faixa preta colada no topo da tela.
fn vault(span: u16, rows: u16, map: &'static [(char, Color)]) -> AsciiArt {
    draw(span, rows, map, palette::COAL, |col, row| {
        let hang = (col as f32 * 0.31).sin() * 1.5 + (col as f32 * 0.11).cos() * 1.7 + 2.2;
        match row as f32 {
            r if r < 1.0 => '█',
            r if r < hang => {
                if col.is_multiple_of(2) {
                    '▓'
                } else {
                    '█'
                }
            }
            r if r < hang + 0.65 => '▀',
            _ => ' ',
        }
    })
}

/// A cabeca do dragao de jade.
///
/// A unica peca grande deste arquivo desenhada a mao, e por um motivo: cabeca
/// e gesto, nao repeticao. O que a faz ler como dragao -- e nao como cobra com
/// chifre -- sao tres coisas que nenhuma senoide entrega: os chifres partindo
/// do craneo para tras, a boca aberta com o vazio entre as duas mandibulas, e
/// o bigode escorrendo do focinho. O corpo, esse sim, e gerado.
const DRAGON_HEAD: &str = "              ▲     ▲
         ▲ ▄▄▄█▄▄▄▄▄█▄
      ▄▄▓███████████▀▀
   ▄▄▓██████████████▄
 ░▒▓███████•█████████▄▄
▒▓█████████████▀▀▀▀▀▀▀▀▀
░▒▓██████████▄
 ░▒▓████████▀ ▄▄▄▄▄▄▄▄▄
  ░▒▓█████▀   ▀▀▀▀▀▀▀
   ░~~~~ ~~~            ";

/// Malho da forja: a haste guia em cima, a massa embaixo.
const HAMMER: &str = "    ████
    ████
▄██████████▄
████████████
▀██████████▀";

/// Bigorna que recebe a pancada.
const ANVIL: &str = " ▄▄▄▄▄▄▄▄▄▄▄▄▄
▐█████████████▌
 ▀▀▀▀██████▀▀▀▀
     ██████
  ▄▄██████████▄▄";

/// Lanterna de pedra do jardim: chapeu, caixa de luz, fuste e base.
const STONE_LANTERN: &str = "   ▄▄▄▄▄
  ▐█████▌
  ▄▀▀▀▀▀▄
 ▐░░███░░▌
  ▀▄▄▄▄▄▀
    ███
   ▄███▄
  ███████";

/// Grou em voo: corpo e duas asas abertas, o bastante a essa distancia.
const CRANE: &str = " ▄ \n▀ ▀";

/// Agua do jardim: jade correndo, nao lodo.
const JADE_FLOW: [(char, Color); 4] = [
    ('≈', palette::SCENE_JADE_LIT),
    ('~', palette::SCENE_JADE),
    ('-', palette::SCENE_JADE),
    ('_', palette::COAL),
];

// --- onde as pecas grandes moram --------------------------------------------
//
// Landmark que anda -- malho, sopro, nucleo -- precisa do mesmo ponto em dois
// lugares: a composicao parada, que desenha o que fica em volta dele, e o
// numero, que o move. Enquanto os dois foram numeros digitados em funcoes
// diferentes, mexer na bigorna deixava o malho batendo no ar ao lado dela.

/// Onde o malho da forja descansa, no alto dos trilhos.
const HAMMER_AT: Vec2 = Vec2::new(250.0, GROUND + 200.0);
/// Quanto ele desce ate encostar na bigorna.
const HAMMER_DROP: f32 = 80.0;

/// Centro do anel de contencao do reator.
const CORE_AT: Vec2 = Vec2::new(0.0, 30.0);

/// Centro do corpo do dragao e da cabeca dele.
///
/// Os dois se encostam coluna com coluna: a cabeca ocupa ate x = -134 e o
/// corpo comeca exatamente ali. A altura tambem e casada -- a espinha da
/// senoide chega na linha em que o pescoco sai da cabeca -- e por isso o bicho
/// le como um corpo so, e nao como duas pecas empilhadas.
const DRAGON_BODY: Vec2 = Vec2::new(98.0, 68.0);
const DRAGON_HEAD_AT: Vec2 = Vec2::new(-230.0, 74.0);
/// A boca, de onde sai o sopro. Sai do mesmo desenho: coluna 3 da arte
/// espelhada, linha 6.
const DRAGON_MAW: Vec2 = Vec2::new(-298.0, 50.0);
/// Para onde o sopro vai, e ate onde.
const BREATH_DIR: Vec2 = Vec2::new(-0.82, -0.57);
const BREATH_REACH: f32 = 340.0;

// --- a composicao de cada tema ----------------------------------------------

/// Serra distante do vulcao. Os numeros sao alturas em linhas.
const VOLCANO_RIDGE: [u16; 15] = [2, 5, 3, 8, 4, 6, 3, 9, 5, 3, 7, 4, 8, 3, 2];
/// Borda interna da caldeira: mais perto, mais alta, e ja morna.
///
/// Duas bandas de serra em profundidades diferentes sao o que faz a montanha
/// ler como cordilheira. Com uma so, o cone fica num palco vazio.
const CALDERA_RIM: [u16; 11] = [3, 8, 5, 11, 6, 4, 9, 5, 10, 4, 3];
/// Cristas partidas do desfiladeiro.
const CHASM_RIDGE: [u16; 11] = [3, 7, 4, 11, 6, 2, 8, 3, 9, 4, 3];
/// Serra do tema oriental: mais baixa e mais longa, para o sol ficar por cima.
const EAST_RIDGE: [u16; 13] = [2, 4, 7, 3, 5, 9, 4, 6, 3, 8, 4, 5, 2];
/// Morros suaves do jardim, atras do dragao.
const GARDEN_RIDGE: [u16; 9] = [2, 5, 3, 4, 2, 6, 3, 5, 2];
/// Colinas do portao, quase planas: o gesto ali e o portao, nao a paisagem.
const GATE_RIDGE: [u16; 8] = [2, 3, 5, 4, 6, 3, 5, 2];
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
            // A borda de dentro da caldeira, entre a serra e o cone: sem esta
            // faixa morna a montanha ficava recortada contra o nada.
            Panel::footed(
                ridge(&CALDERA_RIM, SPAN, palette::SCENE_CINDER),
                Vec2::new(0.0, GROUND + 16.0),
                FAR,
            ),
            Panel::footed(cone(palette::IRON, palette::SCENE_FIRE), VOLCANO_FOOT, MID),
            // Chao derretido correndo na linha do horizonte, na frente da
            // montanha: e o que deixa claro que a arena inteira esta dentro da
            // caldeira, e nao so olhando para ela de longe.
            Panel::footed(
                current(SPAN, 2, 0, &MAGMA_FLOW),
                Vec2::new(0.0, GROUND),
                NEAR,
            ),
            // Lascas de obsidiana emoldurando o poco. Precisam passar da
            // altura da serra para existirem: uma lasca da altura dela e uma
            // peca inteira que se dissolve no fundo sem ninguem notar. A da
            // direita vai espelhada -- o gerador e determinista, e duas copias
            // identicas nas duas pontas da tela leem como moldura de cartaz.
            Panel::footed(basalt(10, 13, &STONE), Vec2::new(-575.0, GROUND), NEAR),
            Panel::footed(
                basalt(10, 13, &STONE).mirrored(),
                Vec2::new(575.0, GROUND),
                NEAR,
            ),
        ],
        Scene::MagmaBridge => vec![
            Panel::new(
                smog(SPAN, 4, palette::COAL),
                Vec2::new(0.0, ARENA_HALF_H - 22.0),
                SKY,
            ),
            Panel::footed(
                ridge(&CHASM_RIDGE, SPAN, palette::COAL),
                Vec2::new(0.0, GROUND),
                SKY,
            ),
            // As duas paredes do desfiladeiro, subindo alem do topo da tela.
            // Elas nao sao paisagem: sao o motivo de a ponte existir.
            Panel::footed(basalt(26, 26, &STONE), Vec2::new(-536.0, GROUND), FAR),
            Panel::footed(
                basalt(26, 26, &STONE).mirrored(),
                Vec2::new(536.0, GROUND),
                FAR,
            ),
            // A travessia que ja caiu, pendurada atras da que ainda esta em pe.
            Panel::new(suspension(60, 12, &STEEL), Vec2::new(0.0, 58.0), MID),
            Panel::footed(
                current(SPAN, 2, 0, &MAGMA_FLOW),
                Vec2::new(0.0, GROUND),
                NEAR,
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
            Panel::footed(furnace(30, 13, &FURNACE), Vec2::new(-330.0, GROUND), MID),
            // Bigorna e trilhos ficam parados; o malho que desce entre eles e
            // uma entidade a parte, em `shows`.
            Panel::footed(
                AsciiArt::tinted(ANVIL, &STEEL, palette::IRON),
                Vec2::new(HAMMER_AT.x, GROUND),
                MID,
            ),
            Panel::footed(
                AsciiArt::fill('║', 1, 15, palette::IRON),
                Vec2::new(HAMMER_AT.x - 56.0, GROUND + 90.0),
                MID,
            ),
            Panel::footed(
                AsciiArt::fill('║', 1, 15, palette::IRON),
                Vec2::new(HAMMER_AT.x + 56.0, GROUND + 90.0),
                MID,
            ),
            Panel::footed(
                current(SPAN, 2, 0, &MAGMA_FLOW),
                Vec2::new(0.0, GROUND),
                NEAR,
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
            // Cinza esverdeada em vez de cinza: o ceu da fabrica ja e o aviso.
            Panel::new(
                smog(SPAN, 4, palette::SCENE_TOXIC),
                Vec2::new(0.0, ARENA_HALF_H - 20.0),
                SKY,
            ),
            Panel::footed(
                ridge(&STACK_RIDGE, SPAN, palette::COAL),
                Vec2::new(0.0, GROUND),
                SKY,
            ),
            // Coluna de destilacao: a peca mais alta do patio.
            Panel::footed(vat(9, 18, &TANK_SKIN), Vec2::new(-470.0, VAT_FOOT), MID),
            Panel::footed(
                vat(VAT_COLS, STACKS[0].1, &TANK_SKIN),
                Vec2::new(STACKS[0].0, VAT_FOOT),
                MID,
            ),
            Panel::footed(
                vat(VAT_COLS, STACKS[1].1, &TANK_SKIN),
                Vec2::new(STACKS[1].0, VAT_FOOT),
                MID,
            ),
            Panel::footed(
                vat(VAT_COLS, STACKS[2].1, &TANK_SKIN),
                Vec2::new(STACKS[2].0, VAT_FOOT),
                MID,
            ),
            // Contra o ceu, e nao contra a serra: o vao da placa e
            // transparente, e sobre a montanha o texto se perde na pedra.
            Panel::new(
                AsciiArt::solid(PLACARD, palette::SCENE_TOXIC),
                Vec2::new(-330.0, 44.0),
                MID,
            ),
            Panel::new(
                AsciiArt::solid(PLACARD, palette::SCENE_RUST),
                Vec2::new(360.0, 44.0),
                MID,
            ),
            // Encanamento por cima de tudo, na frente: da teto ao patio sem
            // fechar a leitura da briga.
            Panel::footed(pipeline(SPAN, &STEEL), Vec2::new(0.0, 122.0), NEAR),
        ],
        Scene::Reactor => vec![
            Panel::footed(
                gantry(12, palette::IRON),
                Vec2::new(0.0, GROUND + 250.0),
                FAR,
            ),
            // Anel de contencao vazado: o nucleo que mora no meio dele pulsa
            // sozinho, em `flows`, e por isso nao entra na composicao parada.
            Panel::new(disc(13, true, &CONTAINMENT), CORE_AT, MID),
            // Os tirantes tem que encostar no anel. Curtos, eles viram dois
            // tracos boiando ao lado dele.
            Panel::footed(
                AsciiArt::fill('═', 34, 1, palette::IRON),
                Vec2::new(-250.0, CORE_AT.y),
                MID,
            ),
            Panel::footed(
                AsciiArt::fill('═', 34, 1, palette::IRON),
                Vec2::new(250.0, CORE_AT.y),
                MID,
            ),
            Panel::footed(drains(SPAN, palette::COAL), Vec2::new(0.0, GROUND), NEAR),
        ],
        Scene::Drainage => vec![
            // Fechado por cima: o unico mapa do jogo que acontece embaixo da
            // terra tem que ter teto, senao ele e um patio escuro qualquer.
            Panel::new(vault(SPAN, 5, &STONE), Vec2::new(0.0, 200.0), SKY),
            // Galeria funda, atras da principal: e ela que da corredor ao
            // subsolo em vez de uma parede de arcos.
            Panel::footed(
                arcade(3, 66, 11, &STONE),
                Vec2::new(0.0, GROUND + 40.0),
                FAR,
            ),
            // Galeria de arcos de verdade: a curva e meia-circunferencia, e e
            // ela que separa subsolo de porta quadrada.
            Panel::footed(arcade(5, 40, 16, &STONE), Vec2::new(0.0, GROUND), MID),
            Panel::footed(
                current(SPAN, 2, 0, &ACID_FLOW),
                Vec2::new(0.0, GROUND),
                NEAR,
            ),
            Panel::footed(
                pipeline(SPAN, &STEEL),
                Vec2::new(0.0, ARENA_HALF_H - 84.0),
                NEAR,
            ),
        ],
        Scene::RedGate => vec![
            // Bruma alta: o portao precisa de um ceu com materia, senao ele
            // fica recortado contra o vazio como um adesivo.
            Panel::new(
                smog(SPAN, 3, palette::SCENE_HAZE),
                Vec2::new(0.0, ARENA_HALF_H - 18.0),
                SKY,
            ),
            Panel::footed(
                ridge(&GATE_RIDGE, SPAN, palette::SCENE_HAZE),
                Vec2::new(0.0, GROUND),
                SKY,
            ),
            // Escadaria e portao no mesmo eixo: o portao pousa no ultimo
            // degrau em vez de flutuar sobre ele.
            Panel::footed(terrace(4, 44, 5, &STONE), Vec2::new(0.0, GROUND), FAR),
            Panel::footed(gate(52, 14, &LACQUER), Vec2::new(0.0, GROUND + 46.0), MID),
            Panel::footed(
                AsciiArt::tinted(STONE_LANTERN, &STONE, palette::IRON),
                Vec2::new(-360.0, GROUND),
                NEAR,
            ),
            Panel::footed(
                AsciiArt::tinted(STONE_LANTERN, &STONE, palette::IRON),
                Vec2::new(360.0, GROUND),
                NEAR,
            ),
            Panel::new(
                AsciiArt::solid(BRANCH, palette::COAL),
                Vec2::new(-450.0, 185.0),
                NEAR,
            ),
        ],
        Scene::SunsetPagoda => vec![
            Panel::new(disc(14, false, &SUNSET), Vec2::new(-330.0, 84.0), SKY),
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
            // O pagode pousa no ultimo degrau do terraco, e nao dentro dele:
            // duas pecas no mesmo plano que se cruzam disputam quem cobre
            // quem, e o que aparece vira sorteio da ordem de spawn.
            Panel::footed(terrace(3, 30, 5, &STONE), Vec2::new(270.0, GROUND), MID),
            Panel::footed(pagoda(5, &TIMBER), Vec2::new(270.0, GROUND + 48.0), MID),
        ],
        Scene::DragonGarden => vec![
            Panel::new(disc(12, true, &LUNAR), Vec2::new(-430.0, 150.0), SKY),
            Panel::footed(
                ridge(&GARDEN_RIDGE, SPAN, palette::COAL),
                Vec2::new(0.0, GROUND),
                FAR,
            ),
            // O dragao: corpo gerado e cabeca desenhada, encostados coluna com
            // coluna. O pescoco sai pela esquerda da cabeca, entao o corpo tem
            // que vir dali -- e a espinha da senoide tem que chegar na mesma
            // linha em que ele sai, senao o bicho nasce deslocado do proprio
            // pescoco.
            Panel::new(serpent(58, 9, &JADE_SKIN).mirrored(), DRAGON_BODY, MID),
            Panel::new(
                AsciiArt::tinted(DRAGON_HEAD, &JADE_SKIN, palette::SCENE_JADE).mirrored(),
                DRAGON_HEAD_AT,
                MID,
            ),
            Panel::footed(jade_pillar(13, &JADE_SKIN), Vec2::new(-556.0, GROUND), NEAR),
            Panel::footed(jade_pillar(13, &JADE_SKIN), Vec2::new(556.0, GROUND), NEAR),
            Panel::footed(bamboo(6, 11, &JADE_SKIN), Vec2::new(-390.0, GROUND), NEAR),
            Panel::footed(
                AsciiArt::tinted(STONE_LANTERN, &STONE, palette::IRON),
                Vec2::new(430.0, GROUND),
                NEAR,
            ),
        ],
    }
}

// --- pecas que se mexem sozinhas --------------------------------------------

/// Uma peca do fundo que troca de arte em ciclo.
///
/// Queda de magma, cascata de acido, nucleo do reator: sao todas a mesma
/// coisa, um punhado de quadros e um relogio. Enquanto fossem tres sistemas,
/// so a que alguem estivesse olhando ganharia conserto.
#[derive(Component)]
struct Flipbook {
    frames: Vec<AsciiArt>,
    clock: Timer,
    frame: usize,
}

fn turn_pages(time: Res<Time>, mut books: Query<(&mut Flipbook, &mut AsciiSprite)>) {
    for (mut book, mut sprite) in &mut books {
        if !book.clock.tick(time.delta()).just_finished() {
            continue;
        }
        book.frame = (book.frame + 1) % book.frames.len();
        sprite.art = book.frames[book.frame].clone();
    }
}

/// Uma peca animada do fundo, ja resolvida em quadros, lugar e ritmo.
///
/// Descrita antes de existir, como os [`Panel`]: o que nasce direto em
/// `Commands` nao passa por teste nenhum, e foi assim que a arte solta do fundo
/// -- justamente a que se mexe, e por isso a mais visivel -- ficou de fora da
/// varredura de CP437 e da de cores reservadas.
struct Reel {
    frames: Vec<AsciiArt>,
    at: Vec2,
    depth: f32,
    fps: f32,
    /// Apoiada na base, como quase tudo que escorre ate o chao.
    foot: bool,
}

/// Quatro quadros de uma queda de liquido.
fn falling(cols: u16, rows: u16, map: &'static [(char, Color)]) -> Vec<AsciiArt> {
    (0..4).map(|f| cascade(cols, rows, f, map)).collect()
}

/// O que escorre, pulsa ou transborda em cada cenario.
fn reels(scene: Scene) -> Vec<Reel> {
    let fall = |x: f32, depth: f32, fps: f32, frames: Vec<AsciiArt>| Reel {
        frames,
        at: Vec2::new(x, GROUND),
        depth,
        fps,
        foot: true,
    };
    match scene {
        // Duas quedas de magma descendo pela cara dos penhascos.
        Scene::MagmaBridge => [-420.0, 420.0]
            .into_iter()
            .map(|x| fall(x, MID, 7.0, falling(7, 22, &MAGMA_FALL)))
            .collect(),
        // Sangria do forno: o metal sai pela base e corre para o canal.
        Scene::ForgeCore => vec![fall(-240.0, MID, 8.0, falling(4, 6, &MAGMA_FALL))],
        // Transbordo entre os tanques -- e de onde a fabrica pinga.
        Scene::AcidWorks => [-95.0, 95.0]
            .into_iter()
            .map(|x| fall(x, MID, 6.0, falling(6, 12, &ACID_FALL)))
            .collect(),
        Scene::Reactor => [-300.0, 300.0]
            .into_iter()
            .map(|x| fall(x, NEAR, 6.0, falling(5, 19, &COOLANT_FALL)))
            .chain([Reel {
                // O nucleo respira: cresce, estoura e volta. Como o sprite e
                // centrado, o pulso sai do meio do anel sem mover o anel.
                frames: [4u16, 5, 6, 5]
                    .into_iter()
                    .map(|r| disc(r, false, &CORE_HEAT))
                    .collect(),
                at: CORE_AT,
                depth: MID,
                fps: 3.6,
                foot: false,
            }])
            .collect(),
        // As duas bocas de saida despejando na calha.
        Scene::Drainage => [-240.0, 240.0]
            .into_iter()
            .map(|x| fall(x, MID, 7.0, falling(8, 10, &ACID_FALL)))
            .collect(),
        // Tanque de agua do jardim, correndo devagar.
        Scene::DragonGarden => vec![fall(
            0.0,
            NEAR,
            3.0,
            (0..4).map(|f| current(40, 2, f, &JADE_FLOW)).collect(),
        )],
        Scene::City | Scene::Caldera | Scene::RedGate | Scene::SunsetPagoda => Vec::new(),
    }
}

fn flows(commands: &mut Commands, scene: Scene) {
    for reel in reels(scene) {
        let sprite = if reel.foot {
            AsciiSprite::footed(reel.frames[0].clone())
        } else {
            AsciiSprite::new(reel.frames[0].clone())
        };
        commands.spawn((
            LevelGeometry,
            Parallax {
                home: reel.at,
                depth: reel.depth,
            },
            sprite,
            Flipbook {
                clock: Timer::from_seconds(1.0 / reel.fps, TimerMode::Repeating),
                frames: reel.frames,
                frame: 0,
            },
            Layer::Background,
            Transform::from_translation(reel.at.extend(-reel.depth)),
        ));
    }
}

/// O acontecimento de um cenario: o que ele faz de tempos em tempos.
///
/// O vulcao ja tinha o dele -- a erupcao -- e e ele que separa a Caldera dos
/// outros oito mapas. Sem um equivalente, cenario e pintura: bonito no
/// primeiro round e mudo no decimo.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Show {
    /// O dragao acorda, junta luz na boca e sopra fogo de jade.
    DragonBreath,
    /// O malho recua, desce, bate e se recolhe.
    ForgeHammer,
    /// A fabrica purga: alarme, transbordo e chuva corrosiva.
    Purge,
}

impl Show {
    /// Quanto dura o numero, em segundos.
    const fn span(self) -> f32 {
        match self {
            Self::DragonBreath => 2.4,
            Self::ForgeHammer => 1.15,
            Self::Purge => 2.0,
        }
    }
}

#[derive(Component)]
struct Landmark {
    show: Show,
    clock: Timer,
    /// Segundos restantes do numero em cena.
    live: f32,
    /// Onde a peca descansa -- ou, para quem nao anda, de onde ela age.
    rest: Vec2,
    /// O quadro de contato ja aconteceu neste numero?
    struck: bool,
}

/// Toca os numeros dos cenarios.
fn run_shows(
    time: Res<Time>,
    mut commands: Commands,
    mut shake: MessageWriter<Shake>,
    mut marks: Query<(&mut Landmark, &mut Parallax)>,
) {
    let dt = time.delta_secs();
    for (mut mark, mut plane) in &mut marks {
        if mark.clock.tick(time.delta()).just_finished() {
            mark.live = mark.show.span();
            mark.struck = false;
            // O malho nao sacode ao comecar: o tranco dele e no contato, e ele
            // acontece no meio do numero.
            match mark.show {
                Show::DragonBreath => {
                    shake.write(Shake(0.30));
                }
                Show::Purge => {
                    shake.write(Shake(0.18));
                }
                Show::ForgeHammer => {}
            }
        }
        if mark.live <= 0.0 {
            continue;
        }
        mark.live = (mark.live - dt).max(0.0);
        let beat = 1.0 - mark.live / mark.show.span();

        match mark.show {
            Show::ForgeHammer => {
                // Antecipacao curta para cima, queda acelerada, pausa no
                // contato e recuperacao amortecida. Sem a antecipacao o malho
                // parece cair sozinho; sem a pausa, quicar.
                let drop = match beat {
                    b if b < 0.32 => -0.10 * (b / 0.32 * std::f32::consts::PI).sin(),
                    b if b < 0.44 => ((b - 0.32) / 0.12).powi(2),
                    b if b < 0.58 => 1.0,
                    b => (1.0 - (b - 0.58) / 0.42).max(0.0),
                };
                plane.home.y = mark.rest.y - HAMMER_DROP * drop;
                if !mark.struck && beat >= 0.44 {
                    mark.struck = true;
                    shake.write(Shake(0.24));
                    for i in 0..12 {
                        let spread = (i as f32 / 11.0 - 0.5) * 120.0;
                        ember(
                            &mut commands,
                            mark.rest + Vec2::new(spread, -HAMMER_DROP - 12.0),
                            plane.depth,
                            palette::EMBER,
                        );
                    }
                }
            }
            Show::DragonBreath => {
                if beat < 0.34 {
                    // Carrega: a luz converge para a boca antes de sair.
                    if fastrand::f32() < dt * 30.0 {
                        let angle = fastrand::f32() * std::f32::consts::TAU;
                        let at =
                            mark.rest + Vec2::from_angle(angle) * (30.0 + fastrand::f32() * 40.0);
                        ember(&mut commands, at, plane.depth, palette::JADE);
                    }
                } else {
                    // Sopra: a frente avanca, e o rastro fica para tras.
                    let front = (beat - 0.34) / 0.66;
                    for _ in 0..3 {
                        let travel = BREATH_REACH * front * (0.45 + fastrand::f32() * 0.55);
                        let wander = Vec2::new(fastrand::f32() - 0.5, fastrand::f32() - 0.5)
                            * (18.0 + travel * 0.18);
                        ember(
                            &mut commands,
                            mark.rest + BREATH_DIR * travel + wander,
                            plane.depth,
                            palette::JADE,
                        );
                    }
                }
            }
            Show::Purge => {
                // Chuva corrosiva vindo do encanamento, de ponta a ponta.
                for _ in 0..2 {
                    let at = Vec2::new(
                        (fastrand::f32() - 0.5) * ARENA_HALF_W * 1.9,
                        mark.rest.y - fastrand::f32() * 90.0,
                    );
                    ember(&mut commands, at, plane.depth, palette::TOXIC);
                }
            }
        }
    }
}

/// Pendura o numero de cada cenario, quando ele tem um.
fn shows(commands: &mut Commands, scene: Scene) {
    let (show, rest, depth, every, art) = match scene {
        Scene::ForgeCore => (
            Show::ForgeHammer,
            HAMMER_AT,
            MID,
            4.6,
            Some(AsciiArt::tinted(HAMMER, &STEEL, palette::IRON)),
        ),
        Scene::AcidWorks => (Show::Purge, Vec2::new(0.0, 118.0), NEAR, 9.5, None),
        Scene::DragonGarden => (Show::DragonBreath, DRAGON_MAW, MID, 13.0, None),
        _ => return,
    };

    let mut entity = commands.spawn((
        LevelGeometry,
        Parallax { home: rest, depth },
        Landmark {
            show,
            clock: Timer::from_seconds(every, TimerMode::Repeating),
            live: 0.0,
            rest,
            struck: false,
        },
        Layer::Background,
        Transform::from_translation(rest.extend(-depth)),
    ));
    if let Some(art) = art {
        entity.insert(AsciiSprite::new(art));
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
            // No pe das duas quedas: onde o magma bate, ele fumega.
            for x in [-420.0, 420.0] {
                let at = Vec2::new(x, GROUND + 20.0);
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
            // A boca do alto-forno respira mesmo parada.
            let at = Vec2::new(-330.0, GROUND + 190.0);
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
            // Em cima de cada tanque, que e de onde a chamine sai.
            for (i, (x, rows)) in STACKS.into_iter().enumerate() {
                let at = Vec2::new(x, stack_top(rows));
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
        Scene::Drainage => {
            // Vapor subindo de onde as bocas despejam na calha.
            for (i, x) in [-240.0, 240.0].into_iter().enumerate() {
                let at = Vec2::new(x, GROUND + 10.0);
                commands.spawn((
                    LevelGeometry,
                    Parallax {
                        home: at,
                        depth: NEAR,
                    },
                    Vent {
                        puff: Timer::from_seconds(0.8 + i as f32 * 0.23, TimerMode::Repeating),
                        hot: false,
                        power: 0.5,
                        blast: 0.0,
                        erupt: None,
                        tone: 0,
                    },
                    Transform::from_translation(at.extend(-NEAR)),
                ));
            }
        }
        Scene::City | Scene::RedGate | Scene::SunsetPagoda | Scene::DragonGarden => {}
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
            ember(&mut commands, plane.home, plane.depth, palette::EMBER);
        }
    }
}

/// Uma faisca parada que apaga sozinha.
///
/// Rastro de bomba de lava, respingo do malho, sopro do dragao e chuva da
/// purga sao a mesma particula com outra cor -- e por isso a cor e parametro.
fn ember(commands: &mut Commands, at: Vec2, depth: f32, color: Color) {
    commands.spawn((
        LevelGeometry,
        Parallax { home: at, depth },
        AsciiSprite::new(AsciiArt::glyph(
            if fastrand::bool() { '·' } else { '°' },
            color,
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

    // Os predios da fabrica recuam de plano: o patio de tanques ocupa o plano
    // medio inteiro, e duas massas na mesma profundidade disputam quem cobre
    // quem -- o que aparece vira sorteio da ordem de spawn.
    let depth = match scene {
        Scene::AcidWorks => FAR,
        Scene::Reactor => SKY,
        _ => MID,
    };
    for &(x, y, cols, rows) in skyline {
        let mut art = AsciiArt::fill('▓', cols, rows, palette::COAL);
        for row in (1..rows.saturating_sub(1)).step_by(2) {
            for col in (2..cols.saturating_sub(1)).step_by(4) {
                art = art.stamp(&AsciiArt::solid("·", window), col, row);
            }
        }
        raise(
            commands,
            Panel::footed(art, Vec2::new(x, y + GROUND), depth),
        );
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
            // Balizas de alerta pelo patio, cada uma no seu compasso: piscar
            // junto viraria pisca-pisca de natal.
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
            // Vazamento pingando dos tanques -- o aviso de que a fabrica vaza
            // muito antes de a purga acontecer.
            for (i, (x, _)) in STACKS.into_iter().enumerate() {
                spark(
                    commands,
                    '\'',
                    Vec2::new(x + 40.0, VAT_FOOT - 6.0),
                    palette::SCENE_ACID,
                    Sway {
                        phase: i as f32 * 2.3,
                        speed: 1.1,
                        travel: Vec2::new(1.0, 12.0),
                    },
                    MID,
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
            // Bando de grous cruzando o poente, alto e devagar.
            if scene == Scene::SunsetPagoda {
                for (i, at) in [
                    Vec2::new(-120.0, 132.0),
                    Vec2::new(-62.0, 150.0),
                    Vec2::new(-8.0, 124.0),
                ]
                .into_iter()
                .enumerate()
                {
                    adrift(
                        commands,
                        AsciiArt::solid(CRANE, palette::SCENE_HAZE),
                        at,
                        Sway {
                            phase: i as f32 * 0.9,
                            speed: 0.22,
                            travel: Vec2::new(70.0, 9.0),
                        },
                        FAR,
                    );
                }
            }
        }
        Scene::DragonGarden => {
            // Vaga-lumes dourados no jardim.
            for (i, x) in [-480.0, -180.0, 120.0, 360.0, 520.0]
                .into_iter()
                .enumerate()
            {
                spark(
                    commands,
                    '·',
                    Vec2::new(x, -60.0 + (i % 3) as f32 * 45.0),
                    palette::SCENE_GOLD,
                    Sway {
                        phase: i as f32 * 1.7,
                        speed: 0.38,
                        travel: Vec2::new(22.0, 10.0),
                    },
                    FAR,
                );
            }
            // A escama que pega luz ao longo do lombo, e os dois olhos. Sao
            // faiscas a parte, e nao celulas da arte, porque piscam sozinhas:
            // e isso que faz o bicho parecer dormindo em vez de esculpido.
            for (i, x) in [40.0, 150.0, 260.0].into_iter().enumerate() {
                spark(
                    commands,
                    '•',
                    Vec2::new(x, DRAGON_BODY.y + 26.0 - i as f32 * 14.0),
                    palette::SCENE_JADE_LIT,
                    Sway {
                        phase: i as f32 * 2.4,
                        speed: 0.5,
                        travel: Vec2::new(3.0, 5.0),
                    },
                    MID,
                );
            }
            spark(
                commands,
                '•',
                Vec2::new(DRAGON_MAW.x + 52.0, DRAGON_MAW.y + 26.0),
                palette::SCENE_GOLD,
                Sway {
                    phase: 0.0,
                    speed: 1.6,
                    travel: Vec2::new(0.0, 2.0),
                },
                MID,
            );
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
    flows(commands, scene);
    shows(commands, scene);
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
                run_shows,
                turn_pages,
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

    /// A arte que nasce durante a partida, e por isso nao passa por [`panels`].
    fn loose_art() -> Vec<&'static str> {
        vec![CRATER_GLOW, BLAST_FLASH, LANTERN, HAMMER, CRANE]
            .into_iter()
            .chain(PUFF)
            .collect()
    }

    /// Todas as celulas do fundo de uma cena: composicao parada mais quadros
    /// animados. E por aqui que as varreduras passam, para nao existir arte de
    /// fundo que escapa de conferencia so por se mexer.
    fn every_cell(scene: Scene) -> Vec<crate::ascii::art::Cell> {
        panels(scene)
            .into_iter()
            .map(|panel| panel.art)
            .chain(reels(scene).into_iter().flat_map(|reel| reel.frames))
            .flat_map(|art| art.cells)
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

        // Do plano mais fundo para o mais raso, que e a ordem em que a tela
        // desenha: assim o preview mostra quem cobre quem, e nao a soma de
        // tudo. Uma peca escondida atras de outra e trabalho jogado fora, e o
        // preview so denuncia isso se respeitar a profundidade.
        let mut ordered = panels(scene);
        ordered.sort_by(|a, b| b.depth.total_cmp(&a.depth));

        for panel in ordered {
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
                grid[y as usize][x as usize] = crate::ascii::cp437::glyph_char(cell.glyph);
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

    /// Cada chamine tem que ter tanque embaixo dela.
    ///
    /// A boca de fumaca e uma entidade a parte da arte do tanque. Enquanto os
    /// dois foram numeros escritos em lugares diferentes, o vapor subia de
    /// tres pontos do ceu ao lado dos tanques e a arte continuava intacta:
    /// nada para um teste pegar, e obvio na tela.
    #[test]
    fn a_fumaca_da_fabrica_sai_de_cima_dos_tanques() {
        let tanques: Vec<(Vec2, f32)> = panels(Scene::AcidWorks)
            .iter()
            .filter(|panel| panel.art.cols == VAT_COLS)
            .map(|panel| (panel.at, panel.at.y + panel.art.size().y))
            .collect();
        assert_eq!(tanques.len(), STACKS.len(), "sumiu um tanque do patio");

        for (x, rows) in STACKS {
            let (_, topo) = tanques
                .iter()
                .find(|(at, _)| (at.x - x).abs() < 1.0)
                .unwrap_or_else(|| panic!("a chamine em {x} fumega sobre o patio vazio"));
            assert!(
                (topo - stack_top(rows)).abs() < 1.0,
                "a chamine em {x} fica {} acima do topo do tanque",
                stack_top(rows) - topo
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
            assert!(
                every_cell(scene).iter().all(|cell| cell.glyph != fallback),
                "{scene:?}: o fundo usa glifo fora da pagina"
            );
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
            palette::JADE,
        ];

        for scene in SCENES {
            assert!(
                every_cell(scene)
                    .iter()
                    .all(|cell| !reserved.contains(&cell.color)),
                "{scene:?}: landmark usa cor reservada ao gameplay"
            );
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

    /// O dragao tem que ser um bicho so.
    ///
    /// Ele nasce de duas pecas: a cabeca, desenhada a mao, e o corpo, gerado
    /// por senoide. Elas se encontram numa coluna e numa linha que ninguem
    /// enxerga lendo o codigo -- sao contas em arquivos de constantes
    /// diferentes. Basta mexer na altura do corpo, na amplitude da onda ou no
    /// tamanho da cabeca para o pescoco descolar, e o resultado nao quebra
    /// nada: o jogo roda, os outros testes passam, e na tela ha uma cabeca
    /// flutuando ao lado de uma cobra.
    #[test]
    fn o_dragao_e_um_bicho_so() {
        // Extremos de uma peca: onde ela comeca e acaba, e a altura media das
        // celulas da coluna pedida.
        let edge = |art: &AsciiArt, at: Vec2, col: u16| {
            let rows: Vec<u16> = art
                .cells
                .iter()
                .filter(|cell| cell.col == col)
                .map(|cell| cell.row)
                .collect();
            assert!(!rows.is_empty(), "a coluna {col} da peca esta vazia");
            let middle = rows.iter().map(|r| *r as f32).sum::<f32>() / rows.len() as f32;
            at.y + art.size().y * 0.5 - (middle + 0.5) * CELL.y
        };

        let head = AsciiArt::tinted(DRAGON_HEAD, &JADE_SKIN, palette::SCENE_JADE).mirrored();
        let body = serpent(58, 9, &JADE_SKIN).mirrored();

        // Encostados coluna com coluna: a cabeca acaba onde o corpo comeca.
        let neck = DRAGON_HEAD_AT.x + head.size().x * 0.5;
        let spine = DRAGON_BODY.x - body.size().x * 0.5;
        assert!(
            (neck - spine).abs() <= CELL.x,
            "cabeca acaba em {neck} e o corpo comeca em {spine}: sobra vao entre os dois"
        );

        // E na mesma altura: o pescoco sai da cabeca na linha em que a espinha
        // chega. Uma celula e meia de folga -- alem disso o olho ve o degrau.
        let sai = edge(&head, DRAGON_HEAD_AT, head.cols - 1);
        let chega = edge(&body, DRAGON_BODY, 0);
        assert!(
            (sai - chega).abs() <= CELL.y * 1.5,
            "o pescoco sai em {sai} e a espinha chega em {chega}"
        );

        // E o corpo tem que afinar da cabeca para o rabo. Ao contrario, o
        // bicho fica com a cabeca espetada na ponta fina.
        let thickness = |col: u16| body.cells.iter().filter(|cell| cell.col == col).count();
        assert!(
            thickness(0) > thickness(body.cols - 1),
            "o corpo esta mais grosso no rabo que no pescoco"
        );
    }

    /// A boca de onde sai o sopro tem que ser a boca do desenho.
    ///
    /// Ela e uma constante a parte da arte. Espelhar a cabeca, mover o dragao
    /// ou mudar a linha do focinho deixa o fogo saindo do ar ao lado dele --
    /// e nada no jogo reclama.
    #[test]
    fn o_sopro_sai_da_boca_do_dragao() {
        let head = AsciiArt::tinted(DRAGON_HEAD, &JADE_SKIN, palette::SCENE_JADE).mirrored();
        let size = head.size();
        let corner = DRAGON_HEAD_AT + Vec2::new(-size.x, size.y) * 0.5;

        let bocas: Vec<Vec2> = head
            .cells
            .iter()
            .map(|cell| {
                corner
                    + Vec2::new(
                        (cell.col as f32 + 0.5) * CELL.x,
                        -(cell.row as f32 + 0.5) * CELL.y,
                    )
            })
            .collect();
        let perto = bocas
            .iter()
            .map(|at| at.distance(DRAGON_MAW))
            .fold(f32::INFINITY, f32::min);
        assert!(
            perto <= CELL.y,
            "a boca marcada esta a {perto} da celula mais proxima do desenho"
        );

        // E o sopro tem que ir para fora da cabeca, nao para dentro dela.
        let frente = DRAGON_MAW + BREATH_DIR * BREATH_REACH;
        assert!(
            frente.x < DRAGON_HEAD_AT.x - size.x * 0.5,
            "o sopro atravessa a propria cabeca"
        );
        assert!(
            frente.x > -ARENA_HALF_W && frente.y > -ARENA_HALF_H,
            "o sopro termina fora da tela, onde ninguem ve"
        );
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
