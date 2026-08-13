//! Armas: contrato, arsenal, drops e projeteis.
//!
//! Uma arma e um [`Weapon`]: ela so descreve como aparece no chao e que tiros
//! produz. Tudo o mais -- pegar, mirar, cadencia, municao, dano -- e generico,
//! entao arma nova e um `impl Weapon` e uma linha no arsenal.

use bevy::prelude::*;

use crate::actor::input::CursorWorld;
use crate::actor::{Attacking, Facing, Intent, Player, Pose};
use crate::ascii::{AsciiArt, AsciiSprite, Layer, palette};
use crate::combat::{Hitbox, Lifetime, MeleeAttack, Parrying};
use crate::level::{ARENA_HALF_W, CurrentLevel};
use crate::physics::{Collider, Falls, Ghost, Grounded, Solid, Velocity, overlap};
use crate::state::{AppSet, GameState};

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
}

impl WeaponStyle {
    /// Quanto a arma alonga o quadro de contato de um golpe.
    ///
    /// Mora aqui e nao na animacao porque e propriedade da arma -- cano mais
    /// longo chega antes do punho. Estilo novo e mais um braco deste `match`, e
    /// nao uma tabela escondida dentro de `animate_limbs`.
    pub fn reach(self) -> f32 {
        match self {
            WeaponStyle::Unarmed | WeaponStyle::Bomb => 0.0,
            WeaponStyle::Knife => 4.0,
            WeaponStyle::Katana => 15.0,
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

/// Pistola: um tiro reto, preciso.
pub struct Pistol;

impl Weapon for Pistol {
    fn name(&self) -> &'static str {
        "PISTOL"
    }
    fn held_art(&self) -> &'static str {
        "\u{2500}\u{2510}"
    }
    fn ground_art(&self) -> &'static str {
        "\u{2500}\u{2500}\u{2510}\n  \u{2514}"
    }
    fn cooldown(&self) -> f32 {
        0.30
    }
    fn ammo(&self) -> u32 {
        7
    }
    fn shots(&self, dir: Vec2) -> Vec<Shot> {
        vec![Shot {
            velocity: dir * 900.0,
            glyph: "\u{2500}",
            color: palette::GOLD,
            damage: 14,
            knockback: dir * 240.0 + Vec2::new(0.0, 120.0),
            life: 1.4,
            kind: ShotKind::Straight,
        }]
    }
    fn recoil(&self) -> Recoil {
        Recoil {
            push: 135.0,
            kick: 0.34,
            shake: 0.10,
        }
    }
    fn melee(&self, step: u8) -> MeleeMove {
        match step % 3 {
            0 => MeleeMove {
                damage: 8,
                reach: 31.0,
                knockback: Vec2::new(220.0, 90.0),
                duration: 0.20,
                contact: 0.28,
            },
            1 => MeleeMove {
                damage: 9,
                reach: 34.0,
                knockback: Vec2::new(245.0, 125.0),
                duration: 0.21,
                contact: 0.25,
            },
            _ => MeleeMove {
                damage: 13,
                reach: 38.0,
                knockback: Vec2::new(330.0, 210.0),
                duration: 0.29,
                contact: 0.38,
            },
        }
    }
    fn style(&self) -> WeaponStyle {
        WeaponStyle::Pistol
    }
}

/// Escopeta: leque de chumbos, forte de perto.
pub struct Shotgun;

impl Weapon for Shotgun {
    fn name(&self) -> &'static str {
        "SHOTGUN"
    }
    fn held_art(&self) -> &'static str {
        "\u{2550}\u{2550}\u{2550}\u{2550}"
    }
    fn ground_art(&self) -> &'static str {
        "\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\n \u{2514}"
    }
    fn cooldown(&self) -> f32 {
        0.38
    }
    fn ammo(&self) -> u32 {
        4
    }
    fn shots(&self, dir: Vec2) -> Vec<Shot> {
        // Leque em torno da mira: girar a direcao mantem o espalhamento igual
        // atirando pra cima, pra baixo ou na diagonal.
        [-0.16f32, -0.05, 0.05, 0.16]
            .iter()
            .map(|spread| Shot {
                velocity: Vec2::from_angle(*spread).rotate(dir) * 700.0,
                glyph: "\u{00B7}",
                color: palette::GOLD,
                damage: 10,
                knockback: dir * 300.0 + Vec2::new(0.0, 150.0),
                life: 0.5,
                kind: ShotKind::Straight,
            })
            .collect()
    }
    fn recoil(&self) -> Recoil {
        // O coice da 12: te arranca do lugar e joga o cano pro alto.
        Recoil {
            push: 720.0,
            kick: 1.20,
            shake: 0.68,
        }
    }
    fn melee(&self, step: u8) -> MeleeMove {
        match step % 3 {
            0 => MeleeMove {
                damage: 11,
                reach: 39.0,
                knockback: Vec2::new(275.0, 105.0),
                duration: 0.27,
                contact: 0.32,
            },
            1 => MeleeMove {
                damage: 12,
                reach: 42.0,
                knockback: Vec2::new(300.0, 145.0),
                duration: 0.29,
                contact: 0.34,
            },
            _ => MeleeMove {
                damage: 18,
                reach: 45.0,
                knockback: Vec2::new(410.0, 250.0),
                duration: 0.38,
                contact: 0.44,
            },
        }
    }
    fn style(&self) -> WeaponStyle {
        WeaponStyle::Shotgun
    }
}

/// Rifle: cadencia alta, dano baixo por tiro.
pub struct Rifle;

impl Weapon for Rifle {
    fn name(&self) -> &'static str {
        "RIFLE"
    }
    fn held_art(&self) -> &'static str {
        "\u{2500}\u{2500}\u{2534}\u{2500}\u{2500}"
    }
    fn ground_art(&self) -> &'static str {
        "\u{2500}\u{2500}\u{2534}\u{2500}\u{2500}\n  \u{2514}"
    }
    fn cooldown(&self) -> f32 {
        0.11
    }
    fn ammo(&self) -> u32 {
        18
    }
    fn shots(&self, dir: Vec2) -> Vec<Shot> {
        vec![Shot {
            velocity: dir * 1150.0,
            glyph: "\u{2500}",
            color: palette::BONE,
            damage: 6,
            knockback: dir * 120.0 + Vec2::new(0.0, 60.0),
            life: 1.2,
            kind: ShotKind::Straight,
        }]
    }
    fn recoil(&self) -> Recoil {
        Recoil {
            push: 60.0,
            kick: 0.16,
            shake: 0.045,
        }
    }
    fn melee(&self, step: u8) -> MeleeMove {
        match step % 3 {
            0 => MeleeMove {
                damage: 10,
                reach: 42.0,
                knockback: Vec2::new(250.0, 100.0),
                duration: 0.23,
                contact: 0.30,
            },
            1 => MeleeMove {
                damage: 11,
                reach: 46.0,
                knockback: Vec2::new(285.0, 150.0),
                duration: 0.25,
                contact: 0.31,
            },
            _ => MeleeMove {
                damage: 16,
                reach: 50.0,
                knockback: Vec2::new(380.0, 225.0),
                duration: 0.34,
                contact: 0.40,
            },
        }
    }
    fn style(&self) -> WeaponStyle {
        WeaponStyle::Rifle
    }
}

/// Bomba de cano: a primeira arma que nao mira em linha reta.
///
/// Todas as outras resolvem no instante do disparo -- apontou, acertou ou
/// errou. Esta troca precisao por area e obriga a prever para onde o outro vai
/// estar daqui a um segundo.
///
/// Acertar o corpo detona na hora, entao ela tem as duas leituras: quem mira em
/// cheio ganha um estouro imediato e imperdivel, quem erra deixa uma bomba
/// quicando pelo chao que ainda pode pegar alguem de surpresa.
pub struct PipeBomb;

