//! O que uma pose e para o jogo, e a coreografia dos golpes.
//!
//! O corpo continua nos 3x4 glifos originais. A complexidade vem de membros
//! independentes sobrepostos em coordenadas sub-celula, nao de engordar o
//! bitmap inteiro.
//!
//! A aparencia de cada pose -- silhueta, deformacao do corpo, membros, papel de
//! cor -- vive na tabela de [`super::rig`], e as cores concretas em
//! [`super::skin`]. Aqui fica so o que o gameplay pergunta a uma pose e a
//! coreografia dos golpes, que e dado de combate e nao de desenho.

use bevy::prelude::*;

use super::rig;
use crate::weapon::WeaponStyle;

pub const BODY_COLS: u16 = 3;
pub const BODY_ROWS: u16 = 4;

/// A ordem das variantes e a ordem da tabela de [`super::rig`], que indexa por
/// discriminante. `Dead` e sempre a ultima.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pose {
    #[default]
    IdleA,
    IdleB,
    RunA,
    RunB,
    RunC,
    RunD,
    RunE,
    RunF,
    Crouch,
    Jump,
    Fall,
    ClimbA,
    ClimbB,
    PunchWindup,
    PunchStrike,
    PunchRecover,
    Shoot,
    Parry,
    Hit,
    Downed,
    Dead,
}

impl Pose {
    /// Quantas poses existem.
    ///
    /// Escrito como "o discriminante da ultima, mais um" para que uma pose nova
    /// mude este numero sozinha: a tabela de [`super::rig`] tem exatamente este
    /// tamanho e para de compilar ate ganhar a linha nova.
    pub const COUNT: usize = Pose::Dead as usize + 1;

    pub fn locks_control(self) -> bool {
        rig::def(self).locks
    }

    /// Quadro do ciclo de corrida, se esta pose for de corrida.
    pub fn run_frame(self) -> Option<usize> {
        rig::RUN.index_of(self)
    }

    /// Pose do quadro `frame` do ciclo de corrida, girando.
    pub fn running(frame: usize) -> Pose {
        rig::RUN.at(frame)
    }

    /// Pose do quadro `beat` da respiracao parada, girando.
    pub fn idling(beat: usize) -> Pose {
        rig::IDLE.at(beat)
    }

    /// Pose do quadro `frame` da escalada, girando.
    pub fn climbing(frame: usize) -> Pose {
        rig::CLIMB.at(frame)
    }

    /// Indice da fase do golpe (preparo, contato, recuperacao), se esta pose
    /// for de corpo-a-corpo.
    pub fn melee_phase(self) -> Option<usize> {
        rig::PUNCH.index_of(self)
    }
}

/// Onde ficam o cotovelo e a mao de um braco num quadro do golpe.
///
/// `x` positivo e sempre "para a frente", no sentido em que o boneco olha:
/// quem desenha um golpe nao precisa pensar em espelhamento.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arm {
    /// Cotovelo, relativo ao centro do corpo.
    pub elbow: Vec2,
    /// Mao, relativa ao centro do corpo.
    pub hand: Vec2,
}

impl Arm {
    pub const fn new(ex: f32, ey: f32, hx: f32, hy: f32) -> Self {
        Self {
            elbow: Vec2::new(ex, ey),
            hand: Vec2::new(hx, hy),
        }
    }
}

/// Onde ficam o joelho e o pe de uma perna num quadro do golpe.
///
/// Mesmo espaco de [`Arm`]: `x` positivo e para a frente.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Leg {
    /// Joelho, relativo ao centro do corpo.
    pub knee: Vec2,
    /// Pe, relativo ao centro do corpo.
    pub foot: Vec2,
}

impl Leg {
    const fn new(kx: f32, ky: f32, fx: f32, fy: f32) -> Self {
        Self {
            knee: Vec2::new(kx, ky),
            foot: Vec2::new(fx, fy),
        }
    }
}

/// Que golpe corpo-a-corpo e este.
///
/// Escolhe a coreografia e diz se ele encadeia. Existe como enum, e nao como
/// um par de bools, porque "pesado e rasteira ao mesmo tempo" nao e um estado
/// que deva ser possivel escrever.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeleeKind {
    /// Elo do combo. Encadeia no proximo.
    Chain,
    /// M2 das armas de contato. Encerra a sequencia.
    Heavy,
    /// Golpe baixo. Derruba, e tambem encerra a sequencia.
    Sweep,
    /// Golpe aereo. So sai fora do chao, e cai junto com quem o deu.
    Dive,
}

/// A coreografia de um golpe corpo-a-corpo, quadro a quadro.
///
/// Os tres quadros sao preparo, contato e recuperacao -- as mesmas fases que
/// [`Pose::melee_phase`] devolve.
#[derive(Debug)]
pub struct Strike {
    /// Nome do golpe, exibido no painel de treino.
    pub name: &'static str,
    /// Braco da frente em cada fase.
    pub front: [Arm; 3],
    /// Braco de tras em cada fase.
    pub back: [Arm; 3],
    /// Perna da frente, quando o golpe usa ela. `None` mantem a passada
    /// normal, que e o caso de todo soco.
    pub legs: Option<[Leg; 3]>,
    /// Escala vertical do corpo no contato. Acima de 1 estica pra cima.
    pub rise: f32,
}

