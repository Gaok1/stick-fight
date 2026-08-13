//! Fases.
//!
//! Um mapa e um `Level`: ele diz onde os jogadores nascem, onde as armas caem,
//! e monta a propria geometria. Trocar de fase e trocar o `Box<dyn Level>` no
//! recurso -- nenhum outro sistema precisa saber qual mapa esta no ar.

use bevy::prelude::*;

use crate::actor::{Health, Player, Stunned};
use crate::ascii::{AsciiArt, AsciiSprite, CELL, Layer, palette};
use crate::backdrop::{Building, Scene, Sign, Theme};
use crate::combat::{Damaged, Lifetime};
use crate::physics::{Collider, Falls, Ghost, KILL_Y, OneWay, Solid, Velocity, overlap};
use crate::state::{AppSet, GameMode, GameState, arena_live};
use crate::weapon::Projectile;

/// Meia-largura da arena em unidades de mundo.
pub const ARENA_HALF_W: f32 = 640.0;
/// Meia-altura visivel.
pub const ARENA_HALF_H: f32 = 240.0;

/// Uma peca de geometria, como dado.
///
/// Fases descrevem, nao constroem: um `&'static [Piece]` pode ser conferido por
/// teste -- se um patamar ficar fora do alcance do pulo, o mapa nasce com um
/// jogador preso e nada no jogo reclama.
#[derive(Debug, Clone, Copy)]
pub enum Piece {
    /// Bloco macico. `top` e o meio da superficie de cima, nao o centro.
    Terrain {
        /// Meio da superficie superior.
        top: Vec2,
        /// Largura em celulas.
        cols: u16,
        /// Altura em celulas.
        rows: u16,
    },
    /// Teto: bloco macico pendurado, medido pela face de baixo.
    ///
    /// Fisicamente e igual a [`Piece::Terrain`] -- a fisica ja para quem sobe
    /// contra um solido. O que muda e que ele nao conta como apoio: o topo de
    /// um teto nao e lugar de ficar de pe, e cobrar que alguem chegue la
    /// reprovaria um mapa correto.
    Ceiling {
        /// Meio da face inferior.
        bottom: Vec2,
        /// Largura em celulas.
        cols: u16,
        /// Espessura em celulas.
        rows: u16,
    },
    /// Plataforma fina, atravessavel por baixo.
    Platform {
        /// Meio da plataforma.
        at: Vec2,
        /// Largura em celulas.
        cols: u16,
    },
    /// Corrente escalavel pendurada a partir de `top`.
    Chain {
        /// Onde ela e presa.
        top: Vec2,
        /// Quantidade de elos.
        links: u16,
    },
    /// Superficie perigosa. Nao e solida: normalmente fica sobre um piso ou
    /// no fundo de um poco e machuca quem encostar.
    Hazard {
        /// Meio da faixa perigosa.
        at: Vec2,
        /// Largura em celulas.
        cols: u16,
        /// Material, que decide arte, dano e empurrao.
        kind: HazardKind,
    },
    /// Fonte que jorra de tempos em tempos.
    ///
    /// A diferenca para [`Piece::Hazard`] nao e de grau: uma poca so cobra
    /// atencao uma vez -- o jogador aprende onde ela esta e nunca mais pisa
    /// ali. Uma fonte cobra atencao o round inteiro, porque o lugar seguro
    /// deixa de ser um lugar e passa a ser um lugar *e uma hora*.
    Geyser {
        /// Base da coluna, na altura do chao de onde ela sai.
        at: Vec2,
        /// Largura da boca, em celulas.
        cols: u16,
        /// Altura do jorro cheio, em celulas.
        rows: u16,
        /// Segundos entre um jorro e o proximo.
        period: f32,
        /// Deslocamento no ciclo. Duas fontes em fase jorram juntas e viram
        /// uma parede; fora de fase, elas fazem o jogador escolher.
        phase: f32,
        kind: HazardKind,
    },
    /// Poca que sobe e desce, engolindo o que estiver baixo demais.
    ///
    /// E o oposto da fonte: em vez de o perigo vir buscar, o chao seguro
    /// afunda. Uma plataforma que so vale metade do tempo vale mais que duas
    /// que valem sempre.
    Tide {
        /// Superficie na mare baixa.
        at: Vec2,
        /// Largura em celulas.
        cols: u16,
        /// Quanto ela sobe, em celulas.
        rise: u16,
        /// Segundos de um ciclo completo, subida e descida.
        period: f32,
        phase: f32,
        kind: HazardKind,
    },
    /// Goteira: uma boca no alto que pinga material corrosivo.
    Drip {
        /// Onde a boca fica.
        from: Vec2,
        /// Largura da boca, para as gotas nao cairem sempre na mesma coluna.
        cols: u16,
        /// Altura em que a gota se desmancha.
        floor: f32,
        /// Segundos entre uma gota e a proxima.
        period: f32,
        phase: f32,
        kind: HazardKind,
    },
}

/// Perigos reutilizados pelas arenas tematicas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HazardKind {
    Lava,
    Acid,
    Spikes,
    /// Fogo de jade: o material do jardim, que o dragao cospe.
    Jade,
}

impl HazardKind {
    fn damage(self) -> i32 {
        match self {
            Self::Lava => 34,
            Self::Acid => 18,
            Self::Spikes => 24,
            Self::Jade => 22,
        }
    }

    /// Para onde ele joga quem encosta.
    ///
    /// Lava explode para cima; acido so corroi e empurra de leve; espinho
    /// devolve; jade queima e levanta. O `x` sempre inverte o proprio impulso
    /// do jogador -- quem entrou correndo sai voltando.
    fn knockback(self, velocity: Vec2) -> Vec2 {
        match self {
            Self::Lava => Vec2::new(velocity.x * -0.35, 430.0),
            Self::Acid => Vec2::new(velocity.x * -0.2, 210.0),
            Self::Spikes => Vec2::new(velocity.x * -0.4, 300.0),
            Self::Jade => Vec2::new(velocity.x * -0.3, 380.0),
        }
    }

    /// Cor do respingo que ele solta.
    fn spray(self) -> Color {
        match self {
            Self::Lava => palette::MAGMA,
            Self::Acid => palette::TOXIC,
            Self::Spikes => palette::ASH,
            Self::Jade => palette::JADE,
        }
    }
}

impl Piece {
    /// Faixa horizontal e altura da superficie em que da para ficar de pe, se
    /// esta peca tiver uma. Corrente nao tem.
    ///
    /// Usado pelos testes de geometria e por quem precisa saber onde alguem
    /// vai parar sem simular a queda.
    pub fn foothold(self) -> Option<(f32, f32, f32)> {
        match self {
            Piece::Terrain { top, cols, .. } => {
                let half = cols as f32 * CELL.x * 0.5;
                Some((top.x - half, top.x + half, top.y))
            }
            Piece::Platform { at, cols } => {
                let half = cols as f32 * CELL.x * 0.5;
                Some((at.x - half, at.x + half, at.y + CELL.y * 0.5))
            }
            Piece::Chain { .. }
            | Piece::Ceiling { .. }
            | Piece::Hazard { .. }
            | Piece::Geyser { .. }
            | Piece::Tide { .. }
            | Piece::Drip { .. } => None,
        }
    }

    /// Faixa horizontal que esta peca torna perigosa, se tornar.
    ///
    /// Um ponto de nascimento dentro dela e um jogador que perde vida antes de
    /// encostar no chao -- e a fonte e a mare tem o agravante de nao estarem
    /// visiveis no instante em que alguem escolhe onde por o spawn.
    ///
    /// So os testes perguntam: em jogo, quem machuca e a entidade ja montada.
    /// Aqui a pergunta e sobre o dado, antes de existir entidade nenhuma.
    #[cfg(test)]
    pub fn menace(self) -> Option<(f32, f32)> {
        let span = |at: Vec2, cols: u16| {
            let half = cols as f32 * CELL.x * 0.5;
            Some((at.x - half, at.x + half))
        };
        match self {
            Piece::Hazard { at, cols, .. }
            | Piece::Geyser { at, cols, .. }
            | Piece::Tide { at, cols, .. } => span(at, cols),
            Piece::Drip { from, cols, .. } => span(from, cols),
            _ => None,
        }
    }
}

/// Contrato de uma fase.
///
/// Tudo aqui e dado. Nenhuma fase toca em `Commands`: quem monta e
/// [`build_level`], e por isso um mapa novo nao pode inventar uma regra de
/// spawn diferente das outras.
pub trait Level: Send + Sync + 'static {
    /// Nome exibido na tela de controles.
    fn name(&self) -> &'static str;
    /// Onde cada jogador nasce, por indice.
    fn spawn_points(&self) -> &'static [Vec2];
    /// Pontos de onde armas sao largadas.
    fn drop_points(&self) -> &'static [Vec2];
    /// Geometria jogavel.
    fn pieces(&self) -> &'static [Piece];
    /// Predios do fundo.
    fn skyline(&self) -> &'static [Building];
    /// Letreiros de neon.
    fn signs(&self) -> &'static [Sign];
    /// Fundo tematico da arena.
    fn theme(&self) -> Theme {
        self.scene().theme()
    }
    /// Pintura concreta do fundo; mapas do mesmo tema nao compartilham cartao.
    fn scene(&self) -> Scene {
        Scene::City
    }

    /// Topo do apoio mais alto que fica abaixo de `from`, na coluna dela.
    ///
    /// E onde alguem largado nesse ponto vai parar. Um ponto de spawn nao e o
    /// chao -- e so de onde os jogadores comecam a cair -- entao quem precisa
    /// posicionar algo em pe sem simular a queda pergunta aqui.
    fn ground_under(&self, from: Vec2) -> Option<f32> {
        self.pieces()
            .iter()
            .filter_map(|piece| piece.foothold())
            .filter(|(x0, x1, y)| from.x >= *x0 && from.x <= *x1 && *y <= from.y)
            .map(|(_, _, y)| y)
            .max_by(f32::total_cmp)
    }
}

/// Fase atualmente carregada.
#[derive(Resource)]
pub struct CurrentLevel(pub Box<dyn Level>);

/// Marca tudo que pertence a geometria da fase, para limpar no restart.
#[derive(Component)]
pub struct LevelGeometry;

/// Corrente escalavel: sobrepor e segurar cima faz o boneco subir.
#[derive(Component)]
pub struct Climbable;

/// Um elo da corrente Verlet. Elos consecutivos mantem distancia fixa; se um
/// deles for destruido, a parte abaixo perde a conexao e cai naturalmente.
#[derive(Component)]
pub struct ChainParticle {
    chain: u8,
    index: u16,
    pub(crate) previous: Vec2,
    pinned: bool,
}

const LINK_LENGTH: f32 = 12.5;

/// Onde um ciclo esta agora, em `[0, 1)`.
///
/// Todo perigo com hora marcada sai daqui, e nao de um `Timer` proprio: a
/// coluna que aparece e a zona que machuca sao entidades diferentes, e dois
/// relogios separados que comecam juntos terminam desencontrados depois de um
/// round inteiro -- o jorro aparece e nao fere, ou fere invisivel.
fn cycle(now: f32, period: f32, phase: f32) -> f32 {
    (now / period.max(0.01) + phase).rem_euclid(1.0)
}

/// A janela do ciclo em que um perigo esta armado.
#[derive(Clone, Copy)]
struct Beat {
    period: f32,
    phase: f32,
    /// Inicio e fim da janela, em fracao do ciclo.
    from: f32,
    to: f32,
}

impl Beat {
    fn open(self, now: f32) -> bool {
        let t = cycle(now, self.period, self.phase);
        t >= self.from && t < self.to
    }
}

/// Faixa de cenario que fere ao toque.
#[derive(Component)]
struct Hazard {
    kind: HazardKind,
    /// Quando ele morde. `None` e sempre -- poca parada, espinho.
    beat: Option<Beat>,
}

impl Hazard {
    fn always(kind: HazardKind) -> Self {
        Self { kind, beat: None }
    }

    fn armed(&self, now: f32) -> bool {
        self.beat.is_none_or(|beat| beat.open(now))
    }
}

#[derive(Component)]
struct HazardVisual {
    kind: HazardKind,
    cols: u16,
    phase: u8,
    bubbles: Timer,
}

/// Peca que sobe e desce sozinha: a mare, e a zona de contato dela.
#[derive(Component)]
struct Bob {
    home: f32,
    rise: f32,
    period: f32,
    phase: f32,
}

/// A coluna de uma fonte, que cresce e desaba com o ciclo.
#[derive(Component)]
struct Jet {
    kind: HazardKind,
    cols: u16,
    rows: u16,
    beat: Beat,
    /// Quadro desenhado agora, para a arte nao ser remontada a cada frame.
    drawn: u8,
    bubbles: Timer,
}

/// Boca que pinga.
#[derive(Component)]
struct Spout {
    kind: HazardKind,
    cols: u16,
    floor: f32,
    clock: Timer,
}

