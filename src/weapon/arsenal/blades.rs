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
    /// Um leque, e nao uma pilha. Tres linhas paralelas era o que o desenho
    /// antigo entregava -- `░▀▀▀▀►` tres vezes, uma em cima da outra --, e nada
    /// naquilo dizia faca: dizia grade.
    ///
    /// O que separa as tres agora e a **inclinacao**. A de cima nasce na meia
    /// celula de baixo da primeira linha (`▄`) e termina na de cima (`▀`): ela
    /// sobe. A de baixo faz o contrario e desce. A do meio corre reta no `═`, que
    /// e fino e mora no centro da celula. Assim as pontas se afastam do punho,
    /// que e o que um punhado de facas na mao faz.
    ///
    /// A meia celula vazia entre cada uma continua sendo o que impede o leque de
    /// fundir num gume so, e o `░` na base das duas de fora e o cabo delas: sem
    /// ele as laminas comecavam soltas no ar a uma celula do punho, que e o que
    /// fazia as tres lerem como barras em vez de facas.
    ///
    /// ```text
    ///   ░▄▄▄▀▀►
    /// █╪═══════►
    ///   ░▀▀▀▄▄►
    /// ```
    fn held_art(&self) -> &'static str {
        "  \u{2591}\u{2584}\u{2584}\u{2584}\u{2580}\u{2580}\u{25ba}\n\u{2588}\u{256a}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{25ba}\n  \u{2591}\u{2580}\u{2580}\u{2580}\u{2584}\u{2584}\u{25ba}"
    }
    /// ```text
    ///   ░▄▄▄▄▀▀►
    /// █╪════════►
    ///   ░▀▀▀▀▄▄►
    /// ```
    fn ground_art(&self) -> &'static str {
        "  \u{2591}\u{2584}\u{2584}\u{2584}\u{2584}\u{2580}\u{2580}\u{25ba}\n\u{2588}\u{256a}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{25ba}\n  \u{2591}\u{2580}\u{2580}\u{2580}\u{2580}\u{2584}\u{2584}\u{25ba}"
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
    /// A mesma lamina da katana em escala de bolso: costa escura, gume claro,
    /// ponta que afina meia celula antes do `►`. Cabo de duas celulas contra
    /// cinco de lamina -- e a proporcao que separa faca de espada, e nao o
    /// desenho.
    ///
    /// A guarda e o par `╤`/`╧`, a versao magra do `╦`/`╩` da katana: atravessa
    /// a linha da lamina para os dois lados sem virar tsuba. Cabo e lamina
    /// dividem a mesma linha; o `▀▀` que antes pendurava meia celula abaixo do
    /// punho fazia a faca parecer dobrada no cabo.
    ///
    /// ```text
    /// ▄▄╤▄▄▄▄►
    /// ▌≡╧▀▀▀
    ///  '
    /// ```
    fn held_art(&self) -> &'static str {
        "\u{2584}\u{2584}\u{2564}\u{2584}\u{2584}\u{2584}\u{2584}\u{25ba}\n\u{258c}\u{2261}\u{2567}\u{2580}\u{2580}\u{2580}\n '"
    }
    /// ```text
    /// ▄▄╤▄▄▄▄▄▄►
    /// ▌≡╧▀▀▀▀▀
    ///  ░▒░▒
    /// ```
    fn ground_art(&self) -> &'static str {
        "\u{2584}\u{2584}\u{2564}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{25ba}\n\u{258c}\u{2261}\u{2567}\u{2580}\u{2580}\u{2580}\u{2580}\u{2580}\n \u{2591}\u{2592}\u{2591}\u{2592}"
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
    /// Depressa e de perto. Ela troca dano por tempo: o combo inteiro fecha em
    /// 0,43 s -- o mais curto do arsenal -- e nenhum elo passa de 15 de dano.
    /// Quem escolhe a faca escolhe estar dentro do alcance do outro o tempo
    /// todo, e o preco de errar e baixo justamente porque o premio tambem e.
    fn melee(&self, step: u8) -> MeleeMove {
        match step % 3 {
            0 => MeleeMove {
                damage: 8,
                reach: 28.0,
                knockback: Vec2::new(195.0, 70.0),
                duration: 0.12,
                contact: 0.22,
            },
            1 => MeleeMove {
                damage: 9,
                reach: 30.0,
                knockback: Vec2::new(220.0, 105.0),
                duration: 0.12,
                contact: 0.20,
            },
            _ => MeleeMove {
                damage: 15,
                reach: 33.0,
                knockback: Vec2::new(310.0, 190.0),
                duration: 0.19,
                contact: 0.30,
            },
        }
    }
    fn heavy(&self) -> Option<MeleeMove> {
        Some(MeleeMove {
            damage: 21,
            reach: 42.0,
            knockback: Vec2::new(370.0, 120.0),
            duration: 0.28,
            contact: 0.40,
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
    /// Cabo, tsuba e lamina na mesma linha. A lamina e uma costa escura (`▄`)
    /// por cima de um gume claro (`▀`), e nas duas ultimas celulas o gume acaba:
    /// ela afina e sobe meia celula, que e o `sori`, e fecha no `►`.
    ///
    /// O cabo e o que faltava. Antes eram tres glifos -- `▒█○` -- com um `▀▀`
    /// pendurado meia celula abaixo, e o desenho lia como um bastao torto com
    /// uma conta enfiada no meio. Agora ele tem o comprimento certo (quatro
    /// celulas contra dez de lamina), a trama de losango do `ito` em `X≡X` e o
    /// `▌` do kashira fechando a ponta.
    ///
    /// A tsuba e o par `╦`/`╩`: um empilhado no outro sai uma barra atravessada
    /// com tampa em cima e embaixo, centrada exatamente na linha da lamina. Um
    /// `○` no meio do cabo nunca foi guarda -- guarda e o que atravessa.
    ///
    /// ```text
    /// ▄▄▄▄╦▄▄▄▄▄▄▄▄▄►
    /// ▌X≡X╩▀▀▀▀▀▀▀
    ///  '
    /// ```
    fn held_art(&self) -> &'static str {
        "\u{2584}\u{2584}\u{2584}\u{2584}\u{2566}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{25ba}\n\u{258c}X\u{2261}X\u{2569}\u{2580}\u{2580}\u{2580}\u{2580}\u{2580}\u{2580}\u{2580}\n '"
    }
    /// No chao ela vem com a bainha ao lado, que e a leitura que a lamina
    /// sozinha nao da.
    ///
    /// ```text
    /// ▄▄▄▄╦▄▄▄▄▄▄▄▄▄▄▄►
    /// ▌X≡X╩▀▀▀▀▀▀▀▀▀
    ///  ░▒░▒░▒░▒░▒░▒
    /// ```
    fn ground_art(&self) -> &'static str {
        "\u{2584}\u{2584}\u{2584}\u{2584}\u{2566}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{25ba}\n\u{258c}X\u{2261}X\u{2569}\u{2580}\u{2580}\u{2580}\u{2580}\u{2580}\u{2580}\u{2580}\u{2580}\u{2580}\n \u{2591}\u{2592}\u{2591}\u{2592}\u{2591}\u{2592}\u{2591}\u{2592}\u{2591}\u{2592}\u{2591}\u{2592}"
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
    /// O maior alcance e o maior dano por elo do arsenal, pagos em tempo: cada
    /// corte custa quase o triplo do de uma faca, e o finalizador deixa quase
    /// meio segundo de recuperacao. Errar tem que doer, e e a duracao -- nao o
    /// dano -- que cobra.
    fn melee(&self, step: u8) -> MeleeMove {
        match step % 3 {
            0 => MeleeMove {
                damage: 16,
                reach: 50.0,
                knockback: Vec2::new(300.0, 105.0),
                duration: 0.30,
                contact: 0.40,
            },
            1 => MeleeMove {
                damage: 18,
                reach: 54.0,
                knockback: Vec2::new(340.0, 150.0),
                duration: 0.34,
                contact: 0.42,
            },
            _ => MeleeMove {
                damage: 27,
                reach: 60.0,
                knockback: Vec2::new(465.0, 255.0),
                duration: 0.46,
                contact: 0.50,
            },
        }
    }
    fn heavy(&self) -> Option<MeleeMove> {
        Some(MeleeMove {
            damage: 36,
            reach: 68.0,
            knockback: Vec2::new(540.0, 85.0),
            duration: 0.66,
            contact: 0.56,
        })
    }
    fn style(&self) -> WeaponStyle {
        WeaponStyle::Katana
    }
}

/// Fency sword: o estoque. Lamina fina de ponta, desenhada no Glyph Forge.
///
/// E a arma de alcance do arsenal de contato. A katana bate mais forte por
/// golpe, mas precisa chegar perto e gira o corpo inteiro para isso; o estoque
/// alcanca de longe e sai barato -- e paga essa vantagem em dano por acerto e
/// em nao ter nenhuma leitura alta ou baixa alem da propria linha.
///
/// A silhueta nao mora aqui: ela vem de
/// `glyph_forge/creations/armas/fency_sword.glyph.json`, como a do livro e a do
/// nunchaku. `held_art` e `ground_art` ficam vazios de proposito -- quem desenha
/// e `sword_model_parts`.
pub struct FencySword;

impl Weapon for FencySword {
    fn name(&self) -> &'static str {
        "FENCY SWORD"
    }

    fn held_art(&self) -> &'static str {
        " "
    }

    fn ground_art(&self) -> &'static str {
        " "
    }

    fn cooldown(&self) -> f32 {
        0.74
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

    /// Alcance de katana com dano de faca, e rapido como faca no primeiro elo.
    ///
    /// O contato vem cedo (0,22 do preparo no primeiro elo): a estocada e a
    /// coisa mais rapida a sair de uma guarda, e e nisso que ela ganha. O
    /// finalizador inverte -- alcance maximo do jogo, mas quase meio segundo
    /// preso na extensao, que e onde o adversario cobra o erro.
    fn melee(&self, step: u8) -> MeleeMove {
        match step % 3 {
            0 => MeleeMove {
                damage: 11,
                reach: 52.0,
                knockback: Vec2::new(190.0, 60.0),
                duration: 0.20,
                contact: 0.22,
            },
            1 => MeleeMove {
                damage: 12,
                reach: 56.0,
                knockback: Vec2::new(210.0, 95.0),
                duration: 0.24,
                contact: 0.24,
            },
            _ => MeleeMove {
                damage: 20,
                reach: 66.0,
                knockback: Vec2::new(330.0, 120.0),
                duration: 0.46,
                contact: 0.28,
            },
        }
    }

    /// O M2 atravessa: o maior alcance do jogo, e a maior recuperacao entre as
    /// laminas. Dano abaixo do da katana de proposito -- o estoque cobra
    /// distancia, nao peso, e ganhar as duas coisas o deixaria sem resposta.
    fn heavy(&self) -> Option<MeleeMove> {
        Some(MeleeMove {
            damage: 26,
            reach: 78.0,
            knockback: Vec2::new(300.0, 70.0),
            duration: 0.58,
            contact: 0.34,
        })
    }

    fn style(&self) -> WeaponStyle {
        WeaponStyle::FencySword
    }
}
