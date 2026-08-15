use super::*;

fn mirando(aim: Vec2) -> Intent {
    Intent {
        aim,
        ..Intent::default()
    }
}

/// O soco sai na linha do cursor, e nao so para o lado em que o corpo olha.
#[test]
fn o_soco_vai_para_onde_o_cursor_aponta() {
    let olhando = Facing(1.0);
    for aim in [Vec2::Y, Vec2::NEG_Y, Vec2::new(-0.6, 0.8).normalize()] {
        let dir = strike_dir(MeleeKind::Chain, &mirando(aim), &olhando);
        assert!(
            (dir - aim).length() < 0.001,
            "o soco ignorou a mira {aim:?} e foi para {dir:?}"
        );
    }
}

/// Sem mouse -- o jogador 2, a CPU, quem so tem teclado -- nada muda.
///
/// A mira vazia vale como "para onde estou olhando", entao o mesmo codigo
/// serve os dois sem um caminho separado que ninguem testa.
#[test]
fn quem_nao_mira_continua_socando_para_a_frente() {
    for lado in [-1.0f32, 1.0] {
        let dir = strike_dir(MeleeKind::Chain, &Intent::default(), &Facing(lado));
        assert_eq!(dir, Vec2::new(lado, 0.0));
    }
}

/// Golpe de perna nao segue cursor.
///
/// A rasteira varre o chao e a voadora e a propria queda: mandar as duas
/// para o cursor daria ao jogador um golpe que se teleporta na diagonal, e
/// tiraria de cada uma o que a define.
#[test]
fn rasteira_e_voadora_nao_seguem_o_cursor() {
    for kind in [MeleeKind::Sweep, MeleeKind::Dive] {
        assert!(!follows_aim(kind), "{kind:?} virou golpe de mira");
        let dir = strike_dir(kind, &mirando(Vec2::Y), &Facing(-1.0));
        assert_eq!(dir, Vec2::new(-1.0, 0.0), "{kind:?} saiu do chao");
    }
}

/// Socar para cima joga o alvo para cima; socar para baixo o prega no chao.
///
/// O empurrao gira com o golpe, mas o levante nao: sem ele o gancho mirado
/// na horizontal deixaria de levantar o oponente, e o combo inteiro --
/// que se sustenta em manter o outro no ar -- iria junto.
#[test]
fn o_empurrao_segue_a_linha_do_golpe() {
    let gancho = unarmed_move(2);
    let empurrao = |dir: Vec2| dir * gancho.knockback.x + Vec2::Y * gancho.knockback.y;

    assert!(
        empurrao(Vec2::Y).y > gancho.knockback.y,
        "socar para cima nao joga para cima"
    );
    assert!(
        empurrao(Vec2::NEG_Y).y < 0.0,
        "socar para baixo nao prega no chao"
    );
    assert_eq!(
        empurrao(Vec2::X),
        gancho.knockback,
        "o soco reto mudou de empurrao"
    );
}

/// Quem clica atras do proprio boneco tem que ve-lo se virar.
#[test]
fn bater_para_tras_vira_o_boneco() {
    use crate::weapon::turn_to_aim;

    let mut facing = Facing(1.0);
    turn_to_aim(&mut facing, &mirando(Vec2::new(-0.9, 0.4).normalize()));
    assert_eq!(facing.0, -1.0, "ele socou por cima do proprio ombro");

    // Mira quase vertical nao tem lado: mexer nela nao pode fazer o boneco
    // girar no lugar a cada quadro.
    let mut olhando = Facing(1.0);
    turn_to_aim(&mut olhando, &mirando(Vec2::new(-0.05, 0.99).normalize()));
    assert_eq!(olhando.0, 1.0);
}

