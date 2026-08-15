use super::*;
use crate::ascii::cp437::glyph_char;
use bevy::ecs::system::RunSystemOnce;

const THEMES: [Theme; 4] = [
    Theme::City,
    Theme::Volcano,
    Theme::Industrial,
    Theme::Oriental,
];
const SCENES: [Scene; 10] = [
    Scene::City,
    Scene::Caldera,
    Scene::MagmaBridge,
    Scene::ForgeCore,
    Scene::AcidWorks,
    Scene::Reactor,
    Scene::Drainage,
    Scene::RedGate,
    Scene::SunsetPagoda,
    Scene::DragonGarden,
];

/// A arte que nasce durante a partida, e por isso nao passa por [`panels`].
fn loose_art() -> Vec<&'static str> {
    vec![CRATER_GLOW, BLAST_FLASH, LANTERN, HAMMER, CRANE]
        .into_iter()
        .chain(PUFF)
        .collect()
}

/// A arte do dragao, que nao passa por [`panels`] porque ele nao e
/// composicao parada.
///
/// Ela entra em [`every_cell`] junto com o resto: um bicho que anda e
/// exatamente o tipo de coisa que sai da varredura sem ninguem notar, e
/// glifo fora da pagina nele apareceria como uma fileira de interrogacoes
/// voando pelo ceu do jardim.
fn dragon_art() -> Vec<AsciiArt> {
    let skin = |art| AsciiArt::tinted(art, &JADE_SKIN, palette::SCENE_JADE);
    let mut art = vec![
        skin(DRAGON_HEAD),
        skin(DRAGON_SCALE),
        skin(DRAGON_LIMB),
        skin(DRAGON_TAIL),
    ];
    for &(_, nodes, _) in &WHISKERS {
        art.extend((0..nodes).map(|i| whisker_art(i, nodes)));
    }
    art
}

/// Todas as celulas do fundo de uma cena: composicao parada, quadros
/// animados e o bicho. E por aqui que as varreduras passam, para nao
/// existir arte de fundo que escapa de conferencia so por se mexer.
fn every_cell(scene: Scene) -> Vec<crate::ascii::art::Cell> {
    let live = if scene == Scene::DragonGarden {
        dragon_art()
    } else {
        Vec::new()
    };
    panels(scene)
        .into_iter()
        .map(|panel| panel.art)
        .chain(reels(scene).into_iter().flat_map(|reel| reel.frames))
        .chain(live)
        .flat_map(|art| art.cells)
        .collect()
}

/// Desenha a composicao parada de um tema num grid de texto.
///
/// E o unico jeito de conferir fundo sem abrir o jogo: um `Panel` fora do
/// lugar nao quebra nada, nao falha teste nenhum, e so aparece como uma
/// montanha atravessada na tela.
const PREVIEW_COLS: usize = 160;
const PREVIEW_ROWS: usize = 30;

fn preview(scene: Scene) -> String {
    let mut grid = vec![vec![' '; PREVIEW_COLS]; PREVIEW_ROWS];

    // Do plano mais fundo para o mais raso, que e a ordem em que a tela
    // desenha: assim o preview mostra quem cobre quem, e nao a soma de
    // tudo. Uma peca escondida atras de outra e trabalho jogado fora, e o
    // preview so denuncia isso se respeitar a profundidade.
    let mut ordered = panels(scene);
    ordered.sort_by(|a, b| b.depth.total_cmp(&a.depth));

    for panel in ordered {
        let size = panel.art.size();
        let left = panel.at.x - size.x * 0.5;
        let top = if panel.foot {
            panel.at.y + size.y
        } else {
            panel.at.y + size.y * 0.5
        };
        for cell in &panel.art.cells {
            let x = ((left + cell.col as f32 * CELL.x + ARENA_HALF_W) / CELL.x).round();
            let y = ((ARENA_HALF_H - top + cell.row as f32 * CELL.y) / CELL.y).round();
            if x < 0.0 || y < 0.0 || x >= PREVIEW_COLS as f32 || y >= PREVIEW_ROWS as f32 {
                continue;
            }
            grid[y as usize][x as usize] = crate::ascii::cp437::glyph_char(cell.glyph);
        }
    }

    rendered(&grid)
}

