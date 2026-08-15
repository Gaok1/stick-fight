macro_rules! stage {
    ($name:literal, $scene:expr, $spawns:expr, $pieces:expr, $sky:expr, $signs:expr) => {
        StageDef {
            name: $name,
            scene: $scene,
            spawns: $spawns,
            drops: &DROPS,
            pieces: $pieces,
            skyline: $sky,
            signs: $signs,
        }
    };
}

const CALDERA: StageDef = stage!(
    "LAVA 01 - CALDERA",
    Scene::Caldera,
    &SPAWNS_WIDE,
    &LAVA_1,
    &VOLCANO_SKY,
    &CALDERA_SIGNS
);
const MAGMA_BRIDGE: StageDef = stage!(
    "LAVA 02 - MAGMA BRIDGE",
    Scene::MagmaBridge,
    &SPAWNS_CHASM,
    &LAVA_2,
    &VOLCANO_SKY,
    &BRIDGE_SIGNS
);
const FORGE_CORE: StageDef = stage!(
    "LAVA 03 - FORGE CORE",
    Scene::ForgeCore,
    &SPAWNS_WIDE,
    &LAVA_3,
    &VOLCANO_SKY,
    &FORGE_SIGNS
);
const ACID_WORKS: StageDef = stage!(
    "INDUSTRIAL 01 - ACID WORKS",
    Scene::AcidWorks,
    &SPAWNS_YARD,
    &ACID_1,
    &INDUSTRIAL_SKY,
    &ACID_SIGNS
);
const REACTOR: StageDef = stage!(
    "INDUSTRIAL 02 - REACTOR",
    Scene::Reactor,
    &SPAWNS_INNER,
    &ACID_2,
    &INDUSTRIAL_SKY,
    &REACTOR_SIGNS
);
const DRAINAGE: StageDef = stage!(
    "INDUSTRIAL 03 - DRAINAGE",
    Scene::Drainage,
    &SPAWNS_WIDE,
    &ACID_3,
    &INDUSTRIAL_SKY,
    &DRAIN_SIGNS
);
const RED_GATE: StageDef = stage!(
    "ORIENTAL 01 - RED GATE",
    Scene::RedGate,
    &SPAWNS_WIDE,
    &EAST_1,
    &ORIENTAL_SKY,
    &RED_GATE_SIGNS
);
const SUNSET_PAGODA: StageDef = stage!(
    "ORIENTAL 02 - SUNSET PAGODA",
    Scene::SunsetPagoda,
    &SPAWNS_INNER,
    &EAST_2,
    &ORIENTAL_SKY,
    &PAGODA_SIGNS
);
const DRAGON_GARDEN: StageDef = stage!(
    "ORIENTAL 03 - DRAGON GARDEN",
    Scene::DragonGarden,
    &SPAWNS_INNER,
    &EAST_3,
    &ORIENTAL_SKY,
    &DRAGON_SIGNS
);

/// Catalogo de fases, na ordem em que o menu as lista.
///
/// Adicionar um mapa e escrever o `Level` e por o construtor aqui -- nenhum
/// outro arquivo precisa saber que ele existe.
pub const CATALOG: [fn() -> Box<dyn Level>; 12] = [
    || Box::new(Arena01),
    || Box::new(Arena02),
    || Box::new(Arena03),
    || Box::new(ThemedArena(&CALDERA)),
    || Box::new(ThemedArena(&MAGMA_BRIDGE)),
    || Box::new(ThemedArena(&FORGE_CORE)),
    || Box::new(ThemedArena(&ACID_WORKS)),
    || Box::new(ThemedArena(&REACTOR)),
    || Box::new(ThemedArena(&DRAINAGE)),
    || Box::new(ThemedArena(&RED_GATE)),
    || Box::new(ThemedArena(&SUNSET_PAGODA)),
    || Box::new(ThemedArena(&DRAGON_GARDEN)),
];

/// Constroi a fase de indice `index`, girando a lista se ele passar do fim.
pub fn level_at(index: usize) -> Box<dyn Level> {
    CATALOG[index % CATALOG.len()]()
}

/// Nome da fase de indice `index`, para o menu.
pub fn level_name(index: usize) -> &'static str {
    level_at(index).name()
}

