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

/// Onde a rede entrega os pacotes do quadro.
///
/// Existe para quem precisa correr *depois* de os pacotes chegarem e *antes* da
/// troca de estado do Bevy poder dizer isso sem enxergar os sistemas por dentro
/// -- e a fase escolhida pelo dono e exatamente esse caso.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NetReceive;

const CHANNEL: u32 = 0;
/// Muda junto com o formato dos pacotes: uma sala de outra versao nao pode ser
/// encontrada por esta, porque os dois lados leriam bytes diferentes.
const GAME_TAG: &str = "stick-fightt-v3";

/// Cliente -> dono: a entrada deste cliente.
const PACKET_INPUT: u8 = 1;
const PACKET_START: u8 = 2;
const PACKET_SNAPSHOT: u8 = 3;
const PACKET_ROUND_OVER: u8 = 4;
/// Cliente -> dono: a pele escolhida.
const PACKET_SKIN: u8 = 5;
/// Dono -> clientes: a entrada de todos os lugares.
///
/// O snapshot corrige posicao, mas quem escolhe a pose de um boneco e a
/// intencao dele. Sem devolver as entradas, os adversarios deslizariam pela
/// arena parados na pose de descanso.
const PACKET_INPUTS: u8 = 6;
/// Dono -> clientes: as armas da arena e o que cada um tem na mao.
const PACKET_WEAPONS: u8 = 7;

/// Envios de entrada por segundo.
const INPUT_HZ: f32 = 60.0;
/// Snapshots por segundo.
const SNAPSHOT_HZ: f32 = 30.0;
/// Pacotes de arma por segundo. Arma quase sempre esta parada no chao, entao
/// ela nao precisa do ritmo de um corpo em movimento.
const WEAPON_HZ: f32 = 12.0;
/// Segundos entre duas releituras da sala.
///
/// A Steam avisa por callback, mas o aviso pode chegar antes de o dono ter
/// publicado a tabela de lugares. Sem esta rede de seguranca, quem entrava
/// ficava com a sala vazia na tela ate a proxima pessoa chegar.
const LOBBY_POLL: f32 = 0.4;

/// Bytes de um lutador dentro do snapshot: posicao, velocidade, lado e vida.
const ACTOR_BYTES: usize = 24;
/// Bytes de uma intencao codificada.
const INTENT_BYTES: usize = 12;
/// Botoes que valem por aperto, e nao por estar segurados.
const PULSES: usize = 6;
/// Bytes de uma arma caida dentro do pacote de armas.
const GROUND_BYTES: usize = 22;
/// Teto de armas descritas num pacote. A arena nunca chega perto disso; o
/// limite existe para um pacote corrompido nao virar uma alocacao enorme.
const MAX_GROUND: usize = 32;

/// Erro de posicao acima do qual a correcao vira teletransporte.
///
/// Abaixo dele o cliente e puxado aos poucos: cravar a posicao do dono a cada
/// snapshot faz o boneco tremer, porque o que chega descreve o passado.
const SNAP_DIST: f32 = 190.0;
/// Quanto de um erro pequeno cada snapshot corrige, num boneco dos outros.
const CORRECTION: f32 = 0.45;
/// O mesmo para o proprio boneco.
///
/// Bem mais fraco: aqui o teclado ja mexeu nele neste quadro, e puxar com forca
/// para onde o dono achava que ele estava um instante atras e exatamente o
/// efeito elastico de andar e ser jogado de volta.
const LOCAL_CORRECTION: f32 = 0.12;
const LOCAL_SNAP_DIST: f32 = 260.0;
/// Diferenca de velocidade que so um empurrao explica -- soco, estouro, queda
/// de mare. O proprio boneco aceita a velocidade do dono nesses casos, senao
/// o knockback nao chegaria em quem levou.
const SHOVE_SPEED: f32 = 220.0;

/// O que o menu e o lobby mandam a rede fazer.
///
/// Existe para a tela nao precisar saber de Steam: o clique do mouse e a tecla
/// escrevem a mesma mensagem, e um so lugar sabe o que cada uma significa.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum LobbyCommand {
    /// Abrir uma sala nova.
    Create,
    /// Procurar uma sala aberta.
    Find,
    /// Abrir o convite da Steam.
    Invite,
    /// Comecar a luta (so o dono).
    Start,
    /// Sair da sala.
    Leave,
}

/// Contadores de aperto, um por botao de pulso.
///
/// A ordem e a mesma de [`RemoteState::take_pulse`] e de [`bump_presses`]; ela
/// so existe nos dois lados do fio, entao mudar um sem o outro troca pulo por
/// soco.
type Presses = [u8; PULSES];

#[derive(Default)]
struct RemoteState {
    /// Eixo, mira e teclas seguradas: sempre o ultimo valor conhecido.
    latest: Intent,
    /// Apertos ainda nao entregues ao boneco.
    pending: [u8; PULSES],
    /// Ultimo contador visto, para saber quantos apertos sao novos.
    seen: Option<Presses>,
}

impl RemoteState {
    /// Quantos apertos novos este pacote traz, por botao.
    fn absorb(&mut self, presses: Presses) {
        // O primeiro pacote so calibra: tratar o contador inicial como "n
        // apertos de uma vez" faria o boneco entrar na arena socando.
        if let Some(seen) = self.seen {
            for (at, pending) in self.pending.iter_mut().enumerate() {
                *pending = pending.saturating_add(presses[at].wrapping_sub(seen[at]));
            }
        }
        self.seen = Some(presses);
    }

    /// Gasta um aperto do botao `at`, se houver algum guardado.
    fn take_pulse(&mut self, at: usize) -> bool {
        if self.pending[at] == 0 {
            return false;
        }
        self.pending[at] -= 1;
        true
    }
}

/// Fonte de entrada alimentada por um peer da Steam.
///
/// Existe uma por lugar. Enquanto todos os remotos dividiram a mesma, cada
/// pacote que chegava mandava em todos os bonecos de uma vez -- com dois
/// jogadores isso passava por acerto, com quatro vira um bloco andando junto.
#[derive(Clone, Default)]
pub struct RemoteInput(Arc<Mutex<RemoteState>>);

impl RemoteInput {
    fn push(&self, next: Intent, presses: Presses) {
        let mut state = self.0.lock().expect("remote input poisoned");
        state.latest = next;
        state.absorb(presses);
    }

    fn clear(&self) {
        *self.0.lock().expect("remote input poisoned") = RemoteState::default();
    }
}

impl InputSource for RemoteInput {
    fn poll(&self, _keys: &ButtonInput<KeyCode>, _sense: &Sense) -> Intent {
        let mut state = self.0.lock().expect("remote input poisoned");
        let mut intent = state.latest;
        intent.jump = state.take_pulse(0);
        intent.attack = state.take_pulse(1);
        intent.special = state.take_pulse(2);
        intent.parry = state.take_pulse(3);
        intent.grapple = state.take_pulse(4);
        intent.throw_weapon = state.take_pulse(5);
        intent
    }
}

/// Soma um aperto a cada botao que disparou neste quadro.
fn bump_presses(presses: &mut Presses, intent: &Intent) {
    for (at, pressed) in [
        intent.jump,
        intent.attack,
        intent.special,
        intent.parry,
        intent.grapple,
        intent.throw_weapon,
    ]
    .into_iter()
    .enumerate()
    {
        if pressed {
            presses[at] = presses[at].wrapping_add(1);
        }
    }
}

/// Estado de todos os lugares num quadro.
type Snapshot = [Option<ActorSnapshot>; MAX_PLAYERS];

/// Estado visivel do lobby. Continua existindo quando a Steam nao esta aberta,
/// para o menu conseguir explicar o problema sem derrubar o jogo local.
#[derive(Resource)]
pub struct OnlineSession {
    pub status: String,
    pub lobby: Option<LobbyId>,
    /// Nome de quem ocupa cada lugar, na ordem dos lugares. Sempre com
    /// `MAX_PLAYERS` entradas; lugar vazio vira traco na tela.
    pub members: Vec<String>,
    /// Quem ocupa cada lugar. O indice e o id do `Player`.
    slots: [Option<SteamId>; MAX_PLAYERS],
    /// Dono da sala, como a Steam o reporta.
    owner: Option<SteamId>,
    /// Este cliente.
    me: Option<SteamId>,
    /// Lugar deste cliente.
    local: u8,
    /// A tabela de lugares ja disse qual e o nosso lugar?
    ///
    /// Cair no lugar zero enquanto ela nao chegou poria este teclado no comando
    /// do boneco do dono -- e era o que acontecia com quem acabava de entrar.
    seated: bool,
    /// Lugares em jogo na luta corrente.
    ///
    /// Congelado no inicio da luta de proposito: se ele mudasse no meio de um
    /// round, o snapshot trocaria de tamanho no meio do voo e o placar ficaria
    /// sem base. Quem chega entre rounds entra na luta seguinte.
    seats: u8,
    host: bool,
    /// Fase publicada pelo dono, para a sala inteira aquecer no mesmo mapa.
    stage: Option<usize>,
    remotes: [RemoteInput; MAX_PLAYERS],
    remote_looks: [Option<Look>; MAX_PLAYERS],
    sent_look: Option<Look>,
    pending_snapshot: Option<Snapshot>,
    pending_weapons: Option<WeaponState>,
    /// Apertos do jogador daqui, acumulados desde sempre.
    local_presses: Presses,
    /// O mesmo, por lugar, do lado do dono: e o que ele reenvia a todos.
    table_presses: [Presses; MAX_PLAYERS],
}

