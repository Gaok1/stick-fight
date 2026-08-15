use super::*;

/// `def` indexa a tabela pelo discriminante da pose. Uma linha fora de
/// lugar daria a uma pose a deformacao e os membros de outra, calada.
#[test]
fn a_tabela_segue_a_ordem_do_enum() {
    for (i, (pose, _)) in POSES.iter().enumerate() {
        assert_eq!(*pose as usize, i, "{pose:?} fora de lugar na tabela");
    }
}

/// Uma pose nao pode estar em dois clipes: `frame` devolveria o quadro de
/// um deles e a animacao andaria pelo outro.
#[test]
fn nenhuma_pose_esta_em_dois_clipes() {
    for pose in all() {
        let clipes = CLIPS
            .iter()
            .filter(|clip| clip.index_of(pose).is_some())
            .count();
        assert!(clipes <= 1, "{pose:?} aparece em {clipes} clipes");
    }
}

/// Todo quadro de um clipe tem que estar na tabela, e o clipe tem que dar a
/// volta no lugar certo.
#[test]
fn os_clipes_giram_no_proprio_tamanho() {
    for clip in CLIPS {
        assert!(clip.len() > 0);
        assert_eq!(clip.at(clip.len()), clip.at(0));
        for i in 0..clip.len() {
            assert_eq!(clip.index_of(clip.at(i)), Some(i));
        }
    }
}

/// O ciclo de corrida tem que fechar com degraus iguais.
///
/// A tabela escrita a mao que existia aqui andava -1, -0.6, 0, 0.6, 1, 0.35
/// e pulava 1.35 ao dar a volta -- mais que o dobro do maior degrau normal,
/// o que faz a perna dar um tranco toda passada. Nada no jogo reclamava
/// disso; era so feio.
#[test]
fn o_ciclo_de_corrida_fecha_com_passo_uniforme() {
    let fases: Vec<f32> = (0..RUN_FRAMES).map(run_gait).collect();
    let degraus: Vec<f32> = (0..RUN_FRAMES)
        .map(|i| (fases[(i + 1) % RUN_FRAMES] - fases[i]).abs())
        .collect();
    let maior = degraus.iter().cloned().fold(f32::MIN, f32::max);
    let menor = degraus.iter().cloned().fold(f32::MAX, f32::min);
    assert!(
        (maior - menor).abs() < 0.01,
        "degraus desiguais: {degraus:?} para {fases:?}"
    );
}

/// A passada tem que varrer o balanco inteiro, nos dois sentidos: um ciclo
/// que so vai pra frente anima um boneco mancando.
#[test]
fn a_passada_vai_e_volta() {
    let fases: Vec<f32> = (0..RUN_FRAMES).map(run_gait).collect();
    assert!(fases.iter().cloned().fold(f32::MIN, f32::max) >= 0.99);
    assert!(fases.iter().cloned().fold(f32::MAX, f32::min) <= -0.99);
    assert_eq!(run_gait(RUN_FRAMES), run_gait(0));
}

/// So quem tira o controle pode pintar de dano ou de morte: se uma pose
/// jogavel virasse `Hurt`, o jogador ficaria vermelho podendo agir e a cor
/// pararia de significar "esta apanhando".
#[test]
fn a_cor_de_dano_so_aparece_em_pose_travada() {
    for pose in all() {
        let def = def(pose);
        if def.tone != Tone::Body {
            assert!(def.locks, "{pose:?} pinta de dano mas deixa agir");
        }
    }
}

/// A passada so pode mexer o corpo em quem esta correndo. Uma pose parada
/// que lesse a fase da passada moveria a perna sozinha.
#[test]
fn so_a_corrida_se_inclina_com_a_velocidade() {
    for pose in all() {
        assert_eq!(
            def(pose).body.sway == Sway::Lean,
            RUN.index_of(pose).is_some(),
            "{pose:?} discorda sobre estar correndo"
        );
    }
}

