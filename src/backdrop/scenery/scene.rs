/// Letreiro de fundo que pulsa como neon velho.
#[derive(Component)]
struct NeonSign {
    bright: Color,
    dim: Color,
    speed: f32,
    phase: f32,
    lit: bool,
}

fn flicker_neon(time: Res<Time>, mut signs: Query<(&mut NeonSign, &mut AsciiSprite)>) {
    for (mut sign, mut sprite) in &mut signs {
        let wave = (time.elapsed_secs() * sign.speed + sign.phase).sin();
        let lit = wave > -0.82;
        if lit != sign.lit {
            sign.lit = lit;
            sprite.art = sprite
                .art
                .recolored(if lit { sign.bright } else { sign.dim });
        }
    }
}

// --- montagem ---------------------------------------------------------------

/// Predios do tema, com janelas acesas.
fn towers(commands: &mut Commands, skyline: &[Building], scene: Scene) {
    let window = match scene {
        Scene::City => palette::IRON,
        Scene::AcidWorks => palette::SCENE_TOXIC,
        Scene::Reactor => palette::SCENE_BLUE,
        _ => return,
    };

    // Os distantes nao tem janela acesa: e a falta dela, mais que o tamanho,
    // que os empurra para longe.
    if scene == Scene::City {
        for &(x, y, cols, rows) in &CITY_FAR {
            raise(
                commands,
                Panel::footed(
                    AsciiArt::fill('▓', cols, rows, palette::COAL),
                    Vec2::new(x, y + GROUND),
                    FAR,
                ),
            );
        }
    }

    // Os predios da fabrica recuam de plano: o patio de tanques ocupa o plano
    // medio inteiro, e duas massas na mesma profundidade disputam quem cobre
    // quem -- o que aparece vira sorteio da ordem de spawn.
    let depth = match scene {
        Scene::AcidWorks => FAR,
        Scene::Reactor => SKY,
        _ => MID,
    };
    for &(x, y, cols, rows) in skyline {
        let mut art = AsciiArt::fill('▓', cols, rows, palette::COAL);
        for row in (1..rows.saturating_sub(1)).step_by(2) {
            for col in (2..cols.saturating_sub(1)).step_by(4) {
                art = art.stamp(&AsciiArt::solid("·", window), col, row);
            }
        }
        raise(
            commands,
            Panel::footed(art, Vec2::new(x, y + GROUND), depth),
        );
    }
}