/// Uma gota no ar. Ela e um perigo que anda -- e por isso reaproveita a mesma
/// zona de contato da poca parada em vez de inventar uma regra propria.
#[derive(Component)]
struct Droplet {
    kind: HazardKind,
    floor: f32,
}

/// Impede que uma faixa de lava retire a barra inteira em um unico frame.
#[derive(Component)]
struct HazardCooldown(Timer);

/// Um bloco solido de terreno, com topo destacado da massa.
///
/// O topo em `ASH` e o miolo em `IRON` dao leitura de superficie sem precisar
/// de sombra ou segunda camada.
fn terrain(commands: &mut Commands, center: Vec2, cols: u16, rows: u16) {
    let mut art = AsciiArt::fill('\u{2588}', cols, 1, palette::ASH);
    if rows > 1 {
        art = art.stamp(
            &AsciiArt::fill('\u{2592}', cols, rows - 1, palette::IRON),
            0,
            1,
        );
    }
    let size = Vec2::new(cols as f32, rows as f32) * CELL;

    commands.spawn((
        LevelGeometry,
        AsciiSprite::new(art),
        Layer::Terrain,
        Transform::from_translation(center.extend(0.0)),
        Collider::size(size.x, size.y),
        Solid,
    ));
}

/// Plataforma fina, atravessavel por baixo.
fn platform(commands: &mut Commands, center: Vec2, cols: u16) {
    let art = AsciiArt::fill('\u{2550}', cols, 1, palette::ASH);

    commands.spawn((
        LevelGeometry,
        AsciiSprite::new(art),
        Layer::Terrain,
        Transform::from_translation(center.extend(0.0)),
        Collider::size(cols as f32 * CELL.x, CELL.y),
        Solid,
        OneWay,
    ));
}

fn hazard_art(kind: HazardKind, cols: u16, phase: u8) -> AsciiArt {
    match kind {
        HazardKind::Lava => {
            // Quatro temperaturas e rachaduras em movimento. O padrao muda
            // por coluna para a piscina fluir, em vez de apenas piscar inteira.
            let row = |choices: &[char], offset: u8, color| {
                let text: String = (0..cols)
                    .map(|col| choices[((col as u8 + phase + offset) as usize) % choices.len()])
                    .collect();
                AsciiArt::solid(&text, color)
            };
            row(&['≈', '~', '~', '≈', '_'], 0, palette::MAGMA)
                .stamp(&row(&['▒', '▓', '▒', '█'], 1, palette::EMBER), 0, 1)
                .stamp(&row(&['▓', '█', '▓', '▒'], 2, palette::BLOOD), 0, 2)
                .stamp(&row(&['█', '▓', '█', '▓'], 3, palette::COAL), 0, 3)
        }
        HazardKind::Acid => {
            let surface: String = (0..cols)
                .map(|col| match (col as u8 + phase) % 7 {
                    0 => '°',
                    3 => 'o',
                    _ => '≈',
                })
                .collect();
            AsciiArt::solid(&surface, palette::TOXIC)
                .stamp(&AsciiArt::fill('▒', cols, 1, palette::MOSS), 0, 1)
                .stamp(&AsciiArt::fill('▓', cols, 1, palette::SLUDGE), 0, 2)
        }
        HazardKind::Jade => {
            // Lingua, corpo e a boca de pedra que solta o fogo. A lingua e
            // rala de proposito: chama cheia ate o topo le como parede verde,
            // e o que faz fogo parecer fogo e a borda quebrada.
            let tongue: String = (0..cols)
                .map(|col| match (col as u8 + phase) % 5 {
                    0 => '▲',
                    2 => '^',
                    _ => ' ',
                })
                .collect();
            let body: String = (0..cols)
                .map(|col| {
                    if (col as u8 + phase).is_multiple_of(3) {
                        '≈'
                    } else {
                        '~'
                    }
                })
                .collect();
            AsciiArt::solid(&tongue, palette::JADE)
                .stamp(&AsciiArt::solid(&body, palette::MOSS), 0, 1)
                .stamp(&AsciiArt::fill('▓', cols, 1, palette::SLUDGE), 0, 2)
        }
        HazardKind::Spikes => AsciiArt::fill('▲', cols, 1, palette::ASH).stamp(
            &AsciiArt::fill('▄', cols, 1, palette::IRON),
            0,
            1,
        ),
    }
}

/// A coluna de uma fonte, com `rows` linhas de altura agora.
///
/// Ela afunila para cima e se desfaz na ponta. Uma coluna de largura constante
/// le como bloco subindo; e o afunilamento -- e o topo rasgado -- que faz o
/// olho ler pressao.
fn jet_art(kind: HazardKind, cols: u16, rows: u16, phase: u8) -> AsciiArt {
    let (core, body, edge) = match kind {
        HazardKind::Lava => (palette::MAGMA, palette::EMBER, palette::BLOOD),
        HazardKind::Acid => (palette::TOXIC, palette::MOSS, palette::SLUDGE),
        HazardKind::Jade => (palette::JADE, palette::MOSS, palette::SLUDGE),
        HazardKind::Spikes => (palette::BONE, palette::ASH, palette::IRON),
    };
    let middle = (cols as f32 - 1.0) * 0.5;

    let mut art = AsciiArt::default();
    for row in 0..rows {
        // 0 na ponta, 1 na base: e ele que abre a coluna para baixo.
        let down = (row + 1) as f32 / rows as f32;
        let half = 0.6 + down * middle;
        for col in 0..cols {
            let out = (col as f32 - middle).abs();
            if out > half {
                continue;
            }
            // A ponta chia em vez de terminar reta.
            let (glyph, color) = if row == 0 || (row == 1 && (col + phase as u16) % 2 == 0) {
                (
                    if (col + row + phase as u16).is_multiple_of(2) {
                        '*'
                    } else {
                        '°'
                    },
                    core,
                )
            } else if out < half * 0.45 {
                ('█', core)
            } else if out < half * 0.8 {
                ('▓', body)
            } else {
                ('▒', edge)
            };
            art = art.stamp(&AsciiArt::glyph(glyph, color), col, row);
        }
    }
    art
}

fn hazard(commands: &mut Commands, at: Vec2, cols: u16, kind: HazardKind) {
    // Arte e zona de contato sao entidades separadas: a piscina pode ter
    // profundidade visual sem transformar tres linhas inteiras em hitbox.
    commands.spawn((
        LevelGeometry,
        HazardVisual {
            kind,
            cols,
            phase: 0,
            bubbles: Timer::from_seconds(
                if kind == HazardKind::Spikes {
                    99.0
                } else {
                    0.18
                },
                TimerMode::Repeating,
            ),
        },
        AsciiSprite::pivoted(hazard_art(kind, cols, 0), Vec2::new(0.0, 0.5)),
        Layer::Terrain,
        // O topo da arte fica alinhado ao centro da faixa de contato.
        Transform::from_translation((at + Vec2::Y * CELL.y * 0.5).extend(0.0)),
    ));
    commands.spawn((
        LevelGeometry,
        Hazard::always(kind),
        Transform::from_translation(at.extend(0.0)),
        Collider::size(cols as f32 * CELL.x, CELL.y),
    ));
}

/// Fracao do ciclo que a fonte passa aberta.
///
/// Curta de proposito: uma fonte aberta metade do tempo e uma parede com
/// intervalo, e o mapa perde a rota em vez de ganhar ritmo.
const JET_WINDOW: (f32, f32) = (0.62, 0.86);
/// Onde, dentro da janela, a coluna esta na altura cheia.
const JET_HOLD: (f32, f32) = (0.18, 0.74);

/// Altura do jorro agora, de 0 a 1.
fn jet_rise(t: f32) -> f32 {
    let (from, to) = JET_WINDOW;
    if t < from || t >= to {
        return 0.0;
    }
    // Sobe rapido, segura, desaba. Sem o patamar o jorro e um piscar; sem a
    // subida rapida ele nao tem estouro.
    let (hold, drop) = JET_HOLD;
    match (t - from) / (to - from) {
        u if u < hold => u / hold,
        u if u < drop => 1.0,
        u => (1.0 - u) / (1.0 - drop),
    }
}

/// Quando a coluna morde.
///
/// Mais estreita que a janela do desenho, e nao igual a ela. A zona de contato
/// cobre a coluna inteira de uma vez -- encolhe-la junto com a arte custaria um
/// `Collider` reescrito por quadro. Armada na janela cheia, ela cobraria dano
/// no topo enquanto a coluna ainda esta na altura de um degrau: hitbox
/// invisivel. Assim o erro cai para o outro lado, que e o lado certo -- fogo
/// visivel que ainda nao machuca ensina; dano vindo do nada, nao.
fn jet_beat(period: f32, phase: f32) -> Beat {
    let (from, to) = JET_WINDOW;
    let span = to - from;
    Beat {
        period,
        phase,
        from: from + JET_HOLD.0 * span,
        to: from + JET_HOLD.1 * span,
    }
}

/// Uma fonte: coluna que aparece e some, e a zona que ela arma junto.
fn geyser(
    commands: &mut Commands,
    at: Vec2,
    cols: u16,
    rows: u16,
    period: f32,
    phase: f32,
    kind: HazardKind,
) {
    let beat = jet_beat(period, phase);
    let height = rows as f32 * CELL.y;

    commands.spawn((
        LevelGeometry,
        Jet {
            kind,
            cols,
            rows,
            beat,
            drawn: 0,
            bubbles: Timer::from_seconds(0.11, TimerMode::Repeating),
        },
        AsciiSprite::footed(AsciiArt::default()),
        Layer::Terrain,
        Transform::from_translation(at.extend(0.0)),
    ));
    // A zona cobre a coluna inteira e so morde na janela. Encolhe-la junto com
    // a arte custaria um `Collider` reescrito por frame para separar o pe do
    // topo de um jorro que dura um quarto de segundo.
    commands.spawn((
        LevelGeometry,
        Hazard {
            kind,
            beat: Some(beat),
        },
        Transform::from_translation((at + Vec2::Y * height * 0.5).extend(0.0)),
        Collider::size(cols as f32 * CELL.x, height),
    ));
}

/// Uma mare: a poca inteira sobe e desce, arte e zona de contato juntas.
fn tide(
    commands: &mut Commands,
    at: Vec2,
    cols: u16,
    rise: u16,
    period: f32,
    phase: f32,
    kind: HazardKind,
) {
    let swell = |home: f32| Bob {
        home,
        rise: rise as f32 * CELL.y,
        period,
        phase,
    };
    // A arte e alta o bastante para o corpo da poca continuar tapando o fundo
    // do tanque na mare cheia. Com a altura da poca parada, a mare sobe e
    // deixa um vao de nada entre a superficie e o piso.
    commands.spawn((
        LevelGeometry,
        HazardVisual {
            kind,
            cols,
            phase: 0,
            bubbles: Timer::from_seconds(0.14, TimerMode::Repeating),
        },
        swell(at.y + CELL.y * 0.5),
        AsciiSprite::pivoted(hazard_art(kind, cols, 0), Vec2::new(0.0, 0.5)),
        Layer::Terrain,
        Transform::from_translation((at + Vec2::Y * CELL.y * 0.5).extend(0.0)),
    ));
    commands.spawn((
        LevelGeometry,
        Hazard::always(kind),
        swell(at.y),
        Transform::from_translation(at.extend(0.0)),
        Collider::size(cols as f32 * CELL.x, CELL.y),
    ));
}

/// Uma goteira: a boca fica, as gotas nascem dela.
fn spout(
    commands: &mut Commands,
    from: Vec2,
    cols: u16,
    floor: f32,
    period: f32,
    phase: f32,
    kind: HazardKind,
) {
    let mut clock = Timer::from_seconds(period, TimerMode::Repeating);
    // Avanca o relogio no nascimento: sem isto, todas as bocas de um mapa
    // pingam no mesmo instante, e a chuva vira um metronomo.
    clock.set_elapsed(std::time::Duration::from_secs_f32(
        period * phase.rem_euclid(1.0),
    ));

    commands.spawn((
        LevelGeometry,
        Spout {
            kind,
            cols,
            floor,
            clock,
        },
        AsciiSprite::new(AsciiArt::fill('╥', cols, 1, palette::IRON)),
        Layer::Terrain,
        Transform::from_translation(from.extend(0.0)),
    ));
}