fn rendered(grid: &[Vec<char>]) -> String {
    grid.iter()
        .map(|row| row.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Carimba no grid uma peca que ja girou, com o giro e a escala que ela
/// tem agora.
///
/// Refaz a conta que o `rebuild_glyphs` faz na tela. E a unica forma de o
/// preview mostrar o dragao como ele aparece de verdade -- desenha-lo pela
/// arte parada mostraria um bicho que nunca virou a cabeca, que e
/// exatamente o que esta secao existe para nao ser.
fn brand(grid: &mut [Vec<char>], art: &AsciiArt, flip: bool, at: Vec2, spin: Quat, size: Vec3) {
    let art = if flip { art.mirrored() } else { art.clone() };
    let span = art.size();
    let turn = Vec2::from_angle(spin.to_scaled_axis().z);
    for cell in &art.cells {
        let local = Vec2::new(
            (cell.col as f32 + 0.5) * CELL.x - span.x * 0.5,
            span.y * 0.5 - (cell.row as f32 + 0.5) * CELL.y,
        ) * size.truncate();
        let world = at + turn.rotate(local);
        let x = ((world.x + ARENA_HALF_W) / CELL.x).round();
        let y = ((ARENA_HALF_H - world.y) / CELL.y).round();
        if x < 0.0 || y < 0.0 || x >= PREVIEW_COLS as f32 || y >= PREVIEW_ROWS as f32 {
            continue;
        }
        grid[y as usize][x as usize] = crate::ascii::cp437::glyph_char(cell.glyph);
    }
}

/// A silhueta do cone tem que ser um cone.
///
/// Ela e gerada justamente porque desenhar treze linhas que alargam sempre
/// o mesmo tanto e o tipo de coisa que sai com um degrau no meio -- e um
/// degrau numa encosta le como erro de desenho, nao como rocha.
#[test]
fn o_cone_do_vulcao_alarga_sempre_igual() {
    let art = cone(palette::IRON, palette::EMBER);
    assert_eq!(art.rows, CONE_ROWS);

    let mut anterior = None;
    for row in 0..CONE_ROWS {
        let linha: Vec<u16> = art
            .cells
            .iter()
            .filter(|c| c.row == row)
            .map(|c| c.col)
            .collect();
        let largura = linha.iter().max().unwrap() - linha.iter().min().unwrap() + 1;
        let esperada = CONE_TOP + row * CONE_FLARE * 2;
        assert_eq!(largura, esperada, "linha {row} saiu do compasso");
        if let Some(antes) = anterior {
            assert!(largura > antes, "linha {row} nao alargou");
        }
        anterior = Some(largura);
    }
}

/// A boca tem que ficar aberta, e o brilho tem que caber exatamente nela.
///
/// Uma celula de rocha sobrando ali desenha por cima da lava, e a montanha
/// apaga sozinha.
#[test]
fn a_cratera_e_um_buraco_do_tamanho_do_brilho() {
    let art = cone(palette::IRON, palette::EMBER);
    let glow = AsciiArt::solid(CRATER_GLOW, palette::MAGMA);
    assert_eq!(glow.cols, CRATER_COLS, "o brilho nao cabe na boca");
    assert_eq!(glow.rows, CRATER_ROWS);

    for row in 0..CRATER_ROWS {
        let largura = CONE_TOP + row * CONE_FLARE * 2;
        let mouth = (largura - CRATER_COLS) / 2;
        let lead = (art.cols - largura) / 2;
        for col in mouth..mouth + CRATER_COLS {
            assert!(
                !art.cells
                    .iter()
                    .any(|c| c.row == row && c.col == lead + col),
                "a cratera esta tapada em {row},{col}"
            );
        }
    }
}

/// Duas celulas no mesmo lugar sao um carimbo fora do lugar.
///
/// Elas desenham uma por cima da outra, no mesmo Z: o que aparece e sorteio
/// da ordem de spawn, e o desenho muda sozinho entre uma partida e outra.
#[test]
fn nenhum_carimbo_pisa_em_cima_de_outro() {
    for scene in SCENES {
        for panel in panels(scene) {
            let mut lugares: Vec<(u16, u16)> =
                panel.art.cells.iter().map(|c| (c.col, c.row)).collect();
            lugares.sort_unstable();
            let antes = lugares.len();
            lugares.dedup();
            assert_eq!(antes, lugares.len(), "{scene:?}: carimbo sobreposto");
        }
    }
}

/// Nada do fundo pode nascer enterrado.
///
/// O terreno desenha na frente, entao uma peca inteira abaixo da linha do
/// chao custa entidade e nunca aparece. Nenhum teste pegava isso porque
/// nada quebra: o cano de escoamento da fabrica e o rio de lava do vulcao
/// passaram a primeira versao inteira debaixo do piso.
#[test]
fn nenhuma_peca_nasce_enterrada() {
    for scene in SCENES {
        for panel in panels(scene) {
            let size = panel.art.size();
            let top = panel.at.y + if panel.foot { size.y } else { size.y * 0.5 };
            assert!(
                top > GROUND,
                "{scene:?}: peca de {size:?} em {:?} fica toda embaixo do chao",
                panel.at
            );
        }
    }
}

/// O fundo nao pode nascer fora da tela.
///
/// Uma peca centrada onde ninguem ve e trabalho jogado fora que nada
/// denuncia -- ela simplesmente nao aparece.
#[test]
fn todo_plano_encosta_na_tela() {
    for scene in SCENES {
        for panel in panels(scene) {
            let size = panel.art.size();
            let low = panel.at.x - size.x * 0.5;
            let high = panel.at.x + size.x * 0.5;
            assert!(
                high > -ARENA_HALF_W && low < ARENA_HALF_W,
                "{scene:?}: peca de {size:?} em {:?} esta fora da tela",
                panel.at
            );
        }
    }
}

/// Um plano largo tem que cobrir a tela mesmo no deslize maximo.
///
/// Quem atravessa a tela e cenario de ponta a ponta: serra, viga, cano,
/// chao derretido. Se ele tiver so a largura da tela, a briga andar para um
/// canto abre um buraco de vazio no outro -- e isso acontece exatamente
/// quando os dois jogadores estao no mesmo lado, que e quando alguem esta
/// olhando para la.
#[test]
fn o_cenario_largo_cobre_a_tela_no_deslize_maximo() {
    for scene in SCENES {
        for panel in panels(scene) {
            let half = panel.art.size().x * 0.5;
            if half < ARENA_HALF_W * 0.5 {
                continue;
            }
            let drift = REACH.x * panel.depth;
            assert!(
                panel.at.x - half + drift <= -ARENA_HALF_W
                    && panel.at.x + half - drift >= ARENA_HALF_W,
                "{scene:?}: plano de {} colunas em {:?} descobre a borda",
                panel.art.cols,
                panel.at
            );
        }
    }
}

/// Cada chamine tem que ter tanque embaixo dela.
///
/// A boca de fumaca e uma entidade a parte da arte do tanque. Enquanto os
/// dois foram numeros escritos em lugares diferentes, o vapor subia de
/// tres pontos do ceu ao lado dos tanques e a arte continuava intacta:
/// nada para um teste pegar, e obvio na tela.
#[test]
fn a_fumaca_da_fabrica_sai_de_cima_dos_tanques() {
    let tanques: Vec<(Vec2, f32)> = panels(Scene::AcidWorks)
        .iter()
        .filter(|panel| panel.art.cols == VAT_COLS)
        .map(|panel| (panel.at, panel.at.y + panel.art.size().y))
        .collect();
    assert_eq!(tanques.len(), STACKS.len(), "sumiu um tanque do patio");

    for (x, rows) in STACKS {
        let (_, topo) = tanques
            .iter()
            .find(|(at, _)| (at.x - x).abs() < 1.0)
            .unwrap_or_else(|| panic!("a chamine em {x} fumega sobre o patio vazio"));
        assert!(
            (topo - stack_top(rows)).abs() < 1.0,
            "a chamine em {x} fica {} acima do topo do tanque",
            stack_top(rows) - topo
        );
    }
}

/// Arte fora da CP437 vira interrogacao na tela, e o jogo roda assim mesmo.
///
/// A varredura passa pela composicao ja montada, e nao pela lista de
/// strings: assim ela cobre tambem o que e gerado -- serra, cone, predio --
/// que e justamente o que ninguem pensa em conferir. Foi ela que pegou o
/// `▟` da refinaria, um glifo que a IBM nunca desenhou.
#[test]
fn a_arte_do_fundo_cabe_na_cp437() {
    use crate::ascii::cp437::glyph_index;
    let fallback = glyph_index('?') as u8;

    for scene in SCENES {
        assert!(
            every_cell(scene).iter().all(|cell| cell.glyph != fallback),
            "{scene:?}: o fundo usa glifo fora da pagina"
        );
    }

    let solto: String = loose_art()
        .join("")
        .chars()
        .chain(weather_glyphs())
        .chain(LEAVES)
        .collect();
    for ch in solto.chars().filter(|c| *c != '\n') {
        assert_ne!(
            glyph_index(ch) as u8,
            fallback,
            "U+{:04X} {ch:?} nao existe na pagina",
            ch as u32
        );
    }
}

/// Fundo usa uma gama propria: as cores de jogador/perigo ficam livres
/// para continuar legiveis mesmo quando a silhueta cruza um landmark.
#[test]
fn o_fundo_nao_rouba_as_cores_de_gameplay() {
    let reserved = [
        palette::BONE,
        palette::P1,
        palette::P2,
        palette::P3,
        palette::P4,
        palette::BLOOD,
        palette::GOLD,
        palette::MAGMA,
        palette::EMBER,
        palette::TOXIC,
        palette::MOSS,
        palette::JADE,
    ];

    for scene in SCENES {
        assert!(
            every_cell(scene)
                .iter()
                .all(|cell| !reserved.contains(&cell.color)),
            "{scene:?}: landmark usa cor reservada ao gameplay"
        );
    }
    for theme in THEMES {
        assert!(
            weather(theme)
                .colors
                .iter()
                .all(|color| !reserved.contains(color)),
            "{theme:?}: clima usa cor reservada ao gameplay"
        );
    }
}

/// Os glifos de todo clima do jogo.
fn weather_glyphs() -> Vec<char> {
    THEMES
        .into_iter()
        .flat_map(|theme| weather(theme).glyphs.to_vec())
        .collect()
}

/// Os dois veios de lava tem que descer inteiros.
///
/// Eles saem de uma conta sobre a largura da linha, e uma conta que caia no
/// bordo do cone perde a celula para a pedra: o veio some no meio da
/// encosta e volta embaixo, como se a montanha piscasse.
#[test]
fn a_lava_desce_a_encosta_inteira() {
    let art = cone(palette::IRON, palette::EMBER);
    for row in CRATER_ROWS..CONE_ROWS {
        let brasas = art
            .cells
            .iter()
            .filter(|cell| cell.row == row && cell.color == palette::EMBER)
            .count();
        assert_eq!(brasas, VEINS.len(), "a lava sumiu na linha {row}");
    }
}

/// O dragao tem que ser um bicho so.
///
/// Cabeca e corpo nao se encostam mais por coincidencia de coordenada:
/// o corpo le o rastro que a cabeca deixou. Isso troca o modo de errar,
/// nao acaba com ele -- basta o passo do rastro deixar de casar com o vao
/// entre vertebras para o corpo abrir buraco ou se sobrepor, e nada no
/// jogo reclama: o bicho so fica pontilhado.
#[test]
fn o_dragao_e_um_bicho_so() {
    let dragon = Dragon::new();

    // O rastro tem que cobrir o bicho inteiro mais a folga da tangente do
    // rabo. Curto, o rabo gruda no ultimo ponto gravado e para de andar.
    let precisa = NECK + SCALES as f32 * SCALE_SPAN;
    assert!(
        TRAIL_LEN as f32 * TRAIL_STEP >= precisa,
        "o rastro cobre {} e o bicho mede {precisa}",
        TRAIL_LEN as f32 * TRAIL_STEP
    );

    // Vertebra vizinha fica a um vao de distancia -- nem mais, senao abre
    // buraco; nem menos, senao o corpo engorda sozinho na curva.
    for n in 1..SCALES {
        let (at, _) = dragon.joint(n);
        let (antes, _) = dragon.joint(n - 1);
        let vao = at.distance(antes);
        assert!(
            (vao - SCALE_SPAN).abs() <= SCALE_SPAN * 0.35,
            "entre a vertebra {} e a {n} sobram {vao}, e nao {SCALE_SPAN}",
            n - 1
        );
    }

    // E a primeira encosta no pescoco, e nao fica pendurada atras dele.
    let (pescoco, _) = dragon.joint(0);
    let corpo = pescoco.distance(dragon.at);
    assert!(
        corpo <= NECK + SCALE_SPAN,
        "a primeira vertebra nasce a {corpo} da cabeca"
    );

    // E o corpo afina da cabeca para o rabo. Ao contrario, o bicho fica
    // com a cabeca espetada na ponta fina.
    assert!(
        girth(0) > girth(SCALES - 1) * 2.0,
        "o rabo esta tao grosso quanto o pescoco"
    );
}

/// A boca de onde sai o sopro tem que ser a boca do desenho, dos dois
/// lados.
///
/// Ela e uma coordenada a parte da arte, e agora a cabeca gira e espelha.
/// Espelhar nao e girar meia volta: quem esquece a inversao do `y` ganha um
/// dragao que cospe fogo pela nuca em metade do oito -- e nada no jogo
/// reclama.
#[test]
fn o_sopro_sai_da_boca_do_dragao() {
    let head = AsciiArt::tinted(DRAGON_HEAD, &JADE_SKIN, palette::SCENE_JADE);
    let size = head.size();

    // Onde cada celula do desenho cai, dado um giro e um espelho.
    let desenho = |dragon: &Dragon| -> Vec<Vec2> {
        head.cells
            .iter()
            .map(|cell| {
                let local = Vec2::new(
                    (cell.col as f32 + 0.5) * CELL.x - size.x * 0.5,
                    size.y * 0.5 - (cell.row as f32 + 0.5) * CELL.y,
                );
                dragon.on_head(local)
            })
            .collect()
    };

    for (nome, facing, flip) in [
        ("olhando para a direita", 0.0, false),
        ("olhando para a esquerda", std::f32::consts::PI, true),
        ("subindo de lado", 0.9, false),
    ] {
        let dragon = Dragon {
            at: Vec2::new(40.0, 90.0),
            facing,
            flip,
            ..Dragon::new()
        };
        let maw = dragon.maw();
        let perto = desenho(&dragon)
            .iter()
            .map(|at| at.distance(maw))
            .fold(f32::INFINITY, f32::min);
        // Uma celula e meia: a boca mora no vao entre as duas mandibulas,
        // entao ela nao cai em cima de celula nenhuma -- so perto.
        assert!(
            perto <= CELL.y * 1.5,
            "{nome}: a boca esta a {perto} da celula mais proxima do desenho"
        );

        // E ela fica na frente da cabeca, nunca atras do craneo.
        let frente = (maw - dragon.at).dot(Vec2::from_angle(facing));
        assert!(
            frente > size.x * 0.25,
            "{nome}: a boca saiu para tras da cabeca"
        );
    }
}

/// O bicho tem que voar, cuspir fogo na pista e voltar.
///
/// E o unico teste que roda o dragao de verdade. Sem ele, um estado que
/// nunca fecha -- uma rasante que nao chega no alvo, um `Roost` que nao
/// reencontra o oito -- deixaria o dragao parado no canto da tela pelo
/// resto da partida, com todos os outros testes verdes.
#[test]
fn o_dragao_voa_e_varre_a_pista() {
    let mut app = App::new();
    app.init_resource::<Time>()
        .init_resource::<Focus>()
        .add_message::<Shake>()
        .add_systems(Startup, |mut commands: Commands| {
            hatch_dragon(&mut commands, Scene::DragonGarden)
        })
        .add_systems(
            Update,
            (fly_dragon, coil_dragon, wave_whiskers, fly_jade_flames).chain(),
        );
    app.update();

    // Meio minuto em passos de um quadro: mais que o ciclo inteiro, e sem
    // um salto unico que faria a rasante comecar e acabar entre dois
    // quadros.
    let mut visto = Vec::new();
    let mut longe = Vec2::ZERO;
    let mut sumiu = 0usize;
    let mut meio = false;
    let mut chao = f32::INFINITY;
    for _ in 0..1800 {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(16));
        app.update();

        let world = app.world_mut();
        let (mood, at) = world
            .query::<&Dragon>()
            .iter(world)
            .map(|dragon| (dragon.mood, dragon.at))
            .next()
            .expect("o dragao sumiu do jardim");
        if !visto.contains(&mood) {
            visto.push(mood);
        }
        longe = longe.max(at.abs());
        // Enrolado ele nunca sai da moldura -- e o estado em que ele passa
        // quase todo o tempo, e um dragao escondido atras da borda nao e
        // cenario de coisa nenhuma.
        if mood == Mood::Coiled && (at.x.abs() > ARENA_HALF_W || at.y.abs() > ARENA_HALF_H) {
            sumiu += 1;
        }
        // A rasante, essa sim, entra e sai por fora -- mas tem que passar
        // pelo meio da arena no caminho, senao o fogo cai onde ninguem
        // esta brigando.
        meio |= mood == Mood::Dive && at.x.abs() < 200.0;

        let world = app.world_mut();
        chao = world
            .query::<(&JadeFlame, &Parallax)>()
            .iter(world)
            .map(|(_, plane)| plane.home.y)
            .fold(chao, f32::min);
    }

    for mood in [Mood::Coiled, Mood::Rise, Mood::Dive, Mood::Roost] {
        assert!(visto.contains(&mood), "o dragao nunca chegou em {mood:?}");
    }
    assert_eq!(
        sumiu, 0,
        "o dragao passou {sumiu} quadros enrolado fora da tela (foi ate {longe:?})"
    );
    assert!(meio, "a rasante nao passou pelo meio da arena");
    // E o sopro tem que chegar na pista. Fogo que morre no ar e fumaca.
    assert!(
        chao < GROUND + 60.0,
        "o fogo da rasante parou em y = {chao}, e o chao esta em {GROUND}"
    );
    // A curva de entrada e saida pode passar da borda, mas nao pode virar
    // fuga: se a direcao divergir, o bicho some e nunca mais volta.
    assert!(
        longe.x < ARENA_HALF_W * 2.0 && longe.y < ARENA_HALF_H * 1.6,
        "o dragao foi parar em {longe:?}"
    );
}

/// O bigode fica preso no focinho e nao estica.
///
/// E fisica solta pendurada num ponto que anda depressa: sem a restricao
/// -- ou com passadas de menos -- o fio arrebenta na primeira rasante e
/// fica atravessado na tela ate o fim da partida.
#[test]
fn o_bigode_fica_preso_no_focinho() {
    let mut app = App::new();
    app.init_resource::<Time>()
        .init_resource::<Focus>()
        .add_message::<Shake>()
        .add_systems(Startup, |mut commands: Commands| {
            hatch_dragon(&mut commands, Scene::DragonGarden)
        })
        .add_systems(Update, (fly_dragon, wave_whiskers).chain());
    app.update();

    for _ in 0..900 {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(16));
        app.update();
    }

    // A cabeca, remontada com o que importa: e dela que sai a raiz de cada
    // fio, e so `on_head` sabe resolver giro e espelho.
    let world = app.world_mut();
    let (at, facing, flip) = world
        .query::<&Dragon>()
        .iter(world)
        .map(|dragon| (dragon.at, dragon.facing, dragon.flip))
        .next()
        .expect("o dragao sumiu do jardim");
    let cabeca = Dragon {
        at,
        facing,
        flip,
        ..Dragon::new()
    };

    let world = app.world_mut();
    let mut fios: Vec<(u8, u16, f32, Vec2)> = world
        .query::<(&Whisker, &Parallax)>()
        .iter(world)
        .map(|(node, plane)| (node.chain, node.index, node.link, plane.home))
        .collect();
    fios.sort_by_key(|fio| (fio.0, fio.1));
    assert_eq!(
        fios.len(),
        WHISKERS.iter().map(|(_, nodes, _)| nodes).sum::<usize>(),
        "sumiu no de bigode"
    );

    let mut antes: Option<(u8, Vec2)> = None;
    for (chain, index, link, at) in fios {
        if index == 0 {
            let raiz = cabeca.on_head(WHISKERS[chain as usize].0);
            assert!(
                at.distance(raiz) < 0.5,
                "o bigode {chain} soltou do focinho: raiz em {at:?}, focinho em {raiz:?}"
            );
        } else if let Some((anterior, lead)) = antes {
            assert_eq!(anterior, chain, "os fios se misturaram");
            let elo = at.distance(lead);
            assert!(
                (elo - link).abs() < 0.5,
                "o elo {index} do bigode {chain} mede {elo}, e nao {link}"
            );
        }
        antes = Some((chain, at));
    }
}

