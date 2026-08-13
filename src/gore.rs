//! Gore: sangue que fica, membro que sai voando, boneco que estoura.
//!
//! Tudo aqui e consequencia de uma mensagem de dano -- nenhum sistema deste
//! modulo decide quem morre, quanto dói ou quem ganha o round. Por isso o
//! combate nunca precisou saber que existe sangue: tirar o plugin da lista
//! deixa o jogo inteiro em pe, so que limpo.
//!
//! A escolha que sustenta o resto e que sangue aqui e materia, e nao particula:
//! a gota tem colisor, cai, encosta no chao e vira mancha. E o que faz uma
//! briga longa deixar o cenario sujo em vez de piscar vermelho e esquecer.

use bevy::prelude::*;

use crate::actor::{ActorLimb, ActorSkin, ActorTint, Health, Severed};
use crate::ascii::{AsciiArt, AsciiSprite, Layer, palette};
use crate::combat::{Damaged, Lifetime};
use crate::physics::{Collider, Falls, Ghost, Grounded, Velocity};
use crate::state::{AppSet, GameState, arena_live};

/// Dano a partir do qual um golpe pode arrancar um membro.
///
/// Abaixo disto nao ha sorteio nenhum: jab nao desmembra, senao a primeira
/// troca de socos ja deixaria os dois bonecos pela metade e o exagero perderia
/// a graca por repeticao.
const LIMB_DAMAGE: i32 = 12;

/// Dano fatal que estoura o boneco em vez de deita-lo.
///
/// So o finalizador de cano, a pancada pesada e os estouros passam daqui --
/// que e exatamente a lista de golpes que merecem uma morte espalhafatosa.
const GIB_DAMAGE: i32 = 22;

/// Teto de manchas no chao.
///
/// Sangue que fica e o ponto do modulo, mas sangue que fica para sempre e
/// entidade acumulando num round que pode durar minutos. Ao passar do teto as
/// mais antigas somem, entao a arena fica suja sem ficar cara.
const MAX_STAINS: usize = 90;

/// Glifos de gota no ar.
const DROPS: [char; 3] = ['\u{00B7}', '\u{00B0}', '\u{2022}'];

/// Glifos de mancha ja assentada.
const STAINS: [char; 3] = ['\u{2584}', '\u{2591}', '\u{2592}'];

/// Vísceras: o que sai de dentro quando o boneco abre.
const ORGANS: [char; 4] = ['\u{2665}', '\u{25D8}', '\u{2022}', '\u{00B0}'];

/// Pedaco de boneco no ar.
#[derive(Component)]
struct Gib {
    /// Giro em radianos por segundo. Some quando o pedaco pousa.
    spin: f32,
    /// Cadencia do sangue que ele vai deixando pelo caminho.
    drip: Timer,
}

/// Gota de sangue ainda voando.
#[derive(Component)]
struct BloodDrop;

/// Mancha ja assentada no chao.
#[derive(Component)]
struct Stain;

/// Boneco estourado: o corpo sumiu, o que sobrou foram os pedacos.
#[derive(Component)]
struct Gibbed;

/// Uma gota com fisica: cai, encosta no chao e vira mancha.
fn drop_blood(commands: &mut Commands, at: Vec2, velocity: Vec2) {
    commands.spawn((
        BloodDrop,
        AsciiSprite::new(AsciiArt::glyph(
            DROPS[fastrand::usize(..DROPS.len())],
            palette::BLOOD,
        )),
        Layer::Fx,
        Transform::from_translation(at.extend(0.0)),
        Velocity(velocity),
        Collider::size(4.0, 4.0),
        Grounded::default(),
        Falls,
        // Gota que cai no vao nao tem chao pra manchar: o teto de vida evita
        // que ela caia para sempre.
        Lifetime(Timer::from_seconds(4.0, TimerMode::Once)),
        DespawnOnExit(GameState::Fighting),
    ));
}