impl Default for OnlineSession {
    fn default() -> Self {
        Self {
            status: "STEAM OFFLINE. OPEN STEAM AND RESTART THE GAME.".into(),
            lobby: None,
            members: Vec::new(),
            slots: [None; MAX_PLAYERS],
            owner: None,
            me: None,
            local: 0,
            seated: false,
            seats: MIN_PLAYERS as u8,
            host: false,
            stage: None,
            remotes: Default::default(),
            remote_looks: [None; MAX_PLAYERS],
            sent_look: None,
            pending_snapshot: None,
            pending_weapons: None,
            local_presses: [0; PULSES],
            table_presses: [[0; PULSES]; MAX_PLAYERS],
        }
    }
}

impl OnlineSession {
    pub fn local_player_id(&self) -> u8 {
        self.local
    }

    pub fn is_host(&self) -> bool {
        self.host
    }

    /// Estamos numa sala da Steam agora?
    pub fn in_lobby(&self) -> bool {
        self.lobby.is_some()
    }

    /// A sala ja disse qual e o nosso lugar?
    ///
    /// Fora de uma sala a resposta e sim: sem Steam o jogador e o lugar zero e
    /// nada disputa isso com ele.
    pub fn seated(&self) -> bool {
        self.seated || self.lobby.is_none()
    }

    /// Lugares em jogo na luta corrente.
    pub fn seats(&self) -> usize {
        (self.seats as usize).clamp(MIN_PLAYERS, MAX_PLAYERS)
    }

    /// Lugares ocupados no lobby agora -- que nao e a mesma pergunta que
    /// [`Self::seats`]: alguem pode ter entrado depois do inicio da luta.
    pub fn filled(&self) -> usize {
        self.slots.iter().flatten().count()
    }

    /// Este lugar tem alguem nele agora?
    ///
    /// Pergunta do aquecimento, nao da luta: no lobby os bonecos acompanham a
    /// sala quadro a quadro, enquanto na luta a contagem esta congelada.
    pub fn seat_taken(&self, id: u8) -> bool {
        self.slots.get(id as usize).is_some_and(Option::is_some)
    }

    /// O dono da sala pode comecar a luta agora?
    pub fn can_start(&self) -> bool {
        self.host && self.filled() >= MIN_PLAYERS
    }

    pub fn remote_input(&self, id: u8) -> RemoteInput {
        self.remotes[(id as usize).min(MAX_PLAYERS - 1)].clone()
    }

    /// Quantos lugares a partida precisa cobrir: o ultimo ocupado, mais um.
    ///
    /// Nao e a mesma coisa que [`Self::filled`]. Com o lugar do meio vago --
    /// alguem saiu e a tabela nao renumera ninguem -- contar ocupados daria
    /// tres, e o jogador do quarto lugar entraria na luta sem boneco. O lugar
    /// vago nasce e sai na hora, pela mesma regra de quem desiste.
    fn span(&self) -> usize {
        self.slots
            .iter()
            .rposition(|slot| slot.is_some())
            .map_or(0, |at| at + 1)
    }

    /// Todo mundo na sala menos este cliente.
    fn peers(&self) -> Vec<SteamId> {
        self.slots
            .iter()
            .flatten()
            .copied()
            .filter(|who| Some(*who) != self.me)
            .collect()
    }

    fn slot_of(&self, who: SteamId) -> Option<u8> {
        self.slots
            .iter()
            .position(|slot| *slot == Some(who))
            .map(|at| at as u8)
    }

    fn clear_remotes(&self) {
        for remote in &self.remotes {
            remote.clear();
        }
    }

    /// Esquece tudo que veio da sala anterior.
    fn forget_room(&mut self) {
        self.slots = [None; MAX_PLAYERS];
        self.owner = None;
        self.local = 0;
        self.seated = false;
        self.stage = None;
        self.members.clear();
        self.remote_looks = [None; MAX_PLAYERS];
        self.sent_look = None;
        self.pending_snapshot = None;
        self.pending_weapons = None;
        self.table_presses = [[0; PULSES]; MAX_PLAYERS];
        self.clear_remotes();
    }
}

#[derive(Clone, Copy, Default)]
struct ActorSnapshot {
    at: Vec2,
    velocity: Vec2,
    hp: i32,
    facing: f32,
}

/// Uma arma caida, como ela viaja.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct GroundState {
    net: u16,
    kind: u8,
    ammo: u32,
    at: Vec2,
    velocity: Vec2,
    /// Ainda esta no ar depois de um arremesso.
    thrown: bool,
}

/// A arma na mao de um lugar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct HeldState {
    kind: u8,
    ammo: u32,
}

/// Todo o armamento da arena num instante.
///
/// Viaja inteiro, e nao como "caiu uma arma ali" / "fulano pegou aquela": um
/// aviso perdido deixaria as duas telas discordando para sempre, enquanto um
/// retrato perdido se conserta no proximo. Era exatamente disso que vinham a
/// arma que so um lado enxergava e a que ninguem conseguia pegar.
#[derive(Clone, Debug, Default, PartialEq)]
struct WeaponState {
    held: [Option<HeldState>; MAX_PLAYERS],
    ground: Vec<GroundState>,
}

enum SteamEvent {
    Created(Result<LobbyId, String>),
    Joined(Result<LobbyId, String>),
    Found(Result<Vec<LobbyId>, String>),
    JoinRequested(LobbyId),
    /// A sala mudou: alguem entrou, alguem saiu, ou os dados dela foram
    /// reescritos. As tres coisas pedem a mesma releitura.
    RoomChanged(LobbyId),
}

#[derive(Resource)]
struct SteamRuntime {
    client: Client,
    tx: mpsc::Sender<SteamEvent>,
    rx: Mutex<mpsc::Receiver<SteamEvent>>,
    _callbacks: Vec<CallbackHandle>,
}

fn init_steam(mut commands: Commands) {
    let Ok(client) = Client::init() else {
        commands.insert_resource(OnlineSession::default());
        return;
    };

    let (tx, rx) = mpsc::channel();
    let invite_tx = tx.clone();
    let invite = client.register_callback(move |event: GameLobbyJoinRequested| {
        let _ = invite_tx.send(SteamEvent::JoinRequested(event.lobby_steam_id));
    });
    let members_tx = tx.clone();
    let members = client.register_callback(move |event: LobbyChatUpdate| {
        let _ = members_tx.send(SteamEvent::RoomChanged(event.lobby));
    });
    // A tabela de lugares e a fase moram nos dados da sala, e mudar dados nao
    // dispara `LobbyChatUpdate`. Sem este callback, quem entrava so descobria o
    // proprio lugar quando a **proxima** pessoa entrasse ou saisse -- ate la
    // jogava no boneco de outro, sem mapa e sem placar.
    let data_tx = tx.clone();
    let data = client.register_callback(move |event: LobbyDataUpdate| {
        if event.success {
            let _ = data_tx.send(SteamEvent::RoomChanged(event.lobby));
        }
    });

    client
        .networking_messages()
        .session_request_callback(|request| {
            request.accept();
        });

    let name = client.friends().name();
    commands.insert_resource(OnlineSession {
        status: format!("CONNECTED TO STEAM AS {name}"),
        ..default()
    });
    commands.insert_resource(SteamRuntime {
        client,
        tx,
        rx: Mutex::new(rx),
        _callbacks: vec![invite, members, data],
    });
}

fn pump_callbacks(runtime: Option<Res<SteamRuntime>>) {
    if let Some(runtime) = runtime {
        runtime.client.run_callbacks();
    }
}

fn create_lobby(runtime: &SteamRuntime, session: &mut OnlineSession) {
    session.status = "CREATING PUBLIC LOBBY...".into();
    let tx = runtime.tx.clone();
    runtime.client.matchmaking().create_lobby(
        LobbyType::Public,
        MAX_PLAYERS as u32,
        move |result| {
            let _ = tx.send(SteamEvent::Created(
                result.map_err(|error| format!("{error:?}")),
            ));
        },
    );
}

fn join_lobby(runtime: &SteamRuntime, session: &mut OnlineSession, lobby: LobbyId) {
    if let Some(current) = session.lobby.take() {
        runtime.client.matchmaking().leave_lobby(current);
    }
    session.forget_room();
    session.status = "JOINING LOBBY...".into();
    let tx = runtime.tx.clone();
    runtime
        .client
        .matchmaking()
        .join_lobby(lobby, move |result| {
            let _ = tx.send(SteamEvent::Joined(
                result.map_err(|_| "LOBBY REFUSED THE CONNECTION".into()),
            ));
        });
}

fn find_lobby(runtime: &SteamRuntime, session: &mut OnlineSession) {
    session.status = "SEARCHING OPEN LOBBIES...".into();
    let tx = runtime.tx.clone();
    runtime
        .client
        .matchmaking()
        .add_request_lobby_list_string_filter(StringFilter(
            LobbyKey::new("game"),
            GAME_TAG,
            StringFilterKind::Equal,
        ))
        .set_request_lobby_list_slots_available_filter(1)
        .request_lobby_list(move |result| {
            let _ = tx.send(SteamEvent::Found(
                result.map_err(|error| format!("{error:?}")),
            ));
        });
}

/// Nome da chave onde a tabela de lugares e publicada.
fn slot_key(at: usize) -> String {
    format!("slot{at}")
}

/// Chave da fase escolhida pelo dono.
const STAGE_KEY: &str = "stage";

