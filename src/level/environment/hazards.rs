const LINK_LENGTH: f32 = 12.5;

/// Onde um ciclo esta agora, em `[0, 1)`.
///
/// Todo perigo com hora marcada sai daqui, e nao de um `Timer` proprio: a
/// coluna que aparece e a zona que machuca sao entidades diferentes, e dois
/// relogios separados que comecam juntos terminam desencontrados depois de um
/// round inteiro -- o jorro aparece e nao fere, ou fere invisivel.
fn cycle(now: f32, period: f32, phase: f32) -> f32 {
    (now / period.max(0.01) + phase).rem_euclid(1.0)
}

/// A janela do ciclo em que um perigo esta armado.
#[derive(Clone, Copy)]
struct Beat {
    period: f32,
    phase: f32,
    /// Inicio e fim da janela, em fracao do ciclo.
    from: f32,
    to: f32,
}

impl Beat {
    fn open(self, now: f32) -> bool {
        let t = cycle(now, self.period, self.phase);
        t >= self.from && t < self.to
    }
}

/// Faixa de cenario que fere ao toque.
#[derive(Component)]
struct Hazard {
    kind: HazardKind,
    /// Quando ele morde. `None` e sempre -- poca parada, espinho.
    beat: Option<Beat>,
}

impl Hazard {
    fn always(kind: HazardKind) -> Self {
        Self { kind, beat: None }
    }

    fn armed(&self, now: f32) -> bool {
        self.beat.is_none_or(|beat| beat.open(now))
    }
}

#[derive(Component)]
struct HazardVisual {
    kind: HazardKind,
    cols: u16,
    phase: u8,
    bubbles: Timer,
}

/// Peca que sobe e desce sozinha: a mare, e a zona de contato dela.
#[derive(Component)]
struct Bob {
    home: f32,
    rise: f32,
    period: f32,
    phase: f32,
}

/// A coluna de uma fonte, que cresce e desaba com o ciclo.
#[derive(Component)]
struct Jet {
    kind: HazardKind,
    cols: u16,
    rows: u16,
    beat: Beat,
    /// Quadro desenhado agora, para a arte nao ser remontada a cada frame.
    drawn: u8,
    bubbles: Timer,
}

/// Boca que pinga.
#[derive(Component)]
struct Spout {
    kind: HazardKind,
    cols: u16,
    floor: f32,
    clock: Timer,
}

/// Uma gota no ar. Ela e um perigo que anda -- e por isso reaproveita a mesma
/// zona de contato da poca parada em vez de inventar uma regra propria.
#[derive(Component)]
struct Droplet {
    kind: HazardKind,
    floor: f32,
}

/// Impede que uma faixa de lava retire a barra inteira em um unico frame.
#[derive(Component)]
struct HazardCooldown(Timer);

/// Um bloco solido de terreno, com topo destacado da massa.
///
/// O topo em `ASH` e o miolo em `IRON` dao leitura de superficie sem precisar
/// de sombra ou segunda camada.
fn terrain(commands: &mut Commands, center: Vec2, cols: u16, rows: u16) {
    let mut art = AsciiArt::fill('\u{2588}', cols, 1, palette::ASH);
    if rows > 1 {
        art = art.stamp(
            &AsciiArt::fill('\u{2592}', cols, rows - 1, palette::IRON),
            0,
            1,
        );
    }
    let size = Vec2::new(cols as f32, rows as f32) * CELL;

    commands.spawn((
        LevelGeometry,
        AsciiSprite::new(art),
        Layer::Terrain,
        Transform::from_translation(center.extend(0.0)),
        Collider::size(size.x, size.y),
        Solid,
    ));
}

/// Plataforma fina, atravessavel por baixo.
fn platform(commands: &mut Commands, center: Vec2, cols: u16) {
    let art = AsciiArt::fill('\u{2550}', cols, 1, palette::ASH);

    commands.spawn((
        LevelGeometry,
        AsciiSprite::new(art),
        Layer::Terrain,
        Transform::from_translation(center.extend(0.0)),
        Collider::size(cols as f32 * CELL.x, CELL.y),
        Solid,
        OneWay,
    ));
}

