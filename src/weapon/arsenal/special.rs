/// Nunchaku: alcance de corrente com cadencia alta e pouco empurrao.
pub struct Nunchaku;

impl Weapon for Nunchaku {
    fn name(&self) -> &'static str {
        "NUNCHAKU"
    }
    /// Na mao a arte e so o porrete empunhado. Os oito `o`, o segundo porrete e
    /// o fixador dourado viram pecas fisicas; esta e a pose-base do JSON antes
    /// de a corrente comecar a reagir.
    ///
    /// ```text
    ///  ▄▄▄▄▄
    /// }▐▐▐▐▐
    ///  ▀▀▀▀▀
    /// ```
    fn held_art(&self) -> &'static str {
        " "
    }
    /// No chao e um sprite composto unico, fiel a pose de spawn modelada: dois
    /// porretes paralelos e oito elos fazendo a volta pela direita.
    ///
    /// ```text
    /// }▐▐▐▐▐oooo
    ///          o
    /// }▐▐▐▐▐ooo
    /// ```
    fn ground_art(&self) -> &'static str {
        " "
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
    /// A particularidade que nenhuma outra arma tem: o golpe continua depois
    /// que o braco parou.
    ///
    /// E o `contact` que escreve isso. Nas outras armas ele fica entre 0,2 e
    /// 0,5 da duracao -- o punho encontra o alvo no meio do arco. Aqui ele
    /// passa de 0,6: o braco ja completou o gesto e e a corrente que ainda
    /// esta chegando. Em tempo absoluto continua sendo o golpe mais rapido do
    /// arsenal depois da faca, porque a duracao inteira e curta; o que muda e
    /// que o acerto vem no fim dela, e nao no meio.
    ///
    /// O preco e o dano: seis por acerto, o menor de todas as armas de contato.
    fn melee(&self, step: u8) -> MeleeMove {
        match step % 3 {
            0 => MeleeMove {
                damage: 6,
                reach: 43.0,
                knockback: Vec2::new(185.0, 70.0),
                duration: 0.15,
                contact: 0.62,
            },
            1 => MeleeMove {
                damage: 7,
                reach: 46.0,
                knockback: Vec2::new(200.0, 100.0),
                duration: 0.16,
                contact: 0.64,
            },
            _ => MeleeMove {
                damage: 13,
                reach: 51.0,
                knockback: Vec2::new(300.0, 190.0),
                duration: 0.24,
                contact: 0.72,
            },
        }
    }
    fn heavy(&self) -> Option<MeleeMove> {
        Some(MeleeMove {
            damage: 18,
            reach: 56.0,
            knockback: Vec2::new(350.0, 150.0),
            duration: 0.34,
            contact: 0.76,
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
    /// Objeto achado, e nao fabricado. O perfil e irregular de proposito: sai
    /// grosso onde o amassado incha para baixo, magro no meio, e engrossa de
    /// novo na luva rosqueada da ponta. Simetria perfeita mataria a leitura --
    /// um tubo de perfil constante e um bastao.
    ///
    /// ```text
    ///          ▄▄▄
    /// ▐█▓█▒███▓≡≡╪
    ///   ▀▀
    /// ```
    fn held_art(&self) -> &'static str {
        "         \u{2584}\u{2584}\u{2584}\n\u{2590}\u{2588}\u{2593}\u{2588}\u{2592}\u{2588}\u{2588}\u{2588}\u{2593}\u{2261}\u{2261}\u{256a}\n  \u{2580}\u{2580}"
    }
    /// ```text
    ///           ▄▄▄
    /// ▐█▓█▒███▓█≡≡╪
    ///  ░▀▀  ░ ▀
    /// ```
    fn ground_art(&self) -> &'static str {
        "          \u{2584}\u{2584}\u{2584}\n\u{2590}\u{2588}\u{2593}\u{2588}\u{2592}\u{2588}\u{2588}\u{2588}\u{2593}\u{2588}\u{2261}\u{2261}\u{256a}\n \u{2591}\u{2580}\u{2580}  \u{2591} \u{2580}"
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
    /// A mais pesada. O que a separa da katana nao e o dano nem o alcance --
    /// nesses ela perde -- e o empurrao: o finalizador manda o outro mais longe
    /// que qualquer golpe do jogo. E o contato vem tarde dentro de um arco ja
    /// lento, entao errar com o cano deixa o corpo aberto por mais tempo que
    /// errar com qualquer outra coisa.
    fn melee(&self, step: u8) -> MeleeMove {
        match step % 3 {
            0 => MeleeMove {
                damage: 15,
                reach: 44.0,
                knockback: Vec2::new(330.0, 115.0),
                duration: 0.27,
                contact: 0.44,
            },
            1 => MeleeMove {
                damage: 17,
                reach: 48.0,
                knockback: Vec2::new(370.0, 160.0),
                duration: 0.32,
                contact: 0.46,
            },
            _ => MeleeMove {
                damage: 25,
                reach: 53.0,
                knockback: Vec2::new(520.0, 285.0),
                duration: 0.43,
                contact: 0.52,
            },
        }
    }
    /// Pancada de cima pra baixo, com a preparacao mais longa do arsenal depois
    /// da katana. Quem erra fica devendo mais de meio segundo.
    fn heavy(&self) -> Option<MeleeMove> {
        Some(MeleeMove {
            damage: 33,
            reach: 56.0,
            knockback: Vec2::new(430.0, -150.0),
            duration: 0.62,
            contact: 0.60,
        })
    }
    fn style(&self) -> WeaponStyle {
        WeaponStyle::Pipe
    }
}

/// Grimorio: dispara sigilos lentos e deixa poeira arcana no ar.
///
/// Na mao ele esta aberto de perfil: a pagina direita cobre quase toda a
/// esquerda, que aparece uma linha acima pelo desnivel. No chao a capa fica
/// fechada e de frente; o grimorio so abre quando alguem pega.
pub struct MagicBook;

impl Weapon for MagicBook {
    fn name(&self) -> &'static str {
        "MAGIC BOOK"
    }

    fn held_art(&self) -> &'static str {
        " "
    }

    fn ground_art(&self) -> &'static str {
        " "
    }

    fn cooldown(&self) -> f32 {
        0.42
    }

    fn ammo(&self) -> u32 {
        9
    }

    fn shots(&self, dir: Vec2) -> Vec<Shot> {
        let runes = magic_runes();
        let rune = runes[fastrand::usize(..runes.len())];
        vec![Shot {
            velocity: dir * 520.0,
            glyph: &rune.glyph,
            color: forge_color(&rune.color),
            damage: 12,
            knockback: dir * 210.0 + Vec2::new(0.0, 135.0),
            life: 1.8,
            kind: ShotKind::Arcane,
        }]
    }

    fn recoil(&self) -> Recoil {
        Recoil {
            push: 0.0,
            kick: 0.0,
            shake: 0.02,
        }
    }

    fn melee(&self, step: u8) -> MeleeMove {
        match step % 3 {
            0 => MeleeMove {
                damage: 7,
                reach: 30.0,
                knockback: Vec2::new(185.0, 90.0),
                duration: 0.23,
                contact: 0.30,
            },
            1 => MeleeMove {
                damage: 8,
                reach: 32.0,
                knockback: Vec2::new(210.0, 115.0),
                duration: 0.25,
                contact: 0.30,
            },
            _ => MeleeMove {
                damage: 12,
                reach: 35.0,
                knockback: Vec2::new(285.0, 180.0),
                duration: 0.31,
                contact: 0.36,
            },
        }
    }

    fn style(&self) -> WeaponStyle {
        WeaponStyle::Book
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
    // No fim para preservar os indices de rede das armas existentes.
    || Box::new(MagicBook),
    || Box::new(FencySword),
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

