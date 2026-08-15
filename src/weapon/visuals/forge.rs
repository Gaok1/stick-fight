#[derive(Deserialize)]
struct ForgeScene {
    elements: Vec<ForgeElement>,
    #[serde(default)]
    rig: ForgeRig,
    #[serde(default)]
    attention_points: Vec<ForgePoint>,
    #[serde(default)]
    labels: Vec<ForgeLabel>,
}

#[derive(Deserialize)]
struct ForgeElement {
    id: String,
    glyph: String,
    x: f32,
    y: f32,
    font_size: f32,
    scale_x: f32,
    scale_y: f32,
    flip_x: bool,
    flip_y: bool,
    rotation: f32,
    color: String,
    layer: i32,
}

#[derive(Deserialize, Default)]
struct ForgeRig {
    #[serde(default)]
    joints: Vec<ForgePoint>,
}

#[derive(Deserialize)]
struct ForgePoint {
    x: f32,
    y: f32,
}

#[derive(Deserialize)]
struct ForgeLabel {
    id: String,
    name: String,
    #[serde(default)]
    element_ids: Vec<String>,
    #[serde(default)]
    label_ids: Vec<String>,
}

fn forge_scene(source: &'static str) -> ForgeScene {
    serde_json::from_str(source).expect("JSON do Glyph Forge invalido")
}

fn nunchaku_scene() -> &'static ForgeScene {
    static SCENE: OnceLock<ForgeScene> = OnceLock::new();
    SCENE.get_or_init(|| {
        forge_scene(include_str!(
            "../../../glyph_forge/creations/armas/nunchako.glyph.json"
        ))
    })
}

fn book_spawn_scene() -> &'static ForgeScene {
    static SCENE: OnceLock<ForgeScene> = OnceLock::new();
    SCENE.get_or_init(|| {
        forge_scene(include_str!(
            "../../../glyph_forge/creations/armas/magic_book.glyph.json"
        ))
    })
}

fn sword_scene() -> &'static ForgeScene {
    static SCENE: OnceLock<ForgeScene> = OnceLock::new();
    SCENE.get_or_init(|| {
        forge_scene(include_str!(
            "../../../glyph_forge/creations/armas/fency_sword.glyph.json"
        ))
    })
}

fn book_held_scene() -> &'static ForgeScene {
    static SCENE: OnceLock<ForgeScene> = OnceLock::new();
    SCENE.get_or_init(|| {
        forge_scene(include_str!(
            "../../../glyph_forge/creations/armas/magic_book_open_side.glyph.json"
        ))
    })
}

fn collect_label_ids<'a>(
    scene: &'a ForgeScene,
    label: &'a ForgeLabel,
    ids: &mut HashSet<&'a str>,
) {
    ids.extend(label.element_ids.iter().map(String::as_str));
    for child in &label.label_ids {
        let nested = scene
            .labels
            .iter()
            .find(|candidate| candidate.id == *child)
            .unwrap_or_else(|| panic!("sub-rotulo {child} ausente no Glyph Forge"));
        collect_label_ids(scene, nested, ids);
    }
}

fn labeled_elements(
    scene: &'static ForgeScene,
    name: &str,
) -> Vec<&'static ForgeElement> {
    let label = scene
        .labels
        .iter()
        .find(|label| label.name.eq_ignore_ascii_case(name))
        .unwrap_or_else(|| panic!("rotulo {name} ausente no Glyph Forge"));
    let mut ids = HashSet::new();
    collect_label_ids(scene, label, &mut ids);
    let mut elements: Vec<_> = scene
        .elements
        .iter()
        .filter(|element| ids.contains(element.id.as_str()))
        .collect();
    elements.sort_by_key(|element| element.layer);
    elements
}

