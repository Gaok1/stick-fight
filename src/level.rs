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
}

/// Perigos reutilizados pelas arenas tematicas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HazardKind {
    Lava,
    Acid,
    Spikes,
}

impl HazardKind {
    fn damage(self) -> i32 {
        match self {
            Self::Lava => 34,
            Self::Acid => 18,
            Self::Spikes => 24,
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
            Piece::Chain { .. } | Piece::Ceiling { .. } | Piece::Hazard { .. } => None,
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

/// Faixa de cenario que fere ao toque.
#[derive(Component)]
struct Hazard {
    kind: HazardKind,
}

#[derive(Component)]
struct HazardVisual {
    kind: HazardKind,
    cols: u16,
    phase: u8,
    bubbles: Timer,
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
        HazardKind::Spikes => AsciiArt::fill('▲', cols, 1, palette::ASH).stamp(
            &AsciiArt::fill('▄', cols, 1, palette::IRON),
            0,
            1,
        ),
    }
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
        Hazard { kind },
        Transform::from_translation(at.extend(0.0)),
        Collider::size(cols as f32 * CELL.x, CELL.y),
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

fn hurt_on_hazards(
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
        let Some((hazard, _, _)) = hazards
            .iter()
            .find(|(_, at, area)| overlap(body, area.aabb(at.translation.truncate())))
        else {
            continue;
        };
        health.hp -= hazard.kind.damage();
        let knockback = match hazard.kind {
            HazardKind::Lava => Vec2::new(velocity.0.x * -0.35, 430.0),
            HazardKind::Acid => Vec2::new(velocity.0.x * -0.2, 210.0),
            HazardKind::Spikes => Vec2::new(velocity.0.x * -0.4, 300.0),
        };
        velocity.0 = knockback;
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

const LAVA_1: [Piece; 8] = [
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
const LAVA_2: [Piece; 9] = [
    Piece::Terrain {
        top: Vec2::new(0.0, -170.0),
        cols: 160,
        rows: 6,
    },
    Piece::Hazard {
        at: Vec2::new(-250.0, -162.0),
        cols: 18,
        kind: HazardKind::Lava,
    },
    Piece::Hazard {
        at: Vec2::new(250.0, -162.0),
        cols: 18,
        kind: HazardKind::Lava,
    },
    Piece::Platform {
        at: Vec2::new(-430.0, -100.0),
        cols: 14,
    },
    Piece::Platform {
        at: Vec2::new(-230.0, -30.0),
        cols: 13,
    },
    Piece::Platform {
        at: Vec2::new(0.0, -100.0),
        cols: 20,
    },
    Piece::Platform {
        at: Vec2::new(230.0, -30.0),
        cols: 13,
    },
    Piece::Platform {
        at: Vec2::new(430.0, -100.0),
        cols: 14,
    },
    Piece::Chain {
        top: Vec2::new(0.0, 175.0),
        links: 18,
    },
];
const LAVA_3: [Piece; 10] = [
    Piece::Terrain {
        top: Vec2::new(0.0, -170.0),
        cols: 160,
        rows: 6,
    },
    Piece::Hazard {
        at: Vec2::new(-330.0, -162.0),
        cols: 12,
        kind: HazardKind::Lava,
    },
    Piece::Hazard {
        at: Vec2::new(0.0, -162.0),
        cols: 12,
        kind: HazardKind::Lava,
    },
    Piece::Hazard {
        at: Vec2::new(330.0, -162.0),
        cols: 12,
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

const ACID_1: [Piece; 8] = [
    Piece::Terrain {
        top: Vec2::new(0.0, -170.0),
        cols: 160,
        rows: 6,
    },
    Piece::Hazard {
        at: Vec2::new(0.0, -162.0),
        cols: 28,
        kind: HazardKind::Acid,
    },
    Piece::Platform {
        at: Vec2::new(-360.0, -100.0),
        cols: 18,
    },
    Piece::Platform {
        at: Vec2::new(-160.0, -30.0),
        cols: 12,
    },
    Piece::Platform {
        at: Vec2::new(160.0, -30.0),
        cols: 12,
    },
    Piece::Platform {
        at: Vec2::new(360.0, -100.0),
        cols: 18,
    },
    Piece::Chain {
        top: Vec2::new(-40.0, 190.0),
        links: 18,
    },
    Piece::Chain {
        top: Vec2::new(40.0, 190.0),
        links: 18,
    },
];
const ACID_2: [Piece; 10] = [
    Piece::Terrain {
        top: Vec2::new(0.0, -170.0),
        cols: 160,
        rows: 6,
    },
    Piece::Hazard {
        at: Vec2::new(-300.0, -162.0),
        cols: 16,
        kind: HazardKind::Acid,
    },
    Piece::Hazard {
        at: Vec2::new(300.0, -162.0),
        cols: 16,
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
];
const ACID_3: [Piece; 9] = [
    Piece::Terrain {
        top: Vec2::new(0.0, -170.0),
        cols: 160,
        rows: 6,
    },
    Piece::Hazard {
        at: Vec2::new(-210.0, -162.0),
        cols: 12,
        kind: HazardKind::Acid,
    },
    Piece::Hazard {
        at: Vec2::new(0.0, -162.0),
        cols: 12,
        kind: HazardKind::Acid,
    },
    Piece::Hazard {
        at: Vec2::new(210.0, -162.0),
        cols: 12,
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
    Piece::Chain {
        top: Vec2::new(0.0, 190.0),
        links: 24,
    },
];

const EAST_1: [Piece; 8] = [
    Piece::Terrain {
        top: Vec2::new(0.0, -170.0),
        cols: 160,
        rows: 6,
    },
    Piece::Hazard {
        at: Vec2::new(0.0, -162.0),
        cols: 10,
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
const EAST_2: [Piece; 10] = [
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
const EAST_3: [Piece; 9] = [
    Piece::Terrain {
        top: Vec2::new(0.0, -170.0),
        cols: 160,
        rows: 6,
    },
    Piece::Hazard {
        at: Vec2::new(-190.0, -162.0),
        cols: 8,
        kind: HazardKind::Spikes,
    },
    Piece::Hazard {
        at: Vec2::new(190.0, -162.0),
        cols: 8,
        kind: HazardKind::Spikes,
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
    &SPAWNS_INNER,
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
    &SPAWNS_WIDE,
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
    &SPAWNS_INNER,
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

fn apply_level_pick(mut commands: Commands, pick: Res<LevelPick>) {
    if pick.is_changed() {
        commands.insert_resource(CurrentLevel(level_at(pick.0)));
    }
}

/// Monta a fase ao entrar em `Fighting`.
///
/// Um unico lugar traduz peca em entidade. As fases nao spawnam nada por conta
/// propria, entao nao existe mapa com regra de colisao ou de camada diferente
/// dos outros.
fn build_level(mut commands: Commands, level: Res<CurrentLevel>) {
    crate::backdrop::build(
        &mut commands,
        level.0.skyline(),
        level.0.signs(),
        level.0.scene(),
    );

    let mut next_chain = 0u8;
    for piece in level.0.pieces() {
        match *piece {
            Piece::Terrain { top, cols, rows } => {
                let height = rows as f32 * CELL.y;
                terrain(
                    &mut commands,
                    Vec2::new(top.x, top.y - height * 0.5),
                    cols,
                    rows,
                );
            }
            Piece::Ceiling { bottom, cols, rows } => {
                let height = rows as f32 * CELL.y;
                terrain(
                    &mut commands,
                    Vec2::new(bottom.x, bottom.y + height * 0.5),
                    cols,
                    rows,
                );
            }
            Piece::Platform { at, cols } => platform(&mut commands, at, cols),
            Piece::Chain { top, links } => {
                chain(&mut commands, next_chain, top, links);
                next_chain += 1;
            }
            Piece::Hazard { at, cols, kind } => hazard(&mut commands, at, cols, kind),
        }
    }
}

/// Limpa a geometria ao sair de `Fighting`.
fn clear_level(mut commands: Commands, q: Query<Entity, With<LevelGeometry>>) {
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
            .add_systems(Update, apply_level_pick.in_set(AppSet::Logic))
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
                (hurt_on_hazards, tick_hazard_cooldowns)
                    .chain()
                    .in_set(AppSet::Logic)
                    .run_if(arena_live),
            )
            .add_systems(
                Update,
                animate_hazards.in_set(AppSet::Animate).run_if(arena_live),
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

            let hazards: Vec<(f32, f32)> = level
                .pieces()
                .iter()
                .filter_map(|piece| match *piece {
                    Piece::Hazard { at, cols, .. } => Some((at.x, cols as f32 * CELL.x * 0.5)),
                    _ => None,
                })
                .collect();
            assert!(!hazards.is_empty(), "{}: arena sem interacao", level.name());
            for spawn in level.spawn_points() {
                assert!(
                    hazards.iter().all(|(x, half)| (spawn.x - x).abs() > *half),
                    "{}: jogador nasce dentro do perigo em {spawn:?}",
                    level.name()
                );
            }
        }
        assert_eq!(themes, [3, 3, 3]);
        assert_eq!(scenes.len(), 9);
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
