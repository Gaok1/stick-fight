use super::*;

/// Margem sobre o alcance teorico. Um mapa desenhado no limite exato do
/// pulo e frustrante mesmo quando e possivel.
const SAFETY: f32 = 0.75;

/// Um no do grafo de travessia.
#[derive(Debug, Clone, Copy)]
enum Node {
    /// Superficie de apoio: uma faixa horizontal numa altura.
    Ledge { x0: f32, x1: f32, y: f32 },
    /// Corrente: uma coluna que se agarra em qualquer altura do trecho.
    Rope { x: f32, low: f32, high: f32 },
}

/// Da para ir de `a` a `b`?
///
/// Escalar corrente conta: o jogo trata isso como travessia normal, entao
/// exigir que tudo se resolva no pulo reprovaria mapas que funcionam.
fn connects(a: Node, b: Node) -> bool {
    let reachable =
        |climb: f32, gap: f32| jump_reach(climb).is_some_and(|reach| gap <= reach * SAFETY);
    match (a, b) {
        (
            Node::Ledge { x0, x1, y },
            Node::Ledge {
                x0: bx0,
                x1: bx1,
                y: by,
            },
        ) => {
            let gap = (bx0 - x1).max(x0 - bx1).max(0.0);
            // Descer e sempre mais facil que subir, entao o par so conecta
            // se o lado dificil -- a subida -- couber no pulo.
            reachable((by - y).abs(), gap)
        }
        (Node::Ledge { x0, x1, y }, Node::Rope { x, low, high })
        | (Node::Rope { x, low, high }, Node::Ledge { x0, x1, y }) => {
            // Agarra na altura mais conveniente do trecho da corrente.
            let grab = y.clamp(low, high);
            reachable(grab - y, (x0 - x).max(x - x1).max(0.0))
        }
        // Pular de corrente em corrente depende do balanco, que e dinamico
        // demais para um teste estatico prometer.
        (Node::Rope { .. }, Node::Rope { .. }) => false,
    }
}

fn footholds(level: &dyn Level) -> Vec<(f32, f32, f32)> {
    level.pieces().iter().filter_map(|p| p.foothold()).collect()
}

/// O alcance so vale como criterio se for a fisica do jogo, nao um numero
/// solto que ninguem revisita quando o pulo muda.
#[test]
fn alcance_do_pulo_bate_com_a_fisica() {
    use crate::actor::motion::{JUMP_SPEED, RUN_SPEED};
    use crate::physics::GRAVITY;

    let apex = JUMP_SPEED * JUMP_SPEED / (2.0 * GRAVITY);
    assert!(jump_reach(apex - 1.0).is_some());
    assert!(jump_reach(apex + 1.0).is_none(), "passou do topo do pulo");

    // Sem subida, o alcance e a corrida durante o arco inteiro.
    let flat = jump_reach(0.0).unwrap();
    let expected = RUN_SPEED * 2.0 * JUMP_SPEED / GRAVITY;
    assert!((flat - expected).abs() < 0.5, "arco plano deu {flat}");
}

fn graph(level: &dyn Level) -> Vec<Node> {
    level
        .pieces()
        .iter()
        .filter_map(|piece| match *piece {
            Piece::Chain { top, links } => Some(Node::Rope {
                x: top.x,
                low: top.y - (links.saturating_sub(1)) as f32 * LINK_LENGTH,
                high: top.y,
            }),
            // Teto nao e no de travessia: nao se sobe nele nem se agarra.
            Piece::Ceiling { .. } => None,
            other => other
                .foothold()
                .map(|(x0, x1, y)| Node::Ledge { x0, x1, y }),
        })
        .collect()
}

