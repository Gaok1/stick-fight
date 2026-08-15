#[derive(Component)]
struct LobbyPanel;

/// Painel da sala: quem esta dentro e o que o jogo esta esperando.
///
/// Ele mora encostado num canto porque a arena esta viva atras: o lobby e
/// jogavel, e um painel no meio da tela ficaria em cima do lugar onde se
/// briga enquanto os amigos nao chegam.
fn lobby_art(session: &OnlineSession) -> AsciiArt {
    let lobby_id = session
        .lobby
        .map(|lobby| lobby.raw().to_string())
        .unwrap_or_else(|| "NONE".into());
    let local = session.local_player_id();

    let mut lines = vec![
        format!("STEAM ONLINE      {lobby_id}"),
        SEPARATOR.to_string(),
        format!("{:<44}", session.status),
        SEPARATOR.to_string(),
    ];

    // Uma linha por lugar, cheio ou nao: ver a vaga aberta e o que diz que
    // ainda cabe gente, e por isso a sala mostra os quatro mesmo com dois.
    let roster = lines.len();
    lines.extend((0..MAX_PLAYERS).map(|slot| {
        let who = session
            .members
            .get(slot)
            .map(String::as_str)
            .unwrap_or("---");
        let you = if slot as u8 == local && session.seated() {
            RIGHT
        } else {
            " "
        };
        let host = if slot == 0 { "HOST" } else { "    " };
        format!("{you} P{}  {:<28} {host}", slot + 1, who)
    }));

    lines.extend([
        SEPARATOR.to_string(),
        "  WARM UP WHILE YOU WAIT".to_string(),
    ]);

    let text: Vec<&str> = lines.iter().map(String::as_str).collect();
    let mut art = box_art(&text);
    // Cada lugar leva a cor do boneco dele -- a mesma que esta correndo na
    // arena atras do painel. E o que liga um nome na lista a um boneco na tela.
    for slot in 0..MAX_PLAYERS {
        art = art.stamp(
            &AsciiArt::solid(&format!("P{}", slot + 1), palette::player(slot as u8)),
            4,
            (roster + slot + 1) as u16,
        );
    }
    art = highlight(art, &text, RIGHT, palette::GOLD);
    highlight(art, &text, "WARM UP WHILE YOU WAIT", palette::MOSS)
}

/// Os botoes da sala, de cima para baixo.
///
/// A lista e a mesma sempre; o que muda e quem esta aceso. Uma sala que
/// esconde botoes conforme o estado faz a fileira dancar, e o jogador clica no
/// lugar onde o botao estava um instante atras.
fn lobby_buttons(session: &OnlineSession) -> [(MenuAction, &'static str, bool); 5] {
    let idle = !session.in_lobby();
    [
        (
            MenuAction::Room(LobbyCommand::Create),
            "CREATE ROOM",
            idle,
        ),
        (MenuAction::Room(LobbyCommand::Find), "FIND ROOM", idle),
        (
            MenuAction::Room(LobbyCommand::Invite),
            "INVITE FRIEND",
            session.in_lobby(),
        ),
        (
            MenuAction::Room(LobbyCommand::Start),
            "START MATCH",
            session.can_start(),
        ),
        (MenuAction::Room(LobbyCommand::Leave), "LEAVE", true),
    ]
}

/// Onde o botao `at` da sala fica.
fn lobby_slot(at: usize) -> Vec2 {
    Vec2::new(430.0, 300.0 - at as f32 * 34.0)
}

fn spawn_lobby_screen(mut commands: Commands, session: Res<OnlineSession>, pick: Res<LevelPick>) {
    commands.spawn((
        LobbyPanel,
        // Ancorado pelo canto superior esquerdo: a caixa cresce para dentro da
        // tela conforme o status muda de tamanho, em vez de escorregar de lado
        // a cada palavra -- e desce a partir do topo, deixando livre a faixa do
        // chao, que e onde os bonecos ficam.
        AsciiSprite::pivoted(lobby_art(&session), Vec2::new(-0.5, 0.5)),
        Layer::Hud,
        Transform::from_translation(Vec3::new(-628.0, 300.0, 0.0)),
        DespawnOnExit(GameState::Lobby),
    ));

    for (at, (action, label, enabled)) in lobby_buttons(&session).into_iter().enumerate() {
        Button::new(label, action)
            .width(13)
            .enabled(enabled)
            .spawn(&mut commands, GameState::Lobby, lobby_slot(at));
    }

    // A fase e do dono: quem entrou ve qual e, e so ele a troca.
    let stage_y = lobby_slot(lobby_buttons(&session).len()).y - 12.0;
    Button::new(LEFT, MenuAction::Stage(-1))
        .enabled(session.is_host())
        .spawn(
            &mut commands,
            GameState::Lobby,
            Vec2::new(430.0 - widest_stage() as f32 * 4.0 - 24.0, stage_y),
        );
    Button::new(level_name(pick.0), MenuAction::Stage(1))
        .width(widest_stage())
        .chosen(true)
        .enabled(session.is_host())
        .spawn(&mut commands, GameState::Lobby, Vec2::new(430.0, stage_y));
    Button::new(RIGHT, MenuAction::Stage(1))
        .enabled(session.is_host())
        .spawn(
            &mut commands,
            GameState::Lobby,
            Vec2::new(430.0 + widest_stage() as f32 * 4.0 + 24.0, stage_y),
        );
}

fn update_lobby_screen(
    session: Res<OnlineSession>,
    pick: Res<LevelPick>,
    mut panels: Query<&mut AsciiSprite, (With<LobbyPanel>, Without<Button>)>,
    mut buttons: Query<(&mut Button, &mut AsciiSprite)>,
) {
    for mut panel in &mut panels {
        let art = lobby_art(&session);
        if panel.art != art {
            panel.art = art;
        }
    }
    let states = lobby_buttons(&session);
    for (mut button, mut sprite) in &mut buttons {
        let next = match button.action {
            MenuAction::Room(_) => {
                let Some((action, label, enabled)) =
                    states.iter().find(|(action, ..)| *action == button.action)
                else {
                    continue;
                };
                Button::new(label, *action).width(13).enabled(*enabled)
            }
            MenuAction::Stage(1) if button.width > 1 => {
                Button::new(level_name(pick.0), button.action)
                    .width(widest_stage())
                    .chosen(true)
                    .enabled(session.is_host())
            }
            MenuAction::Stage(_) => Button::new(&button.label.clone(), button.action)
                .enabled(session.is_host()),
            _ => continue,
        };
        restyle(&mut button, &mut sprite, next);
    }
}

// --- escolha de lutador -----------------------------------------------------