fn animate_hazards(
    time: Res<Time>,
    mut commands: Commands,
    mut hazards: Query<(&mut HazardVisual, &mut AsciiSprite, &Transform)>,
) {
    for (mut hazard, mut sprite, transform) in &mut hazards {
        let phase = (time.elapsed_secs() * 7.0).floor() as u8 & 3;
        if phase != hazard.phase {
            hazard.phase = phase;
            sprite.art = hazard_art(hazard.kind, hazard.cols, phase);
        }
        if hazard.kind == HazardKind::Spikes || !hazard.bubbles.tick(time.delta()).just_finished() {
            continue;
        }
        let width = hazard.cols as f32 * CELL.x;
        let count = if hazard.kind == HazardKind::Lava {
            2
        } else {
            1
        };
        for _ in 0..count {
            let x = transform.translation.x + (fastrand::f32() - 0.5) * (width - CELL.x);
            let lava = hazard.kind == HazardKind::Lava;
            commands.spawn((
                LevelGeometry,
                AsciiSprite::new(AsciiArt::glyph(
                    if lava && fastrand::bool() { '*' } else { '°' },
                    if lava { palette::MAGMA } else { palette::TOXIC },
                )),
                Layer::Fx,
                Transform::from_translation(Vec3::new(x, transform.translation.y - 2.0, 0.0)),
                Velocity(if lava {
                    Vec2::new(fastrand::f32() * 70.0 - 35.0, 55.0 + fastrand::f32() * 75.0)
                } else {
                    Vec2::new(fastrand::f32() * 16.0 - 8.0, 25.0 + fastrand::f32() * 20.0)
                }),
                Ghost,
                Lifetime(Timer::from_seconds(
                    if lava { 0.8 } else { 0.65 },
                    TimerMode::Once,
                )),
            ));
        }
    }
}

/// Faz a mare subir e descer -- arte e zona de contato pela mesma conta.
fn swell_tides(time: Res<Time>, mut tides: Query<(&Bob, &mut Transform)>) {
    let now = time.elapsed_secs();
    for (bob, mut transform) in &mut tides {
        // Cosseno, e nao dente de serra: a mare tem que demorar nos extremos e
        // passar rapido pelo meio, senao ela le como elevador.
        let wave = 0.5 - (cycle(now, bob.period, bob.phase) * std::f32::consts::TAU).cos() * 0.5;
        transform.translation.y = bob.home + bob.rise * wave;
    }
}

/// Toca as fontes: aviso, jorro e desabamento.
fn erupt_geysers(
    time: Res<Time>,
    mut commands: Commands,
    mut jets: Query<(&mut Jet, &mut AsciiSprite, &Transform)>,
) {
    let now = time.elapsed_secs();
    for (mut jet, mut sprite, transform) in &mut jets {
        let t = cycle(now, jet.beat.period, jet.beat.phase);
        let rows = (jet_rise(t) * jet.rows as f32).round() as u16;

        // A arte so e remontada quando a altura muda de linha: um jorro de
        // doze celulas reconstruido a sessenta quadros por segundo respawna
        // mais glifo por segundo que a briga inteira.
        let frame = rows as u8;
        if frame != jet.drawn {
            jet.drawn = frame;
            sprite.art = if rows == 0 {
                AsciiArt::default()
            } else {
                jet_art(jet.kind, jet.cols, rows, (now * 9.0) as u8 & 3)
            };
        }

        // O aviso: antes de abrir, a boca borbulha. Sem ele a fonte e uma
        // armadilha invisivel, e morrer para uma delas nao ensina nada. Ele
        // olha para a janela do desenho, e nao para a da zona de contato: o
        // que tem que ser anunciado e o instante em que a coluna aparece.
        let warning = t > JET_WINDOW.0 - 0.12 && t < JET_WINDOW.0;
        if !(warning || rows > 0) || !jet.bubbles.tick(time.delta()).just_finished() {
            continue;
        }
        let at = transform.translation.truncate();
        let width = jet.cols as f32 * CELL.x;
        commands.spawn((
            LevelGeometry,
            AsciiSprite::new(AsciiArt::glyph(
                if warning { '°' } else { '*' },
                jet.kind.spray(),
            )),
            Layer::Fx,
            Transform::from_translation(
                (at + Vec2::new((fastrand::f32() - 0.5) * width, rows as f32 * CELL.y * 0.9))
                    .extend(0.0),
            ),
            Velocity(Vec2::new(
                fastrand::f32() * 90.0 - 45.0,
                if warning { 40.0 } else { 190.0 } + fastrand::f32() * 90.0,
            )),
            Ghost,
            Falls,
            Lifetime(Timer::from_seconds(0.6, TimerMode::Once)),
        ));
    }
}

/// Solta as gotas das bocas.
fn drip_spouts(
    time: Res<Time>,
    mut commands: Commands,
    mut spouts: Query<(&mut Spout, &Transform)>,
) {
    for (mut spout, transform) in &mut spouts {
        if !spout.clock.tick(time.delta()).just_finished() {
            continue;
        }
        let at = transform.translation.truncate();
        let x = at.x + (fastrand::f32() - 0.5) * spout.cols as f32 * CELL.x;
        commands.spawn((
            LevelGeometry,
            Droplet {
                kind: spout.kind,
                floor: spout.floor,
            },
            // Uma gota e um perigo que anda: reaproveitar a zona de contato da
            // poca parada e o que garante que ela ferve pela mesma regra, com
            // o mesmo tempo de invulnerabilidade depois.
            Hazard::always(spout.kind),
            Collider::size(CELL.x, CELL.y * 0.8),
            AsciiSprite::new(AsciiArt::glyph('\'', spout.kind.spray())),
            Layer::Projectile,
            Transform::from_translation(Vec3::new(x, at.y - CELL.y, 0.0)),
            Velocity(Vec2::new(0.0, -60.0)),
            Ghost,
            Falls,
        ));
    }
}

/// Desmancha a gota quando ela chega ao chao que a boca aponta.
fn splash_droplets(mut commands: Commands, drops: Query<(Entity, &Droplet, &Transform)>) {
    for (entity, drop, transform) in &drops {
        if transform.translation.y > drop.floor {
            continue;
        }
        let at = transform.translation.truncate();
        commands.entity(entity).despawn();
        for _ in 0..3 {
            commands.spawn((
                LevelGeometry,
                AsciiSprite::new(AsciiArt::glyph('·', drop.kind.spray())),
                Layer::Fx,
                Transform::from_translation(at.extend(0.0)),
                Velocity(Vec2::new(
                    fastrand::f32() * 110.0 - 55.0,
                    40.0 + fastrand::f32() * 60.0,
                )),
                Ghost,
                Falls,
                Lifetime(Timer::from_seconds(0.3, TimerMode::Once)),
            ));
        }
    }
}

fn hurt_on_hazards(
    time: Res<Time>,
    mut commands: Commands,
    mut damaged: MessageWriter<Damaged>,
    hazards: Query<(&Hazard, &Transform, &Collider)>,
    mut players: Query<
        (Entity, &Transform, &Collider, &mut Health, &mut Velocity),
        (With<Player>, Without<HazardCooldown>),
    >,
) {
    for (entity, transform, collider, mut health, mut velocity) in &mut players {
        if health.is_dead() {
            continue;
        }
        let body = collider.aabb(transform.translation.truncate());
        let now = time.elapsed_secs();
        let Some((hazard, _, _)) = hazards.iter().find(|(hazard, at, area)| {
            hazard.armed(now) && overlap(body, area.aabb(at.translation.truncate()))
        }) else {
            continue;
        };
        health.hp -= hazard.kind.damage();
        velocity.0 = hazard.kind.knockback(velocity.0);
        commands.entity(entity).insert((
            HazardCooldown(Timer::from_seconds(0.65, TimerMode::Once)),
            Stunned(Timer::from_seconds(0.22, TimerMode::Once)),
        ));
        damaged.write(Damaged {
            target: entity,
            amount: hazard.kind.damage(),
            at: transform.translation.truncate(),
            dir: Vec2::Y,
            move_name: "HAZARD",
            // Lava desmancha; espinho e acido so furam. Quem morre queimado
            // merece a mesma saida de quem morre num estouro.
            explosive: hazard.kind == HazardKind::Lava,
        });
    }
}

fn tick_hazard_cooldowns(
    time: Res<Time>,
    mut commands: Commands,
    mut players: Query<(Entity, &mut HazardCooldown)>,
) {
    for (entity, mut cooldown) in &mut players {
        if cooldown.0.tick(time.delta()).is_finished() {
            commands.entity(entity).remove::<HazardCooldown>();
        }
    }
}

/// Corrente feita de elos fisicos sobrepostos, nao de uma coluna decorativa.
fn chain(commands: &mut Commands, id: u8, top: Vec2, rows: u16) {
    for index in 0..rows {
        let at = top - Vec2::Y * index as f32 * LINK_LENGTH;
        let (glyph, color) = if index == 0 {
            ("\u{2566}", palette::ASH)
        } else if index % 2 == 0 {
            ("\u{256B}", palette::MOSS)
        } else {
            ("\u{2551}", palette::ASH)
        };
        commands.spawn((
            LevelGeometry,
            Climbable,
            ChainParticle {
                chain: id,
                index,
                previous: at,
                pinned: index == 0,
            },
            AsciiSprite::new(AsciiArt::solid(glyph, color)),
            Layer::Terrain,
            Transform::from_translation(at.extend(0.0)),
            Collider::size(16.0, 18.0),
        ));
    }
}

fn simulate_chains(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut ChainParticle, &mut Transform), Without<Velocity>>,
) {
    struct Link {
        entity: Entity,
        chain: u8,
        index: u16,
        pos: Vec2,
        previous: Vec2,
        pinned: bool,
    }

    let dt = time.delta_secs().min(1.0 / 30.0);
    let mut links: Vec<Link> = query
        .iter()
        .map(|(entity, particle, transform)| Link {
            entity,
            chain: particle.chain,
            index: particle.index,
            pos: transform.translation.truncate(),
            previous: particle.previous,
            pinned: particle.pinned,
        })
        .collect();
    links.sort_by_key(|link| (link.chain, link.index));

    for link in &mut links {
        if !link.pinned {
            let velocity = (link.pos - link.previous) * 0.992;
            link.previous = link.pos;
            link.pos += velocity + Vec2::new(0.0, -900.0) * dt * dt;
        }
    }

    for _ in 0..7 {
        for i in 1..links.len() {
            let (left, right) = links.split_at_mut(i);
            let a = &mut left[i - 1];
            let b = &mut right[0];
            if a.chain != b.chain || b.index != a.index + 1 {
                continue;
            }
            let delta = b.pos - a.pos;
            let error = delta.length() - LINK_LENGTH;
            let correction = delta.normalize_or(Vec2::Y) * error;
            match (a.pinned, b.pinned) {
                (true, false) => b.pos -= correction,
                (false, true) => a.pos += correction,
                (false, false) => {
                    a.pos += correction * 0.5;
                    b.pos -= correction * 0.5;
                }
                (true, true) => {}
            }
        }
    }

    for i in 0..links.len() {
        if links[i].pos.y < KILL_Y - 100.0 {
            commands.entity(links[i].entity).despawn();
            continue;
        }
        let direction = if i > 0
            && links[i - 1].chain == links[i].chain
            && links[i - 1].index + 1 == links[i].index
        {
            links[i].pos - links[i - 1].pos
        } else {
            Vec2::NEG_Y
        };
        if let Ok((_, mut particle, mut transform)) = query.get_mut(links[i].entity) {
            particle.previous = links[i].previous;
            transform.translation.x = links[i].pos.x;
            transform.translation.y = links[i].pos.y;
            transform.rotation =
                Quat::from_rotation_z(direction.to_angle() + std::f32::consts::FRAC_PI_2);
        }
    }
}

fn break_chains(
    mut commands: Commands,
    projectiles: Query<(Entity, &Transform, &Collider), With<Projectile>>,
    links: Query<(Entity, &Transform, &Collider), With<ChainParticle>>,
) {
    for (projectile, shot_transform, shot_collider) in &projectiles {
        let shot = shot_collider.aabb(shot_transform.translation.truncate());
        for (link, link_transform, link_collider) in &links {
            if !overlap(
                shot,
                link_collider.aabb(link_transform.translation.truncate()),
            ) {
                continue;
            }
            let at = link_transform.translation.truncate();
            commands.entity(projectile).despawn();
            commands.entity(link).despawn();
            for _ in 0..6 {
                commands.spawn((
                    AsciiSprite::new(AsciiArt::solid("*", palette::GOLD)),
                    Layer::Fx,
                    Transform::from_translation(at.extend(0.0)),
                    Velocity(Vec2::new(
                        fastrand::f32() * 260.0 - 130.0,
                        80.0 + fastrand::f32() * 180.0,
                    )),
                    Ghost,
                    Falls,
                    Lifetime(Timer::from_seconds(0.35, TimerMode::Once)),
                    DespawnOnExit(GameState::Fighting),
                ));
            }
            break;
        }
    }
}

/// Primeiro mapa: chao com dois buracos, quatro plataformas e duas correntes.
pub struct Arena01;