/// Nenhum patamar pode ficar ilhado.
///
/// Este e o erro que um mapa novo comete calado: a geometria aparece, o
/// jogo roda, e so quem joga descobre que nao da pra sair da plataforma --
/// ou que a arma caiu num lugar aonde ninguem chega.
#[test]
fn todo_patamar_e_alcancavel() {
    for build in CATALOG {
        let level = build();
        let nodes = graph(level.as_ref());

        // Busca em largura a partir do primeiro patamar; no fim, todo
        // apoio tem que ter sido visitado. Corrente solta e so inutil, nao
        // e erro, entao ela nao entra na cobranca.
        let start = nodes
            .iter()
            .position(|n| matches!(n, Node::Ledge { .. }))
            .expect("fase sem nenhum apoio");
        let mut seen = vec![false; nodes.len()];
        let mut queue = vec![start];
        seen[start] = true;
        while let Some(from) = queue.pop() {
            for (to, node) in nodes.iter().enumerate() {
                if !seen[to] && connects(nodes[from], *node) {
                    seen[to] = true;
                    queue.push(to);
                }
            }
        }

        let ilhados: Vec<Node> = nodes
            .iter()
            .zip(&seen)
            .filter(|(node, seen)| !**seen && matches!(node, Node::Ledge { .. }))
            .map(|(node, _)| *node)
            .collect();
        assert!(
            ilhados.is_empty(),
            "{}: patamares fora de alcance {ilhados:?}",
            level.name()
        );
    }
}

/// O teto da Arena 03 so muda o jogo se cortar o pulo de verdade.
///
/// Com folga maior que o arco do salto ele viraria decoracao, e a fase
/// perderia a unica coisa que a distingue: nas laterais nao ha jogo aereo.
#[test]
fn o_teto_da_vault_corta_o_pulo() {
    let level = Arena03;
    let sob_o_teto = level.spawn_points()[0];
    let chao = level.ground_under(sob_o_teto).expect("spawn sem chao");
    let cabeca = chao + crate::actor::body_half_height() * 2.0;

    let teto = level
        .pieces()
        .iter()
        .find_map(|piece| match *piece {
            Piece::Ceiling { bottom, cols, .. }
                if (sob_o_teto.x - bottom.x).abs() <= cols as f32 * CELL.x * 0.5 =>
            {
                Some(bottom.y)
            }
            _ => None,
        })
        .expect("o spawn lateral nao esta sob teto nenhum");

    use crate::actor::motion::JUMP_SPEED;
    let arco = JUMP_SPEED * JUMP_SPEED / (2.0 * crate::physics::GRAVITY);
    let folga = teto - cabeca;

    assert!(folga > 0.0, "o teto encosta na cabeca de quem esta em pe");
    assert!(
        folga < arco,
        "o teto deixa {folga} de folga e o pulo sobe {arco}: ele nao corta nada"
    );
}

/// A percepcao de chao tem que enxergar os buracos reais dos mapas.
///
/// Os testes do adversario montam a percepcao a mao; este confere que a
/// geometria de verdade produz a mesma coisa. Este e o vao da Arena 01 que
/// o fazia perder sozinho.
#[test]
fn o_buraco_da_arena_01_aparece_para_quem_percebe() {
    let level = level_at(0);
    // Em pe no trecho da direita, pes em -170, centro em -140.
    let em_pe = Vec2::new(300.0, -140.0);
    assert!(
        level.ground_under(em_pe).is_some_and(|y| y == -170.0),
        "o chao onde o adversario nasce sumiu"
    );
    // Dentro do vao entre os trechos, na mesma altura.
    let sobre_o_vao = Vec2::new(190.0, -140.0);
    assert_eq!(
        level.ground_under(sobre_o_vao),
        None,
        "o buraco reportou chao, e o adversario andaria para dentro dele"
    );
    // A plataforma alta em x=150 nao pode contar: ela esta acima da
    // cabeca, nao debaixo dos pes.
    assert!(level.ground_under(Vec2::new(150.0, 200.0)).is_some());
}

/// Arma que nasce sobre o vazio cai direto no buraco e some do round.
#[test]
fn toda_arma_cai_sobre_algum_patamar() {
    for build in CATALOG {
        let level = build();
        let spots = footholds(level.as_ref());
        for drop in level.drop_points() {
            assert!(
                spots
                    .iter()
                    .any(|s| drop.x >= s.0 && drop.x <= s.1 && s.2 < drop.y),
                "{}: arma largada em {drop:?} nao tem chao embaixo",
                level.name()
            );
        }
    }
}