#[test]
fn a_partida_so_acaba_com_o_placar_cheio() {
    let mut result = RoundResult::default();
    assert_eq!(result.match_winner(), None);

    for round in 1..MATCH_WINS {
        result.score[1] = round;
        assert_eq!(
            result.match_winner(),
            None,
            "a partida acabou com {round} de {MATCH_WINS}"
        );
    }

    result.score[1] = MATCH_WINS;
    assert_eq!(result.match_winner(), Some(1));
}

/// Empate nao da ponto a ninguem, mas gasta um round. Sem contador
/// separado o "ROUND N" da tela travaria no mesmo numero.
#[test]
fn empate_gasta_round_sem_dar_ponto() {
    let mut result = RoundResult::default();
    result.rounds += 1;
    assert_eq!(result.score, [0; MAX_PLAYERS]);
    assert_eq!(result.rounds, 1);
    assert_eq!(result.match_winner(), None);
}

/// Entrar na arena com a partida ja decidida tem que comecar outra --
/// senao o vencedor entra no round seguinte ja campeao e a tela de fim
/// repete "TAKES IT" para sempre.
#[test]
fn partida_decidida_recomeca_na_arena_seguinte() {
    let mut app = App::new();
    app.insert_resource(RoundResult {
        winner: Some(0),
        score: [MATCH_WINS, 1, 0, 0],
        rounds: MATCH_WINS + 1,
        players: 2,
    })
    .add_systems(Update, start_new_match_if_over);

    app.update();

    let result = app.world().resource::<RoundResult>();
    assert_eq!(result.score, [0; MAX_PLAYERS], "o placar nao zerou");
    assert_eq!(result.rounds, 0, "a contagem de rounds nao zerou");
    assert_eq!(result.match_winner(), None);
}

/// Partida em andamento nao pode ser zerada ao entrar na arena, ou nenhum
/// placar sobreviveria do round 1 para o 2.
#[test]
fn partida_em_andamento_sobrevive_ao_round_seguinte() {
    let mut app = App::new();
    app.insert_resource(RoundResult {
        winner: Some(0),
        score: [1, 1, 0, 0],
        rounds: 2,
        players: 2,
    })
    .add_systems(Update, start_new_match_if_over);

    app.update();

    assert_eq!(app.world().resource::<RoundResult>().score, [1, 1, 0, 0]);
}

/// Com quatro em campo, a primeira morte so tira um da briga.
///
/// Enquanto a regra foi "alguem morreu", o primeiro a cair encerrava o
/// round e entregava a vitoria ao primeiro sobrevivente que a busca
/// encontrasse -- os outros dois nem terminavam a luta.
#[test]
fn o_round_de_quatro_so_acaba_quando_sobra_um() {
    fn arena(vivos: [bool; MAX_PLAYERS]) -> App {
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<RoundResult>()
            .init_resource::<NextState<GameState>>()
            .add_systems(Update, check_round_over);
        for (id, vivo) in vivos.into_iter().enumerate() {
            app.world_mut().spawn((
                Player {
                    id: id as u8,
                    color: palette::BONE,
                },
                Health {
                    hp: if vivo { 100 } else { 0 },
                    max: 100,
                },
            ));
        }
        app
    }

    let mut app = arena([true, false, true, true]);
    app.update();
    assert!(
        app.world().get_resource::<RoundEndDelay>().is_none(),
        "o round acabou com tres ainda de pe"
    );

    let mut app = arena([false, false, true, false]);
    app.update();
    assert!(
        app.world().get_resource::<RoundEndDelay>().is_some(),
        "sobrou um e o round nao acabou"
    );
    let result = app.world().resource::<RoundResult>();
    assert_eq!(result.winner, Some(2), "o ponto foi para o lugar errado");
    assert_eq!(result.score, [0, 0, 1, 0]);
    assert_eq!(
        result.players, 4,
        "o placar nao registrou os quatro lugares"
    );
}