/// Segundos de pavio.
///
/// Deixou de ser o relogio do golpe quando a granada passou a estourar no
/// toque: hoje ele e o teto, o tempo que ela fica quicando sem achar ninguem.
const BOMB_FUSE: f32 = 1.15;

impl Weapon for PipeBomb {
    fn name(&self) -> &'static str {
        "BOMB"
    }
    fn held_art(&self) -> &'static str {
        "\u{25D8}"
    }
    fn ground_art(&self) -> &'static str {
        "\u{25D8}\u{2510}"
    }
    fn cooldown(&self) -> f32 {
        1.20
    }
    fn ammo(&self) -> u32 {
        2
    }
    fn shots(&self, dir: Vec2) -> Vec<Shot> {
        vec![Shot {
            // Sai na direcao da mira com um empurrao pra cima somado: e o que
            // transforma a mira reta num arco sem pedir outro controle.
            velocity: dir * 520.0 + Vec2::new(0.0, 190.0),
            glyph: "\u{2022}",
            color: palette::BLOOD,
            damage: 0,
            knockback: Vec2::ZERO,
            life: BOMB_FUSE,
            kind: ShotKind::Lobbed {
                fuse: BOMB_FUSE,
                blast: 150.0,
                damage: 30,
            },
        }]
    }
    fn recoil(&self) -> Recoil {
        // Arremesso nao tem coice; o tranco todo e o do braco.
        Recoil {
            push: 20.0,
            kick: 0.0,
            shake: 0.03,
        }
    }
    fn melee(&self, step: u8) -> MeleeMove {
        // Bater com a bomba na mao e ruim de proposito: ela e pra ser jogada.
        match step % 3 {
            0 => MeleeMove {
                damage: 6,
                reach: 26.0,
                knockback: Vec2::new(200.0, 90.0),
                duration: 0.18,
                contact: 0.27,
            },
            1 => MeleeMove {
                damage: 7,
                reach: 28.0,
                knockback: Vec2::new(230.0, 130.0),
                duration: 0.20,
                contact: 0.25,
            },
            _ => MeleeMove {
                damage: 11,
                reach: 30.0,
                knockback: Vec2::new(320.0, 210.0),
                duration: 0.28,
                contact: 0.39,
            },
        }
    }
    fn style(&self) -> WeaponStyle {
        WeaponStyle::Bomb
    }
}

/// Facas de arremesso: a arma que deixa marca depois do acerto.
///
/// Toda outra arma resolve e some -- a bala apaga, o estouro passa. Esta fica:
/// quem levou tres facas anda o resto do round com tres facas cravadas. E a
/// unica coisa no jogo que conta o que ja aconteceu sem barra de vida nenhuma,
/// e e por isso que ela existe.
pub struct Knives;

impl Weapon for Knives {
    fn name(&self) -> &'static str {
        "KNIVES"
    }
    fn held_art(&self) -> &'static str {
        "\u{2500}\u{25BA}"
    }
    fn ground_art(&self) -> &'static str {
        "\u{2500}\u{25BA}\n\u{2500}\u{25BA}"
    }
    fn cooldown(&self) -> f32 {
        0.26
    }
    fn ammo(&self) -> u32 {
        6
    }
    fn shots(&self, dir: Vec2) -> Vec<Shot> {
        vec![Shot {
            // Mais lenta que bala e mais rapida que bomba: da pra desviar de
            // uma faca, e e isso que faz gastar as seis exigir mira.
            velocity: dir * 780.0,
            glyph: "\u{2500}\u{25BA}",
            color: palette::BONE,
            damage: 12,
            knockback: dir * 170.0 + Vec2::new(0.0, 95.0),
            life: 1.6,
            kind: ShotKind::Sticky,
        }]
    }
    fn recoil(&self) -> Recoil {
        // Arremesso de braco: quase nao empurra quem joga.
        Recoil {
            push: 35.0,
            kick: 0.12,
            shake: 0.06,
        }
    }
    /// Rapido e curto: a faca na mao corta mais que o punho, mas quem quer
    /// dano de verdade tem que jogar ela.
    fn melee(&self, step: u8) -> MeleeMove {
        match step % 3 {
            0 => MeleeMove {
                damage: 9,
                reach: 29.0,
                knockback: Vec2::new(210.0, 85.0),
                duration: 0.16,
                contact: 0.26,
            },
            1 => MeleeMove {
                damage: 10,
                reach: 31.0,
                knockback: Vec2::new(235.0, 120.0),
                duration: 0.17,
                contact: 0.24,
            },
            _ => MeleeMove {
                damage: 16,
                reach: 34.0,
                knockback: Vec2::new(330.0, 200.0),
                duration: 0.26,
                contact: 0.36,
            },
        }
    }
    fn style(&self) -> WeaponStyle {
        WeaponStyle::Knife
    }
}

/// Faca de combate: alcance minimo, mas o combo mais rapido do arsenal.
pub struct Knife;

impl Weapon for Knife {
    fn name(&self) -> &'static str {
        "KNIFE"
    }
    fn held_art(&self) -> &'static str {
        "o\u{2500}>"
    }
    fn ground_art(&self) -> &'static str {
        "o\u{2500}\u{2500}>"
    }
    fn cooldown(&self) -> f32 {
        0.48
    }
    fn ammo(&self) -> u32 {
        0
    }
    fn shots(&self, _dir: Vec2) -> Vec<Shot> {
        Vec::new()
    }
    fn recoil(&self) -> Recoil {
        Recoil {
            push: 0.0,
            kick: 0.0,
            shake: 0.0,
        }
    }
    fn melee(&self, step: u8) -> MeleeMove {
        match step % 3 {
            0 => MeleeMove {
                damage: 10,
                reach: 30.0,
                knockback: Vec2::new(205.0, 70.0),
                duration: 0.13,
                contact: 0.24,
            },
            1 => MeleeMove {
                damage: 11,
                reach: 33.0,
                knockback: Vec2::new(230.0, 105.0),
                duration: 0.14,
                contact: 0.22,
            },
            _ => MeleeMove {
                damage: 17,
                reach: 36.0,
                knockback: Vec2::new(325.0, 190.0),
                duration: 0.21,
                contact: 0.34,
            },
        }
    }
    fn heavy(&self) -> Option<MeleeMove> {
        Some(MeleeMove {
            damage: 23,
            reach: 43.0,
            knockback: Vec2::new(385.0, 120.0),
            duration: 0.32,
            contact: 0.42,
        })
    }
    fn style(&self) -> WeaponStyle {
        WeaponStyle::Knife
    }
}

/// Katana: cortes longos de duas maos, fortes e deliberados.
pub struct Katana;

impl Weapon for Katana {
    fn name(&self) -> &'static str {
        "KATANA"
    }
    fn held_art(&self) -> &'static str {
        "-|---->"
    }
    fn ground_art(&self) -> &'static str {
        "o\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}>"
    }
    fn cooldown(&self) -> f32 {
        0.92
    }
    fn ammo(&self) -> u32 {
        0
    }
    fn shots(&self, _dir: Vec2) -> Vec<Shot> {
        Vec::new()
    }
    fn recoil(&self) -> Recoil {
        Recoil {
            push: 0.0,
            kick: 0.0,
            shake: 0.0,
        }
    }
    fn melee(&self, step: u8) -> MeleeMove {
        match step % 3 {
            0 => MeleeMove {
                damage: 14,
                reach: 49.0,
                knockback: Vec2::new(285.0, 100.0),
                duration: 0.25,
                contact: 0.32,
            },
            1 => MeleeMove {
                damage: 16,
                reach: 53.0,
                knockback: Vec2::new(320.0, 145.0),
                duration: 0.28,
                contact: 0.34,
            },
            _ => MeleeMove {
                damage: 24,
                reach: 58.0,
                knockback: Vec2::new(445.0, 245.0),
                duration: 0.39,
                contact: 0.45,
            },
        }
    }
    fn heavy(&self) -> Option<MeleeMove> {
        Some(MeleeMove {
            damage: 32,
            reach: 66.0,
            knockback: Vec2::new(520.0, 80.0),
            duration: 0.58,
            contact: 0.54,
        })
    }
    fn style(&self) -> WeaponStyle {
        WeaponStyle::Katana
    }
}