fn element_reach(element: &ForgeElement) -> Vec2 {
    let rows = element.glyph.lines().count().max(1) as f32;
    let cols = element
        .glyph
        .lines()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(1) as f32;
    let ratio = element.font_size / 16.0;
    let half = Vec2::new(
        cols * 8.0 * ratio * element.scale_x.abs(),
        rows * 16.0 * ratio * element.scale_y.abs(),
    ) * 0.5;
    let angle = element.rotation.to_radians();
    let (sin, cos) = (angle.sin().abs(), angle.cos().abs());
    Vec2::new(
        half.x * cos + half.y * sin,
        half.x * sin + half.y * cos,
    )
}

fn forge_bounds(elements: &[&ForgeElement]) -> Rect {
    let mut min = Vec2::splat(f32::INFINITY);
    let mut max = Vec2::splat(f32::NEG_INFINITY);
    for element in elements {
        let at = Vec2::new(element.x, element.y);
        let reach = element_reach(element);
        min = min.min(at - reach);
        max = max.max(at + reach);
    }
    Rect::from_corners(min, max)
}

fn forge_fit(elements: &[&ForgeElement], display: Vec2, parent_scale: f32) -> f32 {
    let size = forge_bounds(elements).size();
    (display.x / (size.x * parent_scale))
        .min(display.y / (size.y * parent_scale))
}

fn forge_color(value: &str) -> Color {
    let value = value.strip_prefix('#').unwrap_or(value);
    assert_eq!(value.len(), 6, "cor {value:?} invalida no Glyph Forge");
    let channel = |at| u8::from_str_radix(&value[at..at + 2], 16).unwrap() as f32 / 255.0;
    Color::srgb(channel(0), channel(2), channel(4))
}

/// O JSON desenha oito `o` e sete ligaduras entre eles. O primeiro elo e o
/// proprio ponto preso ao porrete; nao existe um segmento invisivel antes dele.
const CHAIN_POINTS: usize = 8;

struct ChainMotion {
    points: [Vec2; CHAIN_POINTS],
    previous: [Vec2; CHAIN_POINTS],
    rest: [Vec2; CHAIN_POINTS],
    links: [f32; CHAIN_POINTS - 1],
    anchor: Vec2,
    ready: bool,
    attacking: bool,
}

impl Default for ChainMotion {
    fn default() -> Self {
        Self {
            points: [Vec2::ZERO; CHAIN_POINTS],
            previous: [Vec2::ZERO; CHAIN_POINTS],
            rest: [Vec2::ZERO; CHAIN_POINTS],
            links: [0.0; CHAIN_POINTS - 1],
            anchor: Vec2::ZERO,
            ready: false,
            attacking: false,
        }
    }
}

#[derive(Clone, Copy)]
enum PartRole {
    Static,
    Sight,
    Bolt,
    Pump,
    Fuse,
    BladeGleam,
    Charm,
    ChainLink(usize),
    FreeBaton,
    FreeBatonCap,
}

#[derive(Component)]
struct WeaponPart {
    role: PartRole,
    rest: Vec2,
    scale: Vec2,
    angle: f32,
    authored: bool,
}

#[derive(Clone, Copy)]
struct PartSpec {
    art: &'static str,
    color: Color,
    at: Vec2,
    scale: Vec2,
    angle: f32,
    role: PartRole,
    authored: bool,
}

impl PartSpec {
    const fn new(art: &'static str, color: Color, at: Vec2, role: PartRole) -> Self {
        Self {
            art,
            color,
            at,
            scale: Vec2::ONE,
            angle: 0.0,
            role,
            authored: false,
        }
    }

    const fn scaled(mut self, x: f32, y: f32) -> Self {
        self.scale = Vec2::new(x, y);
        self
    }

    const fn angled(mut self, angle: f32) -> Self {
        self.angle = angle;
        self
    }

    const fn authored(mut self) -> Self {
        self.authored = true;
        self
    }
}

fn forge_at(element: &ForgeElement, anchor: Vec2, fit: f32) -> Vec2 {
    Vec2::new(element.x - anchor.x, anchor.y - element.y) * fit
}