impl Level for Arena01 {
    fn name(&self) -> &'static str {
        "ARENA 01 - THE GAP"
    }

    fn spawn_points(&self) -> &'static [Vec2] {
        // Os dois primeiros sao os extremos de sempre: uma sala de dois nasce
        // identica ao que era antes de existir sala de quatro. Os outros dois
        // sao o trecho central de chao, simetricos em torno dele.
        const POINTS: [Vec2; 4] = [
            Vec2::new(-500.0, 0.0),
            Vec2::new(500.0, 0.0),
            Vec2::new(-150.0, 0.0),
            Vec2::new(60.0, 0.0),
        ];
        &POINTS
    }

    fn drop_points(&self) -> &'static [Vec2] {
        const POINTS: [Vec2; 4] = [
            Vec2::new(-420.0, 180.0),
            Vec2::new(-60.0, 200.0),
            Vec2::new(330.0, 180.0),
            Vec2::new(140.0, 220.0),
        ];
        &POINTS
    }

    fn pieces(&self) -> &'static [Piece] {
        // Chao em tres trechos; os vaos entre eles sao os buracos.
        //
        // As quatro correntes nao sao enfeite: as plataformas altas estao a
        // mais de 93 unidades do chao, que e o teto do pulo, entao escalar e o
        // unico jeito de chegar la -- e por isso o letreiro promete CLIMB.
        const PIECES: [Piece; 11] = [
            Piece::Terrain {
                top: Vec2::new(-460.0, -170.0),
                cols: 45,
                rows: 6,
            },
            Piece::Terrain {
                top: Vec2::new(-45.0, -170.0),
                cols: 47,
                rows: 6,
            },
            Piece::Terrain {
                top: Vec2::new(415.0, -170.0),
                cols: 45,
                rows: 6,
            },
            Piece::Platform {
                at: Vec2::new(-430.0, -40.0),
                cols: 14,
            },
            Piece::Platform {
                at: Vec2::new(-70.0, 40.0),
                cols: 16,
            },
            Piece::Platform {
                at: Vec2::new(340.0, -40.0),
                cols: 14,
            },
            Piece::Platform {
                at: Vec2::new(150.0, 150.0),
                cols: 12,
            },
            Piece::Chain {
                top: Vec2::new(-210.0, 150.0),
                links: 24,
            },
            Piece::Chain {
                top: Vec2::new(470.0, 190.0),
                links: 28,
            },
            // Estas duas servem as plataformas que antes nao tinham acesso
            // nenhum -- e onde caem duas das quatro armas.
            Piece::Chain {
                top: Vec2::new(-340.0, 150.0),
                links: 22,
            },
            Piece::Chain {
                top: Vec2::new(250.0, 185.0),
                links: 24,
            },
        ];
        &PIECES
    }

    fn skyline(&self) -> &'static [Building] {
        const SKYLINE: [Building; 7] = [
            (-565.0, 25.0, 17, 13),
            (-410.0, 5.0, 21, 10),
            (-230.0, 40.0, 15, 15),
            (-55.0, 0.0, 23, 10),
            (150.0, 28.0, 18, 14),
            (335.0, 8.0, 22, 11),
            (535.0, 36.0, 18, 15),
        ];
        &SKYLINE
    }

    fn signs(&self) -> &'static [Sign] {
        const SIGNS: [Sign; 2] = [
            ("[ KNOCKOUT DISTRICT ]", 188.0, palette::SCENE_RED, 0.0),
            ("PUNCH // CLIMB // SURVIVE", 164.0, palette::SCENE_GOLD, 2.3),
        ];
        &SIGNS
    }
}

/// Segundo mapa: tres torres separadas por vaos largos.
///
/// A Arena 01 e uma briga no chao com dois buracos para evitar. Aqui nao existe
/// chao: o piso e a excecao, e atravessar o mapa exige plataforma ou corrente.
/// Isso troca a ameaca principal de dano para queda sem mudar regra nenhuma.
pub struct Arena02;

impl Level for Arena02 {
    fn name(&self) -> &'static str {
        "ARENA 02 - THE STACKS"
    }

    fn spawn_points(&self) -> &'static [Vec2] {
        // Torres externas primeiro; os dois extras caem nas varandas altas, que
        // sao os unicos apoios simetricos que sobram sem colar num dos dois.
        const POINTS: [Vec2; 4] = [
            Vec2::new(-520.0, -85.0),
            Vec2::new(520.0, -85.0),
            Vec2::new(-200.0, 175.0),
            Vec2::new(200.0, 175.0),
        ];
        &POINTS
    }

    fn drop_points(&self) -> &'static [Vec2] {
        // Cada ponto tem que ter chao embaixo, senao a arma nasce e cai direto
        // no vao.
        const POINTS: [Vec2; 4] = [
            Vec2::new(0.0, 215.0),
            Vec2::new(-200.0, 215.0),
            Vec2::new(200.0, 215.0),
            Vec2::new(-380.0, 200.0),
        ];
        &POINTS
    }

    fn pieces(&self) -> &'static [Piece] {
        // A escada de cada lado sobe em degraus de 50 a 70 unidades. O pulo
        // alcanca 93 de altura, entao todo degrau cabe com folga -- e o teste
        // `todo_patamar_e_alcancavel` nao deixa isso apodrecer.
        const PIECES: [Piece; 13] = [
            // torres externas
            Piece::Terrain {
                top: Vec2::new(-520.0, -120.0),
                cols: 20,
                rows: 12,
            },
            Piece::Terrain {
                top: Vec2::new(520.0, -120.0),
                cols: 20,
                rows: 12,
            },
            // torre central, a mais alta: quem a domina bate de cima, mas tem
            // menos chao pra errar.
            Piece::Terrain {
                top: Vec2::new(0.0, 10.0),
                cols: 16,
                rows: 20,
            },
            // escada esquerda
            Piece::Platform {
                at: Vec2::new(-380.0, -70.0),
                cols: 8,
            },
            Piece::Platform {
                at: Vec2::new(-250.0, 0.0),
                cols: 9,
            },
            Piece::Platform {
                at: Vec2::new(-130.0, 60.0),
                cols: 10,
            },
            // escada direita
            Piece::Platform {
                at: Vec2::new(380.0, -70.0),
                cols: 8,
            },
            Piece::Platform {
                at: Vec2::new(250.0, 0.0),
                cols: 9,
            },
            Piece::Platform {
                at: Vec2::new(130.0, 60.0),
                cols: 10,
            },
            // varanda alta dos dois lados: rota rapida, mas exposta ao tiro de
            // quem esta na torre central.
            Piece::Platform {
                at: Vec2::new(-200.0, 130.0),
                cols: 10,
            },
            Piece::Platform {
                at: Vec2::new(200.0, 130.0),
                cols: 10,
            },
            // correntes nos corredores livres entre os degraus
            Piece::Chain {
                top: Vec2::new(-320.0, 175.0),
                links: 20,
            },
            Piece::Chain {
                top: Vec2::new(320.0, 175.0),
                links: 20,
            },
        ];
        &PIECES
    }

    fn skyline(&self) -> &'static [Building] {
        // Mais altos e mais estreitos que os da Arena 01: o fundo repete a
        // verticalidade da geometria jogavel.
        const SKYLINE: [Building; 8] = [
            (-600.0, 30.0, 10, 16),
            (-460.0, 5.0, 12, 13),
            (-310.0, 45.0, 9, 18),
            (-165.0, 15.0, 13, 14),
            (10.0, 40.0, 10, 17),
            (175.0, 8.0, 12, 13),
            (350.0, 38.0, 9, 18),
            (525.0, 12.0, 13, 14),
        ];
        &SKYLINE
    }

    fn signs(&self) -> &'static [Sign] {
        const SIGNS: [Sign; 2] = [
            ("[ SCRAP TOWER 7 ]", 188.0, palette::SCENE_TOXIC, 1.1),
            ("MIND THE GAP", 164.0, palette::SCENE_RED, 0.4),
        ];
        &SIGNS
    }
}

/// Terceiro mapa: chao inteiro, sem buraco nenhum, e teto nos dois lados.
///
/// As outras duas fases decidem a briga pela queda. Esta decide pelo dano, e
/// usa o teto para dividir o espaco: encostado na parede o teto e baixo, o
/// pulo morre cedo e so sobra o jogo de chao -- combo e rasteira. No meio a
/// sala abre, e ai o gancho e a voadora voltam a valer. Onde voce esta decide
/// que golpes voce tem.
pub struct Arena03;

impl Level for Arena03 {
    fn name(&self) -> &'static str {
        "ARENA 03 - THE VAULT"
    }

    fn spawn_points(&self) -> &'static [Vec2] {
        // Os dois de sempre nascem sob as lajes laterais; os extras caem no
        // primeiro degrau do vao central -- o pedaco aberto do mapa, onde o
        // gancho e a voadora valem.
        const POINTS: [Vec2; 4] = [
            Vec2::new(-520.0, -110.0),
            Vec2::new(520.0, -110.0),
            Vec2::new(-120.0, -40.0),
            Vec2::new(120.0, -40.0),
        ];
        &POINTS
    }

    fn drop_points(&self) -> &'static [Vec2] {
        // Todos no vao central aberto: sob o teto a arma cairia em cima dele,
        // fora do alcance. Isso tambem faz do centro o lugar disputado.
        const POINTS: [Vec2; 4] = [
            Vec2::new(0.0, 215.0),
            Vec2::new(-120.0, 190.0),
            Vec2::new(120.0, 190.0),
            Vec2::new(0.0, 120.0),
        ];
        &POINTS
    }

    fn pieces(&self) -> &'static [Piece] {
        const PIECES: [Piece; 8] = [
            // chao continuo de ponta a ponta: aqui nao se perde caindo
            Piece::Terrain {
                top: Vec2::new(0.0, -170.0),
                cols: 160,
                rows: 6,
            },
            // as duas lajes que fecham as laterais
            Piece::Ceiling {
                bottom: Vec2::new(-460.0, -70.0),
                cols: 45,
                rows: 6,
            },
            Piece::Ceiling {
                bottom: Vec2::new(460.0, -70.0),
                cols: 45,
                rows: 6,
            },
            // escada do vao central, em degraus de 70
            Piece::Platform {
                at: Vec2::new(-120.0, -100.0),
                cols: 10,
            },
            Piece::Platform {
                at: Vec2::new(120.0, -100.0),
                cols: 10,
            },
            Piece::Platform {
                at: Vec2::new(-70.0, -30.0),
                cols: 9,
            },
            Piece::Platform {
                at: Vec2::new(70.0, -30.0),
                cols: 9,
            },
            // o poleiro: quem o segura domina o unico pedaco de ceu do mapa
            Piece::Platform {
                at: Vec2::new(0.0, 40.0),
                cols: 14,
            },
        ];
        &PIECES
    }

    fn skyline(&self) -> &'static [Building] {
        // Poucos e baixos: quase tudo fica escondido atras das lajes, entao
        // gastar predio aqui seria desenhar pro nada.
        const SKYLINE: [Building; 4] = [
            (-330.0, 10.0, 14, 12),
            (-110.0, 34.0, 11, 15),
            (110.0, 30.0, 12, 14),
            (330.0, 8.0, 14, 12),
        ];
        &SKYLINE
    }

    fn signs(&self) -> &'static [Sign] {
        const SIGNS: [Sign; 2] = [
            ("[ THE VAULT ]", 188.0, palette::SCENE_GOLD, 0.7),
            ("NO EXIT // NO FALLS", 164.0, palette::SCENE_RED, 1.9),
        ];
        &SIGNS
    }
}

/// Arena tematica descrita so por dados. Assim os nove mapas novos nao
/// repetem nove vezes a mesma implementacao de `Level`.
struct ThemedArena(&'static StageDef);

struct StageDef {
    name: &'static str,
    scene: Scene,
    spawns: &'static [Vec2],
    drops: &'static [Vec2],
    pieces: &'static [Piece],
    skyline: &'static [Building],
    signs: &'static [Sign],
}

impl Level for ThemedArena {
    fn name(&self) -> &'static str {
        self.0.name
    }
    fn spawn_points(&self) -> &'static [Vec2] {
        self.0.spawns
    }
    fn drop_points(&self) -> &'static [Vec2] {
        self.0.drops
    }
    fn pieces(&self) -> &'static [Piece] {
        self.0.pieces
    }
    fn skyline(&self) -> &'static [Building] {
        self.0.skyline
    }
    fn signs(&self) -> &'static [Sign] {
        self.0.signs
    }
    fn scene(&self) -> Scene {
        self.0.scene
    }
}

