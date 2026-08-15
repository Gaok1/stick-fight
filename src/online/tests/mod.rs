use super::*;

fn presses_of(intent: &Intent) -> Presses {
    let mut presses = [0; PULSES];
    bump_presses(&mut presses, intent);
    presses
}

#[test]
fn intent_faz_round_trip_no_pacote() {
    let original = Intent {
        move_x: -1.0,
        up: true,
        attack: true,
        parry: true,
        aim: Vec2::new(0.5, -0.25),
        ..default()
    };
    let (decoded, presses) =
        decode_intent(&encode_intent(&original, presses_of(&original))).unwrap();
    assert_eq!(decoded.move_x, -1.0);
    assert!(decoded.up);
    assert!((decoded.aim - original.aim).length() < 0.001);
    // Golpe e defesa viajam como contagem, nao como bit do quadro.
    assert_eq!(presses[1], 1, "o golpe nao foi contado");
    assert_eq!(presses[3], 1, "a defesa nao foi contada");
    assert_eq!(presses[0], 0, "pulo contado sem ter sido apertado");
}

/// Um pacote perdido nao pode custar um golpe.
///
/// E a razao de a entrada viajar como contador. Enquanto ela viajava como
/// "ataquei neste quadro", o pacote que se perdia levava o soco junto -- e
/// como ela era enviada com garantia de entrega para evitar isso, cada
/// quadro entrava numa fila que so crescia. O atraso vinha dai.
#[test]
fn pacote_perdido_nao_engole_o_golpe() {
    let remote = RemoteInput::default();
    let keys = ButtonInput::<KeyCode>::default();
    let sense = sense_parado();
    let parado = Intent::default();
    let socando = Intent {
        attack: true,
        ..default()
    };

    let mut presses = [0; PULSES];
    // Calibra: o primeiro pacote so diz de onde a contagem parte.
    remote.push(parado, presses);
    remote.poll(&keys, &sense);

    // Tres socos seguidos, e so o ultimo pacote chega.
    for _ in 0..3 {
        bump_presses(&mut presses, &socando);
    }
    remote.push(parado, presses);

    let entregues = (0..5).filter(|_| remote.poll(&keys, &sense).attack).count();
    assert_eq!(entregues, 3, "os socos perdidos nao foram recuperados");
}

/// E o mesmo pacote chegando duas vezes nao pode virar dois golpes.
#[test]
fn pacote_repetido_nao_duplica_o_golpe() {
    let remote = RemoteInput::default();
    let keys = ButtonInput::<KeyCode>::default();
    let sense = sense_parado();
    let mut presses = [0; PULSES];

    remote.push(Intent::default(), presses);
    bump_presses(
        &mut presses,
        &Intent {
            jump: true,
            ..default()
        },
    );
    remote.push(Intent::default(), presses);
    remote.push(Intent::default(), presses);

    let pulos = (0..5).filter(|_| remote.poll(&keys, &sense).jump).count();
    assert_eq!(pulos, 1, "o pacote repetido pulou duas vezes");
}

/// O primeiro pacote nao pode chegar como uma rajada de apertos.
#[test]
fn a_primeira_leitura_nao_dispara_nada() {
    let remote = RemoteInput::default();
    let keys = ButtonInput::<KeyCode>::default();
    let sense = sense_parado();

    remote.push(Intent::default(), [200; PULSES]);
    let intent = remote.poll(&keys, &sense);
    assert!(!intent.attack && !intent.jump && !intent.special);
}

fn sense_parado() -> Sense {
    Sense {
        at: Vec2::ZERO,
        foe: None,
        grounded: true,
        health: 1.0,
        time: 0.0,
        left: default(),
        right: default(),
        armed: None,
    }
}

/// O pacote de inicio carrega a aparencia inteira de cada lugar.
///
/// Enquanto ele levava so a pele, o rosto escolhido ficava em casa: o
/// adversario nascia com o rosto padrao na tela de quem o enfrentava, e
/// cada cliente via uma cara diferente do mesmo boneco.
#[test]
fn inicio_faz_round_trip_com_fase_lugar_pele_e_rosto() {
    let looks = [
        Look {
            skin: 3,
            face: Face {
                hair: 2,
                eyes: 4,
                nose: 1,
                mouth: 3,
            },
        },
        Look {
            skin: 5,
            face: Face::variety(1),
        },
        Look {
            skin: 0,
            face: Face::variety(2),
        },
        Look {
            skin: 2,
            face: Face::default(),
        },
    ];
    let packet = encode_start(2, 3, 1, looks);
    assert_eq!(decode_start(&packet), Some((2, 3, 1, looks)));
    assert_eq!(decode_start(&packet[..3]), None);
}

