/// Painel e atores da tela de escolha cosmetica.
#[derive(Component)]
struct SkinPanel;

#[derive(Component)]
struct SkinPreview(u8);

/// Linha destacada no seletor de lutador.
///
/// Zero e a pele; o resto sao as pecas do rosto, na ordem de [`Part::CHOSEN`].
/// Pele e rosto sao a mesma pergunta -- com que cara este boneco entra --
/// entao eles moram no mesmo cursor.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
struct FighterRow(usize);

/// Quantas linhas o seletor tem.
fn fighter_rows() -> usize {
    1 + Part::CHOSEN.len()
}

fn row_label(row: usize) -> &'static str {
    match row {
        0 => "SKIN",
        at => Part::CHOSEN[(at - 1) % Part::CHOSEN.len()].label(),
    }
}

/// Nome da opcao escolhida agora por este lugar.
fn row_value(row: usize, skin_pick: usize, face: Face) -> &'static str {
    match row {
        0 => skin::skin(skin_pick).name,
        at => face.look(Part::CHOSEN[(at - 1) % Part::CHOSEN.len()]).name,
    }
}

/// Gira a escolha desta linha.
fn row_cycle(row: usize, skin_pick: &mut usize, face: &mut Face, step: i32) {
    match row {
        0 => *skin_pick = cycle(*skin_pick, step, skin::CATALOG.len()),
        at => face.cycle(Part::CHOSEN[(at - 1) % Part::CHOSEN.len()], step),
    }
}

/// Largura fixa da celula de valor, para o seletor nao mudar de tamanho ao
/// navegar -- um botao que encolhe leva a area de clique junto.
fn fighter_cell() -> usize {
    skin::CATALOG
        .iter()
        .map(|s| s.name.chars().count())
        .chain(
            Part::CHOSEN
                .iter()
                .flat_map(|part| part.catalog())
                .map(|look| look.name.chars().count()),
        )
        .max()
        .unwrap_or(0)
}

/// A escolha de um lugar do seletor: pele e rosto.
fn seat_choice(picks: &SkinSelections, mode: GameMode, seat: u8) -> (usize, Face) {
    if mode == GameMode::Online {
        (picks.online_local, picks.online_face)
    } else {
        (
            picks.players[seat as usize],
            picks.faces[seat as usize],
        )
    }
}

/// Altura da linha `row` do seletor.
fn fighter_row_y(row: usize) -> f32 {
    190.0 - row as f32 * 34.0
}

/// Onde a coluna de um lugar comeca.
fn fighter_column_x(mode: GameMode, seat: u8) -> f32 {
    if mode == GameMode::Online {
        60.0
    } else if seat == 0 {
        -150.0
    } else {
        230.0
    }
}

/// Quantos lugares esta tela deixa escolher.
///
/// Online so ha um -- o seu. No treino o segundo lugar e do dummy, e mostrar
/// um seletor la prometeria uma escolha que nao existe.
fn fighter_seats(mode: GameMode) -> u8 {
    match mode {
        GameMode::Online | GameMode::Training => 1,
        _ => 2,
    }
}

