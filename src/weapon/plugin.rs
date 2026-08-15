/// Arsenal, drops, disparo e projeteis.
pub struct WeaponPlugin;

impl Plugin for WeaponPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NextWeaponId>()
            .add_systems(
                OnEnter(GameState::Fighting),
                (arm_drop_schedule, spawn_crosshair),
            )
            // Quem larga, quem pega e quem arremessa e a autoridade da partida.
            // Online o cliente recebe o resultado pronto: dois sorteios
            // independentes davam duas arenas armadas de jeitos diferentes.
            .add_systems(
                Update,
                (drop_weapons, sink_lost_weapons, throw_weapon, pick_up)
                    .chain()
                    .in_set(AppSet::Logic)
                    .run_if(in_state(GameState::Fighting))
                    .run_if(crate::online::is_authority),
            )
            // O resto roda em todo mundo: tiro, pavio e rastro saem da arma que a
            // replicacao ja poe na mao, entao o cliente ve o disparo na hora em vez
            // de esperar o proximo pacote para desenhar a bala.
            .add_systems(
                Update,
                (
                    fire,
                    tick_fuses,
                    tick_recoil,
                    emit_projectile_trails,
                    despawn_spent_projectiles,
                    clear_weapon_icon,
                )
                    .chain()
                    .after(pick_up)
                    .in_set(AppSet::Logic)
                    .run_if(in_state(GameState::Fighting)),
            )
            .add_systems(
                Update,
                (
                    animate_weapon_icon,
                    animate_weapon_rigs.after(animate_weapon_icon),
                    animate_thrown_weapons,
                    animate_tumbling_shots,
                    // Depois de toda a logica do quadro, e nao junto dela: assim a
                    // faca e presa no mesmo quadro em que cravou, sem depender de
                    // quem resolve dano ter rodado antes deste sistema.
                    stick_projectiles,
                    pulse_pickups,
                    emit_book_aura,
                    animate_arcane_flames.after(emit_book_aura),
                    track_crosshair,
                )
                    .in_set(AppSet::Animate)
                    .run_if(in_state(GameState::Fighting)),
            );
    }
}

