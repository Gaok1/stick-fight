/// Uma boca que solta fumaca.
///
/// A chamine da fabrica e a cratera do vulcao sao a mesma coisa com numeros
/// diferentes: uma so fumega, a outra tambem explode de tempos em tempos. Como e
/// um componente so, a fumaca das duas envelhece pela mesma regra -- enquanto
/// foram dois sistemas, so a que alguem estava olhando ficava bonita.
#[derive(Component)]
struct Vent {
    /// Intervalo entre baforadas em repouso.
    puff: Timer,
    /// Fumaca quente de rocha derretida, ou vapor de agua.
    hot: bool,
    /// Forca da coluna: quanto a baforada sobe e quanto ela dura.
    power: f32,
    /// Segundos restantes de erupcao.
    blast: f32,
    /// Conta para a proxima erupcao. `None` numa chamine, que so fumega.
    erupt: Option<Timer>,
    /// Tom aceso agora, para o brilho nao ser reconstruido a cada quadro.
    tone: usize,
}

/// Quanto tempo a boca cospe depois de estourar.
const BLAST_TIME: f32 = 3.4;
/// Quantas vezes ela cospe mais rapido enquanto isso.
const BLAST_RUSH: f32 = 5.0;

/// Tons da cratera, do descanso ao estouro.
const HEAT: [Color; 3] = [palette::SCENE_RED, palette::SCENE_FIRE, palette::SCENE_GOLD];

/// Uma baforada, do nascimento a dissipacao.
const PUFF: [&str; 6] = [
    "∙",
    "░▒░",
    "▒▓▒\n░▒░",
    " ▒▓▓▒\n▒▓██▓\n ░▒▒░",
    "░▒▓▓▒░\n▒▓▓██▓▒\n ░▒▓▒░",
    "░ ▒ ░ \n ░ ▒ ░\n░  ░  ",
];

/// Cor de cada quadro da fumaca quente: sai brasa e vira cinza.
const SOOT: [Color; 6] = [
    palette::SCENE_FIRE,
    palette::SCENE_RED,
    palette::IRON,
    palette::IRON,
    palette::COAL,
    palette::COAL,
];
/// Cor de cada quadro do vapor.
const STEAM: [Color; 6] = [
    palette::ASH,
    palette::ASH,
    palette::ASH,
    palette::IRON,
    palette::IRON,
    palette::COAL,
];

/// Uma baforada solta no ar.
#[derive(Component)]
struct Smoke {
    age: f32,
    life: f32,
    rise: f32,
    drift: f32,
    frame: usize,
    hot: bool,
}

/// Pedra derretida cuspida pela cratera. Vive no plano do fundo: nao colide com
/// nada, so risca o ceu e apaga.
#[derive(Component)]
struct LavaBomb {
    velocity: Vec2,
    trail: Timer,
}

/// Gravidade das bombas. Menor que a do jogo de proposito -- o arco tem que
/// durar o suficiente para ser visto do outro lado da arena.
const BOMB_GRAVITY: f32 = 300.0;

