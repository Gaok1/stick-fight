//! Movimento, escalada e escolha de pose.

use bevy::prelude::*;

use super::rig::{self, Joints, Rigging, Swaying};
use super::{
    ActorLimb, ActorSkin, ActorTint, ActorVisual, ActorVisualMotion, Attacking, BreathPhase,
    Climbing, Downed, Facing, Flash, Health, Intent, JumpAssist, LimbKind, LimbMotion, Player,
    Pose, Severed, StepDust, Stunned,
};
use crate::ascii::{AsciiArt, AsciiSprite, Layer, palette};
use crate::combat::{Lifetime, MeleeAttack, Parrying};
use crate::level::{ChainParticle, Climbable};
use crate::physics::{
    Collider, DropThrough, Ghost, Grounded, KILL_Y, Velocity, Weightless, overlap,
};
use crate::state::{AppSet, GameState};
use crate::weapon::{Held, Recoiling};

#[derive(Component)]
struct ChainRelease(Timer);

/// Velocidade maxima de corrida.
pub const RUN_SPEED: f32 = 265.0;
/// Aceleracao com os pes no chao.
pub const GROUND_ACCEL: f32 = 2600.0;
/// Aceleracao no ar -- menor, para o pulo ter compromisso.
pub const AIR_ACCEL: f32 = 1300.0;
/// Desaceleracao ao soltar o controle no chao.
pub const FRICTION: f32 = 2400.0;
/// Impulso vertical do pulo.
pub const JUMP_SPEED: f32 = 510.0;
/// Velocidade de subida/descida na corrente.
pub const CLIMB_SPEED: f32 = 175.0;
/// Acima disso o boneco conta como correndo, para fins de animacao.
const RUN_ANIM_THRESHOLD: f32 = 25.0;

/// Aplica `Intent` a velocidade: andar, pular, atravessar plataforma.
fn drive(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<
        (
            Entity,
            &Intent,
            &Pose,
            &Climbing,
            &Grounded,
            &mut Velocity,
            &mut Facing,
            &mut JumpAssist,
            Option<&DropThrough>,
        ),
        With<Player>,
    >,
) {
    let dt = time.delta_secs();

    for (entity, intent, pose, climbing, grounded, mut vel, mut facing, mut jump, dropping) in
        &mut q
    {
        if pose.locks_control() {
            continue;
        }

        // --- horizontal ---
        let target = intent.move_x * RUN_SPEED;
        let accel = if grounded.0 { GROUND_ACCEL } else { AIR_ACCEL };
        if intent.move_x.abs() > f32::EPSILON {
            vel.x = move_towards(vel.x, target, accel * dt);
            if facing.0 != intent.move_x.signum() {
                facing.0 = intent.move_x.signum();
            }
        } else if grounded.0 {
            vel.x = move_towards(vel.x, 0.0, FRICTION * dt);
        }

        // --- vertical ---
        if grounded.0 {
            jump.coyote = 0.11;
        } else {
            jump.coyote = (jump.coyote - dt).max(0.0);
        }
        if intent.jump {
            jump.buffer = 0.12;
        } else {
            jump.buffer = (jump.buffer - dt).max(0.0);
        }

        if climbing.0 {
            continue; // escalada manda na velocidade vertical
        }
        if jump.buffer > 0.0 && jump.coyote > 0.0 {
            vel.y = JUMP_SPEED;
            jump.buffer = 0.0;
            jump.coyote = 0.0;
        } else if !intent.up && vel.y > 0.0 {
            // Soltar cedo encurta o salto; segurar preserva o arco cheio.
            vel.y = (vel.y - 1150.0 * dt).max(0.0);
        }
        if intent.down && !grounded.0 {
            vel.y -= 900.0 * dt;
        }
        // Segurar baixo no chao atravessa plataformas one-way.
        if intent.down && grounded.0 && dropping.is_none() {
            commands
                .entity(entity)
                .insert(DropThrough(Timer::from_seconds(0.25, TimerMode::Once)));
        }
    }
}

