//! Atores: os bonecos, seus componentes e o ciclo de vida deles.

pub mod face;
pub mod input;
pub mod motion;
pub mod pose;
pub mod rig;
pub mod skin;

use bevy::prelude::*;

pub use input::{Controller, Intent, KeyBindings};
pub use pose::Pose;
pub use skin::{ActorSkin, Flash, Skin, SkinSelections};

use crate::ascii::{AsciiSprite, CELL, Layer, palette};
use crate::level::CurrentLevel;
use crate::physics::{Collider, Falls, Grounded, Velocity};
use crate::state::{GameMode, GameState};

/// Vida inicial de um jogador.
pub const MAX_HP: i32 = 100;

/// Teto de lutadores numa partida.
///
/// E o tamanho dos vetores fixos do jogo (placar, snapshot, peles), nao a
/// contagem de um round: uma sala vale de dois a quatro, e quem precisa saber
/// quantos estao em campo conta os `Player` vivos em vez de assumir este
/// numero. Enquanto os dois conceitos foram o mesmo, uma sala de dois lia
/// placar de quatro e dois slots vazios apareciam na tela.
pub const MAX_PLAYERS: usize = 4;

/// Minimo para haver partida.
pub const MIN_PLAYERS: usize = 2;

/// Colisor do boneco, derivado da caixa da arte e encolhido de uma celula.
///
/// Amarrar os dois evita que eles saiam de sincronia se a arte mudar, e a folga
/// e o que impede o boneco de enganchar em quina de plataforma.
const BODY_SIZE: Vec2 = Vec2::new(
    pose::BODY_COLS as f32 * CELL.x - 4.0,
    pose::BODY_ROWS as f32 * CELL.y - 4.0,
);

/// Filho puramente visual, ancorado nos pes do colisor do jogador.
#[derive(Component)]
pub struct ActorVisual;

/// Memoria minima para misturar poses e dar impacto visual sem tocar na fisica.
#[derive(Component, Default)]
pub struct ActorVisualMotion {
    pub initialized: bool,
    pub was_airborne: bool,
    pub landing: f32,
}

/// Defasagem da respiracao parada.
///
/// E dado, e nao o indice do jogador nem o da entidade: a camada visual nao
/// deve ler identidade pra decidir estetica, e assim qualquer boneco -- dummy
/// incluso -- entra na animacao sem virar caso especial. Dois com a mesma fase
/// respiram em unissono, e o olho le isso como bug.
#[derive(Component, Debug, Clone, Copy)]
pub struct BreathPhase(pub f32);

/// Cor de destaque de um boneco.
///
/// A camada visual le isto em vez de `Player::color`: assim ela nao precisa
/// saber se o que esta desenhando e um jogador. Foi o que manteve o dummy sem
/// animacao nenhuma ate agora -- ele nao e `Player`, entao nada o pintava.
#[derive(Component, Debug, Clone, Copy)]
pub struct ActorTint(pub Color);

/// Segmento independente do esqueleto visual.
#[derive(Component)]
pub struct ActorLimb {
    pub owner: Entity,
    pub side: f32,
    pub kind: LimbKind,
}

#[derive(Component, Default)]
pub struct LimbMotion(pub bool);

/// Membro ja arrancado: ele para de ser animado e some do boneco.
///
/// Mora aqui, junto de [`ActorLimb`], e nao no modulo de sangue, porque quem
/// anima os membros precisa saber pular os que sairam -- e a animacao nao tem
/// por que conhecer gore. Sair do boneco e voar pedaco sao duas coisas: esta e
/// so a primeira.
#[derive(Component)]
pub struct Severed;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimbKind {
    UpperArm,
    Forearm,
    Thigh,
    Shin,
}

/// Um jogador.
#[derive(Component, Debug, Clone, Copy)]
pub struct Player {
    /// Indice do jogador (0 ou 1).
    pub id: u8,
    /// Cor de destaque, compartilhada entre boneco e HUD.
    pub color: Color,
}