fn vents(commands: &mut Commands, scene: Scene) {
    match scene {
        Scene::Caldera => {
            commands.spawn((
                LevelGeometry,
                Parallax {
                    home: CRATER,
                    depth: MID,
                },
                Vent {
                    puff: Timer::from_seconds(0.42, TimerMode::Repeating),
                    hot: true,
                    power: 1.0,
                    blast: 0.0,
                    erupt: Some(Timer::from_seconds(11.0, TimerMode::Repeating)),
                    tone: 1,
                },
                AsciiSprite::new(AsciiArt::solid(CRATER_GLOW, HEAT[1])),
                Layer::Background,
                Transform::from_translation(CRATER.extend(-MID)),
            ));
        }
        Scene::MagmaBridge => {
            // No pe das duas quedas: onde o magma bate, ele fumega.
            for x in [-420.0, 420.0] {
                let at = Vec2::new(x, GROUND + 20.0);
                commands.spawn((
                    LevelGeometry,
                    Parallax {
                        home: at,
                        depth: MID,
                    },
                    Vent {
                        puff: Timer::from_seconds(0.9, TimerMode::Repeating),
                        hot: true,
                        power: 0.55,
                        blast: 0.0,
                        erupt: None,
                        tone: 0,
                    },
                    Transform::from_translation(at.extend(-MID)),
                ));
            }
        }
        Scene::ForgeCore => {
            // A boca do alto-forno respira mesmo parada.
            let at = Vec2::new(-330.0, GROUND + 190.0);
            commands.spawn((
                LevelGeometry,
                Parallax {
                    home: at,
                    depth: MID,
                },
                Vent {
                    puff: Timer::from_seconds(0.58, TimerMode::Repeating),
                    hot: true,
                    power: 0.72,
                    blast: 0.0,
                    erupt: None,
                    tone: 0,
                },
                Transform::from_translation(at.extend(-MID)),
            ));
        }
        Scene::AcidWorks => {
            // Em cima de cada tanque, que e de onde a chamine sai.
            for (i, (x, rows)) in STACKS.into_iter().enumerate() {
                let at = Vec2::new(x, stack_top(rows));
                commands.spawn((
                    LevelGeometry,
                    Parallax {
                        home: at,
                        depth: MID,
                    },
                    Vent {
                        puff: Timer::from_seconds(0.7 + i as f32 * 0.19, TimerMode::Repeating),
                        hot: false,
                        power: 0.62,
                        blast: 0.0,
                        erupt: None,
                        tone: 0,
                    },
                    Transform::from_translation(at.extend(-MID)),
                ));
            }
        }
        Scene::Reactor => {
            for x in [-190.0, 190.0] {
                let at = Vec2::new(x, GROUND + 180.0);
                commands.spawn((
                    LevelGeometry,
                    Parallax {
                        home: at,
                        depth: FAR,
                    },
                    Vent {
                        puff: Timer::from_seconds(1.15, TimerMode::Repeating),
                        hot: false,
                        power: 0.48,
                        blast: 0.0,
                        erupt: None,
                        tone: 0,
                    },
                    Transform::from_translation(at.extend(-FAR)),
                ));
            }
        }
        Scene::Drainage => {
            // Vapor subindo de onde as bocas despejam na calha.
            for (i, x) in [-240.0, 240.0].into_iter().enumerate() {
                let at = Vec2::new(x, GROUND + 10.0);
                commands.spawn((
                    LevelGeometry,
                    Parallax {
                        home: at,
                        depth: NEAR,
                    },
                    Vent {
                        puff: Timer::from_seconds(0.8 + i as f32 * 0.23, TimerMode::Repeating),
                        hot: false,
                        power: 0.5,
                        blast: 0.0,
                        erupt: None,
                        tone: 0,
                    },
                    Transform::from_translation(at.extend(-NEAR)),
                ));
            }
        }
        Scene::City | Scene::RedGate | Scene::SunsetPagoda | Scene::DragonGarden => {}
    }
}

fn puff(commands: &mut Commands, at: Vec2, depth: f32, power: f32, hot: bool) {
    let life = (1.9 + fastrand::f32() * 1.4) * power.max(0.4);
    commands.spawn((
        LevelGeometry,
        Parallax { home: at, depth },
        Smoke {
            age: 0.0,
            life,
            rise: (34.0 + fastrand::f32() * 42.0) * power,
            drift: (fastrand::f32() - 0.35) * 26.0,
            frame: 0,
            hot,
        },
        AsciiSprite::new(AsciiArt::solid(
            PUFF[0],
            if hot { SOOT[0] } else { STEAM[0] },
        )),
        Layer::Background,
        Transform::from_translation(at.extend(-depth)),
    ));
}

/// Toca as bocas: baforada, contagem para a erupcao e o brilho da cratera.
fn run_vents(
    time: Res<Time>,
    mut commands: Commands,
    mut shake: MessageWriter<Shake>,
    mut vents: Query<(&mut Vent, &Parallax, Option<&mut AsciiSprite>)>,
) {
    let delta = time.delta();
    let dt = time.delta_secs();
    let now = time.elapsed_secs();

    for (mut vent, plane, glow) in &mut vents {
        vent.blast = (vent.blast - dt).max(0.0);

        // --- a erupcao ---
        let mut warning = 0.0;
        if let Some(clock) = vent.erupt.as_mut() {
            warning = clock.fraction().powi(6);
            if clock.tick(delta).just_finished() {
                vent.blast = BLAST_TIME;
                shake.write(Shake(0.55));
                blast(&mut commands, plane.home, plane.depth);
            }
        }

        // --- a fumaca ---
        let erupting = vent.blast > 0.0;
        let rush = if erupting { BLAST_RUSH } else { 1.0 };
        if vent.puff.tick(delta.mul_f32(rush)).just_finished() {
            let force = vent.power * if erupting { 2.6 } else { 1.0 };
            let spread = if erupting { 46.0 } else { 12.0 };
            let from = plane.home + Vec2::new((fastrand::f32() - 0.5) * spread, 8.0);
            puff(&mut commands, from, plane.depth, force, vent.hot);
        }

        // --- o brilho ---
        let Some(mut sprite) = glow else {
            continue;
        };
        let pulse = (now * 2.6).sin() * 0.5 + 0.5;
        let heat = (warning + vent.blast.min(1.0) + pulse * 0.35).min(1.0);
        let tone = ((heat * HEAT.len() as f32) as usize).min(HEAT.len() - 1);
        if tone != vent.tone {
            vent.tone = tone;
            sprite.art = sprite.art.recolored(HEAT[tone]);
        }
    }
}