/// Encaixa os membros da sala na tabela de lugares.
///
/// Quem ja tem um lugar fica nele. Compactar a tabela a cada saida
/// renumeraria os sobreviventes no meio da luta -- trocaria a cor, a linha do
/// placar e o boneco de quem nao fez nada -- e faria a entrada de saida
/// apontar para o alvo errado.
fn assign_slots(
    current: [Option<SteamId>; MAX_PLAYERS],
    owner: SteamId,
    members: &[SteamId],
) -> [Option<SteamId>; MAX_PLAYERS] {
    let mut slots = current;

    // Quem saiu abre o lugar.
    for slot in slots.iter_mut() {
        if !slot.is_some_and(|who| members.contains(&who)) {
            *slot = None;
        }
    }
    // O primeiro lugar e do dono: e dele que sai a autoridade da partida.
    if slots[0] != Some(owner) {
        for slot in slots.iter_mut() {
            if *slot == Some(owner) {
                *slot = None;
            }
        }
        slots[0] = Some(owner);
    }
    // Quem chegou ocupa a primeira vaga livre.
    let newcomers: Vec<SteamId> = members
        .iter()
        .copied()
        .filter(|member| !slots.contains(&Some(*member)))
        .collect();
    for member in newcomers {
        let Some(free) = slots.iter_mut().find(|slot| slot.is_none()) else {
            break;
        };
        *free = Some(member);
    }
    slots
}

fn refresh_members(runtime: &SteamRuntime, session: &mut OnlineSession) {
    let Some(lobby) = session.lobby else {
        return;
    };
    let matchmaking = runtime.client.matchmaking();
    let me = runtime.client.user().steam_id();
    let owner = matchmaking.lobby_owner(lobby);
    let before = session.slots;

    session.me = Some(me);
    session.owner = Some(owner);
    session.host = owner == me;

    // O dono decide os lugares e publica a tabela; a Steam a replica para
    // todos. Se cada cliente ordenasse `lobby_members` por conta propria, dois
    // deles poderiam discordar de quem e o jogador 3 -- essa ordem nao e
    // prometida igual em todo mundo.
    if session.host {
        let members = matchmaking.lobby_members(lobby);
        let slots = assign_slots(session.slots, owner, &members);
        session.slots = slots;
        for (at, slot) in slots.iter().enumerate() {
            matchmaking.set_lobby_data(
                lobby,
                &slot_key(at),
                &slot.map_or(String::new(), |who| who.raw().to_string()),
            );
        }
    } else {
        for at in 0..MAX_PLAYERS {
            session.slots[at] = matchmaking
                .lobby_data(lobby, &slot_key(at))
                .and_then(|raw| raw.parse::<u64>().ok())
                .map(SteamId::from_raw);
        }
        session.stage = matchmaking
            .lobby_data(lobby, STAGE_KEY)
            .and_then(|raw| raw.parse::<usize>().ok())
            .map(|stage| stage % LEVEL_CATALOG.len());
    }

    // So mexe no proprio lugar quando a tabela realmente diz qual e. Cair no
    // lugar zero enquanto a copia do cliente ainda nao replicou o poria no
    // comando do boneco do dono por alguns quadros.
    if let Some(slot) = session.slot_of(me) {
        session.local = slot;
        session.seated = true;
    }

    // Quem sai leva a escolha dele junto: reaproveitar a pele anterior
    // pintaria o proximo a ocupar o lugar com o gosto de outra pessoa.
    for (slot, pick) in session.remote_looks.iter_mut().enumerate() {
        if before[slot] != session.slots[slot] {
            *pick = None;
        }
    }
    for (slot, remote) in session.remotes.iter().enumerate() {
        if session.slots[slot].is_none() {
            remote.clear();
        }
    }
    if before != session.slots {
        session.sent_look = None;
    }

    session.members = session
        .slots
        .iter()
        .map(|slot| {
            slot.map_or_else(
                || "---".to_string(),
                |who| runtime.client.friends().get_friend(who).name(),
            )
        })
        .collect();

    let filled = session.filled();
    session.status = if !session.seated {
        "TAKING A SEAT IN THE ROOM...".into()
    } else if filled < MIN_PLAYERS {
        "WAITING FOR PLAYERS. INVITE A FRIEND.".into()
    } else if session.host {
        format!("{filled}/{MAX_PLAYERS} IN THE ROOM. READY WHEN YOU ARE.")
    } else {
        format!("{filled}/{MAX_PLAYERS} IN THE ROOM. WAITING FOR THE HOST...")
    };
}

/// Rele a sala de tempos em tempos.
///
/// Callback nao e garantia de ordem: o aviso de que alguem entrou pode chegar
/// antes de a tabela de lugares desse alguem ter sido publicada. Esta releitura
/// e barata (le memoria local da Steam) e fecha essa janela.
fn poll_lobby(
    time: Res<Time>,
    runtime: Option<Res<SteamRuntime>>,
    mut session: ResMut<OnlineSession>,
    mut since: Local<f32>,
) {
    let Some(runtime) = runtime else {
        return;
    };
    if session.lobby.is_none() {
        return;
    }
    *since += time.delta_secs();
    if *since < LOBBY_POLL {
        return;
    }
    *since = 0.0;
    refresh_members(&runtime, &mut session);
}

fn handle_events(
    runtime: Option<Res<SteamRuntime>>,
    mut session: ResMut<OnlineSession>,
    mut mode: ResMut<GameMode>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
) {
    let Some(runtime) = runtime else {
        return;
    };
    let events: Vec<_> = runtime
        .rx
        .lock()
        .expect("steam event receiver poisoned")
        .try_iter()
        .collect();

    for event in events {
        match event {
            SteamEvent::Created(Ok(lobby)) => {
                let matchmaking = runtime.client.matchmaking();
                matchmaking.set_lobby_data(lobby, "game", GAME_TAG);
                session.lobby = Some(lobby);
                refresh_members(&runtime, &mut session);
            }
            SteamEvent::Joined(Ok(lobby)) => {
                if runtime
                    .client
                    .matchmaking()
                    .lobby_data(lobby, "game")
                    .as_deref()
                    != Some(GAME_TAG)
                {
                    runtime.client.matchmaking().leave_lobby(lobby);
                    session.status = "THIS LOBBY BELONGS TO ANOTHER GAME/BUILD.".into();
                    continue;
                }
                *mode = GameMode::Online;
                session.lobby = Some(lobby);
                refresh_members(&runtime, &mut session);
                if *state.get() != GameState::Lobby {
                    next.set(GameState::SkinSelect);
                }
            }
            SteamEvent::Found(Ok(lobbies)) => {
                let found = lobbies.into_iter().find(|lobby| {
                    runtime
                        .client
                        .matchmaking()
                        .lobby_data(*lobby, "game")
                        .as_deref()
                        == Some(GAME_TAG)
                        && runtime.client.matchmaking().lobby_member_count(*lobby) < MAX_PLAYERS
                });
                if let Some(lobby) = found {
                    join_lobby(&runtime, &mut session, lobby);
                } else {
                    session.status = "NO OPEN LOBBY FOUND. CREATE ONE.".into();
                }
            }
            SteamEvent::JoinRequested(lobby) => join_lobby(&runtime, &mut session, lobby),
            SteamEvent::RoomChanged(lobby) if session.lobby == Some(lobby) => {
                let owner_before = session.owner;
                refresh_members(&runtime, &mut session);

                // Quem sai no meio da luta so conta como derrotado -- os outros
                // continuam brigando. Mas sem o dono nao ha quem decida o round,
                // e a Steam promove um novo dono que nao tem o estado da
                // partida: nesse caso a luta acaba para todo mundo.
                let fighting = matches!(state.get(), GameState::Fighting | GameState::RoundOver);
                let lost_host = owner_before.is_some() && session.owner != owner_before;
                if fighting && (session.filled() < MIN_PLAYERS || lost_host) {
                    next.set(GameState::Lobby);
                }
            }
            SteamEvent::Created(Err(error))
            | SteamEvent::Joined(Err(error))
            | SteamEvent::Found(Err(error)) => session.status = format!("STEAM ERROR: {error}"),
            SteamEvent::RoomChanged(_) => {}
        }
    }
}

fn leave_lobby(runtime: Option<Res<SteamRuntime>>, mut session: ResMut<OnlineSession>) {
    let Some(runtime) = runtime else {
        return;
    };
    if let Some(lobby) = session.lobby.take() {
        runtime.client.matchmaking().leave_lobby(lobby);
    }
    session.forget_room();
    session.status = format!("CONNECTED TO STEAM AS {}", runtime.client.friends().name());
}

fn send(runtime: &SteamRuntime, peer: SteamId, bytes: &[u8], flags: SendFlags) {
    if let Err(error) = runtime.client.networking_messages().send_message_to_user(
        NetworkingIdentity::new_steam_id(peer),
        flags,
        bytes,
        CHANNEL,
    ) {
        warn!("Steam P2P send failed: {error:?}");
    }
}

/// Manda o mesmo pacote para todos os outros da sala.
fn broadcast(runtime: &SteamRuntime, session: &OnlineSession, bytes: &[u8], flags: SendFlags) {
    for peer in session.peers() {
        send(runtime, peer, bytes, flags);
    }
}

/// Ja passou o intervalo de envio deste canal?
///
/// Um acumulador, e nao um "resetar para zero": com reset, um quadro longo
/// atrasaria o envio seguinte e o ritmo escorregaria junto com o framerate.
fn due(since: &mut f32, dt: f32, hz: f32) -> bool {
    *since += dt;
    let step = 1.0 / hz;
    if *since < step {
        return false;
    }
    // Descontar em vez de zerar mantem a media no ritmo pedido, mas sem deixar
    // uma pausa longa virar uma rajada de pacotes.
    *since = (*since - step).min(step);
    true
}

