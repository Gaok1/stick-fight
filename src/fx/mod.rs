//! Camada de efeitos.
//!
//! Desenha acima de todo o ASCII (`Layer::Fx`) e existe justamente para nao
//! ficar preso a ele: hoje as particulas sao glifos, mas um efeito futuro pode
//! ser um `Sprite` comum, um shader ou uma malha, sem tocar em nada do resto.
//! O que os sistemas de jogo enxergam e o trait [`Effect`], nunca a
//! implementacao.

use bevy::prelude::*;
use bevy::time::{Real, Virtual};

use crate::ascii::{AsciiArt, AsciiSprite, CELL, Layer, palette};
use crate::combat::{Damaged, Lifetime, Parried};
use crate::physics::{Falls, Ghost, Velocity};
use crate::state::{AppSet, GameState};

/// Algo que pode ser emitido num ponto do mundo.
pub trait Effect: Send + Sync + 'static {
    /// Cria o efeito em `at`, jogado na direcao `dir`.
    fn emit(&self, commands: &mut Commands, at: Vec2, dir: Vec2);
}

/// Estilhaços que saem do ponto de impacto.
pub struct SparkBurst {
    /// Quantas particulas.
    pub count: usize,
    /// Glifo de cada uma.
    pub glyph: char,
    /// Cor.
    pub color: Color,
    /// Velocidade base.
    pub speed: f32,
    /// Segundos de vida.
    pub life: f32,
}

impl Effect for SparkBurst {
    fn emit(&self, commands: &mut Commands, at: Vec2, dir: Vec2) {
        let base = if dir == Vec2::ZERO { Vec2::Y } else { dir };

        for _ in 0..self.count {
            // Espalha em torno da direcao do golpe, com sobra pra cima.
            let angle = base.to_angle() + (fastrand::f32() - 0.5) * 1.7;
            let speed = self.speed * (0.45 + fastrand::f32());
            let velocity = Vec2::from_angle(angle) * speed + Vec2::new(0.0, 90.0);

            commands.spawn((
                AsciiSprite::new(AsciiArt::solid(&self.glyph.to_string(), self.color)),
                Layer::Fx,
                Transform::from_translation(at.extend(0.0)),
                Velocity(velocity),
                Ghost,
                Falls,
                Lifetime(Timer::from_seconds(
                    self.life * (0.6 + fastrand::f32() * 0.6),
                    TimerMode::Once,
                )),
                DespawnOnExit(GameState::Fighting),
            ));
        }
    }
}

/// Clarao da boca do cano.
///
/// A forma e o parametro, e nao so o tamanho. A escopeta abre um leque curto e
/// largo, o rifle cospe uma lanca fina e comprida, a pistola solta um estalo:
/// se os tres fossem o mesmo sopro em escalas diferentes, o disparo leria como
/// a mesma arma vista de mais longe.
///
/// Cada lingua e um bloco deitado que o `Transform` estica, e nao uma string
/// desenhada por comprimento. String por comprimento vira uma arte por arma, e
/// duas artes de fogo escritas em dias diferentes nunca ficam do mesmo
/// material.
///
/// O bloco e o cheio, e nao um traco: so o cheio ocupa a celula inteira, entao
/// `width` vira espessura em pixels direto. Com um glifo de tinta parcial a
/// mesma escala daria linguas quase invisiveis, e o ajuste seria adivinhar
/// quanto de tinta cada glifo tem.
pub struct MuzzleFlash {
    /// Quantas linguas abrem no leque. Poucas e finas leem como lanca.
    pub tongues: usize,
    /// Abertura total do leque, em radianos.
    pub spread: f32,
    /// Comprimento da lingua central, em unidades de mundo.
    pub reach: f32,
    /// Espessura de cada lingua.
    pub width: f32,
    /// Cor do miolo do leque.
    pub core: Color,
    /// Cor das linguas de fora.
    pub edge: Color,
    /// Segundos de vida da lingua central.
    pub life: f32,
}