/// Nevoa fina: rapida, sem colisao, so para vender o instante do impacto.
fn blood_mist(commands: &mut Commands, at: Vec2, dir: Vec2, count: usize) {
    for _ in 0..count {
        let angle = dir.to_angle() + (fastrand::f32() - 0.5) * 1.9;
        let speed = 180.0 + fastrand::f32() * 320.0;
        commands.spawn((
            AsciiSprite::new(AsciiArt::glyph(
                DROPS[fastrand::usize(..DROPS.len())],
                palette::BLOOD,
            )),
            Layer::Fx,
            Transform::from_translation(at.extend(0.0)),
            Velocity(Vec2::from_angle(angle) * speed + Vec2::new(0.0, 60.0)),
            Ghost,
            Falls,
            Lifetime(Timer::from_seconds(
                0.25 + fastrand::f32() * 0.35,
                TimerMode::Once,
            )),
            DespawnOnExit(GameState::Fighting),
        ));
    }
}

/// Um pedaco solto no mundo: gira, pinga e pousa onde der.
fn toss_gib(commands: &mut Commands, at: Vec2, art: AsciiArt, velocity: Vec2) {
    commands.spawn((
        Gib {
            spin: (fastrand::f32() - 0.5) * 26.0,
            drip: Timer::from_seconds(0.05, TimerMode::Repeating),
        },
        AsciiSprite::new(art),
        Layer::Fx,
        Transform::from_translation(at.extend(0.0)),
        Velocity(velocity),
        Collider::size(8.0, 8.0),
        Grounded::default(),
        Falls,
        DespawnOnExit(GameState::Fighting),
    ));
}

/// Chance de um golpe arrancar um membro, pelo dano que ele deu.
///
/// Curva e nao limiar: um jab nunca desmembra, um finalizador as vezes, uma
/// paulada na cabeca quase sempre. E o que faz o exagero ser um premio pelo
/// golpe caro em vez de um enfeite que sai em todo soco.
fn tear_chance(amount: i32, explosive: bool) -> f32 {
    if explosive {
        return 1.0;
    }
    ((amount - LIMB_DAMAGE) as f32 / 26.0).clamp(0.0, 0.7)
}

/// Todo dano espirra.
///
/// Sao duas coisas ao mesmo tempo de proposito: a nevoa vende o instante do
/// golpe e some, as gotas viram sujeira que fica. Uma sozinha nao faz o
/// trabalho da outra.
fn splatter(mut commands: Commands, mut damaged: MessageReader<Damaged>) {
    for hit in damaged.read() {
        let dir = if hit.dir == Vec2::ZERO {
            Vec2::Y
        } else {
            hit.dir
        };
        let force = (hit.amount as f32 / 12.0).clamp(0.5, 3.0);

        blood_mist(&mut commands, hit.at, dir, 4 + (force * 4.0) as usize);
        for _ in 0..2 + (force * 2.0) as usize {
            let angle = dir.to_angle() + (fastrand::f32() - 0.5) * 2.4;
            let speed = (110.0 + fastrand::f32() * 230.0) * force;
            drop_blood(
                &mut commands,
                hit.at,
                Vec2::from_angle(angle) * speed + Vec2::new(0.0, 130.0),
            );
        }
    }
}