/// Nunchaku: alcance de corrente com cadencia alta e pouco empurrao.
pub struct Nunchaku;

impl Weapon for Nunchaku {
    fn name(&self) -> &'static str {
        "NUNCHAKU"
    }
    fn held_art(&self) -> &'static str {
        "o~o"
    }
    fn ground_art(&self) -> &'static str {
        "o~~\n  o"
    }
    fn cooldown(&self) -> f32 {
        0.56
    }
    fn ammo(&self) -> u32 {
        0
    }
    fn shots(&self, _dir: Vec2) -> Vec<Shot> {
        Vec::new()
    }
    fn recoil(&self) -> Recoil {
        Recoil {
            push: 0.0,
            kick: 0.0,
            shake: 0.0,
        }
    }
    fn melee(&self, step: u8) -> MeleeMove {
        match step % 3 {
            0 => MeleeMove {
                damage: 7,
                reach: 42.0,
                knockback: Vec2::new(210.0, 75.0),
                duration: 0.14,
                contact: 0.22,
            },
            1 => MeleeMove {
                damage: 8,
                reach: 45.0,
                knockback: Vec2::new(225.0, 105.0),
                duration: 0.15,
                contact: 0.20,
            },
            _ => MeleeMove {
                damage: 15,
                reach: 50.0,
                knockback: Vec2::new(350.0, 205.0),
                duration: 0.23,
                contact: 0.34,
            },
        }
    }
    fn heavy(&self) -> Option<MeleeMove> {
        Some(MeleeMove {
            damage: 20,
            reach: 55.0,
            knockback: Vec2::new(390.0, 155.0),
            duration: 0.36,
            contact: 0.46,
        })
    }
    fn style(&self) -> WeaponStyle {
        WeaponStyle::Nunchaku
    }
}

/// Cano de ferro: a primeira arma que nao atira.
///
/// Ela existe para dar a quem pega uma escolha diferente de "aponte e clique":
/// alcance e dano de arma com o risco de ter que chegar perto. Nao gasta nada,
/// entao nunca vira peso morto -- so sai da mao arremessada.
pub struct Pipe;

impl Weapon for Pipe {
    fn name(&self) -> &'static str {
        "PIPE"
    }
    fn held_art(&self) -> &'static str {
        "o\u{2550}\u{2550}\u{2550}\u{2564}"
    }
    fn ground_art(&self) -> &'static str {
        "o\u{2550}\u{2550}\u{2550}\u{2550}\u{2564}"
    }
    fn cooldown(&self) -> f32 {
        // O M2 e o golpe pesado; este e o intervalo entre dois deles.
        0.85
    }
    fn ammo(&self) -> u32 {
        0
    }
    fn shots(&self, _dir: Vec2) -> Vec<Shot> {
        Vec::new()
    }
    fn recoil(&self) -> Recoil {
        Recoil {
            push: 0.0,
            kick: 0.0,
            shake: 0.0,
        }
    }
    /// Combo mais lento e mais longo que o punho, e o finalizador arranca
    /// pedaco: e o contrapeso de ter que estar no alcance de mao.
    fn melee(&self, step: u8) -> MeleeMove {
        match step % 3 {
            0 => MeleeMove {
                damage: 14,
                reach: 43.0,
                knockback: Vec2::new(295.0, 100.0),
                duration: 0.24,
                contact: 0.32,
            },
            1 => MeleeMove {
                damage: 15,
                reach: 47.0,
                knockback: Vec2::new(330.0, 145.0),
                duration: 0.28,
                contact: 0.31,
            },
            _ => MeleeMove {
                damage: 23,
                reach: 52.0,
                knockback: Vec2::new(455.0, 260.0),
                duration: 0.38,
                contact: 0.43,
            },
        }
    }
    /// Pancada de cima pra baixo: o dano mais alto do jogo, com a preparacao
    /// mais longa. Quem erra fica devendo meio segundo.
    fn heavy(&self) -> Option<MeleeMove> {
        Some(MeleeMove {
            damage: 31,
            reach: 55.0,
            knockback: Vec2::new(390.0, -140.0),
            duration: 0.56,
            contact: 0.56,
        })
    }
    fn style(&self) -> WeaponStyle {
        WeaponStyle::Pipe
    }
}

/// Tudo que pode cair na arena.
///
/// Publico porque os testes de arte varrem o arsenal inteiro: arma nova entra
/// nas conferencias sozinha, em vez de escapar por esquecimento.
pub const ARSENAL: &[fn() -> Box<dyn Weapon>] = &[
    || Box::new(Pistol),
    || Box::new(Shotgun),
    || Box::new(Rifle),
    || Box::new(Pipe),
    || Box::new(Nunchaku),
    || Box::new(Katana),
    || Box::new(Knife),
    || Box::new(PipeBomb),
    || Box::new(Knives),
];

/// Constroi a arma de indice `kind`, girando a lista se ele passar do fim.
///
/// O indice, e nao o ponteiro de funcao, e o que identifica uma arma na rede:
/// um `fn()` nao atravessa um pacote, e dois processos nem sequer o teriam no
/// mesmo endereco.
pub fn weapon_at(kind: u8) -> Box<dyn Weapon> {
    ARSENAL[kind as usize % ARSENAL.len()]()
}

/// Sorteia uma arma do arsenal.
pub fn random_kind() -> u8 {
    fastrand::usize(..ARSENAL.len()) as u8
}

/// Arma na mao de um jogador.
#[derive(Component)]
pub struct Held {
    /// A arma.
    pub weapon: Box<dyn Weapon>,
    /// Municao restante.
    pub ammo: u32,
    /// Cadencia.
    pub cooldown: Timer,
    /// Indice no arsenal: e o que volta ao chao sem perder a identidade da arma.
    pub kind: u8,
}

/// Arma caida, esperando ser pega.
#[derive(Component)]
pub struct GroundWeapon {
    /// Indice no arsenal.
    pub kind: u8,
    /// Municao que ela ainda carrega.
    pub ammo: u32,
}

/// Identidade de uma arma na rede.
///
/// O dono numera cada arma que larga, e todo mundo passa a falar dela por esse
/// numero. Sem isso o cliente nao tem como saber se a espingarda que chegou no
/// pacote e a que ele ja desenhou ou uma segunda -- e a escolha errada ou
/// duplica a arma ou some com ela.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct NetWeapon(pub u16);

/// Proximo numero de arma a distribuir. So o dono escreve nele.
#[derive(Resource, Default)]
pub struct NextWeaponId(pub u16);