/// Agarra correntes e move o boneco por elas.
///
/// A corrente nao e solida: ela so define uma regiao onde a gravidade e
/// suspensa e o eixo Y passa a ser controlado direto.
fn climb(
    time: Res<Time>,
    mut commands: Commands,
    mut players: Query<
        (
            Entity,
            &Intent,
            &Transform,
            &Collider,
            &mut Velocity,
            &mut Climbing,
            &Pose,
            &Facing,
            Option<&ChainRelease>,
        ),
        With<Player>,
    >,
    mut chains: Query<(&Transform, &Collider, &mut ChainParticle), With<Climbable>>,
) {
    for (entity, intent, transform, collider, mut vel, mut climbing, pose, facing, released) in
        &mut players
    {
        let body = collider.aabb(transform.translation.truncate());
        let mut attached_x = None;
        let mut chain_velocity = 0.0;
        for (chain_transform, chain_collider, mut particle) in &mut chains {
            if overlap(
                body,
                chain_collider.aabb(chain_transform.translation.truncate()),
            ) {
                attached_x = Some(chain_transform.translation.x);
                chain_velocity = (chain_transform.translation.x - particle.previous.x)
                    / time.delta_secs().max(1.0 / 240.0);
                if climbing.0 {
                    // Empurrar para os lados transfere impulso ao elo; o resto
                    // da corrente recebe o movimento pelas restricoes Verlet.
                    particle.previous.x -= intent.move_x * 1.7;
                }
                break;
            }
        }

        // Solta a corrente ao sair dela, morrer ou apanhar.
        if attached_x.is_none() || pose.locks_control() {
            if climbing.0 {
                climbing.0 = false;
                commands.entity(entity).remove::<Weightless>();
            }
            continue;
        }

        // A mesma tecla agarra e solta; enquanto preso, A/D transfere impulso
        // para a corrente e permite balancar.
        let was_climbing = climbing.0;
        if !climbing.0 && released.is_none() && intent.grapple {
            climbing.0 = true;
            commands.entity(entity).insert(Weightless);
        }

        if climbing.0 {
            if was_climbing && intent.grapple {
                climbing.0 = false;
                commands
                    .entity(entity)
                    .remove::<Weightless>()
                    .insert(ChainRelease(Timer::from_seconds(0.22, TimerMode::Once)));
                vel.x = chain_velocity * 0.65
                    + if intent.move_x.abs() > f32::EPSILON {
                        intent.move_x * RUN_SPEED * 0.85
                    } else {
                        facing.0 * RUN_SPEED * 0.65
                    };
                vel.y = JUMP_SPEED * 0.78;
                continue;
            }
            let dir = intent.up as i32 as f32 - intent.down as i32 as f32;
            vel.y = dir * CLIMB_SPEED;
            let spring = (attached_x.unwrap() - transform.translation.x) * 14.0;
            vel.x = spring + intent.move_x * 70.0;
        }
    }
}

/// Traduz estado fisico em pose. Unico lugar que escreve `Pose`.
fn choose_pose(
    time: Res<Time>,
    mut q: Query<(
        &mut Pose,
        &Player,
        &Intent,
        &Health,
        &Grounded,
        &Climbing,
        &Velocity,
        &Transform,
        Option<&Stunned>,
        Option<&Attacking>,
        Option<&MeleeAttack>,
        Option<&Parrying>,
        Option<&Held>,
        Has<Downed>,
    )>,
) {
    for (
        mut pose,
        player,
        intent,
        health,
        grounded,
        climbing,
        vel,
        transform,
        stunned,
        attacking,
        melee,
        parrying,
        held,
        downed,
    ) in &mut q
    {
        let next = if health.is_dead() {
            Pose::Dead
        } else if stunned.is_some() {
            if downed { Pose::Downed } else { Pose::Hit }
        } else if parrying.is_some() {
            Pose::Parry
        } else if let Some(attack) = attacking {
            if melee.is_none() && held.is_some() {
                Pose::Shoot
            } else {
                let phase = attack.0.fraction();
                match phase {
                    t if t < 0.26 => Pose::PunchWindup,
                    t if t < 0.64 => Pose::PunchStrike,
                    _ => Pose::PunchRecover,
                }
            }
        } else if climbing.0 {
            // Quadro amarrado a altura, como o da corrida e a distancia: a
            // bracada acompanha a subida sem timer nenhum.
            let frame = (transform.translation.y / 16.0).floor() as i32;
            Pose::climbing(frame.rem_euclid(rig::CLIMB.len() as i32) as usize)
        } else if !grounded.0 {
            if vel.y > 0.0 { Pose::Jump } else { Pose::Fall }
        } else if intent.down {
            Pose::Crouch
        } else if vel.x.abs() > RUN_ANIM_THRESHOLD {
            // Quadro amarrado a distancia percorrida, nao a um timer: a
            // cadencia da passada acompanha a velocidade de graca.
            let frame = (transform.translation.x / 9.0).floor() as i32;
            Pose::running(frame.rem_euclid(rig::RUN.len() as i32) as usize)
        } else {
            let beat = (time.elapsed_secs() * 2.2 + player.id as f32 * 0.5) as i32;
            Pose::idling(beat.rem_euclid(rig::IDLE.len() as i32) as usize)
        };

        if *pose != next {
            *pose = next;
        }
    }
}