/// Alvo imovel e indestrutivel do modo treino.
///
/// Ele nao tem `Velocity`, entao ignora knockback de proposito: um saco de
/// pancada que sai voando torna impossivel medir um combo ate o fim.
#[derive(Component)]
pub struct TrainingDummy;

/// Vida corrente.
#[derive(Component, Debug, Clone, Copy)]
pub struct Health {
    /// Pontos restantes.
    pub hp: i32,
    /// Maximo, para a barra do HUD.
    pub max: i32,
}

impl Health {
    /// Vida cheia.
    pub fn full(max: i32) -> Self {
        Self { hp: max, max }
    }

    /// Fracao de vida em [0, 1].
    pub fn fraction(&self) -> f32 {
        (self.hp as f32 / self.max as f32).clamp(0.0, 1.0)
    }

    /// Ja morreu?
    pub fn is_dead(&self) -> bool {
        self.hp <= 0
    }
}

/// Para que lado o boneco olha: -1 esquerda, +1 direita.
#[derive(Component, Debug, Clone, Copy)]
pub struct Facing(pub f32);

impl Default for Facing {
    fn default() -> Self {
        Self(1.0)
    }
}

/// Agarrado numa corrente.
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct Climbing(pub bool);

/// Janela de animacao de ataque -- so visual, o hitbox tem vida propria.
#[derive(Component, Debug)]
pub struct Attacking(pub Timer);

/// Atordoado por dano: ignora entrada ate acabar.
#[derive(Component, Debug)]
pub struct Stunned(pub Timer);

/// No chao, nao so recuando.
///
/// Acompanha o `Stunned` de uma rasteira e sai junto com ele. E um marcador
/// proprio, e nao um limiar sobre a duracao do atordoamento, para que mudar o
/// tempo de um golpe nunca transforme um recuo em queda por acidente.
#[derive(Component, Debug)]
pub struct Downed;

/// Cadencia das pequenas nuvens de pe levantadas pela corrida.
#[derive(Component)]
pub struct StepDust(pub Timer);

#[derive(Component, Default)]
pub struct JumpAssist {
    coyote: f32,
    buffer: f32,
}

/// Monta o corpo visual de um ator: a silhueta e os oito membros.
///
/// Jogador e dummy passam por aqui para que nenhum boneco nasca pela metade.
/// Enquanto isto estava escrito solto dentro do spawn dos jogadores, o dummy
/// vivia sem membro nenhum -- um palito chapado ao lado de bonecos
/// articulados.
///
/// A pele decide os glifos e as cores do corpo; o `tint` decide so o acento,
/// que e o que diz de quem e o boneco. Por isso dois jogadores podem vestir a
/// mesma pele sem ficarem indistinguiveis.
pub(crate) fn spawn_actor_body(
    commands: &mut Commands,
    root: Entity,
    skin: &'static Skin,
    tint: Color,
    facing: f32,
    phase: f32,
    face: face::Face,
) {
    commands
        .entity(root)
        .insert((BreathPhase(phase), ActorSkin(skin), Flash::default()));
    let body = commands
        .spawn((
            ActorVisual,
            ActorVisualMotion::default(),
            AsciiSprite::footed(skin.draw(Pose::IdleA, tint, None)).flipped(facing < 0.0),
            Layer::Actor,
            Transform::from_translation(Vec3::new(0.0, -BODY_SIZE.y * 0.5, 0.0)),
            ChildOf(root),
        ))
        .id();
    // O rosto vem depois porque ele pende do corpo: e o corpo que carrega a
    // deformacao da pose, e a cara tem que ir junto com a cabeca.
    face::spawn_face(commands, root, body, face);
    for side in [-1.0, 1.0] {
        for kind in [
            LimbKind::UpperArm,
            LimbKind::Forearm,
            LimbKind::Thigh,
            LimbKind::Shin,
        ] {
            commands.spawn((
                ActorLimb {
                    owner: root,
                    side,
                    kind,
                },
                LimbMotion::default(),
                AsciiSprite::new(skin.limb_art(None)),
                Layer::Actor,
                Transform::default(),
                ChildOf(root),
            ));
        }
    }
}