/// Poe uma arma no chao (ou no ar, se ela foi arremessada).
///
/// Um lugar so monta a entidade, e o dono e o cliente chamam o mesmo: enquanto
/// cada lado tinha a sua copia da montagem, a arma do cliente nascia sem
/// colisor e atravessava o chao enquanto a do dono ficava pousada.
pub fn spawn_ground_weapon(
    commands: &mut Commands,
    net: Option<u16>,
    kind: u8,
    ammo: u32,
    at: Vec2,
    velocity: Vec2,
) -> Entity {
    let weapon = weapon_at(kind);
    let mut entity = commands.spawn((
        GroundWeapon { kind, ammo },
        PickupPulse(fastrand::f32() * 6.0),
        AsciiSprite::footed(AsciiArt::solid(weapon.ground_art(), palette::GOLD)),
        Layer::Pickup,
        Transform::from_translation(at.extend(0.0)),
        Velocity(velocity),
        Grounded::default(),
        Falls,
        Collider::size(42.0, 24.0),
        DespawnOnExit(GameState::Fighting),
    ));
    if let Some(net) = net {
        entity.insert(NetWeapon(net));
    }
    entity.id()
}

/// Poe a arma `kind` na mao de `player`, com o icone que o boneco carrega.
///
/// Publica porque a replicacao precisa armar um boneco sem passar por encostar
/// na arma: quem manda no que cada um segura e o dono da sala, e o cliente so
/// obedece.
pub fn equip(commands: &mut Commands, player: Entity, kind: u8, ammo: u32, facing: f32) {
    let held = held_weapon(kind, ammo);
    let held_art = held.weapon.held_art();

    commands.entity(player).insert(held);
    commands.spawn((
        WeaponIcon,
        AsciiSprite::new(AsciiArt::solid(held_art, palette::GOLD)).flipped(facing < 0.0),
        Layer::Actor,
        Transform::from_translation(Vec3::new(facing * 18.0, 4.0, 0.1)),
        ChildOf(player),
    ));
}

/// A arma `kind` pronta para ir para a mao, ja carregada e fora de recarga.
pub fn held_weapon(kind: u8, ammo: u32) -> Held {
    let weapon = weapon_at(kind);
    let mut cooldown = Timer::from_seconds(weapon.cooldown(), TimerMode::Once);
    // Comeca pronta pra atirar.
    cooldown.tick(cooldown.duration());
    Held {
        weapon,
        ammo,
        cooldown,
        kind,
    }
}

/// Poe uma arma replicada a girar, como a que saiu de uma mao de verdade.
pub fn mark_thrown(commands: &mut Commands, weapon: Entity) {
    commands
        .entity(weapon)
        .insert(ThrownWeapon(Timer::from_seconds(0.55, TimerMode::Once)));
}

/// Arma arremessada; enquanto o timer corre, gira e causa um unico dano.
#[derive(Component)]
pub struct ThrownWeapon(pub Timer);

/// Glifo da arma desenhado ao lado do boneco.
///
/// Vive numa entidade filha propria em vez de ser carimbado na arte da pose:
/// assim nenhum sistema de animacao disputa a escrita do mesmo sprite.
#[derive(Component)]
pub struct WeaponIcon;

/// Coice em andamento: enquanto dura, a arma recua e o cano fica pro alto.
#[derive(Component)]
pub struct Recoiling {
    timer: Timer,
    kick: f32,
}

/// Aplica o coice a mira visual. Bracos e arma consomem o mesmo vetor para o
/// cano nunca subir sozinho.
pub fn animated_aim(
    intent: &Intent,
    facing: &Facing,
    recoiling: Option<&Recoiling>,
) -> (Vec2, f32) {
    let recoil = recoiling.map_or(0.0, |r| r.kick * (1.0 - r.timer.fraction()));
    let aim = aim_dir(intent, facing);
    let local = Vec2::new(aim.x * facing.0, aim.y);
    let kicked = Vec2::from_angle(recoil).rotate(local);
    (Vec2::new(kicked.x * facing.0, kicked.y), recoil)
}

/// Reticulo desenhado sobre o cursor.
#[derive(Component)]
struct Crosshair;

/// Projetil no ar.
#[derive(Component)]
pub struct Projectile;

#[derive(Component)]
struct Trail {
    timer: Timer,
    color: Color,
}

#[derive(Component)]
struct PickupPulse(f32);

/// Agenda o proximo drop.
#[derive(Resource)]
struct DropSchedule(Timer);

/// Comeca o cronometro de drops ao entrar na luta.
fn arm_drop_schedule(mut commands: Commands) {
    commands.insert_resource(DropSchedule(Timer::from_seconds(
        FIRST_DROP_DELAY,
        TimerMode::Repeating,
    )));
}

/// Larga uma arma aleatoria num dos pontos da fase.
///
/// So a autoridade sorteia. Enquanto isto rodava em toda maquina, cada uma
/// tirava um ponto e uma arma diferentes do proprio `fastrand`: o dono via uma
/// espingarda na plataforma, o cliente via uma katana no fosso, e encostar em
/// qualquer uma das duas dava o resultado errado do outro lado -- era dai que
/// saiam a arma invisivel e a arma que ninguem conseguia pegar.
fn drop_weapons(
    time: Res<Time>,
    mut commands: Commands,
    mut schedule: ResMut<DropSchedule>,
    mut ids: ResMut<NextWeaponId>,
    level: Res<CurrentLevel>,
) {
    if !schedule.0.tick(time.delta()).just_finished() {
        return;
    }
    // Depois do primeiro, o ritmo passa a ser o normal.
    schedule
        .0
        .set_duration(std::time::Duration::from_secs_f32(DROP_INTERVAL));

    let points = level.0.drop_points();
    if points.is_empty() {
        return;
    }
    let at = points[fastrand::usize(..points.len())];
    let kind = random_kind();
    let ammo = weapon_at(kind).ammo();
    ids.0 = ids.0.wrapping_add(1);

    spawn_ground_weapon(&mut commands, Some(ids.0), kind, ammo, at, Vec2::ZERO);
}

/// Encosta na arma, pega a arma. Quem ja tem uma ignora o chao.
fn pick_up(
    mut commands: Commands,
    drops: Query<(Entity, &GroundWeapon, &Transform, &Collider), Without<ThrownWeapon>>,
    players: Query<(Entity, &Transform, &Collider, &Facing), (With<Player>, Without<Held>)>,
) {
    for (player, player_transform, player_collider, facing) in &players {
        let body = player_collider.aabb(player_transform.translation.truncate());

        for (drop, ground, drop_transform, drop_collider) in &drops {
            if !overlap(
                body,
                drop_collider.aabb(drop_transform.translation.truncate()),
            ) {
                continue;
            }

            equip(&mut commands, player, ground.kind, ground.ammo, facing.0);
            commands.entity(drop).despawn();
            break;
        }
    }
}

/// Forca com que a arma sai da mao.
const THROW_SPEED: f32 = 590.0;
/// Empurrao horizontal de quem leva a arma na cara.
const THROW_PUSH: f32 = 390.0;

/// O arco de um arremesso mirando em `dir`.
///
/// A direcao gira com o cursor, o levante nao: mirar na horizontal tem que dar
/// exatamente o arremesso de sempre, e mirar no chao tem que cravar a arma la,
/// e nao jogar ela pra tras. E a mesma regra do empurrao do soco.
fn thrown_arc(dir: Vec2, lift: f32, speed: f32) -> Vec2 {
    dir * speed + Vec2::Y * lift
}

/// Arremessa a arma para onde o jogador esta mirando.
fn throw_weapon(
    mut commands: Commands,
    mut ids: ResMut<NextWeaponId>,
    mut players: Query<
        (Entity, &Intent, &Transform, &mut Facing, &Pose, &Held),
        (With<Player>, Without<Attacking>, Without<Parrying>),
    >,
) {
    for (player, intent, transform, mut facing, pose, held) in &mut players {
        if !intent.throw_weapon || pose.locks_control() {
            continue;
        }

        let dir = turn_to_aim(&mut facing, intent);
        // Sai da mao, e nao do centro do corpo: e de la que a arma parte.
        let at = transform.translation.truncate() + Vec2::new(0.0, 10.0) + dir * 46.0;
        ids.0 = ids.0.wrapping_add(1);
        let thrown = spawn_ground_weapon(
            &mut commands,
            Some(ids.0),
            held.kind,
            held.ammo,
            at,
            thrown_arc(dir, 260.0, THROW_SPEED),
        );
        commands.entity(thrown).insert((
            ThrownWeapon(Timer::from_seconds(0.55, TimerMode::Once)),
            Hitbox {
                owner: player,
                damage: held.weapon.melee(2).damage,
                knockback: thrown_arc(dir, 190.0, THROW_PUSH),
                stun: crate::combat::HIT_STUN,
            },
        ));
        commands.entity(player).remove::<Held>();
    }
}