fn hazard_art(kind: HazardKind, cols: u16, phase: u8) -> AsciiArt {
    match kind {
        HazardKind::Lava => {
            // Quatro temperaturas e rachaduras em movimento. O padrao muda
            // por coluna para a piscina fluir, em vez de apenas piscar inteira.
            let row = |choices: &[char], offset: u8, color| {
                let text: String = (0..cols)
                    .map(|col| choices[((col as u8 + phase + offset) as usize) % choices.len()])
                    .collect();
                AsciiArt::solid(&text, color)
            };
            row(&['≈', '~', '~', '≈', '_'], 0, palette::MAGMA)
                .stamp(&row(&['▒', '▓', '▒', '█'], 1, palette::EMBER), 0, 1)
                .stamp(&row(&['▓', '█', '▓', '▒'], 2, palette::BLOOD), 0, 2)
                .stamp(&row(&['█', '▓', '█', '▓'], 3, palette::COAL), 0, 3)
        }
        HazardKind::Acid => {
            let surface: String = (0..cols)
                .map(|col| match (col as u8 + phase) % 7 {
                    0 => '°',
                    3 => 'o',
                    _ => '≈',
                })
                .collect();
            AsciiArt::solid(&surface, palette::TOXIC)
                .stamp(&AsciiArt::fill('▒', cols, 1, palette::MOSS), 0, 1)
                .stamp(&AsciiArt::fill('▓', cols, 1, palette::SLUDGE), 0, 2)
        }
        HazardKind::Jade => {
            // Linguas, brasas e carvao de jade. Nao usa onda nem superficie:
            // esses glifos pertencem a liquido e faziam o fogo ler como acido.
            let tongue: String = (0..cols)
                .map(|col| match (col as u8 + phase) % 5 {
                    0 => '\u{25b2}',
                    2 => '^',
                    _ => ' ',
                })
                .collect();
            let embers: String = (0..cols)
                .map(|col| match (col as u8 * 3 + phase) % 4 {
                    0 => '\u{2666}',
                    1 => '*',
                    _ => '\u{2592}',
                })
                .collect();
            AsciiArt::solid(&tongue, palette::JADE)
                .stamp(&AsciiArt::solid(&embers, palette::SCENE_JADE_LIT), 0, 1)
                .stamp(
                    &AsciiArt::fill('\u{2593}', cols, 1, palette::SCENE_JADE),
                    0,
                    2,
                )
        }
        HazardKind::Spikes => AsciiArt::fill('▲', cols, 1, palette::ASH).stamp(
            &AsciiArt::fill('▄', cols, 1, palette::IRON),
            0,
            1,
        ),
    }
}

/// A coluna de uma fonte, com `rows` linhas de altura agora.
///
/// Ela afunila para cima e se desfaz na ponta. Uma coluna de largura constante
/// le como bloco subindo; e o afunilamento -- e o topo rasgado -- que faz o
/// olho ler pressao.
fn jet_art(kind: HazardKind, cols: u16, rows: u16, phase: u8) -> AsciiArt {
    if kind == HazardKind::Jade {
        return jade_fire_art(cols, rows, phase);
    }
    let (core, body, edge) = match kind {
        HazardKind::Lava => (palette::MAGMA, palette::EMBER, palette::BLOOD),
        HazardKind::Acid => (palette::TOXIC, palette::MOSS, palette::SLUDGE),
        HazardKind::Jade => unreachable!(),
        HazardKind::Spikes => (palette::BONE, palette::ASH, palette::IRON),
    };
    let middle = (cols as f32 - 1.0) * 0.5;

    let mut art = AsciiArt::default();
    for row in 0..rows {
        // 0 na ponta, 1 na base: e ele que abre a coluna para baixo.
        let down = (row + 1) as f32 / rows as f32;
        let half = 0.6 + down * middle;
        for col in 0..cols {
            let out = (col as f32 - middle).abs();
            if out > half {
                continue;
            }
            // A ponta chia em vez de terminar reta.
            let (glyph, color) = if row == 0 || (row == 1 && (col + phase as u16) % 2 == 0) {
                (
                    if (col + row + phase as u16).is_multiple_of(2) {
                        '*'
                    } else {
                        '°'
                    },
                    core,
                )
            } else if out < half * 0.45 {
                ('█', core)
            } else if out < half * 0.8 {
                ('▓', body)
            } else {
                ('▒', edge)
            };
            art = art.stamp(&AsciiArt::glyph(glyph, color), col, row);
        }
    }
    art
}

