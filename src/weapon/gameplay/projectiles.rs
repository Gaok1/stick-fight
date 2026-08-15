/// Distancia em que a granada sente um corpo e estoura na hora.
///
/// Um pouco maior que o colisor dela para o estouro acontecer ao encostar, e
/// nao depois de atravessar meio boneco.
const BOMB_TOUCH: f32 = 30.0;

/// Conta o pavio -- ou o toque -- e estoura.
///
/// O estouro e so mais um [`Hitbox`] -- grande, curto e parado. Nada no
/// combate precisou saber que existe explosao: a mesma resolucao de dano que
/// trata soco e bala trata isto.
///
/// Encostar num corpo detona na hora. O pavio deixou de ser o relogio do golpe
/// e virou so o teto: e o tempo maximo que ela fica quicando sem achar
/// ninguem. Isso troca "prever onde o outro vai estar daqui a um segundo" por
/// "acertar o corpo", que e uma mira mais direta e bem mais engracada de levar.
fn tick_fuses(
    time: Res<Time>,
    mut commands: Commands,
    mut shake: MessageWriter<crate::fx::Shake>,
    bodies: Query<
        (Entity, &Transform, &Collider),
        Or<(With<Player>, With<crate::actor::TrainingDummy>)>,
    >,
    mut bombs: Query<(Entity, &mut Fuse, &Transform)>,
) {
    for (entity, mut fuse, transform) in &mut bombs {
        let at = transform.translation.truncate();
        // Nao vale o dono: a granada sai da mao dele e passaria raspando pelo
        // proprio corpo antes de chegar a qualquer lugar.
        let touched = bodies.iter().any(|(body, body_at, collider)| {
            body != fuse.owner
                && overlap(
                    Rect::from_center_half_size(at, Vec2::splat(BOMB_TOUCH * 0.5)),
                    collider.aabb(body_at.translation.truncate()),
                )
        });
        if !fuse.timer.tick(time.delta()).is_finished() && !touched {
            continue;
        }
        commands.entity(entity).despawn();

        commands.spawn((
            Hitbox {
                owner: fuse.owner,
                damage: fuse.damage,
                // Empurrao pra cima: o estouro lanca, e num mapa com buraco
                // isso vale mais que o dano.
                knockback: Vec2::new(0.0, 430.0),
                stun: crate::combat::HIT_STUN,
            },
            // Quem morre aqui nao cai: desmonta.
            crate::combat::Explosive,
            Lifetime(Timer::from_seconds(0.09, TimerMode::Once)),
            Collider::size(fuse.blast, fuse.blast * 0.8),
            Transform::from_translation(at.extend(0.0)),
            DespawnOnExit(GameState::Fighting),
        ));

        commands.spawn((
            AsciiSprite::new(AsciiArt::solid(
                "\u{2591}\u{2592}\u{2593}\u{2588}\u{2593}\u{2592}\u{2591}",
                palette::BLOOD,
            )),
            Layer::Fx,
            Transform::from_translation(at.extend(0.0)).with_scale(Vec3::splat(2.4)),
            Lifetime(Timer::from_seconds(0.16, TimerMode::Once)),
            DespawnOnExit(GameState::Fighting),
        ));
        for i in 0..26 {
            // Fogo por fora, brasa por dentro: duas cores no mesmo estouro
            // separam a bola de fogo da fumaca que sobra.
            let (glyph, color) = if i % 3 == 0 {
                ("\u{2593}", palette::EMBER)
            } else {
                ("*", palette::GOLD)
            };
            commands.spawn((
                AsciiSprite::new(AsciiArt::solid(glyph, color)),
                Layer::Fx,
                Transform::from_translation(at.extend(0.0)),
                Velocity(Vec2::new(
                    fastrand::f32() * 760.0 - 380.0,
                    fastrand::f32() * 620.0 - 140.0,
                )),
                Ghost,
                Falls,
                Lifetime(Timer::from_seconds(
                    0.30 + fastrand::f32() * 0.35,
                    TimerMode::Once,
                )),
                DespawnOnExit(GameState::Fighting),
            ));
        }
        shake.write(crate::fx::Shake(0.75));
    }
}

