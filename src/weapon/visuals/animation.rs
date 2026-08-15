/// O quadro de contato deixa uma pincelada propria da arma. Nao e hitbox: e a
/// memoria visual do arco, atras da lamina ou do bastao que realmente acertou.
fn melee_sweep(commands: &mut Commands, look: WeaponLook, at: Vec2, angle: f32, step: u8) {
    let (art, color, scale, life) = match look {
        WeaponLook::Katana => (
            "\u{00b7}\u{2500}\u{2500}\u{2550}\u{2550}\u{2666}",
            palette::BONE,
            Vec2::new(1.55, 0.66),
            0.14,
        ),
        WeaponLook::Nunchaku => (
            "\u{00b0}\u{00b7}\u{00b0}\u{00b7}\u{00b0}",
            palette::GOLD,
            Vec2::new(1.25, 0.8),
            0.12,
        ),
        WeaponLook::Knife | WeaponLook::Knives => (
            "\u{00b7}\u{2500}\u{25ba}",
            palette::BONE,
            Vec2::new(1.18, 0.62),
            0.09,
        ),
        WeaponLook::Pipe => (
            "\u{2591}\u{2592}\u{2593}\u{2588}",
            palette::ASH,
            Vec2::new(1.2, 0.72),
            0.13,
        ),
        _ => (
            "\u{00b7}\u{2500}\u{2666}",
            palette::GOLD,
            Vec2::new(0.9, 0.55),
            0.08,
        ),
    };
    let spin = (step as f32 - 1.0) * 0.24;
    commands.spawn((
        AsciiSprite::new(AsciiArt::solid(art, color.with_alpha(0.78))),
        Layer::Fx,
        Transform::from_translation(at.extend(0.0))
            .with_rotation(Quat::from_rotation_z(angle + spin))
            .with_scale(scale.extend(1.0)),
        Lifetime(Timer::from_seconds(life, TimerMode::Once)),
        DespawnOnExit(GameState::Fighting),
    ));
    for i in 0..5 {
        let side = i as f32 - 2.0;
        let dir = Vec2::from_angle(angle + side * 0.14);
        commands.spawn((
            AsciiSprite::new(AsciiArt::glyph(
                if i == 2 { '\u{2666}' } else { '\u{00b7}' },
                if i == 2 { palette::GOLD } else { palette::ASH },
            )),
            Layer::Fx,
            Transform::from_translation((at + dir * (12.0 + i as f32 * 5.0)).extend(0.0))
                .with_scale(Vec3::splat(0.45 + i as f32 * 0.07)),
            Velocity(dir * (70.0 + i as f32 * 18.0)),
            Ghost,
            Lifetime(Timer::from_seconds(
                0.12 + i as f32 * 0.025,
                TimerMode::Once,
            )),
            DespawnOnExit(GameState::Fighting),
        ));
    }
}