/// Direcao do disparo: a mira do mouse quando existe, senao o lado para o qual
/// o boneco olha -- e o que mantem o jogador 2, sem mouse, jogavel do mesmo
/// jeito de antes.
pub fn aim_dir(intent: &Intent, facing: &Facing) -> Vec2 {
    if intent.aim == Vec2::ZERO {
        Vec2::new(facing.0, 0.0)
    } else {
        intent.aim
    }
}

/// Vira o boneco para a mira e devolve a direcao do golpe.
///
/// Tiro, soco e arremesso saem os tres para onde o cursor esta, e o corpo tem
/// que acompanhar os tres do mesmo jeito -- senao a arma sai pelas costas de
/// quem a jogou. Uma regra, um lugar: enquanto ela foi copiada por sistema,
/// cada copia tinha a propria margem.
///
/// A margem existe porque mira quase vertical nao tem lado: sem ela, passar o
/// cursor por cima da cabeca faria o boneco girar no lugar a cada quadro.
pub fn turn_to_aim(facing: &mut Facing, intent: &Intent) -> Vec2 {
    let dir = aim_dir(intent, facing);
    if dir.x.abs() > 0.15 {
        facing.0 = dir.x.signum();
    }
    dir
}

/// Rotacao que faz uma arte desenhada apontando para a direita mirar em `dir`,
/// ja descontando o espelhamento horizontal do sprite.
fn aim_angle(dir: Vec2, flipped: bool) -> f32 {
    let angle = dir.to_angle();
    if flipped {
        angle - std::f32::consts::PI
    } else {
        angle
    }
}

/// Dispara a arma equipada e gasta municao.
fn fire(
    time: Res<Time>,
    mut commands: Commands,
    mut shake: MessageWriter<crate::fx::Shake>,
    mut shooters: Query<
        (
            Entity,
            &Intent,
            &Transform,
            &mut Facing,
            &Pose,
            &mut Velocity,
            &mut Held,
        ),
        (With<Player>, Without<Attacking>, Without<Parrying>),
    >,
) {
    for (player, intent, transform, mut facing, pose, mut velocity, mut held) in &mut shooters {
        held.cooldown.tick(time.delta());

        if !intent.special || held.ammo == 0 || pose.locks_control() || !held.cooldown.is_finished()
        {
            continue;
        }

        // Atirar pras costas vira o boneco: o corpo acompanha a mira.
        let dir = turn_to_aim(&mut facing, intent);
        let muzzle = transform.translation.truncate() + Vec2::new(0.0, 8.0) + dir * 26.0;

        for shot in held.weapon.shots(dir) {
            let color = shot.color;
            let common = (
                Trail {
                    timer: Timer::from_seconds(0.035, TimerMode::Repeating),
                    color,
                },
                AsciiSprite::new(AsciiArt::solid(shot.glyph, color)),
                Layer::Projectile,
                // Deitado na linha de voo: uma lamina atravessada nao le como
                // lamina, e o tiro reto tambem so ganha em apontar pra onde vai.
                Transform::from_translation(muzzle.extend(0.0))
                    .with_rotation(Quat::from_rotation_z(shot.velocity.to_angle())),
                Velocity(shot.velocity),
                Collider::size(10.0, 10.0),
                DespawnOnExit(GameState::Fighting),
            );

            match shot.kind {
                ShotKind::Straight | ShotKind::Sticky => {
                    let mut projectile = commands.spawn((
                        common,
                        Projectile,
                        Hitbox {
                            owner: player,
                            damage: shot.damage,
                            knockback: shot.knockback,
                            stun: crate::combat::HIT_STUN,
                        },
                        Lifetime(Timer::from_seconds(shot.life, TimerMode::Once)),
                        Ghost,
                    ));
                    if shot.kind == ShotKind::Sticky {
                        projectile.insert(Sticky);
                    }
                }
                // Sem `Ghost` ele bate no cenario e para; sem `Hitbox` ele nao
                // machuca ao encostar. Sem `Projectile` ele nao arrebenta
                // corrente -- quem faz isso e o estouro, nao o arremesso.
                ShotKind::Lobbed {
                    fuse,
                    blast,
                    damage,
                } => {
                    commands.spawn((
                        common,
                        Falls,
                        Fuse {
                            timer: Timer::from_seconds(fuse, TimerMode::Once),
                            blast,
                            damage,
                            owner: player,
                        },
                    ));
                }
            }
        }

        let recoil = held.weapon.recoil();

        // Clarao proporcional ao coice -- o da 12 cospe fogo.
        commands.spawn((
            AsciiSprite::new(AsciiArt::solid("{*}", palette::GOLD)).flipped(facing.0 < 0.0),
            Layer::Fx,
            Transform::from_translation(muzzle.extend(0.0))
                .with_rotation(Quat::from_rotation_z(aim_angle(dir, facing.0 < 0.0)))
                .with_scale(Vec3::splat(1.0 + recoil.kick)),
            Lifetime(Timer::from_seconds(0.075, TimerMode::Once)),
            DespawnOnExit(GameState::Fighting),
        ));

        // O coice empurra contra o tiro; o pulinho extra e o que deixa a 12
        // servir de propulsor quando se atira pra baixo.
        velocity.0 -= dir * recoil.push;
        velocity.y += recoil.push * 0.18;
        shake.write(crate::fx::Shake(recoil.shake));

        held.cooldown.reset();
        held.ammo = held.ammo.saturating_sub(1);

        commands.entity(player).insert((
            Attacking(Timer::from_seconds(
                held.weapon.cooldown().min(0.18),
                TimerMode::Once,
            )),
            Recoiling {
                timer: Timer::from_seconds(0.10 + recoil.kick * 0.16, TimerMode::Once),
                kick: recoil.kick,
            },
        ));
    }
}

/// Distancia em que a granada sente um corpo e estoura na hora.
///
/// Um pouco maior que o colisor dela para o estouro acontecer ao encostar, e
/// nao depois de atravessar meio boneco.
const BOMB_TOUCH: f32 = 30.0;