/// Executa o que o menu pediu.
fn run_lobby_commands(
    mut commands: MessageReader<LobbyCommand>,
    runtime: Option<Res<SteamRuntime>>,
    mut session: ResMut<OnlineSession>,
    pick: Res<LevelPick>,
    skins: Res<SkinSelections>,
    mut next: ResMut<NextState<GameState>>,
) {
    let Some(runtime) = runtime else {
        // Sem Steam nao ha o que fazer, mas sair tem que continuar saindo.
        for command in commands.read() {
            if *command == LobbyCommand::Leave {
                next.set(GameState::Controls);
            }
        }
        return;
    };

    for command in commands.read() {
        match command {
            LobbyCommand::Create if session.lobby.is_none() => {
                create_lobby(&runtime, &mut session)
            }
            LobbyCommand::Find if session.lobby.is_none() => find_lobby(&runtime, &mut session),
            LobbyCommand::Invite => {
                if let Some(lobby) = session.lobby {
                    runtime.client.friends().activate_invite_dialog(lobby);
                }
            }
            // Basta ter com quem lutar. Esperar a pele de todo mundo chegar
            // deixaria a sala travada por um pacote perdido, e o pacote de
            // inicio ja carrega a tabela inteira de peles de qualquer jeito.
            LobbyCommand::Start if session.can_start() => {
                if let Some(lobby) = session.lobby {
                    runtime
                        .client
                        .matchmaking()
                        .set_lobby_joinable(lobby, false);
                }
                send_start(&runtime, &mut session, pick.0, &skins);
                next.set(GameState::Fighting);
            }
            LobbyCommand::Leave => next.set(GameState::Controls),
            _ => {}
        }
    }
}

/// O dono publica a fase para a sala aquecer no mesmo mapa.
fn publish_stage(
    runtime: Option<Res<SteamRuntime>>,
    session: Res<OnlineSession>,
    pick: Res<LevelPick>,
    mut published: Local<Option<usize>>,
) {
    let Some(runtime) = runtime else {
        return;
    };
    let (true, Some(lobby)) = (session.host, session.lobby) else {
        return;
    };
    if *published == Some(pick.0) {
        return;
    }
    *published = Some(pick.0);
    runtime
        .client
        .matchmaking()
        .set_lobby_data(lobby, STAGE_KEY, &pick.0.to_string());
}

/// O cliente segue a fase do dono enquanto espera na sala.
///
/// Sem isto cada um aquecia num mapa diferente: os bonecos apareciam parados no
/// ar, porque o chao onde o dono os via nao existia na tela de quem entrou.
fn follow_host_stage(session: Res<OnlineSession>, mut pick: ResMut<LevelPick>) {
    if session.host {
        return;
    }
    if let Some(stage) = session.stage
        && pick.0 != stage
    {
        pick.0 = stage;
    }
}

fn reopen_lobby(runtime: Option<Res<SteamRuntime>>, session: Res<OnlineSession>) {
    let Some(runtime) = runtime else {
        return;
    };
    if session.host {
        if let Some(lobby) = session.lobby {
            runtime.client.matchmaking().set_lobby_joinable(lobby, true);
        }
    }
}

fn round_over_controls(
    keys: Res<ButtonInput<KeyCode>>,
    runtime: Option<Res<SteamRuntime>>,
    mut session: ResMut<OnlineSession>,
    pick: Res<LevelPick>,
    skins: Res<SkinSelections>,
    mut next: ResMut<NextState<GameState>>,
) {
    // A mesma exigencia do lobby: sem com quem lutar, o round seguinte abriria
    // com um lugar vazio que morre no ato e cairia direto nesta tela de novo.
    if !session.can_start()
        || !(keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter))
    {
        return;
    }
    let Some(runtime) = runtime else {
        return;
    };
    send_start(&runtime, &mut session, pick.0, &skins);
    next.set(GameState::Fighting);
}

/// Abre a luta: congela quantos lugares valem e conta a cada cliente qual e o
/// dele.
///
/// O lugar vai no pacote em vez de sair da tabela do lobby porque a replicacao
/// da tabela e assincrona: um cliente que acabou de entrar poderia comecar a
/// luta sem saber ainda quem ele e.
fn send_start(
    runtime: &SteamRuntime,
    session: &mut OnlineSession,
    stage: usize,
    skins: &SkinSelections,
) {
    session.clear_remotes();
    session.table_presses = [[0; PULSES]; MAX_PLAYERS];
    session.seats = session.span().max(MIN_PLAYERS) as u8;
    let seats = session.seats;
    let mut looks = [Look::default(); MAX_PLAYERS];
    for (at, look) in looks.iter_mut().enumerate() {
        *look = Look {
            skin: skins.players[at],
            face: skins.faces[at],
        };
    }
    for slot in 0..MAX_PLAYERS {
        let Some(peer) = session.slots[slot] else {
            continue;
        };
        if Some(peer) == session.me {
            continue;
        }
        send(
            runtime,
            peer,
            &encode_start(stage, seats, slot as u8, looks),
            SendFlags::RELIABLE_NO_NAGLE,
        );
    }
}

/// Cabecalho do pacote de inicio: tipo, fase, lugares e o lugar do destinatario.
const START_HEAD: usize = 4;
/// Bytes de aparencia por lugar: a pele mais as pecas do rosto.
const LOOK_BYTES: usize = 1 + FACE_BYTES;
/// Tamanho do pacote de inicio.
const START_BYTES: usize = START_HEAD + MAX_PLAYERS * LOOK_BYTES;

/// A aparencia escolhida por um lugar: pele e rosto viajam juntos porque sao a
/// mesma pergunta, e separa-los deixaria um chegar sem o outro.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Look {
    skin: usize,
    face: Face,
}

fn encode_start(
    stage: usize,
    seats: u8,
    slot: u8,
    looks: [Look; MAX_PLAYERS],
) -> [u8; START_BYTES] {
    let mut out = [0; START_BYTES];
    out[0] = PACKET_START;
    out[1] = stage as u8;
    out[2] = seats;
    out[3] = slot;
    for (at, look) in looks.iter().enumerate() {
        let cell = &mut out[START_HEAD + at * LOOK_BYTES..][..LOOK_BYTES];
        cell[0] = look.skin as u8;
        cell[1..].copy_from_slice(&look.face.to_bytes());
    }
    out
}

fn decode_start(data: &[u8]) -> Option<(usize, u8, u8, [Look; MAX_PLAYERS])> {
    if data.len() < START_BYTES || data[0] != PACKET_START {
        return None;
    }
    let mut looks = [Look::default(); MAX_PLAYERS];
    for (at, look) in looks.iter_mut().enumerate() {
        let cell = &data[START_HEAD + at * LOOK_BYTES..][..LOOK_BYTES];
        look.skin = cell[0] as usize % skin::CATALOG.len();
        look.face = Face::from_bytes(&cell[1..]);
    }
    Some((
        data[1] as usize,
        (data[2] as usize).clamp(MIN_PLAYERS, MAX_PLAYERS) as u8,
        data[3].min(MAX_PLAYERS as u8 - 1),
        looks,
    ))
}

fn receive_packets(
    runtime: Option<Res<SteamRuntime>>,
    mut session: ResMut<OnlineSession>,
    mut pick: ResMut<LevelPick>,
    mut result: ResMut<RoundResult>,
    mut skins: ResMut<SkinSelections>,
    mut next: ResMut<NextState<GameState>>,
) {
    let Some(runtime) = runtime else {
        return;
    };
    for message in runtime
        .client
        .networking_messages()
        .receive_messages_on_channel(CHANNEL, 64)
    {
        let Some(sender) = message.identity_peer().steam_id() else {
            continue;
        };
        // Quem nao esta na sala nao fala. O dono e aceito mesmo sem lugar
        // conhecido: a tabela dele e que manda, e a copia deste cliente pode
        // estar um instante atrasada quando alguem acabou de entrar.
        let slot = session.slot_of(sender);
        let from_owner = session.owner == Some(sender);
        if slot.is_none() && !from_owner {
            continue;
        }

        let data = message.data();
        match data.first().copied() {
            Some(PACKET_INPUT) if session.host => {
                if let (Some(slot), Some((intent, presses))) =
                    (slot, decode_intent(data.get(1..).unwrap_or_default()))
                {
                    session.remotes[slot as usize].push(intent, presses);
                }
            }
            Some(PACKET_INPUTS) if !session.host => {
                apply_input_table(&session, data);
            }
            Some(PACKET_START) if !session.host => {
                let Some((stage, seats, slot, selected)) = decode_start(data) else {
                    continue;
                };
                session.clear_remotes();
                session.seats = seats;
                session.local = slot;
                session.seated = true;
                session.pending_weapons = None;
                session.pending_snapshot = None;
                pick.0 = stage % LEVEL_CATALOG.len();
                for (at, look) in selected.iter().enumerate() {
                    skins.players[at] = look.skin;
                    skins.faces[at] = look.face;
                }
                next.set(GameState::Fighting);
            }
            Some(PACKET_SNAPSHOT) if !session.host => {
                session.pending_snapshot = decode_snapshot(data);
            }
            Some(PACKET_WEAPONS) if !session.host => {
                session.pending_weapons = decode_weapons(data);
            }
            Some(PACKET_ROUND_OVER) if !session.host => {
                if let Some(decoded) = decode_round_over(data) {
                    *result = decoded;
                    next.set(GameState::RoundOver);
                }
            }
            Some(PACKET_SKIN) if session.host && data.len() > LOOK_BYTES => {
                if let Some(slot) = slot {
                    let chosen = Look {
                        skin: data[1] as usize % skin::CATALOG.len(),
                        face: Face::from_bytes(&data[2..]),
                    };
                    session.remote_looks[slot as usize] = Some(chosen);
                    skins.players[slot as usize] = chosen.skin;
                    skins.faces[slot as usize] = chosen.face;
                }
            }
            _ => {}
        }
    }
}

