/// Cliente -> dono: so a propria entrada.
fn send_local_input(
    time: Res<Time>,
    runtime: Option<Res<SteamRuntime>>,
    mut session: ResMut<OnlineSession>,
    mut since: Local<f32>,
    players: Query<(&Player, &Intent)>,
) {
    if session.host {
        return;
    }
    let local = session.local_player_id();
    let Some((_, intent)) = players.iter().find(|(player, _)| player.id == local) else {
        return;
    };
    // Contar todo quadro e enviar de vez em quando: e o contador que faz a
    // entrada sobreviver a um pacote perdido, entao ele nao pode pular os
    // quadros em que nao houve envio -- seria justamente o aperto perdido.
    let intent = *intent;
    bump_presses(&mut session.local_presses, &intent);

    let (Some(runtime), Some(owner)) = (runtime, session.owner) else {
        return;
    };
    if !due(&mut since, time.delta_secs(), INPUT_HZ) {
        return;
    }
    let mut packet = Vec::with_capacity(1 + INTENT_BYTES);
    packet.push(PACKET_INPUT);
    packet.extend(encode_intent(&intent, session.local_presses));
    send(&runtime, owner, &packet, SendFlags::UNRELIABLE_NO_DELAY);
}

/// Dono -> clientes: a entrada de todos os lugares, para os bonecos remotos
/// animarem em vez de deslizarem parados.
fn broadcast_inputs(
    time: Res<Time>,
    runtime: Option<Res<SteamRuntime>>,
    mut session: ResMut<OnlineSession>,
    mut since: Local<f32>,
    players: Query<(&Player, &Intent)>,
) {
    if !session.host {
        return;
    }
    let mut table: [Option<Intent>; MAX_PLAYERS] = [None; MAX_PLAYERS];
    for (player, intent) in &players {
        if let Some(slot) = table.get_mut(player.id as usize) {
            *slot = Some(*intent);
        }
        if let Some(presses) = session.table_presses.get_mut(player.id as usize) {
            bump_presses(presses, intent);
        }
    }
    let Some(runtime) = runtime else {
        return;
    };
    if !due(&mut since, time.delta_secs(), INPUT_HZ) {
        return;
    }
    let packet = encode_input_table(&table, &session.table_presses);
    broadcast(&runtime, &session, &packet, SendFlags::UNRELIABLE_NO_DELAY);
}

fn apply_input_table(session: &OnlineSession, data: &[u8]) {
    let Some(table) = decode_input_table(data) else {
        return;
    };
    for (slot, entry) in table.iter().enumerate() {
        // O proprio boneco anda pelo teclado daqui; o que vem do dono sobre ele
        // e sempre mais velho que o que acabou de ser digitado.
        if slot as u8 == session.local_player_id() {
            continue;
        }
        if let Some((intent, presses)) = entry {
            session.remotes[slot].push(*intent, *presses);
        }
    }
}

fn send_snapshot(
    time: Res<Time>,
    runtime: Option<Res<SteamRuntime>>,
    session: Res<OnlineSession>,
    mut since: Local<f32>,
    players: Query<(&Player, &Transform, &Velocity, &Health, &Facing)>,
) {
    let Some(runtime) = runtime else {
        return;
    };
    if !session.host || !due(&mut since, time.delta_secs(), SNAPSHOT_HZ) {
        return;
    }
    let mut actors: Snapshot = [None; MAX_PLAYERS];
    for (player, transform, velocity, health, facing) in &players {
        if let Some(slot) = actors.get_mut(player.id as usize) {
            *slot = Some(ActorSnapshot {
                at: transform.translation.truncate(),
                velocity: velocity.0,
                hp: health.hp,
                facing: facing.0,
            });
        }
    }
    broadcast(
        &runtime,
        &session,
        &encode_snapshot(&actors),
        SendFlags::UNRELIABLE_NO_DELAY,
    );
}

/// Costura o estado do dono no que este cliente ja simulou.
///
/// Cravar a posicao recebida era o que fazia os bonecos tremerem: o pacote
/// descreve o passado, e o proprio jogador via o boneco andar e ser puxado de
/// volta a cada quadro. Aqui a fisica local continua valendo e o snapshot so
/// puxa o erro -- de uma vez quando ele e grande demais para ser erro, aos
/// poucos quando cabe numa correcao.
fn apply_snapshot(
    mut session: ResMut<OnlineSession>,
    mut players: Query<(
        &Player,
        &mut Transform,
        &mut Velocity,
        &mut Health,
        &mut Facing,
    )>,
) {
    if session.host {
        session.pending_snapshot = None;
        return;
    }
    let local = session.local_player_id();
    let Some(snapshot) = session.pending_snapshot.take() else {
        return;
    };
    for (player, mut transform, mut velocity, mut health, mut facing) in &mut players {
        let Some(actor) = snapshot.get(player.id as usize).copied().flatten() else {
            continue;
        };
        let mine = player.id == local;
        let (limit, pull) = if mine {
            (LOCAL_SNAP_DIST, LOCAL_CORRECTION)
        } else {
            (SNAP_DIST, CORRECTION)
        };

        let at = transform.translation.truncate();
        let error = actor.at - at;
        let next = if error.length() > limit {
            actor.at
        } else {
            at + error * pull
        };
        transform.translation.x = next.x;
        transform.translation.y = next.y;

        // O proprio boneco so aceita a velocidade do dono quando ela nao tem
        // como ter saido do teclado daqui: e assim que o knockback chega em
        // quem apanhou sem que andar de lado vire uma briga entre os dois.
        if !mine || (actor.velocity - velocity.0).length() > SHOVE_SPEED {
            velocity.0 = actor.velocity;
        }
        if !mine && facing.0 != actor.facing {
            facing.0 = actor.facing;
        }
        if health.hp != actor.hp {
            health.hp = actor.hp;
        }
    }
}

