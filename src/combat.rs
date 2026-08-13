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

/// Quanto tempo o hitbox do soco fica no ar.
const MELEE_ACTIVE: f32 = 0.075;
/// Altura da caixa de um golpe corpo-a-corpo.
///
/// Junto com [`melee_height`] e o tamanho do corpo, e ela que decide o que
/// alcanca quem esta no ar e o que passa por baixo.
const MELEE_BOX_H: f32 = 42.0;
/// Atordoamento apos levar dano. O padrao de todo golpe que nao derruba.
pub const HIT_STUN: f32 = 0.26;
/// Invulnerabilidade apos levar dano.
const HIT_INVULN: f32 = 0.34;
/// Espera entre a morte e a tela de fim de round.
const ROUND_END_DELAY: f32 = 1.4;
const COMBO_GRACE: f32 = 0.44;
const PARRY_TOTAL: f32 = 0.30;
const PARRY_ACTIVE: f32 = 0.13;
const PARRY_COOLDOWN: f32 = 0.58;

/// Uma area que causa dano. Vale para soco e para projetil.
#[derive(Component, Debug)]
pub struct Hitbox {
    /// Quem disparou -- nunca acerta o proprio dono.
    pub owner: Entity,
    /// Dano aplicado.
    pub damage: i32,
    /// Empurrao aplicado ao alvo.
    pub knockback: Vec2,
    /// Quanto tempo o alvo fica sem controle. A rasteira usa isso para
    /// derrubar: ela troca dano por vantagem.
    pub stun: f32,
}

/// Marca um hitbox como estouro.
///
/// Nao muda nada na resolucao de dano -- e um recado para a camada de gore, que
/// trata quem morre num estouro diferente de quem morre de porrada. Fica como
/// componente, e nao como campo do [`Hitbox`], para que uma arma nova continue
/// podendo nascer sem opinar sobre sangue.
#[derive(Component)]
pub struct Explosive;

/// Hitbox que acompanha quem a criou em vez de ficar onde nasceu.
///
/// A voadora precisa disso: ela viaja junto com o corpo, entao uma area parada
/// no ponto de contato erraria todo mundo que nao estivesse exatamente ali no
/// instante certo. Guarda o deslocamento ja com o sinal do lado.
#[derive(Component)]
struct FollowsOwner(Vec2);

/// Arrasta as hitboxes que seguem o dono, antes de resolver dano.
fn move_following_hitboxes(
    owners: Query<&Transform, Without<FollowsOwner>>,
    mut boxes: Query<(&Hitbox, &FollowsOwner, &mut Transform)>,
) {
    for (hitbox, follow, mut transform) in &mut boxes {
        let Ok(owner) = owners.get(hitbox.owner) else {
            continue;
        };
        let at = owner.translation.truncate() + follow.0;
        transform.translation.x = at.x;
        transform.translation.y = at.y;
    }
}

/// Despawn automatico ao fim do timer.
#[derive(Component, Debug)]
pub struct Lifetime(pub Timer);

/// Imune a dano por um instante.
#[derive(Component, Debug)]
pub struct Invulnerable(pub Timer);

/// Area que pode ser atingida, quando ela difere do colisor de terreno.
///
/// Agachar precisa encolher o que da pra acertar sem encolher o que colide com
/// o chao -- mexer no `Collider` faria o boneco afundar ou flutuar, porque a
/// resolucao de colisao usa ele centrado na `Transform`. Separar as duas
/// caixas e o que permite ter postura sem mexer na fisica.
#[derive(Component, Debug, Clone, Copy)]
pub struct Hurtbox {
    /// Meia-largura e meia-altura.
    pub half: Vec2,
    /// Deslocamento do centro em relacao a `Transform`.
    pub offset: Vec2,
}

impl Hurtbox {
    /// Postura agachada de um corpo com este colisor.
    ///
    /// A caixa desce junto com o encolhimento: os pes ficam onde estavam e o
    /// que some e o topo, que e exatamente o que o gancho procura.
    pub fn crouched(collider: &Collider) -> Self {
        let half = Vec2::new(collider.half.x, collider.half.y * CROUCH_HEIGHT);
        Self {
            half,
            offset: Vec2::new(0.0, half.y - collider.half.y),
        }
    }

    /// Area atingivel no mundo, dado o centro da entidade.
    pub fn aabb(&self, center: Vec2) -> Rect {
        Rect::from_center_half_size(center + self.offset, self.half)
    }
}

/// Fracao da altura do corpo que sobra quando ele agacha.
const CROUCH_HEIGHT: f32 = 0.5;

/// Encolhe a area atingivel de quem esta agachado, e devolve ao levantar.
///
/// A caixa desce junto: os pes ficam onde estavam e o que some e o topo, que
/// e exatamente o que o gancho procura.
fn crouch_hurtbox(
    mut commands: Commands,
    actors: Query<(Entity, &Pose, &Collider, Has<Hurtbox>), Changed<Pose>>,
) {
    for (entity, pose, collider, tem_hurtbox) in &actors {
        if *pose == Pose::Crouch {
            commands.entity(entity).insert(Hurtbox::crouched(collider));
        } else if tem_hurtbox {
            commands.entity(entity).remove::<Hurtbox>();
        }
    }
}

/// Emitida quando alguem toma dano. A camada de FX escuta isso.
///
/// Carrega o evento inteiro, nao so o que o efeito atual consome: quem quiser
/// somar dano, tocar som ou sacudir a camera ja tem o que precisa.
#[derive(Message, Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct Damaged {
    /// Quem apanhou.
    pub target: Entity,
    /// Quanto.
    pub amount: i32,
    /// Onde, em coordenadas de mundo.
    pub at: Vec2,
    /// Direcao do golpe, normalizada.
    pub dir: Vec2,
    /// Nome do golpe corpo-a-corpo que acertou; vazio para tiro e arremesso.
    pub move_name: &'static str,
    /// Veio de um estouro.
    ///
    /// Quem apanha nao se importa -- o dano e o mesmo -- mas quem desenha
    /// sangue sim: estouro desmonta o boneco, soco so arranca pedaco.
    pub explosive: bool,
}

#[derive(Message, Debug, Clone, Copy)]
pub struct Parried {
    pub defender: Entity,
    pub attacker: Entity,
    pub at: Vec2,
}

/// Rounds que levam a partida.
pub const MATCH_WINS: u32 = 3;

/// Resultado do round corrente e da partida em andamento.
#[derive(Resource, Debug, PartialEq, Eq)]
pub struct RoundResult {
    /// Indice do vencedor do ultimo round, se houve.
    pub winner: Option<u8>,
    /// Rounds vencidos por cada um nesta partida.
    pub score: [u32; MAX_PLAYERS],
    /// Rounds ja disputados, empate incluso.
    ///
    /// Contado a parte da soma do placar porque empate encerra round sem dar
    /// ponto a ninguem -- sem isto o contador travaria no mesmo numero.
    pub rounds: u32,
    /// Lugares que esta partida usa.
    ///
    /// O placar tem sempre quatro casas, mas so estas valem: sem isto uma sala
    /// de dois mostraria dois lugares vazios zerados na tela de fim de round.
    pub players: u8,
}

impl Default for RoundResult {
    fn default() -> Self {
        Self {
            winner: None,
            score: [0; MAX_PLAYERS],
            rounds: 0,
            players: MIN_PLAYERS as u8,
        }
    }
}

impl RoundResult {
    /// Os lugares em jogo, para quem precisa varrer o placar.
    pub fn seats(&self) -> usize {
        (self.players as usize).clamp(MIN_PLAYERS, MAX_PLAYERS)
    }

