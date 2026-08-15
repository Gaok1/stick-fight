/// Cabecalho do pacote de inicio: tipo, fase, lugares e o lugar do destinatario.
const START_HEAD: usize = 4;
/// Bytes de aparencia por lugar: a pele mais as pecas do rosto.
const LOOK_BYTES: usize = 1 + FACE_BYTES;
/// Tamanho do pacote de inicio.
const START_BYTES: usize = START_HEAD + MAX_PLAYERS * LOOK_BYTES;

/// A aparencia escolhida por um lugar: pele e rosto viajam juntos porque sao a
/// mesma pergunta, e separa-los deixaria um chegar sem o outro.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Look {
    skin: usize,
    face: Face,
}

fn encode_start(
    stage: usize,
    seats: u8,
    slot: u8,
    looks: [Look; MAX_PLAYERS],
) -> [u8; START_BYTES] {
    let mut out = [0; START_BYTES];
    out[0] = PACKET_START;
    out[1] = stage as u8;
    out[2] = seats;
    out[3] = slot;
    for (at, look) in looks.iter().enumerate() {
        let cell = &mut out[START_HEAD + at * LOOK_BYTES..][..LOOK_BYTES];
        cell[0] = look.skin as u8;
        cell[1..].copy_from_slice(&look.face.to_bytes());
    }
    out
}

fn decode_start(data: &[u8]) -> Option<(usize, u8, u8, [Look; MAX_PLAYERS])> {
    if data.len() < START_BYTES || data[0] != PACKET_START {
        return None;
    }
    let mut looks = [Look::default(); MAX_PLAYERS];
    for (at, look) in looks.iter_mut().enumerate() {
        let cell = &data[START_HEAD + at * LOOK_BYTES..][..LOOK_BYTES];
        look.skin = cell[0] as usize % skin::CATALOG.len();
        look.face = Face::from_bytes(&cell[1..]);
    }
    Some((
        data[1] as usize,
        (data[2] as usize).clamp(MIN_PLAYERS, MAX_PLAYERS) as u8,
        data[3].min(MAX_PLAYERS as u8 - 1),
        looks,
    ))
}

fn receive_packets(
    runtime: Option<Res<SteamRuntime>>,
    mut session: ResMut<OnlineSession>,
    mut pick: ResMut<LevelPick>,
    mut result: ResMut<RoundResult>,
    mut skins: ResMut<SkinSelections>,
    mut next: ResMut<NextState<GameState>>,
) {
    let Some(runtime) = runtime else {
        return;
    };
    for message in runtime
        .client
        .networking_messages()
        .receive_messages_on_channel(CHANNEL, 64)
    {
        let Some(sender) = message.identity_peer().steam_id() else {
            continue;
        };
        // Quem nao esta na sala nao fala. O dono e aceito mesmo sem lugar
        // conhecido: a tabela dele e que manda, e a copia deste cliente pode
        // estar um instante atrasada quando alguem acabou de entrar.
        let slot = session.slot_of(sender);
        let from_owner = session.owner == Some(sender);
        if slot.is_none() && !from_owner {
            continue;
        }

        let data = message.data();
        match data.first().copied() {
            Some(PACKET_INPUT) if session.host => {
                if let (Some(slot), Some((intent, presses))) =
                    (slot, decode_intent(data.get(1..).unwrap_or_default()))
                {
                    session.remotes[slot as usize].push(intent, presses);
                }
            }
            Some(PACKET_INPUTS) if !session.host => {
                apply_input_table(&session, data);
            }
            Some(PACKET_START) if !session.host => {
                let Some((stage, seats, slot, selected)) = decode_start(data) else {
                    continue;
                };
                session.clear_remotes();
                session.seats = seats;
                session.local = slot;
                session.seated = true;
                session.pending_weapons = None;
                session.pending_snapshot = None;
                pick.0 = stage % LEVEL_CATALOG.len();
                for (at, look) in selected.iter().enumerate() {
                    skins.players[at] = look.skin;
                    skins.faces[at] = look.face;
                }
                next.set(GameState::Fighting);
            }
            Some(PACKET_SNAPSHOT) if !session.host => {
                session.pending_snapshot = decode_snapshot(data);
            }
            Some(PACKET_WEAPONS) if !session.host => {
                session.pending_weapons = decode_weapons(data);
            }
            Some(PACKET_ROUND_OVER) if !session.host => {
                if let Some(decoded) = decode_round_over(data) {
                    *result = decoded;
                    next.set(GameState::RoundOver);
                }
            }
            Some(PACKET_SKIN) if session.host && data.len() > LOOK_BYTES => {
                if let Some(slot) = slot {
                    let chosen = Look {
                        skin: data[1] as usize % skin::CATALOG.len(),
                        face: Face::from_bytes(&data[2..]),
                    };
                    session.remote_looks[slot as usize] = Some(chosen);
                    skins.players[slot as usize] = chosen.skin;
                    skins.faces[slot as usize] = chosen.face;
                }
            }
            _ => {}
        }
    }
}

/// Publica a escolha local para o dono e aplica o que ja se sabe dos outros.
///
/// So o dono junta as escolhas; ele as devolve para todos no pacote de inicio.
/// Assim ninguem precisa falar com ninguem alem dele.
fn sync_skin_choice(
    runtime: Option<Res<SteamRuntime>>,
    mut session: ResMut<OnlineSession>,
    mut skins: ResMut<SkinSelections>,
) {
    let local = session.local_player_id() as usize;
    let selected = Look {
        skin: skins.online_local % skin::CATALOG.len(),
        face: skins.online_face,
    };
    skins.players[local] = selected.skin;
    skins.faces[local] = selected.face;
    for (slot, known) in session.remote_looks.iter().enumerate() {
        if slot != local
            && let Some(known) = known
        {
            skins.players[slot] = known.skin;
            skins.faces[slot] = known.face;
        }
    }

    if session.host {
        return;
    }
    let (Some(runtime), Some(owner)) = (runtime, session.owner) else {
        return;
    };
    if session.sent_look == Some(selected) {
        return;
    }
    let mut packet = [0; 1 + LOOK_BYTES];
    packet[0] = PACKET_SKIN;
    packet[1] = selected.skin as u8;
    packet[2..].copy_from_slice(&selected.face.to_bytes());
    send(&runtime, owner, &packet, SendFlags::RELIABLE_NO_NAGLE);
    session.sent_look = Some(selected);
}

/// Quem saiu da sala para de lutar.
///
/// Zerar a vida em vez de tirar o boneco da arena faz o round terminar pela
/// regra de sempre -- sobrou um -- sem precisar de um caso especial para
/// desistencia.
fn retire_missing_players(session: Res<OnlineSession>, mut players: Query<(&Player, &mut Health)>) {
    if !session.host {
        return;
    }
    for (player, mut health) in &mut players {
        if session.slots[player.id as usize].is_none() && !health.is_dead() {
            health.hp = 0;
        }
    }
}