impl Effect for MuzzleFlash {
    fn emit(&self, commands: &mut Commands, at: Vec2, dir: Vec2) {
        let dir = dir.normalize_or(Vec2::X);

        for i in 0..self.tongues.max(1) {
            // De -0.5 a 0.5, com o zero no meio: leque de numero par sai sem
            // lingua central, e um sopro com buraco no meio le como duas armas
            // disparando lado a lado.
            let side = if self.tongues > 1 {
                i as f32 / (self.tongues - 1) as f32 - 0.5
            } else {
                0.0
            };
            // As pontas do leque sao mais curtas que o meio. Todas do mesmo
            // tamanho desenham um arco de roda dentada, nao fogo.
            let reach = self.reach * (1.0 - side.abs() * 0.85);
            let ray = Vec2::from_angle(side * self.spread).rotate(dir);
            commands.spawn((
                AsciiSprite::new(AsciiArt::glyph(
                    '\u{2588}',
                    if side.abs() < 0.2 {
                        self.core
                    } else {
                        self.edge
                    },
                )),
                Layer::Fx,
                Transform::from_translation((at + ray * reach * 0.5).extend(0.0))
                    .with_rotation(Quat::from_rotation_z(ray.to_angle()))
                    .with_scale(Vec3::new(reach / CELL.x, self.width / CELL.y, 1.0)),
                Lifetime(Timer::from_seconds(
                    self.life * (1.0 - side.abs() * 0.6),
                    TimerMode::Once,
                )),
                DespawnOnExit(GameState::Fighting),
            ));
        }

        // O estalo branco na propria boca. E a unica parte igual em todas as
        // armas, de proposito: o que separa uma da outra e o leque em volta
        // dele, e ter duas coisas variando de uma vez nao separaria nada.
        commands.spawn((
            AsciiSprite::new(AsciiArt::glyph('\u{2666}', palette::BONE)),
            Layer::Fx,
            Transform::from_translation(at.extend(0.02))
                .with_scale(Vec3::splat(0.3 + self.width / 30.0)),
            Lifetime(Timer::from_seconds(self.life * 0.5, TimerMode::Once)),
            DespawnOnExit(GameState::Fighting),
        ));
    }
}

/// Capsula ejetada: sai da culatra, cai e some.
///
/// Sao os mesmos quatro componentes do sangue -- `Velocity`, `Falls`, `Ghost` e
/// `Lifetime` -- e ela existe porque e o unico pedaco do disparo que continua
/// na tela depois de o clarao apagar.
pub struct Casing {
    /// Glifo da capsula.
    pub glyph: char,
    /// Cor do latao.
    pub color: Color,
    /// Forca da ejecao.
    pub speed: f32,
    /// Segundos ate sumir.
    pub life: f32,
}

impl Effect for Casing {
    fn emit(&self, commands: &mut Commands, at: Vec2, dir: Vec2) {
        let dir = dir.normalize_or(Vec2::X);
        // Para cima e para tras do cano, e nao para um lado fixo do mundo: a
        // janela de ejecao e da arma, entao ela vira junto quando o boneco
        // vira. Amarrada ao mundo, atirar para a esquerda cuspiria a capsula
        // para dentro do proprio corpo.
        let toss = (-dir * 0.5 + Vec2::Y * 0.95) * self.speed
            + Vec2::new(fastrand::f32() * 44.0 - 22.0, fastrand::f32() * 34.0);
        commands.spawn((
            AsciiSprite::new(AsciiArt::glyph(self.glyph, self.color)),
            Layer::Fx,
            Transform::from_translation((at - dir * 9.0).extend(0.0))
                .with_rotation(Quat::from_rotation_z(dir.to_angle() + 1.1))
                .with_scale(Vec3::splat(0.5)),
            Velocity(toss),
            Ghost,
            Falls,
            Lifetime(Timer::from_seconds(self.life, TimerMode::Once)),
            DespawnOnExit(GameState::Fighting),
        ));
    }
}

/// Fumaca que sobe da boca e dissolve.
///
/// Sem `Falls`: fumaca que cai e poeira. O que faz a nuvem crescer em vez de
/// viajar inteira e cada bafo sair com um pouco menos de empurrao para a
/// frente que o anterior.
pub struct Smoke {
    /// Quantos bafos.
    pub puffs: usize,
    /// Quanto ela sobe, em unidades por segundo.
    pub rise: f32,
    /// Quanto o primeiro bafo ainda viaja no sentido do tiro.
    pub drift: f32,
    /// Segundos de vida do primeiro bafo.
    pub life: f32,
    /// Cor da nuvem.
    pub color: Color,
}