fn forge_spec(
    element: &'static ForgeElement,
    anchor: Vec2,
    fit: f32,
    role: PartRole,
) -> PartSpec {
    let ratio = element.font_size / 16.0 * fit;
    PartSpec::new(
        &element.glyph,
        forge_color(&element.color),
        forge_at(element, anchor, fit),
        role,
    )
    .scaled(
        ratio * element.scale_x * if element.flip_x { -1.0 } else { 1.0 },
        ratio * element.scale_y * if element.flip_y { -1.0 } else { 1.0 },
    )
    .angled(-element.rotation.to_radians())
    .authored()
}

fn book_model_parts(held: bool) -> Vec<PartSpec> {
    let (scene, label, display, parent) = if held {
        (
            book_held_scene(),
            "livro_magico_aberto_vista_leste",
            Vec2::new(56.0, 22.0),
            held_scale(WeaponLook::Book),
        )
    } else {
        (
            book_spawn_scene(),
            "magic book",
            Vec2::new(52.0, 44.0),
            GROUND_SCALE,
        )
    };
    let elements = labeled_elements(scene, label);
    let bounds = forge_bounds(&elements);
    let fit = forge_fit(&elements, display, parent);
    let anchor = if held {
        let grip = scene
            .attention_points
            .first()
            .expect("livro aberto sem pegada no Glyph Forge");
        Vec2::new(grip.x, grip.y)
    } else {
        Vec2::new(bounds.center().x, bounds.max.y)
    };
    elements
        .into_iter()
        .map(|element| forge_spec(element, anchor, fit, PartRole::Static))
        .collect()
}

fn nunchaku_chain() -> Vec<&'static ForgeElement> {
    let scene = nunchaku_scene();
    let elements = labeled_elements(scene, "Correntes do nunchako");
    assert_eq!(elements.len(), CHAIN_POINTS, "o JSON mudou a quantidade de elos");
    assert_eq!(
        scene.rig.joints.len(),
        CHAIN_POINTS - 1,
        "o JSON precisa de uma ligadura entre cada elo"
    );

    let mut graph = vec![Vec::<usize>::new(); elements.len()];
    for joint in &scene.rig.joints {
        let mut nearest: Vec<(f32, usize)> = elements
            .iter()
            .enumerate()
            .map(|(index, element)| {
                (
                    Vec2::new(element.x - joint.x, element.y - joint.y).length_squared(),
                    index,
                )
            })
            .collect();
        nearest.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (a, b) = (nearest[0].1, nearest[1].1);
        if !graph[a].contains(&b) {
            graph[a].push(b);
            graph[b].push(a);
        }
    }

    let grip = scene
        .attention_points
        .first()
        .expect("nunchaku sem primeiro ponto de pega");
    let start = (0..elements.len())
        .filter(|index| graph[*index].len() == 1)
        .min_by(|a, b| {
            let distance = |index: usize| {
                Vec2::new(
                    elements[index].x - grip.x,
                    elements[index].y - grip.y,
                )
                .length_squared()
            };
            distance(*a).total_cmp(&distance(*b))
        })
        .expect("ligaduras do nunchaku nao formam uma corrente");

    let mut ordered = Vec::with_capacity(elements.len());
    let (mut previous, mut current) = (None, start);
    loop {
        ordered.push(elements[current]);
        let next = graph[current]
            .iter()
            .copied()
            .find(|candidate| Some(*candidate) != previous);
        let Some(next) = next else { break };
        previous = Some(current);
        current = next;
    }
    assert_eq!(ordered.len(), CHAIN_POINTS, "ligaduras nao percorrem os oito elos");
    ordered
}

fn nunchaku_fit() -> f32 {
    let elements = labeled_elements(nunchaku_scene(), "Nunchako");
    forge_fit(
        &elements,
        Vec2::new(58.0, 28.0),
        held_scale(WeaponLook::Nunchaku),
    )
    .min(forge_fit(
        &elements,
        Vec2::new(68.0, 36.0),
        GROUND_SCALE,
    ))
}

