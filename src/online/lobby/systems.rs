/// Executa o que o menu pediu.
fn run_lobby_commands(
    mut commands: MessageReader<LobbyCommand>,
    runtime: Option<Res<SteamRuntime>>,
    mut session: ResMut<OnlineSession>,
    pick: Res<LevelPick>,
    skins: Res<SkinSelections>,
    mut next: ResMut<NextState<GameState>>,
) {
    let Some(runtime) = runtime else {
        // Sem Steam nao ha o que fazer, mas sair tem que continuar saindo.
        for command in commands.read() {
            if *command == LobbyCommand::Leave {
                next.set(GameState::Controls);
            }
        }
        return;
    };

    for command in commands.read() {
        match command {
            LobbyCommand::Create if session.lobby.is_none() => {
                create_lobby(&runtime, &mut session)
            }
            LobbyCommand::Find if session.lobby.is_none() => find_lobby(&runtime, &mut session),
            LobbyCommand::Invite => {
                if let Some(lobby) = session.lobby {
                    runtime.client.friends().activate_invite_dialog(lobby);
                }
            }
            // Basta ter com quem lutar. Esperar a pele de todo mundo chegar
            // deixaria a sala travada por um pacote perdido, e o pacote de
            // inicio ja carrega a tabela inteira de peles de qualquer jeito.
            LobbyCommand::Start if session.can_start() => {
                if let Some(lobby) = session.lobby {
                    runtime
                        .client
                        .matchmaking()
                        .set_lobby_joinable(lobby, false);
                }
                send_start(&runtime, &mut session, pick.0, &skins);
                next.set(GameState::Fighting);
            }
            LobbyCommand::Leave => next.set(GameState::Controls),
            _ => {}
        }
    }
}

/// O dono publica a fase para a sala aquecer no mesmo mapa.
fn publish_stage(
    runtime: Option<Res<SteamRuntime>>,
    session: Res<OnlineSession>,
    pick: Res<LevelPick>,
    mut published: Local<Option<usize>>,
) {
    let Some(runtime) = runtime else {
        return;
    };
    let (true, Some(lobby)) = (session.host, session.lobby) else {
        return;
    };
    if *published == Some(pick.0) {
        return;
    }
    *published = Some(pick.0);
    runtime
        .client
        .matchmaking()
        .set_lobby_data(lobby, STAGE_KEY, &pick.0.to_string());
}

/// O cliente segue a fase do dono enquanto espera na sala.
///
/// Sem isto cada um aquecia num mapa diferente: os bonecos apareciam parados no
/// ar, porque o chao onde o dono os via nao existia na tela de quem entrou.
fn follow_host_stage(session: Res<OnlineSession>, mut pick: ResMut<LevelPick>) {
    if session.host {
        return;
    }
    if let Some(stage) = session.stage
        && pick.0 != stage
    {
        pick.0 = stage;
    }
}

fn reopen_lobby(runtime: Option<Res<SteamRuntime>>, session: Res<OnlineSession>) {
    let Some(runtime) = runtime else {
        return;
    };
    if session.host {
        if let Some(lobby) = session.lobby {
            runtime.client.matchmaking().set_lobby_joinable(lobby, true);
        }
    }
}

fn round_over_controls(
    keys: Res<ButtonInput<KeyCode>>,
    runtime: Option<Res<SteamRuntime>>,
    mut session: ResMut<OnlineSession>,
    pick: Res<LevelPick>,
    skins: Res<SkinSelections>,
    mut next: ResMut<NextState<GameState>>,
) {
    // A mesma exigencia do lobby: sem com quem lutar, o round seguinte abriria
    // com um lugar vazio que morre no ato e cairia direto nesta tela de novo.
    if !session.can_start()
        || !(keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter))
    {
        return;
    }
    let Some(runtime) = runtime else {
        return;
    };
    send_start(&runtime, &mut session, pick.0, &skins);
    next.set(GameState::Fighting);
}

/// Abre a luta: congela quantos lugares valem e conta a cada cliente qual e o
/// dele.
///
/// O lugar vai no pacote em vez de sair da tabela do lobby porque a replicacao
/// da tabela e assincrona: um cliente que acabou de entrar poderia comecar a
/// luta sem saber ainda quem ele e.
fn send_start(
    runtime: &SteamRuntime,
    session: &mut OnlineSession,
    stage: usize,
    skins: &SkinSelections,
) {
    session.clear_remotes();
    session.table_presses = [[0; PULSES]; MAX_PLAYERS];
    session.seats = session.span().max(MIN_PLAYERS) as u8;
    let seats = session.seats;
    let mut looks = [Look::default(); MAX_PLAYERS];
    for (at, look) in looks.iter_mut().enumerate() {
        *look = Look {
            skin: skins.players[at],
            face: skins.faces[at],
        };
    }
    for slot in 0..MAX_PLAYERS {
        let Some(peer) = session.slots[slot] else {
            continue;
        };
        if Some(peer) == session.me {
            continue;
        }
        send(
            runtime,
            peer,
            &encode_start(stage, seats, slot as u8, looks),
            SendFlags::RELIABLE_NO_NAGLE,
        );
    }
}