/// Toda arena precisa caber uma sala cheia.
///
/// Sem isto, uma fase com dois pontos so aceitaria a sala antiga: o
/// terceiro e o quarto jogador nasceriam empilhados em `Vec2::ZERO`, que
/// na Arena 02 e o vazio entre as torres.
#[test]
fn toda_arena_tem_lugar_para_a_sala_cheia() {
    for build in CATALOG {
        let level = build();
        let spawns = level.spawn_points();
        assert_eq!(
            spawns.len(),
            crate::actor::MAX_PLAYERS,
            "{}: a fase nao tem lugar para todos",
            level.name()
        );

        // Dois bonecos no mesmo ponto nascem presos um no outro.
        for (at, spawn) in spawns.iter().enumerate() {
            for other in &spawns[at + 1..] {
                assert!(
                    spawn.distance(*other) > 60.0,
                    "{}: {spawn:?} e {other:?} nascem colados",
                    level.name()
                );
            }
        }
    }
}

/// Jogador que nasce sobre o vazio morre antes de encostar no chao.
#[test]
fn todo_jogador_nasce_sobre_algum_patamar() {
    for build in CATALOG {
        let level = build();
        let spots = footholds(level.as_ref());
        for spawn in level.spawn_points() {
            assert!(
                spots
                    .iter()
                    .any(|s| spawn.x >= s.0 && spawn.x <= s.1 && s.2 < spawn.y),
                "{}: jogador nasce em {spawn:?} sem chao embaixo",
                level.name()
            );
        }
    }
}

#[test]
fn cada_tema_novo_tem_tres_mapas_e_spawn_seguro() {
    let mut themes = [0; 3];
    let mut scenes = Vec::new();
    for build in &CATALOG[3..] {
        let level = build();
        themes[match level.theme() {
            Theme::Volcano => 0,
            Theme::Industrial => 1,
            Theme::Oriental => 2,
            Theme::City => panic!("mapa novo sem tema"),
        }] += 1;
        assert!(
            !scenes.contains(&level.scene()),
            "{} reutiliza a pintura de outro mapa",
            level.name()
        );
        scenes.push(level.scene());

        // Poca, fonte, mare e goteira contam igual: o que a arena precisa
        // e de algo que cobre atencao, nao de um material especifico.
        let hazards: Vec<(f32, f32)> = level
            .pieces()
            .iter()
            .filter_map(|piece| piece.menace())
            .collect();
        assert!(!hazards.is_empty(), "{}: arena sem interacao", level.name());
        for spawn in level.spawn_points() {
            assert!(
                hazards
                    .iter()
                    .all(|(x0, x1)| spawn.x < *x0 || spawn.x > *x1),
                "{}: jogador nasce dentro do perigo em {spawn:?}",
                level.name()
            );
        }
    }
    assert_eq!(themes, [3, 3, 3]);
    assert_eq!(scenes.len(), 9);
}

/// Toda fase do catalogo tem que conseguir nascer.
///
/// Os outros testes leem os dados; este monta. Um `saturating_sub` que
/// zera, um vetor de quadros vazio ou uma peca de largura zero nao
/// aparecem em lugar nenhum ate virarem entidade -- e ai o jogo fecha
/// sozinho na hora de entrar no round, que e o pior momento possivel.
#[test]
fn toda_fase_do_catalogo_monta_sem_quebrar() {
    for index in 0..CATALOG.len() {
        let level = level_at(index);
        let nome = level.name();
        let mut app = App::new();
        app.insert_resource(CurrentLevel(level)).add_systems(
            Startup,
            |mut commands: Commands, level: Res<CurrentLevel>| raise_level(&mut commands, &level),
        );
        app.update();

        let world = app.world_mut();
        let pecas = world.query::<&LevelGeometry>().iter(world).count();
        assert!(pecas > 0, "{nome} nasceu vazia");
    }
}