    /// Quem ja levou a partida, se alguem levou.
    pub fn match_winner(&self) -> Option<u8> {
        self.score
            .iter()
            .position(|wins| *wins >= MATCH_WINS)
            .map(|index| index as u8)
    }
}

/// Contagem regressiva ate a tela de fim de round.
#[derive(Resource, Debug)]
pub(crate) struct RoundEndDelay(Timer);

/// Placar vivo do treino.
///
/// Um combo termina quando passa [`COMBO_DROP`] sem nenhum acerto novo no
/// dummy -- a mesma ideia da janela de encadeamento, so que medida do lado de
/// quem apanha, para que arma arremessada e projetil tambem contem.
#[derive(Resource, Debug)]
pub struct ComboMeter {
    /// Acertos do combo em andamento.
    pub hits: u32,
    /// Dano somado no combo em andamento.
    pub damage: i32,
    /// Maior contagem de acertos da sessao.
    pub best_hits: u32,
    /// Maior dano somado da sessao.
    pub best_damage: i32,
    /// Nome do ultimo golpe que acertou, para o painel nomear o que aconteceu.
    pub last_move: &'static str,
    /// Tempo desde o ultimo acerto.
    idle: f32,
}

impl Default for ComboMeter {
    fn default() -> Self {
        Self {
            hits: 0,
            damage: 0,
            best_hits: 0,
            best_damage: 0,
            last_move: "",
            idle: f32::MAX,
        }
    }
}

/// Silencio que encerra um combo no treino.
const COMBO_DROP: f32 = 1.1;

#[derive(Component, Debug)]
pub struct MeleeAttack {
    pub step: u8,
    pub style: WeaponStyle,
    /// Elo de combo, pancada pesada ou rasteira. Escolhe a coreografia e diz
    /// se o golpe encadeia.
    pub kind: MeleeKind,
    move_data: MeleeMove,
    launched: bool,
}

#[derive(Component, Debug)]
struct ComboChain {
    next: u8,
    grace: Timer,
}

#[derive(Component)]
struct QueuedAttack;

#[derive(Component, Debug)]
pub struct Parrying(pub Timer);

/// Uma postura de defesa recem-iniciada.
///
/// Existe para o dummy de treino poder aparar sem que `actor` precise conhecer
/// os tempos de parry -- eles sao regra de combate e ficam aqui.
pub fn guard_stance() -> Parrying {
    Parrying(Timer::from_seconds(PARRY_TOTAL, TimerMode::Once))
}

#[derive(Component, Debug)]
struct ParryCooldown(Timer);

fn unarmed_move(step: u8) -> MeleeMove {
    match step % 3 {
        0 => MeleeMove {
            damage: 8,
            reach: 28.0,
            knockback: Vec2::new(225.0, 100.0),
            duration: 0.18,
            contact: 0.27,
        },
        1 => MeleeMove {
            damage: 9,
            reach: 31.0,
            knockback: Vec2::new(255.0, 145.0),
            duration: 0.20,
            contact: 0.25,
        },
        _ => MeleeMove {
            damage: 14,
            reach: 35.0,
            knockback: Vec2::new(360.0, 235.0),
            duration: 0.28,
            contact: 0.39,
        },
    }
}

fn begin_melee(commands: &mut Commands, entity: Entity, step: u8, held: Option<&Held>) {
    let (style, move_data) = held
        .map(|held| (held.weapon.style(), held.weapon.melee(step)))
        .unwrap_or((WeaponStyle::Unarmed, unarmed_move(step)));
    commands.entity(entity).insert((
        Attacking(Timer::from_seconds(move_data.duration, TimerMode::Once)),
        MeleeAttack {
            step,
            style,
            kind: MeleeKind::Chain,
            move_data,
            launched: false,
        },
        ComboChain {
            next: (step + 1) % 3,
            grace: Timer::from_seconds(COMBO_GRACE, TimerMode::Once),
        },
    ));
}

/// Rasteira do combate desarmado.
///
/// Dano baixo, mas o alvo fica no chao o dobro do tempo -- ela compra
/// vantagem, nao vida. Somada ao combo em pe, e o que da ao ataque uma leitura
/// alta e uma baixa em vez de um botao so.
const SWEEP_MOVE: MeleeMove = MeleeMove {
    damage: 7,
    reach: 34.0,
    knockback: Vec2::new(160.0, 60.0),
    duration: 0.30,
    contact: 0.42,
};

/// Quanto tempo a rasteira deixa o alvo no chao.
const SWEEP_STUN: f32 = 0.62;

/// Voadora: o golpe do ar.
///
/// Ela e comprometida de proposito -- quem aperta troca o controle da queda
/// pela chance de acertar. Errar significa aterrissar onde o oponente escolher.
const DIVE_MOVE: MeleeMove = MeleeMove {
    damage: 11,
    reach: 26.0,
    knockback: Vec2::new(240.0, -280.0),
    duration: 0.50,
    contact: 0.20,
};

/// Impulso da voadora. Substitui a velocidade em vez de somar: e o mergulho
/// que faz o golpe valer o risco.
const DIVE_LAUNCH: Vec2 = Vec2::new(300.0, -560.0);

/// Voadora ja gasta neste pulo. Sai ao encostar no chao.
///
/// Sem isso a voadora se cancelaria e se repetiria no ar, e o jogador nunca
/// perderia a altura que o golpe deveria custar.
#[derive(Component)]
struct DiveSpent;

/// Baixo + M1 com os pes no chao: rasteira.
///
/// Nao encadeia, igual a pancada pesada -- ela e o fim de uma sequencia
/// ("jab, cross, rasteira"), nao mais um elo dela.
fn start_sweep(
    mut commands: Commands,
    q: Query<
        (Entity, &Intent, &Pose, &Grounded),
        (With<Player>, Without<Attacking>, Without<Parrying>),
    >,
) {
    for (entity, intent, pose, grounded) in &q {
        if !intent.attack || !intent.down || !grounded.0 || pose.locks_control() {
            continue;
        }
        commands.entity(entity).insert((
            Attacking(Timer::from_seconds(SWEEP_MOVE.duration, TimerMode::Once)),
            MeleeAttack {
                step: 0,
                style: WeaponStyle::Unarmed,
                kind: MeleeKind::Sweep,
                move_data: SWEEP_MOVE,
                launched: false,
            },
        ));
        commands
            .entity(entity)
            .remove::<(ComboChain, QueuedAttack)>();
    }
}

/// M1 fora do chao: voadora.
///
/// Uma por pulo. O `DiveSpent` so sai ao aterrissar, entao nao da pra ficar
/// picotando o golpe no ar pra planar.
fn start_dive(
    mut commands: Commands,
    q: Query<
        (Entity, &Intent, &Pose, &Grounded),
        (
            With<Player>,
            Without<Attacking>,
            Without<Parrying>,
            Without<DiveSpent>,
        ),
    >,
) {
    for (entity, intent, pose, grounded) in &q {
        if !intent.attack || grounded.0 || pose.locks_control() {
            continue;
        }
        commands.entity(entity).insert((
            DiveSpent,
            Attacking(Timer::from_seconds(DIVE_MOVE.duration, TimerMode::Once)),
            MeleeAttack {
                step: 0,
                style: WeaponStyle::Unarmed,
                kind: MeleeKind::Dive,
                move_data: DIVE_MOVE,
                launched: false,
            },
        ));
        commands
            .entity(entity)
            .remove::<(ComboChain, QueuedAttack)>();
    }
}