/// Fogo de jade em linguas sobrepostas. A silhueta serpenteia, abre buracos e
/// termina em pontas; uma piramide macica de blocos parece jato de liquido.
fn jade_fire_art(cols: u16, rows: u16, phase: u8) -> AsciiArt {
    let middle = (cols as f32 - 1.0) * 0.5;
    let mut art = AsciiArt::default();

    for row in 0..rows {
        let down = (row + 1) as f32 / rows as f32;
        let sway = ((row as f32 * 1.7 + phase as f32) * 1.3).sin() * (1.0 - down) * middle * 0.55;
        let half = 0.35 + down * middle;
        for col in 0..cols {
            let out = (col as f32 - middle - sway).abs();
            if out > half {
                continue;
            }

            let pattern = (col * 5 + row * 3 + phase as u16) % 11;
            if row > 1 && pattern == 0 && out > half * 0.28 {
                continue;
            }
            let (glyph, color) = if row <= 1 || (pattern == 1 && row < rows / 2) {
                ('\u{25b2}', palette::JADE)
            } else if out < half * 0.35 {
                ('\u{2588}', palette::JADE)
            } else if out < half * 0.72 {
                ('\u{2593}', palette::SCENE_JADE_LIT)
            } else {
                ('\u{2591}', palette::SCENE_JADE)
            };
            art = art.stamp(&AsciiArt::glyph(glyph, color), col, row);
        }
    }
    art
}

fn hazard(commands: &mut Commands, at: Vec2, cols: u16, kind: HazardKind) {
    // Arte e zona de contato sao entidades separadas: a piscina pode ter
    // profundidade visual sem transformar tres linhas inteiras em hitbox.
    commands.spawn((
        LevelGeometry,
        HazardVisual {
            kind,
            cols,
            phase: 0,
            bubbles: Timer::from_seconds(
                if kind == HazardKind::Spikes {
                    99.0
                } else {
                    0.18
                },
                TimerMode::Repeating,
            ),
        },
        AsciiSprite::pivoted(hazard_art(kind, cols, 0), Vec2::new(0.0, 0.5)),
        Layer::Terrain,
        // O topo da arte fica alinhado ao centro da faixa de contato.
        Transform::from_translation((at + Vec2::Y * CELL.y * 0.5).extend(0.0)),
    ));
    commands.spawn((
        LevelGeometry,
        Hazard::always(kind),
        Transform::from_translation(at.extend(0.0)),
        Collider::size(cols as f32 * CELL.x, CELL.y),
    ));
}

/// Fracao do ciclo que a fonte passa aberta.
///
/// Curta de proposito: uma fonte aberta metade do tempo e uma parede com
/// intervalo, e o mapa perde a rota em vez de ganhar ritmo.
const JET_WINDOW: (f32, f32) = (0.62, 0.86);
/// Onde, dentro da janela, a coluna esta na altura cheia.
const JET_HOLD: (f32, f32) = (0.18, 0.74);

/// Altura do jorro agora, de 0 a 1.
fn jet_rise(t: f32) -> f32 {
    let (from, to) = JET_WINDOW;
    if t < from || t >= to {
        return 0.0;
    }
    // Sobe rapido, segura, desaba. Sem o patamar o jorro e um piscar; sem a
    // subida rapida ele nao tem estouro.
    let (hold, drop) = JET_HOLD;
    match (t - from) / (to - from) {
        u if u < hold => u / hold,
        u if u < drop => 1.0,
        u => (1.0 - u) / (1.0 - drop),
    }
}

/// Quando a coluna morde.
///
/// Mais estreita que a janela do desenho, e nao igual a ela. A zona de contato
/// cobre a coluna inteira de uma vez -- encolhe-la junto com a arte custaria um
/// `Collider` reescrito por quadro. Armada na janela cheia, ela cobraria dano
/// no topo enquanto a coluna ainda esta na altura de um degrau: hitbox
/// invisivel. Assim o erro cai para o outro lado, que e o lado certo -- fogo
/// visivel que ainda nao machuca ensina; dano vindo do nada, nao.
fn jet_beat(period: f32, phase: f32) -> Beat {
    let (from, to) = JET_WINDOW;
    let span = to - from;
    Beat {
        period,
        phase,
        from: from + JET_HOLD.0 * span,
        to: from + JET_HOLD.1 * span,
    }
}