/// Da peso ao sprite sem contaminar a fisica: inclinacao, respiracao e
/// squash/stretch vivem apenas na escala/rotacao visual do pai.
fn animate_body(
    time: Res<Time>,
    actors: Query<(
        &BreathPhase,
        &Pose,
        &Facing,
        // O dummy nao tem `Velocity` de proposito -- e imune a knockback --
        // entao pedir uma o deixaria de fora da animacao inteira.
        Option<&Velocity>,
        Option<&Grounded>,
        &Children,
        Option<&MeleeAttack>,
    )>,
    mut visuals: Query<(&mut Transform, &mut ActorVisualMotion), With<ActorVisual>>,
) {
    let dt = time.delta_secs().min(0.05);
    for (phase, pose, facing, velocity, grounded, children, melee) in &actors {
        // O gancho estica pra cima, a rasteira afunda: e o corpo que conta o
        // que o golpe faz. O `rise` vem do golpe em curso pelo tipo dele, e nao
        // so pelo elo do combo -- enquanto vinha so pelo elo, a rasteira era
        // animada com o `rise` de um soco e o corpo dela nao abaixava.
        let rise = melee.map_or(1.0, |m| {
            super::pose::strike_for(m.step, m.kind, m.style).rise
        });
        let (mut scale, angle) = rig::def(*pose).body.resolve(&Swaying {
            pulse: (time.elapsed_secs() * 5.0 + phase.0).sin(),
            facing: facing.0,
            speed: velocity.map_or(0.0, |v| v.x) / RUN_SPEED,
            rise,
        });
        let airborne = grounded.map_or(matches!(*pose, Pose::Jump | Pose::Fall), |g| !g.0);
        for child in children.iter() {
            if let Ok((mut transform, mut motion)) = visuals.get_mut(child) {
                if motion.was_airborne && !airborne {
                    motion.landing = 0.12;
                }
                motion.was_airborne = airborne;
                if motion.landing > 0.0 {
                    let impact = motion.landing / 0.12;
                    scale.x *= 1.0 + impact * 0.10;
                    scale.y *= 1.0 - impact * 0.12;
                    motion.landing = (motion.landing - dt).max(0.0);
                }

                let target_scale = scale.extend(1.0);
                let target_rotation = Quat::from_rotation_z(angle);
                if !motion.initialized {
                    transform.scale = target_scale;
                    transform.rotation = target_rotation;
                    motion.initialized = true;
                } else {
                    let blend = 1.0 - (-dt * 20.0).exp();
                    transform.scale = transform.scale.lerp(target_scale, blend);
                    transform.rotation = transform.rotation.slerp(target_rotation, blend);
                }
            }
        }
    }
}