/// Os tres elos do combo desarmado.
///
/// Sao golpes diferentes, nao o mesmo soco tres vezes: jab curto de guarda,
/// cruzado que gira o corpo e um gancho que sobe. Como a diferenca esta aqui e
/// nao espalhada em `match`, inventar um golpe e escrever mais uma entrada --
/// `animate_limbs` nao muda uma linha.
///
/// Os numeros de dano e empurrao de cada elo vivem em `combat::unarmed_move`;
/// a ordem tem que ser a mesma, e e o que o teste
/// `a_coreografia_seque_os_golpes_do_combate` cobra.
pub const UNARMED_COMBO: [Strike; 3] = [
    Strike {
        name: "JAB",
        // Rapido e curto: so o braco da frente sai, o de tras nem abaixa.
        front: [
            Arm::new(2.0, 13.0, -4.0, 10.0),
            Arm::new(13.0, 14.0, 26.0, 14.0),
            Arm::new(8.0, 12.0, 11.0, 8.0),
        ],
        back: [
            Arm::new(-4.0, 11.0, -2.0, 18.0),
            Arm::new(-5.0, 11.0, -3.0, 18.0),
            Arm::new(-4.0, 11.0, -2.0, 17.0),
        ],
        legs: None,
        rise: 1.0,
    },
    Strike {
        name: "CROSS",
        // O braco da frente recolhe pra guarda enquanto o de tras atravessa --
        // e o recolhimento que faz o giro do corpo ler.
        front: [
            Arm::new(10.0, 12.0, 16.0, 10.0),
            Arm::new(-5.0, 12.0, -9.0, 16.0),
            Arm::new(4.0, 11.0, 6.0, 7.0),
        ],
        back: [
            Arm::new(-8.0, 12.0, -14.0, 9.0),
            Arm::new(16.0, 13.0, 33.0, 12.0),
            Arm::new(9.0, 10.0, 12.0, 5.0),
        ],
        legs: None,
        rise: 1.0,
    },
    Strike {
        name: "UPPERCUT",
        // Finalizador: o punho parte de baixo do quadril e termina acima da
        // cabeca, que e o arco que combina com o empurrao pra cima do elo 3.
        front: [
            Arm::new(6.0, 2.0, 9.0, -6.0),
            Arm::new(12.0, 14.0, 18.0, 34.0),
            Arm::new(10.0, 16.0, 14.0, 24.0),
        ],
        back: [
            Arm::new(-6.0, 10.0, -4.0, 16.0),
            Arm::new(-9.0, 6.0, -14.0, -2.0),
            Arm::new(-7.0, 9.0, -6.0, 14.0),
        ],
        legs: None,
        rise: 1.18,
    },
];

/// Estocada, corte curto e rasgo ascendente da faca.
pub const KNIFE_COMBO: [Strike; 3] = [
    Strike {
        name: "STAB",
        front: [
            Arm::new(-2.0, 9.0, -7.0, 7.0),
            Arm::new(15.0, 12.0, 36.0, 12.0),
            Arm::new(8.0, 10.0, 12.0, 7.0),
        ],
        back: [
            Arm::new(-5.0, 13.0, -2.0, 20.0),
            Arm::new(-4.0, 14.0, -1.0, 20.0),
            Arm::new(-5.0, 12.0, -2.0, 18.0),
        ],
        legs: None,
        rise: 0.98,
    },
    Strike {
        name: "SLASH",
        front: [
            Arm::new(-1.0, 18.0, 4.0, 29.0),
            Arm::new(18.0, 9.0, 34.0, 1.0),
            Arm::new(9.0, 7.0, 14.0, -2.0),
        ],
        back: [
            Arm::new(-6.0, 11.0, -3.0, 18.0),
            Arm::new(-5.0, 12.0, -2.0, 18.0),
            Arm::new(-4.0, 12.0, -1.0, 18.0),
        ],
        legs: None,
        rise: 0.94,
    },
    Strike {
        name: "RIP",
        front: [
            Arm::new(5.0, 1.0, 8.0, -8.0),
            Arm::new(15.0, 18.0, 23.0, 36.0),
            Arm::new(11.0, 15.0, 16.0, 23.0),
        ],
        back: [
            Arm::new(-6.0, 10.0, -3.0, 17.0),
            Arm::new(-8.0, 7.0, -11.0, 2.0),
            Arm::new(-5.0, 10.0, -2.0, 16.0),
        ],
        legs: None,
        rise: 1.12,
    },
];