/// Conta o pavio -- ou o toque -- e estoura.
///
/// O estouro e so mais um [`Hitbox`] -- grande, curto e parado. Nada no
/// combate precisou saber que existe explosao: a mesma resolucao de dano que
/// trata soco e bala trata isto.
///
/// Encostar num corpo detona na hora. O pavio deixou de ser o relogio do golpe
/// e virou so o teto: e o tempo maximo que ela fica quicando sem achar
/// ninguem. Isso troca "prever onde o outro vai estar daqui a um segundo" por
/// "acertar o corpo", que e uma mira mais direta e bem mais engracada de levar.
fn tick_fuses(
    time: Res<Time>,
    mut commands: Commands,
    mut shake: MessageWriter<crate::fx::Shake>,
    bodies: Query<
        (Entity, &Transform, &Collider),
        Or<(With<Player>, With<crate::actor::TrainingDummy>)>,
    >,
    mut bombs: Query<(Entity, &mut Fuse, &Transform)>,
) {
    for (entity, mut fuse, transform) in &mut bombs {
        let at = transform.translation.truncate();
        // Nao vale o dono: a granada sai da mao dele e passaria raspando pelo
        // proprio corpo antes de chegar a qualquer lugar.
        let touched = bodies.iter().any(|(body, body_at, collider)| {
            body != fuse.owner
                && overlap(
                    Rect::from_center_half_size(at, Vec2::splat(BOMB_TOUCH * 0.5)),
                    collider.aabb(body_at.translation.truncate()),
                )
        });
        if !fuse.timer.tick(time.delta()).is_finished() && !touched {
            continue;
        }
        commands.entity(entity).despawn();

        commands.spawn((
            Hitbox {
                owner: fuse.owner,
                damage: fuse.damage,
                // Empurrao pra cima: o estouro lanca, e num mapa com buraco
                // isso vale mais que o dano.
                knockback: Vec2::new(0.0, 430.0),
                stun: crate::combat::HIT_STUN,
            },
            // Quem morre aqui nao cai: desmonta.
            crate::combat::Explosive,
            Lifetime(Timer::from_seconds(0.09, TimerMode::Once)),
            Collider::size(fuse.blast, fuse.blast * 0.8),
            Transform::from_translation(at.extend(0.0)),
            DespawnOnExit(GameState::Fighting),
        ));

        commands.spawn((
            AsciiSprite::new(AsciiArt::solid(
                "\u{2591}\u{2592}\u{2593}\u{2588}\u{2593}\u{2592}\u{2591}",
                palette::BLOOD,
            )),
            Layer::Fx,
            Transform::from_translation(at.extend(0.0)).with_scale(Vec3::splat(2.4)),
            Lifetime(Timer::from_seconds(0.16, TimerMode::Once)),
            DespawnOnExit(GameState::Fighting),
        ));
        for i in 0..26 {
            // Fogo por fora, brasa por dentro: duas cores no mesmo estouro
            // separam a bola de fogo da fumaca que sobra.
            let (glyph, color) = if i % 3 == 0 {
                ("\u{2593}", palette::EMBER)
            } else {
                ("*", palette::GOLD)
            };
            commands.spawn((
                AsciiSprite::new(AsciiArt::solid(glyph, color)),
                Layer::Fx,
                Transform::from_translation(at.extend(0.0)),
                Velocity(Vec2::new(
                    fastrand::f32() * 760.0 - 380.0,
                    fastrand::f32() * 620.0 - 140.0,
                )),
                Ghost,
                Falls,
                Lifetime(Timer::from_seconds(0.30 + fastrand::f32() * 0.35, TimerMode::Once)),
                DespawnOnExit(GameState::Fighting),
            ));
        }
        shake.write(crate::fx::Shake(0.75));
    }
}

/// Prende no alvo o projetil que o combate marcou como cravado.
///
/// Deixa de ser projetil e vira enfeite: sem velocidade nao entra na fisica,
/// sem rastro nao pinga mais, e como filho do alvo ele anda junto com o corpo
/// sem que nada precise ficar reposicionando faca todo quadro.
fn stick_projectiles(
    mut commands: Commands,
    stuck: Query<(Entity, &StuckInto, &Transform), Added<StuckInto>>,
) {
    for (entity, stuck, transform) in &stuck {
        commands
            .entity(entity)
            .remove::<(Projectile, Sticky, StuckInto, Velocity, Ghost, Collider, Trail, Lifetime)>()
            .insert(
                Transform::from_translation(stuck.offset.extend(Layer::Projectile.z()))
                    .with_rotation(transform.rotation),
            )
            .insert(ChildOf(stuck.host));
    }
}

/// Consome o coice; sem isto a arma ficaria apontada pro ceu.
fn tick_recoil(time: Res<Time>, mut commands: Commands, mut q: Query<(Entity, &mut Recoiling)>) {
    for (entity, mut recoiling) in &mut q {
        if recoiling.timer.tick(time.delta()).is_finished() {
            commands.entity(entity).remove::<Recoiling>();
        }
    }
}

fn emit_projectile_trails(
    time: Res<Time>,
    mut commands: Commands,
    mut projectiles: Query<(&Transform, &mut Trail)>,
) {
    for (transform, mut trail) in &mut projectiles {
        if !trail.timer.tick(time.delta()).just_finished() {
            continue;
        }
        let at = transform.translation.truncate()
            + Vec2::new(fastrand::f32() * 4.0 - 2.0, fastrand::f32() * 4.0 - 2.0);
        commands.spawn((
            AsciiSprite::new(AsciiArt::solid("\u{00B7}", trail.color.with_alpha(0.7))),
            Layer::Fx,
            Transform::from_translation(at.extend(0.0)),
            Velocity(Vec2::new(0.0, 12.0)),
            Ghost,
            Lifetime(Timer::from_seconds(0.20, TimerMode::Once)),
            DespawnOnExit(GameState::Fighting),
        ));
    }
}

fn pulse_pickups(
    time: Res<Time>,
    mut q: Query<(&PickupPulse, &mut Transform), Without<ThrownWeapon>>,
) {
    for (pulse, mut transform) in &mut q {
        let wave = (time.elapsed_secs() * 4.0 + pulse.0).sin();
        transform.scale = Vec3::splat(1.0 + wave * 0.08);
        transform.rotation = Quat::from_rotation_z(wave * 0.045);
    }
}

fn animate_thrown_weapons(
    time: Res<Time>,
    mut commands: Commands,
    mut weapons: Query<(Entity, &mut ThrownWeapon, &mut Transform)>,
) {
    for (entity, mut thrown, mut transform) in &mut weapons {
        transform.rotate_z(time.delta_secs() * 13.0);
        if thrown.0.tick(time.delta()).is_finished() {
            commands.entity(entity).remove::<(ThrownWeapon, Hitbox)>();
        }
    }
}

/// Some com o icone da arma quando ela acaba.
fn clear_weapon_icon(
    mut commands: Commands,
    mut removed: RemovedComponents<Held>,
    icons: Query<(Entity, &ChildOf), With<WeaponIcon>>,
) {
    for player in removed.read() {
        for (icon, parent) in &icons {
            if parent.0 == player {
                commands.entity(icon).despawn();
            }
        }
    }
}

/// Mantem o icone da arma do lado para o qual o boneco olha.
fn animate_weapon_icon(
    players: Query<
        (
            &Facing,
            &Pose,
            &Intent,
            &Transform,
            &Held,
            Option<&MeleeAttack>,
            Option<&Recoiling>,
            &Children,
        ),
        With<Player>,
    >,
    mut icons: Query<(&mut Transform, &mut AsciiSprite), (With<WeaponIcon>, Without<Player>)>,
) {
    use crate::actor::pose::MeleeKind;

    for (facing, pose, intent, root, held, melee, recoiling, children) in &players {
        let (aim, recoil) = animated_aim(intent, facing, recoiling);
        let flipped = facing.0 < 0.0;
        let gait = pose.run_frame().map_or(0.0, |_| {
            -(root.translation.x / (9.0 * crate::actor::rig::RUN.len() as f32)
                * std::f32::consts::TAU)
                .cos()
        });
        let (kind, step) = melee.map_or((MeleeKind::Chain, 0), |m| (m.kind, m.step));
        let weapon_side = if pose.melee_phase().is_some() {
            crate::actor::pose::weapon_side(kind, step, held.weapon.style())
        } else {
            1.0
        };
        let rigging = crate::actor::rig::Rigging {
            side: weapon_side,
            facing: facing.0,
            gait,
            aim,
            strike: pose.melee_phase().map(|phase| {
                (
                    crate::actor::pose::strike_for(step, kind, held.weapon.style()),
                    phase,
                )
            }),
            reach: melee.map_or(0.0, |m| m.style.reach()),
            cycle: root.translation.y / 32.0 * std::f32::consts::TAU,
            air: 0.0,
        };
        let arm = crate::actor::rig::armed_joints(*pose, held.weapon.style(), recoil, &rigging);
        for child in children.iter() {
            if let Ok((mut transform, mut sprite)) = icons.get_mut(child) {
                let along = (arm.hand - arm.elbow).normalize_or(Vec2::new(facing.0, 0.0));
                transform.translation.x = arm.hand.x;
                transform.translation.y = arm.hand.y;
                transform.rotation = Quat::from_rotation_z(aim_angle(along, flipped));
                sprite.flip_x = flipped;
                let art = AsciiArt::solid(held.weapon.held_art(), palette::GOLD);
                if sprite.art != art {
                    sprite.art = art;
                }
            }
        }
    }
}