/// Dono -> clientes: as armas da arena.
fn broadcast_weapons(
    time: Res<Time>,
    runtime: Option<Res<SteamRuntime>>,
    session: Res<OnlineSession>,
    mut since: Local<f32>,
    holders: Query<(&Player, Option<&Held>)>,
    drops: Query<(&NetWeapon, &GroundWeapon, &Transform, &Velocity, Has<ThrownWeapon>)>,
) {
    let Some(runtime) = runtime else {
        return;
    };
    if !session.host || !due(&mut since, time.delta_secs(), WEAPON_HZ) {
        return;
    }
    let mut state = WeaponState::default();
    for (player, held) in &holders {
        if let Some(slot) = state.held.get_mut(player.id as usize) {
            *slot = held.map(|held| HeldState {
                kind: held.kind,
                ammo: held.ammo,
            });
        }
    }
    for (net, ground, transform, velocity, thrown) in &drops {
        if state.ground.len() == MAX_GROUND {
            break;
        }
        state.ground.push(GroundState {
            net: net.0,
            kind: ground.kind,
            ammo: ground.ammo,
            at: transform.translation.truncate(),
            velocity: velocity.0,
            thrown,
        });
    }
    broadcast(
        &runtime,
        &session,
        &encode_weapons(&state),
        SendFlags::UNRELIABLE_NO_DELAY,
    );
}

/// Poe a arena do cliente de acordo com o retrato que o dono mandou.
fn apply_weapon_state(
    mut commands: Commands,
    state_now: Res<State<GameState>>,
    mut session: ResMut<OnlineSession>,
    mut drops: Query<(
        Entity,
        &NetWeapon,
        &mut GroundWeapon,
        &mut Transform,
        &mut Velocity,
    )>,
    mut holders: Query<(Entity, &Player, &Facing, Option<&mut Held>)>,
) {
    // Fora da luta nao existe arma para replicar, e um retrato atrasado
    // chegando na espera armaria a sala inteira do nada.
    if session.host || *state_now.get() != GameState::Fighting {
        session.pending_weapons = None;
        return;
    }
    let Some(state) = session.pending_weapons.take() else {
        return;
    };

    let mut known: Vec<u16> = Vec::new();
    for (entity, net, mut ground, mut transform, mut velocity) in &mut drops {
        let Some(target) = state.ground.iter().find(|weapon| weapon.net == net.0) else {
            // Sumiu do retrato: alguem pegou, ou ela caiu fora da arena.
            commands.entity(entity).despawn();
            continue;
        };
        known.push(net.0);
        ground.ammo = target.ammo;
        // Uma folga antes de reposicionar: a arma quase sempre esta parada, e
        // reescrever a posicao dela a cada pacote faria o glifo vibrar no chao.
        if transform.translation.truncate().distance(target.at) > 6.0 {
            transform.translation.x = target.at.x;
            transform.translation.y = target.at.y;
        }
        velocity.0 = target.velocity;
    }

    for target in state.ground.iter().filter(|w| !known.contains(&w.net)) {
        let spawned = crate::weapon::spawn_ground_weapon(
            &mut commands,
            Some(target.net),
            target.kind,
            target.ammo,
            target.at,
            target.velocity,
        );
        if target.thrown {
            crate::weapon::mark_thrown(&mut commands, spawned);
        }
    }

    for (entity, player, facing, held) in &mut holders {
        let target = state.held.get(player.id as usize).copied().flatten();
        match (target, held) {
            // Mesma arma: so a municao muda, e trocar o `Held` inteiro
            // reiniciaria a cadencia a cada pacote.
            (Some(want), Some(mut have)) if have.kind == want.kind => {
                if have.ammo != want.ammo {
                    have.ammo = want.ammo;
                }
            }
            // Trocou de arma na mao: sobrescrever em vez de tirar e por de
            // volta, senao o icone que pende do boneco seria removido junto e
            // a arma ficaria invisivel ate a proxima troca.
            (Some(want), Some(_)) => {
                commands
                    .entity(entity)
                    .insert(crate::weapon::held_weapon(want.kind, want.ammo));
            }
            (Some(want), None) => {
                crate::weapon::equip(&mut commands, entity, want.kind, want.ammo, facing.0)
            }
            (None, Some(_)) => {
                commands.entity(entity).remove::<Held>();
            }
            (None, None) => {}
        }
    }
}

fn broadcast_round_over(
    runtime: Option<Res<SteamRuntime>>,
    session: Res<OnlineSession>,
    result: Res<RoundResult>,
) {
    let Some(runtime) = runtime else {
        return;
    };
    if !session.host {
        return;
    }
    broadcast(
        &runtime,
        &session,
        &encode_round_over(&result),
        SendFlags::RELIABLE_NO_NAGLE,
    );
}

