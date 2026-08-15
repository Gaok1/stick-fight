/// Arena tematica descrita so por dados. Assim os nove mapas novos nao
/// repetem nove vezes a mesma implementacao de `Level`.
struct ThemedArena(&'static StageDef);

struct StageDef {
    name: &'static str,
    scene: Scene,
    spawns: &'static [Vec2],
    drops: &'static [Vec2],
    pieces: &'static [Piece],
    skyline: &'static [Building],
    signs: &'static [Sign],
}

impl Level for ThemedArena {
    fn name(&self) -> &'static str {
        self.0.name
    }
    fn spawn_points(&self) -> &'static [Vec2] {
        self.0.spawns
    }
    fn drop_points(&self) -> &'static [Vec2] {
        self.0.drops
    }
    fn pieces(&self) -> &'static [Piece] {
        self.0.pieces
    }
    fn skyline(&self) -> &'static [Building] {
        self.0.skyline
    }
    fn signs(&self) -> &'static [Sign] {
        self.0.signs
    }
    fn scene(&self) -> Scene {
        self.0.scene
    }
}

const SPAWNS_WIDE: [Vec2; 4] = [
    Vec2::new(-500.0, 0.0),
    Vec2::new(500.0, 0.0),
    Vec2::new(-220.0, 0.0),
    Vec2::new(220.0, 0.0),
];
const SPAWNS_INNER: [Vec2; 4] = [
    Vec2::new(-430.0, 0.0),
    Vec2::new(430.0, 0.0),
    Vec2::new(-140.0, 0.0),
    Vec2::new(140.0, 0.0),
];
/// Os dois patamares da ponte: no desfiladeiro nao ha chao no meio, entao os
/// quatro lugares tem que caber nas bordas.
const SPAWNS_CHASM: [Vec2; 4] = [
    Vec2::new(-560.0, 0.0),
    Vec2::new(560.0, 0.0),
    Vec2::new(-390.0, 0.0),
    Vec2::new(390.0, 0.0),
];
/// As duas alas da fabrica, fora da bacia que enche.
const SPAWNS_YARD: [Vec2; 4] = [
    Vec2::new(-560.0, 0.0),
    Vec2::new(560.0, 0.0),
    Vec2::new(-330.0, 0.0),
    Vec2::new(330.0, 0.0),
];
const DROPS: [Vec2; 4] = [
    Vec2::new(-420.0, 190.0),
    Vec2::new(420.0, 190.0),
    Vec2::new(-120.0, 210.0),
    Vec2::new(120.0, 210.0),
];
const VOLCANO_SKY: [Building; 5] = [
    (-540.0, -10.0, 18, 9),
    (-310.0, 16.0, 16, 12),
    (0.0, -20.0, 24, 8),
    (310.0, 16.0, 16, 12),
    (540.0, -10.0, 18, 9),
];
const INDUSTRIAL_SKY: [Building; 6] = [
    (-550.0, 30.0, 16, 14),
    (-350.0, -5.0, 22, 10),
    (-120.0, 42.0, 14, 16),
    (120.0, 42.0, 14, 16),
    (350.0, -5.0, 22, 10),
    (550.0, 30.0, 16, 14),
];
const ORIENTAL_SKY: [Building; 5] = [
    (-520.0, -20.0, 14, 8),
    (-280.0, 8.0, 18, 10),
    (0.0, 45.0, 20, 15),
    (280.0, 8.0, 18, 10),
    (520.0, -20.0, 14, 8),
];

const CALDERA_SIGNS: [Sign; 2] = [
    ("[ CALDERA // 01 ]", 188.0, palette::SCENE_RED, 0.0),
    ("THE MOUNTAIN IS AWAKE", 164.0, palette::SCENE_GOLD, 1.7),
];
const BRIDGE_SIGNS: [Sign; 2] = [
    ("[ BASALT CROSSING ]", 188.0, palette::SCENE_FIRE, 0.6),
    ("NO GROUND BELOW", 164.0, palette::IRON, 2.0),
];
const FORGE_SIGNS: [Sign; 2] = [
    ("[ FORGE CORE // 03 ]", 188.0, palette::SCENE_FIRE, 1.1),
    ("PRESSURE AT MAXIMUM", 164.0, palette::SCENE_RED, 2.4),
];
const ACID_SIGNS: [Sign; 2] = [
    ("[ ACID WORKS // 01 ]", 188.0, palette::SCENE_TOXIC, 0.4),
    ("CORROSIVE // KEEP MOVING", 164.0, palette::IRON, 2.1),
];
const REACTOR_SIGNS: [Sign; 2] = [
    ("[ REACTOR 02 // ONLINE ]", 188.0, palette::SCENE_BLUE, 0.9),
    ("CORE LOAD UNSTABLE", 164.0, palette::IRON, 2.5),
];
const DRAIN_SIGNS: [Sign; 2] = [
    ("[ DRAINAGE // LEVEL -3 ]", 188.0, palette::SCENE_TOXIC, 1.4),
    ("WATER NOT SAFE", 164.0, palette::SCENE_BLUE, 2.8),
];
const RED_GATE_SIGNS: [Sign; 2] = [
    ("[ RED GATE // ╬╪╫ ]", 188.0, palette::SCENE_RED, 0.8),
    ("ENTER WITH RESPECT", 164.0, palette::IRON, 2.6),
];
const PAGODA_SIGNS: [Sign; 2] = [
    ("[ SUNSET PAGODA ]", 188.0, palette::SCENE_GOLD, 1.2),
    ("FIVE ROOFS // ONE WINNER", 164.0, palette::SCENE_RED, 2.1),
];
const DRAGON_SIGNS: [Sign; 2] = [
    ("[ STONE DRAGON GARDEN ]", 188.0, palette::SCENE_TOXIC, 1.6),
    ("WAKE NOTHING", 164.0, palette::SCENE_GOLD, 2.9),
];

