/// Coice em andamento: enquanto dura, a arma recua e o cano fica pro alto.
#[derive(Component)]
pub struct Recoiling {
    timer: Timer,
    kick: f32,
}

/// Aplica o coice a mira visual. Bracos e arma consomem o mesmo vetor para o
/// cano nunca subir sozinho.
pub fn animated_aim(
    intent: &Intent,
    facing: &Facing,
    recoiling: Option<&Recoiling>,
) -> (Vec2, f32) {
    let recoil = recoiling.map_or(0.0, |r| r.kick * (1.0 - r.timer.fraction()));
    let aim = aim_dir(intent, facing);
    let local = Vec2::new(aim.x * facing.0, aim.y);
    let kicked = Vec2::from_angle(recoil).rotate(local);
    (Vec2::new(kicked.x * facing.0, kicked.y), recoil)
}

/// Reticulo desenhado sobre o cursor.
#[derive(Component)]
struct Crosshair;

/// Projetil no ar.
#[derive(Component)]
pub struct Projectile;

#[derive(Component)]
struct Trail {
    timer: Timer,
    color: Color,
    look: WeaponLook,
}

#[derive(Component)]
struct TumblingShot(f32);

#[derive(Component)]
struct PickupPulse(f32);

/// Poeira viva que denuncia o grimorio antes mesmo de ele ser pego.
#[derive(Component)]
struct BookAura {
    clock: Timer,
    step: usize,
    held: bool,
}

/// Uma lingua de magia: nasce como sigilo, curva no ar e termina em poeira.
#[derive(Component)]
struct ArcaneFlame {
    velocity: Vec2,
    age: f32,
    life: f32,
    curl: f32,
    frame: usize,
    size: f32,
}

/// Agenda o proximo drop.
#[derive(Resource)]
struct DropSchedule(Timer);

/// Comeca o cronometro de drops ao entrar na luta.
fn arm_drop_schedule(mut commands: Commands) {
    commands.insert_resource(DropSchedule(Timer::from_seconds(
        FIRST_DROP_DELAY,
        TimerMode::Repeating,
    )));
}

/// Quantas armas o cenario aguenta com `players` lutando.
///
/// Uma a mais que o numero de lutadores. E a menor folga que ainda deixa a luta
/// ser uma disputa: com uma arma por cabeca ninguem precisa correr para o chao,
/// e com duas a mais sempre sobra uma no canto sem dono. Um a mais e exatamente
/// uma arma em disputa.
///
/// Sala vazia continua tendo orcamento 1: o primeiro drop cai antes de qualquer
/// boneco entrar em fases de teste, e um teto zero prenderia a fase sem arma
/// nenhuma.
fn weapon_budget(players: usize) -> usize {
    players + 1
}

/// Armas que ja existem na fase: as caidas mais as que estao em maos.
///
/// As duas contam. Contar so o chao larga arma nova a cada sete segundos
/// enquanto todo mundo ja esta armado, que e como a fase entope: as armas somem
/// do chao para as maos, o contador zera e o cronometro repoe.
fn armed_arena(fighters: &Query<Has<Held>, With<Player>>, loose: &Query<(), With<GroundWeapon>>) -> usize {
    loose.iter().count() + fighters.iter().filter(|held| *held).count()
}

/// Larga uma arma aleatoria num dos pontos da fase.
///
/// So a autoridade sorteia. Enquanto isto rodava em toda maquina, cada uma
/// tirava um ponto e uma arma diferentes do proprio `fastrand`: o dono via uma
/// espingarda na plataforma, o cliente via uma katana no fosso, e encostar em
/// qualquer uma das duas dava o resultado errado do outro lado -- era dai que
/// saiam a arma invisivel e a arma que ninguem conseguia pegar.
fn drop_weapons(
    time: Res<Time>,
    mut commands: Commands,
    mut schedule: ResMut<DropSchedule>,
    mut ids: ResMut<NextWeaponId>,
    level: Res<CurrentLevel>,
    fighters: Query<Has<Held>, With<Player>>,
    loose: Query<(), With<GroundWeapon>>,
) {
    if !schedule.0.tick(time.delta()).just_finished() {
        return;
    }
    // Depois do primeiro, o ritmo passa a ser o normal.
    schedule
        .0
        .set_duration(std::time::Duration::from_secs_f32(DROP_INTERVAL));

    // O cronometro so pergunta *quando*; quem responde *se* e a conta de armas.
    // Enquanto o relogio sozinho mandava, uma fase longa acabava com uma arma no
    // chao a cada sete segundos ate o cenario virar um deposito -- e ninguem
    // precisava pegar nenhuma, porque sempre havia outra a um passo.
    if armed_arena(&fighters, &loose) >= weapon_budget(fighters.iter().count()) {
        return;
    }

    let points = level.0.drop_points();
    if points.is_empty() {
        return;
    }
    let at = points[fastrand::usize(..points.len())];
    let kind = random_kind();
    let ammo = weapon_at(kind).ammo();
    ids.0 = ids.0.wrapping_add(1);

    spawn_ground_weapon(&mut commands, Some(ids.0), kind, ammo, at, Vec2::ZERO);
}

/// Arma que caiu no vao para de existir.
///
/// Nada mais a apagava: o colisor dela nao encosta em nada la embaixo, entao
/// ela caia para sempre, invisivel e impossivel de pegar. Online era pior --
/// `broadcast_weapons` continuava anunciando cada uma delas, e o pacote de
/// armas crescia a cada drop perdido ate o fim do round. Quem decide e a
/// autoridade, junto com o resto do ciclo de vida da arma: o cliente ja trata
/// "sumiu do retrato" como arma que caiu fora.
fn sink_lost_weapons(
    mut commands: Commands,
    drops: Query<(Entity, &Transform), With<GroundWeapon>>,
) {
    for (entity, transform) in &drops {
        if transform.translation.y < KILL_Y {
            commands.entity(entity).despawn();
        }
    }
}

