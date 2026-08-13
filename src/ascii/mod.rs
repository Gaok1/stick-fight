//! Camada de renderizacao ASCII.
//!
//! ASCII aqui e estilo de arte, nao meio de saida: nada disso roda num
//! terminal. Os sprites vivem em espaco continuo com `Transform` proprio, e a
//! camada de FX (`crate::fx`) desenha por cima sem passar por glifo nenhum.

pub mod art;
pub mod cp437;
mod cp437_table;
pub mod palette;
pub mod sprite;

use bevy::prelude::*;

pub use art::{Accent, AsciiArt};
pub use cp437::CELL;
pub use sprite::{AsciiSprite, Layer};

use crate::state::AppSet;

/// Registra o atlas de glifos e os sistemas de rebuild.
pub struct AsciiPlugin;

impl Plugin for AsciiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, cp437::init_glyph_atlas)
            .add_systems(
                Update,
                (sprite::apply_layer_z, sprite::rebuild_glyphs)
                    .chain()
                    .in_set(AppSet::Render),
            );
    }
}