fn spawn_crosshair(mut commands: Commands) {
    commands.spawn((
        Crosshair,
        AsciiSprite::new(AsciiArt::solid("\u{253C}", palette::ASH)),
        Layer::Fx,
        Transform::default(),
        DespawnOnExit(GameState::Fighting),
    ));
}

/// Gruda o reticulo no cursor e o acende quando ha arma na mao.
fn track_crosshair(
    cursor: Res<CursorWorld>,
    mode: Res<crate::state::GameMode>,
    online: Res<crate::online::OnlineSession>,
    players: Query<(&Player, Has<Held>)>,
    mut crosshair: Query<(&mut Transform, &mut AsciiSprite), With<Crosshair>>,
) {
    let Some(at) = cursor.0 else {
        return;
    };
    // O reticulo e de quem esta no mouse aqui, que online nem sempre e o
    // jogador 1: quem entra num lugar mais alto olhava a arma de outra pessoa.
    let local = if *mode == crate::state::GameMode::Online {
        online.local_player_id()
    } else {
        0
    };
    let armed = players
        .iter()
        .any(|(player, held)| player.id == local && held);
    for (mut transform, mut sprite) in &mut crosshair {
        transform.translation = at.extend(0.0);
        transform.scale = Vec3::splat(if armed { 1.0 } else { 0.7 });
        let art = AsciiArt::solid(
            "\u{253C}",
            if armed {
                palette::GOLD
            } else {
                palette::ASH.with_alpha(0.55)
            },
        );
        if sprite.art != art {
            sprite.art = art;
        }
    }
}

/// Projetil que bate em terreno solido ou sai da arena morre.
///
/// Menos a faca: ela crava na parede e fica tremendo la ate sumir. Custa uma
/// linha e devolve a leitura de onde os tiros passaram.
fn despawn_spent_projectiles(
    mut commands: Commands,
    projectiles: Query<(Entity, &Transform, &Collider, Has<Sticky>), With<Projectile>>,
    solids: Query<(&Transform, &Collider), (With<Solid>, Without<Projectile>)>,
) {
    let blocks: Vec<Rect> = solids
        .iter()
        .map(|(t, c)| c.aabb(t.translation.truncate()))
        .collect();

    for (entity, transform, collider, sticky) in &projectiles {
        let at = transform.translation.truncate();
        let body = collider.aabb(at);
        if at.x.abs() > ARENA_HALF_W + 80.0 {
            commands.entity(entity).despawn();
            continue;
        }
        if !blocks.iter().any(|b| overlap(body, *b)) {
            continue;
        }
        if sticky {
            commands
                .entity(entity)
                .remove::<(Projectile, Sticky, Hitbox, Velocity, Ghost, Collider, Trail)>()
                .insert(Lifetime(Timer::from_seconds(4.0, TimerMode::Once)));
        } else {
            commands.entity(entity).despawn();
        }
    }
}

/// Arsenal, drops, disparo e projeteis.
pub struct WeaponPlugin;