/// Arranca membro e estoura boneco, conforme o tamanho do golpe.
///
/// Le a vida depois de a resolucao de dano ja ter rodado neste quadro, entao
/// "este golpe matou" e so perguntar se o alvo esta morto -- nao ha estado
/// duplicado dizendo quem morreu de que.
fn butcher(
    mut commands: Commands,
    mut damaged: MessageReader<Damaged>,
    mut shake: MessageWriter<crate::fx::Shake>,
    actors: Query<(&Transform, &Health, &ActorSkin, &ActorTint)>,
    limbs: Query<(Entity, &ActorLimb, &Transform), Without<Severed>>,
) {
    // Um quadro pode trazer varios acertos no mesmo boneco -- o leque da 12 sao
    // quatro. Como `Severed` so entra no mundo depois que os comandos rodam, a
    // consulta ainda mostraria como inteiro o membro que o chumbo anterior ja
    // arrancou, e sairiam dois pedacos da mesma perna.
    let mut arrancados_agora: Vec<Entity> = Vec::new();
    let mut estourados: Vec<Entity> = Vec::new();

    for hit in damaged.read() {
        let Ok((transform, health, skin, tint)) = actors.get(hit.target) else {
            continue;
        };
        if estourados.contains(&hit.target) {
            continue;
        }
        let at = transform.translation.truncate();
        let dir = if hit.dir == Vec2::ZERO {
            Vec2::Y
        } else {
            hit.dir
        };

        // Os membros vivos deste boneco, ja em coordenada de mundo: o corpo nao
        // gira nem escala, entao somar a posicao local basta.
        let inteiros: Vec<(Entity, Vec2)> = limbs
            .iter()
            .filter(|(entity, limb, _)| {
                limb.owner == hit.target && !arrancados_agora.contains(entity)
            })
            .map(|(entity, _, local)| (entity, at + local.translation.truncate()))
            .collect();

        let estourou = health.is_dead() && (hit.explosive || hit.amount >= GIB_DAMAGE);
        let arrancados = if estourou {
            inteiros.len()
        } else if fastrand::f32() < tear_chance(hit.amount, hit.explosive) {
            // Estouro que nao matou ainda leva dois; golpe comum leva um.
            usize::from(hit.explosive) + 1
        } else {
            0
        };

        if !inteiros.is_empty() {
            let primeiro = fastrand::usize(..inteiros.len());
            for i in 0..arrancados.min(inteiros.len()) {
                let (limb, limb_at) = inteiros[(primeiro + i) % inteiros.len()];
                arrancados_agora.push(limb);
                commands.entity(limb).insert((Severed, Visibility::Hidden));
                toss_gib(
                    &mut commands,
                    limb_at,
                    skin.0.limb_art(None),
                    dir * (150.0 + fastrand::f32() * 220.0)
                        + Vec2::new(
                            fastrand::f32() * 160.0 - 80.0,
                            190.0 + fastrand::f32() * 200.0,
                        ),
                );
                drop_blood(
                    &mut commands,
                    limb_at,
                    Vec2::new(fastrand::f32() * 200.0 - 100.0, 150.0),
                );
            }
        }

        // Um boneco sem membro nenhum ainda pode estourar: a cabeca e o resto
        // saem do mesmo jeito. Por isso isto nao mora dentro do laco acima.
        if !estourou {
            continue;
        }
        estourados.push(hit.target);

        // Estourar e tirar o corpo de cena e devolver ele em pedacos. Esconder
        // a raiz leva junto tudo que pendurava nela -- arma na mao, facas
        // cravadas, o rosto -- que e exatamente o que tem que sumir.
        commands
            .entity(hit.target)
            .insert((Gibbed, Visibility::Hidden));

        let cabeca = crate::actor::body_half_height() - 10.0;
        toss_gib(
            &mut commands,
            at + Vec2::Y * cabeca,
            // A cabeca sai com a cara da pele: um robo perde uma cabeca
            // quadrada, e nao o mesmo 'O' de todo mundo.
            AsciiArt::solid("O", tint.0).swapped(skin.0.swap),
            dir * 210.0 + Vec2::new(fastrand::f32() * 120.0 - 60.0, 430.0),
        );
        for organ in ORGANS {
            toss_gib(
                &mut commands,
                at + Vec2::Y * (fastrand::f32() * 20.0 - 6.0),
                AsciiArt::glyph(organ, palette::BLOOD),
                dir * (120.0 + fastrand::f32() * 260.0)
                    + Vec2::new(fastrand::f32() * 300.0 - 150.0, 120.0 + fastrand::f32() * 320.0),
            );
        }
        for _ in 0..16 {
            let angle = fastrand::f32() * std::f32::consts::TAU;
            drop_blood(
                &mut commands,
                at,
                Vec2::from_angle(angle) * (120.0 + fastrand::f32() * 320.0) + Vec2::Y * 120.0,
            );
        }
        blood_mist(&mut commands, at, Vec2::Y, 22);
        shake.write(crate::fx::Shake(0.9));
    }
}

/// Coto sangra: quem perdeu um membro vai pingando por onde anda.
///
/// O membro arrancado continua existindo, congelado onde estava quando saiu --
/// e ele que diz onde e o coto, sem precisar de nenhuma tabela de ferida.
fn bleed(
    time: Res<Time>,
    mut commands: Commands,
    mut clock: Local<f32>,
    owners: Query<&Transform, Without<ActorLimb>>,
    stumps: Query<(&ActorLimb, &Transform), With<Severed>>,
) {
    *clock += time.delta_secs();
    if *clock < 0.11 {
        return;
    }
    *clock = 0.0;

    for (limb, local) in &stumps {
        let Ok(root) = owners.get(limb.owner) else {
            continue;
        };
        if fastrand::f32() > 0.45 {
            continue;
        }
        drop_blood(
            &mut commands,
            root.translation.truncate() + local.translation.truncate(),
            Vec2::new(fastrand::f32() * 70.0 - 35.0, 25.0),
        );
    }
}