/// Arena vazia nao e round decidido.
///
/// No quadro em que ninguem nasceu ainda, "sobrou no maximo um" e verdade
/// por vacuidade -- sem a guarda de contagem a luta acabaria antes de
/// comecar.
#[test]
fn arena_vazia_nao_encerra_round() {
    let mut app = App::new();
    app.init_resource::<Time>()
        .init_resource::<RoundResult>()
        .init_resource::<NextState<GameState>>()
        .add_systems(Update, check_round_over);

    app.update();

    assert!(app.world().get_resource::<RoundEndDelay>().is_none());
    assert_eq!(app.world().resource::<RoundResult>().rounds, 0);
}

/// A rasteira compra vantagem, nao vida. Se ela desse mais dano que o
/// primeiro elo e ainda derrubasse, o combo em pe nunca valeria a pena.
#[test]
fn a_rasteira_troca_dano_por_vantagem() {
    assert!(
        SWEEP_MOVE.damage < unarmed_move(0).damage,
        "a rasteira dói mais que o jab"
    );
    assert!(
        SWEEP_STUN > HIT_STUN * 2.0,
        "a rasteira nao derruba por tempo suficiente pra compensar"
    );
}

/// Golpe que nao encadeia tambem tem que ser limpo.
///
/// `continue_combo` so enxergava quem tinha `ComboChain`, entao a pancada,
/// a rasteira e a voadora deixavam um `MeleeAttack` pendurado no jogador
/// depois de acabar -- e um acerto de tiro passava a ser nomeado com o
/// ultimo golpe corpo-a-corpo que tinha saido.
#[test]
fn golpe_que_nao_encadeia_e_limpo_ao_terminar() {
    for kind in [MeleeKind::Heavy, MeleeKind::Sweep, MeleeKind::Dive] {
        let mut app = App::new();
        app.add_systems(Update, continue_combo);
        let player = app
            .world_mut()
            .spawn(MeleeAttack {
                step: 0,
                style: WeaponStyle::Unarmed,
                kind,
                move_data: SWEEP_MOVE,
                launched: true,
            })
            .id();
        app.update();
        assert!(
            app.world().get::<MeleeAttack>(player).is_none(),
            "{kind:?} deixou o MeleeAttack pendurado"
        );
    }
}

/// A voadora viaja com o corpo, entao a janela dela tem que ser mais longa
/// que a dos golpes parados -- senao so acerta quem estiver exatamente no
/// ponto de contato.
#[test]
fn a_voadora_tem_janela_mais_longa() {
    assert!(melee_active(MeleeKind::Dive) > melee_active(MeleeKind::Chain));
    assert!(
        DIVE_LAUNCH.y < 0.0,
        "a voadora tem que descer, nao subir: {DIVE_LAUNCH:?}"
    );
    assert!(DIVE_LAUNCH.x > 0.0, "a voadora tem que avancar");
}

/// O `Hop` do dummy so treina antiaereo se a altura dele cair na janela
/// certa: alto o bastante pra rasteira passar por baixo, baixo o bastante
/// pro gancho ainda alcancar.
///
/// A janela tem menos de 10 unidades. Sem este teste, mexer em
/// `DUMMY_HOP`, na altura das hitboxes ou no tamanho do corpo transformaria
/// o modo num boneco que so sobe e desce.
#[test]
fn o_pulo_do_dummy_cai_na_janela_do_antiaereo() {
    use crate::actor::DUMMY_HOP;

    let half_body = crate::actor::body_half_height();
    let half_hit = MELEE_BOX_H * 0.5;
    // Atacante no chao em y = 0; dummy no topo do pulo.
    let corpo = (DUMMY_HOP - half_body, DUMMY_HOP + half_body);
    let alcanca = |kind: MeleeKind, step: u8| {
        let mid = melee_height(kind, step);
        mid - half_hit < corpo.1 && corpo.0 < mid + half_hit
    };

    assert!(
        !alcanca(MeleeKind::Sweep, 0),
        "a rasteira alcanca o dummy no ar; ela deveria passar por baixo"
    );
    assert!(
        alcanca(MeleeKind::Chain, 2),
        "o gancho nao alcanca o dummy no ar; nao ha antiaereo pra treinar"
    );
}