/// Poe um glifo esticado exatamente entre duas juntas.
///
/// O eixo longo do sprite e o Y local, entao alinha-lo com o segmento pede
/// `delta.to_angle() - PI/2`. O sinal importa: com o angulo negado o desenho
/// sai espelhado no eixo vertical, e como a translacao usa o ponto medio -- que
/// continua certo -- so as pontas erram. Os membros deixam de se encontrar e o
/// boneco vira um punhado de riscos soltos, sem nada quebrar.
fn segment_target(a: Vec2, b: Vec2, z: f32) -> Transform {
    let delta = b - a;
    Transform {
        translation: ((a + b) * 0.5).extend(z),
        rotation: Quat::from_rotation_z(delta.to_angle() - std::f32::consts::FRAC_PI_2),
        scale: Vec3::new(0.72, delta.length() / 16.0, 1.0),
    }
}

/// Oito sticks de verdade: braco/antebraco e coxa/canela, dos dois lados.
///
/// Este sistema nao sabe o que e agachar, escalar ou cair. Ele monta a passada
/// padrao, entrega a pose ao ajuste que a tabela do rig indica e desenha o
/// resultado. Quadro novo e uma linha naquela tabela, e nao mais um braco do
/// `match` que morava aqui.
fn animate_limbs(
    time: Res<Time>,
    actors: Query<
        (
            &Pose,
            &Facing,
            &Intent,
            &Transform,
            Option<&Velocity>,
            Option<&MeleeAttack>,
            Option<&Held>,
            Option<&Recoiling>,
        ),
        Without<ActorLimb>,
    >,
    // Membro arrancado nao volta pro lugar: ele ficou congelado onde estava e
    // o pedaco que voa e outra entidade.
    mut limbs: Query<(&ActorLimb, &mut LimbMotion, &mut Transform), Without<Severed>>,
) {
    let dt = time.delta_secs().min(0.05);
    for (limb, mut motion, mut transform) in &mut limbs {
        let Ok((pose, facing, intent, root, velocity, melee, held, recoiling)) =
            actors.get(limb.owner)
        else {
            continue;
        };
        let def = rig::def(*pose);
        let gait = pose.run_frame().map_or(0.0, |_| {
            -(root.translation.x / (9.0 * rig::RUN.len() as f32) * std::f32::consts::TAU).cos()
        });
        let (aim, recoil) = crate::weapon::animated_aim(intent, facing, recoiling);
        let rigging = Rigging {
            side: limb.side,
            facing: facing.0,
            gait,
            aim,
            strike: pose.melee_phase().map(|phase| {
                let choreo = melee.map_or(super::pose::strike(0), |m| {
                    super::pose::strike_for(m.step, m.kind, m.style)
                });
                (choreo, phase)
            }),
            // Cano mais longo chega antes do punho.
            reach: melee.map_or(0.0, |m| m.style.reach()),
            cycle: root.translation.y / 32.0 * std::f32::consts::TAU,
            air: velocity.map_or(0.0, |v| (v.y / JUMP_SPEED).clamp(-1.4, 1.0)),
        };

        let joints = held.map_or_else(
            || {
                let mut joints = Joints::gait(&rigging);
                (def.rig)(&mut joints, &rigging);
                joints
            },
            |held| rig::armed_joints(*pose, held.weapon.style(), recoil, &rigging),
        );

        let (a, b) = match limb.kind {
            LimbKind::UpperArm => (joints.shoulder, joints.elbow),
            LimbKind::Forearm => (joints.elbow, joints.hand),
            LimbKind::Thigh => (joints.hip, joints.knee),
            LimbKind::Shin => (joints.knee, joints.foot),
        };
        let idle = (time.elapsed_secs() * 3.2 + limb.side).sin() * 0.3;
        let z = if rigging.front() { 30.4 } else { 29.6 } + idle * 0.01;
        let target = segment_target(a, b, z);
        if !motion.0 {
            *transform = target;
            motion.0 = true;
        } else {
            // Contato chega quase instantaneamente; locomocao preserva inercia.
            let rate = if *pose == Pose::PunchStrike {
                42.0
            } else {
                18.0
            };
            let blend = 1.0 - (-dt * rate).exp();
            transform.translation = transform.translation.lerp(target.translation, blend);
            transform.rotation = transform.rotation.slerp(target.rotation, blend);
            transform.scale = transform.scale.lerp(target.scale, blend);
        }
    }
}

