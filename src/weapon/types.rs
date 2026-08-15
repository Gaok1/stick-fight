/// Tempo antes do primeiro drop -- o comeco do round e mao a mao.
pub const FIRST_DROP_DELAY: f32 = 5.0;
/// Intervalo entre drops seguintes.
pub const DROP_INTERVAL: f32 = 7.0;

/// Um tiro a ser criado.
pub struct Shot {
    /// Velocidade inicial.
    pub velocity: Vec2,
    /// Arte do projetil.
    ///
    /// E uma string e nao um glifo porque nem todo projetil e um ponto: a faca
    /// precisa de lamina e ponta para dar pra ver de que lado ela entrou.
    pub glyph: &'static str,
    /// Cor do projetil.
    pub color: Color,
    /// Dano no impacto.
    pub damage: i32,
    /// Empurrao no impacto.
    pub knockback: Vec2,
    /// Segundos ate sumir sozinho.
    pub life: f32,
    /// Como ele se comporta depois de sair do cano.
    ///
    /// Nao tem valor padrao de proposito: uma arma nova tem que escolher, em
    /// vez de herdar "reto" sem perceber.
    pub kind: ShotKind,
}

/// O que o projetil faz depois de sair do cano.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShotKind {
    /// Vai reto, atravessa cenario e some no primeiro alvo.
    Straight,
    /// Vai reto, mas em vez de sumir no alvo fica cravado nele.
    ///
    /// E a mesma trajetoria do tiro reto -- o que muda e o que sobra depois. Um
    /// boneco atravessado de facas continua lutando, e e essa imagem que a arma
    /// vende.
    Sticky,
    /// Magia reta: usa a mesma colisao de um tiro, mas nao ejeta capsula nem
    /// deixa rastro de polvora.
    Arcane,
    /// Descreve um arco, para onde bater e estoura ao encostar num corpo -- ou
    /// quando o pavio acaba, o que vier primeiro.
    ///
    /// Ele nao machuca no contato: o toque so adianta o estouro, e o dano todo
    /// continua sendo o da area. Errar o arremesso ainda e errar o golpe
    /// inteiro; o que mudou e que acertar em cheio nao da mais tempo de sair de
    /// perto.
    Lobbed {
        /// Segundos de pavio.
        fuse: f32,
        /// Largura da area do estouro.
        blast: f32,
        /// Dano de quem esta dentro dela.
        damage: i32,
    },
}

/// Projetil que crava em vez de sumir.
///
/// Quem resolve dano so precisa saber que este acerto nao apaga o projetil; o
/// resto -- desmontar rastro, velocidade e colisao -- e problema deste modulo.
#[derive(Component)]
pub struct Sticky;

/// Pedido de fixacao: o projetil cravou em `host`, naquele deslocamento.
///
/// Existe para o combate poder dizer "cravou aqui" sem conhecer nada de
/// projetil, e para a fixacao acontecer num sistema so, que sabe tudo que
/// precisa sair.
#[derive(Component)]
pub struct StuckInto {
    /// Em quem cravou.
    pub host: Entity,
    /// Onde, relativo ao centro do alvo.
    pub offset: Vec2,
}

/// Pavio contando para o estouro.
#[derive(Component)]
pub struct Fuse {
    timer: Timer,
    blast: f32,
    damage: i32,
    owner: Entity,
}

/// Silhueta e ritmo do combo corpo-a-corpo de uma arma.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponStyle {
    Unarmed,
    Pistol,
    Shotgun,
    Rifle,
    /// Grimorio aberto: duas maos proximas, sem pose de coronha e cano.
    Book,
    /// Cano pesado, empunhado com as duas maos.
    Pipe,
    /// Arremessavel: sai da mao em arco.
    Bomb,
    /// Lamina curta: estocadas rapidas e cortes curtos.
    Knife,
    /// Lamina longa: cortes amplos com as duas maos.
    Katana,
    /// Dois bastoes ligados: golpes circulares e muito rapidos.
    Nunchaku,
    /// Estoque: lamina fina de ponta. Estende em vez de girar, e afunda a
    /// perna junto -- o unico estilo cujos tres elos mexem nas pernas.
    FencySword,
}

