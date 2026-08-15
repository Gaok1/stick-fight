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