/// Publica a escolha local para o dono e aplica o que ja se sabe dos outros.
///
/// So o dono junta as escolhas; ele as devolve para todos no pacote de inicio.
/// Assim ninguem precisa falar com ninguem alem dele.
fn sync_skin_choice(
    runtime: Option<Res<SteamRuntime>>,
    mut session: ResMut<OnlineSession>,
    mut skins: ResMut<SkinSelections>,
) {
    let local = session.local_player_id() as usize;
    let selected = Look {
        skin: skins.online_local % skin::CATALOG.len(),
        face: skins.online_face,
    };
    skins.players[local] = selected.skin;
    skins.faces[local] = selected.face;
    for (slot, known) in session.remote_looks.iter().enumerate() {
        if slot != local
            && let Some(known) = known
        {
            skins.players[slot] = known.skin;
            skins.faces[slot] = known.face;
        }
    }

    if session.host {
        return;
    }
    let (Some(runtime), Some(owner)) = (runtime, session.owner) else {
        return;
    };
    if session.sent_look == Some(selected) {
        return;
    }
    let mut packet = [0; 1 + LOOK_BYTES];
    packet[0] = PACKET_SKIN;
    packet[1] = selected.skin as u8;
    packet[2..].copy_from_slice(&selected.face.to_bytes());
    send(&runtime, owner, &packet, SendFlags::RELIABLE_NO_NAGLE);
    session.sent_look = Some(selected);
}

/// Quem saiu da sala para de lutar.
///
/// Zerar a vida em vez de tirar o boneco da arena faz o round terminar pela
/// regra de sempre -- sobrou um -- sem precisar de um caso especial para
/// desistencia.
fn retire_missing_players(session: Res<OnlineSession>, mut players: Query<(&Player, &mut Health)>) {
    if !session.host {
        return;
    }
    for (player, mut health) in &mut players {
        if session.slots[player.id as usize].is_none() && !health.is_dead() {
            health.hp = 0;
        }
    }
}

/// Cliente -> dono: so a propria entrada.
fn send_local_input(
    time: Res<Time>,
    runtime: Option<Res<SteamRuntime>>,
    mut session: ResMut<OnlineSession>,
    mut since: Local<f32>,
    players: Query<(&Player, &Intent)>,
) {
    if session.host {
        return;
    }
    let local = session.local_player_id();
    let Some((_, intent)) = players.iter().find(|(player, _)| player.id == local) else {
        return;
    };
    // Contar todo quadro e enviar de vez em quando: e o contador que faz a
    // entrada sobreviver a um pacote perdido, entao ele nao pode pular os
    // quadros em que nao houve envio -- seria justamente o aperto perdido.
    let intent = *intent;
    bump_presses(&mut session.local_presses, &intent);

    let (Some(runtime), Some(owner)) = (runtime, session.owner) else {
        return;
    };
    if !due(&mut since, time.delta_secs(), INPUT_HZ) {
        return;
    }
    let mut packet = Vec::with_capacity(1 + INTENT_BYTES);
    packet.push(PACKET_INPUT);
    packet.extend(encode_intent(&intent, session.local_presses));
    send(&runtime, owner, &packet, SendFlags::UNRELIABLE_NO_DELAY);
}

/// Dono -> clientes: a entrada de todos os lugares, para os bonecos remotos
/// animarem em vez de deslizarem parados.
fn broadcast_inputs(
    time: Res<Time>,
    runtime: Option<Res<SteamRuntime>>,
    mut session: ResMut<OnlineSession>,
    mut since: Local<f32>,
    players: Query<(&Player, &Intent)>,
) {
    if !session.host {
        return;
    }
    let mut table: [Option<Intent>; MAX_PLAYERS] = [None; MAX_PLAYERS];
    for (player, intent) in &players {
        if let Some(slot) = table.get_mut(player.id as usize) {
            *slot = Some(*intent);
        }
        if let Some(presses) = session.table_presses.get_mut(player.id as usize) {
            bump_presses(presses, intent);
        }
    }
    let Some(runtime) = runtime else {
        return;
    };
    if !due(&mut since, time.delta_secs(), INPUT_HZ) {
        return;
    }
    let packet = encode_input_table(&table, &session.table_presses);
    broadcast(&runtime, &session, &packet, SendFlags::UNRELIABLE_NO_DELAY);
}

fn apply_input_table(session: &OnlineSession, data: &[u8]) {
    let Some(table) = decode_input_table(data) else {
        return;
    };
    for (slot, entry) in table.iter().enumerate() {
        // O proprio boneco anda pelo teclado daqui; o que vem do dono sobre ele
        // e sempre mais velho que o que acabou de ser digitado.
        if slot as u8 == session.local_player_id() {
            continue;
        }
        if let Some((intent, presses)) = entry {
            session.remotes[slot].push(*intent, *presses);
        }
    }
}

fn send_snapshot(
    time: Res<Time>,
    runtime: Option<Res<SteamRuntime>>,
    session: Res<OnlineSession>,
    mut since: Local<f32>,
    players: Query<(&Player, &Transform, &Velocity, &Health, &Facing)>,
) {
    let Some(runtime) = runtime else {
        return;
    };
    if !session.host || !due(&mut since, time.delta_secs(), SNAPSHOT_HZ) {
        return;
    }
    let mut actors: Snapshot = [None; MAX_PLAYERS];
    for (player, transform, velocity, health, facing) in &players {
        if let Some(slot) = actors.get_mut(player.id as usize) {
            *slot = Some(ActorSnapshot {
                at: transform.translation.truncate(),
                velocity: velocity.0,
                hp: health.hp,
                facing: facing.0,
            });
        }
    }
    broadcast(
        &runtime,
        &session,
        &encode_snapshot(&actors),
        SendFlags::UNRELIABLE_NO_DELAY,
    );
}

/// Costura o estado do dono no que este cliente ja simulou.
///
/// Cravar a posicao recebida era o que fazia os bonecos tremerem: o pacote
/// descreve o passado, e o proprio jogador via o boneco andar e ser puxado de
/// volta a cada quadro. Aqui a fisica local continua valendo e o snapshot so
/// puxa o erro -- de uma vez quando ele e grande demais para ser erro, aos
/// poucos quando cabe numa correcao.
fn apply_snapshot(
    mut session: ResMut<OnlineSession>,
    mut players: Query<(
        &Player,
        &mut Transform,
        &mut Velocity,
        &mut Health,
        &mut Facing,
    )>,
) {
    if session.host {
        session.pending_snapshot = None;
        return;
    }
    let local = session.local_player_id();
    let Some(snapshot) = session.pending_snapshot.take() else {
        return;
    };
    for (player, mut transform, mut velocity, mut health, mut facing) in &mut players {
        let Some(actor) = snapshot.get(player.id as usize).copied().flatten() else {
            continue;
        };
        let mine = player.id == local;
        let (limit, pull) = if mine {
            (LOCAL_SNAP_DIST, LOCAL_CORRECTION)
        } else {
            (SNAP_DIST, CORRECTION)
        };

        let at = transform.translation.truncate();
        let error = actor.at - at;
        let next = if error.length() > limit {
            actor.at
        } else {
            at + error * pull
        };
        transform.translation.x = next.x;
        transform.translation.y = next.y;

        // O proprio boneco so aceita a velocidade do dono quando ela nao tem
        // como ter saido do teclado daqui: e assim que o knockback chega em
        // quem apanhou sem que andar de lado vire uma briga entre os dois.
        if !mine || (actor.velocity - velocity.0).length() > SHOVE_SPEED {
            velocity.0 = actor.velocity;
        }
        if !mine && facing.0 != actor.facing {
            facing.0 = actor.facing;
        }
        if health.hp != actor.hp {
            health.hp = actor.hp;
        }
    }
}

/// Dono -> clientes: as armas da arena.
fn broadcast_weapons(
    time: Res<Time>,
    runtime: Option<Res<SteamRuntime>>,
    session: Res<OnlineSession>,
    mut since: Local<f32>,
    holders: Query<(&Player, Option<&Held>)>,
    drops: Query<(&NetWeapon, &GroundWeapon, &Transform, &Velocity, Has<ThrownWeapon>)>,
) {
    let Some(runtime) = runtime else {
        return;
    };
    if !session.host || !due(&mut since, time.delta_secs(), WEAPON_HZ) {
        return;
    }
    let mut state = WeaponState::default();
    for (player, held) in &holders {
        if let Some(slot) = state.held.get_mut(player.id as usize) {
            *slot = held.map(|held| HeldState {
                kind: held.kind,
                ammo: held.ammo,
            });
        }
    }
    for (net, ground, transform, velocity, thrown) in &drops {
        if state.ground.len() == MAX_GROUND {
            break;
        }
        state.ground.push(GroundState {
            net: net.0,
            kind: ground.kind,
            ammo: ground.ammo,
            at: transform.translation.truncate(),
            velocity: velocity.0,
            thrown,
        });
    }
    broadcast(
        &runtime,
        &session,
        &encode_weapons(&state),
        SendFlags::UNRELIABLE_NO_DELAY,
    );
}