/// Quantos lugares a partida corrente usa.
///
/// Uma fonte so para a contagem, consultada tanto por quem cria os bonecos
/// quanto por quem monta placar e HUD. Enquanto cada um tinha a sua, bastava um
/// deles assumir dois para uma sala de tres mostrar um placar errado.
///
/// Online a contagem vem da sessao, que a congela no comeco da luta: ela nao
/// pode mudar no meio de um round, senao o snapshot troca de tamanho no meio do
/// voo e o placar fica sem base.
pub fn seats(mode: GameMode, online: Option<&crate::online::OnlineSession>) -> usize {
    match mode {
        GameMode::Training => 1,
        GameMode::Online => online.map_or(MIN_PLAYERS, |session| session.seats()),
        _ => MIN_PLAYERS,
    }
}

/// Poe um lutador em campo, no ponto de nascimento do lugar dele.
///
/// `until` e o estado a que a vida do boneco fica presa: a luta despeja os
/// seus ao acabar, o aquecimento do lobby despeja os dele ao comecar a luta.
/// Sem esse parametro os bonecos da espera atravessariam para dentro da
/// partida, e nasceriam dois de cada.
fn spawn_fighter(
    commands: &mut Commands,
    id: u8,
    level: &CurrentLevel,
    mode: GameMode,
    online: Option<&crate::online::OnlineSession>,
    skins: &SkinSelections,
    until: GameState,
) {
    let color = palette::player(id);
    let at = level
        .0
        .spawn_points()
        .get(id as usize)
        .copied()
        .unwrap_or(Vec2::ZERO);
    let facing = if at.x < 0.0 { 1.0 } else { -1.0 };
    let local = online.map_or(0, |session| session.local_player_id());

    // O segundo lugar e do jogo quando o modo pede: trocar a fonte de entrada
    // e tudo que separa um humano de um adversario. Nenhum sistema de
    // movimento, combate ou arma sabe a diferenca.
    //
    // Online cada lugar remoto tem a sua propria fila de entrada. Enquanto
    // todos dividiram uma so, tres adversarios andavam colados: cada pacote que
    // chegava mandava nos tres bonecos ao mesmo tempo.
    let source: Box<dyn input::InputSource> = match mode {
        GameMode::Online if id != local => Box::new(
            online
                .map(|session| session.remote_input(id))
                .unwrap_or_default(),
        ),
        GameMode::Online => Box::new(KeyBindings::player_one()),
        GameMode::Cpu if id == 1 => Box::new(input::CpuBrain { phase: 0.0 }),
        _ => match id {
            0 => Box::new(KeyBindings::player_one()),
            1 => Box::new(KeyBindings::player_two()),
            // O teclado so tem dois conjuntos de teclas. Um modo local com mais
            // lugares preenche o resto com o jogo -- dois humanos na mesma
            // tecla seria pior que um adversario a mais.
            _ => Box::new(input::CpuBrain {
                phase: id as f32 * 1.3,
            }),
        },
    };

    let root = commands
        .spawn((
            Player { id, color },
            ActorTint(color),
            Health::full(MAX_HP),
            Facing(facing),
            Climbing(false),
            Intent::default(),
            Controller(source),
            Pose::IdleA,
            Transform::from_translation(at.extend(0.0)),
            Visibility::default(),
            Velocity::default(),
            Grounded::default(),
            Falls,
            Collider::size(BODY_SIZE.x, BODY_SIZE.y),
            DespawnOnExit(until),
        ))
        .insert((
            StepDust(Timer::from_seconds(0.09, TimerMode::Repeating)),
            JumpAssist::default(),
        ))
        .id();

    // Uma pele por lugar no arsenal de peles: e o que faz o catalogo estar em
    // uso de verdade, e nao decorando o codigo.
    spawn_actor_body(
        commands,
        root,
        skins.for_player(id),
        color,
        facing,
        id as f32 * 1.7,
        skins.face_for(id),
    );
}