/// Aterrissar encerra a voadora e devolve o golpe para o proximo pulo.
///
/// Sem isso o boneco pousaria e ficaria parado na pose de chute ate o timer
/// acabar, sem conseguir se defender.
fn land_dive(
    mut commands: Commands,
    q: Query<(Entity, &Grounded, Option<&MeleeAttack>), With<DiveSpent>>,
) {
    for (entity, grounded, melee) in &q {
        if !grounded.0 {
            continue;
        }
        commands.entity(entity).remove::<DiveSpent>();
        if melee.is_some_and(|m| m.kind == MeleeKind::Dive) {
            commands.entity(entity).remove::<(MeleeAttack, Attacking)>();
        }
    }
}

/// M2 de uma arma de contato: um golpe unico, caro, que nao entra no combo.
///
/// Nao insere `ComboChain` de proposito -- encadear a pancada pesada faria dela
/// a unica coisa que vale apertar.
fn start_heavy(
    mut commands: Commands,
    mut q: Query<
        (Entity, &Intent, &Pose, &mut Facing, &mut Held),
        (With<Player>, Without<Attacking>, Without<Parrying>),
    >,
) {
    for (entity, intent, pose, mut facing, mut held) in &mut q {
        if !intent.special || pose.locks_control() {
            continue;
        }
        let Some(move_data) = held.weapon.heavy() else {
            continue;
        };
        // Quem tica o timer e `fire`, que roda para toda arma equipada; aqui
        // ele so e consumido, senao a cadencia correria em dobro.
        if !held.cooldown.is_finished() {
            continue;
        }
        held.cooldown.reset();
        crate::weapon::turn_to_aim(&mut facing, intent);
        commands.entity(entity).insert((
            Attacking(Timer::from_seconds(move_data.duration, TimerMode::Once)),
            MeleeAttack {
                step: 2,
                style: held.weapon.style(),
                kind: MeleeKind::Heavy,
                move_data,
                launched: false,
            },
        ));
        // Um golpe pesado corta qualquer combo em andamento: ele e o fim da
        // sequencia, nao mais um elo dela.
        commands
            .entity(entity)
            .remove::<(ComboChain, QueuedAttack)>();
    }
}

/// M1 sempre e corpo-a-corpo; cada arma troca alcance, ritmo e finalizador.
fn start_melee(
    mut commands: Commands,
    mut q: Query<
        (
            Entity,
            &Intent,
            &Pose,
            &Grounded,
            &mut Facing,
            Option<&Held>,
            Option<&ComboChain>,
        ),
        (With<Player>, Without<Attacking>, Without<Parrying>),
    >,
) {
    for (entity, intent, pose, grounded, mut facing, held, combo) in &mut q {
        // Baixo + M1 e a rasteira e fora do chao e a voadora; os dois tem
        // sistema proprio. Aqui fica so o combo em pe.
        if !intent.attack || intent.down || !grounded.0 || pose.locks_control() {
            continue;
        }
        crate::weapon::turn_to_aim(&mut facing, intent);
        let step = combo
            .filter(|c| !c.grace.is_finished())
            .map_or(0, |c| c.next);
        begin_melee(&mut commands, entity, step, held);
    }
}

fn queue_melee(
    mut commands: Commands,
    q: Query<(Entity, &Intent), (With<Player>, With<Attacking>, With<MeleeAttack>)>,
) {
    for (entity, intent) in &q {
        if intent.attack {
            commands.entity(entity).insert(QueuedAttack);
        }
    }
}

/// Altura em que a hitbox abre, relativa ao centro do atacante.
///
/// E o que o oponente le. A rasteira nasce na canela e por isso passa por
/// baixo de quem esta no ar; o gancho nasce no alto e e a resposta pra esse
/// caso. Sem essa diferenca, alto e baixo seriam o mesmo golpe com nomes
/// distintos.
fn melee_height(kind: MeleeKind, step: u8) -> f32 {
    match kind {
        MeleeKind::Sweep => -18.0,
        // A voadora desce em cima do alvo: o pe chega abaixo do centro dela.
        MeleeKind::Dive => -12.0,
        // A pancada termina com o cano embaixo, entao ela abre onde a mao
        // para -- e nao no alto, como o gancho.
        MeleeKind::Heavy => -6.0,
        // O gancho sobe de verdade: alto o bastante para passar por cima de
        // quem esta agachado, que e a resposta defensiva a ele.
        _ if step == 2 => 22.0,
        _ => 2.0,
    }
}

/// Quanto tempo a hitbox fica no ar.
///
/// A voadora precisa de uma janela longa porque ela viaja com o corpo -- com
/// os 0,075 s dos outros golpes ela so acertaria quem estivesse exatamente no
/// ponto de contato.
fn melee_active(kind: MeleeKind) -> f32 {
    match kind {
        MeleeKind::Dive => 0.26,
        _ => MELEE_ACTIVE,
    }
}

/// O golpe sai na direcao da mira, ou apenas para o lado em que o boneco olha?
///
/// Soco e pancada saem do braco, e o braco aponta para onde o jogador esta
/// olhando -- o mesmo contrato que o tiro ja tinha. Rasteira e voadora saem da
/// perna e do corpo inteiro: a rasteira varre o chao e a voadora e a propria
/// queda, entao mandar as duas para o cursor tiraria delas o que as define.
fn follows_aim(kind: MeleeKind) -> bool {
    matches!(kind, MeleeKind::Chain | MeleeKind::Heavy)
}

/// Para onde este golpe vai.
fn strike_dir(kind: MeleeKind, intent: &Intent, facing: &Facing) -> Vec2 {
    if follows_aim(kind) {
        crate::weapon::aim_dir(intent, facing)
    } else {
        Vec2::new(facing.0, 0.0)
    }
}

