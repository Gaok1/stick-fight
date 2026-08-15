/// Carrega `Arena01` e liga o ciclo de vida da geometria ao estado de luta.
pub struct LevelPlugin;

impl Plugin for LevelPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CurrentLevel(level_at(0)))
            .init_resource::<LevelPick>()
            .init_resource::<BuiltStage>()
            // Antes da troca de estado do quadro, e depois dos pacotes: e essa
            // ordem que faz o cliente entrar na luta ja com o mapa do host.
            .add_systems(PreUpdate, apply_level_pick.after(crate::online::NetReceive))
            // A fase gira a cada round. Antes do placar aparecer, para a tela
            // conseguir anunciar para onde a briga vai.
            .add_systems(
                OnEnter(GameState::RoundOver),
                rotate_stage
                    .before(crate::ui::spawn_round_over_screen)
                    .run_if(crate::online::can_decide_round),
            )
            .add_systems(
                Update,
                rebuild_on_stage_change
                    .in_set(AppSet::Logic)
                    .run_if(arena_live),
            )
            // A arena existe tanto na espera quanto na luta -- no lobby da pra
            // andar nela. Sair de qualquer um dos dois a derruba, e entrar no
            // outro a levanta de novo: a geometria some por marcador, entao ela
            // nao precisa saber de que estado veio.
            .add_systems(
                OnEnter(GameState::Lobby),
                build_level.run_if(resource_equals(GameMode::Online)),
            )
            .add_systems(OnExit(GameState::Lobby), clear_level)
            .add_systems(OnEnter(GameState::Fighting), build_level)
            .add_systems(OnExit(GameState::Fighting), clear_level)
            .add_systems(
                Update,
                break_chains.in_set(AppSet::Logic).run_if(arena_live),
            )
            .add_systems(
                Update,
                simulate_chains.in_set(AppSet::Physics).run_if(arena_live),
            )
            .add_systems(
                Update,
                (
                    drip_spouts,
                    splash_droplets,
                    hurt_on_hazards,
                    tick_hazard_cooldowns,
                )
                    .chain()
                    .in_set(AppSet::Logic)
                    .run_if(arena_live),
            )
            // A mare anda antes de qualquer coisa perguntar onde ela esta:
            // arte e zona de contato sao entidades diferentes, e mover uma
            // depois de o dano ja ter sido decidido faria a poca ferir na
            // altura do frame anterior.
            .add_systems(
                Update,
                swell_tides
                    .in_set(AppSet::Physics)
                    .before(simulate_chains)
                    .run_if(arena_live),
            )
            .add_systems(
                Update,
                (animate_hazards, erupt_geysers)
                    .in_set(AppSet::Animate)
                    .run_if(arena_live),
            );
    }
}