const SPAWNS_WIDE: [Vec2; 4] = [
    Vec2::new(-500.0, 0.0),
    Vec2::new(500.0, 0.0),
    Vec2::new(-220.0, 0.0),
    Vec2::new(220.0, 0.0),
];
const SPAWNS_INNER: [Vec2; 4] = [
    Vec2::new(-430.0, 0.0),
    Vec2::new(430.0, 0.0),
    Vec2::new(-140.0, 0.0),
    Vec2::new(140.0, 0.0),
];
/// Os dois patamares da ponte: no desfiladeiro nao ha chao no meio, entao os
/// quatro lugares tem que caber nas bordas.
const SPAWNS_CHASM: [Vec2; 4] = [
    Vec2::new(-560.0, 0.0),
    Vec2::new(560.0, 0.0),
    Vec2::new(-390.0, 0.0),
    Vec2::new(390.0, 0.0),
];
/// As duas alas da fabrica, fora da bacia que enche.
const SPAWNS_YARD: [Vec2; 4] = [
    Vec2::new(-560.0, 0.0),
    Vec2::new(560.0, 0.0),
    Vec2::new(-330.0, 0.0),
    Vec2::new(330.0, 0.0),
];
const DROPS: [Vec2; 4] = [
    Vec2::new(-420.0, 190.0),
    Vec2::new(420.0, 190.0),
    Vec2::new(-120.0, 210.0),
    Vec2::new(120.0, 210.0),
];
const VOLCANO_SKY: [Building; 5] = [
    (-540.0, -10.0, 18, 9),
    (-310.0, 16.0, 16, 12),
    (0.0, -20.0, 24, 8),
    (310.0, 16.0, 16, 12),
    (540.0, -10.0, 18, 9),
];
const INDUSTRIAL_SKY: [Building; 6] = [
    (-550.0, 30.0, 16, 14),
    (-350.0, -5.0, 22, 10),
    (-120.0, 42.0, 14, 16),
    (120.0, 42.0, 14, 16),
    (350.0, -5.0, 22, 10),
    (550.0, 30.0, 16, 14),
];
const ORIENTAL_SKY: [Building; 5] = [
    (-520.0, -20.0, 14, 8),
    (-280.0, 8.0, 18, 10),
    (0.0, 45.0, 20, 15),
    (280.0, 8.0, 18, 10),
    (520.0, -20.0, 14, 8),
];

const CALDERA_SIGNS: [Sign; 2] = [
    ("[ CALDERA // 01 ]", 188.0, palette::SCENE_RED, 0.0),
    ("THE MOUNTAIN IS AWAKE", 164.0, palette::SCENE_GOLD, 1.7),
];
const BRIDGE_SIGNS: [Sign; 2] = [
    ("[ BASALT CROSSING ]", 188.0, palette::SCENE_FIRE, 0.6),
    ("NO GROUND BELOW", 164.0, palette::IRON, 2.0),
];
const FORGE_SIGNS: [Sign; 2] = [
    ("[ FORGE CORE // 03 ]", 188.0, palette::SCENE_FIRE, 1.1),
    ("PRESSURE AT MAXIMUM", 164.0, palette::SCENE_RED, 2.4),
];
const ACID_SIGNS: [Sign; 2] = [
    ("[ ACID WORKS // 01 ]", 188.0, palette::SCENE_TOXIC, 0.4),
    ("CORROSIVE // KEEP MOVING", 164.0, palette::IRON, 2.1),
];
const REACTOR_SIGNS: [Sign; 2] = [
    ("[ REACTOR 02 // ONLINE ]", 188.0, palette::SCENE_BLUE, 0.9),
    ("CORE LOAD UNSTABLE", 164.0, palette::IRON, 2.5),
];
const DRAIN_SIGNS: [Sign; 2] = [
    ("[ DRAINAGE // LEVEL -3 ]", 188.0, palette::SCENE_TOXIC, 1.4),
    ("WATER NOT SAFE", 164.0, palette::SCENE_BLUE, 2.8),
];
const RED_GATE_SIGNS: [Sign; 2] = [
    ("[ RED GATE // ╬╪╫ ]", 188.0, palette::SCENE_RED, 0.8),
    ("ENTER WITH RESPECT", 164.0, palette::IRON, 2.6),
];
const PAGODA_SIGNS: [Sign; 2] = [
    ("[ SUNSET PAGODA ]", 188.0, palette::SCENE_GOLD, 1.2),
    ("FIVE ROOFS // ONE WINNER", 164.0, palette::SCENE_RED, 2.1),
];
const DRAGON_SIGNS: [Sign; 2] = [
    ("[ STONE DRAGON GARDEN ]", 188.0, palette::SCENE_TOXIC, 1.6),
    ("WAKE NOTHING", 164.0, palette::SCENE_GOLD, 2.9),
];

const LAVA_1: [Piece; 10] = [
    Piece::Terrain {
        top: Vec2::new(0.0, -170.0),
        cols: 160,
        rows: 6,
    },
    Piece::Hazard {
        at: Vec2::new(0.0, -162.0),
        cols: 24,
        kind: HazardKind::Lava,
    },
    // Duas fontes fora de fase nas alas: elas fecham a passagem rasteira
    // metade do tempo cada uma, e nunca as duas ao mesmo tempo.
    Piece::Geyser {
        at: Vec2::new(-430.0, -170.0),
        cols: 3,
        rows: 9,
        period: 5.5,
        phase: 0.0,
        kind: HazardKind::Lava,
    },
    Piece::Geyser {
        at: Vec2::new(430.0, -170.0),
        cols: 3,
        rows: 9,
        period: 5.5,
        phase: 0.5,
        kind: HazardKind::Lava,
    },
    Piece::Platform {
        at: Vec2::new(-310.0, -100.0),
        cols: 15,
    },
    Piece::Platform {
        at: Vec2::new(-150.0, -30.0),
        cols: 12,
    },
    Piece::Platform {
        at: Vec2::new(0.0, 40.0),
        cols: 14,
    },
    Piece::Platform {
        at: Vec2::new(150.0, -30.0),
        cols: 12,
    },
    Piece::Platform {
        at: Vec2::new(310.0, -100.0),
        cols: 15,
    },
    Piece::Chain {
        top: Vec2::new(0.0, 190.0),
        links: 11,
    },
];
/// A ponte: chao so nas duas bordas, e a travessia por cima do vazio.
///
/// O letreiro promete NO GROUND BELOW desde a primeira versao; ate agora o
/// mapa era um piso inteirico com duas pocas em cima dele.
const LAVA_2: [Piece; 12] = [
    Piece::Terrain {
        top: Vec2::new(-480.0, -170.0),
        cols: 35,
        rows: 6,
    },
    Piece::Terrain {
        top: Vec2::new(480.0, -170.0),
        cols: 35,
        rows: 6,
    },
    Piece::Hazard {
        at: Vec2::new(-480.0, -162.0),
        cols: 10,
        kind: HazardKind::Lava,
    },
    Piece::Hazard {
        at: Vec2::new(480.0, -162.0),
        cols: 10,
        kind: HazardKind::Lava,
    },
    // Saem do fundo do desfiladeiro, fora da tela, e cortam exatamente os dois
    // vaos que a travessia obriga a pular.
    Piece::Geyser {
        at: Vec2::new(-175.0, -250.0),
        cols: 4,
        rows: 14,
        period: 6.0,
        phase: 0.0,
        kind: HazardKind::Lava,
    },
    Piece::Geyser {
        at: Vec2::new(175.0, -250.0),
        cols: 4,
        rows: 14,
        period: 6.0,
        phase: 0.5,
        kind: HazardKind::Lava,
    },
    Piece::Platform {
        at: Vec2::new(-250.0, -105.0),
        cols: 13,
    },
    Piece::Platform {
        at: Vec2::new(250.0, -105.0),
        cols: 13,
    },
    Piece::Platform {
        at: Vec2::new(0.0, -45.0),
        cols: 30,
    },
    Piece::Platform {
        at: Vec2::new(0.0, 30.0),
        cols: 12,
    },
    Piece::Chain {
        top: Vec2::new(-90.0, 180.0),
        links: 9,
    },
    Piece::Chain {
        top: Vec2::new(90.0, 180.0),
        links: 9,
    },
];
const LAVA_3: [Piece; 12] = [
    Piece::Terrain {
        top: Vec2::new(0.0, -170.0),
        cols: 160,
        rows: 6,
    },
    Piece::Hazard {
        at: Vec2::new(0.0, -162.0),
        cols: 12,
        kind: HazardKind::Lava,
    },
    // O forno transborda: as duas pocas das alas sobem ate engolir a beira dos
    // patamares baixos, e descem de novo.
    Piece::Tide {
        at: Vec2::new(-330.0, -162.0),
        cols: 14,
        rise: 5,
        period: 8.0,
        phase: 0.0,
        kind: HazardKind::Lava,
    },
    Piece::Tide {
        at: Vec2::new(330.0, -162.0),
        cols: 14,
        rise: 5,
        period: 8.0,
        phase: 0.5,
        kind: HazardKind::Lava,
    },
    // Pinga do teto: e o unico perigo do jogo que vem de cima, e ele existe
    // porque o teto desta arena ja obriga a briga a ficar rasteira.
    Piece::Drip {
        from: Vec2::new(-100.0, 146.0),
        cols: 6,
        floor: -162.0,
        period: 1.1,
        phase: 0.0,
        kind: HazardKind::Lava,
    },
    Piece::Drip {
        from: Vec2::new(100.0, 146.0),
        cols: 6,
        floor: -162.0,
        period: 1.1,
        phase: 0.5,
        kind: HazardKind::Lava,
    },
    Piece::Platform {
        at: Vec2::new(-400.0, -100.0),
        cols: 12,
    },
    Piece::Platform {
        at: Vec2::new(-200.0, -30.0),
        cols: 12,
    },
    Piece::Platform {
        at: Vec2::new(0.0, 40.0),
        cols: 15,
    },
    Piece::Platform {
        at: Vec2::new(200.0, -30.0),
        cols: 12,
    },
    Piece::Platform {
        at: Vec2::new(400.0, -100.0),
        cols: 12,
    },
    Piece::Ceiling {
        bottom: Vec2::new(0.0, 150.0),
        cols: 34,
        rows: 3,
    },
];

/// A fabrica: duas alas altas e uma bacia funda no meio, que enche.
///
/// A bacia e o coracao do mapa. Descer nela e um atalho e um risco ao mesmo
/// tempo -- os dois patamares que ficam la embaixo so existem enquanto a mare
/// esta baixa, e quem estiver neles quando ela subir tem um ciclo para sair.
const ACID_1: [Piece; 14] = [
    Piece::Terrain {
        top: Vec2::new(-430.0, -170.0),
        cols: 52,
        rows: 6,
    },
    Piece::Terrain {
        top: Vec2::new(430.0, -170.0),
        cols: 52,
        rows: 6,
    },
    Piece::Terrain {
        top: Vec2::new(0.0, -250.0),
        cols: 54,
        rows: 6,
    },
    Piece::Tide {
        at: Vec2::new(0.0, -242.0),
        cols: 50,
        rise: 6,
        period: 10.0,
        phase: 0.0,
        kind: HazardKind::Acid,
    },
    Piece::Platform {
        at: Vec2::new(-120.0, -175.0),
        cols: 12,
    },
    Piece::Platform {
        at: Vec2::new(120.0, -175.0),
        cols: 12,
    },
    // Duas bocas de cano vazando sobre as alas: e o que faz o patio inteiro
    // pedir atencao, e nao so a bacia.
    Piece::Drip {
        from: Vec2::new(-250.0, 150.0),
        cols: 6,
        floor: -170.0,
        period: 1.3,
        phase: 0.0,
        kind: HazardKind::Acid,
    },
    Piece::Drip {
        from: Vec2::new(250.0, 150.0),
        cols: 6,
        floor: -170.0,
        period: 1.3,
        phase: 0.5,
        kind: HazardKind::Acid,
    },
    Piece::Platform {
        at: Vec2::new(-380.0, -100.0),
        cols: 16,
    },
    Piece::Platform {
        at: Vec2::new(380.0, -100.0),
        cols: 16,
    },
    Piece::Platform {
        at: Vec2::new(-170.0, -30.0),
        cols: 14,
    },
    Piece::Platform {
        at: Vec2::new(170.0, -30.0),
        cols: 14,
    },
    Piece::Platform {
        at: Vec2::new(0.0, 40.0),
        cols: 16,
    },
    Piece::Chain {
        top: Vec2::new(0.0, 190.0),
        links: 10,
    },
];
const ACID_2: [Piece; 12] = [
    Piece::Terrain {
        top: Vec2::new(0.0, -170.0),
        cols: 160,
        rows: 6,
    },
    Piece::Tide {
        at: Vec2::new(-300.0, -162.0),
        cols: 16,
        rise: 4,
        period: 7.5,
        phase: 0.0,
        kind: HazardKind::Acid,
    },
    Piece::Tide {
        at: Vec2::new(300.0, -162.0),
        cols: 16,
        rise: 4,
        period: 7.5,
        phase: 0.5,
        kind: HazardKind::Acid,
    },
    // Vazamento do nucleo, bem no eixo do reator.
    Piece::Drip {
        from: Vec2::new(0.0, 148.0),
        cols: 8,
        floor: -170.0,
        period: 0.9,
        phase: 0.0,
        kind: HazardKind::Acid,
    },
    Piece::Platform {
        at: Vec2::new(-450.0, -100.0),
        cols: 12,
    },
    Piece::Platform {
        at: Vec2::new(-250.0, -30.0),
        cols: 12,
    },
    Piece::Platform {
        at: Vec2::new(-70.0, 40.0),
        cols: 12,
    },
    Piece::Platform {
        at: Vec2::new(70.0, 40.0),
        cols: 12,
    },
    Piece::Platform {
        at: Vec2::new(250.0, -30.0),
        cols: 12,
    },
    Piece::Platform {
        at: Vec2::new(450.0, -100.0),
        cols: 12,
    },
    Piece::Ceiling {
        bottom: Vec2::new(0.0, 155.0),
        cols: 26,
        rows: 3,
    },
    Piece::Hazard {
        at: Vec2::new(0.0, -162.0),
        cols: 10,
        kind: HazardKind::Acid,
    },
];
/// A drenagem: a calha do meio enche e esvazia, e com ela a rota rasteira.
const ACID_3: [Piece; 10] = [
    Piece::Terrain {
        top: Vec2::new(0.0, -170.0),
        cols: 160,
        rows: 6,
    },
    Piece::Tide {
        at: Vec2::new(0.0, -162.0),
        cols: 44,
        rise: 5,
        period: 11.0,
        phase: 0.0,
        kind: HazardKind::Acid,
    },
    Piece::Hazard {
        at: Vec2::new(-330.0, -162.0),
        cols: 10,
        kind: HazardKind::Acid,
    },
    Piece::Hazard {
        at: Vec2::new(330.0, -162.0),
        cols: 10,
        kind: HazardKind::Acid,
    },
    Piece::Drip {
        from: Vec2::new(-260.0, 168.0),
        cols: 6,
        floor: -170.0,
        period: 1.5,
        phase: 0.0,
        kind: HazardKind::Acid,
    },
    Piece::Drip {
        from: Vec2::new(260.0, 168.0),
        cols: 6,
        floor: -170.0,
        period: 1.5,
        phase: 0.5,
        kind: HazardKind::Acid,
    },
    Piece::Platform {
        at: Vec2::new(-320.0, -100.0),
        cols: 14,
    },
    Piece::Platform {
        at: Vec2::new(-120.0, -30.0),
        cols: 12,
    },
    Piece::Platform {
        at: Vec2::new(120.0, -30.0),
        cols: 12,
    },
    Piece::Platform {
        at: Vec2::new(320.0, -100.0),
        cols: 14,
    },
];