/// O estouro: clarao na boca e um leque de bombas de lava.
fn blast(commands: &mut Commands, crater: Vec2, depth: f32) {
    commands.spawn((
        LevelGeometry,
        Parallax {
            home: crater,
            depth,
        },
        AsciiSprite::new(AsciiArt::solid(BLAST_FLASH, palette::MAGMA)),
        Layer::Background,
        Transform::from_translation(crater.extend(-depth)),
        Lifetime(Timer::from_seconds(0.22, TimerMode::Once)),
    ));

    for i in 0..9 {
        // Leque aberto em torno da vertical: nenhuma bomba sai rasante, senao
        // ela cruza a arena na altura da briga e vira ruido no meio do jogo.
        let angle = std::f32::consts::FRAC_PI_2 + (i as f32 / 8.0 - 0.5) * 1.7;
        let speed = 190.0 + fastrand::f32() * 170.0;
        let at = crater + Vec2::new((fastrand::f32() - 0.5) * 30.0, 10.0);
        commands.spawn((
            LevelGeometry,
            Parallax { home: at, depth },
            LavaBomb {
                velocity: Vec2::from_angle(angle) * speed,
                trail: Timer::from_seconds(0.06, TimerMode::Repeating),
            },
            AsciiSprite::new(AsciiArt::glyph(
                if i % 2 == 0 { '*' } else { '°' },
                palette::MAGMA,
            )),
            Layer::Background,
            Transform::from_translation(at.extend(-depth)),
        ));
    }
}

/// Envelhece a fumaca: ela sobe perdendo forca, incha e apaga.
fn drift_smoke(
    time: Res<Time>,
    mut commands: Commands,
    mut puffs: Query<(Entity, &mut Smoke, &mut Parallax, &mut AsciiSprite)>,
) {
    let dt = time.delta_secs();
    for (entity, mut smoke, mut plane, mut sprite) in &mut puffs {
        smoke.age += dt;
        if smoke.age >= smoke.life {
            commands.entity(entity).despawn();
            continue;
        }
        plane.home.y += smoke.rise * dt;
        plane.home.x += smoke.drift * dt;
        // Fumaca que esfria perde o empuxo e passa a so vagar com o vento.
        smoke.rise *= 1.0 - dt * 0.6;
        smoke.drift += dt * 9.0;

        let frame = ((smoke.age / smoke.life * PUFF.len() as f32) as usize).min(PUFF.len() - 1);
        if frame != smoke.frame {
            smoke.frame = frame;
            let tone = if smoke.hot { SOOT[frame] } else { STEAM[frame] };
            sprite.art = AsciiArt::solid(PUFF[frame], tone);
        }
    }
}

/// Voa as bombas de lava e deixa rastro de brasa.
fn fly_bombs(
    time: Res<Time>,
    mut commands: Commands,
    mut bombs: Query<(Entity, &mut LavaBomb, &mut Parallax)>,
) {
    let dt = time.delta_secs();
    for (entity, mut bomb, mut plane) in &mut bombs {
        bomb.velocity.y -= BOMB_GRAVITY * dt;
        plane.home += bomb.velocity * dt;

        if plane.home.y < -ARENA_HALF_H + 40.0 || plane.home.x.abs() > ARENA_HALF_W + 100.0 {
            commands.entity(entity).despawn();
            continue;
        }
        if bomb.trail.tick(time.delta()).just_finished() {
            ember(&mut commands, plane.home, plane.depth, palette::EMBER);
        }
    }
}