/// Gira o pedaco no ar, pinga sangue atras dele e o acalma ao pousar.
fn animate_gibs(
    time: Res<Time>,
    mut commands: Commands,
    mut gibs: Query<(&mut Gib, &mut Transform, &mut Velocity, &Grounded)>,
) {
    let dt = time.delta_secs();
    for (mut gib, mut transform, mut velocity, grounded) in &mut gibs {
        transform.rotate_z(gib.spin * dt);
        if grounded.0 {
            // Atrito: sem isto o pedaco desliza pela arena inteira para sempre.
            let brake = (1.0 - dt * 7.0).max(0.0);
            gib.spin *= brake;
            velocity.x *= brake;
            continue;
        }
        if gib.drip.tick(time.delta()).just_finished() {
            drop_blood(
                &mut commands,
                transform.translation.truncate(),
                Vec2::new(velocity.x * 0.12, 0.0),
            );
        }
    }
}

/// Gota que encostou no chao vira mancha.
///
/// Ela para de ser fisica -- sem velocidade, sem colisor -- e desce para a
/// camada dos itens, atras dos bonecos: sangue no chao nao pode tampar quem
/// esta lutando em cima dele.
fn stain_floor(mut commands: Commands, drops: Query<(Entity, &Grounded), With<BloodDrop>>) {
    for (entity, grounded) in &drops {
        if !grounded.0 {
            continue;
        }
        commands
            .entity(entity)
            .remove::<(BloodDrop, Velocity, Collider, Grounded, Falls)>()
            .insert((
                Stain,
                Layer::Pickup,
                AsciiSprite::new(AsciiArt::glyph(
                    STAINS[fastrand::usize(..STAINS.len())],
                    palette::BLOOD.with_alpha(0.85),
                )),
                Lifetime(Timer::from_seconds(
                    9.0 + fastrand::f32() * 6.0,
                    TimerMode::Once,
                )),
            ));
    }
}

/// Mantem a poca dentro do teto, apagando as mais antigas.
fn limit_stains(mut commands: Commands, stains: Query<Entity, With<Stain>>) {
    let total = stains.iter().count();
    for entity in stains.iter().take(total.saturating_sub(MAX_STAINS)) {
        commands.entity(entity).despawn();
    }
}

/// Vida cheia devolve o corpo inteiro.
///
/// Vale para o respawn do treino e do lobby e para o dummy entre combos. A
/// condicao e a vida no maximo, e nao "esta vivo": um boneco com 1 de vida tem
/// que continuar sem o braco que perdeu.
fn regrow(
    mut commands: Commands,
    healed: Query<(Entity, &Health, Has<Gibbed>), Changed<Health>>,
    limbs: Query<(Entity, &ActorLimb), With<Severed>>,
) {
    for (actor, health, gibbed) in &healed {
        if health.hp < health.max {
            continue;
        }
        if gibbed {
            commands
                .entity(actor)
                .remove::<Gibbed>()
                .insert(Visibility::Inherited);
        }
        for (limb, owned) in &limbs {
            if owned.owner == actor {
                commands
                    .entity(limb)
                    .remove::<Severed>()
                    .insert(Visibility::Inherited);
            }
        }
    }
}

/// Sangue, membros e pedacos.
pub struct GorePlugin;