const EAST_1: [Piece; 10] = [
    Piece::Terrain {
        top: Vec2::new(0.0, -170.0),
        cols: 160,
        rows: 6,
    },
    // A boca de pedra do portao: fogo de jade subindo no eixo da arena.
    Piece::Geyser {
        at: Vec2::new(0.0, -170.0),
        cols: 5,
        rows: 11,
        period: 6.5,
        phase: 0.0,
        kind: HazardKind::Jade,
    },
    Piece::Hazard {
        at: Vec2::new(-280.0, -162.0),
        cols: 8,
        kind: HazardKind::Spikes,
    },
    Piece::Hazard {
        at: Vec2::new(280.0, -162.0),
        cols: 8,
        kind: HazardKind::Spikes,
    },
    Piece::Platform {
        at: Vec2::new(-360.0, -100.0),
        cols: 18,
    },
    Piece::Platform {
        at: Vec2::new(-170.0, -30.0),
        cols: 16,
    },
    Piece::Platform {
        at: Vec2::new(0.0, 40.0),
        cols: 16,
    },
    Piece::Platform {
        at: Vec2::new(170.0, -30.0),
        cols: 16,
    },
    Piece::Platform {
        at: Vec2::new(360.0, -100.0),
        cols: 18,
    },
    Piece::Chain {
        top: Vec2::new(0.0, 195.0),
        links: 11,
    },
];
const EAST_2: [Piece; 12] = [
    Piece::Terrain {
        top: Vec2::new(0.0, -170.0),
        cols: 160,
        rows: 6,
    },
    Piece::Hazard {
        at: Vec2::new(-280.0, -162.0),
        cols: 8,
        kind: HazardKind::Spikes,
    },
    Piece::Hazard {
        at: Vec2::new(280.0, -162.0),
        cols: 8,
        kind: HazardKind::Spikes,
    },
    // Duas fontes de jade sob os patamares do meio: elas nao alcancam o
    // tabuado, mas fecham a descida enquanto estao abertas.
    Piece::Geyser {
        at: Vec2::new(-90.0, -170.0),
        cols: 4,
        rows: 9,
        period: 5.0,
        phase: 0.0,
        kind: HazardKind::Jade,
    },
    Piece::Geyser {
        at: Vec2::new(90.0, -170.0),
        cols: 4,
        rows: 9,
        period: 5.0,
        phase: 0.5,
        kind: HazardKind::Jade,
    },
    Piece::Platform {
        at: Vec2::new(-450.0, -100.0),
        cols: 14,
    },
    Piece::Platform {
        at: Vec2::new(-240.0, -30.0),
        cols: 18,
    },
    Piece::Platform {
        at: Vec2::new(-70.0, 40.0),
        cols: 11,
    },
    Piece::Platform {
        at: Vec2::new(70.0, 40.0),
        cols: 11,
    },
    Piece::Platform {
        at: Vec2::new(240.0, -30.0),
        cols: 18,
    },
    Piece::Platform {
        at: Vec2::new(450.0, -100.0),
        cols: 14,
    },
    Piece::Ceiling {
        bottom: Vec2::new(0.0, 165.0),
        cols: 32,
        rows: 2,
    },
];
/// O jardim do dragao: tres bocas de pedra e dois braseiros, todos de jade.
///
/// O mapa que dava nome ao dragao nao tinha nada de jade nem de dragao no que
/// se joga. Aqui as tres fontes sao o bicho do fundo respirando: a do meio
/// sobe ate lamber o tabuado alto, e as duas das alas cortam a rota rasteira
/// em tempos diferentes.
const EAST_3: [Piece; 12] = [
    Piece::Terrain {
        top: Vec2::new(0.0, -170.0),
        cols: 160,
        rows: 6,
    },
    Piece::Geyser {
        at: Vec2::new(0.0, -170.0),
        cols: 5,
        rows: 13,
        period: 6.0,
        phase: 0.0,
        kind: HazardKind::Jade,
    },
    Piece::Geyser {
        at: Vec2::new(-290.0, -170.0),
        cols: 4,
        rows: 9,
        period: 6.0,
        phase: 0.33,
        kind: HazardKind::Jade,
    },
    Piece::Geyser {
        at: Vec2::new(290.0, -170.0),
        cols: 4,
        rows: 9,
        period: 6.0,
        phase: 0.66,
        kind: HazardKind::Jade,
    },
    Piece::Hazard {
        at: Vec2::new(-190.0, -162.0),
        cols: 8,
        kind: HazardKind::Jade,
    },
    Piece::Hazard {
        at: Vec2::new(190.0, -162.0),
        cols: 8,
        kind: HazardKind::Jade,
    },
    Piece::Platform {
        at: Vec2::new(-380.0, -100.0),
        cols: 15,
    },
    Piece::Platform {
        at: Vec2::new(-190.0, -30.0),
        cols: 13,
    },
    Piece::Platform {
        at: Vec2::new(0.0, 40.0),
        cols: 20,
    },
    Piece::Platform {
        at: Vec2::new(190.0, -30.0),
        cols: 13,
    },
    Piece::Platform {
        at: Vec2::new(380.0, -100.0),
        cols: 15,
    },
    Piece::Chain {
        top: Vec2::new(0.0, 190.0),
        links: 10,
    },
];

macro_rules! stage {
    ($name:literal, $scene:expr, $spawns:expr, $pieces:expr, $sky:expr, $signs:expr) => {
        StageDef {
            name: $name,
            scene: $scene,
            spawns: $spawns,
            drops: &DROPS,
            pieces: $pieces,
            skyline: $sky,
            signs: $signs,
        }
    };
}

const CALDERA: StageDef = stage!(
    "LAVA 01 - CALDERA",
    Scene::Caldera,
    &SPAWNS_WIDE,
    &LAVA_1,
    &VOLCANO_SKY,
    &CALDERA_SIGNS
);
const MAGMA_BRIDGE: StageDef = stage!(
    "LAVA 02 - MAGMA BRIDGE",
    Scene::MagmaBridge,
    &SPAWNS_CHASM,
    &LAVA_2,
    &VOLCANO_SKY,
    &BRIDGE_SIGNS
);
const FORGE_CORE: StageDef = stage!(
    "LAVA 03 - FORGE CORE",
    Scene::ForgeCore,
    &SPAWNS_WIDE,
    &LAVA_3,
    &VOLCANO_SKY,
    &FORGE_SIGNS
);
const ACID_WORKS: StageDef = stage!(
    "INDUSTRIAL 01 - ACID WORKS",
    Scene::AcidWorks,
    &SPAWNS_YARD,
    &ACID_1,
    &INDUSTRIAL_SKY,
    &ACID_SIGNS
);
const REACTOR: StageDef = stage!(
    "INDUSTRIAL 02 - REACTOR",
    Scene::Reactor,
    &SPAWNS_INNER,
    &ACID_2,
    &INDUSTRIAL_SKY,
    &REACTOR_SIGNS
);
const DRAINAGE: StageDef = stage!(
    "INDUSTRIAL 03 - DRAINAGE",
    Scene::Drainage,
    &SPAWNS_WIDE,
    &ACID_3,
    &INDUSTRIAL_SKY,
    &DRAIN_SIGNS
);
const RED_GATE: StageDef = stage!(
    "ORIENTAL 01 - RED GATE",
    Scene::RedGate,
    &SPAWNS_WIDE,
    &EAST_1,
    &ORIENTAL_SKY,
    &RED_GATE_SIGNS
);
const SUNSET_PAGODA: StageDef = stage!(
    "ORIENTAL 02 - SUNSET PAGODA",
    Scene::SunsetPagoda,
    &SPAWNS_INNER,
    &EAST_2,
    &ORIENTAL_SKY,
    &PAGODA_SIGNS
);
const DRAGON_GARDEN: StageDef = stage!(
    "ORIENTAL 03 - DRAGON GARDEN",
    Scene::DragonGarden,
    &SPAWNS_INNER,
    &EAST_3,
    &ORIENTAL_SKY,
    &DRAGON_SIGNS
);

/// Catalogo de fases, na ordem em que o menu as lista.
///
/// Adicionar um mapa e escrever o `Level` e por o construtor aqui -- nenhum
/// outro arquivo precisa saber que ele existe.
pub const CATALOG: [fn() -> Box<dyn Level>; 12] = [
    || Box::new(Arena01),
    || Box::new(Arena02),
    || Box::new(Arena03),
    || Box::new(ThemedArena(&CALDERA)),
    || Box::new(ThemedArena(&MAGMA_BRIDGE)),
    || Box::new(ThemedArena(&FORGE_CORE)),
    || Box::new(ThemedArena(&ACID_WORKS)),
    || Box::new(ThemedArena(&REACTOR)),
    || Box::new(ThemedArena(&DRAINAGE)),
    || Box::new(ThemedArena(&RED_GATE)),
    || Box::new(ThemedArena(&SUNSET_PAGODA)),
    || Box::new(ThemedArena(&DRAGON_GARDEN)),
];

/// Constroi a fase de indice `index`, girando a lista se ele passar do fim.
pub fn level_at(index: usize) -> Box<dyn Level> {
    CATALOG[index % CATALOG.len()]()
}

/// Nome da fase de indice `index`, para o menu.
pub fn level_name(index: usize) -> &'static str {
    level_at(index).name()
}

/// Fase destacada no menu.
///
/// O gameplay so conhece [`CurrentLevel`]; o indice existe apenas para o menu
/// poder girar a lista, e um sistema mantem os dois em sincronia.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct LevelPick(pub usize);

/// Qual fase a geometria que esta no ar representa.
///
/// Sem isso nao ha como perguntar "a arena montada ainda e a fase escolhida?",
/// e a resposta importa: online a fase muda por pacote, no meio de qualquer
/// quadro, e nao so na porta de entrada do estado.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltStage(pub Option<usize>);

/// Traduz o indice escolhido no `CurrentLevel` que o resto do jogo le.
///
/// Roda no `PreUpdate`, logo depois dos pacotes, e nao no `Update`: a troca de
/// estado do Bevy acontece entre os dois. Enquanto isto rodava depois, o
/// cliente que recebia o pacote de inicio entrava na luta com o mapa **antigo**
/// -- o pacote dizia a fase, a transicao levantava a geometria no mesmo quadro,
/// e a fase nova so chegava ao `CurrentLevel` um quadro tarde demais. Era esse
/// o "mapa que nao atualiza" de quem entrava na sala.
fn apply_level_pick(pick: Res<LevelPick>, mut current: ResMut<CurrentLevel>) {
    if pick.is_changed() {
        current.0 = level_at(pick.0);
    }
}