/// Mantem o icone da arma do lado para o qual o boneco olha.
fn animate_weapon_icon(
    time: Res<Time>,
    mut commands: Commands,
    players: Query<
        (
            &Facing,
            &Pose,
            &Intent,
            &Transform,
            &Held,
            Option<&MeleeAttack>,
            Option<&Recoiling>,
            &Children,
        ),
        With<Player>,
    >,
    mut icons: Query<
        (&mut Transform, &mut AsciiSprite, &mut WeaponRig),
        (With<WeaponIcon>, Without<Player>),
    >,
) {
    use crate::actor::pose::MeleeKind;

    let dt = time.delta_secs().min(0.05);

    for (facing, pose, intent, root, held, melee, recoiling, children) in &players {
        let (aim, recoil) = animated_aim(intent, facing, recoiling);
        let flipped = facing.0 < 0.0;
        let gait = pose.run_frame().map_or(0.0, |_| {
            -(root.translation.x / (9.0 * crate::actor::rig::RUN.len() as f32)
                * std::f32::consts::TAU)
                .cos()
        });
        let (kind, step) = melee.map_or((MeleeKind::Chain, 0), |m| (m.kind, m.step));
        let weapon_side = if pose.melee_phase().is_some() {
            crate::actor::pose::weapon_side(kind, step, held.weapon.style())
        } else {
            1.0
        };
        let rigging = crate::actor::rig::Rigging {
            side: weapon_side,
            facing: facing.0,
            gait,
            aim,
            strike: pose.melee_phase().map(|phase| {
                (
                    crate::actor::pose::strike_for(step, kind, held.weapon.style()),
                    phase,
                )
            }),
            reach: melee.map_or(0.0, |m| m.style.reach()),
            cycle: root.translation.y / 32.0 * std::f32::consts::TAU,
            air: 0.0,
        };
        let arm = crate::actor::rig::armed_joints(*pose, held.weapon.style(), recoil, &rigging);
        for child in children.iter() {
            if let Ok((mut transform, mut sprite, mut visual)) = icons.get_mut(child) {
                let along = (arm.hand - arm.elbow).normalize_or(Vec2::new(facing.0, 0.0));
                let scale = held_scale(visual.look);
                // O centro do sprite sai do punho pela mesma conta que a boca
                // do cano. Uma so: assim a arma desenhada e o tiro nunca
                // discordam sobre onde ela esta nem sobre para que lado ela
                // aponta.
                let center = on_weapon(visual.look, Vec2::ZERO, arm.hand, along, flipped);
                let target = Transform::from_translation(center.extend(0.1))
                    .with_rotation(Quat::from_rotation_z(aim_angle(along, flipped)))
                    .with_scale(Vec3::splat(scale));
                // O antebraco usa a mesma inercia. Arma e mao chegam juntas
                // ao alvo, em vez de o icone saltar de junta em junta.
                let rate = if *pose == Pose::PunchStrike {
                    42.0
                } else {
                    18.0
                };
                let blend = 1.0 - (-dt * rate).exp();
                transform.translation = transform.translation.lerp(target.translation, blend);
                transform.rotation = transform.rotation.slerp(target.rotation, blend);
                transform.scale = transform.scale.lerp(target.scale, blend);
                // Comparar antes de escrever, como no resto da arte: qualquer
                // escrita em `AsciiSprite` respawna os glifos filhos, e o lado
                // para o qual o boneco olha quase nunca muda.
                if sprite.flip_x != flipped {
                    sprite.flip_x = flipped;
                }
                let art = weapon_art(held.weapon.held_art());
                if sprite.art != art {
                    sprite.art = art;
                }
                visual.phase = pose.melee_phase();
                visual.step = step;
                visual.heavy = kind == MeleeKind::Heavy;
                visual.recoil = recoil;
                visual.flipped = flipped;
                if visual.phase == Some(1) && visual.last_phase != Some(1) {
                    // A pincelada sai da ponta desenhada da arma, e nao de uma
                    // distancia estimada a partir do punho: lamina longa e
                    // faca curta deixam o rastro em lugares diferentes porque
                    // as pontas delas estao em lugares diferentes.
                    let tip =
                        on_weapon(visual.look, muzzle_local(visual.look), arm.hand, along, flipped);
                    let world = root.translation.truncate()
                        + tip
                        + along * held.weapon.style().reach() * 0.35;
                    melee_sweep(&mut commands, visual.look, world, along.to_angle(), step);
                }
                visual.last_phase = visual.phase;
            }
        }
    }
}