/// Cortes largos de duas maos: saque, corte cruzado e corte ascendente.
pub const KATANA_COMBO: [Strike; 3] = [
    Strike {
        name: "DRAW",
        front: [
            Arm::new(-5.0, 5.0, -12.0, 0.0),
            Arm::new(17.0, 13.0, 39.0, 17.0),
            Arm::new(10.0, 11.0, 17.0, 8.0),
        ],
        back: [
            Arm::new(-8.0, 7.0, -14.0, 2.0),
            Arm::new(11.0, 11.0, 27.0, 14.0),
            Arm::new(5.0, 10.0, 11.0, 7.0),
        ],
        legs: None,
        rise: 0.96,
    },
    Strike {
        name: "CROSSCUT",
        front: [
            Arm::new(1.0, 23.0, 8.0, 36.0),
            Arm::new(19.0, 7.0, 42.0, -3.0),
            Arm::new(10.0, 5.0, 17.0, -5.0),
        ],
        back: [
            Arm::new(-3.0, 20.0, 4.0, 32.0),
            Arm::new(12.0, 7.0, 30.0, -1.0),
            Arm::new(5.0, 6.0, 12.0, -3.0),
        ],
        legs: None,
        rise: 0.88,
    },
    Strike {
        name: "RISING CUT",
        front: [
            Arm::new(3.0, 0.0, 8.0, -11.0),
            Arm::new(16.0, 22.0, 31.0, 38.0),
            Arm::new(10.0, 18.0, 18.0, 27.0),
        ],
        back: [
            Arm::new(-1.0, 2.0, 4.0, -8.0),
            Arm::new(10.0, 18.0, 24.0, 32.0),
            Arm::new(5.0, 15.0, 13.0, 24.0),
        ],
        legs: None,
        rise: 1.16,
    },
];

/// Estoque: tudo e ponta, nada e fio.
///
/// Os outros combos de lamina giram o braco -- este estende. A forca de uma
/// arma de estocada nao sai do arco, sai da mao indo para a frente e da perna
/// que joga o corpo atras dela, e e por isso que este e o unico combo de
/// contato em que os tres elos tem `legs`: sem o afundo, a estocada vira um
/// soco comprido com uma barra na mao.
///
/// O braco de tras sobe e recua em vez de acompanhar. E contrapeso de
/// esgrimista, e e ele que faz a pose ler de longe.
///
/// Desenhado quadro a quadro no Glyph Forge, em `glyph_forge/espada.py`: os
/// numeros aqui sao os de la, transcritos.
pub const RAPIER_COMBO: [Strike; 3] = [
    Strike {
        name: "THRUST",
        // A mao recolhe ate o quadril e explode: o preparo e o unico quadro do
        // arsenal em que o punho fica *atras* do ombro.
        front: [
            Arm::new(1.0, 10.0, -6.0, 9.0),
            Arm::new(21.0, 11.0, 44.0, 11.0),
            Arm::new(12.0, 11.0, 24.0, 12.0),
        ],
        back: [
            Arm::new(-8.0, 16.0, -13.0, 24.0),
            Arm::new(-11.0, 19.0, -19.0, 27.0),
            Arm::new(-9.0, 17.0, -15.0, 25.0),
        ],
        legs: Some([
            Leg::new(7.0, -20.0, 11.0, -31.0),
            Leg::new(20.0, -24.0, 34.0, -31.0),
            Leg::new(12.0, -21.0, 19.0, -31.0),
        ]),
        rise: 0.86,
    },
    Strike {
        name: "HIGH LINE",
        // Mesma mecanica, outra linha: a ponta sobe para o rosto. E a linha,
        // e nao a forca, que o adversario tem que ler.
        front: [
            Arm::new(2.0, 17.0, -4.0, 21.0),
            Arm::new(20.0, 17.0, 42.0, 23.0),
            Arm::new(12.0, 14.0, 24.0, 17.0),
        ],
        back: [
            Arm::new(-8.0, 15.0, -13.0, 22.0),
            Arm::new(-12.0, 20.0, -20.0, 28.0),
            Arm::new(-9.0, 17.0, -15.0, 25.0),
        ],
        legs: Some([
            Leg::new(7.0, -20.0, 11.0, -31.0),
            Leg::new(20.0, -24.0, 34.0, -31.0),
            Leg::new(12.0, -21.0, 19.0, -31.0),
        ]),
        rise: 0.92,
    },
    Strike {
        name: "FLECHE",
        // Finta e afundo. O preparo ja arrisca meia extensao -- e a mentira --
        // e o contato vai mais fundo que qualquer outro golpe do jogo.
        front: [
            Arm::new(13.0, 12.0, 26.0, 13.0),
            Arm::new(25.0, 9.0, 52.0, 8.0),
            Arm::new(14.0, 11.0, 28.0, 12.0),
        ],
        back: [
            Arm::new(-8.0, 16.0, -13.0, 23.0),
            Arm::new(-14.0, 21.0, -24.0, 29.0),
            Arm::new(-10.0, 18.0, -17.0, 26.0),
        ],
        legs: Some([
            Leg::new(10.0, -20.0, 16.0, -31.0),
            Leg::new(25.0, -26.0, 43.0, -31.0),
            Leg::new(20.0, -24.0, 34.0, -31.0),
        ]),
        rise: 0.79,
    },
];

