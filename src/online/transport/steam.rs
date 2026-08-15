#[derive(Resource)]
struct SteamRuntime {
    client: Client,
    tx: mpsc::Sender<SteamEvent>,
    rx: Mutex<mpsc::Receiver<SteamEvent>>,
    _callbacks: Vec<CallbackHandle>,
}

fn init_steam(mut commands: Commands) {
    let Ok(client) = Client::init() else {
        commands.insert_resource(OnlineSession::default());
        return;
    };

    let (tx, rx) = mpsc::channel();
    let invite_tx = tx.clone();
    let invite = client.register_callback(move |event: GameLobbyJoinRequested| {
        let _ = invite_tx.send(SteamEvent::JoinRequested(event.lobby_steam_id));
    });
    let members_tx = tx.clone();
    let members = client.register_callback(move |event: LobbyChatUpdate| {
        let _ = members_tx.send(SteamEvent::RoomChanged(event.lobby));
    });
    // A tabela de lugares e a fase moram nos dados da sala, e mudar dados nao
    // dispara `LobbyChatUpdate`. Sem este callback, quem entrava so descobria o
    // proprio lugar quando a **proxima** pessoa entrasse ou saisse -- ate la
    // jogava no boneco de outro, sem mapa e sem placar.
    let data_tx = tx.clone();
    let data = client.register_callback(move |event: LobbyDataUpdate| {
        if event.success {
            let _ = data_tx.send(SteamEvent::RoomChanged(event.lobby));
        }
    });

    client
        .networking_messages()
        .session_request_callback(|request| {
            request.accept();
        });

    let name = client.friends().name();
    commands.insert_resource(OnlineSession {
        status: format!("CONNECTED TO STEAM AS {name}"),
        ..default()
    });
    commands.insert_resource(SteamRuntime {
        client,
        tx,
        rx: Mutex::new(rx),
        _callbacks: vec![invite, members, data],
    });
}

fn pump_callbacks(runtime: Option<Res<SteamRuntime>>) {
    if let Some(runtime) = runtime {
        runtime.client.run_callbacks();
    }
}

fn create_lobby(runtime: &SteamRuntime, session: &mut OnlineSession) {
    session.status = "CREATING PUBLIC LOBBY...".into();
    let tx = runtime.tx.clone();
    runtime.client.matchmaking().create_lobby(
        LobbyType::Public,
        MAX_PLAYERS as u32,
        move |result| {
            let _ = tx.send(SteamEvent::Created(
                result.map_err(|error| format!("{error:?}")),
            ));
        },
    );
}

fn join_lobby(runtime: &SteamRuntime, session: &mut OnlineSession, lobby: LobbyId) {
    if let Some(current) = session.lobby.take() {
        runtime.client.matchmaking().leave_lobby(current);
    }
    session.forget_room();
    session.status = "JOINING LOBBY...".into();
    let tx = runtime.tx.clone();
    runtime
        .client
        .matchmaking()
        .join_lobby(lobby, move |result| {
            let _ = tx.send(SteamEvent::Joined(
                result.map_err(|_| "LOBBY REFUSED THE CONNECTION".into()),
            ));
        });
}

fn find_lobby(runtime: &SteamRuntime, session: &mut OnlineSession) {
    session.status = "SEARCHING OPEN LOBBIES...".into();
    let tx = runtime.tx.clone();
    runtime
        .client
        .matchmaking()
        .add_request_lobby_list_string_filter(StringFilter(
            LobbyKey::new("game"),
            GAME_TAG,
            StringFilterKind::Equal,
        ))
        .set_request_lobby_list_slots_available_filter(1)
        .request_lobby_list(move |result| {
            let _ = tx.send(SteamEvent::Found(
                result.map_err(|error| format!("{error:?}")),
            ));
        });
}

/// Nome da chave onde a tabela de lugares e publicada.
fn slot_key(at: usize) -> String {
    format!("slot{at}")
}

/// Chave da fase escolhida pelo dono.
const STAGE_KEY: &str = "stage";

/// Encaixa os membros da sala na tabela de lugares.
///
/// Quem ja tem um lugar fica nele. Compactar a tabela a cada saida
/// renumeraria os sobreviventes no meio da luta -- trocaria a cor, a linha do
/// placar e o boneco de quem nao fez nada -- e faria a entrada de saida
/// apontar para o alvo errado.
fn assign_slots(
    current: [Option<SteamId>; MAX_PLAYERS],
    owner: SteamId,
    members: &[SteamId],
) -> [Option<SteamId>; MAX_PLAYERS] {
    let mut slots = current;

    // Quem saiu abre o lugar.
    for slot in slots.iter_mut() {
        if !slot.is_some_and(|who| members.contains(&who)) {
            *slot = None;
        }
    }
    // O primeiro lugar e do dono: e dele que sai a autoridade da partida.
    if slots[0] != Some(owner) {
        for slot in slots.iter_mut() {
            if *slot == Some(owner) {
                *slot = None;
            }
        }
        slots[0] = Some(owner);
    }
    // Quem chegou ocupa a primeira vaga livre.
    let newcomers: Vec<SteamId> = members
        .iter()
        .copied()
        .filter(|member| !slots.contains(&Some(*member)))
        .collect();
    for member in newcomers {
        let Some(free) = slots.iter_mut().find(|slot| slot.is_none()) else {
            break;
        };
        *free = Some(member);
    }
    slots
}

