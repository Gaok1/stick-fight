/// O acontecimento de um cenario: o que ele faz de tempos em tempos.
///
/// O vulcao ja tinha o dele -- a erupcao -- e e ele que separa a Caldera dos
/// outros oito mapas. Sem um equivalente, cenario e pintura: bonito no
/// primeiro round e mudo no decimo.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Show {
    /// O malho recua, desce, bate e se recolhe.
    ForgeHammer,
    /// A fabrica purga: alarme, transbordo e chuva corrosiva.
    Purge,
}

impl Show {
    /// Quanto dura o numero, em segundos.
    const fn span(self) -> f32 {
        match self {
            Self::ForgeHammer => 1.15,
            Self::Purge => 2.0,
        }
    }
}

#[derive(Component)]
struct Landmark {
    show: Show,
    clock: Timer,
    /// Segundos restantes do numero em cena.
    live: f32,
    /// Onde a peca descansa -- ou, para quem nao anda, de onde ela age.
    rest: Vec2,
    /// O quadro de contato ja aconteceu neste numero?
    struck: bool,
}

/// Uma lingua do fogo de jade. O glifo e so a pincelada: velocidade, rotacao,
/// alongamento e troca de densidade e que fazem a chama ler como fogo.
#[derive(Component)]
struct JadeFlame {
    velocity: Vec2,
    age: f32,
    life: f32,
    curl: f32,
    frame: usize,
}

/// Toca os numeros dos cenarios.
fn run_shows(
    time: Res<Time>,
    mut commands: Commands,
    mut shake: MessageWriter<Shake>,
    mut marks: Query<(&mut Landmark, &mut Parallax)>,
) {
    let dt = time.delta_secs();
    for (mut mark, mut plane) in &mut marks {
        if mark.clock.tick(time.delta()).just_finished() {
            mark.live = mark.show.span();
            mark.struck = false;
            // O malho nao sacode ao comecar: o tranco dele e no contato, e ele
            // acontece no meio do numero.
            match mark.show {
                Show::Purge => {
                    shake.write(Shake(0.18));
                }
                Show::ForgeHammer => {}
            }
        }
        if mark.live <= 0.0 {
            continue;
        }
        mark.live = (mark.live - dt).max(0.0);
        let beat = 1.0 - mark.live / mark.show.span();

        match mark.show {
            Show::ForgeHammer => {
                // Antecipacao curta para cima, queda acelerada, pausa no
                // contato e recuperacao amortecida. Sem a antecipacao o malho
                // parece cair sozinho; sem a pausa, quicar.
                let drop = match beat {
                    b if b < 0.32 => -0.10 * (b / 0.32 * std::f32::consts::PI).sin(),
                    b if b < 0.44 => ((b - 0.32) / 0.12).powi(2),
                    b if b < 0.58 => 1.0,
                    b => (1.0 - (b - 0.58) / 0.42).max(0.0),
                };
                plane.home.y = mark.rest.y - HAMMER_DROP * drop;
                if !mark.struck && beat >= 0.44 {
                    mark.struck = true;
                    shake.write(Shake(0.24));
                    for i in 0..12 {
                        let spread = (i as f32 / 11.0 - 0.5) * 120.0;
                        ember(
                            &mut commands,
                            mark.rest + Vec2::new(spread, -HAMMER_DROP - 12.0),
                            plane.depth,
                            palette::EMBER,
                        );
                    }
                }
            }
            Show::Purge => {
                // Chuva corrosiva vindo do encanamento, de ponta a ponta.
                for _ in 0..2 {
                    let at = Vec2::new(
                        (fastrand::f32() - 0.5) * ARENA_HALF_W * 1.9,
                        mark.rest.y - fastrand::f32() * 90.0,
                    );
                    ember(&mut commands, at, plane.depth, palette::TOXIC);
                }
            }
        }
    }
}

/// Pendura o numero de cada cenario, quando ele tem um.
fn shows(commands: &mut Commands, scene: Scene) {
    let (show, rest, depth, every, art) = match scene {
        Scene::ForgeCore => (
            Show::ForgeHammer,
            HAMMER_AT,
            MID,
            4.6,
            Some(AsciiArt::tinted(HAMMER, &STEEL, palette::IRON)),
        ),
        Scene::AcidWorks => (Show::Purge, Vec2::new(0.0, 118.0), NEAR, 9.5, None),
        _ => return,
    };

    let mut entity = commands.spawn((
        LevelGeometry,
        Parallax { home: rest, depth },
        Landmark {
            show,
            clock: Timer::from_seconds(every, TimerMode::Repeating),
            live: 0.0,
            rest,
            struck: false,
        },
        Layer::Background,
        Transform::from_translation(rest.extend(-depth)),
    ));
    if let Some(art) = art {
        entity.insert(AsciiSprite::new(art));
    }
}

// --- o dragao de jade -------------------------------------------------------
//
// O resto deste arquivo desenha; esta secao cria um bicho. A diferenca nao e
// de grau: um painel sabe onde esta, e o dragao sabe onde esteve.
//
// Ele e uma cabeca que anda, um rastro que ela deixa e vinte e tantas
// vertebras que leem esse rastro atras dela. Nada aqui gira uma vertebra a
// mao, e por isso o corpo nunca abre buraco nem se dobra sozinho: se a cabeca
// desenhou a curva, o corpo passa exatamente por ela alguns metros depois.
//
// Foi essa inversao que resolveu o dragao. A versao anterior era escultura --
// duas artes paradas com dois pixels de sobe-e-desce -- e ninguem percebia que
// ela respirava, porque nao respirava: tremia.

