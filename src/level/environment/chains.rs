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

