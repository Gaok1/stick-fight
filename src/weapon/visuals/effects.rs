/// O disparo de cada arma, nas tres camadas que ele tem: clarao, capsula e
/// fumaca.
///
/// Uma tabela so, para que "a escopeta abre um leque" e "o rifle cospe uma
/// lanca" fiquem uma linha ao lado da outra e possam ser comparadas de relance.
/// Enquanto foram tres blocos espalhados pelo modulo, as tres armas acabaram
/// com o mesmo sopro em tamanhos diferentes -- que e como se descobre, tarde,
/// que nao ha diferenca nenhuma para o jogador.
///
/// `None` e arma de contato. Ela nunca chega em `fire`, e
/// `toda_arma_de_fogo_tem_disparo_proprio` cobra que continue assim.
fn gunfire(look: WeaponLook) -> Option<(MuzzleFlash, Option<Casing>, Smoke)> {
    let shot = match look {
        // Estalo: leque curto, mas grosso. O que se ve de uma pistola e o
        // clarao redondo, nao o sopro.
        WeaponLook::Pistol => (
            MuzzleFlash {
                tongues: 5,
                spread: 0.95,
                reach: 20.0,
                width: 7.0,
                core: palette::GOLD,
                edge: palette::EMBER,
                life: 0.07,
            },
            Some(Casing {
                glyph: '\u{00b0}',
                color: palette::GOLD,
                speed: 150.0,
                life: 0.8,
            }),
            Smoke {
                puffs: 3,
                rise: 44.0,
                drift: 60.0,
                life: 0.34,
                color: palette::ASH,
            },
        ),
        // Leque de quase noventa graus e mais curto que o cano: a 12 e largura,
        // nao alcance. E a unica que enche a boca de fumaca.
        WeaponLook::Shotgun => (
            MuzzleFlash {
                tongues: 9,
                spread: 1.55,
                reach: 30.0,
                width: 11.0,
                core: palette::MAGMA,
                edge: palette::EMBER,
                life: 0.13,
            },
            Some(Casing {
                glyph: 'O',
                color: palette::BLOOD,
                speed: 190.0,
                life: 0.95,
            }),
            Smoke {
                puffs: 9,
                rise: 40.0,
                drift: 110.0,
                life: 0.62,
                color: palette::ASH,
            },
        ),
        // Lanca: tres linguas quase paralelas e o dobro do alcance da 12, que
        // apaga antes de a fumaca aparecer.
        WeaponLook::Rifle => (
            MuzzleFlash {
                tongues: 3,
                spread: 0.20,
                reach: 62.0,
                width: 5.0,
                core: palette::BONE,
                edge: palette::GOLD,
                life: 0.055,
            },
            Some(Casing {
                glyph: '\u{00b7}',
                color: palette::GOLD,
                speed: 210.0,
                life: 0.7,
            }),
            Smoke {
                puffs: 2,
                rise: 52.0,
                drift: 40.0,
                life: 0.22,
                color: palette::ASH,
            },
        ),
        // Arremesso nao tem polvora: o risco e o do braco, e nao sobra capsula
        // nenhuma para cair no chao.
        WeaponLook::Knives => (
            MuzzleFlash {
                tongues: 2,
                spread: 0.55,
                reach: 26.0,
                width: 3.0,
                core: palette::ASH,
                edge: palette::IRON,
                life: 0.09,
            },
            None,
            Smoke {
                puffs: 2,
                rise: 18.0,
                drift: 30.0,
                life: 0.18,
                color: palette::IRON,
            },
        ),
        WeaponLook::Bomb => (
            MuzzleFlash {
                tongues: 4,
                spread: 1.20,
                reach: 14.0,
                width: 9.0,
                core: palette::EMBER,
                edge: palette::ASH,
                life: 0.11,
            },
            None,
            Smoke {
                puffs: 5,
                rise: 66.0,
                drift: 24.0,
                life: 0.5,
                color: palette::ASH,
            },
        ),
        _ => return None,
    };
    Some(shot)
}

fn arcane_flame_frame(index: usize) -> (char, Color) {
    let runes = magic_runes();
    let rune = runes[index % runes.len()];
    (
        rune.glyph.chars().next().expect("runa vazia no Glyph Forge"),
        forge_color(&rune.color),
    )
}