fn refresh_members(runtime: &SteamRuntime, session: &mut OnlineSession) {
    let Some(lobby) = session.lobby else {
        return;
    };
    let matchmaking = runtime.client.matchmaking();
    let me = runtime.client.user().steam_id();
    let owner = matchmaking.lobby_owner(lobby);
    let before = session.slots;

    session.me = Some(me);
    session.owner = Some(owner);
    session.host = owner == me;

    // O dono decide os lugares e publica a tabela; a Steam a replica para
    // todos. Se cada cliente ordenasse `lobby_members` por conta propria, dois
    // deles poderiam discordar de quem e o jogador 3 -- essa ordem nao e
    // prometida igual em todo mundo.
    if session.host {
        let members = matchmaking.lobby_members(lobby);
        let slots = assign_slots(session.slots, owner, &members);
        session.slots = slots;
        for (at, slot) in slots.iter().enumerate() {
            matchmaking.set_lobby_data(
                lobby,
                &slot_key(at),
                &slot.map_or(String::new(), |who| who.raw().to_string()),
            );
        }
    } else {
        for at in 0..MAX_PLAYERS {
            session.slots[at] = matchmaking
                .lobby_data(lobby, &slot_key(at))
                .and_then(|raw| raw.parse::<u64>().ok())
                .map(SteamId::from_raw);
        }
        session.stage = matchmaking
            .lobby_data(lobby, STAGE_KEY)
            .and_then(|raw| raw.parse::<usize>().ok())
            .map(|stage| stage % LEVEL_CATALOG.len());
    }

    // So mexe no proprio lugar quando a tabela realmente diz qual e. Cair no
    // lugar zero enquanto a copia do cliente ainda nao replicou o poria no
    // comando do boneco do dono por alguns quadros.
    if let Some(slot) = session.slot_of(me) {
        session.local = slot;
        session.seated = true;
    }

    // Quem sai leva a escolha dele junto: reaproveitar a pele anterior
    // pintaria o proximo a ocupar o lugar com o gosto de outra pessoa.
    for (slot, pick) in session.remote_looks.iter_mut().enumerate() {
        if before[slot] != session.slots[slot] {
            *pick = None;
        }
    }
    for (slot, remote) in session.remotes.iter().enumerate() {
        if session.slots[slot].is_none() {
            remote.clear();
        }
    }
    if before != session.slots {
        session.sent_look = None;
    }

    session.members = session
        .slots
        .iter()
        .map(|slot| {
            slot.map_or_else(
                || "---".to_string(),
                |who| runtime.client.friends().get_friend(who).name(),
            )
        })
        .collect();

    let filled = session.filled();
    session.status = if !session.seated {
        "TAKING A SEAT IN THE ROOM...".into()
    } else if filled < MIN_PLAYERS {
        "WAITING FOR PLAYERS. INVITE A FRIEND.".into()
    } else if session.host {
        format!("{filled}/{MAX_PLAYERS} IN THE ROOM. READY WHEN YOU ARE.")
    } else {
        format!("{filled}/{MAX_PLAYERS} IN THE ROOM. WAITING FOR THE HOST...")
    };
}

/// Rele a sala de tempos em tempos.
///
/// Callback nao e garantia de ordem: o aviso de que alguem entrou pode chegar
/// antes de a tabela de lugares desse alguem ter sido publicada. Esta releitura
/// e barata (le memoria local da Steam) e fecha essa janela.
fn poll_lobby(
    time: Res<Time>,
    runtime: Option<Res<SteamRuntime>>,
    mut session: ResMut<OnlineSession>,
    mut since: Local<f32>,
) {
    let Some(runtime) = runtime else {
        return;
    };
    if session.lobby.is_none() {
        return;
    }
    *since += time.delta_secs();
    if *since < LOBBY_POLL {
        return;
    }
    *since = 0.0;
    refresh_members(&runtime, &mut session);
}