fn nunchaku_model_parts(held: bool) -> Vec<PartSpec> {
    let scene = nunchaku_scene();
    let elements = labeled_elements(scene, "Nunchako");
    let bounds = forge_bounds(&elements);
    let grip = scene
        .attention_points
        .first()
        .expect("nunchaku sem ponto de pega");
    let anchor = if held {
        Vec2::new(grip.x, grip.y)
    } else {
        Vec2::new(bounds.center().x, bounds.max.y)
    };
    let fit = nunchaku_fit();
    if !held {
        return elements
            .into_iter()
            .map(|element| forge_spec(element, anchor, fit, PartRole::Static))
            .collect();
    }

    let chain = nunchaku_chain();
    let loose: HashSet<&str> = labeled_elements(scene, "porrete_do_nunchako_2")
        .into_iter()
        .map(|element| element.id.as_str())
        .collect();
    elements
        .into_iter()
        .map(|element| {
            let role = if let Some(index) = chain.iter().position(|link| link.id == element.id) {
                PartRole::ChainLink(index)
            } else if loose.contains(element.id.as_str()) {
                if element.glyph == "\u{2590}" {
                    PartRole::FreeBaton
                } else {
                    PartRole::FreeBatonCap
                }
            } else {
                PartRole::Static
            };
            forge_spec(element, anchor, fit, role)
        })
        .collect()
}

fn nunchaku_chain_anchor() -> Vec2 {
    let scene = nunchaku_scene();
    let grip = scene.attention_points.first().unwrap();
    forge_at(
        nunchaku_chain()[0],
        Vec2::new(grip.x, grip.y),
        nunchaku_fit(),
    )
}

/// O estoque: uma lamina so, desenhada de lado.
///
/// O punho nao vem de um ponto de atencao como no livro -- a espada nao tem um
/// -- e sim da propria geometria: e a ponta *de tras*, o pomo. Numa arma que so
/// aponta para a frente isso e exato, e evita depender de o desenho ganhar um
/// ponto marcado a mao para continuar compilando.
fn sword_grip(elements: &[&'static ForgeElement]) -> Vec2 {
    let bounds = forge_bounds(elements);
    Vec2::new(bounds.min.x + 6.0, bounds.center().y)
}

fn sword_display(held: bool) -> Vec2 {
    if held {
        Vec2::new(60.0, 16.0)
    } else {
        Vec2::new(64.0, 22.0)
    }
}

fn sword_model_parts(held: bool) -> Vec<PartSpec> {
    let elements = labeled_elements(sword_scene(), "fency_sword");
    let anchor = sword_grip(&elements);
    let fit = forge_fit(
        &elements,
        sword_display(held),
        if held {
            held_scale(WeaponLook::FencySword)
        } else {
            GROUND_SCALE
        },
    );
    elements
        .into_iter()
        .map(|element| forge_spec(element, anchor, fit, PartRole::Static))
        .collect()
}

/// A ponta da lamina. Numa arma de estocada e dali que sai tudo: alcance,
/// faisca de contato e a leitura de onde o golpe acerta.
fn sword_tip() -> Vec2 {
    let elements = labeled_elements(sword_scene(), "fency_sword");
    let bounds = forge_bounds(&elements);
    let grip = sword_grip(&elements);
    let fit = forge_fit(
        &elements,
        sword_display(true),
        held_scale(WeaponLook::FencySword),
    );
    Vec2::new(bounds.max.x - grip.x, grip.y - bounds.center().y) * fit
}

fn book_muzzle() -> Vec2 {
    let scene = book_held_scene();
    let elements = labeled_elements(scene, "livro_magico_aberto_vista_leste");
    let bounds = forge_bounds(&elements);
    let grip = scene.attention_points.first().unwrap();
    let fit = forge_fit(
        &elements,
        Vec2::new(56.0, 22.0),
        held_scale(WeaponLook::Book),
    );
    Vec2::new(bounds.max.x - grip.x, grip.y - bounds.center().y) * fit
}

fn magic_runes() -> Vec<&'static ForgeElement> {
    labeled_elements(book_spawn_scene(), "Magic Runes")
}

