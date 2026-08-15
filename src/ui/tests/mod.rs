use super::*;

#[test]
fn treino_spawna_a_arma_escolhida() {
    let mut app = App::new();
    app.init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<DummyBehavior>()
        .init_resource::<ShowBoxes>()
        .init_resource::<TrainingWeaponPick>()
        .add_systems(Update, training_controls);
    app.world_mut().spawn((
        Player {
            id: 0,
            color: palette::player(0),
        },
        Facing(1.0),
        Transform::default(),
    ));
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyX);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyC);

    app.update();

    assert_eq!(app.world().resource::<TrainingWeaponPick>().0, 1);
    let world = app.world_mut();
    let mut drops = world.query::<(&GroundWeapon, &Transform)>();
    let spawned: Vec<(u8, Vec2)> = drops
        .iter(world)
        .map(|(weapon, transform)| (weapon.kind, transform.translation.truncate()))
        .collect();
    assert_eq!(spawned, vec![(1, Vec2::new(48.0, 70.0))]);
}

#[test]
fn moldura_fecha_com_largura_uniforme() {
    let art = framed(&["abc", "de", SEPARATOR, "fghij"]);
    let widths: Vec<usize> = art.lines().map(|l| l.chars().count()).collect();
    assert!(
        widths.windows(2).all(|w| w[0] == w[1]),
        "linhas desalinhadas: {widths:?}"
    );
    // 5 de conteudo + 2 de espaco + 2 de borda
    assert_eq!(widths[0], 9);
}

#[test]
fn separador_vira_linha_dupla() {
    let art = framed(&["a", SEPARATOR]);
    assert!(art.contains('\u{2560}'));
    assert!(art.contains('\u{2563}'));
}

/// O realce do painel de teclas tem que achar o que pinta.
///
/// A cor entra por busca sobre o texto ja montado. Se a linha de teclas
/// mudar e o alvo do realce nao, a cor simplesmente some -- o menu volta a
/// ser branco sem nada quebrar e sem ninguem notar.
#[test]
fn todo_realce_do_menu_acha_o_alvo() {
    let owned = menu_lines();
    let lines: Vec<&str> = owned.iter().map(String::as_str).collect();
    for alvo in ["PLAYER 1", "PLAYER 2"] {
        assert!(
            locate(&lines, alvo).is_some(),
            "o realce {alvo:?} nao acha o que pintar"
        );
    }
}

/// Trocar de valor nao pode mexer no tamanho do botao.
///
/// A area de clique sai do tamanho da arte: um botao que encolhe ao mudar
/// de texto passa a ser clicavel num retangulo que nao e o que se ve, e o
/// clique cai no vizinho.
#[test]
fn botao_nao_muda_de_tamanho_ao_trocar_de_valor() {
    let mut sizes = Vec::new();
    for stage in 0..LEVEL_CATALOG.len() {
        let button = Button::new(level_name(stage), MenuAction::Stage(1))
            .width(widest_stage())
            .chosen(true);
        sizes.push(button_art(&button).size());
    }
    assert!(
        sizes.windows(2).all(|pair| pair[0] == pair[1]),
        "o seletor de fase muda de tamanho: {sizes:?}"
    );

    let mut sizes = Vec::new();
    for mode in GameMode::ALL {
        for chosen in [false, true] {
            let button = Button::new(mode.label(), MenuAction::PickMode(mode))
                .width(widest_label())
                .chosen(chosen);
            sizes.push(button_art(&button).size());
        }
    }
    assert!(
        sizes.windows(2).all(|pair| pair[0] == pair[1]),
        "o seletor de modo muda de tamanho: {sizes:?}"
    );
}

/// O seletor de lutador varre pele **e** as quatro pecas do rosto: basta um
/// nome mais longo que a celula para os botoes pularem ao trocar de opcao.
#[test]
fn seletor_de_lutador_nao_pula_ao_navegar() {
    let mut sizes = Vec::new();
    for row in 0..fighter_rows() {
        for pick in 0..skin::CATALOG.len() {
            let mut face = Face::default();
            for passo in 0..8 {
                for part in Part::CHOSEN {
                    face.cycle(part, passo % 3 - 1);
                }
                let button = Button::new(row_value(row, pick, face), MenuAction::Confirm)
                    .width(fighter_cell());
                sizes.push(button_art(&button).size());
            }
        }
    }
    assert!(
        sizes.windows(2).all(|pair| pair[0] == pair[1]),
        "a tela de lutador muda de tamanho: {sizes:?}"
    );
}