fn arcane_flame(commands: &mut Commands, at: Vec2, aim: Vec2, speed: f32, size: f32) {
    let aim = aim.normalize_or(Vec2::Y);
    let normal = Vec2::new(-aim.y, aim.x);
    let direction = (aim + normal * (fastrand::f32() - 0.5) * 0.55).normalize_or(aim);
    let start = at + normal * (fastrand::f32() - 0.5) * 8.0;
    commands.spawn((
        ArcaneFlame {
            velocity: direction * speed,
            age: 0.0,
            life: 0.45 + fastrand::f32() * 0.28,
            curl: (fastrand::f32() - 0.5) * 110.0,
            frame: 0,
            size,
        },
        AsciiSprite::new(AsciiArt::glyph(
            arcane_flame_frame(0).0,
            arcane_flame_frame(0).1,
        )),
        Layer::Fx,
        Transform::from_translation(start.extend(0.02))
            .with_rotation(Quat::from_rotation_z(
                direction.to_angle() - std::f32::consts::FRAC_PI_2,
            ))
            .with_scale(Vec3::new(0.52 * size, 1.55 * size, 1.0)),
        DespawnOnExit(GameState::Fighting),
    ));
}

fn animate_arcane_flames(
    time: Res<Time>,
    mut commands: Commands,
    mut flames: Query<(
        Entity,
        &mut ArcaneFlame,
        &mut AsciiSprite,
        &mut Transform,
    )>,
) {
    let dt = time.delta_secs();
    for (entity, mut flame, mut sprite, mut transform) in &mut flames {
        flame.age += dt;
        if flame.age >= flame.life {
            commands.entity(entity).despawn();
            continue;
        }
        let life = flame.age / flame.life;
        let normal = Vec2::new(-flame.velocity.y, flame.velocity.x).normalize_or_zero();
        let curl = flame.curl;
        flame.velocity +=
            (Vec2::Y * 62.0 + normal * curl * (life * std::f32::consts::PI).sin()) * dt;
        flame.velocity *= 1.0 - dt * 0.42;
        transform.translation += (flame.velocity * dt).extend(0.0);

        let frames = magic_runes().len();
        let frame = ((life * frames as f32) as usize).min(frames - 1);
        if frame != flame.frame {
            flame.frame = frame;
            let (glyph, color) = arcane_flame_frame(frame);
            sprite.art = AsciiArt::glyph(glyph, color);
        }
        transform.rotation = Quat::from_rotation_z(
            flame.velocity.to_angle() - std::f32::consts::FRAC_PI_2,
        );
        transform.scale = Vec3::new(
            (0.52 - life * 0.24) * flame.size,
            (1.55 + life * 0.75) * flame.size,
            1.0,
        );
    }
}

/// Flor de sigilos vivos: nenhuma chama, capsula ou fumaca de arma entra aqui.
fn arcane_bloom(commands: &mut Commands, at: Vec2, dir: Vec2) {
    commands.spawn((
        AsciiSprite::new(AsciiArt::glyph('\u{03a6}', palette::P3)),
        Layer::Fx,
        Transform::from_translation(at.extend(0.03)).with_scale(Vec3::splat(0.72)),
        Lifetime(Timer::from_seconds(0.18, TimerMode::Once)),
        DespawnOnExit(GameState::Fighting),
    ));

    for i in 0..16 {
        let angle = i as f32 / 16.0 * std::f32::consts::TAU;
        let ray = Vec2::from_angle(angle);
        arcane_flame(
            commands,
            at + ray * 5.0,
            ray + dir * 0.55 + Vec2::Y * 0.15,
            72.0 + fastrand::f32() * 62.0,
            0.64 + fastrand::f32() * 0.38,
        );
    }
}

/// Descarrega o disparo na boca do cano, orientado por ela.
fn muzzle_bloom(commands: &mut Commands, at: Vec2, dir: Vec2, look: WeaponLook, recoil: Recoil) {
    // O grimorio conjura uma coroa viva; nao reutiliza clarao e fumaca de arma.
    if look == WeaponLook::Book {
        arcane_bloom(commands, at, dir);
        return;
    }
    let Some((mut flash, casing, smoke)) = gunfire(look) else {
        return;
    };
    // O coice estica o clarao: arma que pula mais cospe mais fogo, e e o mesmo
    // numero que joga o cano para o alto.
    flash.reach *= 1.0 + recoil.kick * 0.30;
    flash.emit(commands, at, dir);
    if let Some(casing) = casing {
        casing.emit(commands, at, dir);
    }
    smoke.emit(commands, at, dir);
}