impl WeaponStyle {
    /// Quanto a arma alonga o quadro de contato de um golpe.
    ///
    /// Mora aqui e nao na animacao porque e propriedade da arma -- cano mais
    /// longo chega antes do punho. Estilo novo e mais um braco deste `match`, e
    /// nao uma tabela escondida dentro de `animate_limbs`.
    pub fn reach(self) -> f32 {
        match self {
            WeaponStyle::Unarmed | WeaponStyle::Bomb | WeaponStyle::Book => 0.0,
            WeaponStyle::Knife => 4.0,
            WeaponStyle::Katana => 15.0,
            // O maior do arsenal: numa arma de ponta o alcance *e* o golpe.
            WeaponStyle::FencySword => 18.0,
            WeaponStyle::Nunchaku => 11.0,
            WeaponStyle::Pistol => 2.0,
            WeaponStyle::Shotgun => 7.0,
            WeaponStyle::Rifle => 10.0,
            WeaponStyle::Pipe => 10.0,
        }
    }
}

/// Coice: o que o disparo faz com quem puxou o gatilho.
///
/// E o contrapeso do dano -- a 12 arranca pedaco, mas te joga pra tras e larga
/// a mira no chao por um instante.
#[derive(Debug, Clone, Copy)]
pub struct Recoil {
    /// Empurrao no atirador, contra a direcao do tiro.
    pub push: f32,
    /// Quanto o cano pula, em radianos.
    pub kick: f32,
    /// Tremor de camera somado ao trauma, em [0, 1].
    pub shake: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct MeleeMove {
    pub damage: i32,
    pub reach: f32,
    pub knockback: Vec2,
    pub duration: f32,
    pub contact: f32,
}

/// Contrato de uma arma.
pub trait Weapon: Send + Sync + 'static {
    /// Nome mostrado no HUD.
    fn name(&self) -> &'static str;
    /// Glifo que aparece na mao do boneco.
    fn held_art(&self) -> &'static str;
    /// Arte quando esta caida no chao.
    fn ground_art(&self) -> &'static str;
    /// Segundos entre disparos.
    fn cooldown(&self) -> f32;
    /// Municao ao pegar.
    fn ammo(&self) -> u32;
    /// Projeteis produzidos por um disparo na direcao `dir`.
    fn shots(&self, dir: Vec2) -> Vec<Shot>;
    /// Coice do disparo.
    fn recoil(&self) -> Recoil;
    /// Tres golpes M1, proprios para a massa e o alcance da arma.
    fn melee(&self, step: u8) -> MeleeMove;
    fn style(&self) -> WeaponStyle;

    /// Golpe pesado do M2, para armas que nao atiram.
    ///
    /// `None` -- o padrao -- quer dizer "o M2 dispara", que e o que toda arma
    /// de fogo faz. Quem devolve `Some` troca o disparo por um golpe unico,
    /// lento e caro, sem que `fire` precise saber que essa arma existe.
    fn heavy(&self) -> Option<MeleeMove> {
        None
    }

    /// Arma de contato: sem municao, sem disparo, sem contador no HUD.
    ///
    /// Sai de `heavy` em vez de ser declarado a mao para que as duas coisas
    /// nao possam discordar.
    fn is_melee(&self) -> bool {
        self.heavy().is_some()
    }
}

/// Identidade visual, separada do estilo de combate. `KNIFE` e `KNIVES`, por
/// exemplo, compartilham coreografia mas nao silhueta nem pecas secundarias.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WeaponLook {
    Pistol,
    Shotgun,
    Rifle,
    Pipe,
    Nunchaku,
    Katana,
    Knife,
    Bomb,
    Knives,
    Book,
    FencySword,
}