/// Um olho no dragao em movimento, quadro a quadro:
/// `cargo test olhar_o_dragao -- --nocapture --ignored`.
///
/// O preview parado nao serve mais para ele: um bicho cuja graca inteira e
/// a curva que ele desenha nao se confere numa foto de um instante so.
#[test]
#[ignore = "so para olhar"]
fn olhar_o_dragao() {
    let mut app = App::new();
    app.init_resource::<Time>()
        .init_resource::<Focus>()
        .add_message::<Shake>()
        .add_systems(Startup, |mut commands: Commands| {
            hatch_dragon(&mut commands, Scene::DragonGarden)
        })
        .add_systems(
            Update,
            (fly_dragon, coil_dragon, wave_whiskers, fly_jade_flames).chain(),
        );
    app.update();

    let fundo: Vec<Vec<char>> = preview(Scene::DragonGarden)
        .lines()
        .map(|line| line.chars().collect())
        .collect();

    for quadro in 0..1500u32 {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(16));
        app.update();
        // Um quadro a cada segundo e meio: o bastante para ver o oito, a
        // subida e a rasante sem imprimir mil telas iguais.
        if !quadro.is_multiple_of(90) {
            continue;
        }

        let world = app.world_mut();
        let mood = world
            .query::<&Dragon>()
            .iter(world)
            .map(|dragon| dragon.mood)
            .next()
            .expect("o dragao sumiu do jardim");
        let world = app.world_mut();
        let pecas: Vec<(AsciiArt, bool, Vec2, Quat, Vec3)> = world
            .query::<(&AsciiSprite, &Parallax, &Transform)>()
            .iter(world)
            .map(|(sprite, plane, transform)| {
                (
                    sprite.art.clone(),
                    sprite.flip_x,
                    plane.home,
                    transform.rotation,
                    transform.scale,
                )
            })
            .collect();

        let mut grid = fundo.clone();
        for (art, flip, at, spin, size) in pecas {
            brand(&mut grid, &art, flip, at, spin, size);
        }
        println!(
            "\n=== {:>5.1}s  {mood:?} ===\n{}",
            quadro as f32 / 60.0,
            rendered(&grid)
        );
    }
}

