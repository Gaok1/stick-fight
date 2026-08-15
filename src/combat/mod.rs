//! Combate: hitboxes, dano, morte.
//!
//! Soco e projetil produzem a mesma coisa -- uma entidade com [`Hitbox`] --
//! entao existe um unico sistema que resolve dano. Adicionar uma arma nova
//! nunca exige mexer aqui.

use bevy::prelude::*;

use crate::actor::pose::MeleeKind;
use crate::actor::{
    Attacking, Downed, Facing, Flash, Health, Intent, MAX_PLAYERS, MIN_PLAYERS, Player, Pose,
    Stunned, TrainingDummy,
};
use crate::ascii::{AsciiArt, AsciiSprite, Layer, palette};
use crate::physics::{Collider, Grounded, Velocity, overlap};
use crate::state::{AppSet, GameMode, GameState, arena_live};
use crate::weapon::{Held, MeleeMove, WeaponStyle};
use crate::weapon::{Sticky, ThrownWeapon};

include!("types.rs");
include!("systems/attacks.rs");
include!("systems/resolution.rs");
include!("systems/debug.rs");
include!("plugin.rs");

#[cfg(test)]
mod tests;
