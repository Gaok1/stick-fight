/// Anima o fundo: profundidade, clima, fumaca e erupcao.
pub struct BackdropPlugin;

impl Plugin for BackdropPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Focus>().add_systems(
            Update,
            // Encadeados de proposito: tudo mexe em `Parallax::home`, e
            // `drift_planes` -- o unico que escreve `Transform` -- roda por
            // ultimo, com o quadro ja fechado.
            (
                track_focus,
                blow_weather,
                run_vents,
                run_shows,
                // Nesta ordem, e nao em qualquer uma: o corpo le o rastro que
                // a cabeca acabou de gravar, e os bigodes se penduram no
                // focinho que ela acabou de virar. Um quadro de atraso em
                // qualquer um dos dois abre uma emenda visivel na curva.
                fly_dragon,
                coil_dragon,
                wave_whiskers,
                turn_pages,
                drift_smoke,
                fly_bombs,
                fly_jade_flames,
                flicker_neon,
                drift_planes,
            )
                .chain()
                .in_set(AppSet::Animate)
                .run_if(arena_live),
        )
        // A ventania do menu roda fora do `arena_live`: ali nao ha arena
        // levantada, nem camera para seguir, nem parallax do que tirar
        // profundidade -- a folha carrega a propria e escreve o proprio
        // `Transform`.
        .add_systems(
            Update,
            blow_gale
                .in_set(AppSet::Animate)
                .run_if(in_state(GameState::Controls)),
        );
    }
}