/// O M2 do estoque: a estocada que atravessa.
///
/// Nao encadeia. E a mesma linha do `THRUST` levada ao extremo, e paga em
/// recuperacao: errar deixa o esgrimista esticado no chao alheio.
pub const RAPIER_HEAVY: Strike = Strike {
    name: "RUN THROUGH",
    front: [
        Arm::new(-2.0, 12.0, -11.0, 12.0),
        Arm::new(27.0, 10.0, 58.0, 10.0),
        Arm::new(15.0, 11.0, 30.0, 11.0),
    ],
    back: [
        Arm::new(-9.0, 17.0, -15.0, 25.0),
        Arm::new(-15.0, 22.0, -26.0, 30.0),
        Arm::new(-11.0, 18.0, -18.0, 26.0),
    ],
    legs: Some([
        Leg::new(6.0, -20.0, 9.0, -31.0),
        Leg::new(27.0, -27.0, 47.0, -31.0),
        Leg::new(18.0, -23.0, 30.0, -31.0),
    ]),
    rise: 0.74,
};

/// Giros curtos e alternados do nunchaku.
pub const NUNCHAKU_COMBO: [Strike; 3] = [
    Strike {
        name: "FLAIL",
        front: [
            Arm::new(-2.0, 18.0, -7.0, 27.0),
            Arm::new(16.0, 8.0, 35.0, 3.0),
            Arm::new(8.0, 5.0, 12.0, -5.0),
        ],
        back: [
            Arm::new(-5.0, 12.0, -2.0, 18.0),
            Arm::new(-4.0, 13.0, -1.0, 19.0),
            Arm::new(-5.0, 11.0, -2.0, 17.0),
        ],
        legs: None,
        rise: 0.95,
    },
    Strike {
        name: "RETURN",
        front: [
            Arm::new(7.0, 1.0, 12.0, -8.0),
            Arm::new(14.0, 18.0, 30.0, 31.0),
            Arm::new(9.0, 17.0, 12.0, 25.0),
        ],
        back: [
            Arm::new(-5.0, 12.0, -2.0, 18.0),
            Arm::new(-6.0, 9.0, -8.0, 14.0),
            Arm::new(-4.0, 11.0, -1.0, 17.0),
        ],
        legs: None,
        rise: 1.05,
    },
    Strike {
        name: "WHIRL",
        front: [
            Arm::new(-7.0, 10.0, -12.0, 6.0),
            Arm::new(18.0, 14.0, 38.0, 18.0),
            Arm::new(9.0, 10.0, 15.0, 7.0),
        ],
        back: [
            Arm::new(5.0, 5.0, 10.0, -3.0),
            Arm::new(-8.0, 17.0, -15.0, 24.0),
            Arm::new(-3.0, 12.0, -1.0, 18.0),
        ],
        legs: None,
        rise: 1.02,
    },
];

/// O cano agora luta como uma arma de duas maos, nao como um soco comprido.
pub const PIPE_COMBO: [Strike; 3] = [
    Strike {
        name: "RAM",
        front: [
            Arm::new(-2.0, 10.0, -8.0, 9.0),
            Arm::new(14.0, 11.0, 35.0, 11.0),
            Arm::new(8.0, 9.0, 14.0, 6.0),
        ],
        back: [
            Arm::new(-7.0, 9.0, -12.0, 8.0),
            Arm::new(9.0, 10.0, 25.0, 10.0),
            Arm::new(3.0, 9.0, 9.0, 7.0),
        ],
        legs: None,
        rise: 0.93,
    },
    Strike {
        name: "HOOK",
        front: [
            Arm::new(-4.0, 19.0, -10.0, 27.0),
            Arm::new(17.0, 15.0, 37.0, 22.0),
            Arm::new(10.0, 11.0, 16.0, 12.0),
        ],
        back: [
            Arm::new(-7.0, 16.0, -11.0, 24.0),
            Arm::new(10.0, 13.0, 27.0, 19.0),
            Arm::new(4.0, 11.0, 10.0, 12.0),
        ],
        legs: None,
        rise: 1.0,
    },
    Strike {
        name: "PIPE UPPERCUT",
        front: [
            Arm::new(4.0, 0.0, 7.0, -12.0),
            Arm::new(15.0, 21.0, 27.0, 39.0),
            Arm::new(10.0, 17.0, 16.0, 27.0),
        ],
        back: [
            Arm::new(-1.0, 1.0, 3.0, -9.0),
            Arm::new(9.0, 18.0, 20.0, 34.0),
            Arm::new(5.0, 15.0, 11.0, 24.0),
        ],
        legs: None,
        rise: 1.13,
    },
];