/// A serra desce ate a base: pico solto no ar nao e montanha.
#[test]
fn a_serra_e_macica_por_baixo() {
    let art = ridge(&[2, 5, 3], 24, palette::COAL);
    for col in 0..art.cols {
        let coluna: Vec<u16> = art
            .cells
            .iter()
            .filter(|c| c.col == col)
            .map(|c| c.row)
            .collect();
        if coluna.is_empty() {
            continue;
        }
        let topo = *coluna.iter().min().unwrap();
        assert_eq!(
            coluna.len() as u16,
            art.rows - topo,
            "a coluna {col} tem buraco"
        );
        assert!(coluna.contains(&(art.rows - 1)), "a coluna {col} flutua");
    }
}

/// A montanha tem que estourar sozinha, e o estouro tem que sair da boca.
///
/// Composicao parada e o que os outros testes conferem; a erupcao e a
/// unica coisa aqui que so existe em movimento. Sem este teste ela podia
/// parar de acontecer -- um timer que nunca vira, uma bomba que nasce e
/// morre no mesmo quadro -- com todos os outros verdes.
#[test]
fn o_vulcao_estoura_sozinho() {
    let mut app = App::new();
    app.init_resource::<Time>()
        .add_message::<Shake>()
        .add_systems(Startup, |mut commands: Commands| {
            vents(&mut commands, Scene::Caldera)
        })
        .add_systems(Update, (run_vents, drift_smoke, fly_bombs).chain());
    app.update();

    // Doze segundos em passos de um quadro: mais que o relogio da erupcao,
    // e sem um salto unico que faria a bomba nascer e sumir junto.
    for _ in 0..200 {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(60));
        app.update();
    }

    let world = app.world_mut();
    let fumaca: Vec<Vec2> = world
        .query::<(&Smoke, &Parallax)>()
        .iter(world)
        .map(|(_, plane)| plane.home)
        .collect();
    assert!(!fumaca.is_empty(), "a cratera parou de fumegar");
    assert!(
        fumaca.iter().any(|at| at.y > CRATER.y + CELL.y),
        "nenhuma baforada subiu: a coluna nasce e fica parada na boca"
    );

    let world = app.world_mut();
    let bombas = world.query::<&LavaBomb>().iter(world).count();
    assert!(bombas > 0, "doze segundos e o vulcao nao cuspiu nada");
}