impl Effect for Smoke {
    fn emit(&self, commands: &mut Commands, at: Vec2, dir: Vec2) {
        let dir = dir.normalize_or(Vec2::X);
        for i in 0..self.puffs {
            let along = i as f32 / self.puffs.max(1) as f32;
            commands.spawn((
                AsciiSprite::new(AsciiArt::glyph(
                    if i % 3 == 0 { '\u{2592}' } else { '\u{2591}' },
                    self.color.with_alpha(0.5),
                )),
                Layer::Fx,
                Transform::from_translation((at + dir * along * 9.0).extend(0.0))
                    .with_scale(Vec3::splat(0.35 + along * 0.55)),
                Velocity(
                    dir * self.drift * (1.0 - along)
                        + Vec2::Y * self.rise
                        + Vec2::new(fastrand::f32() * 28.0 - 14.0, 0.0),
                ),
                Ghost,
                Lifetime(Timer::from_seconds(
                    self.life * (0.6 + along * 0.9),
                    TimerMode::Once,
                )),
                DespawnOnExit(GameState::Fighting),
            ));
        }
    }
}

/// Efeito disparado quando alguem toma dano.
///
/// Fica num recurso para poder ser trocado em runtime -- por um modo sem
/// sangue, por um efeito mais barato em maquina fraca, ou por nada.
#[derive(Resource)]
pub struct HitEffect(pub Box<dyn Effect>);

/// Pedido de sacudida de camera, em [0, 1].
///
/// E mensagem e nao chamada direta para que qualquer sistema (coice de arma,
/// explosao, queda) possa pedir tremor sem conhecer a camera nem este modulo.
#[derive(Message, Debug, Clone, Copy)]
pub struct Shake(pub f32);

#[derive(Resource, Default)]
struct CameraTrauma(f32);

#[derive(Resource, Default)]
struct HitStop(f32);

#[derive(Component)]
struct ImpactWave {
    age: f32,
    frame: u8,
}

impl Default for HitEffect {
    fn default() -> Self {
        Self(Box::new(SparkBurst {
            count: 12,
            glyph: '\u{00B7}',
            color: palette::BLOOD,
            speed: 330.0,
            life: 0.55,
        }))
    }
}

/// Converte mensagens de dano em efeito visual.
fn spawn_hit_effects(
    mut commands: Commands,
    mut damaged: MessageReader<Damaged>,
    effect: Res<HitEffect>,
    mut trauma: ResMut<CameraTrauma>,
    mut stop: ResMut<HitStop>,
    mut virtual_time: ResMut<Time<Virtual>>,
) {
    for hit in damaged.read() {
        effect.0.emit(&mut commands, hit.at, hit.dir);
        commands.spawn((
            ImpactWave { age: 0.0, frame: 0 },
            AsciiSprite::new(AsciiArt::solid("<*>", palette::BONE)),
            Layer::Fx,
            Transform::from_translation(hit.at.extend(0.0)).with_scale(Vec3::splat(0.65)),
            DespawnOnExit(GameState::Fighting),
        ));
        // Golpe caro sacode e congela mais: e o que separa o jab da paulada sem
        // precisar de efeito proprio para cada golpe.
        trauma.0 = (trauma.0 + 0.30 + hit.amount as f32 * 0.026).min(1.0);
        stop.0 = stop.0.max(0.05 + hit.amount as f32 * 0.0035);
        virtual_time.pause();
    }
}

fn spawn_parry_effects(
    mut commands: Commands,
    mut parried: MessageReader<Parried>,
    mut trauma: ResMut<CameraTrauma>,
    mut stop: ResMut<HitStop>,
    mut virtual_time: ResMut<Time<Virtual>>,
) {
    for parry in parried.read() {
        let _ = (parry.defender, parry.attacker);
        SparkBurst {
            count: 13,
            glyph: '*',
            color: palette::GOLD,
            speed: 390.0,
            life: 0.42,
        }
        .emit(&mut commands, parry.at, Vec2::Y);
        commands.spawn((
            AsciiSprite::new(AsciiArt::solid("<< PARRY! >>", palette::GOLD)),
            Layer::Fx,
            Transform::from_translation((parry.at + Vec2::Y * 34.0).extend(0.0))
                .with_scale(Vec3::splat(0.8)),
            Lifetime(Timer::from_seconds(0.34, TimerMode::Once)),
            DespawnOnExit(GameState::Fighting),
        ));
        trauma.0 = 1.0;
        stop.0 = stop.0.max(0.10);
        virtual_time.pause();
    }
}

