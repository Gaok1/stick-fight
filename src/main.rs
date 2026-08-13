//! Stick Fight ASCII -- POC.
//!
//! Jogo de briga 2D local. A arte e ASCII (pagina de codigo 437, a mesma do
//! Dwarf Fortress), mas o jogo nao roda em terminal: cada glifo e um sprite com
//! `Transform` proprio em espaco continuo, entao posicao, escala e rotacao sao
//! livres e a camada de efeitos pode desenhar por cima sem passar por glifo.

mod actor;
mod ascii;
mod audio;
mod backdrop;
mod combat;
mod fx;
mod gore;
mod level;
mod online;
mod physics;
mod state;
mod ui;
mod weapon;

use bevy::camera::ScalingMode;
use bevy::prelude::*;
use bevy::window::PresentMode;

use ascii::palette;
use level::{ARENA_HALF_H, ARENA_HALF_W};
use state::{AppSet, GameState};

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "STICK FIGHT // CP437".into(),
                        resolution: (1280u32, 720u32).into(),
                        present_mode: PresentMode::AutoVsync,
                        ..default()
                    }),
                    ..default()
                })
                // Pixel art: nada de filtro linear nos glifos.
                .set(ImagePlugin::default_nearest()),
        )
        .insert_resource(ClearColor(palette::VOID))
        .init_state::<GameState>()
        // A ordem importa: o rebuild dos glifos (`Render`) tem que ver o estado
        // final do frame, senao a animacao sai atrasada.
        .configure_sets(
            Update,
            (
                AppSet::Input,
                AppSet::Logic,
                AppSet::Physics,
                AppSet::Animate,
                AppSet::Render,
            )
                .chain(),
        )
        .add_plugins((
            ascii::AsciiPlugin,
            audio::AudioPlugin,
            physics::PhysicsPlugin,
            level::LevelPlugin,
            backdrop::BackdropPlugin,
            online::OnlinePlugin,
            actor::ActorPlugin,
            combat::CombatPlugin,
            weapon::WeaponPlugin,
            fx::FxPlugin,
            gore::GorePlugin,
            ui::UiPlugin,
        ))
        .add_systems(Startup, spawn_camera)
        .run();
}

/// Camera ortografica que garante a arena inteira visivel em qualquer proporcao
/// de tela, mostrando sobra no eixo que couber a mais.
fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMin {
                min_width: ARENA_HALF_W * 2.0 + 20.0,
                min_height: ARENA_HALF_H * 2.0 + 20.0,
            },
            ..OrthographicProjection::default_2d()
        }),
    ));
}