/// Toca mecanismos e movimento secundario. A corrente usa Verlet e restricoes
/// de distancia: cada elo carrega inercia, gravidade e o tranco do golpe, em vez
/// de percorrer uma senoide decorativa que atravessaria os proprios bastoes.
fn animate_weapon_rigs(
    time: Res<Time>,
    mut rigs: Query<(&mut WeaponRig, &Transform, &AsciiSprite)>,
    mut parts: Query<(&WeaponPart, &mut Transform, &mut AsciiSprite), Without<WeaponRig>>,
) {
    let now = time.elapsed_secs();
    let dt = time.delta_secs().min(1.0 / 30.0);

    for (mut rig, root, root_sprite) in &mut rigs {
        let side = if rig.flipped || root_sprite.flip_x {
            -1.0
        } else {
            1.0
        };
        let phase = rig.phase;
        let step = rig.step;
        let heavy = rig.heavy;
        // A ancora sai da argola desenhada no bastao, e nao de um numero
        // proprio: redesenhar o bastao mais curto passava a deixar a corrente
        // nascendo no ar ao lado dele.
        let anchor = rig
            .chain
            .as_ref()
            .map_or_else(|| muzzle_local(rig.look) + rig.origin, |chain| chain.anchor);

        if let Some(chain) = rig.chain.as_mut() {
            if !chain.ready {
                for i in 0..CHAIN_POINTS {
                    let point = chain.rest[i];
                    chain.points[i] = point;
                    chain.previous[i] = point;
                }
                chain.ready = true;
            }

            // Cada nova porrada pega a outra extremidade. Como os porretes sao
            // iguais, inverter os pontos troca quem esta fixo sem trocar o
            // sprite raiz; transladar tudo mantem a nova pega exatamente na mao.
            let attacking = phase.is_some();
            if attacking && !chain.attacking {
                chain.points.reverse();
                chain.previous.reverse();
                chain.links.reverse();
                let shift = anchor - chain.points[0];
                for point in &mut chain.points {
                    *point += shift;
                }
                for point in &mut chain.previous {
                    *point += shift;
                }
            }
            chain.attacking = attacking;

            let display_gravity =
                (root.rotation.inverse() * Vec3::new(0.0, -620.0, 0.0)).truncate();
            let gravity = Vec2::new(display_gravity.x * side, display_gravity.y);
            let drive = match phase {
                // O combo e moinho: o bastao solto gira em volta do punho nas
                // tres fases. Enquanto so a fase de contato empurrava, ele
                // parava entre um quadro e outro -- e bastao parado pendurado
                // no fim de uma corda le como bastao unico com um enfeite.
                Some(fase) if !heavy => {
                    let spin = now * 21.0 + step as f32 * 2.1 + fase as f32 * 1.4;
                    Vec2::from_angle(spin) * (950.0 + fase as f32 * 620.0)
                }
                // O pesado envolve: volta larga e lenta, para a corrente dar
                // a volta em torno do corpo em vez de chicotear e voltar pelo
                // mesmo caminho.
                Some(fase) => {
                    let spin = now * 8.5 + fase as f32 * 2.0;
                    Vec2::from_angle(spin) * (1520.0 + fase as f32 * 450.0)
                }
                None => Vec2::from_angle(now * 1.7) * 90.0,
            };

            chain.points[0] = anchor;
            for i in 1..CHAIN_POINTS {
                let velocity = (chain.points[i] - chain.previous[i]) * 0.965;
                chain.previous[i] = chain.points[i];
                chain.points[i] += velocity + (gravity + drive) * dt * dt;
            }
            for _ in 0..8 {
                chain.points[0] = anchor;
                for i in 1..CHAIN_POINTS {
                    let delta = chain.points[i] - chain.points[i - 1];
                    let distance = delta.length().max(0.001);
                    let correction = delta * ((distance - chain.links[i - 1]) / distance);
                    if i == 1 {
                        chain.points[i] -= correction;
                    } else {
                        chain.points[i - 1] += correction * 0.5;
                        chain.points[i] -= correction * 0.5;
                    }
                }
            }
            chain.points[0] = anchor;
        }

        for (order, entity) in rig.parts.iter().copied().enumerate() {
            let Ok((part, mut transform, mut sprite)) = parts.get_mut(entity) else {
                continue;
            };
            let pulse = (now * 5.0 + order as f32 * 0.9).sin();
            let mut at = part.rest;
            let mut angle = part.angle;
            let mut scale = part.scale;

            match part.role {
                PartRole::Static => {}
                PartRole::Sight => {
                    let glow = 1.0 + pulse.max(0.0) * 0.22 + rig.recoil * 0.3;
                    scale *= glow;
                }
                PartRole::Bolt => {
                    at.x -= rig.recoil * 8.0;
                    scale.y *= 1.0 + rig.recoil * 0.22;
                }
                PartRole::Pump => {
                    at.x -= rig.recoil * 12.0;
                    angle -= rig.recoil * 0.08;
                }
                PartRole::Fuse => {
                    at += Vec2::new(pulse * 1.8, pulse.abs() * 1.5);
                    angle += pulse * 0.22;
                    scale *= 0.92 + pulse.max(0.0) * 0.3;
                }
                PartRole::BladeGleam => {
                    let action = rig.phase.map_or(0.0, |phase| (phase as f32 + 1.0) / 3.0);
                    at.x += action * 13.0 + pulse * 0.8;
                    scale.x *= if rig.phase.is_some() { 1.25 } else { 0.72 };
                    scale.y *= 0.72 + pulse.abs() * 0.22;
                }
                PartRole::Charm => {
                    angle += (now * 3.1 + rig.step as f32).sin()
                        * if rig.phase.is_some() { 0.48 } else { 0.16 };
                    at.y += pulse * 1.2;
                }
                PartRole::ChainLink(index) => {
                    let Some(chain) = rig.chain.as_ref() else {
                        continue;
                    };
                    at = chain.points[index];
                    let neighbor = if index == 0 {
                        chain.points[1]
                    } else {
                        chain.points[index - 1]
                    };
                    angle = (at.y - neighbor.y).atan2(at.x - neighbor.x);
                }
                PartRole::FreeBaton => {
                    let Some(chain) = rig.chain.as_ref() else {
                        continue;
                    };
                    let tip = chain.points[CHAIN_POINTS - 1];
                    let before = chain.points[CHAIN_POINTS - 2];
                    let tangent = (tip - before).normalize_or(Vec2::X);
                    let authored = chain.rest[CHAIN_POINTS - 1] - chain.rest[CHAIN_POINTS - 2];
                    let turn = tangent.to_angle() - authored.to_angle();
                    at = tip
                        + Vec2::from_angle(turn)
                            .rotate(part.rest - chain.rest[CHAIN_POINTS - 1]);
                    angle = part.angle + turn;
                    if rig.phase == Some(1) {
                        angle += (now * 22.0).sin() * 0.22;
                    }
                }
                PartRole::FreeBatonCap => {
                    let Some(chain) = rig.chain.as_ref() else {
                        continue;
                    };
                    let tip = chain.points[CHAIN_POINTS - 1];
                    let before = chain.points[CHAIN_POINTS - 2];
                    let tangent = (tip - before).normalize_or(Vec2::X);
                    let authored = chain.rest[CHAIN_POINTS - 1] - chain.rest[CHAIN_POINTS - 2];
                    let turn = tangent.to_angle() - authored.to_angle();
                    at = tip
                        + Vec2::from_angle(turn)
                            .rotate(part.rest - chain.rest[CHAIN_POINTS - 1]);
                    angle = part.angle + turn;
                }
            }

            at.x *= side;
            angle *= side;
            if part.authored {
                scale.x *= side;
            }
            transform.translation = at.extend(0.01 + order as f32 * 0.001);
            transform.rotation = Quat::from_rotation_z(angle);
            transform.scale = scale.extend(1.0);

            // O brilho do pavio e a unica troca de pintura ciclica. Sao dois
            // tons, nao uma reconstrucao continua a cada frame.
            if matches!(part.role, PartRole::Fuse) {
                let color = if pulse > 0.0 {
                    palette::GOLD
                } else {
                    palette::EMBER
                };
                let art = sprite.art.recolored(color);
                if sprite.art != art {
                    sprite.art = art;
                }
            }
        }
    }
}

