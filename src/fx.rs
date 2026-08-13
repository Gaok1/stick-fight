//! Camada de efeitos.
//!
//! Desenha acima de todo o ASCII (`Layer::Fx`) e existe justamente para nao
//! ficar preso a ele: hoje as particulas sao glifos, mas um efeito futuro pode
//! ser um `Sprite` comum, um shader ou uma malha, sem tocar em nada do resto.
//! O que os sistemas de jogo enxergam e o trait [`Effect`], nunca a
//! implementacao.

use bevy::prelude::*;
use bevy::time::{Real, Virtual};

use crate::ascii::{AsciiArt, AsciiSprite, Layer, palette};
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
            .add_systems(
                Update,
                (
                    update_hit_stop,
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
