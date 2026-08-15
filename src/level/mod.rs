//! Fases.
//!
//! Um mapa e um `Level`: ele diz onde os jogadores nascem, onde as armas caem,
//! e monta a propria geometria. Trocar de fase e trocar o `Box<dyn Level>` no
//! recurso -- nenhum outro sistema precisa saber qual mapa esta no ar.

use bevy::prelude::*;

use crate::actor::{Health, Player, Stunned};
use crate::ascii::{AsciiArt, AsciiSprite, CELL, Layer, palette};
use crate::backdrop::{Building, Scene, Sign, Theme};
use crate::combat::{Damaged, Lifetime};
use crate::physics::{Collider, Falls, Ghost, KILL_Y, OneWay, Solid, Velocity, overlap};
use crate::state::{AppSet, GameMode, GameState, arena_live};
use crate::weapon::Projectile;

include!("types.rs");
include!("environment/hazards.rs");
include!("environment/chains.rs");
include!("stages/classic.rs");
include!("stages/types.rs");
include!("stages/volcanic.rs");
include!("stages/industrial.rs");
include!("stages/oriental.rs");
include!("stages/catalog.rs");
include!("lifecycle.rs");
include!("plugin.rs");

#[cfg(test)]
mod tests;