fn weapon_look(weapon: &dyn Weapon) -> WeaponLook {
    match weapon.name() {
        "PISTOL" => WeaponLook::Pistol,
        "SHOTGUN" => WeaponLook::Shotgun,
        "RIFLE" => WeaponLook::Rifle,
        "PIPE" => WeaponLook::Pipe,
        "NUNCHAKU" => WeaponLook::Nunchaku,
        "KATANA" => WeaponLook::Katana,
        "KNIFE" => WeaponLook::Knife,
        "BOMB" => WeaponLook::Bomb,
        "KNIVES" => WeaponLook::Knives,
        "MAGIC BOOK" => WeaponLook::Book,
        "FENCY SWORD" => WeaponLook::FencySword,
        name => panic!("arma {name} sem linguagem visual"),
    }
}

/// Escala na mao. No chao a arma continua grande o bastante para ser lida;
/// equipada, ela precisa caber na silhueta de tres colunas do lutador.
///
/// Toda arte empunhada tem tres linhas -- 48 unidades de altura -- entao o teto
/// aqui e meio: acima disso a arma passa das 24 unidades de largura do boneco e
/// vira uma laje atravessada nele. Quem cobra e
/// `toda_arma_cabe_na_silhueta_do_lutador`, ja com as pecas soltas somadas.
fn held_scale(look: WeaponLook) -> f32 {
    match look {
        WeaponLook::Pistol => 0.46,
        WeaponLook::Shotgun => 0.48,
        WeaponLook::Rifle => 0.44,
        WeaponLook::Pipe => 0.48,
        WeaponLook::Nunchaku => 0.46,
        WeaponLook::Katana => 0.46,
        WeaponLook::Knife => 0.46,
        WeaponLook::Bomb => 0.46,
        WeaponLook::Knives => 0.46,
        WeaponLook::Book => 0.40,
        WeaponLook::FencySword => 0.46,
    }
}

/// Onde o punho pega a arma, em coordenada da arte: origem no centro do sprite,
/// `x` para a ponta e `y` para cima.
///
/// `AsciiSprite` gira pelo centro, entao uma arma cujo cabo nao esteja marcado
/// aqui gira em torno do meio da propria lamina. E um `Vec2`, e nao a distancia
/// escalar que era antes, porque a pistola nao tem o cabo na linha do cano: com
/// um numero so, o punho dela cai dentro do ferrolho e a coronha atravessa o
/// pulso.
fn grip_local(look: WeaponLook) -> Vec2 {
    match look {
        WeaponLook::Pistol => Vec2::new(-28.0, -12.0),
        WeaponLook::Shotgun => Vec2::new(-20.0, 0.0),
        WeaponLook::Rifle => Vec2::new(-28.0, 0.0),
        WeaponLook::Pipe => Vec2::new(-36.0, 0.0),
        WeaponLook::Nunchaku => Vec2::ZERO,
        // Lamina e cabo dividem a linha da meia celula de cima: a mao pega no
        // meio do `ito`, e a arma sai reta do punho em vez de dobrada nele.
        WeaponLook::Katana => Vec2::new(-40.0, 8.0),
        WeaponLook::Knife => Vec2::new(-24.0, 8.0),
        WeaponLook::Bomb => Vec2::new(0.0, -4.0),
        WeaponLook::Knives => Vec2::new(-32.0, 0.0),
        WeaponLook::Book => Vec2::ZERO,
        // As pecas ja saem posicionadas em relacao ao punho, como no livro.
        WeaponLook::FencySword => Vec2::ZERO,
    }
}

/// A ponta que cospe: boca do cano, ponta da lamina, argola de onde sai a
/// corrente.
///
/// Coordenada da arte, como [`grip_local`], e nao uma distancia escrita a
/// parte. Enquanto foi constante, redesenhar o cano deixava o fogacho saindo do
/// ar ao lado da arma e nada no jogo reclamava -- e o mesmo erro que o dragao
/// de `backdrop.rs` cometeu com a boca dele. Quem trava e
/// `a_boca_do_cano_cai_sobre_o_desenho`, nos dois lados para os quais a arma
/// pode olhar.
fn muzzle_local(look: WeaponLook) -> Vec2 {
    match look {
        WeaponLook::Pistol => Vec2::new(28.0, 0.0),
        WeaponLook::Shotgun => Vec2::new(44.0, 4.0),
        WeaponLook::Rifle => Vec2::new(52.0, 4.0),
        WeaponLook::Pipe => Vec2::new(44.0, 0.0),
        WeaponLook::Nunchaku => nunchaku_chain_anchor(),
        WeaponLook::Katana => Vec2::new(56.0, 16.0),
        WeaponLook::Knife => Vec2::new(28.0, 16.0),
        WeaponLook::Bomb => Vec2::new(16.0, 0.0),
        // A do meio e a que sai da mao: e ela que corre reta.
        WeaponLook::Knives => Vec2::new(36.0, 0.0),
        WeaponLook::Book => book_muzzle(),
        WeaponLook::FencySword => sword_tip(),
    }
}

