/// Resolve todo hitbox contra todo jogador vulneravel.
fn resolve_hits(
    mut commands: Commands,
    mut damaged: MessageWriter<Damaged>,
    mut parried: MessageWriter<Parried>,
    mut hitboxes: Query<
        (
            Entity,
            &mut Hitbox,
            &Transform,
            &Collider,
            Option<&mut Velocity>,
            Has<ThrownWeapon>,
            Has<Sticky>,
            Has<Explosive>,
        ),
        Without<Player>,
    >,
    attackers: Query<&MeleeAttack>,
    mut targets: Query<
        (
            Entity,
            &Transform,
            &Collider,
            &mut Health,
            Option<&mut Velocity>,
            Option<&Parrying>,
            Option<&Hurtbox>,
            Has<TrainingDummy>,
        ),
        (
            Or<(With<Player>, With<TrainingDummy>)>,
            Without<Invulnerable>,
            Without<Hitbox>,
        ),
    >,
) {
    for (
        hit_entity,
        mut hitbox,
        hit_transform,
        hit_collider,
        mut projectile_velocity,
        thrown,
        sticky,
        explosive,
    ) in &mut hitboxes
    {
        let hit_at = hit_transform.translation.truncate();
        let area = hit_collider.aabb(hit_at);

        for (target, transform, collider, mut health, velocity, parry, hurtbox, dummy) in
            &mut targets
        {
            if target == hitbox.owner || health.is_dead() {
                continue;
            }
            // Postura manda: quem esta agachado oferece menos area que o
            // proprio colisor de terreno.
            let at = transform.translation.truncate();
            let body = match hurtbox {
                Some(hurtbox) => hurtbox.aabb(at),
                None => collider.aabb(at),
            };
            if !overlap(area, body) {
                continue;
            }

            if parry.is_some_and(|p| p.0.elapsed_secs() <= PARRY_ACTIVE) {
                let attacker = hitbox.owner;
                if let Some(projectile_velocity) = projectile_velocity.as_deref_mut() {
                    hitbox.owner = target;
                    hitbox.knockback.x *= -1.2;
                    projectile_velocity.0 *= -1.18;
                } else {
                    commands.entity(hit_entity).despawn();
                }
                commands
                    .entity(attacker)
                    .insert(Stunned(Timer::from_seconds(0.38, TimerMode::Once)));
                parried.write(Parried {
                    defender: target,
                    attacker,
                    at: transform.translation.truncate(),
                });
                break;
            }

            health.hp = (health.hp - hitbox.damage).max(if dummy { 1 } else { i32::MIN });
            // Quanto mais ferido, mais longe voa: o round escala naturalmente
            // para um final explosivo e quedas no vao ficam mais provaveis.
            let launch = 1.0 + (1.0 - health.fraction()) * 0.75;
            if let Some(mut velocity) = velocity {
                velocity.0 = hitbox.knockback * launch;
            }

            // O dummy nao ganha janela de invulnerabilidade: com 0.34 s de
            // imunidade os elos do combo (0.18-0.28 s) cairiam no vazio e o
            // treino mediria um combo que o jogo real nao tem.
            if !dummy {
                commands.entity(target).insert((
                    Stunned(Timer::from_seconds(hitbox.stun, TimerMode::Once)),
                    Invulnerable(Timer::from_seconds(HIT_INVULN, TimerMode::Once)),
                ));
                // Golpe que atordoa mais que o normal derruba. Sem a pose de
                // queda o alvo passaria a duracao inteira em pe, na animacao
                // de recuo, e a vantagem da rasteira nao leria na tela.
                if hitbox.stun > HIT_STUN {
                    commands.entity(target).insert(Downed);
                }
            }

            damaged.write(Damaged {
                target,
                amount: hitbox.damage,
                at: transform.translation.truncate(),
                dir: hitbox.knockback.normalize_or_zero(),
                // Um hitbox de soco nasce enquanto o dono ainda esta no golpe,
                // entao da pra perguntar a ele qual elo do combo era.
                move_name: attackers.get(hitbox.owner).map_or("", |attack| {
                    crate::actor::pose::strike_for(attack.step, attack.kind, attack.style).name
                }),
                explosive,
            });

            // Um hitbox acerta uma vez e sai de cena.
            if thrown {
                commands.entity(hit_entity).remove::<Hitbox>();
            } else if sticky {
                // A faca nao some no alvo: ela fica cravada nele. Aqui so fica
                // o pedido -- desmontar projetil (rastro, velocidade, colisao)
                // e coisa de quem sabe o que um projetil e.
                commands.entity(hit_entity).remove::<Hitbox>().insert(
                    crate::weapon::StuckInto {
                        host: target,
                        offset: hit_at - at,
                    },
                );
            } else {
                commands.entity(hit_entity).despawn();
            }
            break;
        }
    }
}