/// Uma fonte: coluna que aparece e some, e a zona que ela arma junto.
fn geyser(
    commands: &mut Commands,
    at: Vec2,
    cols: u16,
    rows: u16,
    period: f32,
    phase: f32,
    kind: HazardKind,
) {
    let beat = jet_beat(period, phase);
    let height = rows as f32 * CELL.y;

    commands.spawn((
        LevelGeometry,
        Jet {
            kind,
            cols,
            rows,
            beat,
            drawn: 0,
            bubbles: Timer::from_seconds(0.11, TimerMode::Repeating),
        },
        AsciiSprite::footed(AsciiArt::default()),
        Layer::Terrain,
        Transform::from_translation(at.extend(0.0)),
    ));
    // A zona cobre a coluna inteira e so morde na janela. Encolhe-la junto com
    // a arte custaria um `Collider` reescrito por frame para separar o pe do
    // topo de um jorro que dura um quarto de segundo.
    commands.spawn((
        LevelGeometry,
        Hazard {
            kind,
            beat: Some(beat),
        },
        Transform::from_translation((at + Vec2::Y * height * 0.5).extend(0.0)),
        Collider::size(cols as f32 * CELL.x, height),
    ));
}

/// Uma mare: a poca inteira sobe e desce, arte e zona de contato juntas.
fn tide(
    commands: &mut Commands,
    at: Vec2,
    cols: u16,
    rise: u16,
    period: f32,
    phase: f32,
    kind: HazardKind,
) {
    let swell = |home: f32| Bob {
        home,
        rise: rise as f32 * CELL.y,
        period,
        phase,
    };
    // A arte e alta o bastante para o corpo da poca continuar tapando o fundo
    // do tanque na mare cheia. Com a altura da poca parada, a mare sobe e
    // deixa um vao de nada entre a superficie e o piso.
    commands.spawn((
        LevelGeometry,
        HazardVisual {
            kind,
            cols,
            phase: 0,
            bubbles: Timer::from_seconds(0.14, TimerMode::Repeating),
        },
        swell(at.y + CELL.y * 0.5),
        AsciiSprite::pivoted(hazard_art(kind, cols, 0), Vec2::new(0.0, 0.5)),
        Layer::Terrain,
        Transform::from_translation((at + Vec2::Y * CELL.y * 0.5).extend(0.0)),
    ));
    commands.spawn((
        LevelGeometry,
        Hazard::always(kind),
        swell(at.y),
        Transform::from_translation(at.extend(0.0)),
        Collider::size(cols as f32 * CELL.x, CELL.y),
    ));
}

/// Uma goteira: a boca fica, as gotas nascem dela.
fn spout(
    commands: &mut Commands,
    from: Vec2,
    cols: u16,
    floor: f32,
    period: f32,
    phase: f32,
    kind: HazardKind,
) {
    let mut clock = Timer::from_seconds(period, TimerMode::Repeating);
    // Avanca o relogio no nascimento: sem isto, todas as bocas de um mapa
    // pingam no mesmo instante, e a chuva vira um metronomo.
    clock.set_elapsed(std::time::Duration::from_secs_f32(
        period * phase.rem_euclid(1.0),
    ));

    commands.spawn((
        LevelGeometry,
        Spout {
            kind,
            cols,
            floor,
            clock,
        },
        AsciiSprite::new(AsciiArt::fill('╥', cols, 1, palette::IRON)),
        Layer::Terrain,
        Transform::from_translation(from.extend(0.0)),
    ));
}