/// Encosta na arma, pega a arma. Quem ja tem uma ignora o chao.
fn pick_up(
    mut commands: Commands,
    drops: Query<(Entity, &GroundWeapon, &Transform, &Collider), Without<ThrownWeapon>>,
    players: Query<(Entity, &Transform, &Collider, &Facing), (With<Player>, Without<Held>)>,
) {
    for (player, player_transform, player_collider, facing) in &players {
        let body = player_collider.aabb(player_transform.translation.truncate());

        for (drop, ground, drop_transform, drop_collider) in &drops {
            if !overlap(
                body,
                drop_collider.aabb(drop_transform.translation.truncate()),
            ) {
                continue;
            }

            equip(&mut commands, player, ground.kind, ground.ammo, facing.0);
            commands.entity(drop).despawn();
            break;
        }
    }
}

/// Forca com que a arma sai da mao.
const THROW_SPEED: f32 = 590.0;
/// Empurrao horizontal de quem leva a arma na cara.
const THROW_PUSH: f32 = 390.0;

/// O arco de um arremesso mirando em `dir`.
///
/// A direcao gira com o cursor, o levante nao: mirar na horizontal tem que dar
/// exatamente o arremesso de sempre, e mirar no chao tem que cravar a arma la,
/// e nao jogar ela pra tras. E a mesma regra do empurrao do soco.
fn thrown_arc(dir: Vec2, lift: f32, speed: f32) -> Vec2 {
    dir * speed + Vec2::Y * lift
}

/// Arremessa a arma para onde o jogador esta mirando.
fn throw_weapon(
    mut commands: Commands,
    mut ids: ResMut<NextWeaponId>,
    mut players: Query<
        (Entity, &Intent, &Transform, &mut Facing, &Pose, &Held),
        (With<Player>, Without<Attacking>, Without<Parrying>),
    >,
) {
    for (player, intent, transform, mut facing, pose, held) in &mut players {
        if !intent.throw_weapon || pose.locks_control() {
            continue;
        }

        let dir = turn_to_aim(&mut facing, intent);
        // Sai da mao, e nao do centro do corpo: e de la que a arma parte.
        let at = transform.translation.truncate() + Vec2::new(0.0, 10.0) + dir * 46.0;
        ids.0 = ids.0.wrapping_add(1);
        let thrown = spawn_ground_weapon(
            &mut commands,
            Some(ids.0),
            held.kind,
            held.ammo,
            at,
            thrown_arc(dir, 260.0, THROW_SPEED),
        );
        commands.entity(thrown).insert((
            ThrownWeapon(Timer::from_seconds(0.55, TimerMode::Once)),
            Hitbox {
                owner: player,
                damage: held.weapon.melee(2).damage,
                knockback: thrown_arc(dir, 190.0, THROW_PUSH),
                stun: crate::combat::HIT_STUN,
            },
        ));
        commands.entity(player).remove::<Held>();
    }
}

/// Direcao do disparo: a mira do mouse quando existe, senao o lado para o qual
/// o boneco olha -- e o que mantem o jogador 2, sem mouse, jogavel do mesmo
/// jeito de antes.
pub fn aim_dir(intent: &Intent, facing: &Facing) -> Vec2 {
    if intent.aim == Vec2::ZERO {
        Vec2::new(facing.0, 0.0)
    } else {
        intent.aim
    }
}

/// Vira o boneco para a mira e devolve a direcao do golpe.
///
/// Tiro, soco e arremesso saem os tres para onde o cursor esta, e o corpo tem
/// que acompanhar os tres do mesmo jeito -- senao a arma sai pelas costas de
/// quem a jogou. Uma regra, um lugar: enquanto ela foi copiada por sistema,
/// cada copia tinha a propria margem.
///
/// A margem existe porque mira quase vertical nao tem lado: sem ela, passar o
/// cursor por cima da cabeca faria o boneco girar no lugar a cada quadro.
pub fn turn_to_aim(facing: &mut Facing, intent: &Intent) -> Vec2 {
    let dir = aim_dir(intent, facing);
    if dir.x.abs() > 0.15 {
        facing.0 = dir.x.signum();
    }
    dir
}

/// Rotacao que faz uma arte desenhada apontando para a direita mirar em `dir`,
/// ja descontando o espelhamento horizontal do sprite.
fn aim_angle(dir: Vec2, flipped: bool) -> f32 {
    let angle = dir.to_angle();
    if flipped {
        angle - std::f32::consts::PI
    } else {
        angle
    }
}

/// A mao armada e a linha do cano, pela mesma resposta que o desenho usa.
///
/// O tiro somava um ponto fixo ao centro do corpo enquanto o desenho perguntava
/// a `armed_joints` onde a mao estava. Bastava a pose levantar o braco --
/// mirar para cima, atirar correndo -- para o clarao nascer ao lado da arma em
/// vez de na boca dela.
fn aiming_arm(pose: Pose, style: WeaponStyle, facing: f32, aim: Vec2) -> crate::actor::rig::Joints {
    crate::actor::rig::armed_joints(
        pose,
        style,
        0.0,
        &crate::actor::rig::Rigging {
            side: 1.0,
            facing,
            gait: 0.0,
            aim,
            strike: None,
            reach: 0.0,
            cycle: 0.0,
            air: 0.0,
        },
    )
}

