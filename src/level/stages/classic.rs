/// Primeiro mapa: chao com dois buracos, quatro plataformas e duas correntes.
pub struct Arena01;

impl Level for Arena01 {
    fn name(&self) -> &'static str {
        "ARENA 01 - THE GAP"
    }

    fn spawn_points(&self) -> &'static [Vec2] {
        // Os dois primeiros sao os extremos de sempre: uma sala de dois nasce
        // identica ao que era antes de existir sala de quatro. Os outros dois
        // sao o trecho central de chao, simetricos em torno dele.
        const POINTS: [Vec2; 4] = [
            Vec2::new(-500.0, 0.0),
            Vec2::new(500.0, 0.0),
            Vec2::new(-150.0, 0.0),
            Vec2::new(60.0, 0.0),
        ];
        &POINTS
    }

    fn drop_points(&self) -> &'static [Vec2] {
        const POINTS: [Vec2; 4] = [
            Vec2::new(-420.0, 180.0),
            Vec2::new(-60.0, 200.0),
            Vec2::new(330.0, 180.0),
            Vec2::new(140.0, 220.0),
        ];
        &POINTS
    }

    fn pieces(&self) -> &'static [Piece] {
        // Chao em tres trechos; os vaos entre eles sao os buracos.
        //
        // As quatro correntes nao sao enfeite: as plataformas altas estao a
        // mais de 93 unidades do chao, que e o teto do pulo, entao escalar e o
        // unico jeito de chegar la -- e por isso o letreiro promete CLIMB.
        const PIECES: [Piece; 11] = [
            Piece::Terrain {
                top: Vec2::new(-460.0, -170.0),
                cols: 45,
                rows: 6,
            },
            Piece::Terrain {
                top: Vec2::new(-45.0, -170.0),
                cols: 47,
                rows: 6,
            },
            Piece::Terrain {
                top: Vec2::new(415.0, -170.0),
                cols: 45,
                rows: 6,
            },
            Piece::Platform {
                at: Vec2::new(-430.0, -40.0),
                cols: 14,
            },
            Piece::Platform {
                at: Vec2::new(-70.0, 40.0),
                cols: 16,
            },
            Piece::Platform {
                at: Vec2::new(340.0, -40.0),
                cols: 14,
            },
            Piece::Platform {
                at: Vec2::new(150.0, 150.0),
                cols: 12,
            },
            Piece::Chain {
                top: Vec2::new(-210.0, 150.0),
                links: 24,
            },
            Piece::Chain {
                top: Vec2::new(470.0, 190.0),
                links: 28,
            },
            // Estas duas servem as plataformas que antes nao tinham acesso
            // nenhum -- e onde caem duas das quatro armas.
            Piece::Chain {
                top: Vec2::new(-340.0, 150.0),
                links: 22,
            },
            Piece::Chain {
                top: Vec2::new(250.0, 185.0),
                links: 24,
            },
        ];
        &PIECES
    }

    fn skyline(&self) -> &'static [Building] {
        const SKYLINE: [Building; 7] = [
            (-565.0, 25.0, 17, 13),
            (-410.0, 5.0, 21, 10),
            (-230.0, 40.0, 15, 15),
            (-55.0, 0.0, 23, 10),
            (150.0, 28.0, 18, 14),
            (335.0, 8.0, 22, 11),
            (535.0, 36.0, 18, 15),
        ];
        &SKYLINE
    }

    fn signs(&self) -> &'static [Sign] {
        const SIGNS: [Sign; 2] = [
            ("[ KNOCKOUT DISTRICT ]", 188.0, palette::SCENE_RED, 0.0),
            ("PUNCH // CLIMB // SURVIVE", 164.0, palette::SCENE_GOLD, 2.3),
        ];
        &SIGNS
    }
}

/// Segundo mapa: tres torres separadas por vaos largos.
///
/// A Arena 01 e uma briga no chao com dois buracos para evitar. Aqui nao existe
/// chao: o piso e a excecao, e atravessar o mapa exige plataforma ou corrente.
/// Isso troca a ameaca principal de dano para queda sem mudar regra nenhuma.
pub struct Arena02;

