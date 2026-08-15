/// Painel de treino: vida do dummy, combo em andamento e recorde da sessao.
#[derive(Component)]
struct TrainingPanel;

/// Arma escolhida no painel de treino.
#[derive(Resource, Default)]
struct TrainingWeaponPick(usize);

fn training_art(
    dummy: &Health,
    meter: &ComboMeter,
    behavior: DummyBehavior,
    boxes: ShowBoxes,
    weapon: &TrainingWeaponPick,
) -> AsciiArt {
    let filled = (dummy.fraction() * BAR_CELLS as f32).round() as u16;
    let mut art = AsciiArt::solid("DUMMY", palette::GOLD);
    art = art.stamp(&AsciiArt::fill('\u{2588}', filled, 1, palette::GOLD), 6, 0);
    art = art.stamp(
        &AsciiArt::fill('\u{2591}', BAR_CELLS - filled, 1, palette::IRON),
        6 + filled,
        0,
    );

    // Enquanto o combo corre ele fica em vermelho; parado, apagado. A cor e o
    // que diz se os golpes ainda estao encadeando.
    let live = meter.hits > 0;
    art = art.stamp(
        &AsciiArt::solid(
            &format!("COMBO {:>2} HITS   {:>3} DMG", meter.hits, meter.damage),
            if live { palette::BLOOD } else { palette::IRON },
        ),
        6,
        1,
    );
    // Nomear o elo que acabou de acertar e o que ensina o combo: sem isso os
    // tres golpes sao um contador subindo.
    art = art.stamp(
        &AsciiArt::solid(
            &format!("{:<8}", if live { meter.last_move } else { "" }),
            palette::GOLD,
        ),
        6 + BAR_CELLS + 2,
        1,
    );
    art = art.stamp(
        &AsciiArt::solid(
            &format!(
                "BEST  {:>2} HITS   {:>3} DMG",
                meter.best_hits, meter.best_damage
            ),
            palette::ASH,
        ),
        6,
        2,
    );
    art = art.stamp(
        &AsciiArt::solid(
            &format!(
                "TAB {:<6}  H BOXES {}",
                behavior.label(),
                if boxes.0 { "ON " } else { "OFF" }
            ),
            palette::MOSS,
        ),
        6 + BAR_CELLS + 2,
        2,
    );
    art.stamp(
        &AsciiArt::solid(
            &format!(
                "Z/X WEAPON {:<12}  C SPAWN",
                weapon_at(weapon.0 as u8).name()
            ),
            palette::MOSS,
        ),
        6,
        3,
    )
}

fn spawn_training_panel(
    mut commands: Commands,
    dummies: Query<&Health, With<TrainingDummy>>,
    meter: Res<ComboMeter>,
    behavior: Res<DummyBehavior>,
    boxes: Res<ShowBoxes>,
    weapon: Res<TrainingWeaponPick>,
) {
    let Ok(dummy) = dummies.single() else {
        return;
    };
    commands.spawn((
        TrainingPanel,
        AsciiSprite::pivoted(
            training_art(dummy, &meter, *behavior, *boxes, &weapon),
            Vec2::new(0.5, 0.5),
        ),
        Layer::Hud,
        Transform::from_translation(Vec3::new(630.0, 204.0, 0.0)),
        DespawnOnExit(GameState::Fighting),
    ));
}

fn update_training_panel(
    dummies: Query<&Health, With<TrainingDummy>>,
    healed: Query<(), (With<TrainingDummy>, Changed<Health>)>,
    meter: Res<ComboMeter>,
    behavior: Res<DummyBehavior>,
    boxes: Res<ShowBoxes>,
    weapon: Res<TrainingWeaponPick>,
    mut panels: Query<&mut AsciiSprite, With<TrainingPanel>>,
) {
    // Redesenhar todo frame respawnaria os ~70 glifos do painel a toa.
    if !meter.is_changed()
        && !behavior.is_changed()
        && !boxes.is_changed()
        && !weapon.is_changed()
        && healed.is_empty()
    {
        return;
    }
    let Ok(dummy) = dummies.single() else {
        return;
    };
    for mut panel in &mut panels {
        panel.art = training_art(dummy, &meter, *behavior, *boxes, &weapon);
    }
}

