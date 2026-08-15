/// Arma na mao de um jogador.
#[derive(Component)]
pub struct Held {
    /// A arma.
    pub weapon: Box<dyn Weapon>,
    /// Municao restante.
    pub ammo: u32,
    /// Cadencia.
    pub cooldown: Timer,
    /// Indice no arsenal: e o que volta ao chao sem perder a identidade da arma.
    pub kind: u8,
}

/// Arma caida, esperando ser pega.
#[derive(Component)]
pub struct GroundWeapon {
    /// Indice no arsenal.
    pub kind: u8,
    /// Municao que ela ainda carrega.
    pub ammo: u32,
}

/// Identidade de uma arma na rede.
///
/// O dono numera cada arma que larga, e todo mundo passa a falar dela por esse
/// numero. Sem isso o cliente nao tem como saber se a espingarda que chegou no
/// pacote e a que ele ja desenhou ou uma segunda -- e a escolha errada ou
/// duplica a arma ou some com ela.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct NetWeapon(pub u16);

/// Proximo numero de arma a distribuir. So o dono escreve nele.
#[derive(Resource, Default)]
pub struct NextWeaponId(pub u16);

/// Onde a caixa de pegar uma arma comeca e onde ela para.
///
/// Ela nao pode simplesmente acompanhar a arte. A arte do chao existe para ser
/// lida de longe e cresceu quando o arsenal foi redesenhado -- se o colisor
/// tivesse crescido junto, pegar uma katana teria virado encostar a um corpo e
/// meio de distancia dela, e nenhum teste de silhueta perceberia: as duas
/// coisas parecem certas, e so a soma delas muda o jogo.
const PICKUP_MIN: Vec2 = Vec2::new(42.0, 24.0);
const PICKUP_MAX: Vec2 = Vec2::new(80.0, 48.0);

/// Escala da arma caida, como [`held_scale`] e a da arma na mao.
///
/// Sem ela a arma no chao saia em escala 1: a katana media 96 unidades deitada,
/// quatro vezes a largura do lutador e uma vez e meia a altura dele. Nenhum
/// teste reclamava, porque o sprite sozinho estava certo -- so quem via os dois
/// juntos na tela percebia que a fase estava coberta de armas maiores que os
/// bonecos.
///
/// Fica acima de todo `held_scale` de proposito: a arma no chao precisa ser mais
/// legivel que a da mao, e e isso que
/// `a_arma_caida_nao_flutua_acima_de_onde_se_pega` cobra.
const GROUND_SCALE: f32 = 0.55;

/// Poe uma arma no chao (ou no ar, se ela foi arremessada).
///
/// Um lugar so monta a entidade, e o dono e o cliente chamam o mesmo: enquanto
/// cada lado tinha a sua copia da montagem, a arma do cliente nascia sem
/// colisor e atravessava o chao enquanto a do dono ficava pousada.
pub fn spawn_ground_weapon(
    commands: &mut Commands,
    net: Option<u16>,
    kind: u8,
    ammo: u32,
    at: Vec2,
    velocity: Vec2,
) -> Entity {
    let weapon = weapon_at(kind);
    let look = weapon_look(weapon.as_ref());
    let ground_art = weapon_art(weapon.ground_art());
    // A caixa mede o desenho ja encolhido. Enquanto ela lia a arte crua, pegar
    // uma arma era encostar num retangulo quase o dobro do que aparecia na tela.
    let visual_size = modeled_size(look, false).unwrap_or(ground_art.size() * GROUND_SCALE);
    let pickup_size = visual_size.clamp(PICKUP_MIN, PICKUP_MAX);
    let shift = part_shift(look, &weapon_art(weapon.held_art()), &ground_art);
    let mut entity = commands.spawn((
        GroundWeapon { kind, ammo },
        PickupPulse(fastrand::f32() * 6.0),
        AsciiSprite::footed(ground_art),
        Layer::Pickup,
        Transform::from_translation(at.extend(0.0)).with_scale(Vec3::splat(GROUND_SCALE)),
        Velocity(velocity),
        Grounded::default(),
        Falls,
        Collider::size(pickup_size.x, pickup_size.y),
        DespawnOnExit(GameState::Fighting),
    ));
    if let Some(net) = net {
        entity.insert(NetWeapon(net));
    }
    let root = entity.id();
    attach_weapon_rig(commands, root, look, false, shift);
    root
}

/// Poe a arma `kind` na mao de `player`, com o icone que o boneco carrega.
///
/// Publica porque a replicacao precisa armar um boneco sem passar por encostar
/// na arma: quem manda no que cada um segura e o dono da sala, e o cliente so
/// obedece.
pub fn equip(commands: &mut Commands, player: Entity, kind: u8, ammo: u32, facing: f32) {
    let held = held_weapon(kind, ammo);
    let held_art = held.weapon.held_art();
    let look = weapon_look(held.weapon.as_ref());

    commands.entity(player).insert(held);
    let icon = commands
        .spawn((
            WeaponIcon,
            AsciiSprite::new(weapon_art(held_art)).flipped(facing < 0.0),
            Layer::Actor,
            Transform::from_translation(Vec3::new(facing * 18.0, 4.0, 0.1))
                .with_scale(Vec3::splat(held_scale(look))),
            ChildOf(player),
        ))
        .id();
    attach_weapon_rig(commands, icon, look, true, Vec2::ZERO);
}

/// A arma `kind` pronta para ir para a mao, ja carregada e fora de recarga.
pub fn held_weapon(kind: u8, ammo: u32) -> Held {
    let weapon = weapon_at(kind);
    let mut cooldown = Timer::from_seconds(weapon.cooldown(), TimerMode::Once);
    // Comeca pronta pra atirar.
    cooldown.tick(cooldown.duration());
    Held {
        weapon,
        ammo,
        cooldown,
        kind,
    }
}

/// Poe uma arma replicada a girar, como a que saiu de uma mao de verdade.
pub fn mark_thrown(commands: &mut Commands, weapon: Entity) {
    commands
        .entity(weapon)
        .insert(ThrownWeapon(Timer::from_seconds(0.55, TimerMode::Once)));
}

/// Arma arremessada; enquanto o timer corre, gira e causa um unico dano.
#[derive(Component)]
pub struct ThrownWeapon(pub Timer);

/// Glifo da arma desenhado ao lado do boneco.
///
/// Vive numa entidade filha propria em vez de ser carimbado na arte da pose:
/// assim nenhum sistema de animacao disputa a escrita do mesmo sprite.
#[derive(Component)]
pub struct WeaponIcon;

/// Uma arma e uma pequena cena: corpo rigido no `WeaponIcon`, detalhes em
/// filhos livres e, no nunchaku, uma corrente realmente integrada.
#[derive(Component)]
struct WeaponRig {
    look: WeaponLook,
    parts: Vec<Entity>,
    /// Deslocamento entre a coordenada em que as pecas foram escritas (a arte
    /// empunhada, centrada) e a entidade que as carrega. Zero na mao; no chao
    /// ele resolve a arte maior e a ancora nos pes.
    origin: Vec2,
    phase: Option<usize>,
    step: u8,
    /// O golpe em curso e o pesado? A corrente do nunchaku envolve no pesado e
    /// gira em moinho no combo, e sem esta linha os dois seriam o mesmo tranco.
    heavy: bool,
    last_phase: Option<usize>,
    recoil: f32,
    flipped: bool,
    chain: Option<ChainMotion>,
}

