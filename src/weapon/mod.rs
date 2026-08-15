//! Armas: contrato, arsenal, drops e projeteis.
//!
//! Uma arma e um [`Weapon`]: ela so descreve como aparece no chao e que tiros
//! produz. Tudo o mais -- pegar, mirar, cadencia, municao, dano -- e generico,
//! entao arma nova e um `impl Weapon` e uma linha no arsenal.

use bevy::prelude::*;
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::OnceLock;

use crate::actor::input::CursorWorld;
use crate::actor::{Attacking, Facing, Intent, Player, Pose};
use crate::ascii::{AsciiArt, AsciiSprite, Layer, palette};
use crate::combat::{Hitbox, Lifetime, MeleeAttack, Parrying};
use crate::fx::{Casing, Effect, MuzzleFlash, Smoke};
use crate::level::{ARENA_HALF_W, CurrentLevel};
use crate::physics::{Collider, Falls, Ghost, Grounded, KILL_Y, Solid, Velocity, overlap};
use crate::state::{AppSet, GameState};

include!("types.rs");
include!("arsenal/firearms.rs");
include!("arsenal/blades.rs");
include!("arsenal/special.rs");
include!("gameplay/inventory.rs");
include!("visuals/forge.rs");
include!("gameplay/runtime.rs");
include!("visuals/effects.rs");
include!("gameplay/projectiles.rs");
include!("visuals/animation.rs");
include!("plugin.rs");

#[cfg(test)]
mod tests;