/// Pancada de cima pra baixo do M2 das armas de contato.
///
/// Nao entra em [`UNARMED_COMBO`] de proposito: ela nao e elo de combo, nao
/// encadeia, e o `step` que a escolhe nao gira.
pub const HEAVY_SMASH: Strike = Strike {
    name: "SMASH",
    // As duas maos sobem juntas acima da cabeca e descem na frente do corpo:
    // o arco largo e o que justifica a preparacao longa.
    front: [
        Arm::new(-2.0, 22.0, 2.0, 38.0),
        Arm::new(16.0, 14.0, 30.0, -6.0),
        Arm::new(11.0, 9.0, 17.0, 0.0),
    ],
    back: [
        Arm::new(-5.0, 20.0, -1.0, 35.0),
        Arm::new(12.0, 12.0, 24.0, -4.0),
        Arm::new(8.0, 8.0, 13.0, 1.0),
    ],
    // Desce com o corpo: o peso vai junto com o cano.
    legs: None,
    rise: 0.86,
};

pub const KNIFE_HEAVY: Strike = Strike {
    name: "LUNGE",
    front: [
        Arm::new(-6.0, 8.0, -13.0, 6.0),
        Arm::new(18.0, 10.0, 43.0, 10.0),
        Arm::new(12.0, 8.0, 22.0, 5.0),
    ],
    back: [
        Arm::new(-7.0, 12.0, -3.0, 19.0),
        Arm::new(4.0, 9.0, 15.0, 9.0),
        Arm::new(-2.0, 11.0, 4.0, 16.0),
    ],
    legs: None,
    rise: 0.90,
};

pub const KATANA_HEAVY: Strike = Strike {
    name: "EXECUTE",
    front: [
        Arm::new(-2.0, 25.0, 4.0, 42.0),
        Arm::new(18.0, 8.0, 39.0, -10.0),
        Arm::new(11.0, 7.0, 19.0, -3.0),
    ],
    back: [
        Arm::new(-6.0, 22.0, 0.0, 38.0),
        Arm::new(12.0, 7.0, 30.0, -7.0),
        Arm::new(6.0, 7.0, 14.0, -1.0),
    ],
    legs: None,
    rise: 0.78,
};

pub const NUNCHAKU_HEAVY: Strike = Strike {
    name: "CYCLONE",
    front: [
        Arm::new(-8.0, 13.0, -13.0, 19.0),
        Arm::new(18.0, 17.0, 40.0, 22.0),
        Arm::new(8.0, 8.0, 14.0, 3.0),
    ],
    back: [
        Arm::new(7.0, 5.0, 12.0, -4.0),
        Arm::new(-10.0, 16.0, -18.0, 22.0),
        Arm::new(-3.0, 10.0, 0.0, 16.0),
    ],
    legs: None,
    rise: 0.97,
};

/// Rasteira: o unico golpe que sai da perna.
///
/// Ela e a resposta baixa do jogo. O corpo desce junto com o pe porque a
/// altura e a leitura: se o boneco continuasse em pe, a rasteira seria so um
/// soco fraco com outro nome.
pub const SWEEP: Strike = Strike {
    name: "SWEEP",
    // As maos vao ao chao, apoiando o giro do corpo.
    front: [
        Arm::new(4.0, 4.0, 8.0, -8.0),
        Arm::new(-6.0, -2.0, -12.0, -14.0),
        Arm::new(2.0, 6.0, 4.0, -2.0),
    ],
    back: [
        Arm::new(-6.0, 4.0, -10.0, -6.0),
        Arm::new(-10.0, -4.0, -16.0, -16.0),
        Arm::new(-5.0, 6.0, -8.0, 0.0),
    ],
    // A perna varre rente ao chao e volta.
    legs: Some([
        Leg::new(8.0, -22.0, 4.0, -33.0),
        Leg::new(20.0, -30.0, 40.0, -34.0),
        Leg::new(12.0, -24.0, 16.0, -32.0),
    ]),
    rise: 0.74,
};

/// Voadora: o golpe do ar.
///
/// O corpo se joga na diagonal atras da perna. Ela e o terceiro lado da
/// leitura: a rasteira passa por baixo de quem esta no ar, entao quem esta no
/// ar precisa de um jeito de descer batendo.
pub const DIVE_KICK: Strike = Strike {
    name: "DIVE",
    // Os bracos vao pra tras, de contrapeso -- e o que faz o corpo parecer
    // comprometido com a queda em vez de so caindo.
    front: [
        Arm::new(-4.0, 14.0, -12.0, 18.0),
        Arm::new(-9.0, 16.0, -20.0, 22.0),
        Arm::new(-5.0, 13.0, -11.0, 15.0),
    ],
    back: [
        Arm::new(-6.0, 12.0, -14.0, 14.0),
        Arm::new(-11.0, 14.0, -22.0, 17.0),
        Arm::new(-6.0, 11.0, -12.0, 12.0),
    ],
    // A perna estica pra frente e pra baixo, na linha da queda.
    legs: Some([
        Leg::new(10.0, -16.0, 16.0, -26.0),
        Leg::new(18.0, -20.0, 34.0, -30.0),
        Leg::new(12.0, -18.0, 20.0, -28.0),
    ]),
    // Estica na diagonal: nem sobe como o gancho, nem afunda como a rasteira.
    rise: 1.06,
};