/// Uma sala de dois nao pode pagar o preco de uma de quatro.
#[test]
fn o_snapshot_so_descreve_os_lugares_em_uso() {
    let actor = ActorSnapshot {
        at: Vec2::new(1.0, 2.0),
        velocity: Vec2::new(3.0, 4.0),
        hp: 91,
        facing: 1.0,
    };
    let dois: Snapshot = [Some(actor), Some(actor), None, None];
    let quatro: Snapshot = [Some(actor); MAX_PLAYERS];

    assert_eq!(encode_snapshot(&dois).len(), 2 + 2 * ACTOR_BYTES);
    assert_eq!(encode_snapshot(&quatro).len(), 2 + 4 * ACTOR_BYTES);
}

#[test]
fn snapshot_faz_round_trip_preservando_os_lugares() {
    let mut actors: Snapshot = [None; MAX_PLAYERS];
    actors[0] = Some(ActorSnapshot {
        at: Vec2::new(1.0, 2.0),
        velocity: Vec2::new(3.0, 4.0),
        hp: 91,
        facing: 1.0,
    });
    // Buraco no meio de proposito: o lugar 1 saiu da partida, e o 2 nao
    // pode herdar os bytes dele.
    actors[2] = Some(ActorSnapshot {
        at: Vec2::new(-5.0, 6.0),
        velocity: Vec2::new(7.0, -8.0),
        hp: 42,
        facing: -1.0,
    });

    let decoded = decode_snapshot(&encode_snapshot(&actors)).unwrap();
    assert_eq!(decoded[0].unwrap().at, Vec2::new(1.0, 2.0));
    assert!(decoded[1].is_none(), "lugar vazio virou lutador");
    assert_eq!(decoded[2].unwrap().hp, 42);
    assert_eq!(decoded[2].unwrap().facing, -1.0);
    assert!(decoded[3].is_none());
}

#[test]
fn tabela_de_entrada_faz_round_trip_por_lugar() {
    let mut table: [Option<Intent>; MAX_PLAYERS] = [None; MAX_PLAYERS];
    table[1] = Some(Intent {
        move_x: 1.0,
        jump: true,
        ..default()
    });
    table[3] = Some(Intent {
        attack: true,
        ..default()
    });
    let mut presses = [[0; PULSES]; MAX_PLAYERS];
    for (slot, intent) in table.iter().enumerate() {
        if let Some(intent) = intent {
            bump_presses(&mut presses[slot], intent);
        }
    }

    let decoded = decode_input_table(&encode_input_table(&table, &presses)).unwrap();
    assert!(decoded[0].is_none());
    assert_eq!(decoded[1].unwrap().0.move_x, 1.0);
    assert_eq!(decoded[1].unwrap().1[0], 1, "o pulo do lugar 1 se perdeu");
    assert!(decoded[2].is_none());
    assert_eq!(decoded[3].unwrap().1[1], 1, "o golpe do lugar 3 se perdeu");
}

/// O armamento inteiro tem que atravessar: quem segura o que, e o que ficou
/// caido onde.
#[test]
fn armas_fazem_round_trip() {
    let mut state = WeaponState::default();
    state.held[0] = Some(HeldState { kind: 2, ammo: 7 });
    state.held[3] = Some(HeldState { kind: 5, ammo: 0 });
    state.ground.push(GroundState {
        net: 4211,
        kind: 1,
        ammo: 4,
        at: Vec2::new(-120.0, 30.0),
        velocity: Vec2::new(0.0, -9.0),
        thrown: false,
    });
    state.ground.push(GroundState {
        net: 4212,
        kind: 7,
        ammo: 2,
        at: Vec2::new(88.0, -12.0),
        velocity: Vec2::new(590.0, 260.0),
        thrown: true,
    });

    let decoded = decode_weapons(&encode_weapons(&state)).unwrap();
    assert_eq!(decoded, state);
    assert!(decoded.held[1].is_none(), "lugar sem arma ganhou uma");
}

/// Pacote truncado ou com lugar inexistente nao pode virar estado.
#[test]
fn pacote_corrompido_e_recusado() {
    let actors: Snapshot = [Some(ActorSnapshot::default()), None, None, None];
    let packet = encode_snapshot(&actors);
    assert!(decode_snapshot(&packet[..packet.len() - 1]).is_none());

    let mut mentiroso = packet.clone();
    mentiroso[1] = 0b1111_0001;
    assert!(
        decode_snapshot(&mentiroso).is_none(),
        "mascara citando lugar inexistente foi aceita"
    );

    let mut armas = WeaponState::default();
    armas.ground.push(GroundState::default());
    let packet = encode_weapons(&armas);
    assert!(decode_weapons(&packet[..packet.len() - 1]).is_none());
    // Uma contagem inventada nao pode virar uma alocacao enorme.
    let mut mentiroso = packet.clone();
    mentiroso[2] = 200;
    assert!(decode_weapons(&mentiroso).is_none());
}

