//! Telas e HUD.
//!
//! Tudo aqui e feito das mesmas primitivas de arte do jogo -- nao ha caminho de
//! texto separado. O titulo inclusive e gerado expandindo o bitmap da propria
//! fonte, entao a tela inicial nunca sai de estilo em relacao a arena.
//!
//! Mouse e teclado nao tem caminhos separados: os dois escrevem a mesma
//! [`MenuAction`], e um so lugar sabe o que cada acao significa. Enquanto o
//! menu foi so teclado, cada tela tinha o proprio `if keys.just_pressed`
//! espalhado, e ligar o mouse teria significado escrever tudo de novo do lado.

use bevy::prelude::*;

use crate::actor::face::{Face, Part};
use crate::actor::skin;
use crate::actor::{
    ActorSkin, ActorTint, DummyBehavior, Facing, Health, Intent, MAX_PLAYERS, Player, Pose,
    SkinSelections, TrainingDummy,
};
use crate::ascii::{Accent, AsciiArt, AsciiSprite, Layer, palette};
use crate::combat::{ComboMeter, MATCH_WINS, RoundResult, ShowBoxes};
use crate::level::{CATALOG as LEVEL_CATALOG, LevelPick, level_name};
use crate::online::{LobbyCommand, OnlineSession};
use crate::state::{AppSet, GameMode, GameState};
use crate::weapon::{ARSENAL, GroundWeapon, Held, spawn_ground_weapon, weapon_at};

include!("widgets.rs");
include!("screens/menu.rs");
include!("screens/lobby.rs");
include!("screens/fighter_select.rs");
include!("screens/round_menu.rs");
include!("hud.rs");
include!("screens/training.rs");
include!("plugin.rs");

#[cfg(test)]
mod tests;
