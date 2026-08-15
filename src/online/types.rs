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