/// Redesenha a barra so quando vida ou arma mudam.
///
/// O filtro `Changed` sozinho nao basta: `fire` tica a cadencia da arma todo
/// quadro, e ticar um `Timer` exige `&mut Held` -- entao a mao de quem esta
/// armado conta como mudada sessenta vezes por segundo. Sem a comparacao
/// abaixo, os ~30 glifos de cada barra eram despawnados e recriados em todo
/// quadro em que alguem segurava uma arma.
fn update_hud(
    players: Query<(&Player, &Health, Option<&Held>), Or<(Changed<Health>, Changed<Held>)>>,
    mut bars: Query<(&HudBar, &mut AsciiSprite)>,
) {
    for (player, health, held) in &players {
        for (bar, mut sprite) in &mut bars {
            if bar.0 == player.id {
                let art = hud_art(player.id, player.color, health, held);
                if sprite.art != art {
                    sprite.art = art;
                }
            }
        }
    }
}

/// Redesenha a barra quando a arma acaba (o `Changed` nao ve remocao).
fn refresh_hud_on_weapon_loss(
    mut removed: RemovedComponents<Held>,
    players: Query<(&Player, &Health)>,
    mut bars: Query<(&HudBar, &mut AsciiSprite)>,
) {
    for entity in removed.read() {
        let Ok((player, health)) = players.get(entity) else {
            continue;
        };
        for (bar, mut sprite) in &mut bars {
            if bar.0 == player.id {
                sprite.art = hud_art(player.id, player.color, health, None);
            }
        }
    }
}

fn pulse_low_health(
    time: Res<Time>,
    players: Query<(&Player, &Health)>,
    mut bars: Query<(&HudBar, &mut Transform)>,
) {
    for (bar, mut transform) in &mut bars {
        let low = players
            .iter()
            .find(|(player, _)| player.id == bar.0)
            .is_some_and(|(_, health)| health.fraction() < 0.3);
        let scale = if low {
            1.0 + (time.elapsed_secs() * 9.0).sin().max(0.0) * 0.06
        } else {
            1.0
        };
        transform.scale = Vec3::splat(scale);
    }
}

/// Controla dummy, caixas e o arsenal sob demanda, sem sair da luta.
///
/// Fica na camada de UI porque e um controle de tela, nao de personagem: o
/// gameplay continua sem ler teclado.
fn training_controls(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut behavior: ResMut<DummyBehavior>,
    mut boxes: ResMut<ShowBoxes>,
    mut weapon: ResMut<TrainingWeaponPick>,
    players: Query<(&Transform, &Facing), With<Player>>,
    loose: Query<Entity, With<GroundWeapon>>,
) {
    if keys.just_pressed(KeyCode::KeyH) {
        boxes.0 = !boxes.0;
    }
    if keys.just_pressed(KeyCode::Tab) {
        *behavior = behavior.next();
    }
    let step = keys.just_pressed(KeyCode::KeyX) as i32
        - keys.just_pressed(KeyCode::KeyZ) as i32;
    if step != 0 {
        weapon.0 = cycle(weapon.0, step, ARSENAL.len());
    }
    if !keys.just_pressed(KeyCode::KeyC) {
        return;
    }
    let Ok((transform, facing)) = players.single() else {
        return;
    };
    for entity in &loose {
        commands.entity(entity).despawn();
    }
    let kind = weapon.0 as u8;
    let at = transform.translation.truncate() + Vec2::new(facing.0 * 48.0, 70.0);
    spawn_ground_weapon(
        &mut commands,
        None,
        kind,
        weapon_at(kind).ammo(),
        at,
        Vec2::ZERO,
    );
}