/// Cria os jogadores nos pontos de nascimento da fase.
///
/// No treino so o jogador 1 entra; o lugar do segundo fica com o dummy.
pub fn spawn_players(
    mut commands: Commands,
    level: Res<CurrentLevel>,
    mode: Res<GameMode>,
    online: Option<Res<crate::online::OnlineSession>>,
    skins: Res<SkinSelections>,
) {
    for id in 0..seats(*mode, online.as_deref()) as u8 {
        spawn_fighter(
            &mut commands,
            id,
            &level,
            *mode,
            online.as_deref(),
            &skins,
            GameState::Fighting,
        );
    }
}

/// Mantem os bonecos do aquecimento em dia com quem esta na sala.
///
/// O lobby nao congela lugares como a luta faz: gente entra e sai o tempo
/// todo, e o boneco tem que aparecer junto com o nome no painel. Por isso a
/// contagem aqui e a sala de agora, e nao [`seats`].
fn sync_warmup(
    mut commands: Commands,
    level: Res<CurrentLevel>,
    mode: Res<GameMode>,
    online: Option<Res<crate::online::OnlineSession>>,
    skins: Res<SkinSelections>,
    mut was_local: Local<u8>,
    fighters: Query<(Entity, &Player)>,
) {
    let session = online.as_deref();
    // O proprio boneco entra mesmo sem sala nenhuma: com a Steam fechada da
    // pra varzear sozinho em vez de encarar uma arena vazia.
    let local = session.map_or(0, |session| session.local_player_id());

    // A Steam pode demorar a dizer qual e o nosso lugar, e ele muda depois dos
    // bonecos ja terem nascido. Quando isso acontece todos estao com a fonte de
    // entrada trocada -- o teclado daqui estaria movendo o boneco de outro --
    // entao a arena recomeca em vez de remendar boneco por boneco.
    if *was_local != local {
        *was_local = local;
        for (entity, _) in &fighters {
            commands.entity(entity).despawn();
        }
        return;
    }

    // Enquanto a sala nao disser qual e o nosso lugar, nao ha boneco proprio a
    // criar: chutar o lugar zero poria este teclado no comando do boneco do
    // dono ate a tabela chegar, que era o primeiro segundo de quem entrava.
    let seated = session.is_none_or(|session| session.seated());
    let belongs =
        |id: u8| (seated && id == local) || session.is_some_and(|session| session.seat_taken(id));

    for (entity, player) in &fighters {
        if !belongs(player.id) {
            commands.entity(entity).despawn();
        }
    }

    let present: Vec<u8> = fighters.iter().map(|(_, player)| player.id).collect();
    for id in (0..MAX_PLAYERS as u8).filter(|id| belongs(*id) && !present.contains(id)) {
        spawn_fighter(
            &mut commands,
            id,
            &level,
            *mode,
            session,
            &skins,
            GameState::Lobby,
        );
    }
}

/// Meia-altura do corpo de um ator.
///
/// Exposta porque quem calcula alcance de golpe precisa dela e nao deve
/// recopiar o numero -- se `BODY_SIZE` mudar, o alcance muda junto.
pub fn body_half_height() -> f32 {
    BODY_SIZE.y * 0.5
}

/// Altura em que o dummy fica de pe quando nao esta pulando.
#[derive(Component)]
pub struct DummyHome(pub f32);

/// O que o dummy faz enquanto apanha.
///
/// Cada modo existe para treinar uma das leituras do combate. Um poste so
/// ensina a contar dano.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum DummyBehavior {
    /// Parado. Bom para medir combo e dano.
    #[default]
    Still,
    /// Pula no lugar, saindo do alcance dos golpes baixos.
    Hop,
    /// Apara em intervalos fixos, para treinar a espera.
    Guard,
    /// Fica agachado, para ver o gancho passar por cima.
    Crouch,
}