/// Cada linha do seletor tem que ter nome e valor -- os dois vindos do
/// catalogo de verdade, e nao de uma lista paralela que envelhece sozinha.
#[test]
fn toda_linha_do_seletor_tem_nome_e_valor() {
    for row in 0..fighter_rows() {
        assert!(!row_label(row).is_empty(), "linha {row} sem nome");
        assert!(
            !row_value(row, 0, Face::default()).is_empty(),
            "linha {row} sem valor"
        );
        // E girar tem que mudar alguma coisa, senao a linha e enfeite.
        let (mut pick, mut face) = (0usize, Face::default());
        row_cycle(row, &mut pick, &mut face, 1);
        assert_ne!(
            (pick, face),
            (0, Face::default()),
            "girar a linha {row} nao mudou nada"
        );
    }
}

/// Dois botoes nao podem dividir o mesmo pedaco de tela.
///
/// Eles sao entidades separadas, entao nada no jogo reclamaria: o unico
/// sintoma seria um clique que aciona o botao errado.
#[test]
fn os_botoes_do_menu_nao_se_sobrepoem() {
    let stage_x = (widest_stage() as f32 * crate::ascii::CELL.x) * 0.5 + 32.0;
    let mut caixas: Vec<Rect> = (0..GameMode::ALL.len())
        .map(|at| {
            let button =
                Button::new(GameMode::ALL[at].label(), MenuAction::Play).width(widest_label());
            caixa(button_art(&button).size(), mode_slot(at))
        })
        .collect();
    caixas.push(caixa(
        button_art(&Button::new(level_name(0), MenuAction::Stage(1)).width(widest_stage())).size(),
        Vec2::new(0.0, 64.0),
    ));
    for x in [-stage_x, stage_x] {
        caixas.push(caixa(
            button_art(&Button::new(LEFT, MenuAction::Stage(1))).size(),
            Vec2::new(x, 64.0),
        ));
    }

    for (a, esquerda) in caixas.iter().enumerate() {
        for direita in &caixas[a + 1..] {
            assert!(
                esquerda.min.x > direita.max.x
                    || direita.min.x > esquerda.max.x
                    || esquerda.min.y > direita.max.y
                    || direita.min.y > esquerda.max.y,
                "dois botoes ocupam o mesmo lugar: {esquerda:?} e {direita:?}"
            );
        }
    }
}

fn caixa(size: Vec2, at: Vec2) -> Rect {
    Rect::from_center_size(at, size + Vec2::new(8.0, 10.0))
}

/// A area de clique tem que cobrir o que esta desenhado.
#[test]
fn o_clique_cai_onde_o_botao_esta() {
    let button = Button::new("START MATCH", MenuAction::Play).width(13);
    let sprite = AsciiSprite::new(button_art(&button));
    let transform = Transform::from_translation(Vec3::new(430.0, 300.0, 0.0));
    let rect = button_rect(&transform, &sprite);

    assert!(
        rect.contains(Vec2::new(430.0, 300.0)),
        "o centro nao acerta"
    );
    assert!(
        rect.contains(Vec2::new(430.0 + button_art(&button).size().x * 0.4, 300.0)),
        "a beirada do texto nao acerta"
    );
    assert!(
        !rect.contains(Vec2::new(430.0, 300.0 + 40.0)),
        "o clique acerta bem longe do botao"
    );
}

/// O painel da sala tem que listar os quatro lugares.
#[test]
fn o_painel_da_sala_marca_o_proprio_lugar() {
    let session = OnlineSession::default();
    let art = lobby_art(&session);
    assert!(
        art.rows >= MAX_PLAYERS as u16,
        "a sala nao lista os lugares"
    );
}

/// A sala oferece as mesmas acoes sempre; o que muda e quais estao acesas.
#[test]
fn a_sala_nunca_esconde_um_botao() {
    let fora = OnlineSession::default();
    let acoes: Vec<MenuAction> = lobby_buttons(&fora).iter().map(|(a, ..)| *a).collect();
    assert_eq!(acoes.len(), 5);
    // Fora de uma sala da pra criar e procurar, mas nao convidar nem
    // comecar.
    let ligados: Vec<&str> = lobby_buttons(&fora)
        .iter()
        .filter(|(_, _, on)| *on)
        .map(|(_, label, _)| *label)
        .collect();
    assert_eq!(ligados, vec!["CREATE ROOM", "FIND ROOM", "LEAVE"]);
}
