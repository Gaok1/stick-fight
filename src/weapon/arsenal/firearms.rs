/// Pistola: um tiro reto, preciso.
pub struct Pistol;

impl Weapon for Pistol {
    fn name(&self) -> &'static str {
        "PISTOL"
    }
    /// O L. Ferrolho deitado em cima, coronha caindo para tras embaixo, e a
    /// coronha mais funda atras que na frente -- e a inclinacao dela, e nao o
    /// cano, que faz uma pistola parecer uma pistola.
    ///
    /// ```text
    ///   ▄▄▄▄▄
    /// ▓█████▓►
    /// █▀
    /// ```
    fn held_art(&self) -> &'static str {
        "  \u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\n\u{2593}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2593}\u{25ba}\n\u{2588}\u{2580}"
    }
    /// ```text
    ///    ▄▄▄▄▄
    /// ▓██████▓►
    /// ██▀
    /// ```
    fn ground_art(&self) -> &'static str {
        "   \u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\n\u{2593}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2593}\u{25ba}\n\u{2588}\u{2588}\u{2580}"
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
    /// Bloco atras, tubo fino na frente. O degrau entre os dois -- coronha e
    /// pump grossos, cano magro -- e o que separa a 12 do rifle a distancia; o
    /// entalhe no pulso da coronha e o que impede o bloco de ler como um
    /// tijolo unico.
    ///
    /// ```text
    ///    ▄▄▄▄▄▄▄▄▄
    /// ▓██▀███▀▀▀▀▀
    /// ▀▀
    /// ```
    fn held_art(&self) -> &'static str {
        "   \u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\n\u{2593}\u{2588}\u{2588}\u{2580}\u{2588}\u{2588}\u{2588}\u{2580}\u{2580}\u{2580}\u{2580}\u{2580}\n\u{2580}\u{2580}"
    }
    /// No chao ela ganha os dois cartuchos caidos ao lado, que e a leitura mais
    /// rapida de "isto e uma espingarda" que existe.
    ///
    /// ```text
    ///     ▄▄▄▄▄▄▄▄▄▄
    /// ▓███▀████▀▀▀▀▀
    /// ▀▀▀ ○○
    /// ```
    fn ground_art(&self) -> &'static str {
        "    \u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\n\u{2593}\u{2588}\u{2588}\u{2588}\u{2580}\u{2588}\u{2588}\u{2588}\u{2588}\u{2580}\u{2580}\u{2580}\u{2580}\u{2580}\n\u{2580}\u{2580}\u{2580} \u{25cb}\u{25cb}"
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
    /// Le por cima e por baixo, e nao pelo comprimento: a luneta levanta um
    /// bloco acima da caixa e o carregador desce um abaixo dela. Sem essas
    /// duas saliencias ele vira o mesmo tubo comprido da escopeta.
    ///
    /// ```text
    ///   ▄██▄ ▄▄▄▄▄▄▄
    /// ▓█▓████▀▀▀▀▀▀▀
    /// ▀▀  ▓▌
    /// ```
    fn held_art(&self) -> &'static str {
        "  \u{2584}\u{2588}\u{2588}\u{2584} \u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\n\u{2593}\u{2588}\u{2593}\u{2588}\u{2588}\u{2588}\u{2588}\u{2580}\u{2580}\u{2580}\u{2580}\u{2580}\u{2580}\u{2580}\n\u{2580}\u{2580}  \u{2593}\u{258c}"
    }
    /// ```text
    ///    ▄██▄  ▄▄▄▄▄▄▄▄
    /// ▓██▓█████▀▀▀▀▀▀▀▀
    /// ▀▀▀  ▓▓▌
    /// ```
    fn ground_art(&self) -> &'static str {
        "   \u{2584}\u{2588}\u{2588}\u{2584}  \u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\n\u{2593}\u{2588}\u{2588}\u{2593}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2580}\u{2580}\u{2580}\u{2580}\u{2580}\u{2580}\u{2580}\u{2580}\n\u{2580}\u{2580}\u{2580}  \u{2593}\u{2593}\u{258c}"
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
    /// Cilindro curto com tampa rosqueada nas duas pontas, e o pavio saindo por
    /// cima. A leitura e o pavio: um cilindro sem ele e um pedaco de cano.
    ///
    /// ```text
    ///   ▄
    /// ╞▓█▓╡
    ///  ▀▀▀
    /// ```
    fn held_art(&self) -> &'static str {
        "  \u{2584}\n\u{255e}\u{2593}\u{2588}\u{2593}\u{2561}\n \u{2580}\u{2580}\u{2580}"
    }
    /// ```text
    ///   ▄~*
    /// ╞▓█▓╡
    ///  ▀▀▀
    /// ```
    fn ground_art(&self) -> &'static str {
        "  \u{2584}~*\n\u{255e}\u{2593}\u{2588}\u{2593}\u{2561}\n \u{2580}\u{2580}\u{2580}"
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
            glyph: "\u{25d8}",
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