/// Dispara a arma equipada e gasta municao.
fn fire(
    time: Res<Time>,
    mut commands: Commands,
    mut shake: MessageWriter<crate::fx::Shake>,
    mut shooters: Query<
        (
            Entity,
            &Intent,
            &Transform,
            &mut Facing,
            &Pose,
            &mut Velocity,
            &mut Held,
        ),
        (With<Player>, Without<Attacking>, Without<Parrying>),
    >,
) {
    for (player, intent, transform, mut facing, pose, mut velocity, mut held) in &mut shooters {
        held.cooldown.tick(time.delta());

        if !intent.special || held.ammo == 0 || pose.locks_control() || !held.cooldown.is_finished()
        {
            continue;
        }

        // Atirar pras costas vira o boneco: o corpo acompanha a mira.
        let dir = turn_to_aim(&mut facing, intent);
        let look = weapon_look(held.weapon.as_ref());
        // A bala sai da boca desenhada da arma, que por sua vez sai da mao que
        // o `rig` posicionou. Enquanto foi um ponto fixo somado ao centro do
        // corpo, mirar para cima disparava de dentro do ombro.
        let arm = aiming_arm(*pose, held.weapon.style(), facing.0, dir);
        let along = (arm.hand - arm.elbow).normalize_or(dir);
        let hand = transform.translation.truncate() + arm.hand;
        let muzzle = on_weapon(look, muzzle_local(look), hand, along, facing.0 < 0.0);

        for shot in held.weapon.shots(dir) {
            let color = shot.color;
            let common = (
                Trail {
                    timer: Timer::from_seconds(0.035, TimerMode::Repeating),
                    color,
                    look,
                },
                AsciiSprite::new(AsciiArt::solid(shot.glyph, color)),
                Layer::Projectile,
                // Deitado na linha de voo: uma lamina atravessada nao le como
                // lamina, e o tiro reto tambem so ganha em apontar pra onde vai.
                Transform::from_translation(muzzle.extend(0.0))
                    .with_rotation(Quat::from_rotation_z(shot.velocity.to_angle()))
                    .with_scale(Vec3::splat(if look == WeaponLook::Book {
                        1.18
                    } else {
                        1.0
                    })),
                Velocity(shot.velocity),
                Collider::size(10.0, 10.0),
                DespawnOnExit(GameState::Fighting),
            );

            match shot.kind {
                ShotKind::Straight | ShotKind::Sticky | ShotKind::Arcane => {
                    let mut projectile = commands.spawn((
                        common,
                        Projectile,
                        Hitbox {
                            owner: player,
                            damage: shot.damage,
                            knockback: shot.knockback,
                            stun: crate::combat::HIT_STUN,
                        },
                        Lifetime(Timer::from_seconds(shot.life, TimerMode::Once)),
                        Ghost,
                    ));
                    if shot.kind == ShotKind::Sticky {
                        projectile.insert(Sticky);
                    }
                }
                // Sem `Ghost` ele bate no cenario e para; sem `Hitbox` ele nao
                // machuca ao encostar. Sem `Projectile` ele nao arrebenta
                // corrente -- quem faz isso e o estouro, nao o arremesso.
                ShotKind::Lobbed {
                    fuse,
                    blast,
                    damage,
                } => {
                    commands.spawn((
                        common,
                        Falls,
                        TumblingShot(if facing.0 < 0.0 { -8.5 } else { 8.5 }),
                        Fuse {
                            timer: Timer::from_seconds(fuse, TimerMode::Once),
                            blast,
                            damage,
                            owner: player,
                        },
                    ));
                }
            }
        }

        let recoil = held.weapon.recoil();

        muzzle_bloom(&mut commands, muzzle, dir, look, recoil);

        // O coice empurra contra o tiro; o pulinho extra e o que deixa a 12
        // servir de propulsor quando se atira pra baixo.
        velocity.0 -= dir * recoil.push;
        velocity.y += recoil.push * 0.18;
        shake.write(crate::fx::Shake(recoil.shake));

        held.cooldown.reset();
        held.ammo = held.ammo.saturating_sub(1);

        commands.entity(player).insert((
            Attacking(Timer::from_seconds(
                held.weapon.cooldown().min(0.18),
                TimerMode::Once,
            )),
            Recoiling {
                timer: Timer::from_seconds(0.10 + recoil.kick * 0.16, TimerMode::Once),
                kick: recoil.kick,
            },
        ));
    }
}