impl Level for Arena02 {
    fn name(&self) -> &'static str {
        "ARENA 02 - THE STACKS"
    }

    fn spawn_points(&self) -> &'static [Vec2] {
        // Torres externas primeiro; os dois extras caem nas varandas altas, que
        // sao os unicos apoios simetricos que sobram sem colar num dos dois.
        const POINTS: [Vec2; 4] = [
            Vec2::new(-520.0, -85.0),
            Vec2::new(520.0, -85.0),
            Vec2::new(-200.0, 175.0),
            Vec2::new(200.0, 175.0),
        ];
        &POINTS
    }

    fn drop_points(&self) -> &'static [Vec2] {
        // Cada ponto tem que ter chao embaixo, senao a arma nasce e cai direto
        // no vao.
        const POINTS: [Vec2; 4] = [
            Vec2::new(0.0, 215.0),
            Vec2::new(-200.0, 215.0),
            Vec2::new(200.0, 215.0),
            Vec2::new(-380.0, 200.0),
        ];
        &POINTS
    }

    fn pieces(&self) -> &'static [Piece] {
        // A escada de cada lado sobe em degraus de 50 a 70 unidades. O pulo
        // alcanca 93 de altura, entao todo degrau cabe com folga -- e o teste
        // `todo_patamar_e_alcancavel` nao deixa isso apodrecer.
        const PIECES: [Piece; 13] = [
            // torres externas
            Piece::Terrain {
                top: Vec2::new(-520.0, -120.0),
                cols: 20,
                rows: 12,
            },
            Piece::Terrain {
                top: Vec2::new(520.0, -120.0),
                cols: 20,
                rows: 12,
            },
            // torre central, a mais alta: quem a domina bate de cima, mas tem
            // menos chao pra errar.
            Piece::Terrain {
                top: Vec2::new(0.0, 10.0),
                cols: 16,
                rows: 20,
            },
            // escada esquerda
            Piece::Platform {
                at: Vec2::new(-380.0, -70.0),
                cols: 8,
            },
            Piece::Platform {
                at: Vec2::new(-250.0, 0.0),
                cols: 9,
            },
            Piece::Platform {
                at: Vec2::new(-130.0, 60.0),
                cols: 10,
            },
            // escada direita
            Piece::Platform {
                at: Vec2::new(380.0, -70.0),
                cols: 8,
            },
            Piece::Platform {
                at: Vec2::new(250.0, 0.0),
                cols: 9,
            },
            Piece::Platform {
                at: Vec2::new(130.0, 60.0),
                cols: 10,
            },
            // varanda alta dos dois lados: rota rapida, mas exposta ao tiro de
            // quem esta na torre central.
            Piece::Platform {
                at: Vec2::new(-200.0, 130.0),
                cols: 10,
            },
            Piece::Platform {
                at: Vec2::new(200.0, 130.0),
                cols: 10,
            },
            // correntes nos corredores livres entre os degraus
            Piece::Chain {
                top: Vec2::new(-320.0, 175.0),
                links: 20,
            },
            Piece::Chain {
                top: Vec2::new(320.0, 175.0),
                links: 20,
            },
        ];
        &PIECES
    }

    fn skyline(&self) -> &'static [Building] {
        // Mais altos e mais estreitos que os da Arena 01: o fundo repete a
        // verticalidade da geometria jogavel.
        const SKYLINE: [Building; 8] = [
            (-600.0, 30.0, 10, 16),
            (-460.0, 5.0, 12, 13),
            (-310.0, 45.0, 9, 18),
            (-165.0, 15.0, 13, 14),
            (10.0, 40.0, 10, 17),
            (175.0, 8.0, 12, 13),
            (350.0, 38.0, 9, 18),
            (525.0, 12.0, 13, 14),
        ];
        &SKYLINE
    }

    fn signs(&self) -> &'static [Sign] {
        const SIGNS: [Sign; 2] = [
            ("[ SCRAP TOWER 7 ]", 188.0, palette::SCENE_TOXIC, 1.1),
            ("MIND THE GAP", 164.0, palette::SCENE_RED, 0.4),
        ];
        &SIGNS
    }
}