/// Um olho no fundo montado, quando alguem quiser conferir a composicao:
/// `cargo test olhar_o_fundo -- --nocapture --ignored`.
#[test]
#[ignore = "so para olhar"]
fn olhar_o_fundo() {
    for scene in SCENES {
        println!("\n=== {scene:?} ===\n{}", preview(scene));
    }
}

/// A ventania tem rajada e calmaria, e nunca sopra ao contrario.
///
/// Vento constante nao e ventania -- e ventilador, e a diferenca nao aparece
/// em nenhuma foto: os dois enchem a tela de folha andando. So olhando a
/// forca ao longo do tempo da para ver se existe rajada.
///
/// O piso em zero e o que segura o desenho: a soma tem uma senoide livre por
/// cima, e se ela puxasse o total para baixo de zero a folha andaria de re
/// no meio do sopro.
#[test]
fn a_ventania_tem_rajada_e_calmaria() {
    let mut pico: f32 = 0.0;
    let mut calmo = f32::INFINITY;
    // Um minuto em passos de quadro: mais que os dois ciclos lentos.
    for frame in 0..3600 {
        let forca = gale(frame as f32 / 60.0);
        assert!(
            forca > 0.0,
            "aos {:.1}s o vento inverteu ({forca:.2})",
            frame as f32 / 60.0
        );
        pico = pico.max(forca);
        calmo = calmo.min(forca);
    }
    assert!(pico >= 4.0, "a rajada mais forte foi de so {pico:.2}");
    assert!(
        calmo <= 1.5,
        "nunca houve calmaria: o vento mais fraco foi {calmo:.2}"
    );
}

