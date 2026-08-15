/// Barra de vida + arma de um jogador.
fn hud_art(id: u8, color: Color, health: &Health, held: Option<&Held>) -> AsciiArt {
    let filled = (health.fraction() * BAR_CELLS as f32).round() as u16;
    let label = AsciiArt::solid(&format!("P{}", id + 1), color);

    let mut art = label.stamp(&AsciiArt::fill('\u{2588}', filled, 1, color), 3, 0);
    art = art.stamp(
        &AsciiArt::fill('\u{2591}', BAR_CELLS - filled, 1, palette::IRON),
        3 + filled,
        0,
    );
    art = art.stamp(
        &AsciiArt::solid(&format!("{:>4}", health.hp.max(0)), palette::BONE),
        4 + BAR_CELLS,
        0,
    );

    let weapon = match held {
        // Arma de contato nao tem municao; mostrar "x0" a faria passar por
        // arma de fogo vazia, que e o oposto do que ela e.
        Some(held) if held.weapon.is_melee() => held.weapon.name().to_string(),
        Some(held) => format!("{} x{}", held.weapon.name(), held.ammo),
        None => "FISTS".to_string(),
    };
    art.stamp(&AsciiArt::solid(&weapon, palette::GOLD), 3, 1)
}

/// Altura da segunda fileira de barras.
///
/// A arte de uma barra tem duas linhas de celula; o passo abre uma linha de
/// folga entre as fileiras para elas nao lerem como um bloco so.
const HUD_ROW: f32 = 3.0 * crate::ascii::CELL.y;

/// Onde a barra de um lugar fica: canto e fileira.
///
/// Impares a direita, pares a esquerda, descendo de dois em dois. Assim uma
/// briga de dois mantem exatamente o HUD de sempre e o terceiro jogador
/// aparece sem empurrar ninguem.
fn hud_anchor(id: u8) -> (Vec3, f32) {
    let side = if id.is_multiple_of(2) { -1.0 } else { 1.0 };
    let row = (id / 2) as f32;
    (
        Vec3::new(630.0 * side, 228.0 - row * HUD_ROW, 0.0),
        side * 0.5,
    )
}

/// Cria uma barra por lutador ao comecar a luta.
fn spawn_hud(mut commands: Commands, players: Query<(&Player, &Health)>) {
    for (player, health) in &players {
        let (at, pivot_x) = hud_anchor(player.id);

        commands.spawn((
            HudBar(player.id),
            AsciiSprite::pivoted(
                hud_art(player.id, player.color, health, None),
                Vec2::new(pivot_x, 0.5),
            ),
            Layer::Hud,
            Transform::from_translation(at),
            DespawnOnExit(GameState::Fighting),
        ));
    }
}