/// Toda silhueta tem que ter uma cabeca, e uma so.
///
/// E a celula dela que diz onde vai o rosto e qual celula a silhueta nao
/// desenha. Uma arte sem cabeca cairia no palpite de [`head_at`] -- a
/// celula de cima -- e o rosto de quem esta caido ficaria flutuando onde
/// estaria a cabeca de quem esta de pe, com o circulo de volta no chao.
#[test]
fn toda_silhueta_tem_uma_cabeca() {
    for pose in all() {
        let cabecas = def(pose)
            .art
            .chars()
            .filter(|ch| HEAD_GLYPHS.contains(ch))
            .count();
        assert_eq!(cabecas, 1, "{pose:?} tem {cabecas} cabecas na silhueta");
    }
}

/// O rosto acompanha a cabeca que a silhueta desenha.
///
/// Agachar, cair e morrer baixam a cabeca uma linha ou mais. Enquanto o
/// rosto teve altura propria, ele ficava parado onde a cabeca de quem esta
/// de pe fica -- o boneco agachava e a cara continuava no ar.
#[test]
fn a_cabeca_desce_com_a_silhueta() {
    let y = |pose| head_cell(pose).y;
    assert!(
        y(Pose::Crouch) < y(Pose::IdleA),
        "agachar nao baixa a cabeca"
    );
    assert!(y(Pose::Downed) < y(Pose::Crouch), "cair nao baixa a cabeca");
    assert!(y(Pose::Dead) < y(Pose::Downed), "morrer nao baixa a cabeca");

    // E ela cai dentro da caixa da arte, medida a partir dos pes.
    for pose in all() {
        let head = head_cell(pose);
        assert!(
            head.y > 0.0 && head.y < BODY_ROWS as f32 * CELL.y,
            "{pose:?} poe a cabeca fora da arte: {head:?}"
        );
        assert!(
            head.x.abs() < BODY_COLS as f32 * CELL.x * 0.5,
            "{pose:?} poe a cabeca fora da arte: {head:?}"
        );
    }
}

/// Agachar tem que baixar o corpo inteiro, e nao so as pernas.
///
/// Enquanto `crouch` escrevia so joelho e pe, o ombro ficava na altura de
/// quem esta em pe -- acima da cabeca agachada -- e os bracos pairavam ao
/// lado do boneco. Os pes ficam onde a passada parada os deixa: agachar
/// dobra a perna, nao levanta o boneco do chao.
#[test]
fn agachar_abaixa_o_corpo_inteiro() {
    for side in [-1.0, 1.0] {
        let rigging = Rigging {
            side,
            facing: 1.0,
            gait: 0.0,
            aim: Vec2::X,
            strike: None,
            reach: 0.0,
            cycle: 0.0,
            air: 0.0,
        };
        let de_pe = Joints::gait(&rigging);
        let mut agachado = de_pe;
        crouch(&mut agachado, &rigging);

        for (nome, em_pe, baixo) in [
            ("ombro", de_pe.shoulder.y, agachado.shoulder.y),
            ("cotovelo", de_pe.elbow.y, agachado.elbow.y),
            ("mao", de_pe.hand.y, agachado.hand.y),
            ("quadril", de_pe.hip.y, agachado.hip.y),
        ] {
            assert!(
                baixo < em_pe - 2.0,
                "o {nome} nao desce ao agachar: {em_pe} -> {baixo}"
            );
        }
        assert_eq!(
            agachado.foot.y, de_pe.foot.y,
            "os pes sairam do chao ao agachar"
        );
        // O joelho de um agachamento nao desce: ele abre pra fora,
        // enquanto o quadril e que vem ao encontro dele.
        assert!(
            agachado.knee.x.abs() > de_pe.knee.x.abs() + 2.0,
            "o joelho nao abre ao agachar: {} -> {}",
            de_pe.knee.x,
            agachado.knee.x
        );
    }
}

