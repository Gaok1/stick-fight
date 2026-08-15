use super::*;

use crate::actor::pose::{BODY_COLS, BODY_ROWS, MeleeKind};
use crate::ascii::CELL;
use crate::ascii::cp437::glyph_char;

/// Meia celula por caractere de preview: 4 x 8 unidades de mundo.
///
/// Na escala da mao a arma inteira mede menos de tres celulas de altura. Um
/// grid de celula cheia devolveria a katana como duas fileiras de tracos --
/// que e exatamente a foto que esconde o erro de proporcao que estes
/// previews existem para mostrar.
const SUB: Vec2 = Vec2::new(4.0, 8.0);

/// Uma folha de preview: grid de caracteres com um canto ancorado no mundo.
struct Sheet {
    grid: Vec<Vec<char>>,
    /// Ponto do mundo que cai no canto superior esquerdo.
    corner: Vec2,
}

impl Sheet {
    fn new(span: Rect) -> Self {
        let cols = (span.width() / SUB.x).ceil() as usize;
        let rows = (span.height() / SUB.y).ceil() as usize;
        Self {
            grid: vec![vec![' '; cols]; rows],
            corner: Vec2::new(span.min.x, span.max.y),
        }
    }

    /// Carimba uma arte ja girada e escalada, refazendo a conta que
    /// `rebuild_glyphs` faz na tela.
    ///
    /// Desenhar a arte parada mostraria uma arma que nunca girou nem
    /// encolheu -- e e justamente entre a folha e a mao que uma silhueta
    /// boa no papel chega irreconhecivel.
    fn stamp(&mut self, art: &AsciiArt, flip: bool, at: Vec2, angle: f32, scale: Vec2) {
        let art = if flip { art.mirrored() } else { art.clone() };
        let span = art.size();
        let turn = Vec2::from_angle(angle);
        for cell in &art.cells {
            let local = Vec2::new(
                (cell.col as f32 + 0.5) * CELL.x - span.x * 0.5,
                span.y * 0.5 - (cell.row as f32 + 0.5) * CELL.y,
            ) * scale;
            let world = at + turn.rotate(local);
            let col = ((world.x - self.corner.x) / SUB.x).round();
            let row = ((self.corner.y - world.y) / SUB.y).round();
            if col < 0.0 || row < 0.0 || row as usize >= self.grid.len() {
                continue;
            }
            let (col, row) = (col as usize, row as usize);
            if col < self.grid[row].len() {
                self.grid[row][col] = glyph_char(cell.glyph);
            }
        }
    }

    /// O tronco do lutador, para dar escala. Sao os 3x4 glifos de sempre --
    /// 24 por 64 -- e e contra esta caixa que toda arma e medida.
    fn stamp_fighter(&mut self) {
        let body = AsciiArt::solid(crate::actor::rig::def(Pose::IdleA).art, palette::ASH);
        self.stamp(&body, false, Vec2::new(0.0, -8.0), 0.0, Vec2::ONE);
    }