/// Toda a arte de perigo tem que existir na pagina de codigo.
///
/// A varredura geral de CP437 passa por poses, arsenal e nomes de fase --
/// nao pela arte que a fase gera em runtime. Um glifo fora da pagina aqui
/// vira interrogacao no meio da piscina, e o jogo roda assim mesmo.
#[test]
fn a_arte_dos_perigos_cabe_na_cp437() {
    use crate::ascii::cp437::glyph_index;
    let fallback = glyph_index('?') as u8;

    for kind in [
        HazardKind::Lava,
        HazardKind::Acid,
        HazardKind::Spikes,
        HazardKind::Jade,
    ] {
        for phase in 0..4 {
            for art in [hazard_art(kind, 9, phase), jet_art(kind, 5, 7, phase)] {
                assert!(
                    art.cells.iter().all(|cell| cell.glyph != fallback),
                    "{kind:?}: glifo fora da pagina no quadro {phase}"
                );
            }
        }
    }
}

/// A coluna que machuca tem que ser a coluna que aparece.
///
/// A zona de contato cobre a altura cheia do jorro o tempo todo em que
/// esta armada, entao ela so pode armar quando a coluna ja subiu. Armada
/// junto com o desenho, ela cobraria dano no topo enquanto a coluna ainda
/// mede um degrau -- e o jogador leva um golpe do ar.
#[test]
fn a_fonte_so_machuca_com_a_coluna_inteira_em_pe() {
    let beat = jet_beat(6.0, 0.0);
    let mut armada = 0;
    let mut visivel = 0;

    for step in 0..2000 {
        let t = step as f32 / 2000.0;
        let alta = jet_rise(t);
        if alta > 0.0 {
            visivel += 1;
        }
        if t < beat.from || t >= beat.to {
            continue;
        }
        armada += 1;
        assert!(
            (alta - 1.0).abs() < 1e-3,
            "a zona morde em t={t} com a coluna a {alta} da altura"
        );
    }
    assert!(armada > 0, "a fonte nunca arma");
    assert!(
        visivel > armada,
        "a coluna aparece e some junto com a zona: some o aviso de subida"
    );
}

/// A mare tem que percorrer exatamente o que promete, e voltar.
#[test]
fn a_mare_sobe_o_que_prometeu_e_desce_de_volta() {
    let bob = Bob {
        home: -162.0,
        rise: 6.0 * CELL.y,
        period: 10.0,
        phase: 0.0,
    };
    let at = |t: f32| {
        let wave = 0.5 - (cycle(t, bob.period, bob.phase) * std::f32::consts::TAU).cos() * 0.5;
        bob.home + bob.rise * wave
    };

    let alturas: Vec<f32> = (0..400)
        .map(|s| at(s as f32 * bob.period / 400.0))
        .collect();
    let baixa = alturas.iter().copied().fold(f32::INFINITY, f32::min);
    let cheia = alturas.iter().copied().fold(f32::NEG_INFINITY, f32::max);

    assert!(
        (baixa - bob.home).abs() < 1.0,
        "a mare baixa passa do fundo"
    );
    assert!(
        (cheia - (bob.home + bob.rise)).abs() < 1.0,
        "a mare cheia nao chega onde o mapa promete"
    );
    // Ciclo fechado: o fim do periodo tem que voltar ao comeco, senao a
    // poca anda para cima um pouco a cada volta.
    assert!((at(0.0) - at(bob.period)).abs() < 1.0, "a mare nao fecha");
}

/// Toda goteira tem que pingar sobre alguma coisa.
///
/// A gota se desmancha na altura que a peca declara. Se essa altura nao
/// for a de um apoio real, o respingo acontece no ar -- ou, pior, a gota
/// atravessa o chao e some, e a boca vira enfeite que nunca acerta nada.
#[test]
fn toda_goteira_pinga_sobre_algum_chao() {
    for build in CATALOG {
        let level = build();
        for piece in level.pieces() {
            let Piece::Drip { from, floor, .. } = *piece else {
                continue;
            };
            let chao = level
                .ground_under(from)
                .unwrap_or_else(|| panic!("{}: goteira em {from:?} pinga no vazio", level.name()));
            assert!(
                (floor - chao).abs() <= CELL.y,
                "{}: a goteira em {from:?} se desmancha em {floor} e o chao esta em {chao}",
                level.name()
            );
        }
    }
}