/// Prende no alvo o projetil que o combate marcou como cravado.
///
/// Deixa de ser projetil e vira enfeite: sem velocidade nao entra na fisica,
/// sem rastro nao pinga mais, e como filho do alvo ele anda junto com o corpo
/// sem que nada precise ficar reposicionando faca todo quadro.
fn stick_projectiles(
    mut commands: Commands,
    stuck: Query<(Entity, &StuckInto, &Transform), Added<StuckInto>>,
) {
    for (entity, stuck, transform) in &stuck {
        commands
            .entity(entity)
            .remove::<(
                Projectile,
                Sticky,
                StuckInto,
                Velocity,
                Ghost,
                Collider,
                Trail,
                Lifetime,
            )>()
            .insert(
                Transform::from_translation(stuck.offset.extend(Layer::Projectile.z()))
                    .with_rotation(transform.rotation),
            )
            .insert(ChildOf(stuck.host));
    }
}

/// Consome o coice; sem isto a arma ficaria apontada pro ceu.
fn tick_recoil(time: Res<Time>, mut commands: Commands, mut q: Query<(Entity, &mut Recoiling)>) {
    for (entity, mut recoiling) in &mut q {
        if recoiling.timer.tick(time.delta()).is_finished() {
            commands.entity(entity).remove::<Recoiling>();
        }
    }
}

fn emit_projectile_trails(
    time: Res<Time>,
    mut commands: Commands,
    mut projectiles: Query<(&Transform, &Velocity, &mut Trail)>,
) {
    for (transform, velocity, mut trail) in &mut projectiles {
        if !trail.timer.tick(time.delta()).just_finished() {
            continue;
        }
        let at = transform.translation.truncate()
            + Vec2::new(fastrand::f32() * 4.0 - 2.0, fastrand::f32() * 4.0 - 2.0);
        if trail.look == WeaponLook::Book {
            let behind = -velocity.0.normalize_or(Vec2::X) + Vec2::Y * 0.28;
            arcane_flame(&mut commands, at, behind, 38.0, 0.58);
            if fastrand::usize(..3) == 0 {
                arcane_flame(&mut commands, at, behind.perp(), 24.0, 0.36);
            }
            continue;
        }
        let (art, color, scale, life, drift) = match trail.look {
            WeaponLook::Pistol => (
                "\u{00b7}\u{2500}",
                trail.color,
                Vec2::new(0.9, 0.45),
                0.16,
                8.0,
            ),
            WeaponLook::Shotgun => ("\u{00b7}", palette::GOLD, Vec2::splat(0.52), 0.11, 18.0),
            WeaponLook::Rifle => (
                "\u{2500}\u{2500}",
                trail.color,
                Vec2::new(1.35, 0.34),
                0.12,
                5.0,
            ),
            WeaponLook::Knives => (
                "\u{2500}\u{25ba}",
                palette::ASH,
                Vec2::new(0.82, 0.48),
                0.14,
                3.0,
            ),
            WeaponLook::Bomb => (
                if fastrand::bool() { "*" } else { "\u{2591}" },
                if fastrand::bool() {
                    palette::EMBER
                } else {
                    palette::ASH
                },
                Vec2::splat(0.62),
                0.28,
                28.0,
            ),
            _ => ("\u{00b7}", trail.color, Vec2::splat(0.5), 0.16, 10.0),
        };
        commands.spawn((
            AsciiSprite::new(AsciiArt::solid(art, color.with_alpha(0.72))),
            Layer::Fx,
            Transform::from_translation(at.extend(0.0))
                .with_rotation(Quat::from_rotation_z(velocity.to_angle()))
                .with_scale(scale.extend(1.0)),
            Velocity(Vec2::new(0.0, drift)),
            Ghost,
            Lifetime(Timer::from_seconds(life, TimerMode::Once)),
            DespawnOnExit(GameState::Fighting),
        ));
    }
}

