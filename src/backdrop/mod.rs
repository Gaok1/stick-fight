//! Fundo das arenas.
//!
//! O fundo nao e um cartao parado atras da briga. Ele tem tres coisas que um
//! cartao nao tem:
//!
//! - **profundidade**: cada peca vive num plano, e planos distantes deslizam
//!   junto com a briga em vez de ficarem colados no mundo. E parallax de
//!   verdade, mesmo com a camera fixa -- quem se move e a atencao, nao a lente.
//! - **clima**: cinza, chuva, fuligem e petala caem sem parar, e nunca nascem
//!   nem morrem: a mesma particula da a volta pela moldura.
//! - **acontecimento**: o vulcao acorda de tempos em tempos, cospe fumaca,
//!   bomba de lava e sacode a tela.
//!
//! Nada aqui colide, machuca ou entra na fisica. O arquivo inteiro pode ser
//! reescrito sem que uma regra de jogo mude -- e por isso ele pode ser
//! exagerado a vontade.

use bevy::prelude::*;

use crate::actor::Player;
use crate::ascii::{Accent, AsciiArt, AsciiSprite, CELL, Layer, palette};
use crate::combat::Lifetime;
use crate::fx::Shake;
use crate::level::{ARENA_HALF_H, ARENA_HALF_W, LevelGeometry};
use crate::state::{AppSet, GameState, arena_live};

include!("types.rs");
include!("art/industrial.rs");
include!("art/oriental.rs");
include!("art/panels.rs");
include!("ambience/flows.rs");
include!("scenery/landmarks.rs");
include!("scenery/dragon.rs");
include!("ambience/weather.rs");
include!("ambience/volcano.rs");
include!("scenery/scene.rs");
include!("plugin.rs");

#[cfg(test)]
mod tests;