/// Fonte e mare nao podem nascer debaixo de um teto que as engole.
///
/// Elas medem altura em celulas, longe de onde os patamares sao escritos.
/// Uma coluna que passa do teto desenha por dentro da pedra, e a metade de
/// cima do jorro -- justamente a que ameaca -- some.
#[test]
fn nenhuma_coluna_atravessa_o_teto() {
    for build in CATALOG {
        let level = build();
        let tetos: Vec<(f32, f32, f32)> = level
            .pieces()
            .iter()
            .filter_map(|piece| match *piece {
                Piece::Ceiling { bottom, cols, .. } => {
                    let half = cols as f32 * CELL.x * 0.5;
                    Some((bottom.x - half, bottom.x + half, bottom.y))
                }
                _ => None,
            })
            .collect();

        for piece in level.pieces() {
            let Piece::Geyser { at, rows, .. } = *piece else {
                continue;
            };
            let topo = at.y + rows as f32 * CELL.y;
            for (x0, x1, y) in &tetos {
                if at.x < *x0 || at.x > *x1 {
                    continue;
                }
                assert!(
                    topo <= *y,
                    "{}: a fonte em {at:?} sobe ate {topo} e o teto esta em {y}",
                    level.name()
                );
            }
        }
    }
}

#[test]
fn lava_tem_profundidade_e_quatro_quadros_de_fluxo() {
    let frames: Vec<AsciiArt> = (0..4)
        .map(|phase| hazard_art(HazardKind::Lava, 12, phase))
        .collect();
    assert!(
        frames
            .iter()
            .all(|frame| frame.cols == 12 && frame.rows == 4)
    );
    for pair in frames.windows(2) {
        assert_ne!(pair[0], pair[1], "dois quadros da lava ficaram iguais");
    }
}

/// Onde esta cada bloco solido que subiu, para comparar duas montagens.
fn chao(app: &mut App) -> Vec<(i32, i32)> {
    let world = app.world_mut();
    let mut found: Vec<(i32, i32)> = world
        .query_filtered::<&Transform, With<Solid>>()
        .iter(world)
        .map(|t| (t.translation.x as i32, t.translation.y as i32))
        .collect();
    found.sort();
    found
}

/// Trocar de fase com a arena de pe tem que levantar a fase escolhida.
///
/// O indice muda no `Update` -- o dono mexendo no seletor da sala, ou o
/// cliente seguindo o `stage` do lobby -- e `apply_level_pick` so o traduz
/// para `CurrentLevel` no `PreUpdate` do quadro seguinte. Enquanto o
/// rebuild confiava nesse recurso, ele reerguia a **fase anterior** e ja
/// marcava `BuiltStage` com a nova: como os dois passavam a bater, nada
/// corrigia depois, e quem estava na sala corria no chao do mapa errado
/// enquanto a tela anunciava outro.
#[test]
fn trocar_de_fase_com_a_arena_de_pe_levanta_a_fase_nova() {
    let mut app = App::new();
    app.insert_resource(CurrentLevel(level_at(0)))
        .insert_resource(LevelPick(0))
        .insert_resource(BuiltStage(None))
        .add_systems(Update, rebuild_on_stage_change);

    app.update();
    let antes = chao(&mut app);
    assert!(
        !antes.is_empty(),
        "a primeira montagem nao subiu chao nenhum"
    );

    // De proposito sem passar por `apply_level_pick`: e exatamente esse
    // atraso de um quadro que o rebuild tem que aguentar sozinho.
    let alvo = 5;
    app.insert_resource(LevelPick(alvo));
    app.update();

    assert_eq!(app.world().resource::<BuiltStage>().0, Some(alvo));
    assert_eq!(
        app.world().resource::<CurrentLevel>().0.name(),
        level_name(alvo),
        "a arena continuou sendo a da fase anterior"
    );
    assert_ne!(
        chao(&mut app),
        antes,
        "a geometria no ar nao mudou junto com a fase"
    );
}