impl Plugin for WeaponPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NextWeaponId>()
            .add_systems(
                OnEnter(GameState::Fighting),
                (arm_drop_schedule, spawn_crosshair),
            )
        // Quem larga, quem pega e quem arremessa e a autoridade da partida.
        // Online o cliente recebe o resultado pronto: dois sorteios
        // independentes davam duas arenas armadas de jeitos diferentes.
        .add_systems(
            Update,
            (drop_weapons, throw_weapon, pick_up)
                .chain()
                .in_set(AppSet::Logic)
                .run_if(in_state(GameState::Fighting))
                .run_if(crate::online::is_authority),
        )
        // O resto roda em todo mundo: tiro, pavio e rastro saem da arma que a
        // replicacao ja poe na mao, entao o cliente ve o disparo na hora em vez
        // de esperar o proximo pacote para desenhar a bala.
        .add_systems(
            Update,
            (
                fire,
                tick_fuses,
                tick_recoil,
                emit_projectile_trails,
                despawn_spent_projectiles,
                clear_weapon_icon,
            )
                .chain()
                .after(pick_up)
                .in_set(AppSet::Logic)
                .run_if(in_state(GameState::Fighting)),
        )
        .add_systems(
            Update,
            (
                animate_weapon_icon,
                animate_thrown_weapons,
                // Depois de toda a logica do quadro, e nao junto dela: assim a
                // faca e presa no mesmo quadro em que cravou, sem depender de
                // quem resolve dano ter rodado antes deste sistema.
                stick_projectiles,
                pulse_pickups,
                track_crosshair,
            )
                .in_set(AppSet::Animate)
                .run_if(in_state(GameState::Fighting)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Percorre o arsenal de verdade, nao uma lista a parte: arma nova entra
    /// nas invariantes sozinha, em vez de escapar delas por esquecimento.
    #[test]
    fn cada_arma_tem_combo_e_finalizador_proprios() {
        for make in ARSENAL {
            let weapon = make();
            assert!(
                weapon.melee(2).damage > weapon.melee(0).damage,
                "{}: o finalizador nao passa do primeiro golpe",
                weapon.name()
            );
            // O combate encadeia com `(step + 1) % 3`; o combo tem que girar
            // no mesmo passo, senao o quarto golpe sai do nada.
            assert_eq!(
                weapon.melee(3).damage,
                weapon.melee(0).damage,
                "{}: o combo nao volta ao inicio",
                weapon.name()
            );
        }
    }

    /// Contato e fogo sao excludentes: se a arma tem golpe pesado no M2, ela
    /// nao pode tambem ter municao, senao um botao faria duas coisas.
    #[test]
    fn arma_de_contato_nao_atira() {
        for make in ARSENAL {
            let weapon = make();
            if weapon.heavy().is_none() {
                assert!(
                    !weapon.is_melee(),
                    "{}: marcada como contato sem golpe pesado",
                    weapon.name()
                );
                continue;
            }
            assert!(weapon.is_melee(), "{}", weapon.name());
            assert_eq!(weapon.ammo(), 0, "{} tem municao", weapon.name());
            assert!(
                weapon.shots(Vec2::X).is_empty(),
                "{} produz projetil",
                weapon.name()
            );
        }
    }

    /// O golpe pesado tem que doer mais e demorar mais que o finalizador do
    /// combo -- se nao custar nada, nao ha motivo pra jogar de outro jeito.
    #[test]
    fn o_golpe_pesado_vale_a_preparacao() {
        for make in ARSENAL {
            let weapon = make();
            let Some(heavy) = weapon.heavy() else {
                continue;
            };
            let finisher = weapon.melee(2);
            assert!(
                heavy.damage > finisher.damage,
                "{}: o pesado nao dói mais que o finalizador",
                weapon.name()
            );
            assert!(
                heavy.duration > finisher.duration,
                "{}: o pesado nao custa mais tempo",
                weapon.name()
            );
        }
    }

    /// A arma arremessada vai para onde o cursor aponta.
    ///
    /// Antes ela saia sempre no sentido do olhar, e o mouse so valia para o
    /// tiro: com o oponente pendurado numa corrente acima, jogar a arma nele
    /// era impossivel mesmo com a mira em cima dele.
    #[test]
    fn o_arremesso_vai_para_a_mira() {
        for mira in [Vec2::Y, Vec2::NEG_Y, Vec2::new(-0.6, 0.8).normalize()] {
            let arco = thrown_arc(mira, 260.0, THROW_SPEED);
            let saida = (arco - Vec2::Y * 260.0).normalize();
            assert!(
                (saida - mira).length() < 0.001,
                "o arremesso ignorou a mira {mira:?} e saiu em {saida:?}"
            );
        }
    }

    /// Mirando na horizontal, o arremesso tem que ser o de sempre.
    ///
    /// O levante nao gira junto com a direcao: e ele que faz a arma descrever
    /// um arco em vez de viajar reta, e girar tudo trocaria o arco por um
    /// empurrao para tras cada vez que alguem mirasse para cima.
    #[test]
    fn o_arremesso_reto_nao_mudou() {
        for lado in [-1.0f32, 1.0] {
            let dir = Vec2::new(lado, 0.0);
            assert_eq!(
                thrown_arc(dir, 260.0, THROW_SPEED),
                Vec2::new(lado * THROW_SPEED, 260.0)
            );
            assert_eq!(
                thrown_arc(dir, 190.0, THROW_PUSH),
                Vec2::new(lado * THROW_PUSH, 190.0)
            );
        }
        // Mirando no chao, a arma desce: o levante nao pode salvar o arremesso
        // de cima para baixo.
        assert!(thrown_arc(Vec2::NEG_Y, 260.0, THROW_SPEED).y < 0.0);
    }

    /// O arco e o que distingue a bomba. Um arremesso que sai reto e so uma
    /// bala lenta.
    #[test]
    fn a_bomba_sai_em_arco() {
        // Mirando na horizontal, a componente vertical tem que existir mesmo
        // assim -- e ela que transforma a mira reta em arco.
        let shots = PipeBomb.shots(Vec2::X);
        assert_eq!(shots.len(), 1);
        assert!(shots[0].velocity.y > 0.0, "a bomba sai reta, sem arco");
        assert!(matches!(shots[0].kind, ShotKind::Lobbed { .. }));
    }

    /// Todo o dano esta no estouro, mesmo agora que encostar detona.
    ///
    /// Sao coisas diferentes: o toque decide *quando* estoura, o estouro decide
    /// *quanto* dói. Se a granada tambem machucasse de raspao, um encostao de
    /// lado cobraria duas vezes pelo mesmo acerto.
    #[test]
    fn a_bomba_nao_machuca_ao_encostar() {
        for shot in PipeBomb.shots(Vec2::X) {
            let ShotKind::Lobbed { damage, blast, .. } = shot.kind else {
                panic!("a bomba deixou de ser arremesso");
            };
            assert_eq!(shot.damage, 0, "a bomba machuca antes de estourar");
            assert!(damage > 0, "o estouro nao machuca");
            assert!(
                blast > PipeBomb.melee(2).reach,
                "o estouro nao cobre mais area que uma paulada"
            );
        }
    }

    /// So uma arma arqueia. Se duas passassem a fazer isso sem querer, o
    /// arsenal perderia o contraste que justifica ter cinco.
    #[test]
    fn o_arsenal_tem_um_unico_arremesso() {
        let arqueiam: Vec<&str> = ARSENAL
            .iter()
            .map(|make| make())
            .filter(|weapon| {
                weapon
                    .shots(Vec2::X)
                    .iter()
                    .any(|shot| matches!(shot.kind, ShotKind::Lobbed { .. }))
            })
            .map(|weapon| weapon.name())
            .collect();
        assert_eq!(arqueiam, vec!["BOMB"], "arsenal com arremessos demais");
    }

    /// Dois nomes iguais deixariam o HUD mentindo sobre o que esta na mao.
    #[test]
    fn o_arsenal_nao_repete_nome() {
        let mut names: Vec<&str> = ARSENAL.iter().map(|make| make().name()).collect();
        let total = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), total, "arsenal com nome repetido: {names:?}");
    }

    /// So a faca crava. Se duas armas passassem a fazer isso sem querer, o
    /// arsenal perderia o contraste que justifica ter seis.
    #[test]
    fn o_arsenal_tem_um_unico_projetil_que_crava() {
        let cravam: Vec<&str> = ARSENAL
            .iter()
            .map(|make| make())
            .filter(|weapon| {
                weapon
                    .shots(Vec2::X)
                    .iter()
                    .any(|shot| shot.kind == ShotKind::Sticky)
            })
            .map(|weapon| weapon.name())
            .collect();
        assert_eq!(cravam, vec!["KNIVES"]);
    }

    /// A granada tem que estourar ao encostar num corpo, e nao so quando o
    /// pavio acaba -- e essa a diferenca entre mirar num lugar e mirar em
    /// alguem.
    #[test]
    fn a_granada_estoura_no_toque() {
        // Pavio longo de proposito: se estourar num unico quadro, foi o toque.
        fn arena(distancia: f32) -> App {
            let mut app = App::new();
            app.init_resource::<Time>()
                .add_message::<crate::fx::Shake>()
                .add_systems(Update, tick_fuses);
            let dono = app
                .world_mut()
                .spawn((
                    Player {
                        id: 0,
                        color: palette::player(0),
                    },
                    Transform::from_xyz(-400.0, 0.0, 0.0),
                    Collider::size(30.0, 60.0),
                ))
                .id();
            app.world_mut().spawn((
                Player {
                    id: 1,
                    color: palette::player(1),
                },
                Transform::from_xyz(distancia, 0.0, 0.0),
                Collider::size(30.0, 60.0),
            ));
            app.world_mut().spawn((
                Fuse {
                    timer: Timer::from_seconds(30.0, TimerMode::Once),
                    blast: 150.0,
                    damage: 30,
                    owner: dono,
                },
                Transform::default(),
            ));
            app
        }

        let mut app = arena(10.0);
        app.update();
        let world = app.world_mut();
        assert_eq!(
            world.query::<&Fuse>().iter(world).count(),
            0,
            "a granada encostou no boneco e nao estourou"
        );
        // E o estouro tem que se anunciar como estouro: e o que a camada de
        // sangue le para desmontar quem morreu nele.
        assert_eq!(
            world
                .query_filtered::<&Hitbox, With<crate::combat::Explosive>>()
                .iter(world)
                .count(),
            1,
            "o estouro nao deixou hitbox de explosao"
        );

        let mut longe = arena(600.0);
        longe.update();
        let world = longe.world_mut();
        assert_eq!(
            world.query::<&Fuse>().iter(world).count(),
            1,
            "a granada estourou sem encostar em ninguem"
        );
    }

    #[test]
    fn arma_vazia_e_pega_e_nao_desaparece_ao_disparar() {
        let mut app = App::new();
        app.init_resource::<Time>()
            .add_message::<crate::fx::Shake>()
            .add_systems(Update, (pick_up, fire).chain());
        let player = app
            .world_mut()
            .spawn((
                Player {
                    id: 0,
                    color: palette::player(0),
                },
                Intent {
                    special: true,
                    ..default()
                },
                Pose::IdleA,
                Facing(1.0),
                Transform::default(),
                Velocity::default(),
                Collider::size(30.0, 30.0),
            ))
            .id();
        app.world_mut().spawn((
            GroundWeapon { kind: 0, ammo: 0 },
            Transform::default(),
            Collider::size(30.0, 30.0),
        ));

        app.update();

        assert_eq!(app.world().get::<Held>(player).unwrap().ammo, 0);
    }
}
