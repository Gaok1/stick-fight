/// Telas, HUD e navegacao.
pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FighterRow>()
            .init_resource::<PointerOverUi>()
            .init_resource::<TrainingWeaponPick>()
            .add_message::<MenuAction>()
            .add_systems(OnEnter(GameState::Controls), spawn_controls_screen)
            .add_systems(OnEnter(GameState::SkinSelect), spawn_skin_select_screen)
            .add_systems(OnEnter(GameState::Lobby), spawn_lobby_screen)
            .add_systems(OnEnter(GameState::RoundOver), spawn_round_over_screen)
            // O HUD le os jogadores, entao precisa entrar depois do spawn deles.
            .add_systems(
                OnEnter(GameState::Fighting),
                spawn_hud.after(crate::actor::spawn_players),
            )
            .add_systems(
                OnEnter(GameState::Fighting),
                spawn_training_panel
                    .after(crate::actor::spawn_training_dummy)
                    .run_if(resource_equals(GameMode::Training)),
            )
            // O mouse antes do teclado, e os dois antes de quem decide: assim
            // um clique e uma tecla no mesmo quadro chegam juntos.
            .add_systems(
                Update,
                (point_at_buttons, keyboard_actions)
                    .in_set(AppSet::Input)
                    .before(crate::actor::input::gather_intents),
            )
            .add_systems(Update, apply_menu_action.in_set(AppSet::Logic))
            .add_systems(
                Update,
                update_controls_screen
                    .in_set(AppSet::Animate)
                    .run_if(in_state(GameState::Controls)),
            )
            .add_systems(
                Update,
                (update_skin_select_screen, animate_skin_previews)
                    .in_set(AppSet::Animate)
                    .before(crate::actor::motion::apply_pose)
                    .run_if(in_state(GameState::SkinSelect)),
            )
            .add_systems(
                Update,
                update_lobby_screen
                    .in_set(AppSet::Animate)
                    .run_if(in_state(GameState::Lobby)),
            )
            .add_systems(
                Update,
                (update_hud, refresh_hud_on_weapon_loss, pulse_low_health)
                    .in_set(AppSet::Animate)
                    .run_if(in_state(GameState::Fighting)),
            )
            .add_systems(
                Update,
                (
                    update_training_panel.in_set(AppSet::Animate),
                    training_controls.in_set(AppSet::Input),
                )
                    .run_if(in_state(GameState::Fighting))
                    .run_if(resource_equals(GameMode::Training)),
            );
    }
}