/// Consome `Lifetime` e `Invulnerable`.
fn tick_timers(
    time: Res<Time>,
    mut commands: Commands,
    mut lifetimes: Query<(Entity, &mut Lifetime)>,
    mut invulns: Query<(Entity, &mut Invulnerable)>,
    mut combos: Query<(Entity, &mut ComboChain), Without<Attacking>>,
    mut parries: Query<(Entity, &mut Parrying)>,
    mut parry_cooldowns: Query<(Entity, &mut ParryCooldown)>,
) {
    for (entity, mut life) in &mut lifetimes {
        if life.0.tick(time.delta()).is_finished() {
            commands.entity(entity).despawn();
        }
    }
    for (entity, mut invuln) in &mut invulns {
        if invuln.0.tick(time.delta()).is_finished() {
            commands.entity(entity).remove::<Invulnerable>();
        }
    }
    for (entity, mut combo) in &mut combos {
        if combo.grace.tick(time.delta()).is_finished() {
            commands
                .entity(entity)
                .remove::<(ComboChain, QueuedAttack)>();
        }
    }
    for (entity, mut parry) in &mut parries {
        if parry.0.tick(time.delta()).is_finished() {
            commands.entity(entity).remove::<Parrying>();
        }
    }
    for (entity, mut cooldown) in &mut parry_cooldowns {
        if cooldown.0.tick(time.delta()).is_finished() {
            commands.entity(entity).remove::<ParryCooldown>();
        }
    }
}

/// Detecta a morte e agenda o fim do round.
///
/// A regra e sobrar um, nao alguem morrer: com dois em campo da no mesmo, com
/// quatro a primeira morte so tira um da briga. Publica porque quem cuida de
/// desistencia online precisa correr antes desta decisao no mesmo quadro.
pub(crate) fn check_round_over(
    time: Res<Time>,
    mut commands: Commands,
    mut result: ResMut<RoundResult>,
    mut next: ResMut<NextState<GameState>>,
    delay: Option<ResMut<RoundEndDelay>>,
    players: Query<(&Player, &Health)>,
) {
    let total = players.iter().count();
    let alive: Vec<u8> = players
        .iter()
        .filter(|(_, h)| !h.is_dead())
        .map(|(p, _)| p.id)
        .collect();

    match delay {
        // Precisa haver briga para haver round. Sem esta guarda o quadro em que
        // ninguem nasceu ainda ja contaria como "sobrou um" -- zero vivos --
        // e a luta acabaria antes de comecar.
        None if total >= MIN_PLAYERS && alive.len() <= 1 => {
            // Quem sobrou venceu; se todos morreram junto, e empate.
            result.winner = alive.first().copied();
            result.players = total as u8;
            if let Some(winner) = result.winner {
                result.score[winner as usize] += 1;
            }
            result.rounds += 1;
            commands.insert_resource(RoundEndDelay(Timer::from_seconds(
                ROUND_END_DELAY,
                TimerMode::Once,
            )));
        }
        Some(mut delay) => {
            if delay.0.tick(time.delta()).is_finished() {
                commands.remove_resource::<RoundEndDelay>();
                next.set(GameState::RoundOver);
            }
        }
        None => {}
    }
}

/// Comeca partida nova quando a anterior ja teve vencedor.
///
/// Fica aqui e nao no `Enter` da tela porque a regra e de combate: quem decide
/// que a partida acabou e o placar, nao a UI.
fn start_new_match_if_over(mut result: ResMut<RoundResult>) {
    if result.match_winner().is_some() {
        *result = RoundResult::default();
    }
}

/// Voltar ao menu abandona a partida em andamento.
fn clear_match(mut result: ResMut<RoundResult>) {
    *result = RoundResult::default();
}