/// Braco da guarda.
///
/// Guarda e queda nao sao golpes, entao nao tem `Strike` -- mas a posicao da
/// mao delas precisa morar em algum lugar que a arma tambem possa ler.
pub const PARRY_ARM: Arm = Arm::new(9.0, 18.0, 13.0, 27.0);

/// Braco de quem esta caido no chao.
pub const DOWNED_ARM: Arm = Arm::new(-12.0, -20.0, -24.0, -27.0);

/// Onde esta a mao que segura a arma, neste quadro.
///
/// A arma le a mesma coreografia que o braco em vez de ter numeros proprios.
/// Duas listas de posicoes de mao sempre divergem: a segunda foi escrita
/// quando todo soco era o mesmo arco, e quando o combo virou tres golpes
/// distintos -- mais rasteira e voadora -- a arma passou a flutuar longe da
/// mao sem que nada reclamasse.
///
/// `None` quer dizer "sem pose definida": ai a arma fica na linha da mira.
pub fn weapon_hand(pose: Pose, kind: MeleeKind, step: u8, style: WeaponStyle) -> Option<Arm> {
    if let Some(phase) = pose.melee_phase() {
        let strike = strike_for(step, kind, style);
        return Some(if weapon_side(kind, step, style) > 0.0 {
            strike.front[phase]
        } else {
            strike.back[phase]
        });
    }
    match pose {
        Pose::Parry => Some(PARRY_ARM),
        Pose::Downed => Some(DOWNED_ARM),
        _ => None,
    }
}

/// Mao que leva a arma durante o golpe. Sai do braco mais avancado no quadro
/// de contato, entao o cruzado usa a mao de tras sem trocar no meio do arco.
pub fn weapon_side(kind: MeleeKind, step: u8, style: WeaponStyle) -> f32 {
    let strike = strike_for(step, kind, style);
    if strike.front[1].hand.x >= strike.back[1].hand.x {
        1.0
    } else {
        -1.0
    }
}

/// Coreografia do elo `step` do combo.
pub fn strike(step: u8) -> &'static Strike {
    &UNARMED_COMBO[step as usize % UNARMED_COMBO.len()]
}