fn animate_hazards(
    time: Res<Time>,
    mut commands: Commands,
    mut hazards: Query<(&mut HazardVisual, &mut AsciiSprite, &Transform)>,
) {
    for (mut hazard, mut sprite, transform) in &mut hazards {
        let phase = (time.elapsed_secs() * 7.0).floor() as u8 & 3;
        if phase != hazard.phase {
            hazard.phase = phase;
            sprite.art = hazard_art(hazard.kind, hazard.cols, phase);
        }
        if hazard.kind == HazardKind::Spikes || !hazard.bubbles.tick(time.delta()).just_finished() {
            continue;
        }
        let width = hazard.cols as f32 * CELL.x;
        let count = if hazard.kind == HazardKind::Lava {
            2
        } else {
            1
        };
        for _ in 0..count {
            let x = transform.translation.x + (fastrand::f32() - 0.5) * (width - CELL.x);
            let lava = hazard.kind == HazardKind::Lava;
            commands.spawn((
                LevelGeometry,
                AsciiSprite::new(AsciiArt::glyph(
                    if lava && fastrand::bool() { '*' } else { '°' },
                    if lava { palette::MAGMA } else { palette::TOXIC },
                )),
                Layer::Fx,
                Transform::from_translation(Vec3::new(x, transform.translation.y - 2.0, 0.0)),
                Velocity(if lava {
                    Vec2::new(fastrand::f32() * 70.0 - 35.0, 55.0 + fastrand::f32() * 75.0)
                } else {
                    Vec2::new(fastrand::f32() * 16.0 - 8.0, 25.0 + fastrand::f32() * 20.0)
                }),
                Ghost,
                Lifetime(Timer::from_seconds(
                    if lava { 0.8 } else { 0.65 },
                    TimerMode::Once,
                )),
            ));
        }
    }
}

/// Faz a mare subir e descer -- arte e zona de contato pela mesma conta.
fn swell_tides(time: Res<Time>, mut tides: Query<(&Bob, &mut Transform)>) {
    let now = time.elapsed_secs();
    for (bob, mut transform) in &mut tides {
        // Cosseno, e nao dente de serra: a mare tem que demorar nos extremos e
        // passar rapido pelo meio, senao ela le como elevador.
        let wave = 0.5 - (cycle(now, bob.period, bob.phase) * std::f32::consts::TAU).cos() * 0.5;
        transform.translation.y = bob.home + bob.rise * wave;
    }
}

/// Toca as fontes: aviso, jorro e desabamento.
fn erupt_geysers(
    time: Res<Time>,
    mut commands: Commands,
    mut jets: Query<(&mut Jet, &mut AsciiSprite, &Transform)>,
) {
    let now = time.elapsed_secs();
    for (mut jet, mut sprite, transform) in &mut jets {
        let t = cycle(now, jet.beat.period, jet.beat.phase);
        let rows = (jet_rise(t) * jet.rows as f32).round() as u16;

        // A arte so e remontada quando a altura muda de linha: um jorro de
        // doze celulas reconstruido a sessenta quadros por segundo respawna
        // mais glifo por segundo que a briga inteira.
        let frame = rows as u8;
        if frame != jet.drawn {
            jet.drawn = frame;
            sprite.art = if rows == 0 {
                AsciiArt::default()
            } else {
                jet_art(jet.kind, jet.cols, rows, (now * 9.0) as u8 & 3)
            };
        }

        // O aviso: antes de abrir, a boca borbulha. Sem ele a fonte e uma
        // armadilha invisivel, e morrer para uma delas nao ensina nada. Ele
        // olha para a janela do desenho, e nao para a da zona de contato: o
        // que tem que ser anunciado e o instante em que a coluna aparece.
        let warning = t > JET_WINDOW.0 - 0.12 && t < JET_WINDOW.0;
        if !(warning || rows > 0) || !jet.bubbles.tick(time.delta()).just_finished() {
            continue;
        }
        let at = transform.translation.truncate();
        let width = jet.cols as f32 * CELL.x;
        let jade = jet.kind == HazardKind::Jade;
        let mut spark_at = Transform::from_translation(
            (at + Vec2::new((fastrand::f32() - 0.5) * width, rows as f32 * CELL.y * 0.9))
                .extend(0.0),
        );
        if jade && !warning {
            spark_at.scale = Vec3::new(0.72, 1.65, 1.0);
        }
        let mut spark = commands.spawn((
            LevelGeometry,
            AsciiSprite::new(AsciiArt::glyph(
                if warning {
                    '°'
                } else if jade {
                    '\u{25b2}'
                } else {
                    '*'
                },
                jet.kind.spray(),
            )),
            Layer::Fx,
            spark_at,
            Velocity(Vec2::new(
                (fastrand::f32() - 0.5) * if jade { 55.0 } else { 90.0 },
                if warning { 40.0 } else { 165.0 } + fastrand::f32() * 80.0,
            )),
            Ghost,
            Lifetime(Timer::from_seconds(0.6, TimerMode::Once)),
        ));
        if !jade {
            spark.insert(Falls);
        }
    }
}