fn handle_events(
    runtime: Option<Res<SteamRuntime>>,
    mut session: ResMut<OnlineSession>,
    mut mode: ResMut<GameMode>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
) {
    let Some(runtime) = runtime else {
        return;
    };
    let events: Vec<_> = runtime
        .rx
        .lock()
        .expect("steam event receiver poisoned")
        .try_iter()
        .collect();

    for event in events {
        match event {
            SteamEvent::Created(Ok(lobby)) => {
                let matchmaking = runtime.client.matchmaking();
                matchmaking.set_lobby_data(lobby, "game", GAME_TAG);
                session.lobby = Some(lobby);
                refresh_members(&runtime, &mut session);
            }
            SteamEvent::Joined(Ok(lobby)) => {
                if runtime
                    .client
                    .matchmaking()
                    .lobby_data(lobby, "game")
                    .as_deref()
                    != Some(GAME_TAG)
                {
                    runtime.client.matchmaking().leave_lobby(lobby);
                    session.status = "THIS LOBBY BELONGS TO ANOTHER GAME/BUILD.".into();
                    continue;
                }
                *mode = GameMode::Online;
                session.lobby = Some(lobby);
                refresh_members(&runtime, &mut session);
                if *state.get() != GameState::Lobby {
                    next.set(GameState::SkinSelect);
                }
            }
            SteamEvent::Found(Ok(lobbies)) => {
                let found = lobbies.into_iter().find(|lobby| {
                    runtime
                        .client
                        .matchmaking()
                        .lobby_data(*lobby, "game")
                        .as_deref()
                        == Some(GAME_TAG)
                        && runtime.client.matchmaking().lobby_member_count(*lobby) < MAX_PLAYERS
                });
                if let Some(lobby) = found {
                    join_lobby(&runtime, &mut session, lobby);
                } else {
                    session.status = "NO OPEN LOBBY FOUND. CREATE ONE.".into();
                }
            }
            SteamEvent::JoinRequested(lobby) => join_lobby(&runtime, &mut session, lobby),
            SteamEvent::RoomChanged(lobby) if session.lobby == Some(lobby) => {
                let owner_before = session.owner;
                refresh_members(&runtime, &mut session);

                // Quem sai no meio da luta so conta como derrotado -- os outros
                // continuam brigando. Mas sem o dono nao ha quem decida o round,
                // e a Steam promove um novo dono que nao tem o estado da
                // partida: nesse caso a luta acaba para todo mundo.
                let fighting = matches!(state.get(), GameState::Fighting | GameState::RoundOver);
                let lost_host = owner_before.is_some() && session.owner != owner_before;
                if fighting && (session.filled() < MIN_PLAYERS || lost_host) {
                    next.set(GameState::Lobby);
                }
            }
            SteamEvent::Created(Err(error))
            | SteamEvent::Joined(Err(error))
            | SteamEvent::Found(Err(error)) => session.status = format!("STEAM ERROR: {error}"),
            SteamEvent::RoomChanged(_) => {}
        }
    }
}

fn leave_lobby(runtime: Option<Res<SteamRuntime>>, mut session: ResMut<OnlineSession>) {
    let Some(runtime) = runtime else {
        return;
    };
    if let Some(lobby) = session.lobby.take() {
        runtime.client.matchmaking().leave_lobby(lobby);
    }
    session.forget_room();
    session.status = format!("CONNECTED TO STEAM AS {}", runtime.client.friends().name());
}

fn send(runtime: &SteamRuntime, peer: SteamId, bytes: &[u8], flags: SendFlags) {
    if let Err(error) = runtime.client.networking_messages().send_message_to_user(
        NetworkingIdentity::new_steam_id(peer),
        flags,
        bytes,
        CHANNEL,
    ) {
        warn!("Steam P2P send failed: {error:?}");
    }
}

/// Manda o mesmo pacote para todos os outros da sala.
fn broadcast(runtime: &SteamRuntime, session: &OnlineSession, bytes: &[u8], flags: SendFlags) {
    for peer in session.peers() {
        send(runtime, peer, bytes, flags);
    }
}

/// Ja passou o intervalo de envio deste canal?
///
/// Um acumulador, e nao um "resetar para zero": com reset, um quadro longo
/// atrasaria o envio seguinte e o ritmo escorregaria junto com o framerate.
fn due(since: &mut f32, dt: f32, hz: f32) -> bool {
    *since += dt;
    let step = 1.0 / hz;
    if *since < step {
        return false;
    }
    // Descontar em vez de zerar mantem a media no ritmo pedido: a sobra de um
    // quadro curto entra no proximo em vez de ser perdida.
    //
    // Sobra de um passo inteiro ou mais nao e ritmo, e divida de uma pausa -- e
    // ela e jogada fora. O teto anterior era `min(step)`, que deixava `since`
    // exatamente em um passo: o envio seguinte saia no mesmo instante, e o
    // seguinte, e o outro. Um congelamento (carregar a fase, alt-tab) voltava
    // como uma rajada de pacotes de uma vez so, que e justamente o que o
    // acumulador existia para evitar.
    let carry = *since - step;
    *since = if carry < step { carry } else { 0.0 };
    true
}