/// A folha da tela inicial nunca sai do quadro nem para de andar.
///
/// A moldura da ventania nao e a da arena, e essa e a armadilha: `GALE_HALF`
/// tem 440 de meia altura contra os 240 de `ARENA_HALF_H`, porque o menu
/// desenha bem abaixo do que a arena ocupa. Semeado pela caixa da arena, o
/// campo deixaria a metade de baixo do painel de teclas sem uma folha, e a
/// tela continuaria parecendo certa -- so faltando vento onde ninguem
/// pensou em olhar.
#[test]
fn a_folha_do_menu_nunca_sai_do_quadro() {
    let mut app = App::new();
    app.init_resource::<Time>().add_systems(Update, blow_gale);
    app.world_mut()
        .run_system_once(|mut commands: Commands| seed_gale(&mut commands))
        .unwrap();

    let mut andou = 0.0f32;
    let mut baixo = f32::INFINITY;
    // Vinte segundos: mais de duas rajadas inteiras.
    for _ in 0..1200 {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(16));
        app.update();

        let world = app.world_mut();
        for (transform, _) in world.query::<(&Transform, &Leaf)>().iter(world) {
            let at = transform.translation.truncate();
            assert!(
                at.x.abs() <= GALE_HALF.x + 40.0 && at.y.abs() <= GALE_HALF.y + 40.0,
                "folha escapou para {at:?}, e o quadro e {GALE_HALF:?}"
            );
            andou = andou.max(at.x.abs());
            baixo = baixo.min(at.y);
        }
    }

    // E ela cobre o quadro: um campo que so vive no meio da tela deixa as
    // bordas -- onde o painel de teclas mora -- secas.
    assert!(
        andou > GALE_HALF.x * 0.8,
        "nenhuma folha chegou perto da borda lateral ({andou:.0})"
    );
    assert!(
        baixo < -GALE_HALF.y * 0.8,
        "nenhuma folha desceu ate a base do menu ({baixo:.0})"
    );
}

