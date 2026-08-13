//! Estado global do jogo e ordenacao de sistemas.

use bevy::prelude::*;

/// Fases do jogo. `Controls` e a tela inicial pedida no START.
#[derive(States, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum GameState {
    /// Tela de controles -- primeira coisa que aparece.
    #[default]
    Controls,
    /// Escolha cosmetica dos lutadores, antes de entrar na sala/arena.
    SkinSelect,
    /// Sala online da Steam, antes e entre as lutas.
    Lobby,
    /// Luta em andamento.
    Fighting,
    /// Alguem morreu; mostra placar e espera restart.
    RoundOver,
}

impl GameState {
    /// A arena esta de pe e os bonecos andam nela.
    ///
    /// O lobby e uma arena de verdade: da pra correr, bater e cair na espera
    /// enquanto os amigos chegam. O que ele nao tem e round -- ninguem perde,
    /// ninguem marca ponto, e quem cai volta em pe. Por isso a diferenca entre
    /// esperar e lutar mora nas regras de round, e nao no que se pode fazer.
    pub fn in_arena(self) -> bool {
        matches!(self, GameState::Lobby | GameState::Fighting)
    }
}

/// Condicao de sistema: a arena esta de pe.
///
/// Existe para que ligar o aquecimento tenha sido trocar uma condicao, e nao
/// listar `Lobby` ao lado de `Fighting` em quinze lugares -- o tipo de lista
/// que sempre fica com um esquecido.
pub fn arena_live(state: Res<State<GameState>>) -> bool {
    state.get().in_arena()
}

/// Modo escolhido no menu.
///
/// Ele muda quem entra na arena e o que conta como fim de round -- nunca as
/// regras de golpe. Combo, arma e fisica sao identicos nos dois modos, senao o
/// treino ensinaria um jogo que nao existe.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GameMode {
    /// Dois humanos; quem zerar a barra ou cair no vao perde.
    #[default]
    Versus,
    /// De dois a quatro jogadores conectados por Steam P2P.
    Online,
    /// Jogador 1 contra um adversario do jogo. Round vale igual ao versus.
    Cpu,
    /// So o jogador 1 contra um boneco que nao revida nem morre.
    Training,
}

impl GameMode {
    /// Todos os modos, na ordem em que o menu os lista.
    pub const ALL: [GameMode; 4] = [
        GameMode::Versus,
        GameMode::Online,
        GameMode::Cpu,
        GameMode::Training,
    ];

    /// Nome curto exibido no seletor.
    pub fn label(self) -> &'static str {
        match self {
            GameMode::Versus => "VERSUS",
            GameMode::Online => "ONLINE",
            GameMode::Cpu => "CPU",
            GameMode::Training => "TRAINING",
        }
    }

    /// Uma linha explicando o modo, mostrada abaixo do seletor.
    pub fn blurb(self) -> &'static str {
        match self {
            GameMode::Versus => "  TWO PLAYERS. EMPTY THE BAR OR KNOCK THEM INTO A PIT.",
            GameMode::Online => "  STEAM LOBBY FOR TWO TO FOUR PLAYERS OVER RELAYED P2P.",
            GameMode::Cpu => "  P1 VS THE GAME. SAME MATCH, ONE KEYBOARD.",
            GameMode::Training => "  P1 VS A DUMMY. NOBODY DIES. LIVE COMBO COUNTER.",
        }
    }

    /// Atalho de leitura para as condicoes de sistema.
    pub fn is_training(self) -> bool {
        matches!(self, GameMode::Training)
    }
}

/// Ordem canonica de execucao dentro de `Update`.
///
/// Existe para que o rebuild dos glifos (`Render`) sempre rode depois de toda a
/// logica que pode mexer num `AsciiSprite` no mesmo frame -- sem isso a
/// animacao fica um frame atrasada.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppSet {
    /// Le teclado e preenche `Intent`.
    Input,
    /// Regras do jogo: combate, armas, pickups.
    Logic,
    /// Integracao de velocidade e resolucao de colisao.
    Physics,
    /// Escolhe a pose/arte de cada entidade.
    Animate,
    /// Reconstroi os glifos das artes que mudaram.
    Render,
}
