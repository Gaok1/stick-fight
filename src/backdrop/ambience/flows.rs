/// Uma peca do fundo que troca de arte em ciclo.
///
/// Queda de magma, cascata de acido, nucleo do reator: sao todas a mesma
/// coisa, um punhado de quadros e um relogio. Enquanto fossem tres sistemas,
/// so a que alguem estivesse olhando ganharia conserto.
#[derive(Component)]
struct Flipbook {
    frames: Vec<AsciiArt>,
    clock: Timer,
    frame: usize,
}

fn turn_pages(time: Res<Time>, mut books: Query<(&mut Flipbook, &mut AsciiSprite)>) {
    for (mut book, mut sprite) in &mut books {
        if !book.clock.tick(time.delta()).just_finished() {
            continue;
        }
        book.frame = (book.frame + 1) % book.frames.len();
        sprite.art = book.frames[book.frame].clone();
    }
}

/// Uma peca animada do fundo, ja resolvida em quadros, lugar e ritmo.
///
/// Descrita antes de existir, como os [`Panel`]: o que nasce direto em
/// `Commands` nao passa por teste nenhum, e foi assim que a arte solta do fundo
/// -- justamente a que se mexe, e por isso a mais visivel -- ficou de fora da
/// varredura de CP437 e da de cores reservadas.
struct Reel {
    frames: Vec<AsciiArt>,
    at: Vec2,
    depth: f32,
    fps: f32,
    /// Apoiada na base, como quase tudo que escorre ate o chao.
    foot: bool,
}

/// Quatro quadros de uma queda de liquido.
fn falling(cols: u16, rows: u16, map: &'static [(char, Color)]) -> Vec<AsciiArt> {
    (0..4).map(|f| cascade(cols, rows, f, map)).collect()
}

/// O que escorre, pulsa ou transborda em cada cenario.
fn reels(scene: Scene) -> Vec<Reel> {
    let fall = |x: f32, depth: f32, fps: f32, frames: Vec<AsciiArt>| Reel {
        frames,
        at: Vec2::new(x, GROUND),
        depth,
        fps,
        foot: true,
    };
    match scene {
        // Duas quedas de magma descendo pela cara dos penhascos.
        Scene::MagmaBridge => [-420.0, 420.0]
            .into_iter()
            .map(|x| fall(x, MID, 7.0, falling(7, 22, &MAGMA_FALL)))
            .collect(),
        // Sangria do forno: o metal sai pela base e corre para o canal.
        Scene::ForgeCore => vec![fall(-240.0, MID, 8.0, falling(4, 6, &MAGMA_FALL))],
        // Transbordo entre os tanques -- e de onde a fabrica pinga.
        Scene::AcidWorks => [-95.0, 95.0]
            .into_iter()
            .map(|x| fall(x, MID, 6.0, falling(6, 12, &ACID_FALL)))
            .collect(),
        Scene::Reactor => [-300.0, 300.0]
            .into_iter()
            .map(|x| fall(x, NEAR, 6.0, falling(5, 19, &COOLANT_FALL)))
            .chain([Reel {
                // O nucleo respira: cresce, estoura e volta. Como o sprite e
                // centrado, o pulso sai do meio do anel sem mover o anel.
                frames: [4u16, 5, 6, 5]
                    .into_iter()
                    .map(|r| disc(r, false, &CORE_HEAT))
                    .collect(),
                at: CORE_AT,
                depth: MID,
                fps: 3.6,
                foot: false,
            }])
            .collect(),
        // As duas bocas de saida despejando na calha.
        Scene::Drainage => [-240.0, 240.0]
            .into_iter()
            .map(|x| fall(x, MID, 7.0, falling(8, 10, &ACID_FALL)))
            .collect(),
        // Tanque de agua do jardim, correndo devagar.
        Scene::DragonGarden => vec![fall(
            0.0,
            NEAR,
            3.0,
            (0..4).map(|f| current(40, 2, f, &JADE_FLOW)).collect(),
        )],
        Scene::City | Scene::Caldera | Scene::RedGate | Scene::SunsetPagoda => Vec::new(),
    }
}

fn flows(commands: &mut Commands, scene: Scene) {
    for reel in reels(scene) {
        let sprite = if reel.foot {
            AsciiSprite::footed(reel.frames[0].clone())
        } else {
            AsciiSprite::new(reel.frames[0].clone())
        };
        commands.spawn((
            LevelGeometry,
            Parallax {
                home: reel.at,
                depth: reel.depth,
            },
            sprite,
            Flipbook {
                clock: Timer::from_seconds(1.0 / reel.fps, TimerMode::Repeating),
                frames: reel.frames,
                frame: 0,
            },
            Layer::Background,
            Transform::from_translation(reel.at.extend(-reel.depth)),
        ));
    }
}