/// Uma faisca parada que apaga sozinha.
///
/// Rastro de bomba de lava, respingo do malho, sopro do dragao e chuva da
/// purga sao a mesma particula com outra cor -- e por isso a cor e parametro.
fn ember(commands: &mut Commands, at: Vec2, depth: f32, color: Color) {
    commands.spawn((
        LevelGeometry,
        Parallax { home: at, depth },
        AsciiSprite::new(AsciiArt::glyph(
            if fastrand::bool() { '·' } else { '°' },
            color,
        )),
        Layer::Background,
        Transform::from_translation(at.extend(-depth)),
        Lifetime(Timer::from_seconds(0.55, TimerMode::Once)),
    ));
}

const JADE_FLAME_FRAMES: [(char, Color); 4] = [
    ('\u{25b2}', palette::JADE),
    ('\u{2666}', palette::JADE),
    ('*', palette::SCENE_JADE_LIT),
    ('\u{00b7}', palette::SCENE_JADE),
];

/// Solta uma lingua estreita na boca. O triangulo aponta pela tangente do jato
/// e e esticado como uma pincelada, em vez de continuar preso a celula 8x16.
///
/// A mira e parametro porque a boca anda: o mesmo jato serve para o sopro que
/// varre a pista na rasante e para o fiozinho de vapor que sai do focinho
/// quando o bicho so esta respirando -- muda a direcao e a pressa, nao o fogo.
fn jade_flame(commands: &mut Commands, at: Vec2, aim: Vec2, speed: f32, depth: f32) {
    let normal = Vec2::new(-aim.y, aim.x);
    let direction = (aim + normal * (fastrand::f32() - 0.5) * 0.30).normalize_or_zero();
    let life = 0.55 + fastrand::f32() * 0.30;
    let start = at + normal * (fastrand::f32() - 0.5) * 10.0;
    let mut transform = Transform::from_translation(start.extend(-depth));
    transform.rotation = Quat::from_rotation_z(direction.to_angle() - std::f32::consts::FRAC_PI_2);
    transform.scale = Vec3::new(0.88, 2.10, 1.0);

    commands.spawn((
        LevelGeometry,
        Parallax { home: start, depth },
        JadeFlame {
            velocity: direction * speed,
            age: 0.0,
            life,
            curl: (fastrand::f32() - 0.5) * 75.0,
            frame: 0,
        },
        AsciiSprite::new(AsciiArt::glyph(
            JADE_FLAME_FRAMES[0].0,
            JADE_FLAME_FRAMES[0].1,
        )),
        Layer::Background,
        transform,
    ));
}

/// Faz o jato acelerar para fora, subir nas pontas e dissolver do nucleo claro
/// para uma brasa pequena. A curva quebra o cone liquido que lembrava acido.
fn fly_jade_flames(
    time: Res<Time>,
    mut commands: Commands,
    mut flames: Query<(
        Entity,
        &mut JadeFlame,
        &mut Parallax,
        &mut AsciiSprite,
        &mut Transform,
    )>,
) {
    let dt = time.delta_secs();
    for (entity, mut flame, mut plane, mut sprite, mut transform) in &mut flames {
        flame.age += dt;
        if flame.age >= flame.life {
            commands.entity(entity).despawn();
            continue;
        }
        // Fogo que chega no chao nao atravessa o chao: ele estoura ali e vira
        // brasa. Sem isto o sopro da rasante desaparece por baixo do terreno e
        // a passada inteira perde o unico quadro que prova que ela acertou
        // alguma coisa.
        if plane.home.y <= GROUND + 10.0 {
            ember(&mut commands, plane.home, plane.depth, palette::JADE);
            commands.entity(entity).despawn();
            continue;
        }

        let life = flame.age / flame.life;
        let normal = Vec2::new(-flame.velocity.y, flame.velocity.x).normalize_or_zero();
        let curl = flame.curl;
        flame.velocity +=
            (Vec2::Y * 105.0 + normal * curl * (life * std::f32::consts::PI).sin()) * dt;
        flame.velocity *= 1.0 - dt * 0.34;
        plane.home += flame.velocity * dt;

        let frame =
            ((life * JADE_FLAME_FRAMES.len() as f32) as usize).min(JADE_FLAME_FRAMES.len() - 1);
        if frame != flame.frame {
            flame.frame = frame;
            let (glyph, color) = JADE_FLAME_FRAMES[frame];
            sprite.art = AsciiArt::glyph(glyph, color);
        }

        transform.rotation =
            Quat::from_rotation_z(flame.velocity.to_angle() - std::f32::consts::FRAC_PI_2);
        transform.scale = Vec3::new(0.88 - life * 0.36, 2.10 + life * 1.15, 1.0);
    }
}

// --- letreiros --------------------------------------------------------------

