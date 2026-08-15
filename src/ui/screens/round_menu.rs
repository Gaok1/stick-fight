/// Rounds vencidos, desenhados como blocos cheios sobre os que faltam.
///
/// Numero exige leitura; bloco se conta de relance, que e o que se quer entre
/// um round e o proximo.
fn pips(wins: u32) -> String {
    (0..MATCH_WINS)
        .map(|i| if i < wins { '\u{2588}' } else { '\u{2591}' })
        .collect()
}

/// Placar de fim de round -- ou de fim de partida, quando alguem chega la.
pub(crate) fn spawn_round_over_screen(
    mut commands: Commands,
    result: Res<RoundResult>,
    mode: Res<GameMode>,
    pick: Res<LevelPick>,
    online: Res<OnlineSession>,
) {
    let champion = result.match_winner();

    let (title, color) = match (champion, result.winner) {
        (Some(id), _) => (format!("PLAYER {} TAKES IT", id + 1), palette::player(id)),
        (None, Some(id)) => (format!("PLAYER {} WINS", id + 1), palette::player(id)),
        // Ninguem de pe. Com quatro em campo "DOUBLE" mentiria sobre quantos
        // cairam juntos, entao a palavra some.
        (None, None) => ("EVERYBODY DIES".to_string(), palette::BONE),
    };

    commands.spawn((
        AsciiSprite::new(AsciiArt::banner(&title, '\u{2588}', color)),
        Layer::Hud,
        Transform::from_translation(Vec3::new(0.0, 130.0, 0.0)).with_scale(Vec3::splat(0.45)),
        DespawnOnExit(GameState::RoundOver),
    ));

    // Uma linha por lugar em jogo. Enquanto foi uma linha so com os dois lados
    // frente a frente, nao havia onde por o terceiro -- e mostrar os quatro
    // sempre encheria a tela de zeros numa briga de dois.
    let mut lines: Vec<String> = (0..result.seats())
        .map(|id| {
            format!(
                "  P{}  {}{}",
                id + 1,
                pips(result.score[id]),
                if result.winner == Some(id as u8) {
                    "  <"
                } else {
                    ""
                }
            )
        })
        .collect();
    lines.insert(0, format!("  MATCH   -   FIRST TO {MATCH_WINS}"));

    let progress = match champion {
        Some(_) => format!("  TOOK {MATCH_WINS} ROUNDS IN {}", result.rounds),
        None => format!("  ROUND {} NEXT", result.rounds + 1),
    };
    lines.extend([
        SEPARATOR.to_string(),
        progress,
        format!("  NEXT STAGE   {}", level_name(pick.0)),
    ]);

    commands.spawn((
        AsciiSprite::new(box_art(
            &lines.iter().map(String::as_str).collect::<Vec<_>>(),
        )),
        Layer::Hud,
        Transform::from_translation(Vec3::new(0.0, -20.0, 0.0)),
        DespawnOnExit(GameState::RoundOver),
    ));

    // Quem nao manda na partida ve o botao apagado em vez de nao ver botao
    // nenhum: e assim que fica claro que a espera e pelo dono, e nao um bug.
    let waiting = *mode == GameMode::Online && !online.is_host();
    let label = if champion.is_some() {
        "NEW MATCH"
    } else {
        "NEXT ROUND"
    };
    Button::new(if waiting { "WAITING HOST" } else { label }, MenuAction::NextRound)
        .width(12)
        .accent(palette::BONE)
        .enabled(!waiting)
        .spawn(
            &mut commands,
            GameState::RoundOver,
            Vec2::new(-120.0, -190.0),
        );
    Button::new("LEAVE", MenuAction::Back).width(12).spawn(
        &mut commands,
        GameState::RoundOver,
        Vec2::new(120.0, -190.0),
    );
}

// --- navegacao --------------------------------------------------------------