/// Pecas que explicam mecanismo e material. O corpo continua sendo a silhueta;
/// cada filho acrescenta uma informacao: mira, ferrolho, guarda, pavio ou elo.
///
/// Escritas sempre na coordenada da arte **empunhada**, centrada e com `y` para
/// cima -- a mesma de [`grip_local`] e [`muzzle_local`]. Quem monta a arma
/// caida desloca a lista inteira de uma vez; duas listas de coordenadas, uma
/// por instancia, sempre acabam discordando.
fn weapon_parts(look: WeaponLook, held: bool) -> Vec<PartSpec> {
    match look {
        WeaponLook::Pistol => vec![
            // O ferrolho corre para tras no coice; a massa que ele desliza e o
            // que faz o tiro parecer accionar um mecanismo.
            PartSpec::new("\u{2580}", palette::ASH, Vec2::new(0.0, 10.0), PartRole::Bolt)
                .scaled(3.6, 0.4),
            PartSpec::new("\u{2022}", palette::GOLD, Vec2::new(20.0, 18.0), PartRole::Sight)
                .scaled(0.45, 0.45),
            // Guarda-mato: um `∩` de cabeca para baixo. Girar a peca sai de
            // graca e poupa inventar um glifo que a CP437 nao tem.
            PartSpec::new(
                "\u{2229}",
                palette::IRON,
                Vec2::new(-14.0, -13.0),
                PartRole::Static,
            )
            .scaled(0.75, 0.55)
            .angled(std::f32::consts::PI),
            PartSpec::new("\u{00ac}", palette::IRON, Vec2::new(-27.0, 9.0), PartRole::Static)
                .scaled(0.55, 0.55),
        ],
        WeaponLook::Shotgun => vec![
            PartSpec::new("\u{2593}", palette::IRON, Vec2::new(-4.0, -8.0), PartRole::Pump)
                .scaled(1.9, 0.5),
            PartSpec::new("\u{2022}", palette::GOLD, Vec2::new(42.0, 13.0), PartRole::Sight)
                .scaled(0.4, 0.4),
            // O estrangulador mora na propria boca: a peca e a marca lida
            // de onde o tiro sai, entao ela le a mesma coordenada que o tiro.
            PartSpec::new(
                "\u{2566}",
                palette::BONE,
                muzzle_local(WeaponLook::Shotgun),
                PartRole::Static,
            )
            .scaled(0.5, 0.5),
        ],
        WeaponLook::Rifle => vec![
            PartSpec::new(
                "\u{25d8}",
                palette::GOLD,
                Vec2::new(-20.0, 17.0),
                PartRole::Sight,
            )
            .scaled(0.55, 0.45),
            PartSpec::new(
                "\u{2550}",
                palette::BONE,
                Vec2::new(-12.0, 5.0),
                PartRole::Bolt,
            )
            .scaled(1.2, 0.4),
            // O carregador pende inclinado. Reto ele vira um pino, e e a
            // inclinacao que diz que ali entra municao.
            PartSpec::new(
                "\u{2593}",
                palette::IRON,
                Vec2::new(-16.0, -19.0),
                PartRole::Static,
            )
            .scaled(0.75, 1.0)
            .angled(-0.28),
            PartSpec::new(
                "\u{2666}",
                palette::BONE,
                muzzle_local(WeaponLook::Rifle),
                PartRole::Static,
            )
            .scaled(0.4, 0.36),
        ],
        WeaponLook::Pipe => vec![
            PartSpec::new(
                "\u{2591}",
                palette::BONE,
                Vec2::new(-34.0, 0.0),
                PartRole::Static,
            )
            .scaled(0.8, 0.85),
            PartSpec::new(
                "\u{2591}",
                palette::BONE,
                Vec2::new(-22.0, -13.0),
                PartRole::Static,
            )
            .scaled(0.75, 0.5),
            PartSpec::new(
                "\u{256a}",
                palette::GOLD,
                Vec2::new(44.0, 1.0),
                PartRole::Static,
            )
            .scaled(0.62, 0.7),
            // Fita solta balancando no cabo: nada fabricado tem isso, e e por
            // isso que ela esta aqui.
            PartSpec::new("'", palette::ASH, Vec2::new(-40.0, -12.0), PartRole::Charm)
                .scaled(0.6, 1.0),
        ],
        WeaponLook::Katana => vec![
            // Menuki: o ornamento preso na trama do cabo. Fica no meio do `ito`,
            // que e onde a mao aperta.
            PartSpec::new(
                "\u{2666}",
                palette::GOLD,
                Vec2::new(-48.0, 8.0),
                PartRole::Static,
            )
            .scaled(0.42, 0.45),
            // Hamon: o brilho corre pelo gume, na meia celula de baixo da
            // lamina, e nao pelo ar acima das costas.
            PartSpec::new(
                "\u{2500}",
                palette::BONE,
                Vec2::new(16.0, 4.0),
                PartRole::BladeGleam,
            )
            .scaled(3.2, 0.3),
            // Sageo: o cordao do cabo. E o que sobra balancando quando o corte
            // acaba, e nenhuma outra arma tem.
            PartSpec::new("'", palette::BLOOD, Vec2::new(-52.0, -8.0), PartRole::Charm)
                .scaled(0.7, 1.2),
        ],
        WeaponLook::Knife => vec![
            PartSpec::new(
                "\u{2666}",
                palette::GOLD,
                Vec2::new(-28.0, 8.0),
                PartRole::Static,
            )
            .scaled(0.45, 0.5),
            PartSpec::new(
                "\u{2500}",
                palette::BONE,
                Vec2::new(10.0, 4.0),
                PartRole::BladeGleam,
            )
            .scaled(1.4, 0.26),
        ],
        WeaponLook::Knives => vec![
            // Correia no punho: e ela que amarra as tres laminas num objeto so
            // em vez de tres facas que por acaso estao na mesma mao.
            PartSpec::new(
                "\u{2261}",
                palette::ASH,
                Vec2::new(-36.0, -10.0),
                PartRole::Static,
            )
            .scaled(0.55, 0.45),
            PartSpec::new(
                "\u{2500}",
                palette::BONE,
                Vec2::new(8.0, 0.0),
                PartRole::BladeGleam,
            )
            .scaled(1.6, 0.24),
            PartSpec::new(
                "\u{00b0}",
                palette::ASH,
                Vec2::new(-38.0, 14.0),
                PartRole::Charm,
            )
            .scaled(0.5, 0.8),
        ],
        WeaponLook::Bomb => vec![
            PartSpec::new("~", palette::ASH, Vec2::new(2.0, 20.0), PartRole::Fuse)
                .scaled(0.8, 0.7)
                .angled(0.45),
            PartSpec::new("*", palette::EMBER, Vec2::new(6.0, 22.0), PartRole::Fuse)
                .scaled(0.55, 0.55),
            PartSpec::new(
                "\u{2261}",
                palette::BLOOD,
                Vec2::new(0.0, -2.0),
                PartRole::Static,
            )
            .scaled(0.7, 0.5),
        ],
        WeaponLook::Book => book_model_parts(held),
        WeaponLook::Nunchaku => nunchaku_model_parts(held),
        WeaponLook::FencySword => sword_model_parts(held),
    }
}