/// Abre a hitbox exatamente no quadro visual de contato.
fn launch_melee(
    mut commands: Commands,
    mut q: Query<
        (
            Entity,
            &Transform,
            &Facing,
            &Intent,
            &Attacking,
            &mut MeleeAttack,
            &mut Velocity,
        ),
        With<Player>,
    >,
) {
    for (entity, transform, facing, intent, attacking, mut attack, mut velocity) in &mut q {
        if attack.launched
            || attacking.0.elapsed_secs() < attack.move_data.duration * attack.move_data.contact
        {
            continue;
        }
        attack.launched = true;
        let dir = strike_dir(attack.kind, intent, facing);
        match attack.kind {
            // A voadora troca a velocidade inteira: quem aperta abre mao do
            // controle da queda em troca da chance de acertar.
            MeleeKind::Dive => velocity.0 = Vec2::new(facing.0 * DIVE_LAUNCH.x, DIVE_LAUNCH.y),
            // O passo do golpe acompanha so o quanto a mira aponta para o lado:
            // socar para cima nao pode empurrar o corpo para frente.
            _ => velocity.x += dir.x * (70.0 + attack.step as f32 * 28.0),
        }
        // O punho vai parar onde o cursor esta; a altura continua sendo o que
        // separa gancho de rasteira, e por isso ela e somada, nao girada.
        let offset =
            dir * attack.move_data.reach + Vec2::Y * melee_height(attack.kind, attack.step);
        let at = transform.translation.truncate() + offset;
        let mut hitbox = commands.spawn((
            Hitbox {
                owner: entity,
                damage: attack.move_data.damage,
                // O empurrao sai na linha do golpe, e o levante continua sendo
                // para cima: quem soca de cima para baixo prega o outro no
                // chao, e quem soca para cima o joga para o alto.
                knockback: dir * attack.move_data.knockback.x
                    + Vec2::Y * attack.move_data.knockback.y,
                stun: if attack.kind == MeleeKind::Sweep {
                    SWEEP_STUN
                } else {
                    HIT_STUN
                },
            },
            Lifetime(Timer::from_seconds(
                melee_active(attack.kind),
                TimerMode::Once,
            )),
            Collider::size(32.0 + attack.move_data.reach * 0.18, MELEE_BOX_H),
            Transform::from_translation(at.extend(0.0)),
            DespawnOnExit(GameState::Fighting),
        ));
        if attack.kind == MeleeKind::Dive {
            hitbox.insert(FollowsOwner(offset));
        }
        let arc = match attack.style {
            WeaponStyle::Unarmed => {
                if attack.step == 2 {
                    "))"
                } else {
                    ")"
                }
            }
            WeaponStyle::Pistol => "-)>",
            // Cortes curtos, arco longo de lamina e giro de corrente tem
            // silhuetas proprias para o estilo ser legivel no contato.
            WeaponStyle::Knife => "/)",
            WeaponStyle::Katana => "====/)",
            WeaponStyle::Nunchaku => "o~)))",
            WeaponStyle::Shotgun => "==)>",
            WeaponStyle::Rifle => "---)>",
            WeaponStyle::Bomb => ")",
            // A pancada pesada desce, entao o rastro dela e mais largo que o
            // do combo normal do cano.
            WeaponStyle::Pipe => {
                if attack.kind == MeleeKind::Heavy {
                    "=))))"
                } else {
                    "=))"
                }
            }
        };
        // O rastro e desenhado apontando para a direita: espelha quando o golpe
        // vai para o outro lado e gira ate a linha da mira. Sem o giro, um
        // gancho para cima abria a hitbox no alto e desenhava o arco deitado.
        let flipped = dir.x < 0.0;
        let angle = dir.to_angle() - if flipped { std::f32::consts::PI } else { 0.0 };
        commands.spawn((
            AsciiSprite::new(AsciiArt::solid(arc, palette::GOLD)).flipped(flipped),
            Layer::Fx,
            Transform::from_translation(at.extend(0.0))
                .with_rotation(Quat::from_rotation_z(angle)),
            Lifetime(Timer::from_seconds(0.11, TimerMode::Once)),
            DespawnOnExit(GameState::Fighting),
        ));
    }
}

fn continue_combo(
    mut commands: Commands,
    q: Query<
        (
            Entity,
            Option<&Held>,
            Option<&ComboChain>,
            Has<QueuedAttack>,
        ),
        (With<MeleeAttack>, Without<Attacking>),
    >,
) {
    for (entity, held, combo, queued) in &q {
        commands.entity(entity).remove::<MeleeAttack>();
        // Pancada, rasteira e voadora nao encadeiam, entao nao tem
        // `ComboChain`. Sem este caminho o `MeleeAttack` delas ficaria
        // pendurado no jogador depois do golpe acabar.
        let Some(combo) = combo else {
            continue;
        };
        if queued && !combo.grace.is_finished() {
            commands.entity(entity).remove::<QueuedAttack>();
            begin_melee(&mut commands, entity, combo.next, held);
        }
    }
}

fn start_parry(
    mut commands: Commands,
    q: Query<
        (Entity, &Intent, &Pose),
        (
            With<Player>,
            Without<Attacking>,
            Without<Parrying>,
            Without<ParryCooldown>,
        ),
    >,
) {
    for (entity, intent, pose) in &q {
        if intent.parry && !pose.locks_control() {
            commands.entity(entity).insert((
                Parrying(Timer::from_seconds(PARRY_TOTAL, TimerMode::Once)),
                ParryCooldown(Timer::from_seconds(PARRY_COOLDOWN, TimerMode::Once)),
            ));
        }
    }
}

/// Resolve todo hitbox contra todo jogador vulneravel.
fn resolve_hits(
    mut commands: Commands,
    mut damaged: MessageWriter<Damaged>,
    mut parried: MessageWriter<Parried>,
    mut hitboxes: Query<
        (
            Entity,
            &mut Hitbox,
            &Transform,
            &Collider,
            Option<&mut Velocity>,
            Has<ThrownWeapon>,
            Has<Sticky>,
            Has<Explosive>,
        ),
        Without<Player>,
    >,
    attackers: Query<&MeleeAttack>,
    mut targets: Query<
        (
            Entity,
            &Transform,
            &Collider,
            &mut Health,
            Option<&mut Velocity>,
            Option<&Parrying>,
            Option<&Hurtbox>,
            Has<TrainingDummy>,
        ),
        (
            Or<(With<Player>, With<TrainingDummy>)>,
            Without<Invulnerable>,
            Without<Hitbox>,
        ),
    >,
) {
    for (
        hit_entity,
        mut hitbox,
        hit_transform,
        hit_collider,
        mut projectile_velocity,
        thrown,
        sticky,
        explosive,
    ) in &mut hitboxes
    {
        let hit_at = hit_transform.translation.truncate();
        let area = hit_collider.aabb(hit_at);

        for (target, transform, collider, mut health, velocity, parry, hurtbox, dummy) in
            &mut targets
        {
            if target == hitbox.owner || health.is_dead() {
                continue;
            }
            // Postura manda: quem esta agachado oferece menos area que o
            // proprio colisor de terreno.
            let at = transform.translation.truncate();
            let body = match hurtbox {
                Some(hurtbox) => hurtbox.aabb(at),
                None => collider.aabb(at),
            };
            if !overlap(area, body) {
                continue;
            }

            if parry.is_some_and(|p| p.0.elapsed_secs() <= PARRY_ACTIVE) {
                let attacker = hitbox.owner;
                if let Some(projectile_velocity) = projectile_velocity.as_deref_mut() {
                    hitbox.owner = target;
                    hitbox.knockback.x *= -1.2;
                    projectile_velocity.0 *= -1.18;
                } else {
                    commands.entity(hit_entity).despawn();
                }
                commands
                    .entity(attacker)
                    .insert(Stunned(Timer::from_seconds(0.38, TimerMode::Once)));
                parried.write(Parried {
                    defender: target,
                    attacker,
                    at: transform.translation.truncate(),
                });
                break;
            }

            health.hp = (health.hp - hitbox.damage).max(if dummy { 1 } else { i32::MIN });
            // Quanto mais ferido, mais longe voa: o round escala naturalmente
            // para um final explosivo e quedas no vao ficam mais provaveis.
            let launch = 1.0 + (1.0 - health.fraction()) * 0.75;
            if let Some(mut velocity) = velocity {
                velocity.0 = hitbox.knockback * launch;
            }

            // O dummy nao ganha janela de invulnerabilidade: com 0.34 s de
            // imunidade os elos do combo (0.18-0.28 s) cairiam no vazio e o
            // treino mediria um combo que o jogo real nao tem.
            if !dummy {
                commands.entity(target).insert((
                    Stunned(Timer::from_seconds(hitbox.stun, TimerMode::Once)),
                    Invulnerable(Timer::from_seconds(HIT_INVULN, TimerMode::Once)),
                ));
                // Golpe que atordoa mais que o normal derruba. Sem a pose de
                // queda o alvo passaria a duracao inteira em pe, na animacao
                // de recuo, e a vantagem da rasteira nao leria na tela.
                if hitbox.stun > HIT_STUN {
                    commands.entity(target).insert(Downed);
                }
            }

            damaged.write(Damaged {
                target,
                amount: hitbox.damage,
                at: transform.translation.truncate(),
                dir: hitbox.knockback.normalize_or_zero(),
                // Um hitbox de soco nasce enquanto o dono ainda esta no golpe,
                // entao da pra perguntar a ele qual elo do combo era.
                move_name: attackers.get(hitbox.owner).map_or("", |attack| {
                    crate::actor::pose::strike_for(attack.step, attack.kind, attack.style).name
                }),
                explosive,
            });

            // Um hitbox acerta uma vez e sai de cena.
            if thrown {
                commands.entity(hit_entity).remove::<Hitbox>();
            } else if sticky {
                // A faca nao some no alvo: ela fica cravada nele. Aqui so fica
                // o pedido -- desmontar projetil (rastro, velocidade, colisao)
                // e coisa de quem sabe o que um projetil e.
                commands.entity(hit_entity).remove::<Hitbox>().insert(
                    crate::weapon::StuckInto {
                        host: target,
                        offset: hit_at - at,
                    },
                );
            } else {
                commands.entity(hit_entity).despawn();
            }
            break;
        }
    }
}

