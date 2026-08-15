const LAVA_1: [Piece; 10] = [
    Piece::Terrain {
        top: Vec2::new(0.0, -170.0),
        cols: 160,
        rows: 6,
    },
    Piece::Hazard {
        at: Vec2::new(0.0, -162.0),
        cols: 24,
        kind: HazardKind::Lava,
    },
    // Duas fontes fora de fase nas alas: elas fecham a passagem rasteira
    // metade do tempo cada uma, e nunca as duas ao mesmo tempo.
    Piece::Geyser {
        at: Vec2::new(-430.0, -170.0),
        cols: 3,
        rows: 9,
        period: 5.5,
        phase: 0.0,
        kind: HazardKind::Lava,
    },
    Piece::Geyser {
        at: Vec2::new(430.0, -170.0),
        cols: 3,
        rows: 9,
        period: 5.5,
        phase: 0.5,
        kind: HazardKind::Lava,
    },
    Piece::Platform {
        at: Vec2::new(-310.0, -100.0),
        cols: 15,
    },
    Piece::Platform {
        at: Vec2::new(-150.0, -30.0),
        cols: 12,
    },
    Piece::Platform {
        at: Vec2::new(0.0, 40.0),
        cols: 14,
    },
    Piece::Platform {
        at: Vec2::new(150.0, -30.0),
        cols: 12,
    },
    Piece::Platform {
        at: Vec2::new(310.0, -100.0),
        cols: 15,
    },
    Piece::Chain {
        top: Vec2::new(0.0, 190.0),
        links: 11,
    },
];
/// A ponte: chao so nas duas bordas, e a travessia por cima do vazio.
///
/// O letreiro promete NO GROUND BELOW desde a primeira versao; ate agora o
/// mapa era um piso inteirico com duas pocas em cima dele.
const LAVA_2: [Piece; 12] = [
    Piece::Terrain {
        top: Vec2::new(-480.0, -170.0),
        cols: 35,
        rows: 6,
    },
    Piece::Terrain {
        top: Vec2::new(480.0, -170.0),
        cols: 35,
        rows: 6,
    },
    Piece::Hazard {
        at: Vec2::new(-480.0, -162.0),
        cols: 10,
        kind: HazardKind::Lava,
    },
    Piece::Hazard {
        at: Vec2::new(480.0, -162.0),
        cols: 10,
        kind: HazardKind::Lava,
    },
    // Saem do fundo do desfiladeiro, fora da tela, e cortam exatamente os dois
    // vaos que a travessia obriga a pular.
    Piece::Geyser {
        at: Vec2::new(-175.0, -250.0),
        cols: 4,
        rows: 14,
        period: 6.0,
        phase: 0.0,
        kind: HazardKind::Lava,
    },
    Piece::Geyser {
        at: Vec2::new(175.0, -250.0),
        cols: 4,
        rows: 14,
        period: 6.0,
        phase: 0.5,
        kind: HazardKind::Lava,
    },
    Piece::Platform {
        at: Vec2::new(-250.0, -105.0),
        cols: 13,
    },
    Piece::Platform {
        at: Vec2::new(250.0, -105.0),
        cols: 13,
    },
    Piece::Platform {
        at: Vec2::new(0.0, -45.0),
        cols: 30,
    },
    Piece::Platform {
        at: Vec2::new(0.0, 30.0),
        cols: 12,
    },
    Piece::Chain {
        top: Vec2::new(-90.0, 180.0),
        links: 9,
    },
    Piece::Chain {
        top: Vec2::new(90.0, 180.0),
        links: 9,
    },
];
const LAVA_3: [Piece; 12] = [
    Piece::Terrain {
        top: Vec2::new(0.0, -170.0),
        cols: 160,
        rows: 6,
    },
    Piece::Hazard {
        at: Vec2::new(0.0, -162.0),
        cols: 12,
        kind: HazardKind::Lava,
    },
    // O forno transborda: as duas pocas das alas sobem ate engolir a beira dos
    // patamares baixos, e descem de novo.
    Piece::Tide {
        at: Vec2::new(-330.0, -162.0),
        cols: 14,
        rise: 5,
        period: 8.0,
        phase: 0.0,
        kind: HazardKind::Lava,
    },
    Piece::Tide {
        at: Vec2::new(330.0, -162.0),
        cols: 14,
        rise: 5,
        period: 8.0,
        phase: 0.5,
        kind: HazardKind::Lava,
    },
    // Pinga do teto: e o unico perigo do jogo que vem de cima, e ele existe
    // porque o teto desta arena ja obriga a briga a ficar rasteira.
    Piece::Drip {
        from: Vec2::new(-100.0, 146.0),
        cols: 6,
        floor: -162.0,
        period: 1.1,
        phase: 0.0,
        kind: HazardKind::Lava,
    },
    Piece::Drip {
        from: Vec2::new(100.0, 146.0),
        cols: 6,
        floor: -162.0,
        period: 1.1,
        phase: 0.5,
        kind: HazardKind::Lava,
    },
    Piece::Platform {
        at: Vec2::new(-400.0, -100.0),
        cols: 12,
    },
    Piece::Platform {
        at: Vec2::new(-200.0, -30.0),
        cols: 12,
    },
    Piece::Platform {
        at: Vec2::new(0.0, 40.0),
        cols: 15,
    },
    Piece::Platform {
        at: Vec2::new(200.0, -30.0),
        cols: 12,
    },
    Piece::Platform {
        at: Vec2::new(400.0, -100.0),
        cols: 12,
    },
    Piece::Ceiling {
        bottom: Vec2::new(0.0, 150.0),
        cols: 34,
        rows: 3,
    },
];