/// Leva um ponto da arte para o mundo, dada a mao, a linha do cano e o lado.
///
/// Espelhar nao e girar meia volta. Quando o boneco olha para a esquerda o
/// sprite e refletido, e refletir inverte o perpendicular junto -- quem trata
/// os dois casos como uma rotacao de 180 graus ganha uma pistola que ejeta a
/// capsula para dentro do proprio corpo em metade dos disparos. Foi isto que o
/// dragao ensinou, e e por isso que existe uma funcao so: desenho e disparo
/// resolvem o espelho aqui, e nao cada um do seu jeito.
fn on_weapon(look: WeaponLook, local: Vec2, hand: Vec2, along: Vec2, flipped: bool) -> Vec2 {
    let across = Vec2::new(-along.y, along.x) * if flipped { -1.0 } else { 1.0 };
    let offset = local - grip_local(look);
    hand + (along * offset.x + across * offset.y) * held_scale(look)
}

/// Metal escuro, aresta clara, guarda dourada e energia quente vivem no proprio
/// glifo. Assim a silhueta e a pintura nunca saem de sincronia.
///
/// Os dois meio-blocos sao a regra que segura o arsenal inteiro: `▄` e a metade
/// escura e `▀` a metade clara. Uma lamina desenhada com um por cima do outro
/// ganha costas escuras e gume claro sem mascara nenhuma, e um cano ganha o
/// brilho de tubo pelo mesmo par -- e por isso que a katana e a escopeta
/// parecem feitas do mesmo aco.
const WEAPON_MATERIAL: [(char, Color); 42] = [
    ('\u{2588}', palette::IRON),
    ('\u{2593}', palette::IRON),
    ('\u{2592}', palette::ASH),
    ('\u{2591}', palette::COAL),
    ('\u{2584}', palette::IRON),
    ('\u{2580}', palette::BONE),
    ('\u{2590}', palette::IRON),
    ('\u{258c}', palette::IRON),
    ('\u{25cb}', palette::GOLD),
    ('\u{255e}', palette::ASH),
    ('\u{2561}', palette::ASH),
    ('\u{2261}', palette::ASH),
    ('~', palette::EMBER),
    ('\u{2550}', palette::BONE),
    ('\u{2500}', palette::BONE),
    ('\u{256c}', palette::GOLD),
    ('\u{256a}', palette::GOLD),
    ('\u{256b}', palette::GOLD),
    ('\u{2566}', palette::GOLD),
    ('\u{2569}', palette::GOLD),
    ('\u{2560}', palette::GOLD),
    ('\u{2563}', palette::GOLD),
    ('\u{25ba}', palette::BONE),
    ('\u{25ba}', palette::BONE),
    ('\u{2666}', palette::GOLD),
    ('\u{25d8}', palette::EMBER),
    ('o', palette::ASH),
    ('O', palette::IRON),
    ('*', palette::EMBER),
    ('+', palette::GOLD),
    ('[', palette::IRON),
    (']', palette::IRON),
    ('#', palette::ASH),
    ('\u{03a6}', palette::P3),
    ('\u{03b4}', palette::P3),
    ('\u{221e}', palette::P3),
    ('}', palette::GOLD),
    ('\u{2554}', palette::GOLD),
    ('\u{2557}', palette::GOLD),
    ('\u{255a}', palette::GOLD),
    ('\u{255d}', palette::GOLD),
    ('\u{2551}', palette::GOLD),
];

fn weapon_art(art: &str) -> AsciiArt {
    AsciiArt::tinted(art, &WEAPON_MATERIAL, palette::ASH)
}