/// Consome `Lifetime` e `Invulnerable`.
fn tick_timers(
    time: Res<Time>,
    mut commands: Commands,
    mut lifetimes: Query<(Entity, &mut Lifetime)>,
    mut invulns: Query<(Entity, &mut Invulnerable)>,
    mut combos: Query<(Entity, &mut ComboChain), Without<Attacking>>,
    mut parries: Query<(Entity, &mut Parrying)>,
    mut parry_cooldowns: Query<(Entity, &mut ParryCooldown)>,
) {
    for (entity, mut life) in &mut lifetimes {
        if life.0.tick(time.delta()).is_finished() {
            commands.entity(entity).despawn();
        }
    }
    for (entity, mut invuln) in &mut invulns {
        if invuln.0.tick(time.delta()).is_finished() {
            commands.entity(entity).remove::<Invulnerable>();
        }
    }
    for (entity, mut combo) in &mut combos {
        if combo.grace.tick(time.delta()).is_finished() {
            commands
                .entity(entity)
                .remove::<(ComboChain, QueuedAttack)>();
        }
    }
    for (entity, mut parry) in &mut parries {
        if parry.0.tick(time.delta()).is_finished() {
            commands.entity(entity).remove::<Parrying>();
        }
    }
    for (entity, mut cooldown) in &mut parry_cooldowns {
        if cooldown.0.tick(time.delta()).is_finished() {
            commands.entity(entity).remove::<ParryCooldown>();
        }
    }
}

/// Detecta a morte e agenda o fim do round.
///
/// A regra e sobrar um, nao alguem morrer: com dois em campo da no mesmo, com
/// quatro a primeira morte so tira um da briga. Publica porque quem cuida de
/// desistencia online precisa correr antes desta decisao no mesmo quadro.
pub(crate) fn check_round_over(
    time: Res<Time>,
    mut commands: Commands,
    mut result: ResMut<RoundResult>,
    mut next: ResMut<NextState<GameState>>,
    delay: Option<ResMut<RoundEndDelay>>,
    players: Query<(&Player, &Health)>,
) {
    let total = players.iter().count();
    let alive: Vec<u8> = players
        .iter()
        .filter(|(_, h)| !h.is_dead())
        .map(|(p, _)| p.id)
        .collect();

    match delay {
        // Precisa haver briga para haver round. Sem esta guarda o quadro em que
        // ninguem nasceu ainda ja contaria como "sobrou um" -- zero vivos --
        // e a luta acabaria antes de comecar.
        None if total >= MIN_PLAYERS && alive.len() <= 1 => {
            // Quem sobrou venceu; se todos morreram junto, e empate.
            result.winner = alive.first().copied();
            result.players = total as u8;
            if let Some(winner) = result.winner {
                result.score[winner as usize] += 1;
            }
            result.rounds += 1;
            commands.insert_resource(RoundEndDelay(Timer::from_seconds(
                ROUND_END_DELAY,
                TimerMode::Once,
            )));
        }
        Some(mut delay) => {
            if delay.0.tick(time.delta()).is_finished() {
                commands.remove_resource::<RoundEndDelay>();
                next.set(GameState::RoundOver);
            }
        }
        None => {}
    }
}

/// Comeca partida nova quando a anterior ja teve vencedor.
///
/// Fica aqui e nao no `Enter` da tela porque a regra e de combate: quem decide
/// que a partida acabou e o placar, nao a UI.
fn start_new_match_if_over(mut result: ResMut<RoundResult>) {
    if result.match_winner().is_some() {
        *result = RoundResult::default();
    }
}

/// Voltar ao menu abandona a partida em andamento.
fn clear_match(mut result: ResMut<RoundResult>) {
    *result = RoundResult::default();
}

/// Registra quantos lugares esta luta usa.
///
/// Vem da mesma conta que cria os bonecos, e nao da contagem de entidades: aqui
/// elas ainda nao existem. O cliente recebe o numero pronto no pacote de fim de
/// round, entao os dois lados desenham o mesmo placar.
fn record_seats(
    mut result: ResMut<RoundResult>,
    mode: Res<GameMode>,
    online: Option<Res<crate::online::OnlineSession>>,
) {
    result.players = crate::actor::seats(*mode, online.as_deref()) as u8;
}

/// Zera o combo em andamento a cada entrada na arena, mas guarda o recorde:
/// ele e da sessao, nao do round.
fn reset_combo_meter(mut meter: ResMut<ComboMeter>) {
    meter.hits = 0;
    meter.damage = 0;
    meter.last_move = "";
    meter.idle = f32::MAX;
}

/// Soma os acertos no dummy e recompoe o alvo quando o combo esfria.
fn track_training_combo(
    time: Res<Time>,
    mut meter: ResMut<ComboMeter>,
    mut damaged: MessageReader<Damaged>,
    mut dummies: Query<&mut Health, With<TrainingDummy>>,
) {
    for hit in damaged.read() {
        if !dummies.contains(hit.target) {
            continue;
        }
        meter.hits += 1;
        meter.damage += hit.amount;
        meter.idle = 0.0;
        if !hit.move_name.is_empty() {
            meter.last_move = hit.move_name;
        }
    }

    if meter.idle >= COMBO_DROP {
        return;
    }
    meter.idle += time.delta_secs();
    if meter.idle < COMBO_DROP {
        return;
    }

    // Combo encerrado: guarda o recorde e devolve o dummy inteiro.
    meter.best_hits = meter.best_hits.max(meter.hits);
    meter.best_damage = meter.best_damage.max(meter.damage);
    meter.hits = 0;
    meter.damage = 0;
    for mut health in &mut dummies {
        health.hp = health.max;
    }
}

/// Onde ninguem perde, cair devolve o jogador ao ponto de partida.
///
/// Vale no treino e na espera do lobby: sao as duas telas em que a arena esta
/// de pe mas nao ha round para decidir. Sem isto, quem cai no vao do lobby fica
/// morto ate a luta comecar -- e a espera existe justamente para bagunçar.
fn recover_the_fallen(
    level: Res<crate::level::CurrentLevel>,
    mut players: Query<(&Player, &mut Transform, &mut Health, &mut Velocity)>,
) {
    for (player, mut transform, mut health, mut velocity) in &mut players {
        if !health.is_dead() {
            continue;
        }
        let at = level
            .0
            .spawn_points()
            .get(player.id as usize)
            .copied()
            .unwrap_or(Vec2::ZERO);
        transform.translation = at.extend(transform.translation.z);
        velocity.0 = Vec2::ZERO;
        health.hp = health.max;
    }
}