impl Plugin for GorePlugin {
    fn build(&self, app: &mut App) {
        // Depois da fisica (`Animate`): a gota so vira mancha depois de a
        // resolucao de colisao ter dito que ela encostou no chao neste quadro.
        app.add_systems(
            Update,
            (
                splatter,
                butcher,
                bleed,
                animate_gibs,
                stain_floor,
                limit_stains,
                regrow,
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

    /// Jab nao desmembra, finalizador as vezes, estouro sempre.
    ///
    /// E a curva inteira do modulo num teste: sem o piso, a primeira troca de
    /// socos deixaria os dois bonecos pela metade e o exagero viraria rotina.
    #[test]
    fn so_golpe_caro_arranca_membro() {
        assert_eq!(tear_chance(8, false), 0.0, "o jab desmembra");
        assert_eq!(tear_chance(LIMB_DAMAGE, false), 0.0);
        assert!(tear_chance(20, false) > 0.0, "o finalizador nunca arranca");
        assert!(
            tear_chance(26, false) > tear_chance(20, false),
            "a pancada pesada nao arranca mais que o finalizador"
        );
        assert_eq!(tear_chance(1, true), 1.0, "estouro tem que arrancar sempre");
        // Nem o golpe mais caro do jogo pode ser garantia: se desmembrar virar
        // certeza, todo round acaba igual.
        assert!(tear_chance(999, false) < 1.0);
    }

    /// Um boneco com os oito membros, pronto para apanhar.
    fn arena(hp: i32) -> (App, Entity) {
        let mut app = App::new();
        app.add_message::<Damaged>()
            .add_message::<crate::fx::Shake>()
            .add_systems(Update, butcher);

        let actor = app
            .world_mut()
            .spawn((
                Health { hp, max: 100 },
                ActorSkin::default(),
                ActorTint(palette::BONE),
                Transform::default(),
            ))
            .id();
        for kind in [
            crate::actor::LimbKind::UpperArm,
            crate::actor::LimbKind::Forearm,
            crate::actor::LimbKind::Thigh,
            crate::actor::LimbKind::Shin,
        ] {
            for side in [-1.0, 1.0] {
                app.world_mut().spawn((
                    ActorLimb {
                        owner: actor,
                        side,
                        kind,
                    },
                    Transform::default(),
                ));
            }
        }
        (app, actor)
    }

    fn golpe(target: Entity, amount: i32, explosive: bool) -> Damaged {
        Damaged {
            target,
            amount,
            at: Vec2::ZERO,
            dir: Vec2::Y,
            move_name: "",
            explosive,
        }
    }

    /// Morrer num estouro desmonta o boneco: os oito membros saem, o corpo sai
    /// de cena e o que sobra e pedaco voando.
    #[test]
    fn o_estouro_desmonta_o_boneco() {
        let (mut app, actor) = arena(0);
        app.world_mut().write_message(golpe(actor, 30, true));
        app.update();

        let world = app.world_mut();
        assert_eq!(
            world
                .query_filtered::<Entity, With<Severed>>()
                .iter(world)
                .count(),
            8,
            "sobrou membro no boneco que estourou"
        );
        // Oito membros, uma cabeca e as visceras.
        assert!(
            world.query::<&Gib>().iter(world).count() >= 9 + ORGANS.len(),
            "o estouro nao jogou pedaco suficiente no mundo"
        );
        assert!(
            world.get::<Gibbed>(actor).is_some(),
            "o corpo estourado continuou em cena"
        );
    }

    /// Trocar socos nao pode desmontar ninguem.
    ///
    /// E a outra metade da curva: sem esta garantia o primeiro jab do round ja
    /// deixaria os dois bonecos sem braco e o exagero viraria o estado normal
    /// do jogo.
    #[test]
    fn jab_nao_arranca_nada() {
        let (mut app, actor) = arena(100);
        for _ in 0..40 {
            app.world_mut().write_message(golpe(actor, 8, false));
            app.update();
        }

        let world = app.world_mut();
        assert_eq!(
            world
                .query_filtered::<Entity, With<Severed>>()
                .iter(world)
                .count(),
            0,
            "o jab arrancou membro"
        );
        assert!(world.get::<Gibbed>(actor).is_none());
    }

    /// Todo glifo de sangue tem que existir na pagina de codigo.
    ///
    /// Fora dela o glifo vira `?` em silencio -- e uma poca de interrogacoes no
    /// chao e mais engracada do que deveria.
    #[test]
    fn os_glifos_de_sangue_estao_na_cp437() {
        use crate::ascii::cp437::glyph_index;
        for glyph in DROPS.iter().chain(&STAINS).chain(&ORGANS) {
            assert_ne!(
                glyph_index(*glyph),
                glyph_index('?'),
                "U+{:04X} fora da CP437",
                *glyph as u32
            );
        }
    }
}
