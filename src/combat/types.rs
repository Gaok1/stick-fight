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
    /// Visiveis no crate porque o preview de arma monta um golpe a mao para
    /// carimbar os tres quadros dele. Quem os escreve fora daqui e so um
    /// preview: os sistemas de combate continuam construindo o componente por
    /// `start_melee`.
    pub(crate) move_data: MeleeMove,
    pub(crate) launched: bool,
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