/// Solta as gotas das bocas.
fn drip_spouts(
    time: Res<Time>,
    mut commands: Commands,
    mut spouts: Query<(&mut Spout, &Transform)>,
) {
    for (mut spout, transform) in &mut spouts {
        if !spout.clock.tick(time.delta()).just_finished() {
            continue;
        }
        let at = transform.translation.truncate();
        let x = at.x + (fastrand::f32() - 0.5) * spout.cols as f32 * CELL.x;
        commands.spawn((
            LevelGeometry,
            Droplet {
                kind: spout.kind,
                floor: spout.floor,
            },
            // Uma gota e um perigo que anda: reaproveitar a zona de contato da
            // poca parada e o que garante que ela ferve pela mesma regra, com
            // o mesmo tempo de invulnerabilidade depois.
            Hazard::always(spout.kind),
            Collider::size(CELL.x, CELL.y * 0.8),
            AsciiSprite::new(AsciiArt::glyph('\'', spout.kind.spray())),
            Layer::Projectile,
            Transform::from_translation(Vec3::new(x, at.y - CELL.y, 0.0)),
            Velocity(Vec2::new(0.0, -60.0)),
            Ghost,
            Falls,
        ));
    }
}

/// Desmancha a gota quando ela chega ao chao que a boca aponta.
fn splash_droplets(mut commands: Commands, drops: Query<(Entity, &Droplet, &Transform)>) {
    for (entity, drop, transform) in &drops {
        if transform.translation.y > drop.floor {
            continue;
        }
        let at = transform.translation.truncate();
        commands.entity(entity).despawn();
        for _ in 0..3 {
            commands.spawn((
                LevelGeometry,
                AsciiSprite::new(AsciiArt::glyph('·', drop.kind.spray())),
                Layer::Fx,
                Transform::from_translation(at.extend(0.0)),
                Velocity(Vec2::new(
                    fastrand::f32() * 110.0 - 55.0,
                    40.0 + fastrand::f32() * 60.0,
                )),
                Ghost,
                Falls,
                Lifetime(Timer::from_seconds(0.3, TimerMode::Once)),
            ));
        }
    }
}

fn hurt_on_hazards(
    time: Res<Time>,
    mut commands: Commands,
    mut damaged: MessageWriter<Damaged>,
    hazards: Query<(&Hazard, &Transform, &Collider)>,
    mut players: Query<
        (Entity, &Transform, &Collider, &mut Health, &mut Velocity),
        (With<Player>, Without<HazardCooldown>),
    >,
) {
    for (entity, transform, collider, mut health, mut velocity) in &mut players {
        if health.is_dead() {
            continue;
        }
        let body = collider.aabb(transform.translation.truncate());
        let now = time.elapsed_secs();
        let Some((hazard, _, _)) = hazards.iter().find(|(hazard, at, area)| {
            hazard.armed(now) && overlap(body, area.aabb(at.translation.truncate()))
        }) else {
            continue;
        };
        health.hp -= hazard.kind.damage();
        velocity.0 = hazard.kind.knockback(velocity.0);
        commands.entity(entity).insert((
            HazardCooldown(Timer::from_seconds(0.65, TimerMode::Once)),
            Stunned(Timer::from_seconds(0.22, TimerMode::Once)),
        ));
        damaged.write(Damaged {
            target: entity,
            amount: hazard.kind.damage(),
            at: transform.translation.truncate(),
            dir: Vec2::Y,
            move_name: "HAZARD",
            // Lava desmancha; espinho e acido so furam. Quem morre queimado
            // merece a mesma saida de quem morre num estouro.
            explosive: hazard.kind == HazardKind::Lava,
        });
    }
}

fn tick_hazard_cooldowns(
    time: Res<Time>,
    mut commands: Commands,
    mut players: Query<(Entity, &mut HazardCooldown)>,
) {
    for (entity, mut cooldown) in &mut players {
        if cooldown.0.tick(time.delta()).is_finished() {
            commands.entity(entity).remove::<HazardCooldown>();
        }
    }
}