fn modeled_size(look: WeaponLook, held: bool) -> Option<Vec2> {
    if !matches!(look, WeaponLook::Book | WeaponLook::Nunchaku) {
        return None;
    }
    let mut min = Vec2::splat(f32::INFINITY);
    let mut max = Vec2::splat(f32::NEG_INFINITY);
    for part in weapon_parts(look, held) {
        let half = AsciiArt::solid(part.art, part.color).size() * part.scale.abs() * 0.5;
        let (sin, cos) = (part.angle.sin().abs(), part.angle.cos().abs());
        let reach = Vec2::new(
            half.x * cos + half.y * sin,
            half.x * sin + half.y * cos,
        );
        min = min.min(part.at - reach);
        max = max.max(part.at + reach);
    }
    Some((max - min) * if held { held_scale(look) } else { GROUND_SCALE })
}

/// De quanto as pecas precisam andar para caber numa instancia que nao e a mao.
///
/// Elas sao escritas na arte empunhada, centrada. A arma caida usa outra arte,
/// maior, e ancorada nos pes -- duas diferencas de origem que se somam. Sem
/// esta conta a tsuba da katana no chao flutua ao lado do cabo, e nada no jogo
/// reclama porque cada sprite, sozinho, esta certo.
fn part_shift(look: WeaponLook, held: &AsciiArt, ground: &AsciiArt) -> Vec2 {
    if matches!(look, WeaponLook::Book | WeaponLook::Nunchaku) {
        return Vec2::ZERO;
    }
    Vec2::new(
        (held.cols as f32 - ground.cols as f32) * crate::ascii::CELL.x * 0.5,
        (ground.rows as f32 - held.rows as f32) * crate::ascii::CELL.y * 0.5
            + ground.size().y * 0.5,
    )
}