/// Sorteia uma fase diferente da atual.
///
/// Diferente de proposito: repetir o mapa que acabou de ser jogado le como
/// sorteio quebrado, mesmo sendo um resultado legitimo.
pub fn roll_stage(current: usize) -> usize {
    if CATALOG.len() < 2 {
        return current;
    }
    let mut next = fastrand::usize(..CATALOG.len() - 1);
    if next >= current % CATALOG.len() {
        next += 1;
    }
    next
}

/// Gira a fase depois de cada round.
///
/// Quem decide o round tambem decide o mapa seguinte: online a escolha viaja no
/// pacote de inicio, entao os clientes seguem sem precisar sortear nada -- dois
/// sorteios independentes dariam dois mapas.
fn rotate_stage(mut pick: ResMut<LevelPick>) {
    pick.0 = roll_stage(pick.0);
}

/// Monta a fase ao entrar em `Fighting`.
///
/// Um unico lugar traduz peca em entidade. As fases nao spawnam nada por conta
/// propria, entao nao existe mapa com regra de colisao ou de camada diferente
/// dos outros.
fn build_level(
    mut commands: Commands,
    level: Res<CurrentLevel>,
    pick: Res<LevelPick>,
    mut built: ResMut<BuiltStage>,
) {
    built.0 = Some(pick.0);
    raise_level(&mut commands, &level);
}

/// Reergue a arena quando a fase muda com ela ja de pe.
///
/// A porta de entrada do estado nao basta: online a fase chega por pacote e
/// pode trocar com a sala aberta -- e quem estivesse no aquecimento continuaria
/// correndo na geometria da fase anterior, atravessando chao que so existe na
/// tela dele.
fn rebuild_on_stage_change(
    mut commands: Commands,
    level: Res<CurrentLevel>,
    pick: Res<LevelPick>,
    mut built: ResMut<BuiltStage>,
    geometry: Query<Entity, With<LevelGeometry>>,
) {
    if built.0 == Some(pick.0) {
        return;
    }
    for entity in &geometry {
        commands.entity(entity).despawn();
    }
    built.0 = Some(pick.0);
    raise_level(&mut commands, &level);
}

fn raise_level(commands: &mut Commands, level: &CurrentLevel) {
    crate::backdrop::build(
        commands,
        level.0.skyline(),
        level.0.signs(),
        level.0.scene(),
    );

    let mut next_chain = 0u8;
    for piece in level.0.pieces() {
        match *piece {
            Piece::Terrain { top, cols, rows } => {
                let height = rows as f32 * CELL.y;
                terrain(commands, Vec2::new(top.x, top.y - height * 0.5), cols, rows);
            }
            Piece::Ceiling { bottom, cols, rows } => {
                let height = rows as f32 * CELL.y;
                terrain(
                    commands,
                    Vec2::new(bottom.x, bottom.y + height * 0.5),
                    cols,
                    rows,
                );
            }
            Piece::Platform { at, cols } => platform(commands, at, cols),
            Piece::Chain { top, links } => {
                chain(commands, next_chain, top, links);
                next_chain += 1;
            }
            Piece::Hazard { at, cols, kind } => hazard(commands, at, cols, kind),
            Piece::Geyser {
                at,
                cols,
                rows,
                period,
                phase,
                kind,
            } => geyser(commands, at, cols, rows, period, phase, kind),
            Piece::Tide {
                at,
                cols,
                rise,
                period,
                phase,
                kind,
            } => tide(commands, at, cols, rise, period, phase, kind),
            Piece::Drip {
                from,
                cols,
                floor,
                period,
                phase,
                kind,
            } => spout(commands, from, cols, floor, period, phase, kind),
        }
    }
}

/// Limpa a geometria ao sair de `Fighting`.
fn clear_level(
    mut commands: Commands,
    mut built: ResMut<BuiltStage>,
    q: Query<Entity, With<LevelGeometry>>,
) {
    built.0 = None;
    for entity in &q {
        commands.entity(entity).despawn();
    }
}

/// Alcance horizontal de um pulo que sobe `rise`, ou `None` se for alto demais.
///
/// E a solucao do lancamento obliquo com os mesmos numeros que a fisica usa em
/// jogo, entao mexer em `JUMP_SPEED` ou `GRAVITY` reprova mapas que passaram a
/// nao fechar -- que e exatamente o aviso que se quer.
#[cfg(test)]
fn jump_reach(rise: f32) -> Option<f32> {
    use crate::actor::motion::{JUMP_SPEED, RUN_SPEED};
    use crate::physics::GRAVITY;

    let discriminant = JUMP_SPEED * JUMP_SPEED - 2.0 * GRAVITY * rise;
    if discriminant < 0.0 {
        return None;
    }
    let airtime = (JUMP_SPEED + discriminant.sqrt()) / GRAVITY;
    Some(RUN_SPEED * airtime)
}

/// Carrega `Arena01` e liga o ciclo de vida da geometria ao estado de luta.
pub struct LevelPlugin;