impl DummyBehavior {
    /// Todos, na ordem em que a tecla os percorre.
    pub const ALL: [DummyBehavior; 4] = [
        DummyBehavior::Still,
        DummyBehavior::Hop,
        DummyBehavior::Guard,
        DummyBehavior::Crouch,
    ];

    /// Nome exibido no painel de treino.
    pub fn label(self) -> &'static str {
        match self {
            DummyBehavior::Still => "STILL",
            DummyBehavior::Hop => "HOP",
            DummyBehavior::Guard => "GUARD",
            DummyBehavior::Crouch => "CROUCH",
        }
    }

    /// Proximo da lista, girando.
    pub fn next(self) -> Self {
        let at = Self::ALL.iter().position(|b| *b == self).unwrap_or(0);
        Self::ALL[(at + 1) % Self::ALL.len()]
    }
}

/// Altura do pulo do dummy.
///
/// Escolhida para tirar o corpo do alcance da rasteira sem tirar do alcance do
/// gancho: e isso que faz o modo `Hop` treinar antiaereo em vez de so animar.
/// A janela e estreita, entao `combat` tem um teste que a tranca.
pub const DUMMY_HOP: f32 = 56.0;

/// Onde o dummy fica de pe, dado o ponto de spawn que ele ocupa.
///
/// Ele nao cai: nao tem `Falls` nem `Velocity`, de proposito, para nao sair
/// voando com o knockback. Isso significa que ele precisa nascer ja pousado --
/// usar o ponto de spawn direto o deixa flutuando a altura inteira da queda,
/// e a essa altura nenhum golpe alcanca ele.
pub fn dummy_stand_y(level: &dyn crate::level::Level, spawn: Vec2) -> f32 {
    match level.ground_under(spawn) {
        Some(ground) => ground + body_half_height(),
        None => spawn.y,
    }
}

/// Poe o boneco de treino no lugar que seria do jogador 2.
///
/// A vida dele e a mesma do jogador de proposito: assim a barra le como dano de
/// combo em porcentagem real, nao como um numero abstrato.
pub fn spawn_training_dummy(mut commands: Commands, level: Res<CurrentLevel>) {
    let spawn = level
        .0
        .spawn_points()
        .get(1)
        .copied()
        .unwrap_or(Vec2::new(200.0, 0.0));
    let at = Vec2::new(spawn.x, dummy_stand_y(level.0.as_ref(), spawn));

    let root = commands
        .spawn((
            TrainingDummy,
            DummyHome(at.y),
            ActorTint(palette::GOLD),
            Pose::IdleA,
            // Nunca preenchido -- `gather_intents` so escreve em `Player`.
            // Existe porque a animacao de membros le intencao para mirar, e
            // um dummy sem `Intent` ficaria de fora dela.
            Intent::default(),
            Health::full(MAX_HP),
            Facing(-1.0),
            Transform::from_translation(at.extend(0.0)),
            Visibility::default(),
            Collider::size(BODY_SIZE.x, BODY_SIZE.y),
            DespawnOnExit(GameState::Fighting),
        ))
        .id();

    // Fase fora do intervalo dos jogadores (0 e 1.7) para os tres nao
    // respirarem juntos quando estao na mesma tela. A pele esmaecida separa o
    // saco de pancada de um lutador de verdade sem precisar de caso especial em
    // sistema nenhum.
    // Rosto vazio: o dummy nao e ninguem, e uma cara nele o faria parecer um
    // lutador a mais em vez de um saco de pancada.
    spawn_actor_body(
        &mut commands,
        root,
        &skin::WRAITH,
        palette::GOLD,
        -1.0,
        3.4,
        face::Face {
            hair: 0,
            eyes: 4,
            nose: 0,
            mouth: 2,
        },
    );
    commands.spawn((
        AsciiSprite::new(crate::ascii::AsciiArt::solid("DUMMY", palette::IRON)),
        Layer::Actor,
        Transform::from_translation(Vec3::new(0.0, BODY_SIZE.y * 0.5 + 14.0, 0.0)),
        ChildOf(root),
    ));
}