    fn text(&self) -> String {
        self.grid
            .iter()
            .map(|line| line.iter().collect::<String>().trim_end().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Carimba a arma inteira -- corpo rigido e pecas -- como ela sai na mao.
///
/// Passa por [`on_weapon`], que e a mesma funcao que o jogo usa: preview
/// com conta propria e um segundo desenho que pode discordar do primeiro.
fn stamp_held(sheet: &mut Sheet, weapon: &dyn Weapon, hand: Vec2, along: Vec2, flip: bool) {
    let look = weapon_look(weapon);
    let scale = held_scale(look);
    let angle = aim_angle(along, flip);
    let side = if flip { -1.0 } else { 1.0 };
    sheet.stamp(
        &weapon_art(weapon.held_art()),
        flip,
        on_weapon(look, Vec2::ZERO, hand, along, flip),
        angle,
        Vec2::splat(scale),
    );
    for part in weapon_parts(look, true) {
        let mut part_scale = part.scale * scale;
        if part.authored {
            part_scale.x *= side;
        }
        // A arte da peca nao e espelhada na tela -- so a posicao dela e --,
        // entao o preview tambem nao pode espelhar.
        sheet.stamp(
            &AsciiArt::solid(part.art, part.color),
            false,
            on_weapon(look, part.at, hand, along, flip),
            angle + part.angle * side,
            part_scale,
        );
    }
}

/// A mao que segura a arma parada, na linha da mira.
fn resting_grip(weapon: &dyn Weapon) -> (Vec2, Vec2) {
    let arm = aiming_arm(Pose::IdleA, weapon.style(), 1.0, Vec2::X);
    (arm.hand, (arm.hand - arm.elbow).normalize_or(Vec2::X))
}

/// Preview de autoria: a arte crua e as pecas no grid da propria arte.
fn preview_weapon(weapon: &dyn Weapon) -> String {
    const COLS: usize = 26;
    const ROWS: usize = 9;
    let mut grid = vec![vec![' '; COLS]; ROWS];
    let base = weapon_art(weapon.held_art());
    let center = IVec2::new(9, 4);
    let mut paint = |art: &AsciiArt, at: IVec2| {
        let origin = at - IVec2::new(art.cols as i32 / 2, art.rows as i32 / 2);
        for cell in &art.cells {
            let here = origin + IVec2::new(cell.col as i32, cell.row as i32);
            if here.x >= 0 && here.y >= 0 && here.x < COLS as i32 && here.y < ROWS as i32 {
                grid[here.y as usize][here.x as usize] = glyph_char(cell.glyph);
            }
        }
    };
    paint(&base, center);
    for part in weapon_parts(weapon_look(weapon), true) {
        let at = center
            + IVec2::new(
                (part.at.x / CELL.x).round() as i32,
                (-part.at.y / CELL.y).round() as i32,
            );
        paint(&AsciiArt::solid(part.art, part.color), at);
    }
    grid.into_iter()
        .map(|line| line.into_iter().collect::<String>().trim_end().to_owned())
        .collect::<Vec<_>>()
        .join("\n")
}

/// A arma caida na escala do chao, ao lado do lutador.
///
/// Mesma razao do preview da mao, e o mesmo erro pelo outro lado: a arte do
/// chao e desenhada solta, num grid onde ela sempre parece proporcional, e
/// so encostada no boneco aparece que a katana deitada media duas vezes o
/// lutador.
fn preview_on_ground(weapon: &dyn Weapon) -> String {
    let mut sheet = Sheet::new(Rect::from_corners(
        Vec2::new(-60.0, -56.0),
        Vec2::new(100.0, 56.0),
    ));
    sheet.stamp_fighter();
    let art = weapon_art(weapon.ground_art());
    // A raiz fica nos pes da arma, como `AsciiSprite::footed` a ancora no
    // jogo; a arte sobe meia altura a partir dali e as pecas penduram na
    // raiz, e nao no centro do desenho.
    let feet = Vec2::new(40.0, -32.0);
    let shift = part_shift(weapon_look(weapon), &weapon_art(weapon.held_art()), &art);
    sheet.stamp(
        &art,
        false,
        feet + Vec2::Y * art.size().y * GROUND_SCALE * 0.5,
        0.0,
        Vec2::splat(GROUND_SCALE),
    );
    for part in weapon_parts(weapon_look(weapon), false) {
        sheet.stamp(
            &AsciiArt::solid(part.art, part.color),
            false,
            feet + (part.at + shift) * GROUND_SCALE,
            part.angle,
            part.scale * GROUND_SCALE,
        );
    }
    sheet.text()
}

/// A arma na escala da mao, ao lado do lutador.
///
/// A foto isolada esconde exatamente o que importa. Uma katana caprichada
/// em treze colunas pode chegar na mao do tamanho de um canivete, e nada
/// alem de ver as duas coisas juntas denuncia isso.
fn preview_in_hand(weapon: &dyn Weapon) -> String {
    let mut sheet = Sheet::new(Rect::from_corners(
        Vec2::new(-60.0, -56.0),
        Vec2::new(100.0, 56.0),
    ));
    sheet.stamp_fighter();
    let (hand, along) = resting_grip(weapon);
    stamp_held(&mut sheet, weapon, hand, along, false);
    sheet.text()
}

#[test]
#[ignore = "so para olhar"]
fn olhar_o_arsenal() {
    for make in ARSENAL {
        let weapon = make();
        let caixa = held_extent(weapon.as_ref());
        let (hand, along) = resting_grip(weapon.as_ref());
        let look = weapon_look(weapon.as_ref());
        let boca = on_weapon(look, muzzle_local(look), hand, along, false);
        println!(
            "\n=== {} ===\n{}\n\n  -- na mao, ao lado do lutador (o boneco e {}x{}) --\n{}\n\n  \
                 -- caida, ao lado do lutador --\n{}\n\n  na mao: {:.0} de comprimento por {:.0} \
                 de espessura; a boca fica a {:.0} do centro do corpo\n  no chao: {:.0} por {:.0}",
            weapon.name(),
            preview_weapon(weapon.as_ref()),
            BODY_COLS as f32 * CELL.x,
            BODY_ROWS as f32 * CELL.y,
            preview_in_hand(weapon.as_ref()),
            preview_on_ground(weapon.as_ref()),
            caixa.width(),
            caixa.height(),
            boca.length(),
            modeled_size(look, false)
                .unwrap_or(weapon_art(weapon.ground_art()).size() * GROUND_SCALE)
                .x,
            modeled_size(look, false)
                .unwrap_or(weapon_art(weapon.ground_art()).size() * GROUND_SCALE)
                .y,
        );
    }
}

#[test]
fn toda_arma_tem_silhueta_e_composicao_proprias() {
    let mut previews = Vec::new();
    for make in ARSENAL {
        let weapon = make();
        let preview = preview_weapon(weapon.as_ref());
        assert!(
            preview.chars().filter(|ch| !ch.is_whitespace()).count() >= 5,
            "{} continua pequeno demais para ler",
            weapon.name()
        );
        assert!(
            !weapon_parts(weapon_look(weapon.as_ref()), true).is_empty(),
            "{} nao tem composicao secundaria",
            weapon.name()
        );
        assert!(
            !previews.contains(&preview),
            "{} repete outra silhueta",
            weapon.name()
        );
        previews.push(preview);
    }
}

/// Nenhuma arma volta a ser uma fileira de glifos.
///
/// O arsenal inteiro ja foi assim: a katana era `o'♦══─══►`, nove glifos em
/// uma linha, e a faca `o♦═─►`. Nenhuma parecia com o que era, e nao por
/// falta de detalhe -- por falta de forma. Uma linha nao tem costas e gume,
/// nem cabo e lamina, nem coronha e cano. Aqui o piso e tres linhas com
/// desenho de verdade em cada uma.
#[test]
fn nenhuma_arma_e_uma_fileira_de_glifos() {
    for make in ARSENAL {
        let weapon = make();
        let look = weapon_look(weapon.as_ref());
        for (onde, art) in [
            ("na mao", weapon_art(weapon.held_art())),
            ("no chao", weapon_art(weapon.ground_art())),
        ] {
            // Arma desenhada no Glyph Forge nao tem silhueta em `held_art`:
            // quem a desenha sao as pecas. A contagem e escrita por arma, e nao
            // um minimo comum, porque o que este teste pega e o JSON perdendo
            // peca em silencio -- e um limite frouxo nao pegaria.
            if let Some(minimo) = forge_parts_esperadas(look) {
                assert!(
                    weapon_parts(look, onde == "na mao").len() >= minimo,
                    "{} {onde}: o modelo do Glyph Forge perdeu pecas",
                    weapon.name()
                );
                continue;
            }
            let mut ocupadas: Vec<u16> = art.cells.iter().map(|cell| cell.row).collect();
            ocupadas.sort_unstable();
            ocupadas.dedup();
            assert!(
                ocupadas.len() >= 3,
                "{} {onde}: {} linha(s) desenhada(s), a silhueta nao tem altura",
                weapon.name(),
                ocupadas.len()
            );
        }
    }
}

/// A arma caida nao pode ser desenhada acima de onde se encosta nela.
///
/// O sprite do chao e ancorado nos pes e a caixa de pegar e centrada na
/// mesma origem: toda altura de arte que passa da caixa vira desenho
/// pendurado no ar, longe do lugar em que o jogador precisa estar. Quando o
/// arsenal foi redesenhado a arte do nunchaku caido subiu para cinco linhas
/// -- 80 unidades, mais que o proprio boneco -- e nada reclamou, porque o
/// sprite e o colisor continuavam certos cada um por si.
///
/// Comprimento e outra conversa: arma comprida deitada passa da caixa de
/// lado, e sempre passou -- pisar em qualquer pedaco dela pega.
#[test]
fn a_arma_caida_nao_flutua_acima_de_onde_se_pega() {
    for make in ARSENAL {
        let weapon = make();
        let look = weapon_look(weapon.as_ref());
        let no_chao = modeled_size(look, false)
            .unwrap_or(weapon_art(weapon.ground_art()).size() * GROUND_SCALE);
        let alta = no_chao.y;
        assert!(
            alta <= PICKUP_MAX.y,
            "{}: a arte do chao tem {alta} de altura e a caixa de pegar para em {}",
            weapon.name(),
            PICKUP_MAX.y
        );
        // E o desenho no chao tem que ser maior que o da mao -- e para isso
        // que existem duas artes. As duas medidas ja saem escaladas: comparar
        // arte crua com arte encolhida dava a katana caida como "mais
        // legivel" mesmo quando ela aparecia menor que a da mao na tela.
        let na_mao = modeled_size(look, true)
            .unwrap_or(weapon_art(weapon.held_art()).size() * held_scale(look));
        if look == WeaponLook::Book {
            assert!(
                no_chao.y > na_mao.y,
                "{}: o livro fechado nao ficou mais alto que o perfil aberto",
                weapon.name()
            );
            continue;
        }
        assert!(
            no_chao.x > na_mao.x && no_chao.y > na_mao.y,
            "{}: a arma no chao nao e mais legivel que a da mao ({no_chao:?} contra {na_mao:?})",
            weapon.name()
        );
    }
}

/// Toda arma cabe na silhueta do lutador, com `held_scale` ja aplicado.
///
/// O boneco tem tres colunas por quatro linhas: 24 por 64. Uma arte de
/// tres linhas na escala antiga -- 0,7 -- sai com 34 unidades atravessadas
/// no boneco de 24 de largura, e o jogo desenha isso sem reclamar de nada:
/// cada sprite, sozinho, continua correto. So a soma esta errada, e so
/// medindo os dois juntos da para ver.
///
/// A corrente do nunchaku fica de fora, e nao por ser nunchaku: pecas de
/// papel `ChainLink`, `FreeBaton` e seu fixador sao movidas por fisica justamente para
/// sair da silhueta da mao. E a arma cuja metade solta *tem* que escapar.
#[test]
fn toda_arma_cabe_na_silhueta_do_lutador() {
    let largura = BODY_COLS as f32 * CELL.x;
    let altura = BODY_ROWS as f32 * CELL.y;
    for make in ARSENAL {
        let weapon = make();
        let caixa = held_extent(weapon.as_ref());
        assert!(
            caixa.height() <= largura,
            "{}: {:.1} de espessura na mao, e o boneco tem {largura} de largura",
            weapon.name(),
            caixa.height()
        );
        assert!(
            caixa.width() <= altura,
            "{}: {:.1} de comprimento na mao, mais que os {altura} de altura do boneco",
            weapon.name(),
            caixa.width()
        );
        // E ela tem que ter tamanho: uma arma que encolheu ate caber nao
        // resolveu nada.
        let minimo = if matches!(
            weapon_look(weapon.as_ref()),
            WeaponLook::Book | WeaponLook::Nunchaku
        ) {
            Vec2::new(4.0, 2.0)
        } else {
            Vec2::new(16.0, 12.0)
        };
        assert!(
            caixa.width() >= minimo.x && caixa.height() >= minimo.y,
            "{}: sumiu na mao ({:.1} x {:.1})",
            weapon.name(),
            caixa.width(),
            caixa.height()
        );
    }
}

/// Caixa que a arma ocupa na mao, medida na linha do cano: `x` ao longo
/// dela, `y` atravessado. Ja com a escala da mao e as pecas somadas.
fn held_extent(weapon: &dyn Weapon) -> Rect {
    let look = weapon_look(weapon);
    let scale = held_scale(look);
    let grip = grip_local(look);
    let mut span = Rect::from_center_size(Vec2::ZERO, Vec2::ZERO);

    let art = weapon_art(weapon.held_art());
    let size = art.size();
    for cell in &art.cells {
        let at = Vec2::new(
            (cell.col as f32 + 0.5) * CELL.x - size.x * 0.5,
            size.y * 0.5 - (cell.row as f32 + 0.5) * CELL.y,
        ) - grip;
        span = span.union_point((at - CELL * 0.5) * scale);
        span = span.union_point((at + CELL * 0.5) * scale);
    }

    for part in weapon_parts(look, true) {
        if matches!(
            part.role,
            PartRole::ChainLink(_) | PartRole::FreeBaton | PartRole::FreeBatonCap
        ) {
            continue;
        }
        let half = AsciiArt::solid(part.art, part.color).size() * part.scale.abs() * 0.5;
        // Caixa alinhada de um retangulo girado: o carregador do rifle
        // pende inclinado, e medi-lo reto esconderia o quanto ele desce.
        let (sin, cos) = (part.angle.sin().abs(), part.angle.cos().abs());
        let reach = Vec2::new(half.x * cos + half.y * sin, half.x * sin + half.y * cos);
        let at = part.at - grip;
        span = span.union_point((at - reach) * scale);
        span = span.union_point((at + reach) * scale);
    }
    span
}

/// A boca do cano cai sobre uma celula do desenho da arma, dos dois lados.
///
/// Ela e uma coordenada escrita a parte da arte. Enquanto foi uma distancia
/// fixa, redesenhar o cano deixava o fogacho nascendo no ar ao lado da arma
/// -- e o jogo nao tem como reclamar, porque um clarao num ponto qualquer
/// continua sendo um clarao valido. E o mesmo erro que o dragao de
/// `backdrop.rs` cometeu com a boca dele.
///
/// Os dois lados importam porque espelhar nao e girar meia volta. Quem
/// troca uma coisa pela outra ganha uma arma que cospe pela culatra em
/// metade dos disparos e acerta na outra metade -- o pior tipo de bug, o
/// que funciona quando voce vai conferir.
#[test]
fn a_boca_do_cano_cai_sobre_o_desenho() {
    // Um ponto qualquer que nao seja a origem: mao na origem esconderia
    // erro de sinal que so aparece somado a translacao.
    let hand = Vec2::new(11.0, -7.0);
    for make in ARSENAL {
        let weapon = make();
        let look = weapon_look(weapon.as_ref());
        let art = weapon_art(weapon.held_art());
        let size = art.size();
        let limite = CELL.length() * 0.5 * held_scale(look);

        for (nome, along, flip) in [
            ("olhando pra direita", Vec2::X, false),
            ("olhando pra esquerda", Vec2::NEG_X, true),
            ("mirando pra cima", Vec2::new(0.4, 0.92).normalize(), false),
            (
                "mirando pra cima do avesso",
                Vec2::new(-0.4, 0.92).normalize(),
                true,
            ),
        ] {
            let muzzle = on_weapon(look, muzzle_local(look), hand, along, flip);
            let mut perto = art
                .cells
                .iter()
                .map(|cell| {
                    let local = Vec2::new(
                        (cell.col as f32 + 0.5) * CELL.x - size.x * 0.5,
                        size.y * 0.5 - (cell.row as f32 + 0.5) * CELL.y,
                    );
                    on_weapon(look, local, hand, along, flip).distance(muzzle)
                })
                .fold(f32::INFINITY, f32::min);
            for part in weapon_parts(look, true) {
                let center = on_weapon(look, part.at, hand, along, flip);
                let radius = AsciiArt::solid(part.art, part.color).size()
                    * part.scale.abs()
                    * held_scale(look)
                    * 0.5;
                perto = perto.min((center.distance(muzzle) - radius.length()).max(0.0));
            }
            assert!(
                perto <= limite,
                "{}: {nome}, a boca esta a {perto:.1} da celula mais proxima (limite {limite:.1})",
                weapon.name()
            );
            // E ela fica na ponta, nunca no meio nem atras do punho.
            let frente = (muzzle - hand).dot(along);
            assert!(
                frente > size.x * held_scale(look) * 0.25,
                "{}: {nome}, a boca recuou para dentro da arma",
                weapon.name()
            );
        }

        // Espelhar e refletir: o mesmo ponto visto dos dois lados troca o
        // sinal do `x` e mantem o `y`. Girar meia volta trocaria os dois.
        let direita = on_weapon(look, muzzle_local(look), Vec2::ZERO, Vec2::X, false);
        let esquerda = on_weapon(look, muzzle_local(look), Vec2::ZERO, Vec2::NEG_X, true);
        assert!(
            (direita.x + esquerda.x).abs() < 0.001 && (direita.y - esquerda.y).abs() < 0.001,
            "{}: virar para a esquerda girou a arma em vez de espelhar ({direita:?} contra {esquerda:?})",
            weapon.name()
        );
    }
}

#[test]
fn as_pecas_livres_cabem_na_cp437() {
    use crate::ascii::cp437::glyph_index;
    let fallback = glyph_index('?');
    let conferir = |quem: &str, onde: &str, art: &str| {
        for ch in art.chars().filter(|ch| !ch.is_whitespace()) {
            assert_ne!(
                glyph_index(ch),
                fallback,
                "{quem} usa {ch:?} fora da CP437 {onde}"
            );
        }
    };
    for make in ARSENAL {
        let weapon = make();
        // A silhueta entra na conferencia junto com as pecas: um glifo
        // fora da pagina vira `?` na tela, e um `?` no meio de uma lamina
        // le como sujeira de fonte, nao como erro de arte.
        conferir(weapon.name(), "na mao", weapon.held_art());
        conferir(weapon.name(), "no chao", weapon.ground_art());
        for held in [true, false] {
            for part in weapon_parts(weapon_look(weapon.as_ref()), held) {
                conferir(weapon.name(), "numa peca", part.art);
            }
        }
    }
}

/// Quantas pecas o modelo do Glyph Forge de cada arma tem que entregar.
///
/// `None` quer dizer que a arma desenha a silhueta em `held_art`, do jeito
/// antigo, e cai na conferencia de altura.
fn forge_parts_esperadas(look: WeaponLook) -> Option<usize> {
    match look {
        WeaponLook::Book | WeaponLook::Nunchaku => Some(9),
        // Lamina, guarda, pomo e o ornamento: sete.
        WeaponLook::FencySword => Some(7),
        _ => None,
    }
}

/// Toda arma de fogo tem clarao e capsula, e nenhuma repete a do vizinho.
///
/// Antes o disparo era o mesmo `{*}` em tres tamanhos: as tres armas
/// acendiam a mesma coisa, e quem jogava nao tinha como saber pelo tiro o
/// que estava na mao. Aqui a assinatura do fogacho -- linguas, abertura,
/// alcance e espessura -- tem que ser unica por arma.
///
/// A capsula sai de polvora queimada, e nao de qualquer coisa que voe: a
/// faca e a bomba saem do braco e nao deixam latao no chao.
#[test]
fn toda_arma_de_fogo_tem_disparo_proprio() {
    let mut assinaturas: Vec<(&str, [u32; 4])> = Vec::new();
    for make in ARSENAL {
        let weapon = make();
        let look = weapon_look(weapon.as_ref());
        let Some((flash, casing, smoke)) = gunfire(look) else {
            let arcane = weapon
                .shots(Vec2::X)
                .iter()
                .any(|shot| shot.kind == ShotKind::Arcane);
            assert!(
                weapon.is_melee() || arcane,
                "{} nao tem fogacho, magia nem golpe proprio",
                weapon.name()
            );
            continue;
        };
        assert!(
            !weapon.is_melee(),
            "{} e arma de contato e mesmo assim tem disparo",
            weapon.name()
        );
        assert!(
            flash.tongues > 0 && flash.reach > 0.0 && flash.width > 0.0,
            "{}: o fogacho nao tem forma",
            weapon.name()
        );
        assert!(
            smoke.puffs > 0,
            "{}: o tiro nao deixa fumaca",
            weapon.name()
        );

        let polvora = weapon
            .shots(Vec2::X)
            .iter()
            .any(|shot| shot.kind == ShotKind::Straight);
        assert_eq!(
            casing.is_some(),
            polvora,
            "{}: capsula e polvora discordam",
            weapon.name()
        );

        let assinatura = [
            flash.tongues as u32,
            flash.spread.to_bits(),
            flash.reach.to_bits(),
            flash.width.to_bits(),
        ];
        if let Some((outra, _)) = assinaturas.iter().find(|(_, a)| *a == assinatura) {
            panic!("{} e {outra} acendem o mesmo fogacho", weapon.name());
        }
        assinaturas.push((weapon.name(), assinatura));
    }
    assert!(assinaturas.len() >= 3, "o arsenal perdeu as armas de fogo");
}

/// O disparo acende na boca do cano, e nao no meio do boneco.
///
/// A tabela de `gunfire` pode estar impecavel e nada aparecer na tela:
/// basta o efeito nao ser emitido, ou ser emitido a partir do centro do
/// corpo. Este e o unico teste que roda `fire` de verdade e vai olhar onde
/// as particulas nasceram.
#[test]
fn o_disparo_acende_na_boca_do_cano() {
    for (kind, make) in ARSENAL.iter().enumerate() {
        let weapon = make();
        let look = weapon_look(weapon.as_ref());
        let Some((flash, casing, smoke)) = gunfire(look) else {
            continue;
        };

        let mut app = App::new();
        app.init_resource::<Time>()
            .add_message::<crate::fx::Shake>()
            .add_systems(Update, fire);
        let corpo = Vec2::new(-40.0, 25.0);
        let player = app
            .world_mut()
            .spawn((
                Player {
                    id: 0,
                    color: palette::player(0),
                },
                Intent {
                    special: true,
                    ..default()
                },
                Pose::IdleA,
                Facing(1.0),
                Transform::from_translation(corpo.extend(0.0)),
                Velocity::default(),
            ))
            .id();
        app.world_mut()
            .entity_mut(player)
            .insert(held_weapon(kind as u8, weapon.ammo()));

        app.update();

        // A boca esperada, pela mesma conta que `fire` usa.
        let arm = aiming_arm(Pose::IdleA, weapon.style(), 1.0, Vec2::X);
        let along = (arm.hand - arm.elbow).normalize_or(Vec2::X);
        let muzzle = on_weapon(look, muzzle_local(look), corpo + arm.hand, along, false);

        let world = app.world_mut();
        let fogo: Vec<Vec2> = world
            .query_filtered::<&Transform, (With<Lifetime>, Without<Projectile>)>()
            .iter(world)
            .map(|t| t.translation.truncate())
            .collect();

        // Linguas do leque, o estalo no meio, os bafos de fumaca e, quando
        // ha polvora, a capsula.
        let minimo = flash.tongues + smoke.puffs + 1 + usize::from(casing.is_some());
        assert!(
            fogo.len() >= minimo,
            "{}: o disparo acendeu {} particulas, e o fogacho, a fumaca e a capsula pedem {minimo}",
            weapon.name(),
            fogo.len()
        );
        let limite = flash.reach * 1.5 + 14.0;
        for at in &fogo {
            assert!(
                at.distance(muzzle) <= limite,
                "{}: uma particula nasceu a {:.1} da boca (limite {limite:.1})",
                weapon.name(),
                at.distance(muzzle)
            );
        }
        // E a boca esta la na frente, e nao encostada no peito: se o
        // clarao voltasse a sair do centro do corpo, tudo acima continuaria
        // passando e so isto reclamaria.
        assert!(
            fogo.iter().any(|at| at.distance(corpo) > 25.0),
            "{}: o disparo inteiro nasceu grudado no corpo",
            weapon.name()
        );
    }
}

#[test]
fn o_nunchaku_importa_oito_elos_e_sete_fixadores() {
    assert_eq!(nunchaku_scene().rig.joints.len(), 7);
    assert_eq!(
        weapon_parts(WeaponLook::Nunchaku, false).len(),
        labeled_elements(nunchaku_scene(), "Nunchako").len()
    );
    assert_eq!(
        weapon_parts(WeaponLook::Nunchaku, true)
            .iter()
            .filter(|part| matches!(part.role, PartRole::ChainLink(_)))
            .count(),
        CHAIN_POINTS
    );
}

/// O bastao solto fica preso na corrente e nao estica.
///
/// E fisica solta pendurada num ponto que gira depressa. Sem restricao --
/// ou com passadas de menos -- a corrente arrebenta no primeiro moinho e o
/// bastao fica atravessado na tela ate a arma sair da mao, e nada reclama:
/// ele continua sendo um sprite numa posicao perfeitamente valida.
///
/// E parado ele tem que pendurar, nao alinhar: dois bastoes em linha reta
/// leem como um bastao unico e comprido, que e o oposto de um nunchaku.
#[test]
fn o_bastao_solto_fica_preso_na_corrente() {
    let mut app = App::new();
    app.init_resource::<Time>()
        .add_systems(Startup, |mut commands: Commands| {
            let root = commands
                .spawn((
                    AsciiSprite::new(weapon_art(Nunchaku.held_art())),
                    Transform::default(),
                ))
                .id();
            attach_weapon_rig(&mut commands, root, WeaponLook::Nunchaku, true, Vec2::ZERO);
        })
        .add_systems(Update, animate_weapon_rigs);
    app.update();

    let girar = |app: &mut App, quadros: u32, golpe: Option<usize>, heavy: bool| {
        for quadro in 0..quadros {
            let world = app.world_mut();
            let mut rigs = world.query::<&mut WeaponRig>();
            for mut rig in rigs.iter_mut(world) {
                rig.phase = golpe.map(|_| (quadro % 3) as usize);
                rig.step = (quadro / 3 % 3) as u8;
                rig.heavy = heavy;
            }
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_millis(16));
            app.update();
        }
    };

    // Moinho, pesado e descanso: e no giro que a corrente arrebenta, nao
    // parada.
    girar(&mut app, 300, Some(1), false);
    girar(&mut app, 300, Some(1), true);
    girar(&mut app, 200, None, false);

    let world = app.world_mut();
    let (pontos, ancora, elos, repouso) = world
        .query::<&WeaponRig>()
        .iter(world)
        .map(|rig| {
            let chain = rig.chain.as_ref().expect("o nunchaku perdeu a corrente");
            (chain.points, chain.anchor, chain.links, chain.rest)
        })
        .next()
        .expect("a arma sumiu");

    assert!(
        pontos[0].distance(ancora) < 0.001,
        "a corrente soltou da argola do bastao: {:?} contra {ancora:?}",
        pontos[0]
    );
    for i in 1..CHAIN_POINTS {
        let elo = pontos[i].distance(pontos[i - 1]);
        assert!(
            (elo - elos[i - 1]).abs() < 0.5,
            "o elo {i} esticou para {elo:.2}, e o JSON pede {:.2}",
            elos[i - 1]
        );
    }

    let world = app.world_mut();
    let (solto, descanso_solto) = world
        .query::<(&WeaponPart, &Transform)>()
        .iter(world)
        .find(|(part, _)| matches!(part.role, PartRole::FreeBaton))
        .map(|(part, at)| (at.translation.truncate(), part.rest))
        .expect("o nunchaku perdeu o segundo bastao");
    let ponta = pontos[CHAIN_POINTS - 1];
    let distancia_modelada = descanso_solto.distance(repouso[CHAIN_POINTS - 1]);
    assert!(
        (solto.distance(ponta) - distancia_modelada).abs() < 0.5,
        "o bastao solto saiu da distancia modelada: {solto:?} contra {ponta:?}"
    );

    // Parado, a corda cai: o bastao solto nao pode ficar na linha do que
    // esta na mao.
    let pendurado = (solto - ancora).normalize_or(Vec2::X);
    assert!(
        pendurado.x.abs() < 0.87,
        "os dois bastoes ficaram alinhados ({pendurado:?}); parado, o nunchaku le como bastao unico"
    );
}

/// Nenhuma arma repete a assinatura de peso de outra.
///
/// Duas armas com a mesma duracao e o mesmo dano em cada elo sao a mesma
/// arma com dois nomes -- o HUD passa a ser a unica coisa que as separa. O
/// cano e a katana chegaram a ficar a um centesimo de segundo de distancia
/// em cada elo do combo, e nenhum teste percebeu.
#[test]
fn nenhuma_arma_repete_a_assinatura_de_peso_de_outra() {
    let mut vistas: Vec<(&str, Vec<(u32, i32)>)> = Vec::new();
    for make in ARSENAL {
        let weapon = make();
        let assinatura: Vec<(u32, i32)> = (0..3)
            .map(|step| {
                let elo = weapon.melee(step);
                ((elo.duration * 100.0).round() as u32, elo.damage)
            })
            .collect();
        if let Some((outra, _)) = vistas.iter().find(|(_, ja)| *ja == assinatura) {
            panic!("{} e {outra} pesam igual: {assinatura:?}", weapon.name());
        }
        vistas.push((weapon.name(), assinatura));
    }
}

/// Cada arma de contato pesa de um jeito, sentido sem olhar o HUD.
///
/// Cada `assert` aqui e uma leitura. Se duas armas trocassem de lugar em
/// qualquer uma delas, o jogador deixaria de ter motivo para preferir uma a
/// outra por outra coisa que nao o nome.
#[test]
fn cada_arma_de_contato_pesa_diferente() {
    let contato: Vec<Box<dyn Weapon>> = ARSENAL
        .iter()
        .map(|make| make())
        .filter(|weapon| weapon.is_melee())
        .collect();
    let de = |nome: &str| {
        contato
            .iter()
            .find(|weapon| weapon.name() == nome)
            .unwrap_or_else(|| panic!("{nome} saiu do arsenal"))
    };
    let (faca, katana, cano, nunchaku) = (de("KNIFE"), de("KATANA"), de("PIPE"), de("NUNCHAKU"));
    let combo = |weapon: &dyn Weapon| (0..3).map(|s| weapon.melee(s).duration).sum::<f32>();

    // A katana e a mais lenta e a mais longa: errar com ela tem que doer.
    assert!(combo(katana.as_ref()) > combo(cano.as_ref()));
    assert!(katana.melee(2).reach > cano.melee(2).reach);
    assert!(katana.heavy().unwrap().duration > cano.heavy().unwrap().duration);

    // A faca fecha o combo mais depressa que qualquer outra arma do jogo.
    for make in ARSENAL {
        let outra = make();
        if outra.name() == faca.name() {
            continue;
        }
        assert!(
            combo(outra.as_ref()) > combo(faca.as_ref()),
            "{} fecha o combo tao rapido quanto a faca",
            outra.name()
        );
    }

    // O cano e o mais pesado: empurra mais que todo o resto.
    let empurrao = |weapon: &dyn Weapon| weapon.melee(2).knockback.length();
    for make in ARSENAL {
        let outra = make();
        if outra.name() == cano.name() {
            continue;
        }
        assert!(
            empurrao(outra.as_ref()) < empurrao(cano.as_ref()),
            "{} empurra tanto quanto o cano",
            outra.name()
        );
    }

    // O nunchaku e o unico cujo golpe chega depois de o braco parar: o
    // contato dele acontece na segunda metade da duracao, e o de todo o
    // resto na primeira.
    for step in 0..3u8 {
        for make in ARSENAL {
            let outra = make();
            if outra.name() == nunchaku.name() {
                continue;
            }
            assert!(
                outra.melee(step).contact < nunchaku.melee(step).contact,
                "{} acerta tao tarde quanto o nunchaku no elo {step}",
                outra.name()
            );
        }
        assert!(
            nunchaku.melee(step).contact > 0.5,
            "o nunchaku voltou a acertar no meio do arco"
        );
        // E ele paga isso no dano: o menor entre as armas de contato.
        assert!(nunchaku.melee(step).damage < faca.melee(step).damage);
    }
}

include!("runtime.rs");