/// O que balanca no fundo do tema: brasa, faisca, lanterna.
fn ambience(commands: &mut Commands, scene: Scene) {
    match scene {
        Scene::City => {
            // Luz de aeronave piscando sobre a antena mais alta.
            for (i, at) in [Vec2::new(470.0, 190.0), Vec2::new(-250.0, 145.0)]
                .into_iter()
                .enumerate()
            {
                spark(
                    commands,
                    '°',
                    at,
                    palette::BLOOD,
                    Sway {
                        phase: i as f32 * 2.1,
                        speed: 1.4,
                        travel: Vec2::new(0.0, 3.0),
                    },
                    FAR,
                );
            }
        }
        Scene::Caldera => {
            // Brasas subindo em torno da montanha.
            for i in 0..16 {
                let hot = i % 3 == 0;
                spark(
                    commands,
                    if hot { '*' } else { '·' },
                    Vec2::new(-560.0 + i as f32 * 74.0, -30.0 + (i % 5) as f32 * 44.0),
                    if hot {
                        palette::SCENE_FIRE
                    } else {
                        palette::ASH
                    },
                    Sway {
                        phase: i as f32 * 0.73,
                        speed: 0.45 + (i % 4) as f32 * 0.08,
                        travel: Vec2::new(20.0, 14.0),
                    },
                    MID,
                );
            }
        }
        Scene::MagmaBridge | Scene::ForgeCore => {
            for i in 0..12 {
                spark(
                    commands,
                    if i % 4 == 0 { '*' } else { '·' },
                    Vec2::new(-500.0 + i as f32 * 92.0, -80.0 + (i % 4) as f32 * 48.0),
                    palette::SCENE_FIRE,
                    Sway {
                        phase: i as f32 * 0.61,
                        speed: 0.62,
                        travel: Vec2::new(14.0, 24.0),
                    },
                    MID,
                );
            }
        }
        Scene::AcidWorks | Scene::Reactor => {
            // Balizas de alerta pelo patio, cada uma no seu compasso: piscar
            // junto viraria pisca-pisca de natal.
            for (i, x) in [-430.0, -115.0, 215.0, 470.0].into_iter().enumerate() {
                spark(
                    commands,
                    if i % 2 == 0 { 'o' } else { '°' },
                    Vec2::new(x, 34.0 + i as f32 * 15.0),
                    palette::ASH,
                    Sway {
                        phase: i as f32 * 1.4,
                        speed: 0.7 + i as f32 * 0.08,
                        travel: Vec2::new(10.0, 18.0),
                    },
                    FAR,
                );
            }
            // Vazamento pingando dos tanques -- o aviso de que a fabrica vaza
            // muito antes de a purga acontecer.
            for (i, (x, _)) in STACKS.into_iter().enumerate() {
                spark(
                    commands,
                    '\'',
                    Vec2::new(x + 40.0, VAT_FOOT - 6.0),
                    palette::SCENE_ACID,
                    Sway {
                        phase: i as f32 * 2.3,
                        speed: 1.1,
                        travel: Vec2::new(1.0, 12.0),
                    },
                    MID,
                );
            }
        }
        Scene::Drainage => {
            for (i, x) in [-420.0, -210.0, 15.0, 250.0, 460.0].into_iter().enumerate() {
                spark(
                    commands,
                    if i % 2 == 0 { '·' } else { '\'' },
                    Vec2::new(x, 155.0 - (i % 3) as f32 * 22.0),
                    palette::SCENE_BLUE,
                    Sway {
                        phase: i as f32,
                        speed: 0.36,
                        travel: Vec2::new(2.0, 18.0),
                    },
                    MID,
                );
            }
        }
        Scene::RedGate | Scene::SunsetPagoda => {
            // Lanternas penduradas, cada uma com a propria brasa dentro.
            for (i, x) in [-260.0, -120.0, 255.0, 405.0].into_iter().enumerate() {
                let at = Vec2::new(x, 22.0 + (i % 2) as f32 * 18.0);
                raise(
                    commands,
                    Panel::new(AsciiArt::solid(LANTERN, palette::BLOOD), at, MID),
                );
                spark(
                    commands,
                    if i % 2 == 0 { '•' } else { '·' },
                    at,
                    palette::SCENE_GOLD,
                    Sway {
                        phase: i as f32,
                        speed: 0.55,
                        travel: Vec2::new(6.0, 4.0),
                    },
                    MID,
                );
            }
            // Bando de grous cruzando o poente, alto e devagar.
            if scene == Scene::SunsetPagoda {
                for (i, at) in [
                    Vec2::new(-120.0, 132.0),
                    Vec2::new(-62.0, 150.0),
                    Vec2::new(-8.0, 124.0),
                ]
                .into_iter()
                .enumerate()
                {
                    adrift(
                        commands,
                        AsciiArt::solid(CRANE, palette::SCENE_HAZE),
                        at,
                        Sway {
                            phase: i as f32 * 0.9,
                            speed: 0.22,
                            travel: Vec2::new(70.0, 9.0),
                        },
                        FAR,
                    );
                }
            }
        }
        Scene::DragonGarden => {
            // Vaga-lumes dourados no jardim.
            for (i, x) in [-480.0, -180.0, 120.0, 360.0, 520.0]
                .into_iter()
                .enumerate()
            {
                spark(
                    commands,
                    '·',
                    Vec2::new(x, -60.0 + (i % 3) as f32 * 45.0),
                    palette::SCENE_GOLD,
                    Sway {
                        phase: i as f32 * 1.7,
                        speed: 0.38,
                        travel: Vec2::new(22.0, 10.0),
                    },
                    FAR,
                );
            }
            // A escama solta que o bicho perdeu, boiando alto no jardim. Era
            // um brilho preso ao lombo enquanto o lombo ficava parado; agora
            // que ele voa, faisca ancorada em coordenada fixa ficaria pendurada
            // no vazio metade do tempo.
            for (i, at) in [Vec2::new(-250.0, 44.0), Vec2::new(300.0, 62.0)]
                .into_iter()
                .enumerate()
            {
                spark(
                    commands,
                    '•',
                    at,
                    palette::SCENE_JADE_LIT,
                    Sway {
                        phase: i as f32 * 2.4,
                        speed: 0.5,
                        travel: Vec2::new(9.0, 14.0),
                    },
                    MID,
                );
            }
        }
    }
}

/// Levanta o fundo inteiro do tema.
///
/// Um lugar so monta tudo, na ordem de tras para frente. As fases nao spawnam
/// fundo por conta propria, entao nao existe mapa com uma serra na frente do
/// jogador porque alguem escreveu o `z` errado.
pub fn build(commands: &mut Commands, skyline: &[Building], signs: &[Sign], scene: Scene) {
    let theme = scene.theme();
    for panel in panels(scene) {
        raise(commands, panel);
    }
    towers(commands, skyline, scene);
    ambience(commands, scene);
    vents(commands, scene);
    flows(commands, scene);
    shows(commands, scene);
    hatch_dragon(commands, scene);
    seed_weather(commands, &weather(theme));

    for &(text, y, bright, phase) in signs {
        let at = Vec2::new(0.0, y);
        commands.spawn((
            LevelGeometry,
            Parallax {
                home: at,
                depth: NEAR,
            },
            NeonSign {
                bright,
                dim: palette::IRON,
                speed: 3.0,
                phase,
                lit: true,
            },
            AsciiSprite::new(AsciiArt::solid(text, bright)),
            Layer::Background,
            Transform::from_translation(at.extend(-NEAR)),
        ));
    }
}