/// Um olho na ventania, quadro a quadro:
/// `cargo test olhar_a_ventania -- --nocapture --ignored`.
#[test]
#[ignore = "so para olhar"]
fn olhar_a_ventania() {
    const COLS: usize = 78;
    const ROWS: usize = 24;

    let mut app = App::new();
    app.init_resource::<Time>().add_systems(Update, blow_gale);
    app.world_mut()
        .run_system_once(|mut commands: Commands| seed_gale(&mut commands))
        .unwrap();

    for instante in 0..7 {
        // Um segundo e meio entre retratos: tempo de a rajada mudar de cara.
        for _ in 0..90 {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_millis(16));
            app.update();
        }
        let agora = app.world().resource::<Time>().elapsed_secs();
        let mut grid = vec![vec![' '; COLS]; ROWS];
        let world = app.world_mut();
        for (transform, sprite) in world.query::<(&Transform, &AsciiSprite)>().iter(world) {
            let at = transform.translation.truncate();
            let col = ((at.x + GALE_HALF.x) / (GALE_HALF.x * 2.0) * COLS as f32) as usize;
            let row = ((GALE_HALF.y - at.y) / (GALE_HALF.y * 2.0) * ROWS as f32) as usize;
            if let Some(cell) = grid.get_mut(row).and_then(|line| line.get_mut(col)) {
                *cell = sprite
                    .art
                    .cells
                    .first()
                    .map_or('?', |cell| glyph_char(cell.glyph));
            }
        }
        println!(
            "\n=== {agora:.1}s -- vento {:.2}x ===\n{}",
            gale(agora),
            grid.into_iter()
                .map(|line| line.into_iter().collect::<String>().trim_end().to_owned())
                .collect::<Vec<_>>()
                .join("\n")
        );
        let _ = instante;
    }
}