/// Marca visualmente quem esta invulneravel, piscando o boneco.
///
/// Sem esse retorno o jogador nao entende por que os golpes pararam de contar.
///
/// Ele nao desenha nada: escreve a cor em `Flash` e deixa a camada de animacao
/// desenhar. Enquanto isto escrevia direto no sprite, eram dois sistemas
/// remontando a arte inteira todo frame -- e disputando o campo com `apply_pose`
/// sem ordem definida entre eles.
fn blink_invulnerable(mut actors: Query<(&Invulnerable, &mut Flash)>) {
    for (invuln, mut flash) in &mut actors {
        let on = (invuln.0.elapsed_secs() * 20.0) as i32 % 2 == 0;
        // `set_if_neq` e o que segura o rebuild em vinte por segundo em vez de
        // sessenta: so a virada da piscada suja o componente.
        flash.set_if_neq(Flash(on.then_some(palette::BLOOD)));
    }
}

/// Apaga a piscada quando a invulnerabilidade acaba.
///
/// Sem isto o boneco ficaria congelado na cor em que o piscar parou: nada mais
/// escreve `Flash`, e `apply_pose` so redesenha o que mudou.
fn clear_flash(mut removed: RemovedComponents<Invulnerable>, mut actors: Query<&mut Flash>) {
    for entity in removed.read() {
        if let Ok(mut flash) = actors.get_mut(entity) {
            flash.set_if_neq(Flash(None));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mirando(aim: Vec2) -> Intent {
        Intent {
            aim,
            ..Intent::default()
        }
    }

    /// O soco sai na linha do cursor, e nao so para o lado em que o corpo olha.
    #[test]
    fn o_soco_vai_para_onde_o_cursor_aponta() {
        let olhando = Facing(1.0);
        for aim in [Vec2::Y, Vec2::NEG_Y, Vec2::new(-0.6, 0.8).normalize()] {
            let dir = strike_dir(MeleeKind::Chain, &mirando(aim), &olhando);
            assert!(
                (dir - aim).length() < 0.001,
                "o soco ignorou a mira {aim:?} e foi para {dir:?}"
            );
        }
    }

    /// Sem mouse -- o jogador 2, a CPU, quem so tem teclado -- nada muda.
    ///
    /// A mira vazia vale como "para onde estou olhando", entao o mesmo codigo
    /// serve os dois sem um caminho separado que ninguem testa.
    #[test]
    fn quem_nao_mira_continua_socando_para_a_frente() {
        for lado in [-1.0f32, 1.0] {
            let dir = strike_dir(MeleeKind::Chain, &Intent::default(), &Facing(lado));
            assert_eq!(dir, Vec2::new(lado, 0.0));
        }
    }

    /// Golpe de perna nao segue cursor.
    ///
    /// A rasteira varre o chao e a voadora e a propria queda: mandar as duas
    /// para o cursor daria ao jogador um golpe que se teleporta na diagonal, e
    /// tiraria de cada uma o que a define.
    #[test]
    fn rasteira_e_voadora_nao_seguem_o_cursor() {
        for kind in [MeleeKind::Sweep, MeleeKind::Dive] {
            assert!(!follows_aim(kind), "{kind:?} virou golpe de mira");
            let dir = strike_dir(kind, &mirando(Vec2::Y), &Facing(-1.0));
            assert_eq!(dir, Vec2::new(-1.0, 0.0), "{kind:?} saiu do chao");
        }
    }

    /// Socar para cima joga o alvo para cima; socar para baixo o prega no chao.
    ///
    /// O empurrao gira com o golpe, mas o levante nao: sem ele o gancho mirado
    /// na horizontal deixaria de levantar o oponente, e o combo inteiro --
    /// que se sustenta em manter o outro no ar -- iria junto.
    #[test]
    fn o_empurrao_segue_a_linha_do_golpe() {
        let gancho = unarmed_move(2);
        let empurrao =
            |dir: Vec2| dir * gancho.knockback.x + Vec2::Y * gancho.knockback.y;

        assert!(empurrao(Vec2::Y).y > gancho.knockback.y, "socar para cima nao joga para cima");
        assert!(empurrao(Vec2::NEG_Y).y < 0.0, "socar para baixo nao prega no chao");
        assert_eq!(empurrao(Vec2::X), gancho.knockback, "o soco reto mudou de empurrao");
    }

    /// Quem clica atras do proprio boneco tem que ve-lo se virar.
    #[test]
    fn bater_para_tras_vira_o_boneco() {
        use crate::weapon::turn_to_aim;

        let mut facing = Facing(1.0);
        turn_to_aim(&mut facing, &mirando(Vec2::new(-0.9, 0.4).normalize()));
        assert_eq!(facing.0, -1.0, "ele socou por cima do proprio ombro");

        // Mira quase vertical nao tem lado: mexer nela nao pode fazer o boneco
        // girar no lugar a cada quadro.
        let mut olhando = Facing(1.0);
        turn_to_aim(&mut olhando, &mirando(Vec2::new(-0.05, 0.99).normalize()));
        assert_eq!(olhando.0, 1.0);
    }

    #[test]
    fn a_partida_so_acaba_com_o_placar_cheio() {
        let mut result = RoundResult::default();
        assert_eq!(result.match_winner(), None);

        for round in 1..MATCH_WINS {
            result.score[1] = round;
            assert_eq!(
                result.match_winner(),
                None,
                "a partida acabou com {round} de {MATCH_WINS}"
            );
        }

        result.score[1] = MATCH_WINS;
        assert_eq!(result.match_winner(), Some(1));
    }

    /// Empate nao da ponto a ninguem, mas gasta um round. Sem contador
    /// separado o "ROUND N" da tela travaria no mesmo numero.
    #[test]
    fn empate_gasta_round_sem_dar_ponto() {
        let mut result = RoundResult::default();
        result.rounds += 1;
        assert_eq!(result.score, [0; MAX_PLAYERS]);
        assert_eq!(result.rounds, 1);
        assert_eq!(result.match_winner(), None);
    }

    /// Entrar na arena com a partida ja decidida tem que comecar outra --
    /// senao o vencedor entra no round seguinte ja campeao e a tela de fim
    /// repete "TAKES IT" para sempre.
    #[test]
    fn partida_decidida_recomeca_na_arena_seguinte() {
        let mut app = App::new();
        app.insert_resource(RoundResult {
            winner: Some(0),
            score: [MATCH_WINS, 1, 0, 0],
            rounds: MATCH_WINS + 1,
            players: 2,
        })
        .add_systems(Update, start_new_match_if_over);

        app.update();

        let result = app.world().resource::<RoundResult>();
        assert_eq!(result.score, [0; MAX_PLAYERS], "o placar nao zerou");
        assert_eq!(result.rounds, 0, "a contagem de rounds nao zerou");
        assert_eq!(result.match_winner(), None);
    }

    /// Partida em andamento nao pode ser zerada ao entrar na arena, ou nenhum
    /// placar sobreviveria do round 1 para o 2.
    #[test]
    fn partida_em_andamento_sobrevive_ao_round_seguinte() {
        let mut app = App::new();
        app.insert_resource(RoundResult {
            winner: Some(0),
            score: [1, 1, 0, 0],
            rounds: 2,
            players: 2,
        })
        .add_systems(Update, start_new_match_if_over);

        app.update();

        assert_eq!(app.world().resource::<RoundResult>().score, [1, 1, 0, 0]);
    }

    /// Com quatro em campo, a primeira morte so tira um da briga.
    ///
    /// Enquanto a regra foi "alguem morreu", o primeiro a cair encerrava o
    /// round e entregava a vitoria ao primeiro sobrevivente que a busca
    /// encontrasse -- os outros dois nem terminavam a luta.
    #[test]
    fn o_round_de_quatro_so_acaba_quando_sobra_um() {
        fn arena(vivos: [bool; MAX_PLAYERS]) -> App {
            let mut app = App::new();
            app.init_resource::<Time>()
                .init_resource::<RoundResult>()
                .init_resource::<NextState<GameState>>()
                .add_systems(Update, check_round_over);
            for (id, vivo) in vivos.into_iter().enumerate() {
                app.world_mut().spawn((
                    Player {
                        id: id as u8,
                        color: palette::BONE,
                    },
                    Health {
                        hp: if vivo { 100 } else { 0 },
                        max: 100,
                    },
                ));
            }
            app
        }

        let mut app = arena([true, false, true, true]);
        app.update();
        assert!(
            app.world().get_resource::<RoundEndDelay>().is_none(),
            "o round acabou com tres ainda de pe"
        );

        let mut app = arena([false, false, true, false]);
        app.update();
        assert!(
            app.world().get_resource::<RoundEndDelay>().is_some(),
            "sobrou um e o round nao acabou"
        );
        let result = app.world().resource::<RoundResult>();
        assert_eq!(result.winner, Some(2), "o ponto foi para o lugar errado");
        assert_eq!(result.score, [0, 0, 1, 0]);
        assert_eq!(
            result.players, 4,
            "o placar nao registrou os quatro lugares"
        );
    }

    /// Arena vazia nao e round decidido.
    ///
    /// No quadro em que ninguem nasceu ainda, "sobrou no maximo um" e verdade
    /// por vacuidade -- sem a guarda de contagem a luta acabaria antes de
    /// comecar.
    #[test]
    fn arena_vazia_nao_encerra_round() {
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<RoundResult>()
            .init_resource::<NextState<GameState>>()
            .add_systems(Update, check_round_over);

        app.update();

        assert!(app.world().get_resource::<RoundEndDelay>().is_none());
        assert_eq!(app.world().resource::<RoundResult>().rounds, 0);
    }

    /// A rasteira compra vantagem, nao vida. Se ela desse mais dano que o
    /// primeiro elo e ainda derrubasse, o combo em pe nunca valeria a pena.
    #[test]
    fn a_rasteira_troca_dano_por_vantagem() {
        assert!(
            SWEEP_MOVE.damage < unarmed_move(0).damage,
            "a rasteira dói mais que o jab"
        );
        assert!(
            SWEEP_STUN > HIT_STUN * 2.0,
            "a rasteira nao derruba por tempo suficiente pra compensar"
        );
    }

    /// Golpe que nao encadeia tambem tem que ser limpo.
    ///
    /// `continue_combo` so enxergava quem tinha `ComboChain`, entao a pancada,
    /// a rasteira e a voadora deixavam um `MeleeAttack` pendurado no jogador
    /// depois de acabar -- e um acerto de tiro passava a ser nomeado com o
    /// ultimo golpe corpo-a-corpo que tinha saido.
    #[test]
    fn golpe_que_nao_encadeia_e_limpo_ao_terminar() {
        for kind in [MeleeKind::Heavy, MeleeKind::Sweep, MeleeKind::Dive] {
            let mut app = App::new();
            app.add_systems(Update, continue_combo);
            let player = app
                .world_mut()
                .spawn(MeleeAttack {
                    step: 0,
                    style: WeaponStyle::Unarmed,
                    kind,
                    move_data: SWEEP_MOVE,
                    launched: true,
                })
                .id();
            app.update();
            assert!(
                app.world().get::<MeleeAttack>(player).is_none(),
                "{kind:?} deixou o MeleeAttack pendurado"
            );
        }
    }

    /// A voadora viaja com o corpo, entao a janela dela tem que ser mais longa
    /// que a dos golpes parados -- senao so acerta quem estiver exatamente no
    /// ponto de contato.
    #[test]
    fn a_voadora_tem_janela_mais_longa() {
        assert!(melee_active(MeleeKind::Dive) > melee_active(MeleeKind::Chain));
        assert!(
            DIVE_LAUNCH.y < 0.0,
            "a voadora tem que descer, nao subir: {DIVE_LAUNCH:?}"
        );
        assert!(DIVE_LAUNCH.x > 0.0, "a voadora tem que avancar");
    }

    /// O `Hop` do dummy so treina antiaereo se a altura dele cair na janela
    /// certa: alto o bastante pra rasteira passar por baixo, baixo o bastante
    /// pro gancho ainda alcancar.
    ///
    /// A janela tem menos de 10 unidades. Sem este teste, mexer em
    /// `DUMMY_HOP`, na altura das hitboxes ou no tamanho do corpo transformaria
    /// o modo num boneco que so sobe e desce.
    #[test]
    fn o_pulo_do_dummy_cai_na_janela_do_antiaereo() {
        use crate::actor::DUMMY_HOP;

        let half_body = crate::actor::body_half_height();
        let half_hit = MELEE_BOX_H * 0.5;
        // Atacante no chao em y = 0; dummy no topo do pulo.
        let corpo = (DUMMY_HOP - half_body, DUMMY_HOP + half_body);
        let alcanca = |kind: MeleeKind, step: u8| {
            let mid = melee_height(kind, step);
            mid - half_hit < corpo.1 && corpo.0 < mid + half_hit
        };

        assert!(
            !alcanca(MeleeKind::Sweep, 0),
            "a rasteira alcanca o dummy no ar; ela deveria passar por baixo"
        );
        assert!(
            alcanca(MeleeKind::Chain, 2),
            "o gancho nao alcanca o dummy no ar; nao ha antiaereo pra treinar"
        );
    }

    /// A moldura tem que ser vazada e retangular.
    ///
    /// Cheia, ela esconderia justamente o boneco que esta medindo; torta, ela
    /// mediria errado e o visualizador viraria mais uma fonte de engano.
    #[test]
    fn a_moldura_de_depuracao_e_vazada_e_retangular() {
        let arte = outline(6, 4);
        let linhas: Vec<&str> = arte.lines().collect();

        assert_eq!(linhas.len(), 4);
        assert!(
            linhas.iter().all(|linha| linha.chars().count() == 6),
            "moldura torta: {linhas:?}"
        );
        assert!(
            linhas[1].chars().skip(1).take(4).all(|c| c == ' '),
            "o miolo nao esta vazado: {:?}",
            linhas[1]
        );
        // Uma caixa degenerada nao pode virar arte vazia -- ela ainda precisa
        // aparecer para denunciar que ficou degenerada.
        assert_eq!(outline(0, 0).lines().count(), 2);

        // Mesma licao da `BOMB`: glifo fora da pagina vira `?` em silencio.
        use crate::ascii::cp437::glyph_index;
        for ch in arte.chars().filter(|c| !c.is_whitespace()) {
            assert_ne!(
                glyph_index(ch),
                glyph_index('?'),
                "U+{:04X} fora da CP437",
                ch as u32
            );
        }
    }

    /// Agachar tem que servir pra alguma coisa: ele e a resposta ao gancho.
    ///
    /// Antes disto o mixup so existia do lado de quem ataca -- a rasteira
    /// errava quem estava no ar, mas quem defendia nao tinha resposta a nada.
    /// O jab continua acertando, porque nenhuma postura pode ganhar de tudo, e
    /// a rasteira tambem, que e o castigo de quem agacha demais.
    #[test]
    fn agachar_passa_por_baixo_do_gancho() {
        let corpo = Collider::size(30.0, crate::actor::body_half_height() * 2.0);
        let em_pe = corpo.aabb(Vec2::ZERO);
        let baixo = Hurtbox::crouched(&corpo).aabb(Vec2::ZERO);

        let golpe = |kind, step| {
            Rect::from_center_half_size(
                Vec2::new(0.0, melee_height(kind, step)),
                Vec2::new(40.0, MELEE_BOX_H * 0.5),
            )
        };
        let gancho = golpe(MeleeKind::Chain, 2);

        assert!(
            (baixo.min.y - em_pe.min.y).abs() < 0.01,
            "os pes sairam do lugar ao agachar"
        );
        assert!(
            overlap(gancho, em_pe),
            "o gancho nao acerta quem esta em pe"
        );
        assert!(
            !overlap(gancho, baixo),
            "o gancho ainda acerta quem agachou"
        );
        assert!(
            overlap(golpe(MeleeKind::Chain, 0), baixo),
            "o jab deixou de acertar quem agacha"
        );
        assert!(
            overlap(golpe(MeleeKind::Sweep, 0), baixo),
            "a rasteira nao pune mais quem agacha"
        );
        // A pancada do cano desce, entao ela passa onde o punho nao passa --
        // e o que da a arma uma resposta que o corpo-a-corpo nao tem.
        assert!(
            overlap(golpe(MeleeKind::Heavy, 2), baixo),
            "a pancada tambem virou golpe alto"
        );
    }

    /// Cada tipo de golpe tem que abrir a hitbox numa altura diferente: e a
    /// unica coisa que distingue alto de baixo pra quem apanha.
    #[test]
    fn alto_e_baixo_abrem_em_alturas_diferentes() {
        let baixo = melee_height(MeleeKind::Sweep, 0);
        let alto = melee_height(MeleeKind::Chain, 2);
        assert!(
            alto - baixo > 20.0,
            "alto e baixo abrem quase na mesma altura: {alto} e {baixo}"
        );
    }
}

/// Contorno de caixa desenhado no treino.
#[derive(Component)]
struct DebugBox;

/// Ligado ou desligado pelo `H` durante o treino.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShowBoxes(pub bool);

/// Moldura vazada de `cols` x `rows`, em linha simples.
fn outline(cols: u16, rows: u16) -> String {
    let cols = cols.max(2) as usize;
    let rows = rows.max(2) as usize;
    let mut out = String::new();
    for row in 0..rows {
        for col in 0..cols {
            let borda_h = row == 0 || row == rows - 1;
            let borda_v = col == 0 || col == cols - 1;
            out.push(match (borda_h, borda_v) {
                (true, true) => '\u{253C}',
                (true, false) => '\u{2500}',
                (false, true) => '\u{2502}',
                // Vazado: a moldura nao pode esconder o boneco que ela mede.
                (false, false) => ' ',
            });
        }
        if row + 1 < rows {
            out.push('\n');
        }
    }
    out
}

/// Desenha as caixas de dano e de dano-recebido.
///
/// Existe porque quase todo erro de alcance que apareceu neste jogo foi achado
/// por aritmetica, nao por olhar a tela: dummy flutuando fora de alcance, arma
/// descolada da mao, golpe abrindo na altura errada. Com as caixas a vista,
/// esses erros viram obvios em vez de invisiveis.
///
/// Redesenha do zero a cada quadro: sao poucas caixas, e diferenciar custaria
/// mais que refazer.
fn draw_debug_boxes(
    mut commands: Commands,
    show: Res<ShowBoxes>,
    antigas: Query<Entity, With<DebugBox>>,
    hitboxes: Query<(&Transform, &Collider), With<Hitbox>>,
    corpos: Query<
        (&Transform, &Collider, Option<&Hurtbox>),
        Or<(With<Player>, With<TrainingDummy>)>,
    >,
) {
    for antiga in &antigas {
        commands.entity(antiga).despawn();
    }
    if !show.0 {
        return;
    }

    let mut desenhar = |centro: Vec2, half: Vec2, cor: Color| {
        let cols = (half.x * 2.0 / crate::ascii::CELL.x).round() as u16;
        let rows = (half.y * 2.0 / crate::ascii::CELL.y).round() as u16;
        commands.spawn((
            DebugBox,
            AsciiSprite::new(AsciiArt::solid(&outline(cols, rows), cor)),
            Layer::Fx,
            Transform::from_translation(centro.extend(0.0)),
            DespawnOnExit(GameState::Fighting),
        ));
    };

    for (transform, collider) in &hitboxes {
        desenhar(
            transform.translation.truncate(),
            collider.half,
            palette::BLOOD,
        );
    }
    for (transform, collider, hurtbox) in &corpos {
        let at = transform.translation.truncate();
        match hurtbox {
            Some(hurtbox) => desenhar(at + hurtbox.offset, hurtbox.half, palette::MOSS),
            None => desenhar(at, collider.half, palette::MOSS),
        }
    }
}

/// Soco, dano, morte e fim de round.
pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RoundResult>()
            .init_resource::<ComboMeter>()
            .init_resource::<ShowBoxes>()
            .add_systems(
                Update,
                // Antes do rebuild de glifos: o contorno nasce e e desenhado
                // no mesmo quadro, senao ele pisca -- ele e refeito do zero
                // toda vez.
                draw_debug_boxes
                    .in_set(AppSet::Render)
                    .before(crate::ascii::sprite::rebuild_glyphs)
                    .run_if(in_state(GameState::Fighting))
                    .run_if(resource_equals(GameMode::Training)),
            )
            .add_message::<Damaged>()
            .add_message::<Parried>()
            .add_systems(
                OnEnter(GameState::Fighting),
                (reset_combo_meter, start_new_match_if_over, record_seats),
            )
            .add_systems(OnEnter(GameState::Controls), clear_match)
            .add_systems(
                Update,
                (
                    tick_timers,
                    start_parry,
                    start_heavy,
                    crouch_hurtbox,
                    start_sweep,
                    start_dive,
                    start_melee,
                    queue_melee,
                    launch_melee,
                    continue_combo,
                    move_following_hitboxes,
                    resolve_hits,
                    land_dive,
                )
                    .chain()
                    .in_set(AppSet::Logic)
                    // Vale tambem no aquecimento do lobby: brigar la e o ponto
                    // da espera. O que o lobby nao tem e o round.
                    .run_if(arena_live),
            )
            // Quem decide o fim do round depende do modo: no versus alguem
            // vence, no treino o jogador so volta pro ponto de partida.
            .add_systems(
                Update,
                // Vale em qualquer modo que nao seja o treino: contra o jogo o
                // round decide igual, so muda quem move o segundo boneco.
                check_round_over
                    .after(resolve_hits)
                    .in_set(AppSet::Logic)
                    .run_if(in_state(GameState::Fighting))
                    .run_if(crate::online::can_decide_round),
            )
            .add_systems(
                Update,
                track_training_combo
                    .after(resolve_hits)
                    .in_set(AppSet::Logic)
                    .run_if(in_state(GameState::Fighting))
                    .run_if(resource_equals(GameMode::Training)),
            )
            // Nas duas telas onde ninguem perde: o treino e a espera do lobby.
            .add_systems(
                Update,
                recover_the_fallen
                    .after(resolve_hits)
                    .in_set(AppSet::Logic)
                    .run_if(in_state(GameState::Lobby).or_else(
                        in_state(GameState::Fighting).and_then(resource_equals(GameMode::Training)),
                    )),
            )
            // Antes de `apply_pose`: quem desenha o boneco e ele, e a piscada
            // tem que estar decidida quando ele roda, senao ela sai um frame
            // atrasada.
            .add_systems(
                Update,
                (blink_invulnerable, clear_flash)
                    .chain()
                    .before(crate::actor::motion::apply_pose)
                    .in_set(AppSet::Animate)
                    .run_if(arena_live),
            );
    }
}
