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