impl Plugin for LevelPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CurrentLevel(level_at(0)))
            .init_resource::<LevelPick>()
            .init_resource::<BuiltStage>()
            // Antes da troca de estado do quadro, e depois dos pacotes: e essa
            // ordem que faz o cliente entrar na luta ja com o mapa do host.
            .add_systems(PreUpdate, apply_level_pick.after(crate::online::NetReceive))
            // A fase gira a cada round. Antes do placar aparecer, para a tela
            // conseguir anunciar para onde a briga vai.
            .add_systems(
                OnEnter(GameState::RoundOver),
                rotate_stage
                    .before(crate::ui::spawn_round_over_screen)
                    .run_if(crate::online::can_decide_round),
            )
            .add_systems(
                Update,
                rebuild_on_stage_change
                    .in_set(AppSet::Logic)
                    .run_if(arena_live),
            )
            // A arena existe tanto na espera quanto na luta -- no lobby da pra
            // andar nela. Sair de qualquer um dos dois a derruba, e entrar no
            // outro a levanta de novo: a geometria some por marcador, entao ela
            // nao precisa saber de que estado veio.
            .add_systems(
                OnEnter(GameState::Lobby),
                build_level.run_if(resource_equals(GameMode::Online)),
            )
            .add_systems(OnExit(GameState::Lobby), clear_level)
            .add_systems(OnEnter(GameState::Fighting), build_level)
            .add_systems(OnExit(GameState::Fighting), clear_level)
            .add_systems(
                Update,
                break_chains.in_set(AppSet::Logic).run_if(arena_live),
            )
            .add_systems(
                Update,
                simulate_chains.in_set(AppSet::Physics).run_if(arena_live),
            )
            .add_systems(
                Update,
                (
                    drip_spouts,
                    splash_droplets,
                    hurt_on_hazards,
                    tick_hazard_cooldowns,
                )
                    .chain()
                    .in_set(AppSet::Logic)
                    .run_if(arena_live),
            )
            // A mare anda antes de qualquer coisa perguntar onde ela esta:
            // arte e zona de contato sao entidades diferentes, e mover uma
            // depois de o dano ja ter sido decidido faria a poca ferir na
            // altura do frame anterior.
            .add_systems(
                Update,
                swell_tides
                    .in_set(AppSet::Physics)
                    .before(simulate_chains)
                    .run_if(arena_live),
            )
            .add_systems(
                Update,
                (animate_hazards, erupt_geysers)
                    .in_set(AppSet::Animate)
                    .run_if(arena_live),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Margem sobre o alcance teorico. Um mapa desenhado no limite exato do
    /// pulo e frustrante mesmo quando e possivel.
    const SAFETY: f32 = 0.75;

    /// Um no do grafo de travessia.
    #[derive(Debug, Clone, Copy)]
    enum Node {
        /// Superficie de apoio: uma faixa horizontal numa altura.
        Ledge { x0: f32, x1: f32, y: f32 },
        /// Corrente: uma coluna que se agarra em qualquer altura do trecho.
        Rope { x: f32, low: f32, high: f32 },
    }

    /// Da para ir de `a` a `b`?
    ///
    /// Escalar corrente conta: o jogo trata isso como travessia normal, entao
    /// exigir que tudo se resolva no pulo reprovaria mapas que funcionam.
    fn connects(a: Node, b: Node) -> bool {
        let reachable =
            |climb: f32, gap: f32| jump_reach(climb).is_some_and(|reach| gap <= reach * SAFETY);
        match (a, b) {
            (
                Node::Ledge { x0, x1, y },
                Node::Ledge {
                    x0: bx0,
                    x1: bx1,
                    y: by,
                },
            ) => {
                let gap = (bx0 - x1).max(x0 - bx1).max(0.0);
                // Descer e sempre mais facil que subir, entao o par so conecta
                // se o lado dificil -- a subida -- couber no pulo.
                reachable((by - y).abs(), gap)
            }
            (Node::Ledge { x0, x1, y }, Node::Rope { x, low, high })
            | (Node::Rope { x, low, high }, Node::Ledge { x0, x1, y }) => {
                // Agarra na altura mais conveniente do trecho da corrente.
                let grab = y.clamp(low, high);
                reachable(grab - y, (x0 - x).max(x - x1).max(0.0))
            }
            // Pular de corrente em corrente depende do balanco, que e dinamico
            // demais para um teste estatico prometer.
            (Node::Rope { .. }, Node::Rope { .. }) => false,
        }
    }

    fn footholds(level: &dyn Level) -> Vec<(f32, f32, f32)> {
        level.pieces().iter().filter_map(|p| p.foothold()).collect()
    }

    /// O alcance so vale como criterio se for a fisica do jogo, nao um numero
    /// solto que ninguem revisita quando o pulo muda.
    #[test]
    fn alcance_do_pulo_bate_com_a_fisica() {
        use crate::actor::motion::{JUMP_SPEED, RUN_SPEED};
        use crate::physics::GRAVITY;

        let apex = JUMP_SPEED * JUMP_SPEED / (2.0 * GRAVITY);
        assert!(jump_reach(apex - 1.0).is_some());
        assert!(jump_reach(apex + 1.0).is_none(), "passou do topo do pulo");

        // Sem subida, o alcance e a corrida durante o arco inteiro.
        let flat = jump_reach(0.0).unwrap();
        let expected = RUN_SPEED * 2.0 * JUMP_SPEED / GRAVITY;
        assert!((flat - expected).abs() < 0.5, "arco plano deu {flat}");
    }

    fn graph(level: &dyn Level) -> Vec<Node> {
        level
            .pieces()
            .iter()
            .filter_map(|piece| match *piece {
                Piece::Chain { top, links } => Some(Node::Rope {
                    x: top.x,
                    low: top.y - (links.saturating_sub(1)) as f32 * LINK_LENGTH,
                    high: top.y,
                }),
                // Teto nao e no de travessia: nao se sobe nele nem se agarra.
                Piece::Ceiling { .. } => None,
                other => other
                    .foothold()
                    .map(|(x0, x1, y)| Node::Ledge { x0, x1, y }),
            })
            .collect()
    }

    /// Nenhum patamar pode ficar ilhado.
    ///
    /// Este e o erro que um mapa novo comete calado: a geometria aparece, o
    /// jogo roda, e so quem joga descobre que nao da pra sair da plataforma --
    /// ou que a arma caiu num lugar aonde ninguem chega.
    #[test]
    fn todo_patamar_e_alcancavel() {
        for build in CATALOG {
            let level = build();
            let nodes = graph(level.as_ref());

            // Busca em largura a partir do primeiro patamar; no fim, todo
            // apoio tem que ter sido visitado. Corrente solta e so inutil, nao
            // e erro, entao ela nao entra na cobranca.
            let start = nodes
                .iter()
                .position(|n| matches!(n, Node::Ledge { .. }))
                .expect("fase sem nenhum apoio");
            let mut seen = vec![false; nodes.len()];
            let mut queue = vec![start];
            seen[start] = true;
            while let Some(from) = queue.pop() {
                for (to, node) in nodes.iter().enumerate() {
                    if !seen[to] && connects(nodes[from], *node) {
                        seen[to] = true;
                        queue.push(to);
                    }
                }
            }

            let ilhados: Vec<Node> = nodes
                .iter()
                .zip(&seen)
                .filter(|(node, seen)| !**seen && matches!(node, Node::Ledge { .. }))
                .map(|(node, _)| *node)
                .collect();
            assert!(
                ilhados.is_empty(),
                "{}: patamares fora de alcance {ilhados:?}",
                level.name()
            );
        }
    }

    /// O teto da Arena 03 so muda o jogo se cortar o pulo de verdade.
    ///
    /// Com folga maior que o arco do salto ele viraria decoracao, e a fase
    /// perderia a unica coisa que a distingue: nas laterais nao ha jogo aereo.
    #[test]
    fn o_teto_da_vault_corta_o_pulo() {
        let level = Arena03;
        let sob_o_teto = level.spawn_points()[0];
        let chao = level.ground_under(sob_o_teto).expect("spawn sem chao");
        let cabeca = chao + crate::actor::body_half_height() * 2.0;

        let teto = level
            .pieces()
            .iter()
            .find_map(|piece| match *piece {
                Piece::Ceiling { bottom, cols, .. }
                    if (sob_o_teto.x - bottom.x).abs() <= cols as f32 * CELL.x * 0.5 =>
                {
                    Some(bottom.y)
                }
                _ => None,
            })
            .expect("o spawn lateral nao esta sob teto nenhum");

        use crate::actor::motion::JUMP_SPEED;
        let arco = JUMP_SPEED * JUMP_SPEED / (2.0 * crate::physics::GRAVITY);
        let folga = teto - cabeca;

        assert!(folga > 0.0, "o teto encosta na cabeca de quem esta em pe");
        assert!(
            folga < arco,
            "o teto deixa {folga} de folga e o pulo sobe {arco}: ele nao corta nada"
        );
    }

    /// A percepcao de chao tem que enxergar os buracos reais dos mapas.
    ///
    /// Os testes do adversario montam a percepcao a mao; este confere que a
    /// geometria de verdade produz a mesma coisa. Este e o vao da Arena 01 que
    /// o fazia perder sozinho.
    #[test]
    fn o_buraco_da_arena_01_aparece_para_quem_percebe() {
        let level = level_at(0);
        // Em pe no trecho da direita, pes em -170, centro em -140.
        let em_pe = Vec2::new(300.0, -140.0);
        assert!(
            level.ground_under(em_pe).is_some_and(|y| y == -170.0),
            "o chao onde o adversario nasce sumiu"
        );
        // Dentro do vao entre os trechos, na mesma altura.
        let sobre_o_vao = Vec2::new(190.0, -140.0);
        assert_eq!(
            level.ground_under(sobre_o_vao),
            None,
            "o buraco reportou chao, e o adversario andaria para dentro dele"
        );
        // A plataforma alta em x=150 nao pode contar: ela esta acima da
        // cabeca, nao debaixo dos pes.
        assert!(level.ground_under(Vec2::new(150.0, 200.0)).is_some());
    }

    /// Arma que nasce sobre o vazio cai direto no buraco e some do round.
    #[test]
    fn toda_arma_cai_sobre_algum_patamar() {
        for build in CATALOG {
            let level = build();
            let spots = footholds(level.as_ref());
            for drop in level.drop_points() {
                assert!(
                    spots
                        .iter()
                        .any(|s| drop.x >= s.0 && drop.x <= s.1 && s.2 < drop.y),
                    "{}: arma largada em {drop:?} nao tem chao embaixo",
                    level.name()
                );
            }
        }
    }

    /// Toda arena precisa caber uma sala cheia.
    ///
    /// Sem isto, uma fase com dois pontos so aceitaria a sala antiga: o
    /// terceiro e o quarto jogador nasceriam empilhados em `Vec2::ZERO`, que
    /// na Arena 02 e o vazio entre as torres.
    #[test]
    fn toda_arena_tem_lugar_para_a_sala_cheia() {
        for build in CATALOG {
            let level = build();
            let spawns = level.spawn_points();
            assert_eq!(
                spawns.len(),
                crate::actor::MAX_PLAYERS,
                "{}: a fase nao tem lugar para todos",
                level.name()
            );

            // Dois bonecos no mesmo ponto nascem presos um no outro.
            for (at, spawn) in spawns.iter().enumerate() {
                for other in &spawns[at + 1..] {
                    assert!(
                        spawn.distance(*other) > 60.0,
                        "{}: {spawn:?} e {other:?} nascem colados",
                        level.name()
                    );
                }
            }
        }
    }

    /// Jogador que nasce sobre o vazio morre antes de encostar no chao.
    #[test]
    fn todo_jogador_nasce_sobre_algum_patamar() {
        for build in CATALOG {
            let level = build();
            let spots = footholds(level.as_ref());
            for spawn in level.spawn_points() {
                assert!(
                    spots
                        .iter()
                        .any(|s| spawn.x >= s.0 && spawn.x <= s.1 && s.2 < spawn.y),
                    "{}: jogador nasce em {spawn:?} sem chao embaixo",
                    level.name()
                );
            }
        }
    }

    #[test]
    fn cada_tema_novo_tem_tres_mapas_e_spawn_seguro() {
        let mut themes = [0; 3];
        let mut scenes = Vec::new();
        for build in &CATALOG[3..] {
            let level = build();
            themes[match level.theme() {
                Theme::Volcano => 0,
                Theme::Industrial => 1,
                Theme::Oriental => 2,
                Theme::City => panic!("mapa novo sem tema"),
            }] += 1;
            assert!(
                !scenes.contains(&level.scene()),
                "{} reutiliza a pintura de outro mapa",
                level.name()
            );
            scenes.push(level.scene());

            // Poca, fonte, mare e goteira contam igual: o que a arena precisa
            // e de algo que cobre atencao, nao de um material especifico.
            let hazards: Vec<(f32, f32)> = level
                .pieces()
                .iter()
                .filter_map(|piece| piece.menace())
                .collect();
            assert!(!hazards.is_empty(), "{}: arena sem interacao", level.name());
            for spawn in level.spawn_points() {
                assert!(
                    hazards
                        .iter()
                        .all(|(x0, x1)| spawn.x < *x0 || spawn.x > *x1),
                    "{}: jogador nasce dentro do perigo em {spawn:?}",
                    level.name()
                );
            }
        }
        assert_eq!(themes, [3, 3, 3]);
        assert_eq!(scenes.len(), 9);
    }

    /// Toda fase do catalogo tem que conseguir nascer.
    ///
    /// Os outros testes leem os dados; este monta. Um `saturating_sub` que
    /// zera, um vetor de quadros vazio ou uma peca de largura zero nao
    /// aparecem em lugar nenhum ate virarem entidade -- e ai o jogo fecha
    /// sozinho na hora de entrar no round, que e o pior momento possivel.
    #[test]
    fn toda_fase_do_catalogo_monta_sem_quebrar() {
        for index in 0..CATALOG.len() {
            let level = level_at(index);
            let nome = level.name();
            let mut app = App::new();
            app.insert_resource(CurrentLevel(level)).add_systems(
                Startup,
                |mut commands: Commands, level: Res<CurrentLevel>| {
                    raise_level(&mut commands, &level)
                },
            );
            app.update();

            let world = app.world_mut();
            let pecas = world.query::<&LevelGeometry>().iter(world).count();
            assert!(pecas > 0, "{nome} nasceu vazia");
        }
    }

    /// Toda a arte de perigo tem que existir na pagina de codigo.
    ///
    /// A varredura geral de CP437 passa por poses, arsenal e nomes de fase --
    /// nao pela arte que a fase gera em runtime. Um glifo fora da pagina aqui
    /// vira interrogacao no meio da piscina, e o jogo roda assim mesmo.
    #[test]
    fn a_arte_dos_perigos_cabe_na_cp437() {
        use crate::ascii::cp437::glyph_index;
        let fallback = glyph_index('?') as u8;

        for kind in [
            HazardKind::Lava,
            HazardKind::Acid,
            HazardKind::Spikes,
            HazardKind::Jade,
        ] {
            for phase in 0..4 {
                for art in [hazard_art(kind, 9, phase), jet_art(kind, 5, 7, phase)] {
                    assert!(
                        art.cells.iter().all(|cell| cell.glyph != fallback),
                        "{kind:?}: glifo fora da pagina no quadro {phase}"
                    );
                }
            }
        }
    }

    /// A coluna que machuca tem que ser a coluna que aparece.
    ///
    /// A zona de contato cobre a altura cheia do jorro o tempo todo em que
    /// esta armada, entao ela so pode armar quando a coluna ja subiu. Armada
    /// junto com o desenho, ela cobraria dano no topo enquanto a coluna ainda
    /// mede um degrau -- e o jogador leva um golpe do ar.
    #[test]
    fn a_fonte_so_machuca_com_a_coluna_inteira_em_pe() {
        let beat = jet_beat(6.0, 0.0);
        let mut armada = 0;
        let mut visivel = 0;

        for step in 0..2000 {
            let t = step as f32 / 2000.0;
            let alta = jet_rise(t);
            if alta > 0.0 {
                visivel += 1;
            }
            if t < beat.from || t >= beat.to {
                continue;
            }
            armada += 1;
            assert!(
                (alta - 1.0).abs() < 1e-3,
                "a zona morde em t={t} com a coluna a {alta} da altura"
            );
        }
        assert!(armada > 0, "a fonte nunca arma");
        assert!(
            visivel > armada,
            "a coluna aparece e some junto com a zona: some o aviso de subida"
        );
    }

    /// A mare tem que percorrer exatamente o que promete, e voltar.
    #[test]
    fn a_mare_sobe_o_que_prometeu_e_desce_de_volta() {
        let bob = Bob {
            home: -162.0,
            rise: 6.0 * CELL.y,
            period: 10.0,
            phase: 0.0,
        };
        let at = |t: f32| {
            let wave = 0.5 - (cycle(t, bob.period, bob.phase) * std::f32::consts::TAU).cos() * 0.5;
            bob.home + bob.rise * wave
        };

        let alturas: Vec<f32> = (0..400)
            .map(|s| at(s as f32 * bob.period / 400.0))
            .collect();
        let baixa = alturas.iter().copied().fold(f32::INFINITY, f32::min);
        let cheia = alturas.iter().copied().fold(f32::NEG_INFINITY, f32::max);

        assert!(
            (baixa - bob.home).abs() < 1.0,
            "a mare baixa passa do fundo"
        );
        assert!(
            (cheia - (bob.home + bob.rise)).abs() < 1.0,
            "a mare cheia nao chega onde o mapa promete"
        );
        // Ciclo fechado: o fim do periodo tem que voltar ao comeco, senao a
        // poca anda para cima um pouco a cada volta.
        assert!((at(0.0) - at(bob.period)).abs() < 1.0, "a mare nao fecha");
    }

    /// Toda goteira tem que pingar sobre alguma coisa.
    ///
    /// A gota se desmancha na altura que a peca declara. Se essa altura nao
    /// for a de um apoio real, o respingo acontece no ar -- ou, pior, a gota
    /// atravessa o chao e some, e a boca vira enfeite que nunca acerta nada.
    #[test]
    fn toda_goteira_pinga_sobre_algum_chao() {
        for build in CATALOG {
            let level = build();
            for piece in level.pieces() {
                let Piece::Drip { from, floor, .. } = *piece else {
                    continue;
                };
                let chao = level.ground_under(from).unwrap_or_else(|| {
                    panic!("{}: goteira em {from:?} pinga no vazio", level.name())
                });
                assert!(
                    (floor - chao).abs() <= CELL.y,
                    "{}: a goteira em {from:?} se desmancha em {floor} e o chao esta em {chao}",
                    level.name()
                );
            }
        }
    }

    /// Fonte e mare nao podem nascer debaixo de um teto que as engole.
    ///
    /// Elas medem altura em celulas, longe de onde os patamares sao escritos.
    /// Uma coluna que passa do teto desenha por dentro da pedra, e a metade de
    /// cima do jorro -- justamente a que ameaca -- some.
    #[test]
    fn nenhuma_coluna_atravessa_o_teto() {
        for build in CATALOG {
            let level = build();
            let tetos: Vec<(f32, f32, f32)> = level
                .pieces()
                .iter()
                .filter_map(|piece| match *piece {
                    Piece::Ceiling { bottom, cols, .. } => {
                        let half = cols as f32 * CELL.x * 0.5;
                        Some((bottom.x - half, bottom.x + half, bottom.y))
                    }
                    _ => None,
                })
                .collect();

            for piece in level.pieces() {
                let Piece::Geyser { at, rows, .. } = *piece else {
                    continue;
                };
                let topo = at.y + rows as f32 * CELL.y;
                for (x0, x1, y) in &tetos {
                    if at.x < *x0 || at.x > *x1 {
                        continue;
                    }
                    assert!(
                        topo <= *y,
                        "{}: a fonte em {at:?} sobe ate {topo} e o teto esta em {y}",
                        level.name()
                    );
                }
            }
        }
    }

    #[test]
    fn lava_tem_profundidade_e_quatro_quadros_de_fluxo() {
        let frames: Vec<AsciiArt> = (0..4)
            .map(|phase| hazard_art(HazardKind::Lava, 12, phase))
            .collect();
        assert!(
            frames
                .iter()
                .all(|frame| frame.cols == 12 && frame.rows == 4)
        );
        for pair in frames.windows(2) {
            assert_ne!(pair[0], pair[1], "dois quadros da lava ficaram iguais");
        }
    }
}