fn spawn_skin_select_screen(
    mut commands: Commands,
    mode: Res<GameMode>,
    picks: Res<SkinSelections>,
    row: Res<FighterRow>,
) {
    let title = if fighter_seats(*mode) == 1 {
        "CHOOSE YOUR FIGHTER"
    } else {
        "CHOOSE YOUR FIGHTERS"
    };
    commands.spawn((
        SkinPanel,
        AsciiSprite::new(AsciiArt::solid(title, palette::GOLD)),
        Layer::Hud,
        Transform::from_translation(Vec3::new(0.0, 250.0, 0.0)),
        DespawnOnExit(GameState::SkinSelect),
    ));

    let cell = fighter_cell();
    for line in 0..fighter_rows() {
        let y = fighter_row_y(line);
        Button::new(
            row_label(line),
            MenuAction::Fighter {
                seat: 0,
                row: line,
                step: 0,
            },
        )
        .width(6)
        .chosen(line == row.0)
        .spawn(
            &mut commands,
            GameState::SkinSelect,
            Vec2::new(fighter_column_x(*mode, 0) - 150.0, y),
        );

        for seat in 0..fighter_seats(*mode) {
            let x = fighter_column_x(*mode, seat);
            let (pick, face) = seat_choice(&picks, *mode, seat);
            let step = |step| MenuAction::Fighter {
                seat,
                row: line,
                step,
            };
            let half = cell as f32 * 4.0 + 20.0;
            Button::new(LEFT, step(-1)).spawn(
                &mut commands,
                GameState::SkinSelect,
                Vec2::new(x - half, y),
            );
            Button::new(row_value(line, pick, face), step(1))
                .width(cell)
                .accent(palette::player(seat))
                .spawn(&mut commands, GameState::SkinSelect, Vec2::new(x, y));
            Button::new(RIGHT, step(1)).spawn(
                &mut commands,
                GameState::SkinSelect,
                Vec2::new(x + half, y),
            );
        }
    }

    Button::new("BACK", MenuAction::Back)
        .width(10)
        .spawn(
            &mut commands,
            GameState::SkinSelect,
            Vec2::new(-140.0, -300.0),
        );
    Button::new("CONFIRM", MenuAction::Confirm)
        .width(10)
        .accent(palette::BONE)
        .spawn(
            &mut commands,
            GameState::SkinSelect,
            Vec2::new(140.0, -300.0),
        );

    if fighter_seats(*mode) == 1 {
        let (pick, face) = seat_choice(&picks, *mode, 0);
        spawn_skin_preview(&mut commands, 0, skin::skin(pick), face, Vec2::new(-330.0, -60.0));
    } else {
        spawn_skin_preview(
            &mut commands,
            0,
            skin::skin(picks.players[0]),
            picks.faces[0],
            Vec2::new(-430.0, -60.0),
        );
        spawn_skin_preview(
            &mut commands,
            1,
            skin::skin(picks.players[1]),
            picks.faces[1],
            Vec2::new(430.0, -60.0),
        );
    }
}

fn spawn_skin_preview(
    commands: &mut Commands,
    id: u8,
    chosen: &'static skin::Skin,
    face: Face,
    at: Vec2,
) {
    let color = palette::player(id);
    let root = commands
        .spawn((
            SkinPreview(id),
            Pose::IdleA,
            ActorTint(color),
            Facing(if id == 0 { 1.0 } else { -1.0 }),
            Intent::default(),
            Transform::from_translation(at.extend(0.0)).with_scale(Vec3::splat(1.7)),
            Visibility::default(),
            DespawnOnExit(GameState::SkinSelect),
        ))
        .id();
    crate::actor::spawn_actor_body(
        commands,
        root,
        chosen,
        color,
        if id == 0 { 1.0 } else { -1.0 },
        id as f32 * 1.7,
        face,
    );
}

/// Mantem os botoes e os bonecos de amostra em dia com o que foi escolhido.
fn update_skin_select_screen(
    mode: Res<GameMode>,
    picks: Res<SkinSelections>,
    row: Res<FighterRow>,
    mut buttons: Query<(&mut Button, &mut AsciiSprite)>,
    mut previews: Query<(&SkinPreview, &mut ActorSkin, &mut Face)>,
) {
    let cell = fighter_cell();
    for (mut button, mut sprite) in &mut buttons {
        let MenuAction::Fighter {
            seat,
            row: line,
            step,
        } = button.action
        else {
            continue;
        };
        let next = if step == 0 {
            Button::new(row_label(line), button.action)
                .width(6)
                .chosen(line == row.0)
        } else if button.width > 1 {
            let (pick, face) = seat_choice(&picks, *mode, seat);
            Button::new(row_value(line, pick, face), button.action)
                .width(cell)
                .accent(palette::player(seat))
        } else {
            continue;
        };
        restyle(&mut button, &mut sprite, next);
    }

    for (preview, mut actor_skin, mut face) in &mut previews {
        let (pick, next) = seat_choice(&picks, *mode, preview.0);
        actor_skin.0 = skin::skin(pick);
        face.set_if_neq(next);
    }
}

/// Pequena demonstracao feita das poses reais; nao existe rig paralelo no menu.
fn animate_skin_previews(time: Res<Time>, mut previews: Query<(&SkinPreview, &mut Pose)>) {
    for (preview, mut pose) in &mut previews {
        let t = (time.elapsed_secs() + preview.0 as f32 * 0.18).rem_euclid(4.8);
        let next = match t {
            x if x < 0.8 => Pose::idling((x * 3.0) as usize),
            x if x < 2.1 => Pose::running(((x - 0.8) * 9.0) as usize),
            x if x < 2.55 => Pose::Jump,
            x if x < 2.95 => Pose::Fall,
            x if x < 3.35 => Pose::PunchWindup,
            x if x < 3.65 => Pose::PunchStrike,
            x if x < 4.05 => Pose::PunchRecover,
            _ => Pose::IdleA,
        };
        if *pose != next {
            *pose = next;
        }
    }
}

// --- fim de round -----------------------------------------------------------