fn running_dust(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(&Transform, &Velocity, &Grounded, &mut StepDust), With<Player>>,
) {
    for (transform, velocity, grounded, mut dust) in &mut q {
        if !grounded.0 || velocity.x.abs() < 90.0 || !dust.0.tick(time.delta()).just_finished() {
            continue;
        }
        let at = transform.translation.truncate() + Vec2::new(-velocity.x.signum() * 12.0, -38.0);
        commands.spawn((
            AsciiSprite::new(AsciiArt::solid(".\u{00B7}", palette::ASH.with_alpha(0.65))),
            Layer::Fx,
            Transform::from_translation(at.extend(0.0)),
            Velocity(Vec2::new(-velocity.x * 0.10, 28.0)),
            Ghost,
            Lifetime(Timer::from_seconds(0.32, TimerMode::Once)),
            DespawnOnExit(GameState::Fighting),
        ));
    }
}

/// Reconstroi a silhueta quando algo que a desenha muda.
///
/// Este e o unico sistema que escreve a arte de um boneco. Ja foram tres --
/// este, o piscar de invulneravel e um terceiro so para desfazer o piscar --
/// disputando o mesmo campo sem ordem definida entre eles. O piscar hoje entra
/// como `Flash`, que e mais uma entrada deste sistema em vez de um segundo
/// escritor.
///
/// Sem o filtro `Changed` isso marcaria o `AsciiSprite` como sujo todo frame e
/// o rebuild de glifos rodaria a toa.
pub fn apply_pose(
    actors: Query<
        (&Pose, &Facing, &ActorTint, &ActorSkin, &Flash, &Children),
        Or<(
            Changed<Pose>,
            Changed<Facing>,
            Changed<ActorSkin>,
            Changed<Flash>,
        )>,
    >,
    mut visuals: Query<&mut AsciiSprite, With<ActorVisual>>,
) {
    for (pose, facing, tint, skin, flash, children) in &actors {
        let art = skin.0.draw(*pose, tint.0, flash.0);
        for child in children.iter() {
            if let Ok(mut sprite) = visuals.get_mut(child) {
                sprite.art = art.clone();
                sprite.flip_x = facing.0 < 0.0;
            }
        }
    }
}

/// Repinta os oito membros quando a pele ou a piscada mudam.
///
/// Fica separado de `apply_pose` pelo mesmo motivo que a camada Z fica separada
/// do rebuild de glifos: os motivos sao diferentes. A silhueta muda a cada
/// quadro de animacao, os membros so quando o boneco troca de pele ou pisca --
/// juntar os dois refaria oito sprites por quadro de corrida, a toa.
fn apply_skin(
    actors: Query<(&ActorSkin, &Flash, &Children), Or<(Changed<ActorSkin>, Changed<Flash>)>>,
    mut limbs: Query<&mut AsciiSprite, With<ActorLimb>>,
) {
    for (skin, flash, children) in &actors {
        let art = skin.0.limb_art(flash.0);
        for child in children.iter() {
            if let Ok(mut sprite) = limbs.get_mut(child) {
                sprite.art = art.clone();
            }
        }
    }
}

/// Cair da arena mata na hora.
fn fall_out_of_bounds(mut q: Query<(&Transform, &mut Health), With<Player>>) {
    for (transform, mut health) in &mut q {
        if transform.translation.y < KILL_Y && !health.is_dead() {
            health.hp = 0;
        }
    }
}

/// Consome os timers de atordoamento e de animacao de ataque.
fn tick_actor_timers(
    time: Res<Time>,
    mut commands: Commands,
    mut stunned: Query<(Entity, &mut Stunned)>,
    mut attacking: Query<(Entity, &mut Attacking)>,
    mut releases: Query<(Entity, &mut ChainRelease)>,
) {
    for (entity, mut stun) in &mut stunned {
        if stun.0.tick(time.delta()).is_finished() {
            // `Downed` acompanha o atordoamento; sair junto e o que faz o
            // boneco levantar em vez de ficar deitado pra sempre.
            commands.entity(entity).remove::<(Stunned, Downed)>();
        }
    }
    for (entity, mut attack) in &mut attacking {
        if attack.0.tick(time.delta()).is_finished() {
            commands.entity(entity).remove::<Attacking>();
        }
    }
    for (entity, mut release) in &mut releases {
        if release.0.tick(time.delta()).is_finished() {
            commands.entity(entity).remove::<ChainRelease>();
        }
    }
}