fn spawn_crosshair(mut commands: Commands) {
    commands.spawn((
        Crosshair,
        AsciiSprite::new(AsciiArt::solid("\u{253C}", palette::ASH)),
        Layer::Fx,
        Transform::default(),
        DespawnOnExit(GameState::Fighting),
    ));
}

/// Gruda o reticulo no cursor e o acende quando ha arma na mao.
fn track_crosshair(
    cursor: Res<CursorWorld>,
    mode: Res<crate::state::GameMode>,
    online: Res<crate::online::OnlineSession>,
    players: Query<(&Player, Has<Held>)>,
    mut crosshair: Query<(&mut Transform, &mut AsciiSprite), With<Crosshair>>,
) {
    let Some(at) = cursor.0 else {
        return;
    };
    // O reticulo e de quem esta no mouse aqui, que online nem sempre e o
    // jogador 1: quem entra num lugar mais alto olhava a arma de outra pessoa.
    let local = if *mode == crate::state::GameMode::Online {
        online.local_player_id()
    } else {
        0
    };
    let armed = players
        .iter()
        .any(|(player, held)| player.id == local && held);
    for (mut transform, mut sprite) in &mut crosshair {
        transform.translation = at.extend(0.0);
        transform.scale = Vec3::splat(if armed { 1.0 } else { 0.7 });
        let art = AsciiArt::solid(
            "\u{253C}",
            if armed {
                palette::GOLD
            } else {
                palette::ASH.with_alpha(0.55)
            },
        );
        if sprite.art != art {
            sprite.art = art;
        }
    }
}

/// Projetil que bate em terreno solido ou sai da arena morre.
///
/// Menos a faca: ela crava na parede e fica tremendo la ate sumir. Custa uma
/// linha e devolve a leitura de onde os tiros passaram.
fn despawn_spent_projectiles(
    mut commands: Commands,
    projectiles: Query<(Entity, &Transform, &Collider, Has<Sticky>), With<Projectile>>,
    solids: Query<(&Transform, &Collider), (With<Solid>, Without<Projectile>)>,
) {
    let blocks: Vec<Rect> = solids
        .iter()
        .map(|(t, c)| c.aabb(t.translation.truncate()))
        .collect();

    for (entity, transform, collider, sticky) in &projectiles {
        let at = transform.translation.truncate();
        let body = collider.aabb(at);
        if at.x.abs() > ARENA_HALF_W + 80.0 {
            commands.entity(entity).despawn();
            continue;
        }
        if !blocks.iter().any(|b| overlap(body, *b)) {
            continue;
        }
        if sticky {
            commands
                .entity(entity)
                .remove::<(Projectile, Sticky, Hitbox, Velocity, Ghost, Collider, Trail)>()
                .insert(Lifetime(Timer::from_seconds(4.0, TimerMode::Once)));
        } else {
            commands.entity(entity).despawn();
        }
    }
}

