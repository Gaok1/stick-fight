//! Lobby e transporte P2P pela Steam.
//!
//! A Steam resolve convite, NAT/relay e entrega dos pacotes. O dono do lobby
//! e a autoridade da partida; os outros jogadores enviam entrada e recebem
//! pequenas correcoes do estado dos lutadores.
//!
//! A sala vale de dois a quatro. A topologia e uma estrela em volta do dono:
//! cada cliente fala so com ele, e ele responde a todos. Numa malha de quatro
//! seriam doze ligacoes trocando entrada; aqui sao tres de subida e tres de
//! descida, e existe um so lugar onde o estado da partida e decidido.
//!
//! # O que viaja, com que garantia e a que ritmo
//!
//! Nada aqui e enviado uma vez por quadro. Um jogo a 144 fps mandaria 144
//! pacotes por segundo por adversario, e o gargalo deixava de ser a rede e
//! passava a ser a fila: era dai que vinha a sensacao de atraso mesmo entre
//! duas maquinas na mesma casa.
//!
//! - **Entrada** ([`INPUT_HZ`]): sem garantia de entrega. Um pacote perdido nao
//!   pode custar um pulo, entao o que viaja nao e "pulei neste quadro" e sim
//!   *quantas vezes* cada botao foi apertado ate agora. O outro lado aplica a
//!   diferenca: perder um pacote so adia a leitura, e receber o mesmo duas
//!   vezes nao faz o boneco pular duas.
//! - **Estado dos lutadores** ([`SNAPSHOT_HZ`]): sem garantia. Cada maquina
//!   simula a fisica inteira e o snapshot so corrige o desvio -- e corrige
//!   puxando, nao teletransportando.
//! - **Armas** ([`WEAPON_HZ`]): sem garantia, mas descrevendo a arena inteira.
//!   Assim um pacote perdido se conserta sozinho no seguinte.
//! - **Inicio de luta, fim de round e pele**: com garantia. Sao decisoes unicas
//!   -- perder uma delas trava a partida.

use std::sync::{Arc, Mutex, mpsc};

use bevy::prelude::*;
use steamworks::networking_types::{NetworkingIdentity, SendFlags};
use steamworks::{
    CallbackHandle, Client, GameLobbyJoinRequested, LobbyChatUpdate, LobbyDataUpdate, LobbyId,
    LobbyKey, LobbyType, SteamId, StringFilter, StringFilterKind,
};

use crate::actor::face::{FACE_BYTES, Face};
use crate::actor::input::{InputSource, Sense};
use crate::actor::skin;
use crate::actor::{Facing, Health, Intent, MAX_PLAYERS, MIN_PLAYERS, Player, SkinSelections};
use crate::combat::RoundResult;
use crate::level::{CATALOG as LEVEL_CATALOG, LevelPick};
use crate::physics::Velocity;
use crate::state::{AppSet, GameMode, GameState, arena_live};
use crate::weapon::{GroundWeapon, Held, NetWeapon, ThrownWeapon};

include!("types.rs");
include!("lobby/session.rs");
include!("transport/steam.rs");
include!("lobby/systems.rs");
include!("transport/receive.rs");
include!("gameplay/sync.rs");
include!("transport/protocol.rs");
include!("plugin.rs");

#[cfg(test)]
mod tests;
