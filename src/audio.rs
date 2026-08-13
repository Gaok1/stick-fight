//! Trilha sonora.
//!
//! Uma faixa por estado, trocada na transicao. A regra de qual faixa toca em
//! qual estado fica num unico lugar ([`Soundtrack::track_for`]) para nao virar
//! um `OnEnter` espalhado por cada tela.

use bevy::audio::{PlaybackMode, Volume};
use bevy::prelude::*;

use crate::state::GameState;

/// Volume da trilha. Fundo, nao protagonista.
const MUSIC_VOLUME: f32 = 0.45;

/// Marca a entidade que esta tocando, para poder trocar de faixa.
#[derive(Component)]
struct MusicTrack;

/// Faixas carregadas.
#[derive(Resource)]
struct Soundtrack {
    /// Toca nas telas de menu e de fim de round.
    menu: Handle<AudioSource>,
    /// Toca durante a luta.
    duel: Handle<AudioSource>,
}

impl Soundtrack {
    /// Faixa correspondente a um estado.
    fn track_for(&self, state: &GameState) -> &Handle<AudioSource> {
        match state {
            GameState::Fighting => &self.duel,
            GameState::Controls
            | GameState::SkinSelect
            | GameState::Lobby
            | GameState::RoundOver => &self.menu,
        }
    }
}

/// Carrega as faixas no boot.
fn load_soundtrack(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(Soundtrack {
        menu: assets.load("songs/main menu.mp3"),
        duel: assets.load("songs/Duel.mp3"),
    });
}

/// Troca a faixa quando o estado muda.
///
/// `Controls -> RoundOver` cai na mesma faixa; trocar assim mesmo reiniciaria a
/// musica do zero, entao a troca so acontece quando o handle e outro.
fn sync_track(
    mut commands: Commands,
    soundtrack: Option<Res<Soundtrack>>,
    state: Res<State<GameState>>,
    playing: Query<(Entity, &AudioPlayer), With<MusicTrack>>,
) {
    // O primeiro `OnEnter` pode rodar antes do `PreStartup` que carrega os
    // handles. O `Startup` chama este sistema novamente logo depois.
    let Some(soundtrack) = soundtrack else {
        return;
    };
    let wanted = soundtrack.track_for(state.get());

    if let Ok((entity, current)) = playing.single() {
        if current.0.id() == wanted.id() {
            return;
        }
        commands.entity(entity).despawn();
    }

    commands.spawn((
        MusicTrack,
        AudioPlayer(wanted.clone()),
        PlaybackSettings {
            mode: PlaybackMode::Loop,
            volume: Volume::Linear(MUSIC_VOLUME),
            ..default()
        },
    ));
}

/// Carrega e sincroniza a trilha com o estado do jogo.
pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, load_soundtrack)
            .add_systems(Startup, sync_track)
            .add_systems(OnEnter(GameState::Controls), sync_track)
            .add_systems(OnEnter(GameState::SkinSelect), sync_track)
            .add_systems(OnEnter(GameState::Fighting), sync_track)
            .add_systems(OnEnter(GameState::RoundOver), sync_track);
    }
}