/// Coreografia de um golpe, pelo tipo dele.
pub fn strike_for(step: u8, kind: MeleeKind, style: WeaponStyle) -> &'static Strike {
    match kind {
        MeleeKind::Chain => match style {
            WeaponStyle::Knife => &KNIFE_COMBO[step as usize % KNIFE_COMBO.len()],
            WeaponStyle::Katana => &KATANA_COMBO[step as usize % KATANA_COMBO.len()],
            WeaponStyle::FencySword => &RAPIER_COMBO[step as usize % RAPIER_COMBO.len()],
            WeaponStyle::Nunchaku => &NUNCHAKU_COMBO[step as usize % NUNCHAKU_COMBO.len()],
            WeaponStyle::Pipe => &PIPE_COMBO[step as usize % PIPE_COMBO.len()],
            _ => strike(step),
        },
        MeleeKind::Heavy => match style {
            WeaponStyle::Knife => &KNIFE_HEAVY,
            WeaponStyle::Katana => &KATANA_HEAVY,
            WeaponStyle::FencySword => &RAPIER_HEAVY,
            WeaponStyle::Nunchaku => &NUNCHAKU_HEAVY,
            _ => &HEAVY_SMASH,
        },
        MeleeKind::Sweep => &SWEEP,
        MeleeKind::Dive => &DIVE_KICK,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A lista de poses e a tabela de `rig`; o teste de caixa da arte vive em
    // `skin`, onde ele varre tambem as silhuetas que as peles substituem; e os
    // testes do ciclo de corrida vivem em `rig`, junto do clipe.

    /// Um golpe cuja mao nao avanca no contato foi desenhado ao contrario. O
    /// jogo nao reclama: ele so anima um soco que recua.
    #[test]
    fn todo_golpe_avanca_no_quadro_de_contato() {
        for combo in [
            &UNARMED_COMBO,
            &KNIFE_COMBO,
            &KATANA_COMBO,
            &RAPIER_COMBO,
            &NUNCHAKU_COMBO,
            &PIPE_COMBO,
        ] {
            for strike in combo {
                // O braco que ataca muda por golpe -- no cruzado e o de tras --
                // entao o que importa e o punho mais adiantado.
                let front_most =
                    |phase: usize| strike.front[phase].hand.x.max(strike.back[phase].hand.x);
                assert!(
                    front_most(1) > front_most(0),
                    "{}: o contato nao passa do preparo",
                    strike.name
                );
                assert!(
                    front_most(1) > front_most(2),
                    "{}: a recuperacao nao recolhe",
                    strike.name
                );
            }
        }
    }

    #[test]
    fn cada_arma_de_contato_tem_coreografia_propria() {
        let styles = [
            WeaponStyle::Knife,
            WeaponStyle::Katana,
            WeaponStyle::Nunchaku,
            WeaponStyle::Pipe,
        ];
        let mut names: Vec<&str> = styles
            .iter()
            .flat_map(|style| (0..3).map(|step| strike_for(step, MeleeKind::Chain, *style).name))
            .collect();
        let total = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), total, "dois estilos compartilham um golpe");
        for style in styles {
            assert_eq!(
                strike_for(3, MeleeKind::Chain, style).name,
                strike_for(0, MeleeKind::Chain, style).name
            );
        }
    }

    /// Todos os golpes, para os testes que precisam varrer o repertorio.
    const GOLPES: [(MeleeKind, u8); 6] = [
        (MeleeKind::Chain, 0),
        (MeleeKind::Chain, 1),
        (MeleeKind::Chain, 2),
        (MeleeKind::Heavy, 2),
        (MeleeKind::Sweep, 0),
        (MeleeKind::Dive, 0),
    ];

    /// A arma tem que ir aonde a mao vai, em todo golpe.
    ///
    /// Ela tinha uma lista de posicoes propria, escrita quando todo soco era o
    /// mesmo arco. Quando o combo virou tres golpes distintos, mais rasteira e
    /// voadora, a arma passou a flutuar longe da mao e nada reclamou.
    #[test]
    fn a_arma_segue_a_mao_em_todo_golpe() {
        let fases = [
            (0, Pose::PunchWindup),
            (1, Pose::PunchStrike),
            (2, Pose::PunchRecover),
        ];
        for (kind, step) in GOLPES {
            for (phase, pose) in fases {
                let arma = weapon_hand(pose, kind, step, WeaponStyle::Unarmed)
                    .unwrap_or_else(|| panic!("{kind:?} fase {phase} sem mao definida"));
                assert_eq!(
                    arma,
                    if weapon_side(kind, step, WeaponStyle::Unarmed) > 0.0 {
                        strike_for(step, kind, WeaponStyle::Unarmed).front[phase]
                    } else {
                        strike_for(step, kind, WeaponStyle::Unarmed).back[phase]
                    },
                    "{kind:?} fase {phase}: a arma nao esta na mao"
                );
            }
        }
    }

    /// O contato de cada golpe leva a arma a um lugar diferente. Se todos
    /// coincidissem, seria sinal de que ela voltou a ter numero proprio.
    #[test]
    fn cada_golpe_leva_a_arma_a_um_lugar() {
        let mut contatos: Vec<[u32; 2]> = GOLPES
            .iter()
            .map(|(kind, step)| {
                let hand = weapon_hand(Pose::PunchStrike, *kind, *step, WeaponStyle::Unarmed)
                    .unwrap()
                    .hand;
                [hand.x.to_bits(), hand.y.to_bits()]
            })
            .collect();
        let total = contatos.len();
        contatos.sort_unstable();
        contatos.dedup();
        assert_eq!(
            contatos.len(),
            total,
            "dois golpes poem a arma no mesmo ponto"
        );
    }

    /// Guarda e queda tambem precisam de mao definida: sem isso a arma volta
    /// pra linha da mira e fica apontando pro horizonte com o boneco no chao.
    #[test]
    fn guarda_e_queda_tem_mao_propria() {
        for pose in [Pose::Parry, Pose::Downed] {
            assert!(
                weapon_hand(pose, MeleeKind::Chain, 0, WeaponStyle::Unarmed).is_some(),
                "{pose:?} sem mao definida"
            );
        }
        assert!(
            weapon_hand(Pose::IdleA, MeleeKind::Chain, 0, WeaponStyle::Unarmed,).is_none(),
            "parado a arma deve seguir a mira, nao uma pose"
        );
        // Quem esta no chao segura a arma perto do chao, nao na altura do peito.
        assert!(DOWNED_ARM.hand.y < 0.0);
    }

    /// Queda e recuo tem que ser desenhos diferentes.
    ///
    /// A rasteira compra 0,62 s de vantagem contra os 0,26 de um soco. Se as
    /// duas mostrassem a mesma coisa, o jogador nao teria como saber que
    /// ganhou o tempo -- a vantagem existiria so na tabela de numeros.
    #[test]
    fn queda_e_recuo_nao_desenham_igual() {
        let art = |pose: Pose| rig::def(pose).art;
        assert_ne!(art(Pose::Downed), art(Pose::Hit));
        assert_ne!(art(Pose::Downed), art(Pose::Dead));
        assert!(
            Pose::Downed.locks_control(),
            "quem esta no chao nao pode agir"
        );
    }

    /// A rasteira e o unico golpe que sai da perna, e ela tem que varrer
    /// rente ao chao. Um pe que nao desce nem avanca e um soco disfarcado.
    #[test]
    fn a_rasteira_varre_o_chao() {
        let legs = SWEEP.legs.expect("a rasteira nao usa perna");
        assert!(
            legs[1].foot.x > legs[0].foot.x + 25.0,
            "o pe nao avanca no contato"
        );
        assert!(legs[1].foot.y < -30.0, "o pe nao esta rente ao chao");
        assert!(SWEEP.rise < 1.0, "o corpo nao abaixa junto");
        // O golpe alto e o baixo tem que mover o corpo pra lados opostos --
        // e a altura que o oponente le.
        assert!(SWEEP.rise < strike(2).rise);
    }

    /// Os tres golpes que decidem a leitura movem o corpo pra tres lados
    /// diferentes. Se dois coincidissem, o oponente nao teria o que ler.
    #[test]
    fn alto_baixo_e_aereo_sao_tres_leituras() {
        let (alto, baixo, aereo) = (strike(2).rise, SWEEP.rise, DIVE_KICK.rise);
        assert!(baixo < aereo, "rasteira e voadora afundam igual");
        assert!(aereo < alto, "voadora e gancho sobem igual");
    }

    /// A voadora sai da perna, avancando e descendo -- e o que a distingue da
    /// rasteira, que desce mas nao viaja.
    #[test]
    fn a_voadora_estica_a_perna_pra_frente_e_pra_baixo() {
        let legs = DIVE_KICK.legs.expect("a voadora nao usa perna");
        assert!(legs[1].foot.x > legs[0].foot.x, "o pe nao avanca");
        assert!(legs[1].foot.y < legs[0].foot.y, "o pe nao desce");
        // Ela nao raspa o chao como a rasteira: e um chute na diagonal.
        let sweep = SWEEP.legs.unwrap();
        assert!(
            legs[1].foot.y > sweep[1].foot.y,
            "voadora e rasteira chutam na mesma altura"
        );
    }

    /// So os golpes de perna usam perna. Se um soco passasse a mexer as
    /// pernas, a leitura de alto e baixo sumiria.
    #[test]
    fn nenhum_soco_usa_perna() {
        for strike in UNARMED_COMBO.iter().chain([&HEAVY_SMASH]) {
            assert!(
                strike.legs.is_none(),
                "{} mexe as pernas e nao devia",
                strike.name
            );
        }
    }

    /// A pancada pesada e o oposto do gancho: desce. Se as duas subissem, o
    /// M1 e o M2 do cano leriam como o mesmo golpe.
    #[test]
    fn a_pancada_pesada_desce() {
        assert!(HEAVY_SMASH.rise < 1.0, "o corpo nao acompanha a descida");
        assert!(
            HEAVY_SMASH.front[1].hand.y < HEAVY_SMASH.front[0].hand.y - 20.0,
            "o punho da pancada nao desce"
        );
        assert!(
            HEAVY_SMASH.rise < strike(2).rise,
            "pancada e gancho movem o corpo pro mesmo lado"
        );
    }

    #[test]
    fn o_golpe_pesado_nao_entra_no_combo() {
        // `heavy` escolhe a coreografia, nao o `step`: um elo de combo nunca
        // pode cair na pancada, nem o contrario.
        for step in 0..6u8 {
            assert_eq!(
                strike_for(step, MeleeKind::Heavy, WeaponStyle::Pipe).name,
                HEAVY_SMASH.name
            );
            assert_eq!(
                strike_for(step, MeleeKind::Sweep, WeaponStyle::Unarmed).name,
                SWEEP.name
            );
            assert_eq!(
                strike_for(step, MeleeKind::Dive, WeaponStyle::Unarmed).name,
                DIVE_KICK.name
            );
            assert_eq!(
                strike_for(step, MeleeKind::Chain, WeaponStyle::Unarmed).name,
                strike(step).name
            );
        }
        for special in [&HEAVY_SMASH, &SWEEP, &DIVE_KICK] {
            assert!(
                !UNARMED_COMBO.iter().any(|s| s.name == special.name),
                "{} virou elo de combo",
                special.name
            );
        }
    }

    /// O finalizador tem empurrao para cima; a animacao dele tem que subir,
    /// senao o boneco lanca o oponente com um soco reto.
    #[test]
    fn o_finalizador_sobe() {
        let uppercut = strike(2);
        assert!(uppercut.rise > 1.0, "o gancho nao estica o corpo");
        assert!(
            uppercut.front[1].hand.y > uppercut.front[0].hand.y + 20.0,
            "o punho do gancho nao sobe"
        );
    }

    /// A coreografia e indexada pelo mesmo `step` que escolhe dano e empurrao.
    /// Se as duas listas tiverem tamanhos diferentes, um elo do combo passa a
    /// animar o golpe errado sem erro nenhum.
    #[test]
    fn a_coreografia_segue_os_golpes_do_combate() {
        assert_eq!(UNARMED_COMBO.len(), 3, "o combo do combate tem tres elos");
        for step in 0..UNARMED_COMBO.len() as u8 {
            assert_eq!(strike(step).name, UNARMED_COMBO[step as usize].name);
        }
        // O ciclo tem que dar a volta: o combate manda `(step + 1) % 3`.
        assert_eq!(strike(3).name, strike(0).name);
    }

    #[test]
    fn cada_elo_do_combo_e_um_golpe_diferente() {
        for (a, b) in UNARMED_COMBO.iter().zip(UNARMED_COMBO.iter().skip(1)) {
            assert_ne!(a.name, b.name);
            assert_ne!(
                a.front[1], b.front[1],
                "{} e {} desenham o mesmo contato",
                a.name, b.name
            );
        }
    }
}
