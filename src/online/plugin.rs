pub struct OnlinePlugin;

impl Plugin for OnlinePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OnlineSession>()
            .add_message::<LobbyCommand>()
            .add_systems(Startup, init_steam)
            .add_systems(
                PreUpdate,
                (
                    pump_callbacks,
                    handle_events,
                    poll_lobby,
                    receive_packets,
                    apply_snapshot,
                    apply_weapon_state,
                )
                    .chain()
                    .in_set(NetReceive),
            )
            .add_systems(OnEnter(GameState::Controls), leave_lobby)
            .add_systems(OnEnter(GameState::Lobby), reopen_lobby)
            .add_systems(OnEnter(GameState::RoundOver), broadcast_round_over)
            .add_systems(
                Update,
                (
                    sync_skin_choice,
                    publish_stage,
                    follow_host_stage,
                    run_lobby_commands,
                )
                    .chain()
                    .in_set(AppSet::Logic)
                    .run_if(in_state(GameState::Lobby)),
            )
            .add_systems(
                Update,
                round_over_controls
                    .in_set(AppSet::Input)
                    .run_if(in_state(GameState::RoundOver))
                    .run_if(resource_equals(GameMode::Online)),
            )
            // Antes de o combate decidir o round: quem desistiu tem que ja
            // contar como derrotado neste mesmo quadro.
            .add_systems(
                Update,
                retire_missing_players
                    .in_set(AppSet::Logic)
                    .before(crate::combat::check_round_over)
                    .run_if(in_state(GameState::Fighting))
                    .run_if(resource_equals(GameMode::Online)),
            )
            // Tambem na espera: e a troca de entrada e snapshot que faz os
            // outros aparecerem andando no lobby em vez de so como um nome numa
            // lista. O protocolo e o mesmo -- a espera nao inventa pacote.
            .add_systems(
                PostUpdate,
                (send_local_input, broadcast_inputs, send_snapshot)
                    .run_if(arena_live)
                    .run_if(resource_equals(GameMode::Online)),
            )
            // Arma so cai na luta, entao o retrato dela nao precisa rodar na
            // espera.
            .add_systems(
                PostUpdate,
                broadcast_weapons
                    .run_if(in_state(GameState::Fighting))
                    .run_if(resource_equals(GameMode::Online)),
            );
    }
}