/// Poe a arena do cliente de acordo com o retrato que o dono mandou.
fn apply_weapon_state(
    mut commands: Commands,
    state_now: Res<State<GameState>>,
    mut session: ResMut<OnlineSession>,
    mut drops: Query<(
        Entity,
        &NetWeapon,
        &mut GroundWeapon,
        &mut Transform,
        &mut Velocity,
    )>,
    mut holders: Query<(Entity, &Player, &Facing, Option<&mut Held>)>,
) {
    // Fora da luta nao existe arma para replicar, e um retrato atrasado
    // chegando na espera armaria a sala inteira do nada.
    if session.host || *state_now.get() != GameState::Fighting {
        session.pending_weapons = None;
        return;
    }
    let Some(state) = session.pending_weapons.take() else {
        return;
    };

    let mut known: Vec<u16> = Vec::new();
    for (entity, net, mut ground, mut transform, mut velocity) in &mut drops {
        let Some(target) = state.ground.iter().find(|weapon| weapon.net == net.0) else {
            // Sumiu do retrato: alguem pegou, ou ela caiu fora da arena.
            commands.entity(entity).despawn();
            continue;
        };
        known.push(net.0);
        ground.ammo = target.ammo;
        // Uma folga antes de reposicionar: a arma quase sempre esta parada, e
        // reescrever a posicao dela a cada pacote faria o glifo vibrar no chao.
        if transform.translation.truncate().distance(target.at) > 6.0 {
            transform.translation.x = target.at.x;
            transform.translation.y = target.at.y;
        }
        velocity.0 = target.velocity;
    }

    for target in state.ground.iter().filter(|w| !known.contains(&w.net)) {
        let spawned = crate::weapon::spawn_ground_weapon(
            &mut commands,
            Some(target.net),
            target.kind,
            target.ammo,
            target.at,
            target.velocity,
        );
        if target.thrown {
            crate::weapon::mark_thrown(&mut commands, spawned);
        }
    }

    for (entity, player, facing, held) in &mut holders {
        let target = state.held.get(player.id as usize).copied().flatten();
        match (target, held) {
            // Mesma arma: so a municao muda, e trocar o `Held` inteiro
            // reiniciaria a cadencia a cada pacote.
            (Some(want), Some(mut have)) if have.kind == want.kind => {
                if have.ammo != want.ammo {
                    have.ammo = want.ammo;
                }
            }
            // Trocou de arma na mao: sobrescrever em vez de tirar e por de
            // volta, senao o icone que pende do boneco seria removido junto e
            // a arma ficaria invisivel ate a proxima troca.
            (Some(want), Some(_)) => {
                commands
                    .entity(entity)
                    .insert(crate::weapon::held_weapon(want.kind, want.ammo));
            }
            (Some(want), None) => {
                crate::weapon::equip(&mut commands, entity, want.kind, want.ammo, facing.0)
            }
            (None, Some(_)) => {
                commands.entity(entity).remove::<Held>();
            }
            (None, None) => {}
        }
    }
}

fn broadcast_round_over(
    runtime: Option<Res<SteamRuntime>>,
    session: Res<OnlineSession>,
    result: Res<RoundResult>,
) {
    let Some(runtime) = runtime else {
        return;
    };
    if !session.host {
        return;
    }
    broadcast(
        &runtime,
        &session,
        &encode_round_over(&result),
        SendFlags::RELIABLE_NO_NAGLE,
    );
}

/// Apenas o dono do lobby decide o fim do round; os clientes recebem o
/// resultado.
pub fn can_decide_round(mode: Res<GameMode>, session: Res<OnlineSession>) -> bool {
    !mode.is_training() && (*mode != GameMode::Online || session.is_host())
}

/// Este processo decide o que acontece na arena?
///
/// Vale para tudo que nasce de um sorteio ou de uma decisao: arma que cai,
/// arma que e pega, arma que voa. Fora do online a resposta e sempre sim --
/// nao ha ninguem para discordar.
pub fn is_authority(mode: Res<GameMode>, session: Res<OnlineSession>) -> bool {
    *mode != GameMode::Online || session.is_host()
}

/// Este processo obedece a outro?
pub fn is_guest(mode: Res<GameMode>, session: Res<OnlineSession>) -> bool {
    *mode == GameMode::Online && !session.is_host()
}

fn encode_intent(intent: &Intent, presses: Presses) -> [u8; INTENT_BYTES] {
    let held = (intent.up as u8) | ((intent.down as u8) << 1);
    let aim_x = (intent.aim.x.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
    let aim_y = (intent.aim.y.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
    let mut out = [0; INTENT_BYTES];
    out[0] = (intent.move_x.clamp(-1.0, 1.0) * i8::MAX as f32) as i8 as u8;
    out[1] = held;
    out[2..4].copy_from_slice(&aim_x.to_le_bytes());
    out[4..6].copy_from_slice(&aim_y.to_le_bytes());
    out[6..].copy_from_slice(&presses);
    out
}

fn decode_intent(data: &[u8]) -> Option<(Intent, Presses)> {
    if data.len() < INTENT_BYTES {
        return None;
    }
    let mut presses = [0; PULSES];
    presses.copy_from_slice(&data[6..INTENT_BYTES]);
    Some((
        Intent {
            move_x: data[0] as i8 as f32 / i8::MAX as f32,
            up: data[1] & 1 != 0,
            down: data[1] & 2 != 0,
            aim: Vec2::new(
                i16::from_le_bytes(data[2..4].try_into().ok()?) as f32 / i16::MAX as f32,
                i16::from_le_bytes(data[4..6].try_into().ok()?) as f32 / i16::MAX as f32,
            ),
            ..default()
        },
        presses,
    ))
}

/// Mascara dos lugares presentes num pacote de tabela.
///
/// Sem ela toda sala pagaria o tamanho de uma sala cheia: uma briga de dois
/// gastaria metade de cada pacote descrevendo dois lugares vazios.
fn mask_of<T>(table: &[Option<T>; MAX_PLAYERS]) -> u8 {
    table
        .iter()
        .enumerate()
        .fold(0, |mask, (at, slot)| mask | ((slot.is_some() as u8) << at))
}

/// Quantos lugares a mascara descreve, ou `None` se ela cita um que nao existe.
fn present(mask: u8) -> Option<usize> {
    (mask >> MAX_PLAYERS == 0).then(|| mask.count_ones() as usize)
}

fn encode_input_table(
    table: &[Option<Intent>; MAX_PLAYERS],
    presses: &[Presses; MAX_PLAYERS],
) -> Vec<u8> {
    let mask = mask_of(table);
    let mut out = Vec::with_capacity(2 + table.iter().flatten().count() * INTENT_BYTES);
    out.push(PACKET_INPUTS);
    out.push(mask);
    for (slot, intent) in table.iter().enumerate() {
        if let Some(intent) = intent {
            out.extend(encode_intent(intent, presses[slot]));
        }
    }
    out
}

type InputTable = [Option<(Intent, Presses)>; MAX_PLAYERS];

fn decode_input_table(data: &[u8]) -> Option<InputTable> {
    let mask = *data.get(1)?;
    let present = present(mask)?;
    if data.first() != Some(&PACKET_INPUTS) || data.len() != 2 + present * INTENT_BYTES {
        return None;
    }
    let mut table: InputTable = [None; MAX_PLAYERS];
    let mut at = 2;
    for (slot, entry) in table.iter_mut().enumerate() {
        if mask & (1 << slot) == 0 {
            continue;
        }
        *entry = decode_intent(&data[at..at + INTENT_BYTES]);
        at += INTENT_BYTES;
    }
    Some(table)
}

fn encode_snapshot(actors: &Snapshot) -> Vec<u8> {
    let mask = mask_of(actors);
    let mut out = Vec::with_capacity(2 + actors.iter().flatten().count() * ACTOR_BYTES);
    out.push(PACKET_SNAPSHOT);
    out.push(mask);
    for actor in actors.iter().flatten() {
        for value in [
            actor.at.x,
            actor.at.y,
            actor.velocity.x,
            actor.velocity.y,
            actor.facing,
        ] {
            out.extend(value.to_le_bytes());
        }
        out.extend(actor.hp.to_le_bytes());
    }
    out
}

fn decode_snapshot(data: &[u8]) -> Option<Snapshot> {
    let mask = *data.get(1)?;
    let present = present(mask)?;
    if data.first() != Some(&PACKET_SNAPSHOT) || data.len() != 2 + present * ACTOR_BYTES {
        return None;
    }
    let mut actors: Snapshot = [None; MAX_PLAYERS];
    let mut at = 2;
    for (slot, entry) in actors.iter_mut().enumerate() {
        if mask & (1 << slot) == 0 {
            continue;
        }
        let mut actor = ActorSnapshot::default();
        {
            let mut number = || {
                let value = f32::from_le_bytes(data[at..at + 4].try_into().unwrap());
                at += 4;
                value
            };
            actor.at = Vec2::new(number(), number());
            actor.velocity = Vec2::new(number(), number());
            actor.facing = number();
        }
        actor.hp = i32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        at += 4;
        *entry = Some(actor);
    }
    Some(actors)
}

fn encode_weapons(state: &WeaponState) -> Vec<u8> {
    let mask = mask_of(&state.held);
    let mut out = Vec::with_capacity(3 + state.ground.len() * GROUND_BYTES);
    out.push(PACKET_WEAPONS);
    out.push(mask);
    for held in state.held.iter().flatten() {
        out.push(held.kind);
        out.extend((held.ammo.min(u16::MAX as u32) as u16).to_le_bytes());
    }
    out.push(state.ground.len() as u8);
    for weapon in &state.ground {
        out.extend(weapon.net.to_le_bytes());
        out.push(weapon.kind);
        out.extend((weapon.ammo.min(u16::MAX as u32) as u16).to_le_bytes());
        for value in [
            weapon.at.x,
            weapon.at.y,
            weapon.velocity.x,
            weapon.velocity.y,
        ] {
            out.extend(value.to_le_bytes());
        }
        out.push(weapon.thrown as u8);
    }
    out
}

fn decode_weapons(data: &[u8]) -> Option<WeaponState> {
    if data.first() != Some(&PACKET_WEAPONS) {
        return None;
    }
    let mask = *data.get(1)?;
    let present = present(mask)?;
    let mut at = 2;
    let mut state = WeaponState::default();
    for (slot, entry) in state.held.iter_mut().enumerate() {
        if mask & (1 << slot) == 0 {
            continue;
        }
        if data.len() < at + 3 {
            return None;
        }
        *entry = Some(HeldState {
            kind: data[at],
            ammo: u16::from_le_bytes(data[at + 1..at + 3].try_into().ok()?) as u32,
        });
        at += 3;
    }
    debug_assert_eq!(at, 2 + present * 3);

    let count = *data.get(at)? as usize;
    at += 1;
    if count > MAX_GROUND || data.len() != at + count * GROUND_BYTES {
        return None;
    }
    for _ in 0..count {
        let cell = &data[at..at + GROUND_BYTES];
        let number = |from: usize| f32::from_le_bytes(cell[from..from + 4].try_into().unwrap());
        state.ground.push(GroundState {
            net: u16::from_le_bytes(cell[0..2].try_into().ok()?),
            kind: cell[2],
            ammo: u16::from_le_bytes(cell[3..5].try_into().ok()?) as u32,
            at: Vec2::new(number(5), number(9)),
            velocity: Vec2::new(number(13), number(17)),
            thrown: cell[21] != 0,
        });
        at += GROUND_BYTES;
    }
    Some(state)
}

fn encode_round_over(result: &RoundResult) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + MAX_PLAYERS * 4);
    out.push(PACKET_ROUND_OVER);
    out.push(result.winner.unwrap_or(u8::MAX));
    out.push(result.rounds.min(u8::MAX as u32) as u8);
    out.push(result.players);
    for wins in result.score {
        out.extend(wins.to_le_bytes());
    }
    out
}