/// Move o dummy conforme o comportamento escolhido.
///
/// Ele nunca ganha `Velocity`: o pulo e escrito na `Transform` de proposito,
/// para o dummy continuar imune a knockback. Um saco de pancada que sai voando
/// nao deixa terminar um combo.
fn drive_dummy(
    time: Res<Time>,
    behavior: Res<DummyBehavior>,
    mut commands: Commands,
    mut since_guard: Local<f32>,
    mut dummies: Query<(Entity, &mut Transform, &DummyHome), With<TrainingDummy>>,
) {
    let lift = match *behavior {
        // Meio seno: sobe, volta e fica um tempo no chao, como um pulo.
        DummyBehavior::Hop => (time.elapsed_secs() * 2.4).sin().max(0.0) * DUMMY_HOP,
        _ => 0.0,
    };

    *since_guard += time.delta_secs();
    let parry_now = *behavior == DummyBehavior::Guard && *since_guard >= 1.1;
    if parry_now {
        *since_guard = 0.0;
    }

    for (entity, mut transform, home) in &mut dummies {
        transform.translation.y = home.0 + lift;
        if parry_now {
            commands
                .entity(entity)
                .insert(crate::combat::guard_stance());
        }
    }
}

/// Recuo curto do dummy ao apanhar.
///
/// Ele nao ganha `Stunned` de proposito -- nao deve perder controle nem levar
/// knockback -- entao o retorno visual precisa de um timer proprio.
#[derive(Component)]
struct Flinch(Timer);

/// Traduz o estado do dummy em pose.
///
/// Sem isto ele fica congelado em `IdleA`: o modo `GUARD` apara sem que nada
/// apareca na tela, e nao da pra aprender a esperar uma defesa invisivel.
fn pose_dummy(
    time: Res<Time>,
    behavior: Res<DummyBehavior>,
    mut commands: Commands,
    mut damaged: MessageReader<crate::combat::Damaged>,
    mut dummies: Query<
        (
            Entity,
            &mut Pose,
            Option<&mut Flinch>,
            Has<crate::combat::Parrying>,
        ),
        With<TrainingDummy>,
    >,
) {
    let atingidos: Vec<Entity> = damaged.read().map(|hit| hit.target).collect();

    for (entity, mut pose, flinch, guarding) in &mut dummies {
        let recuando = if atingidos.contains(&entity) {
            commands
                .entity(entity)
                .insert(Flinch(Timer::from_seconds(0.18, TimerMode::Once)));
            true
        } else if let Some(mut flinch) = flinch {
            if flinch.0.tick(time.delta()).is_finished() {
                commands.entity(entity).remove::<Flinch>();
                false
            } else {
                true
            }
        } else {
            false
        };

        // Apanhar manda mais que aparar ou agachar: se ele levou dano, a
        // postura nao segurou.
        let next = if recuando {
            Pose::Hit
        } else if guarding {
            Pose::Parry
        } else if *behavior == DummyBehavior::Crouch {
            // `crouch_hurtbox` cuida do resto: ele encolhe a area atingivel de
            // quem esta nesta pose, sem saber que existe dummy.
            Pose::Crouch
        } else {
            Pose::IdleA
        };
        if *pose != next {
            *pose = next;
        }
    }
}

/// Componentes, spawn e movimento dos atores.
pub struct ActorPlugin;