/// Monta o quadro de contato de um golpe, mirando em `aim`.
fn socando(aim: Vec2, side: f32) -> Joints {
    let rigging = Rigging {
        side,
        facing: 1.0,
        gait: 0.0,
        aim,
        strike: Some((crate::actor::pose::strike(0), 1)),
        reach: 0.0,
        cycle: 0.0,
        air: 0.0,
    };
    let mut joints = Joints::gait(&rigging);
    choreo(&mut joints, &rigging);
    joints
}

/// O punho tem que ir para onde a mira aponta.
///
/// E a metade visual de uma regra que o combate ja aplica na hitbox. Sem
/// ela o golpe abre no alto e o braco continua deitado -- dois desenhos do
/// mesmo soco discordando na mesma tela.
#[test]
fn o_soco_acompanha_a_mira() {
    let reto = socando(Vec2::X, 1.0);
    let alto = socando(Vec2::Y, 1.0);
    let baixo = socando(Vec2::NEG_Y, 1.0);

    assert!(reto.hand.x > 20.0, "o jab parou de sair para a frente");
    assert!(
        alto.hand.y > reto.hand.y + 15.0,
        "mirar para cima nao levantou o punho: {:?}",
        alto.hand
    );
    assert!(
        baixo.hand.y < reto.hand.y - 15.0,
        "mirar para baixo nao baixou o punho: {:?}",
        baixo.hand
    );

    // Girar no ombro nao pode esticar nem encolher o braco: o membro e um
    // sprite de comprimento fixo, e um braco que cresce le como erro.
    let braco = |j: &Joints| (j.hand - j.shoulder).length();
    for (nome, girado) in [("alto", &alto), ("baixo", &baixo)] {
        assert!(
            (braco(girado) - braco(&reto)).abs() < 0.5,
            "o braco mudou de tamanho mirando {nome}"
        );
    }
}

/// So o braco que soca acompanha a mira; o da guarda fica onde esta.
///
/// O quanto cada braco gira sai da propria coreografia -- de o punho estar
/// avancado ou recolhido -- e nao de ser o da frente ou o de tras. E o que
/// faz o cruzado, que soca com o braco de tras, funcionar sem uma segunda
/// regra escrita a parte.
#[test]
fn o_braco_de_guarda_nao_gira_junto() {
    let guarda_reta = socando(Vec2::X, -1.0);
    let guarda_alta = socando(Vec2::Y, -1.0);
    assert!(
        (guarda_alta.hand - guarda_reta.hand).length() < 1.0,
        "a guarda seguiu o cursor junto com o soco"
    );
}

/// A linha da mira sai do ombro da pose, e nao de um numero fixo: quem
/// agacha com arma na mao segura ela na altura do proprio peito.
#[test]
fn a_mira_desce_junto_com_o_ombro() {
    assert!(
        aim_anchor(Pose::Crouch).y < aim_anchor(Pose::IdleA).y - 5.0,
        "agachado, a arma continua na altura de quem esta em pe"
    );
}

/// Cada familia tem peso e apoio legiveis, e o coice recolhe mao e arma
/// pelo mesmo caminho.
#[test]
fn a_empunhadura_muda_com_a_arma_e_o_coice() {
    let rig = |side| Rigging {
        side,
        facing: 1.0,
        gait: 0.0,
        aim: Vec2::X,
        strike: None,
        reach: 0.0,
        cycle: 0.0,
        air: 0.0,
    };
    let pistola_livre = armed_joints(Pose::IdleA, WeaponStyle::Pistol, 0.0, &rig(-1.0));
    let rifle_apoio = armed_joints(Pose::IdleA, WeaponStyle::Rifle, 0.0, &rig(-1.0));
    assert!(
        rifle_apoio.hand.x > pistola_livre.hand.x + 10.0,
        "rifle perdeu a mao de apoio"
    );

    let rifle = armed_joints(Pose::IdleA, WeaponStyle::Rifle, 0.0, &rig(1.0));
    let com_coice = armed_joints(Pose::IdleA, WeaponStyle::Rifle, 0.8, &rig(1.0));
    assert!(
        com_coice.hand.x < rifle.hand.x - 5.0,
        "coice nao recolhe a arma"
    );
}