/// Terceiro mapa: chao inteiro, sem buraco nenhum, e teto nos dois lados.
///
/// As outras duas fases decidem a briga pela queda. Esta decide pelo dano, e
/// usa o teto para dividir o espaco: encostado na parede o teto e baixo, o
/// pulo morre cedo e so sobra o jogo de chao -- combo e rasteira. No meio a
/// sala abre, e ai o gancho e a voadora voltam a valer. Onde voce esta decide
/// que golpes voce tem.
pub struct Arena03;

impl Level for Arena03 {
    fn name(&self) -> &'static str {
        "ARENA 03 - THE VAULT"
    }

    fn spawn_points(&self) -> &'static [Vec2] {
        // Os dois de sempre nascem sob as lajes laterais; os extras caem no
        // primeiro degrau do vao central -- o pedaco aberto do mapa, onde o
        // gancho e a voadora valem.
        const POINTS: [Vec2; 4] = [
            Vec2::new(-520.0, -110.0),
            Vec2::new(520.0, -110.0),
            Vec2::new(-120.0, -40.0),
            Vec2::new(120.0, -40.0),
        ];
        &POINTS
    }

    fn drop_points(&self) -> &'static [Vec2] {
        // Todos no vao central aberto: sob o teto a arma cairia em cima dele,
        // fora do alcance. Isso tambem faz do centro o lugar disputado.
        const POINTS: [Vec2; 4] = [
            Vec2::new(0.0, 215.0),
            Vec2::new(-120.0, 190.0),
            Vec2::new(120.0, 190.0),
            Vec2::new(0.0, 120.0),
        ];
        &POINTS
    }

    fn pieces(&self) -> &'static [Piece] {
        const PIECES: [Piece; 8] = [
            // chao continuo de ponta a ponta: aqui nao se perde caindo
            Piece::Terrain {
                top: Vec2::new(0.0, -170.0),
                cols: 160,
                rows: 6,
            },
            // as duas lajes que fecham as laterais
            Piece::Ceiling {
                bottom: Vec2::new(-460.0, -70.0),
                cols: 45,
                rows: 6,
            },
            Piece::Ceiling {
                bottom: Vec2::new(460.0, -70.0),
                cols: 45,
                rows: 6,
            },
            // escada do vao central, em degraus de 70
            Piece::Platform {
                at: Vec2::new(-120.0, -100.0),
                cols: 10,
            },
            Piece::Platform {
                at: Vec2::new(120.0, -100.0),
                cols: 10,
            },
            Piece::Platform {
                at: Vec2::new(-70.0, -30.0),
                cols: 9,
            },
            Piece::Platform {
                at: Vec2::new(70.0, -30.0),
                cols: 9,
            },
            // o poleiro: quem o segura domina o unico pedaco de ceu do mapa
            Piece::Platform {
                at: Vec2::new(0.0, 40.0),
                cols: 14,
            },
        ];
        &PIECES
    }

    fn skyline(&self) -> &'static [Building] {
        // Poucos e baixos: quase tudo fica escondido atras das lajes, entao
        // gastar predio aqui seria desenhar pro nada.
        const SKYLINE: [Building; 4] = [
            (-330.0, 10.0, 14, 12),
            (-110.0, 34.0, 11, 15),
            (110.0, 30.0, 12, 14),
            (330.0, 8.0, 14, 12),
        ];
        &SKYLINE
    }

    fn signs(&self) -> &'static [Sign] {
        const SIGNS: [Sign; 2] = [
            ("[ THE VAULT ]", 188.0, palette::SCENE_GOLD, 0.7),
            ("NO EXIT // NO FALLS", 164.0, palette::SCENE_RED, 1.9),
        ];
        &SIGNS
    }
}