/// O impacto continua expandindo durante o hit-stop, reforcando o quadro de
/// contato em vez de congelar todo o feedback junto com o gameplay.
fn animate_impact_waves(
    real: Res<Time<Real>>,
    mut commands: Commands,
    mut waves: Query<(Entity, &mut ImpactWave, &mut AsciiSprite, &mut Transform)>,
) {
    for (entity, mut wave, mut sprite, mut transform) in &mut waves {
        wave.age += real.delta_secs();
        if wave.age >= 0.24 {
            commands.entity(entity).despawn();
            continue;
        }
        let frame = (wave.age / 0.06) as u8;
        if frame != wave.frame {
            wave.frame = frame;
            let (art, color) = match frame {
                0 => ("<*>", palette::BONE),
                1 => ("((+))", palette::GOLD),
                2 => ("<     >", palette::ASH),
                _ => (".     .", palette::IRON),
            };
            sprite.art = AsciiArt::solid(art, color);
        }
        let scale = 0.65 + wave.age * 3.2;
        transform.scale = Vec3::splat(scale);
    }
}

fn update_hit_stop(
    real: Res<Time<Real>>,
    mut stop: ResMut<HitStop>,
    mut virtual_time: ResMut<Time<Virtual>>,
) {
    if stop.0 <= 0.0 {
        return;
    }
    stop.0 -= real.delta_secs();
    if stop.0 <= 0.0 {
        virtual_time.unpause();
    }
}

fn apply_shake_requests(mut requests: MessageReader<Shake>, mut trauma: ResMut<CameraTrauma>) {
    for request in requests.read() {
        trauma.0 = (trauma.0 + request.0).clamp(0.0, 1.0);
    }
}

fn shake_camera(
    real: Res<Time<Real>>,
    mut trauma: ResMut<CameraTrauma>,
    mut cameras: Query<&mut Transform, With<Camera2d>>,
) {
    let Ok(mut transform) = cameras.single_mut() else {
        return;
    };
    trauma.0 = (trauma.0 - real.delta_secs() * 2.8).max(0.0);
    let strength = trauma.0 * trauma.0;
    let t = real.elapsed_secs();
    transform.translation.x = (t * 91.0).sin() * 13.0 * strength;
    transform.translation.y = (t * 73.0 + 1.4).sin() * 9.0 * strength;
    transform.rotation = Quat::from_rotation_z((t * 61.0).sin() * 0.012 * strength);
}

/// Escurece a particula conforme ela envelhece.
///
/// Faz o estilhaço apagar em vez de sumir de repente, que e o que denuncia
/// particula barata.
fn fade_particles(mut q: Query<(&Lifetime, &mut AsciiSprite), Without<crate::actor::Player>>) {
    for (life, mut sprite) in &mut q {
        let remaining = life.0.fraction_remaining();
        if remaining > 0.6 {
            continue;
        }
        let faded = sprite
            .art
            .cells
            .iter()
            .map(|c| c.color.with_alpha(remaining / 0.6))
            .next();
        if let Some(color) = faded {
            let art = sprite.art.recolored(color);
            if sprite.art != art {
                sprite.art = art;
            }
        }
    }
}

/// Efeitos acima do ASCII.
pub struct FxPlugin;

impl Plugin for FxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HitEffect>()
            .init_resource::<CameraTrauma>()
            .init_resource::<HitStop>()
            .add_message::<Shake>()
            // Este roda sempre, e nao so com a arena de pe: o hit-stop pausa o
            // relogio virtual, e quem despausa e so ele. Sair da luta durante
            // os ~100 ms de congelamento -- apertar ESC, ou o dono anunciar o
            // fim do round -- deixava o `Time<Virtual>` parado sem ninguem para
            // solta-lo, e o jogo inteiro travava ate alguem voltar para uma
            // arena. Com `stop` zerado a funcao sai na primeira linha, entao
            // rodar fora da luta nao custa nada.
            .add_systems(
                Update,
                update_hit_stop
                    .in_set(AppSet::Animate)
                    .before(spawn_hit_effects),
            )
            .add_systems(
                Update,
                (
                    spawn_hit_effects,
                    spawn_parry_effects,
                    apply_shake_requests,
                    animate_impact_waves,
                    fade_particles,
                    shake_camera,
                )
                    .chain()
                    .in_set(AppSet::Animate)
                    .run_if(crate::state::arena_live),
            );
    }
}