/// A passada padrao tem que ser simetrica: com a fase zerada, os dois lados
/// do corpo caem no mesmo lugar espelhado. Sem isso o boneco nasce torto.
#[test]
fn a_passada_parada_e_simetrica() {
    let make = |side: f32| {
        Joints::gait(&Rigging {
            side,
            facing: 1.0,
            gait: 0.0,
            aim: Vec2::X,
            strike: None,
            reach: 0.0,
            cycle: 0.0,
            air: 0.0,
        })
    };
    let (frente, tras) = (make(1.0), make(-1.0));
    assert_eq!(frente.hand.x, -tras.hand.x);
    assert_eq!(frente.foot.x, -tras.foot.x);
    assert_eq!(frente.hand.y, tras.hand.y);
}

/// Correr nao e andar depressa: o que separa os dois e a altura.
///
/// A passada padrao balanca os membros so na horizontal. Enquanto a
/// corrida herdou esse ajuste, o pe nunca saia da linha do chao e a mao
/// nunca subia -- seis quadros de uma caminhada acelerada.
#[test]
fn a_corrida_tira_o_pe_do_chao() {
    let quadro = |gait: f32, side: f32| {
        let rig = Rigging {
            side,
            facing: 1.0,
            gait,
            aim: Vec2::X,
            strike: None,
            reach: 0.0,
            cycle: 0.0,
            air: 0.0,
        };
        let mut joints = Joints::gait(&rig);
        running(&mut joints, &rig);
        joints
    };

    // Varre o ciclo inteiro e mede o quanto cada ponta sobe e desce.
    let ciclo: Vec<Joints> = (0..24)
        .map(|i| quadro(-(i as f32 / 24.0 * std::f32::consts::TAU).cos(), 1.0))
        .collect();
    let curso = |ponta: fn(&Joints) -> f32| {
        let alto = ciclo.iter().map(ponta).fold(f32::MIN, f32::max);
        let baixo = ciclo.iter().map(ponta).fold(f32::MAX, f32::min);
        alto - baixo
    };
    assert!(curso(|j| j.foot.y) > 8.0, "o pe nao sai do chao");
    assert!(curso(|j| j.hand.y) > 8.0, "a mao nao sobe");

    // Uma perna na frente e a outra atras: se as duas ficam na mesma
    // altura, o boneco corre de pernas juntas.
    assert!(
        quadro(1.0, 1.0).knee.y > quadro(1.0, -1.0).knee.y,
        "as duas pernas correm iguais"
    );
}

/// Quem corre cai pra frente e deixa as pernas alcancarem o corpo.
#[test]
fn a_corrida_cai_pra_frente() {
    let (_, angle) = def(Pose::RunA).body.resolve(&Swaying {
        pulse: 0.0,
        facing: 1.0,
        speed: 1.0,
        rise: 1.0,
    });
    assert!(angle.abs() > 0.12, "a corrida mal se inclina: {angle}");
}

#[test]
fn escalada_fecha_um_ciclo_continuo() {
    let hand = |cycle: f32| {
        let rig = Rigging {
            side: 1.0,
            facing: 1.0,
            gait: 0.0,
            aim: Vec2::Y,
            strike: None,
            reach: 0.0,
            cycle,
            air: 0.0,
        };
        let mut joints = Joints::gait(&rig);
        climb(&mut joints, &rig);
        joints.hand.y
    };
    assert!((hand(0.0) - hand(std::f32::consts::TAU)).abs() < 0.001);
    assert!(hand(0.1) > hand(0.0), "a bracada deveria subir sem degrau");
}
