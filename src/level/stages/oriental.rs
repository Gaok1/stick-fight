const EAST_1: [Piece; 10] = [
    Piece::Terrain {
        top: Vec2::new(0.0, -170.0),
        cols: 160,
        rows: 6,
    },
    // A boca de pedra do portao: fogo de jade subindo no eixo da arena.
    Piece::Geyser {
        at: Vec2::new(0.0, -170.0),
        cols: 5,
        rows: 11,
        period: 6.5,
        phase: 0.0,
        kind: HazardKind::Jade,
    },
    Piece::Hazard {
        at: Vec2::new(-280.0, -162.0),
        cols: 8,
        kind: HazardKind::Spikes,
    },
    Piece::Hazard {
        at: Vec2::new(280.0, -162.0),
        cols: 8,
        kind: HazardKind::Spikes,
    },
    Piece::Platform {
        at: Vec2::new(-360.0, -100.0),
        cols: 18,
    },
    Piece::Platform {
        at: Vec2::new(-170.0, -30.0),
        cols: 16,
    },
    Piece::Platform {
        at: Vec2::new(0.0, 40.0),
        cols: 16,
    },
    Piece::Platform {
        at: Vec2::new(170.0, -30.0),
        cols: 16,
    },
    Piece::Platform {
        at: Vec2::new(360.0, -100.0),
        cols: 18,
    },
    Piece::Chain {
        top: Vec2::new(0.0, 195.0),
        links: 11,
    },
];
const EAST_2: [Piece; 12] = [
    Piece::Terrain {
        top: Vec2::new(0.0, -170.0),
        cols: 160,
        rows: 6,
    },
    Piece::Hazard {
        at: Vec2::new(-280.0, -162.0),
        cols: 8,
        kind: HazardKind::Spikes,
    },
    Piece::Hazard {
        at: Vec2::new(280.0, -162.0),
        cols: 8,
        kind: HazardKind::Spikes,
    },
    // Duas fontes de jade sob os patamares do meio: elas nao alcancam o
    // tabuado, mas fecham a descida enquanto estao abertas.
    Piece::Geyser {
        at: Vec2::new(-90.0, -170.0),
        cols: 4,
        rows: 9,
        period: 5.0,
        phase: 0.0,
        kind: HazardKind::Jade,
    },
    Piece::Geyser {
        at: Vec2::new(90.0, -170.0),
        cols: 4,
        rows: 9,
        period: 5.0,
        phase: 0.5,
        kind: HazardKind::Jade,
    },
    Piece::Platform {
        at: Vec2::new(-450.0, -100.0),
        cols: 14,
    },
    Piece::Platform {
        at: Vec2::new(-240.0, -30.0),
        cols: 18,
    },
    Piece::Platform {
        at: Vec2::new(-70.0, 40.0),
        cols: 11,
    },
    Piece::Platform {
        at: Vec2::new(70.0, 40.0),
        cols: 11,
    },
    Piece::Platform {
        at: Vec2::new(240.0, -30.0),
        cols: 18,
    },
    Piece::Platform {
        at: Vec2::new(450.0, -100.0),
        cols: 14,
    },
    Piece::Ceiling {
        bottom: Vec2::new(0.0, 165.0),
        cols: 32,
        rows: 2,
    },
];
/// O jardim do dragao: tres bocas de pedra e dois braseiros, todos de jade.
///
/// O mapa que dava nome ao dragao nao tinha nada de jade nem de dragao no que
/// se joga. Aqui as tres fontes sao o bicho do fundo respirando: a do meio
/// sobe ate lamber o tabuado alto, e as duas das alas cortam a rota rasteira
/// em tempos diferentes.
const EAST_3: [Piece; 12] = [
    Piece::Terrain {
        top: Vec2::new(0.0, -170.0),
        cols: 160,
        rows: 6,
    },
    Piece::Geyser {
        at: Vec2::new(0.0, -170.0),
        cols: 5,
        rows: 13,
        period: 6.0,
        phase: 0.0,
        kind: HazardKind::Jade,
    },
    Piece::Geyser {
        at: Vec2::new(-290.0, -170.0),
        cols: 4,
        rows: 9,
        period: 6.0,
        phase: 0.33,
        kind: HazardKind::Jade,
    },
    Piece::Geyser {
        at: Vec2::new(290.0, -170.0),
        cols: 4,
        rows: 9,
        period: 6.0,
        phase: 0.66,
        kind: HazardKind::Jade,
    },
    Piece::Hazard {
        at: Vec2::new(-190.0, -162.0),
        cols: 8,
        kind: HazardKind::Jade,
    },
    Piece::Hazard {
        at: Vec2::new(190.0, -162.0),
        cols: 8,
        kind: HazardKind::Jade,
    },
    Piece::Platform {
        at: Vec2::new(-380.0, -100.0),
        cols: 15,
    },
    Piece::Platform {
        at: Vec2::new(-190.0, -30.0),
        cols: 13,
    },
    Piece::Platform {
        at: Vec2::new(0.0, 40.0),
        cols: 20,
    },
    Piece::Platform {
        at: Vec2::new(190.0, -30.0),
        cols: 13,
    },
    Piece::Platform {
        at: Vec2::new(380.0, -100.0),
        cols: 15,
    },
    Piece::Chain {
        top: Vec2::new(0.0, 190.0),
        links: 10,
    },
];

