/// Apenas o dono do lobby decide o fim do round; os clientes recebem o
/// resultado.
pub fn can_decide_round(mode: Res<GameMode>, session: Res<OnlineSession>) -> bool {
    !mode.is_training() && (*mode != GameMode::Online || session.is_host())
}

/// Este processo decide o que acontece na arena?
///
/// Vale para tudo que nasce de um sorteio ou de uma decisao: arma que cai,
/// arma que e pega, arma que voa. Fora do online a resposta e sempre sim --
/// nao ha ninguem para discordar.
pub fn is_authority(mode: Res<GameMode>, session: Res<OnlineSession>) -> bool {
    *mode != GameMode::Online || session.is_host()
}

/// Este processo obedece a outro?
pub fn is_guest(mode: Res<GameMode>, session: Res<OnlineSession>) -> bool {
    *mode == GameMode::Online && !session.is_host()
}

fn encode_intent(intent: &Intent, presses: Presses) -> [u8; INTENT_BYTES] {
    let held = (intent.up as u8) | ((intent.down as u8) << 1);
    let aim_x = (intent.aim.x.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
    let aim_y = (intent.aim.y.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
    let mut out = [0; INTENT_BYTES];
    out[0] = (intent.move_x.clamp(-1.0, 1.0) * i8::MAX as f32) as i8 as u8;
    out[1] = held;
    out[2..4].copy_from_slice(&aim_x.to_le_bytes());
    out[4..6].copy_from_slice(&aim_y.to_le_bytes());
    out[6..].copy_from_slice(&presses);
    out
}

fn decode_intent(data: &[u8]) -> Option<(Intent, Presses)> {
    if data.len() < INTENT_BYTES {
        return None;
    }
    let mut presses = [0; PULSES];
    presses.copy_from_slice(&data[6..INTENT_BYTES]);
    Some((
        Intent {
            move_x: data[0] as i8 as f32 / i8::MAX as f32,
            up: data[1] & 1 != 0,
            down: data[1] & 2 != 0,
            aim: Vec2::new(
                i16::from_le_bytes(data[2..4].try_into().ok()?) as f32 / i16::MAX as f32,
                i16::from_le_bytes(data[4..6].try_into().ok()?) as f32 / i16::MAX as f32,
            ),
            ..default()
        },
        presses,
    ))
}

/// Mascara dos lugares presentes num pacote de tabela.
///
/// Sem ela toda sala pagaria o tamanho de uma sala cheia: uma briga de dois
/// gastaria metade de cada pacote descrevendo dois lugares vazios.
fn mask_of<T>(table: &[Option<T>; MAX_PLAYERS]) -> u8 {
    table
        .iter()
        .enumerate()
        .fold(0, |mask, (at, slot)| mask | ((slot.is_some() as u8) << at))
}

/// Quantos lugares a mascara descreve, ou `None` se ela cita um que nao existe.
fn present(mask: u8) -> Option<usize> {
    (mask >> MAX_PLAYERS == 0).then(|| mask.count_ones() as usize)
}

fn encode_input_table(
    table: &[Option<Intent>; MAX_PLAYERS],
    presses: &[Presses; MAX_PLAYERS],
) -> Vec<u8> {
    let mask = mask_of(table);
    let mut out = Vec::with_capacity(2 + table.iter().flatten().count() * INTENT_BYTES);
    out.push(PACKET_INPUTS);
    out.push(mask);
    for (slot, intent) in table.iter().enumerate() {
        if let Some(intent) = intent {
            out.extend(encode_intent(intent, presses[slot]));
        }
    }
    out
}

type InputTable = [Option<(Intent, Presses)>; MAX_PLAYERS];

fn decode_input_table(data: &[u8]) -> Option<InputTable> {
    let mask = *data.get(1)?;
    let present = present(mask)?;
    if data.first() != Some(&PACKET_INPUTS) || data.len() != 2 + present * INTENT_BYTES {
        return None;
    }
    let mut table: InputTable = [None; MAX_PLAYERS];
    let mut at = 2;
    for (slot, entry) in table.iter_mut().enumerate() {
        if mask & (1 << slot) == 0 {
            continue;
        }
        *entry = decode_intent(&data[at..at + INTENT_BYTES]);
        at += INTENT_BYTES;
    }
    Some(table)
}

fn encode_snapshot(actors: &Snapshot) -> Vec<u8> {
    let mask = mask_of(actors);
    let mut out = Vec::with_capacity(2 + actors.iter().flatten().count() * ACTOR_BYTES);
    out.push(PACKET_SNAPSHOT);
    out.push(mask);
    for actor in actors.iter().flatten() {
        for value in [
            actor.at.x,
            actor.at.y,
            actor.velocity.x,
            actor.velocity.y,
            actor.facing,
        ] {
            out.extend(value.to_le_bytes());
        }
        out.extend(actor.hp.to_le_bytes());
    }
    out
}

fn decode_snapshot(data: &[u8]) -> Option<Snapshot> {
    let mask = *data.get(1)?;
    let present = present(mask)?;
    if data.first() != Some(&PACKET_SNAPSHOT) || data.len() != 2 + present * ACTOR_BYTES {
        return None;
    }
    let mut actors: Snapshot = [None; MAX_PLAYERS];
    let mut at = 2;
    for (slot, entry) in actors.iter_mut().enumerate() {
        if mask & (1 << slot) == 0 {
            continue;
        }
        let mut actor = ActorSnapshot::default();
        {
            let mut number = || {
                let value = f32::from_le_bytes(data[at..at + 4].try_into().unwrap());
                at += 4;
                value
            };
            actor.at = Vec2::new(number(), number());
            actor.velocity = Vec2::new(number(), number());
            actor.facing = number();
        }
        actor.hp = i32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        at += 4;
        *entry = Some(actor);
    }
    Some(actors)
}

fn encode_weapons(state: &WeaponState) -> Vec<u8> {
    let mask = mask_of(&state.held);
    let mut out = Vec::with_capacity(3 + state.ground.len() * GROUND_BYTES);
    out.push(PACKET_WEAPONS);
    out.push(mask);
    for held in state.held.iter().flatten() {
        out.push(held.kind);
        out.extend((held.ammo.min(u16::MAX as u32) as u16).to_le_bytes());
    }
    out.push(state.ground.len() as u8);
    for weapon in &state.ground {
        out.extend(weapon.net.to_le_bytes());
        out.push(weapon.kind);
        out.extend((weapon.ammo.min(u16::MAX as u32) as u16).to_le_bytes());
        for value in [
            weapon.at.x,
            weapon.at.y,
            weapon.velocity.x,
            weapon.velocity.y,
        ] {
            out.extend(value.to_le_bytes());
        }
        out.push(weapon.thrown as u8);
    }
    out
}

fn decode_weapons(data: &[u8]) -> Option<WeaponState> {
    if data.first() != Some(&PACKET_WEAPONS) {
        return None;
    }
    let mask = *data.get(1)?;
    let present = present(mask)?;
    let mut at = 2;
    let mut state = WeaponState::default();
    for (slot, entry) in state.held.iter_mut().enumerate() {
        if mask & (1 << slot) == 0 {
            continue;
        }
        if data.len() < at + 3 {
            return None;
        }
        *entry = Some(HeldState {
            kind: data[at],
            ammo: u16::from_le_bytes(data[at + 1..at + 3].try_into().ok()?) as u32,
        });
        at += 3;
    }
    debug_assert_eq!(at, 2 + present * 3);

    let count = *data.get(at)? as usize;
    at += 1;
    if count > MAX_GROUND || data.len() != at + count * GROUND_BYTES {
        return None;
    }
    for _ in 0..count {
        let cell = &data[at..at + GROUND_BYTES];
        let number = |from: usize| f32::from_le_bytes(cell[from..from + 4].try_into().unwrap());
        state.ground.push(GroundState {
            net: u16::from_le_bytes(cell[0..2].try_into().ok()?),
            kind: cell[2],
            ammo: u16::from_le_bytes(cell[3..5].try_into().ok()?) as u32,
            at: Vec2::new(number(5), number(9)),
            velocity: Vec2::new(number(13), number(17)),
            thrown: cell[21] != 0,
        });
        at += GROUND_BYTES;
    }
    Some(state)
}

fn encode_round_over(result: &RoundResult) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + MAX_PLAYERS * 4);
    out.push(PACKET_ROUND_OVER);
    out.push(result.winner.unwrap_or(u8::MAX));
    out.push(result.rounds.min(u8::MAX as u32) as u8);
    out.push(result.players);
    for wins in result.score {
        out.extend(wins.to_le_bytes());
    }
    out
}

fn decode_round_over(data: &[u8]) -> Option<RoundResult> {
    if data.len() != 4 + MAX_PLAYERS * 4 || data[0] != PACKET_ROUND_OVER {
        return None;
    }
    let mut score = [0; MAX_PLAYERS];
    for (wins, chunk) in score.iter_mut().zip(data[4..].chunks_exact(4)) {
        *wins = u32::from_le_bytes(chunk.try_into().ok()?);
    }
    Some(RoundResult {
        winner: (data[1] < MAX_PLAYERS as u8).then_some(data[1]),
        rounds: data[2] as u32,
        players: data[3].clamp(MIN_PLAYERS as u8, MAX_PLAYERS as u8),
        score,
    })
}

