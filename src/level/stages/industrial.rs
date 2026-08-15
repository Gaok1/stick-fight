/// A fabrica: duas alas altas e uma bacia funda no meio, que enche.
///
/// A bacia e o coracao do mapa. Descer nela e um atalho e um risco ao mesmo
/// tempo -- os dois patamares que ficam la embaixo so existem enquanto a mare
/// esta baixa, e quem estiver neles quando ela subir tem um ciclo para sair.
const ACID_1: [Piece; 14] = [
    Piece::Terrain {
        top: Vec2::new(-430.0, -170.0),
        cols: 52,
        rows: 6,
    },
    Piece::Terrain {
        top: Vec2::new(430.0, -170.0),
        cols: 52,
        rows: 6,
    },
    Piece::Terrain {
        top: Vec2::new(0.0, -250.0),
        cols: 54,
        rows: 6,
    },
    Piece::Tide {
        at: Vec2::new(0.0, -242.0),
        cols: 50,
        rise: 6,
        period: 10.0,
        phase: 0.0,
        kind: HazardKind::Acid,
    },
    Piece::Platform {
        at: Vec2::new(-120.0, -175.0),
        cols: 12,
    },
    Piece::Platform {
        at: Vec2::new(120.0, -175.0),
        cols: 12,
    },
    // Duas bocas de cano vazando sobre as alas: e o que faz o patio inteiro
    // pedir atencao, e nao so a bacia.
    Piece::Drip {
        from: Vec2::new(-250.0, 150.0),
        cols: 6,
        floor: -170.0,
        period: 1.3,
        phase: 0.0,
        kind: HazardKind::Acid,
    },
    Piece::Drip {
        from: Vec2::new(250.0, 150.0),
        cols: 6,
        floor: -170.0,
        period: 1.3,
        phase: 0.5,
        kind: HazardKind::Acid,
    },
    Piece::Platform {
        at: Vec2::new(-380.0, -100.0),
        cols: 16,
    },
    Piece::Platform {
        at: Vec2::new(380.0, -100.0),
        cols: 16,
    },
    Piece::Platform {
        at: Vec2::new(-170.0, -30.0),
        cols: 14,
    },
    Piece::Platform {
        at: Vec2::new(170.0, -30.0),
        cols: 14,
    },
    Piece::Platform {
        at: Vec2::new(0.0, 40.0),
        cols: 16,
    },
    Piece::Chain {
        top: Vec2::new(0.0, 190.0),
        links: 10,
    },
];
const ACID_2: [Piece; 12] = [
    Piece::Terrain {
        top: Vec2::new(0.0, -170.0),
        cols: 160,
        rows: 6,
    },
    Piece::Tide {
        at: Vec2::new(-300.0, -162.0),
        cols: 16,
        rise: 4,
        period: 7.5,
        phase: 0.0,
        kind: HazardKind::Acid,
    },
    Piece::Tide {
        at: Vec2::new(300.0, -162.0),
        cols: 16,
        rise: 4,
        period: 7.5,
        phase: 0.5,
        kind: HazardKind::Acid,
    },
    // Vazamento do nucleo, bem no eixo do reator.
    Piece::Drip {
        from: Vec2::new(0.0, 148.0),
        cols: 8,
        floor: -170.0,
        period: 0.9,
        phase: 0.0,
        kind: HazardKind::Acid,
    },
    Piece::Platform {
        at: Vec2::new(-450.0, -100.0),
        cols: 12,
    },
    Piece::Platform {
        at: Vec2::new(-250.0, -30.0),
        cols: 12,
    },
    Piece::Platform {
        at: Vec2::new(-70.0, 40.0),
        cols: 12,
    },
    Piece::Platform {
        at: Vec2::new(70.0, 40.0),
        cols: 12,
    },
    Piece::Platform {
        at: Vec2::new(250.0, -30.0),
        cols: 12,
    },
    Piece::Platform {
        at: Vec2::new(450.0, -100.0),
        cols: 12,
    },
    Piece::Ceiling {
        bottom: Vec2::new(0.0, 155.0),
        cols: 26,
        rows: 3,
    },
    Piece::Hazard {
        at: Vec2::new(0.0, -162.0),
        cols: 10,
        kind: HazardKind::Acid,
    },
];
/// A drenagem: a calha do meio enche e esvazia, e com ela a rota rasteira.
const ACID_3: [Piece; 10] = [
    Piece::Terrain {
        top: Vec2::new(0.0, -170.0),
        cols: 160,
        rows: 6,
    },
    Piece::Tide {
        at: Vec2::new(0.0, -162.0),
        cols: 44,
        rise: 5,
        period: 11.0,
        phase: 0.0,
        kind: HazardKind::Acid,
    },
    Piece::Hazard {
        at: Vec2::new(-330.0, -162.0),
        cols: 10,
        kind: HazardKind::Acid,
    },
    Piece::Hazard {
        at: Vec2::new(330.0, -162.0),
        cols: 10,
        kind: HazardKind::Acid,
    },
    Piece::Drip {
        from: Vec2::new(-260.0, 168.0),
        cols: 6,
        floor: -170.0,
        period: 1.5,
        phase: 0.0,
        kind: HazardKind::Acid,
    },
    Piece::Drip {
        from: Vec2::new(260.0, 168.0),
        cols: 6,
        floor: -170.0,
        period: 1.5,
        phase: 0.5,
        kind: HazardKind::Acid,
    },
    Piece::Platform {
        at: Vec2::new(-320.0, -100.0),
        cols: 14,
    },
    Piece::Platform {
        at: Vec2::new(-120.0, -30.0),
        cols: 12,
    },
    Piece::Platform {
        at: Vec2::new(120.0, -30.0),
        cols: 12,
    },
    Piece::Platform {
        at: Vec2::new(320.0, -100.0),
        cols: 14,
    },
];