/// A moldura tem que ser vazada e retangular.
///
/// Cheia, ela esconderia justamente o boneco que esta medindo; torta, ela
/// mediria errado e o visualizador viraria mais uma fonte de engano.
#[test]
fn a_moldura_de_depuracao_e_vazada_e_retangular() {
    let arte = outline(6, 4);
    let linhas: Vec<&str> = arte.lines().collect();

    assert_eq!(linhas.len(), 4);
    assert!(
        linhas.iter().all(|linha| linha.chars().count() == 6),
        "moldura torta: {linhas:?}"
    );
    assert!(
        linhas[1].chars().skip(1).take(4).all(|c| c == ' '),
        "o miolo nao esta vazado: {:?}",
        linhas[1]
    );
    // Uma caixa degenerada nao pode virar arte vazia -- ela ainda precisa
    // aparecer para denunciar que ficou degenerada.
    assert_eq!(outline(0, 0).lines().count(), 2);

    // Mesma licao da `BOMB`: glifo fora da pagina vira `?` em silencio.
    use crate::ascii::cp437::glyph_index;
    for ch in arte.chars().filter(|c| !c.is_whitespace()) {
        assert_ne!(
            glyph_index(ch),
            glyph_index('?'),
            "U+{:04X} fora da CP437",
            ch as u32
        );
    }
}

/// Agachar tem que servir pra alguma coisa: ele e a resposta ao gancho.
///
/// Antes disto o mixup so existia do lado de quem ataca -- a rasteira
/// errava quem estava no ar, mas quem defendia nao tinha resposta a nada.
/// O jab continua acertando, porque nenhuma postura pode ganhar de tudo, e
/// a rasteira tambem, que e o castigo de quem agacha demais.
#[test]
fn agachar_passa_por_baixo_do_gancho() {
    let corpo = Collider::size(30.0, crate::actor::body_half_height() * 2.0);
    let em_pe = corpo.aabb(Vec2::ZERO);
    let baixo = Hurtbox::crouched(&corpo).aabb(Vec2::ZERO);

    let golpe = |kind, step| {
        Rect::from_center_half_size(
            Vec2::new(0.0, melee_height(kind, step)),
            Vec2::new(40.0, MELEE_BOX_H * 0.5),
        )
    };
    let gancho = golpe(MeleeKind::Chain, 2);

    assert!(
        (baixo.min.y - em_pe.min.y).abs() < 0.01,
        "os pes sairam do lugar ao agachar"
    );
    assert!(
        overlap(gancho, em_pe),
        "o gancho nao acerta quem esta em pe"
    );
    assert!(
        !overlap(gancho, baixo),
        "o gancho ainda acerta quem agachou"
    );
    assert!(
        overlap(golpe(MeleeKind::Chain, 0), baixo),
        "o jab deixou de acertar quem agacha"
    );
    assert!(
        overlap(golpe(MeleeKind::Sweep, 0), baixo),
        "a rasteira nao pune mais quem agacha"
    );
    // A pancada do cano desce, entao ela passa onde o punho nao passa --
    // e o que da a arma uma resposta que o corpo-a-corpo nao tem.
    assert!(
        overlap(golpe(MeleeKind::Heavy, 2), baixo),
        "a pancada tambem virou golpe alto"
    );
}

/// Cada tipo de golpe tem que abrir a hitbox numa altura diferente: e a
/// unica coisa que distingue alto de baixo pra quem apanha.
#[test]
fn alto_e_baixo_abrem_em_alturas_diferentes() {
    let baixo = melee_height(MeleeKind::Sweep, 0);
    let alto = melee_height(MeleeKind::Chain, 2);
    assert!(
        alto - baixo > 20.0,
        "alto e baixo abrem quase na mesma altura: {alto} e {baixo}"
    );
}