#[test]
fn fim_de_round_faz_round_trip_com_o_placar_inteiro() {
    let result = RoundResult {
        winner: Some(2),
        score: [1, 0, 2, 1],
        rounds: 4,
        players: 4,
    };
    let decoded = decode_round_over(&encode_round_over(&result)).unwrap();
    assert_eq!(decoded.winner, Some(2));
    assert_eq!(decoded.score, [1, 0, 2, 1]);
    assert_eq!(decoded.rounds, 4);
    assert_eq!(decoded.players, 4);
}

/// O ritmo de envio nao pode andar junto com o framerate.
#[test]
fn o_envio_segue_o_relogio_e_nao_o_framerate() {
    let mut since = 0.0;
    // 240 quadros de 1/240 s: a 60 Hz isso tem que dar 60 envios.
    let enviados = (0..240)
        .filter(|_| due(&mut since, 1.0 / 240.0, 60.0))
        .count();
    assert_eq!(enviados, 60, "o envio nao respeitou o ritmo pedido");

    // E um quadro longo nao pode virar uma rajada para compensar.
    let mut since = 0.0;
    assert!(due(&mut since, 2.0, 60.0));
    let seguidos = (0..3).filter(|_| due(&mut since, 0.0, 60.0)).count();
    assert_eq!(seguidos, 0, "a pausa virou rajada");
}

/// Empate nao tem vencedor, e 255 nao pode virar o jogador 255.
/// Quem fica nao pode mudar de lugar quando alguem sai.
///
/// Renumerar entregaria o boneco, a cor e a linha do placar de quem saiu
/// para o vizinho -- e a entrada que chegasse pelo lugar antigo iria
/// mandar no jogador errado.
#[test]
fn sair_da_sala_nao_renumera_quem_fica() {
    let (dono, b, c, d) = (
        SteamId::from_raw(1),
        SteamId::from_raw(2),
        SteamId::from_raw(3),
        SteamId::from_raw(4),
    );

    let cheia = assign_slots([None; MAX_PLAYERS], dono, &[dono, b, c, d]);
    assert_eq!(cheia, [Some(dono), Some(b), Some(c), Some(d)]);

    // O lugar 1 desiste: o 2 e o 3 continuam onde estavam.
    let apos_saida = assign_slots(cheia, dono, &[dono, c, d]);
    assert_eq!(apos_saida, [Some(dono), None, Some(c), Some(d)]);

    // E quem chega depois herda a vaga aberta, nao o fim da fila.
    let e = SteamId::from_raw(5);
    let recomposta = assign_slots(apos_saida, dono, &[dono, c, d, e]);
    assert_eq!(recomposta, [Some(dono), Some(e), Some(c), Some(d)]);
}

/// Sala nova comeca com o dono no primeiro lugar.
#[test]
fn o_dono_fica_no_primeiro_lugar() {
    let (dono, outro) = (SteamId::from_raw(9), SteamId::from_raw(7));
    let slots = assign_slots([None; MAX_PLAYERS], dono, &[outro, dono]);
    assert_eq!(slots[0], Some(dono), "o dono nao ficou com a autoridade");
    assert_eq!(slots[1], Some(outro));
}

/// O lugar vago no meio ainda precisa de boneco.
///
/// Enquanto a luta contou ocupados, uma sala [dono, vago, C, D] abria com
/// tres lugares e o jogador do quarto ficava sem boneco: sem entidade, o
/// snapshot nao o alcancava e ele assistia a propria partida.
#[test]
fn lugar_vago_no_meio_nao_encurta_a_partida() {
    let mut session = OnlineSession {
        slots: [
            Some(SteamId::from_raw(1)),
            None,
            Some(SteamId::from_raw(3)),
            Some(SteamId::from_raw(4)),
        ],
        ..default()
    };
    assert_eq!(session.filled(), 3);
    assert_eq!(session.span(), 4, "o ultimo lugar ficaria sem boneco");

    // Sala de dois continua sendo sala de dois.
    session.slots = [
        Some(SteamId::from_raw(1)),
        Some(SteamId::from_raw(2)),
        None,
        None,
    ];
    assert_eq!(session.span(), 2);
}

/// Sala cheia nao aceita um quinto.
#[test]
fn a_sala_para_no_teto() {
    let membros: Vec<SteamId> = (1..=6).map(SteamId::from_raw).collect();
    let slots = assign_slots([None; MAX_PLAYERS], membros[0], &membros);
    assert_eq!(slots.iter().flatten().count(), MAX_PLAYERS);
}

#[test]
fn empate_atravessa_o_pacote_como_empate() {
    let result = RoundResult {
        winner: None,
        score: [0, 0, 0, 0],
        rounds: 1,
        players: 2,
    };
    let decoded = decode_round_over(&encode_round_over(&result)).unwrap();
    assert_eq!(decoded.winner, None);
}