/// Registra quantos lugares esta luta usa.
///
/// Vem da mesma conta que cria os bonecos, e nao da contagem de entidades: aqui
/// elas ainda nao existem. O cliente recebe o numero pronto no pacote de fim de
/// round, entao os dois lados desenham o mesmo placar.
fn record_seats(
    mut result: ResMut<RoundResult>,
    mode: Res<GameMode>,
    online: Option<Res<crate::online::OnlineSession>>,
) {
    result.players = crate::actor::seats(*mode, online.as_deref()) as u8;
}

/// Zera o combo em andamento a cada entrada na arena, mas guarda o recorde:
/// ele e da sessao, nao do round.
fn reset_combo_meter(mut meter: ResMut<ComboMeter>) {
    meter.hits = 0;
    meter.damage = 0;
    meter.last_move = "";
    meter.idle = f32::MAX;
}

/// Soma os acertos no dummy e recompoe o alvo quando o combo esfria.
fn track_training_combo(
    time: Res<Time>,
    mut meter: ResMut<ComboMeter>,
    mut damaged: MessageReader<Damaged>,
    mut dummies: Query<&mut Health, With<TrainingDummy>>,
) {
    for hit in damaged.read() {
        if !dummies.contains(hit.target) {
            continue;
        }
        meter.hits += 1;
        meter.damage += hit.amount;
        meter.idle = 0.0;
        if !hit.move_name.is_empty() {
            meter.last_move = hit.move_name;
        }
    }

    if meter.idle >= COMBO_DROP {
        return;
    }
    meter.idle += time.delta_secs();
    if meter.idle < COMBO_DROP {
        return;
    }

    // Combo encerrado: guarda o recorde e devolve o dummy inteiro.
    meter.best_hits = meter.best_hits.max(meter.hits);
    meter.best_damage = meter.best_damage.max(meter.damage);
    meter.hits = 0;
    meter.damage = 0;
    for mut health in &mut dummies {
        health.hp = health.max;
    }
}

/// Onde ninguem perde, cair devolve o jogador ao ponto de partida.
///
/// Vale no treino e na espera do lobby: sao as duas telas em que a arena esta
/// de pe mas nao ha round para decidir. Sem isto, quem cai no vao do lobby fica
/// morto ate a luta comecar -- e a espera existe justamente para bagunçar.
fn recover_the_fallen(
    level: Res<crate::level::CurrentLevel>,
    mut players: Query<(&Player, &mut Transform, &mut Health, &mut Velocity)>,
) {
    for (player, mut transform, mut health, mut velocity) in &mut players {
        if !health.is_dead() {
            continue;
        }
        let at = level
            .0
            .spawn_points()
            .get(player.id as usize)
            .copied()
            .unwrap_or(Vec2::ZERO);
        transform.translation = at.extend(transform.translation.z);
        velocity.0 = Vec2::ZERO;
        health.hp = health.max;
    }
}

/// Marca visualmente quem esta invulneravel, piscando o boneco.
///
/// Sem esse retorno o jogador nao entende por que os golpes pararam de contar.
///
/// Ele nao desenha nada: escreve a cor em `Flash` e deixa a camada de animacao
/// desenhar. Enquanto isto escrevia direto no sprite, eram dois sistemas
/// remontando a arte inteira todo frame -- e disputando o campo com `apply_pose`
/// sem ordem definida entre eles.
fn blink_invulnerable(mut actors: Query<(&Invulnerable, &mut Flash)>) {
    for (invuln, mut flash) in &mut actors {
        let on = (invuln.0.elapsed_secs() * 20.0) as i32 % 2 == 0;
        // `set_if_neq` e o que segura o rebuild em vinte por segundo em vez de
        // sessenta: so a virada da piscada suja o componente.
        flash.set_if_neq(Flash(on.then_some(palette::BLOOD)));
    }
}

/// Apaga a piscada quando a invulnerabilidade acaba.
///
/// Sem isto o boneco ficaria congelado na cor em que o piscar parou: nada mais
/// escreve `Flash`, e `apply_pose` so redesenha o que mudou.
fn clear_flash(mut removed: RemovedComponents<Invulnerable>, mut actors: Query<&mut Flash>) {
    for entity in removed.read() {
        if let Ok(mut flash) = actors.get_mut(entity) {
            flash.set_if_neq(Flash(None));
        }
    }
}