impl Plugin for ActorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameMode>()
            .init_resource::<SkinSelections>()
            .init_resource::<DummyBehavior>()
            .add_plugins((input::InputPlugin, motion::MotionPlugin))
            .add_systems(OnEnter(GameState::Fighting), spawn_players)
            .add_systems(
                OnEnter(GameState::Fighting),
                spawn_training_dummy.run_if(resource_equals(GameMode::Training)),
            )
            // Todo quadro, e nao so ao entrar: o lobby e a unica tela onde a
            // lista de quem esta em campo muda sozinha, e o boneco de quem
            // acabou de entrar tem que aparecer junto com o nome dele.
            .add_systems(
                Update,
                sync_warmup
                    .in_set(crate::state::AppSet::Logic)
                    .run_if(in_state(GameState::Lobby))
                    .run_if(resource_equals(GameMode::Online)),
            )
            .add_systems(
                Update,
                drive_dummy
                    .in_set(crate::state::AppSet::Logic)
                    .run_if(in_state(GameState::Fighting))
                    .run_if(resource_equals(GameMode::Training)),
            )
            // Junto com os membros: rosto e membro sao a mesma ideia -- pecas
            // soltas posicionadas por cima do corpo -- e os dois leem a pose
            // depois de ela ja estar decidida neste quadro.
            .add_systems(
                Update,
                face::animate_face
                    .in_set(crate::state::AppSet::Animate)
                    .after(motion::apply_pose),
            )
            // Depois da resolucao de dano, junto com o resto da animacao: a
            // pose tem que refletir o que acabou de acontecer neste frame.
            .add_systems(
                Update,
                pose_dummy
                    .in_set(crate::state::AppSet::Animate)
                    .before(motion::apply_pose)
                    .run_if(in_state(GameState::Fighting))
                    .run_if(resource_equals(GameMode::Training)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O ponto de spawn nao e o chao -- e so de onde os jogadores caem.
    ///
    /// Este teste existe porque a primeira versao do dummy usou o ponto de
    /// spawn direto e ele passou a flutuar a altura inteira da queda, fora do
    /// alcance de qualquer golpe. O modo treino ficou intestavel sem que nada
    /// no jogo reclamasse.
    #[test]
    fn o_dummy_fica_de_pe_no_chao() {
        for build in crate::level::CATALOG {
            let level = build();
            let spawn = level.spawn_points()[1];
            let ground = level
                .ground_under(spawn)
                .unwrap_or_else(|| panic!("{}: spawn sem chao embaixo", level.name()));

            assert!(
                (dummy_stand_y(level.as_ref(), spawn) - (ground + BODY_SIZE.y * 0.5)).abs() < 0.01,
                "{}: os pes do dummy nao encostam no chao",
                level.name()
            );
            // Se o spawn ja fosse o chao, o bug nao teria como acontecer e
            // este teste nao provaria nada.
            assert!(
                spawn.y > ground + BODY_SIZE.y * 0.5,
                "{}: o spawn coincide com o chao, o teste perdeu o sentido",
                level.name()
            );
        }
    }

    /// Jogador e dummy tem que nascer com o mesmo corpo.
    ///
    /// Enquanto o spawn do dummy tinha copia propria da montagem, ele nascia
    /// sem membro nenhum -- um palito chapado ao lado de bonecos articulados
    /// -- e nada apontava isso.
    #[test]
    fn jogador_e_dummy_nascem_com_o_mesmo_corpo() {
        let mut app = App::new();
        app.insert_resource(crate::level::CurrentLevel(crate::level::level_at(0)))
            .insert_resource(GameMode::Training)
            .init_resource::<SkinSelections>()
            .add_systems(Update, (spawn_players, spawn_training_dummy));
        app.update();

        let world = app.world_mut();
        let donos: Vec<Entity> = world
            .query::<&ActorLimb>()
            .iter(world)
            .map(|limb| limb.owner)
            .collect();
        let corpos: Vec<Entity> = world
            .query_filtered::<Entity, Or<(With<Player>, With<TrainingDummy>)>>()
            .iter(world)
            .collect();

        assert_eq!(corpos.len(), 2, "o treino tem um jogador e um dummy");
        for corpo in corpos {
            assert_eq!(
                donos.iter().filter(|dono| **dono == corpo).count(),
                8,
                "corpo nasceu sem os oito membros"
            );
            assert!(
                world.get::<BreathPhase>(corpo).is_some(),
                "corpo nasceu sem fase de respiracao e ficaria fora de animate_body"
            );
        }
    }

    #[test]
    fn jogadores_nascem_com_as_peles_escolhidas() {
        let mut app = App::new();
        app.insert_resource(crate::level::CurrentLevel(crate::level::level_at(0)))
            .insert_resource(GameMode::Versus)
            .insert_resource(SkinSelections {
                players: [4, 5, 0, 1],
                ..default()
            })
            .add_systems(Update, spawn_players);
        app.update();

        let world = app.world_mut();
        let mut chosen: Vec<_> = world
            .query::<(&Player, &ActorSkin)>()
            .iter(world)
            .map(|(player, skin)| (player.id, skin.0.name))
            .collect();
        chosen.sort_by_key(|(id, _)| *id);
        assert_eq!(chosen, vec![(0, "ROBOT"), (1, "INFERNO")]);
    }

    /// O dummy tem que mostrar o que esta fazendo.
    ///
    /// A guarda do modo `GUARD` dura 0,3 s de cada 1,1 s. Se ela nao aparecer
    /// na tela nao ha o que treinar -- o jogador so descobre que o golpe foi
    /// aparado depois de tomar o contragolpe.
    #[test]
    fn o_dummy_mostra_a_guarda_e_o_recuo() {
        let mut app = App::new();
        app.init_resource::<Time>()
            .insert_resource(DummyBehavior::Still)
            .add_message::<crate::combat::Damaged>()
            .add_systems(Update, pose_dummy);

        let dummy = app.world_mut().spawn((TrainingDummy, Pose::IdleA)).id();

        app.update();
        assert_eq!(
            *app.world().get::<Pose>(dummy).unwrap(),
            Pose::IdleA,
            "parado, o dummy nao deveria mudar de pose"
        );

        app.world_mut()
            .entity_mut(dummy)
            .insert(crate::combat::guard_stance());
        app.update();
        assert_eq!(
            *app.world().get::<Pose>(dummy).unwrap(),
            Pose::Parry,
            "a guarda nao aparece na tela"
        );

        // O modo agachado tem que virar a pose que `crouch_hurtbox` procura,
        // senao o dummy "agacha" sem encolher e nao ha o que treinar.
        app.world_mut()
            .entity_mut(dummy)
            .remove::<crate::combat::Parrying>();
        app.insert_resource(DummyBehavior::Crouch);
        app.update();
        assert_eq!(
            *app.world().get::<Pose>(dummy).unwrap(),
            Pose::Crouch,
            "o modo CROUCH nao agacha o dummy"
        );
        app.insert_resource(DummyBehavior::Still);

        // Apanhar manda mais que aparar ou agachar: se levou dano, a postura
        // nao segurou.
        app.world_mut().write_message(crate::combat::Damaged {
            target: dummy,
            amount: 8,
            at: Vec2::ZERO,
            dir: Vec2::X,
            move_name: "JAB",
            explosive: false,
        });
        app.update();
        assert_eq!(
            *app.world().get::<Pose>(dummy).unwrap(),
            Pose::Hit,
            "o dummy nao reage a apanhar"
        );
    }

    #[test]
    fn fracao_de_vida_fica_no_intervalo() {
        assert_eq!(Health::full(100).fraction(), 1.0);
        assert_eq!(Health { hp: 50, max: 100 }.fraction(), 0.5);
        assert_eq!(Health { hp: -30, max: 100 }.fraction(), 0.0);
    }

    #[test]
    fn vida_zerada_conta_como_morte() {
        assert!(Health { hp: 0, max: 100 }.is_dead());
        assert!(!Health { hp: 1, max: 100 }.is_dead());
    }
}