fn attach_weapon_rig(
    commands: &mut Commands,
    root: Entity,
    look: WeaponLook,
    held: bool,
    origin: Vec2,
) {
    let specs = weapon_parts(look, held);
    // A corrente existe se ha elo para mover, e nao porque a arma se chama
    // nunchaku: assim a instancia do chao, que desenha os dois bastoes, nao
    // ganha um terceiro pendurado por fisica.
    let mut chain = specs
        .iter()
        .any(|spec| matches!(spec.role, PartRole::ChainLink(_) | PartRole::FreeBaton))
        .then(ChainMotion::default);
    if let Some(chain) = chain.as_mut() {
        for spec in &specs {
            if let PartRole::ChainLink(index) = spec.role {
                chain.rest[index] = spec.at + origin;
            }
        }
        chain.anchor = chain.rest[0];
        for index in 1..CHAIN_POINTS {
            chain.links[index - 1] = chain.rest[index].distance(chain.rest[index - 1]);
        }
    }

    let mut parts = Vec::new();
    for (order, spec) in specs.into_iter().enumerate() {
        let at = spec.at + origin;
        let part = commands
            .spawn((
                WeaponPart {
                    role: spec.role,
                    rest: at,
                    scale: spec.scale,
                    angle: spec.angle,
                    authored: spec.authored,
                },
                AsciiSprite::new(AsciiArt::solid(spec.art, spec.color)),
                Layer::Background,
                Transform::from_translation(at.extend(0.01 + order as f32 * 0.001))
                    .with_rotation(Quat::from_rotation_z(spec.angle))
                    .with_scale(spec.scale.extend(1.0)),
                ChildOf(root),
            ))
            .id();
        parts.push(part);
    }
    commands.entity(root).insert(WeaponRig {
        look,
        parts,
        origin,
        phase: None,
        step: 0,
        heavy: false,
        last_phase: None,
        recoil: 0.0,
        flipped: false,
        chain,
    });
    if look == WeaponLook::Book {
        commands.entity(root).insert(BookAura {
            clock: Timer::from_seconds(if held { 0.08 } else { 0.06 }, TimerMode::Repeating),
            step: 0,
            held,
        });
    }
}
