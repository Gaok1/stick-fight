/// Soco, dano, morte e fim de round.
pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RoundResult>()
            .init_resource::<ComboMeter>()
            .init_resource::<ShowBoxes>()
            .add_systems(
                Update,
                // Antes do rebuild de glifos: o contorno nasce e e desenhado
                // no mesmo quadro, senao ele pisca -- ele e refeito do zero
                // toda vez.
                draw_debug_boxes
                    .in_set(AppSet::Render)
                    .before(crate::ascii::sprite::rebuild_glyphs)
                    .run_if(in_state(GameState::Fighting))
                    .run_if(resource_equals(GameMode::Training)),
            )
            .add_message::<Damaged>()
            .add_message::<Parried>()
            .add_systems(
                OnEnter(GameState::Fighting),
                (reset_combo_meter, start_new_match_if_over, record_seats),
            )
            .add_systems(OnEnter(GameState::Controls), clear_match)
            .add_systems(
                Update,
                (
                    tick_timers,
                    start_parry,
                    start_heavy,
                    crouch_hurtbox,
                    start_sweep,
                    start_dive,
                    start_melee,
                    queue_melee,
                    launch_melee,
                    continue_combo,
                    move_following_hitboxes,
                    resolve_hits,
                    land_dive,
                )
                    .chain()
                    .in_set(AppSet::Logic)
                    // Vale tambem no aquecimento do lobby: brigar la e o ponto
                    // da espera. O que o lobby nao tem e o round.
                    .run_if(arena_live),
            )
            // Quem decide o fim do round depende do modo: no versus alguem
            // vence, no treino o jogador so volta pro ponto de partida.
            .add_systems(
                Update,
                // Vale em qualquer modo que nao seja o treino: contra o jogo o
                // round decide igual, so muda quem move o segundo boneco.
                check_round_over
                    .after(resolve_hits)
                    .in_set(AppSet::Logic)
                    .run_if(in_state(GameState::Fighting))
                    .run_if(crate::online::can_decide_round),
            )
            .add_systems(
                Update,
                track_training_combo
                    .after(resolve_hits)
                    .in_set(AppSet::Logic)
                    .run_if(in_state(GameState::Fighting))
                    .run_if(resource_equals(GameMode::Training)),
            )
            // Nas duas telas onde ninguem perde: o treino e a espera do lobby.
            .add_systems(
                Update,
                recover_the_fallen
                    .after(resolve_hits)
                    .in_set(AppSet::Logic)
                    .run_if(in_state(GameState::Lobby).or_else(
                        in_state(GameState::Fighting).and_then(resource_equals(GameMode::Training)),
                    )),
            )
            // Antes de `apply_pose`: quem desenha o boneco e ele, e a piscada
            // tem que estar decidida quando ele roda, senao ela sai um frame
            // atrasada.
            .add_systems(
                Update,
                (blink_invulnerable, clear_flash)
                    .chain()
                    .before(crate::actor::motion::apply_pose)
                    .in_set(AppSet::Animate)
                    .run_if(arena_live),
            );
    }
}