fn decode_round_over(data: &[u8]) -> Option<RoundResult> {
    if data.len() != 4 + MAX_PLAYERS * 4 || data[0] != PACKET_ROUND_OVER {
        return None;
    }
    let mut score = [0; MAX_PLAYERS];
    for (wins, chunk) in score.iter_mut().zip(data[4..].chunks_exact(4)) {
        *wins = u32::from_le_bytes(chunk.try_into().ok()?);
    }
    Some(RoundResult {
        winner: (data[1] < MAX_PLAYERS as u8).then_some(data[1]),
        rounds: data[2] as u32,
        players: data[3].clamp(MIN_PLAYERS as u8, MAX_PLAYERS as u8),
        score,
    })
}

pub struct OnlinePlugin;

impl Plugin for OnlinePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OnlineSession>()
            .add_message::<LobbyCommand>()
            .add_systems(Startup, init_steam)
            .add_systems(
                PreUpdate,
                (
                    pump_callbacks,
                    handle_events,
                    poll_lobby,
                    receive_packets,
                    apply_snapshot,
                    apply_weapon_state,
                )
                    .chain()
                    .in_set(NetReceive),
            )
            .add_systems(OnEnter(GameState::Controls), leave_lobby)
            .add_systems(OnEnter(GameState::Lobby), reopen_lobby)
            .add_systems(OnEnter(GameState::RoundOver), broadcast_round_over)
            .add_systems(
                Update,
                (
                    sync_skin_choice,
                    publish_stage,
                    follow_host_stage,
                    run_lobby_commands,
                )
                    .chain()
                    .in_set(AppSet::Logic)
                    .run_if(in_state(GameState::Lobby)),
            )
            .add_systems(
                Update,
                round_over_controls
                    .in_set(AppSet::Input)
                    .run_if(in_state(GameState::RoundOver))
                    .run_if(resource_equals(GameMode::Online)),
            )
            // Antes de o combate decidir o round: quem desistiu tem que ja
            // contar como derrotado neste mesmo quadro.
            .add_systems(
                Update,
                retire_missing_players
                    .in_set(AppSet::Logic)
                    .before(crate::combat::check_round_over)
                    .run_if(in_state(GameState::Fighting))
                    .run_if(resource_equals(GameMode::Online)),
            )
            // Tambem na espera: e a troca de entrada e snapshot que faz os
            // outros aparecerem andando no lobby em vez de so como um nome numa
            // lista. O protocolo e o mesmo -- a espera nao inventa pacote.
            .add_systems(
                PostUpdate,
                (send_local_input, broadcast_inputs, send_snapshot)
                    .run_if(arena_live)
                    .run_if(resource_equals(GameMode::Online)),
            )
            // Arma so cai na luta, entao o retrato dela nao precisa rodar na
            // espera.
            .add_systems(
                PostUpdate,
                broadcast_weapons
                    .run_if(in_state(GameState::Fighting))
                    .run_if(resource_equals(GameMode::Online)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn presses_of(intent: &Intent) -> Presses {
        let mut presses = [0; PULSES];
        bump_presses(&mut presses, intent);
        presses
    }

    #[test]
    fn intent_faz_round_trip_no_pacote() {
        let original = Intent {
            move_x: -1.0,
            up: true,
            attack: true,
            parry: true,
            aim: Vec2::new(0.5, -0.25),
            ..default()
        };
        let (decoded, presses) =
            decode_intent(&encode_intent(&original, presses_of(&original))).unwrap();
        assert_eq!(decoded.move_x, -1.0);
        assert!(decoded.up);
        assert!((decoded.aim - original.aim).length() < 0.001);
        // Golpe e defesa viajam como contagem, nao como bit do quadro.
        assert_eq!(presses[1], 1, "o golpe nao foi contado");
        assert_eq!(presses[3], 1, "a defesa nao foi contada");
        assert_eq!(presses[0], 0, "pulo contado sem ter sido apertado");
    }

    /// Um pacote perdido nao pode custar um golpe.
    ///
    /// E a razao de a entrada viajar como contador. Enquanto ela viajava como
    /// "ataquei neste quadro", o pacote que se perdia levava o soco junto -- e
    /// como ela era enviada com garantia de entrega para evitar isso, cada
    /// quadro entrava numa fila que so crescia. O atraso vinha dai.
    #[test]
    fn pacote_perdido_nao_engole_o_golpe() {
        let remote = RemoteInput::default();
        let keys = ButtonInput::<KeyCode>::default();
        let sense = sense_parado();
        let parado = Intent::default();
        let socando = Intent {
            attack: true,
            ..default()
        };

        let mut presses = [0; PULSES];
        // Calibra: o primeiro pacote so diz de onde a contagem parte.
        remote.push(parado, presses);
        remote.poll(&keys, &sense);

        // Tres socos seguidos, e so o ultimo pacote chega.
        for _ in 0..3 {
            bump_presses(&mut presses, &socando);
        }
        remote.push(parado, presses);

        let entregues = (0..5)
            .filter(|_| remote.poll(&keys, &sense).attack)
            .count();
        assert_eq!(entregues, 3, "os socos perdidos nao foram recuperados");
    }

    /// E o mesmo pacote chegando duas vezes nao pode virar dois golpes.
    #[test]
    fn pacote_repetido_nao_duplica_o_golpe() {
        let remote = RemoteInput::default();
        let keys = ButtonInput::<KeyCode>::default();
        let sense = sense_parado();
        let mut presses = [0; PULSES];

        remote.push(Intent::default(), presses);
        bump_presses(
            &mut presses,
            &Intent {
                jump: true,
                ..default()
            },
        );
        remote.push(Intent::default(), presses);
        remote.push(Intent::default(), presses);

        let pulos = (0..5).filter(|_| remote.poll(&keys, &sense).jump).count();
        assert_eq!(pulos, 1, "o pacote repetido pulou duas vezes");
    }

    /// O primeiro pacote nao pode chegar como uma rajada de apertos.
    #[test]
    fn a_primeira_leitura_nao_dispara_nada() {
        let remote = RemoteInput::default();
        let keys = ButtonInput::<KeyCode>::default();
        let sense = sense_parado();

        remote.push(Intent::default(), [200; PULSES]);
        let intent = remote.poll(&keys, &sense);
        assert!(!intent.attack && !intent.jump && !intent.special);
    }

    fn sense_parado() -> Sense {
        Sense {
            at: Vec2::ZERO,
            foe: None,
            grounded: true,
            health: 1.0,
            time: 0.0,
            left: default(),
            right: default(),
            armed: None,
        }
    }

    /// O pacote de inicio carrega a aparencia inteira de cada lugar.
    ///
    /// Enquanto ele levava so a pele, o rosto escolhido ficava em casa: o
    /// adversario nascia com o rosto padrao na tela de quem o enfrentava, e
    /// cada cliente via uma cara diferente do mesmo boneco.
    #[test]
    fn inicio_faz_round_trip_com_fase_lugar_pele_e_rosto() {
        let looks = [
            Look {
                skin: 3,
                face: Face {
                    hair: 2,
                    eyes: 4,
                    nose: 1,
                    mouth: 3,
                },
            },
            Look {
                skin: 5,
                face: Face::variety(1),
            },
            Look {
                skin: 0,
                face: Face::variety(2),
            },
            Look {
                skin: 2,
                face: Face::default(),
            },
        ];
        let packet = encode_start(2, 3, 1, looks);
        assert_eq!(decode_start(&packet), Some((2, 3, 1, looks)));
        assert_eq!(decode_start(&packet[..3]), None);
    }

    /// Uma sala de dois nao pode pagar o preco de uma de quatro.
    #[test]
    fn o_snapshot_so_descreve_os_lugares_em_uso() {
        let actor = ActorSnapshot {
            at: Vec2::new(1.0, 2.0),
            velocity: Vec2::new(3.0, 4.0),
            hp: 91,
            facing: 1.0,
        };
        let dois: Snapshot = [Some(actor), Some(actor), None, None];
        let quatro: Snapshot = [Some(actor); MAX_PLAYERS];

        assert_eq!(encode_snapshot(&dois).len(), 2 + 2 * ACTOR_BYTES);
        assert_eq!(encode_snapshot(&quatro).len(), 2 + 4 * ACTOR_BYTES);
    }

    #[test]
    fn snapshot_faz_round_trip_preservando_os_lugares() {
        let mut actors: Snapshot = [None; MAX_PLAYERS];
        actors[0] = Some(ActorSnapshot {
            at: Vec2::new(1.0, 2.0),
            velocity: Vec2::new(3.0, 4.0),
            hp: 91,
            facing: 1.0,
        });
        // Buraco no meio de proposito: o lugar 1 saiu da partida, e o 2 nao
        // pode herdar os bytes dele.
        actors[2] = Some(ActorSnapshot {
            at: Vec2::new(-5.0, 6.0),
            velocity: Vec2::new(7.0, -8.0),
            hp: 42,
            facing: -1.0,
        });

        let decoded = decode_snapshot(&encode_snapshot(&actors)).unwrap();
        assert_eq!(decoded[0].unwrap().at, Vec2::new(1.0, 2.0));
        assert!(decoded[1].is_none(), "lugar vazio virou lutador");
        assert_eq!(decoded[2].unwrap().hp, 42);
        assert_eq!(decoded[2].unwrap().facing, -1.0);
        assert!(decoded[3].is_none());
    }

    #[test]
    fn tabela_de_entrada_faz_round_trip_por_lugar() {
        let mut table: [Option<Intent>; MAX_PLAYERS] = [None; MAX_PLAYERS];
        table[1] = Some(Intent {
            move_x: 1.0,
            jump: true,
            ..default()
        });
        table[3] = Some(Intent {
            attack: true,
            ..default()
        });
        let mut presses = [[0; PULSES]; MAX_PLAYERS];
        for (slot, intent) in table.iter().enumerate() {
            if let Some(intent) = intent {
                bump_presses(&mut presses[slot], intent);
            }
        }

        let decoded = decode_input_table(&encode_input_table(&table, &presses)).unwrap();
        assert!(decoded[0].is_none());
        assert_eq!(decoded[1].unwrap().0.move_x, 1.0);
        assert_eq!(decoded[1].unwrap().1[0], 1, "o pulo do lugar 1 se perdeu");
        assert!(decoded[2].is_none());
        assert_eq!(decoded[3].unwrap().1[1], 1, "o golpe do lugar 3 se perdeu");
    }

    /// O armamento inteiro tem que atravessar: quem segura o que, e o que ficou
    /// caido onde.
    #[test]
    fn armas_fazem_round_trip() {
        let mut state = WeaponState::default();
        state.held[0] = Some(HeldState { kind: 2, ammo: 7 });
        state.held[3] = Some(HeldState { kind: 5, ammo: 0 });
        state.ground.push(GroundState {
            net: 4211,
            kind: 1,
            ammo: 4,
            at: Vec2::new(-120.0, 30.0),
            velocity: Vec2::new(0.0, -9.0),
            thrown: false,
        });
        state.ground.push(GroundState {
            net: 4212,
            kind: 7,
            ammo: 2,
            at: Vec2::new(88.0, -12.0),
            velocity: Vec2::new(590.0, 260.0),
            thrown: true,
        });

        let decoded = decode_weapons(&encode_weapons(&state)).unwrap();
        assert_eq!(decoded, state);
        assert!(decoded.held[1].is_none(), "lugar sem arma ganhou uma");
    }

    /// Pacote truncado ou com lugar inexistente nao pode virar estado.
    #[test]
    fn pacote_corrompido_e_recusado() {
        let actors: Snapshot = [Some(ActorSnapshot::default()), None, None, None];
        let packet = encode_snapshot(&actors);
        assert!(decode_snapshot(&packet[..packet.len() - 1]).is_none());

        let mut mentiroso = packet.clone();
        mentiroso[1] = 0b1111_0001;
        assert!(
            decode_snapshot(&mentiroso).is_none(),
            "mascara citando lugar inexistente foi aceita"
        );

        let mut armas = WeaponState::default();
        armas.ground.push(GroundState::default());
        let packet = encode_weapons(&armas);
        assert!(decode_weapons(&packet[..packet.len() - 1]).is_none());
        // Uma contagem inventada nao pode virar uma alocacao enorme.
        let mut mentiroso = packet.clone();
        mentiroso[2] = 200;
        assert!(decode_weapons(&mentiroso).is_none());
    }

    #[test]
    fn fim_de_round_faz_round_trip_com_o_placar_inteiro() {
        let result = RoundResult {
            winner: Some(2),
            score: [1, 0, 2, 1],
            rounds: 4,
            players: 4,
        };
        let decoded = decode_round_over(&encode_round_over(&result)).unwrap();
        assert_eq!(decoded.winner, Some(2));
        assert_eq!(decoded.score, [1, 0, 2, 1]);
        assert_eq!(decoded.rounds, 4);
        assert_eq!(decoded.players, 4);
    }

    /// O ritmo de envio nao pode andar junto com o framerate.
    #[test]
    fn o_envio_segue_o_relogio_e_nao_o_framerate() {
        let mut since = 0.0;
        // 240 quadros de 1/240 s: a 60 Hz isso tem que dar 60 envios.
        let enviados = (0..240)
            .filter(|_| due(&mut since, 1.0 / 240.0, 60.0))
            .count();
        assert_eq!(enviados, 60, "o envio nao respeitou o ritmo pedido");

        // E um quadro longo nao pode virar uma rajada para compensar.
        let mut since = 0.0;
        assert!(due(&mut since, 2.0, 60.0));
        let seguidos = (0..3).filter(|_| due(&mut since, 0.0, 60.0)).count();
        assert_eq!(seguidos, 0, "a pausa virou rajada");
    }

    /// Empate nao tem vencedor, e 255 nao pode virar o jogador 255.
    /// Quem fica nao pode mudar de lugar quando alguem sai.
    ///
    /// Renumerar entregaria o boneco, a cor e a linha do placar de quem saiu
    /// para o vizinho -- e a entrada que chegasse pelo lugar antigo iria
    /// mandar no jogador errado.
    #[test]
    fn sair_da_sala_nao_renumera_quem_fica() {
        let (dono, b, c, d) = (
            SteamId::from_raw(1),
            SteamId::from_raw(2),
            SteamId::from_raw(3),
            SteamId::from_raw(4),
        );

        let cheia = assign_slots([None; MAX_PLAYERS], dono, &[dono, b, c, d]);
        assert_eq!(cheia, [Some(dono), Some(b), Some(c), Some(d)]);

        // O lugar 1 desiste: o 2 e o 3 continuam onde estavam.
        let apos_saida = assign_slots(cheia, dono, &[dono, c, d]);
        assert_eq!(apos_saida, [Some(dono), None, Some(c), Some(d)]);

        // E quem chega depois herda a vaga aberta, nao o fim da fila.
        let e = SteamId::from_raw(5);
        let recomposta = assign_slots(apos_saida, dono, &[dono, c, d, e]);
        assert_eq!(recomposta, [Some(dono), Some(e), Some(c), Some(d)]);
    }

    /// Sala nova comeca com o dono no primeiro lugar.
    #[test]
    fn o_dono_fica_no_primeiro_lugar() {
        let (dono, outro) = (SteamId::from_raw(9), SteamId::from_raw(7));
        let slots = assign_slots([None; MAX_PLAYERS], dono, &[outro, dono]);
        assert_eq!(slots[0], Some(dono), "o dono nao ficou com a autoridade");
        assert_eq!(slots[1], Some(outro));
    }

    /// O lugar vago no meio ainda precisa de boneco.
    ///
    /// Enquanto a luta contou ocupados, uma sala [dono, vago, C, D] abria com
    /// tres lugares e o jogador do quarto ficava sem boneco: sem entidade, o
    /// snapshot nao o alcancava e ele assistia a propria partida.
    #[test]
    fn lugar_vago_no_meio_nao_encurta_a_partida() {
        let mut session = OnlineSession {
            slots: [
                Some(SteamId::from_raw(1)),
                None,
                Some(SteamId::from_raw(3)),
                Some(SteamId::from_raw(4)),
            ],
            ..default()
        };
        assert_eq!(session.filled(), 3);
        assert_eq!(session.span(), 4, "o ultimo lugar ficaria sem boneco");

        // Sala de dois continua sendo sala de dois.
        session.slots = [
            Some(SteamId::from_raw(1)),
            Some(SteamId::from_raw(2)),
            None,
            None,
        ];
        assert_eq!(session.span(), 2);
    }

    /// Sala cheia nao aceita um quinto.
    #[test]
    fn a_sala_para_no_teto() {
        let membros: Vec<SteamId> = (1..=6).map(SteamId::from_raw).collect();
        let slots = assign_slots([None; MAX_PLAYERS], membros[0], &membros);
        assert_eq!(slots.iter().flatten().count(), MAX_PLAYERS);
    }

    #[test]
    fn empate_atravessa_o_pacote_como_empate() {
        let result = RoundResult {
            winner: None,
            score: [0, 0, 0, 0],
            rounds: 1,
            players: 2,
        };
        let decoded = decode_round_over(&encode_round_over(&result)).unwrap();
        assert_eq!(decoded.winner, None);
    }
}