/// Aproxima `current` de `target` no maximo `step`.
fn move_towards(current: f32, target: f32, step: f32) -> f32 {
    if (target - current).abs() <= step {
        target
    } else {
        current + (target - current).signum() * step
    }
}

/// Movimento na fase de logica, pose na fase de animacao.
pub struct MotionPlugin;

impl Plugin for MotionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (tick_actor_timers, drive, climb)
                .chain()
                .in_set(AppSet::Logic),
        )
        .add_systems(
            Update,
            (
                fall_out_of_bounds,
                choose_pose,
                apply_pose,
                apply_skin,
                animate_body,
                animate_limbs,
                running_dust,
            )
                .chain()
                .in_set(AppSet::Animate),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use super::super::skin;
    use crate::ascii::AsciiArt;

    /// Um membro tem que encostar nas duas juntas que ele liga.
    ///
    /// O sprite e um glifo so, esticado e girado para cobrir o segmento. Com o
    /// angulo negado ele sai espelhado no eixo vertical -- e como a translacao
    /// usa o ponto medio, que continua certo, so as pontas erram. Os membros
    /// param de se encontrar e o boneco vira riscos soltos, sem que nada
    /// quebre. Membro vertical e o unico que espelhar nao muda, o que fazia
    /// metade do corpo parecer normal e escondia o resto.
    #[test]
    fn o_membro_encosta_nas_duas_juntas() {
        // Meia altura do glifo antes da escala: e a ponta do sprite em espaco
        // local, e o que a escala estica ate o comprimento do segmento.
        const PONTA: f32 = 8.0;

        for (a, b) in [
            // vertical: o caso que espelhar nao denuncia
            (Vec2::new(0.0, 0.0), Vec2::new(0.0, -16.0)),
            // coxa e canela da passada parada, dos dois lados
            (Vec2::new(2.2, -7.0), Vec2::new(5.0, -18.0)),
            (Vec2::new(5.0, -18.0), Vec2::new(7.0, -31.0)),
            (Vec2::new(-2.2, -7.0), Vec2::new(-5.0, -18.0)),
            // braco esticado pra frente, como num soco
            (Vec2::new(2.5, 15.0), Vec2::new(14.0, 9.0)),
            // e pra tras, para pegar o quadrante oposto
            (Vec2::new(2.5, 15.0), Vec2::new(-11.0, 20.0)),
        ] {
            let alvo = segment_target(a, b, 0.0);
            let ponta = alvo.transform_point(Vec3::new(0.0, PONTA, 0.0)).truncate();
            let raiz = alvo.transform_point(Vec3::new(0.0, -PONTA, 0.0)).truncate();

            assert!(
                ponta.distance(b) < 0.01,
                "o membro {a:?}->{b:?} termina em {ponta:?}, nao na junta"
            );
            assert!(
                raiz.distance(a) < 0.01,
                "o membro {a:?}->{b:?} comeca em {raiz:?}, nao na junta"
            );
        }
    }

    #[test]
    fn aproxima_sem_ultrapassar() {
        assert_eq!(move_towards(0.0, 10.0, 3.0), 3.0);
        assert_eq!(move_towards(0.0, 10.0, 100.0), 10.0);
        assert_eq!(move_towards(10.0, 0.0, 4.0), 6.0);
        assert_eq!(move_towards(-5.0, 0.0, 100.0), 0.0);
    }

    /// Monta um boneco completo -- silhueta e os oito membros -- como o spawn
    /// de verdade monta.
    fn montar(mut commands: Commands) {
        let root = commands
            .spawn((
                Pose::IdleA,
                Facing(1.0),
                ActorTint(palette::P1),
                Transform::default(),
                Visibility::default(),
            ))
            .id();
        super::super::spawn_actor_body(
            &mut commands,
            root,
            &skin::STICK,
            palette::P1,
            1.0,
            0.0,
            super::super::face::Face::default(),
        );
    }

    fn app_com_boneco() -> App {
        let mut app = App::new();
        app.add_systems(Startup, montar)
            .add_systems(Update, (apply_pose, apply_skin).chain());
        app.update();
        app
    }

    fn boneco(app: &mut App) -> Entity {
        let world = app.world_mut();
        world
            .query_filtered::<Entity, With<Pose>>()
            .single(world)
            .unwrap()
    }

    /// A silhueta e a arte de um membro qualquer, que e o par que uma troca de
    /// pele tem que mexer.
    fn desenho(app: &mut App) -> (AsciiArt, AsciiArt) {
        let world = app.world_mut();
        let corpo = world
            .query_filtered::<&AsciiSprite, With<ActorVisual>>()
            .single(world)
            .unwrap()
            .art
            .clone();
        let membro = world
            .query_filtered::<&AsciiSprite, With<ActorLimb>>()
            .iter(world)
            .next()
            .unwrap()
            .art
            .clone();
        (corpo, membro)
    }

    /// Trocar a pele repinta o boneco inteiro no frame seguinte, silhueta e
    /// membros, sem que nada fora da camada visual saiba que houve troca.
    ///
    /// E a promessa que justifica a existencia de `Skin`: se so a silhueta
    /// mudasse, uma pele nova sairia com o corpo de uma e os bracos de outra.
    #[test]
    fn trocar_de_pele_repinta_silhueta_e_membros() {
        let mut app = app_com_boneco();
        let (corpo_antes, membro_antes) = desenho(&mut app);
        let boneco = boneco(&mut app);

        app.world_mut()
            .entity_mut(boneco)
            .insert(ActorSkin(&skin::HEAVY));
        app.update();

        let (corpo_depois, membro_depois) = desenho(&mut app);
        assert_ne!(corpo_antes, corpo_depois, "a silhueta nao trocou de pele");
        assert_ne!(membro_antes, membro_depois, "os membros ficaram na antiga");
    }

    /// O piscar acende e apaga sozinho, pelo mesmo caminho.
    ///
    /// Enquanto o piscar escrevia direto no sprite, apagar exigia um sistema
    /// inteiro so para desfazer -- e sem ele o boneco congelava na cor em que a
    /// piscada tivesse parado. Este teste tranca o retorno ao normal.
    #[test]
    fn o_flash_acende_e_apaga_pelo_mesmo_caminho() {
        let mut app = app_com_boneco();
        let (normal, membro_normal) = desenho(&mut app);
        let boneco = boneco(&mut app);

        app.world_mut()
            .entity_mut(boneco)
            .insert(Flash(Some(palette::BLOOD)));
        app.update();
        let (aceso, membro_aceso) = desenho(&mut app);
        assert!(aceso.cells.iter().all(|c| c.color == palette::BLOOD));
        assert!(membro_aceso.cells.iter().all(|c| c.color == palette::BLOOD));

        app.world_mut().entity_mut(boneco).insert(Flash(None));
        app.update();
        let (apagado, membro_apagado) = desenho(&mut app);
        assert_eq!(apagado, normal, "a silhueta congelou na cor da piscada");
        assert_eq!(membro_apagado, membro_normal, "os membros congelaram");
    }

    /// Mudar de pose nao pode mexer nos membros: a arte deles nao depende da
    /// pose, so a posicao. Sem esta separacao o ciclo de corrida remontaria
    /// oito sprites por quadro a toa.
    #[test]
    fn animar_a_pose_nao_remonta_os_membros() {
        let mut app = app_com_boneco();
        let boneco = boneco(&mut app);

        app.world_mut().entity_mut(boneco).insert(Pose::RunA);
        app.update();

        let world = app.world_mut();
        let mexeu = world
            .query_filtered::<Ref<AsciiSprite>, With<ActorLimb>>()
            .iter(world)
            .any(|sprite| sprite.is_changed());
        assert!(!mexeu, "trocar de pose sujou a arte dos membros");
    }
}