fn pulse_pickups(
    time: Res<Time>,
    mut q: Query<(&PickupPulse, &mut Transform), Without<ThrownWeapon>>,
) {
    for (pulse, mut transform) in &mut q {
        let wave = (time.elapsed_secs() * 4.0 + pulse.0).sin();
        // A respiracao multiplica a escala do chao, nao substitui: escrever
        // `1.0` aqui devolvia toda arma caida ao tamanho cru um quadro depois de
        // ela nascer, e era dai que vinham as armas maiores que os bonecos.
        transform.scale = Vec3::splat(GROUND_SCALE * (1.0 + wave * 0.08));
        transform.rotation = Quat::from_rotation_z(wave * 0.045);
    }
}

/// Esferas pequenas, como polen de flor, misturadas a letras que escapam das
/// paginas. No chao a coroa e mais larga; na mao ela fecha em volta do livro
/// para nao cobrir o rosto do lutador.
fn emit_book_aura(
    time: Res<Time>,
    mut commands: Commands,
    mut books: Query<(&mut BookAura, &GlobalTransform)>,
) {
    for (mut aura, transform) in &mut books {
        if !aura.clock.tick(time.delta()).just_finished() {
            continue;
        }
        let phase = aura.step as f32 * 2.399_963_1;
        let radius = if aura.held {
            Vec2::new(18.0, 9.0)
        } else {
            Vec2::new(27.0, 12.0)
        };
        let offset = Vec2::new(phase.cos() * radius.x, phase.sin() * radius.y + 7.0);
        let tangent = Vec2::new(-phase.sin(), phase.cos());
        let at = transform.translation().truncate() + offset;
        arcane_flame(
            &mut commands,
            at,
            tangent + Vec2::Y * 0.65,
            if aura.held { 28.0 } else { 34.0 },
            if aura.step % 6 < 3 { 0.48 } else { 0.34 },
        );
        if aura.step % 3 == 0 {
            arcane_flame(&mut commands, at, -tangent + Vec2::Y, 20.0, 0.28);
        }
        aura.step = aura.step.wrapping_add(1);
    }
}

fn animate_thrown_weapons(
    time: Res<Time>,
    mut commands: Commands,
    mut weapons: Query<(Entity, &mut ThrownWeapon, &mut Transform)>,
) {
    for (entity, mut thrown, mut transform) in &mut weapons {
        transform.rotate_z(time.delta_secs() * 13.0);
        if thrown.0.tick(time.delta()).is_finished() {
            commands.entity(entity).remove::<(ThrownWeapon, Hitbox)>();
        }
    }
}

fn animate_tumbling_shots(
    time: Res<Time>,
    mut shots: Query<(&TumblingShot, &Velocity, &mut Transform)>,
) {
    for (tumble, velocity, mut transform) in &mut shots {
        let speed = (velocity.length() / 520.0).clamp(0.35, 1.4);
        transform.rotate_z(tumble.0 * speed * time.delta_secs());
        let squash = (velocity.y.abs() / 700.0).clamp(0.0, 0.22);
        transform.scale = Vec3::new(1.0 + squash, 1.0 - squash * 0.45, 1.0);
    }
}

/// Mantem exatamente um icone na mao de quem esta armado, e nenhum no resto.
///
/// Le o estado em vez de escutar a remocao de `Held`. Enquanto isto era um
/// `RemovedComponents`, largar e pegar outra arma no mesmo quadro -- arremessar
/// em cima de um drop, que `throw_weapon` e `pick_up` resolvem em sequencia --
/// tirava e devolvia o componente, e o aviso de remocao sobrevivia aos dois: a
/// limpeza matava o icone recem-criado e a arma nova ficava **invisivel** na
/// mao ate ser perdida de novo.
///
/// O mesmo quadro tambem pode passar por `equip` duas vezes; por isso o
/// segundo icone e descartado, senao ficariam dois glifos empilhados na mesma
/// mao lendo como arma dupla.
fn clear_weapon_icon(
    mut commands: Commands,
    icons: Query<(Entity, &ChildOf), With<WeaponIcon>>,
    armed: Query<(), With<Held>>,
) {
    let mut drawn: Vec<Entity> = Vec::new();
    for (icon, parent) in &icons {
        if armed.contains(parent.0) && !drawn.contains(&parent.0) {
            drawn.push(parent.0);
            continue;
        }
        commands.entity(icon).despawn();
    }
}

