/// Painel de teclas da tela inicial.
#[derive(Component)]
struct MenuPanel;

/// Linha do modo escolhido, abaixo dos botoes.
#[derive(Component)]
struct MenuBlurb;

/// As linhas de teclas da tela inicial.
///
/// Separadas do desenho para que o teste dos realces trabalhe sobre o texto de
/// verdade. Enquanto ele tinha uma copia das linhas, a copia podia envelhecer
/// e o teste continuava passando sobre um menu que nao existia mais.
fn menu_lines() -> Vec<String> {
    [
        "                 PLAYER 1        PLAYER 2",
        "  MOVE            A / D        LEFT / RIGHT",
        "  JUMP              W               UP",
        "  CLIMB           W / S         UP / DOWN",
        "  GRAB / RELEASE     F           NUMPAD 1",
        "  DROP THRU          S              DOWN",
        "  COMBO M1       MOUSE 1          NUMPAD 0",
        "  SWEEP        DOWN + M1      DOWN + NUMPAD 0",
        "  DIVE KICK     M1 IN AIR      NUMPAD 0 IN AIR",
        "  AIM            MOUSE MOVE        FACING",
        "  SHOOT          MOUSE 2          NUMPAD 2",
        "  THROW WEAPON       G           NUMPAD 4",
        "  PARRY              Q           NUMPAD 3",
        SEPARATOR,
        "  FISTS FIRST. WEAPONS START DROPPING LATER.",
        "  A MATCH IS THE FIRST TO 3 ROUNDS.",
        "  THE STAGE ROLLS TO A RANDOM ONE AFTER EVERY ROUND.",
        "  ONLINE HOLDS TWO TO FOUR. WARM UP IN THE LOBBY.",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

/// Conteudo do painel de teclas.
fn menu_art() -> AsciiArt {
    let owned = menu_lines();
    let lines: Vec<&str> = owned.iter().map(String::as_str).collect();
    let art = box_art(&lines);
    let art = highlight(art, &lines, "PLAYER 1", palette::player(0));
    highlight(art, &lines, "PLAYER 2", palette::player(1))
}

/// Largura da maior etiqueta de modo, que e o que fixa a celula do seletor.
fn widest_label() -> usize {
    GameMode::ALL
        .iter()
        .map(|m| m.label().chars().count())
        .max()
        .unwrap_or(0)
}

/// Largura da maior etiqueta de fase.
fn widest_stage() -> usize {
    (0..LEVEL_CATALOG.len())
        .map(|i| level_name(i).chars().count())
        .max()
        .unwrap_or(0)
}

/// Onde cada botao de modo fica.
fn mode_slot(at: usize) -> Vec2 {
    let step = (widest_label() + 4) as f32 * crate::ascii::CELL.x + 16.0;
    let span = step * (GameMode::ALL.len() - 1) as f32;
    Vec2::new(at as f32 * step - span * 0.5, 108.0)
}

/// Tela inicial: titulo, seletores e controles.
fn spawn_controls_screen(mut commands: Commands, mode: Res<GameMode>, pick: Res<LevelPick>) {
    // A ventania primeiro, para ela ficar atras de tudo que vem depois -- e por
    // `Layer::Background`, e nao pela ordem, que a garantia vale.
    crate::backdrop::seed_gale(&mut commands);

    commands.spawn((
        AsciiSprite::new(AsciiArt::banner("STICK FIGHT", '\u{2588}', palette::BONE)),
        Layer::Hud,
        Transform::from_translation(Vec3::new(0.0, 250.0, 0.0)).with_scale(Vec3::splat(0.42)),
        DespawnOnExit(GameState::Controls),
    ));

    for (at, option) in GameMode::ALL.into_iter().enumerate() {
        Button::new(option.label(), MenuAction::PickMode(option))
            .width(widest_label())
            .chosen(option == *mode)
            .spawn(&mut commands, GameState::Controls, mode_slot(at));
    }

    let stage_x = (widest_stage() as f32 * crate::ascii::CELL.x) * 0.5 + 32.0;
    Button::new(LEFT, MenuAction::Stage(-1)).spawn(
        &mut commands,
        GameState::Controls,
        Vec2::new(-stage_x, 64.0),
    );
    Button::new(level_name(pick.0), MenuAction::Stage(1))
        .width(widest_stage())
        .chosen(true)
        .spawn(&mut commands, GameState::Controls, Vec2::new(0.0, 64.0));
    Button::new(RIGHT, MenuAction::Stage(1)).spawn(
        &mut commands,
        GameState::Controls,
        Vec2::new(stage_x, 64.0),
    );

    commands.spawn((
        MenuBlurb,
        AsciiSprite::new(AsciiArt::solid(mode.blurb().trim(), palette::MOSS)),
        Layer::Hud,
        Transform::from_translation(Vec3::new(0.0, 32.0, 0.0)),
        DespawnOnExit(GameState::Controls),
    ));

    Button::new("FIGHT", MenuAction::Play)
        .width(12)
        .accent(palette::BONE)
        .spawn(&mut commands, GameState::Controls, Vec2::new(0.0, -8.0));

    commands.spawn((
        MenuPanel,
        AsciiSprite::new(menu_art()),
        Layer::Hud,
        Transform::from_translation(Vec3::new(0.0, -195.0, 0.0)),
        DespawnOnExit(GameState::Controls),
    ));
}

/// Mantem os botoes da tela inicial em dia com o que esta escolhido.
fn update_controls_screen(
    mode: Res<GameMode>,
    pick: Res<LevelPick>,
    mut buttons: Query<(&mut Button, &mut AsciiSprite), Without<MenuBlurb>>,
    mut blurb: Query<&mut AsciiSprite, With<MenuBlurb>>,
) {
    for (mut button, mut sprite) in &mut buttons {
        let next = match button.action {
            MenuAction::PickMode(option) => Button::new(option.label(), button.action)
                .width(widest_label())
                .chosen(option == *mode),
            MenuAction::Stage(1) if button.width > 1 => {
                Button::new(level_name(pick.0), button.action)
                    .width(widest_stage())
                    .chosen(true)
            }
            _ => continue,
        };
        restyle(&mut button, &mut sprite, next);
    }
    for mut sprite in &mut blurb {
        let art = AsciiArt::solid(mode.blurb().trim(), palette::MOSS);
        if sprite.art != art {
            sprite.art = art;
        }
    }
}

// --- sala online ------------------------------------------------------------