/// Traduz teclado em acao de tela.
fn keyboard_actions(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<GameState>>,
    mode: Res<GameMode>,
    session: Res<OnlineSession>,
    row: Res<FighterRow>,
    mut actions: MessageWriter<MenuAction>,
) {
    let enter = keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter]);
    let back = keys.just_pressed(KeyCode::Escape);
    let vertical = keys.any_just_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) as i32
        - keys.any_just_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) as i32;

    match state.get() {
        GameState::Controls => {
            let horizontal = keys.any_just_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) as i32
                - keys.any_just_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) as i32;
            if horizontal != 0 {
                actions.write(MenuAction::Stage(horizontal));
            }
            if vertical != 0 {
                let at = GameMode::ALL.iter().position(|m| *m == *mode).unwrap_or(0);
                actions.write(MenuAction::PickMode(
                    GameMode::ALL[cycle(at, vertical, GameMode::ALL.len())],
                ));
            }
            if enter {
                actions.write(MenuAction::Play);
            }
        }
        GameState::SkinSelect => {
            // Cada jogador anda no proprio par de teclas, mas a linha e uma so:
            // os dois escolhem a mesma peca ao mesmo tempo, e a tela nao precisa
            // de dois cursores disputando espaco.
            let p1 = keys.just_pressed(KeyCode::KeyD) as i32
                - keys.just_pressed(KeyCode::KeyA) as i32;
            let p2 = keys.just_pressed(KeyCode::ArrowRight) as i32
                - keys.just_pressed(KeyCode::ArrowLeft) as i32;
            if vertical != 0 {
                actions.write(MenuAction::Fighter {
                    seat: 0,
                    row: cycle(row.0, vertical, fighter_rows()),
                    step: 0,
                });
            }
            let solo = fighter_seats(*mode) == 1;
            let first = if solo && p1 == 0 { p2 } else { p1 };
            if first != 0 {
                actions.write(MenuAction::Fighter {
                    seat: 0,
                    row: row.0,
                    step: first,
                });
            }
            if !solo && p2 != 0 {
                actions.write(MenuAction::Fighter {
                    seat: 1,
                    row: row.0,
                    step: p2,
                });
            }
            if enter {
                actions.write(MenuAction::Confirm);
            }
            if back {
                actions.write(MenuAction::Back);
            }
        }
        GameState::Lobby => {
            if enter {
                actions.write(MenuAction::Room(if session.in_lobby() {
                    LobbyCommand::Start
                } else {
                    LobbyCommand::Create
                }));
            }
            if keys.just_pressed(KeyCode::KeyF) {
                actions.write(MenuAction::Room(LobbyCommand::Find));
            }
            if keys.just_pressed(KeyCode::KeyI) {
                actions.write(MenuAction::Room(LobbyCommand::Invite));
            }
            if back {
                actions.write(MenuAction::Back);
            }
        }
        GameState::RoundOver => {
            if enter {
                actions.write(MenuAction::NextRound);
            }
            if back {
                actions.write(MenuAction::Back);
            }
        }
        GameState::Fighting => {
            if back {
                actions.write(MenuAction::Back);
            }
        }
    }
}

/// Faz o que a tela pediu. Um lugar so, para mouse e teclado nao divergirem.
fn apply_menu_action(
    mut actions: MessageReader<MenuAction>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
    mut mode: ResMut<GameMode>,
    mut pick: ResMut<LevelPick>,
    mut picks: ResMut<SkinSelections>,
    mut row: ResMut<FighterRow>,
    mut room: MessageWriter<LobbyCommand>,
) {
    for action in actions.read() {
        match *action {
            MenuAction::PickMode(chosen) => *mode = chosen,
            MenuAction::Stage(step) => pick.0 = cycle(pick.0, step, LEVEL_CATALOG.len()),
            MenuAction::Play => next.set(GameState::SkinSelect),
            MenuAction::Confirm => next.set(if *mode == GameMode::Online {
                GameState::Lobby
            } else {
                GameState::Fighting
            }),
            MenuAction::Fighter {
                seat,
                row: line,
                step,
            } => {
                row.0 = line % fighter_rows();
                if step != 0 {
                    let solo = *mode == GameMode::Online;
                    let (mut chosen, mut face) = seat_choice(&picks, *mode, seat);
                    row_cycle(line, &mut chosen, &mut face, step);
                    if solo {
                        picks.online_local = chosen;
                        picks.online_face = face;
                    } else {
                        picks.players[seat as usize] = chosen;
                        picks.faces[seat as usize] = face;
                    }
                }
            }
            MenuAction::Room(command) => {
                room.write(command);
            }
            MenuAction::NextRound => {
                if *mode == GameMode::Online {
                    room.write(LobbyCommand::Start);
                } else {
                    next.set(GameState::Fighting);
                }
            }
            MenuAction::Back => match state.get() {
                GameState::SkinSelect => next.set(GameState::Controls),
                GameState::Lobby => {
                    room.write(LobbyCommand::Leave);
                }
                GameState::Fighting | GameState::RoundOver => {
                    next.set(if *mode == GameMode::Online {
                        GameState::Lobby
                    } else {
                        GameState::Controls
                    })
                }
                GameState::Controls => {}
            },
        }
    }
}

// --- HUD --------------------------------------------------------------------

