    /// Roda os sistemas de arma de verdade e devolve os tres quadros do golpe.
    fn run_strike(kind: u8, melee: MeleeKind, step: u8) -> Vec<String> {
        #[derive(Resource, Clone, Copy)]
        struct Armed(u8);

        let weapon = weapon_at(kind);
        let mut app = App::new();
        app.init_resource::<Time>()
            .insert_resource(Armed(kind))
            .add_systems(Startup, |mut commands: Commands, armed: Res<Armed>| {
                let player = commands
                    .spawn((
                        Player {
                            id: 0,
                            color: palette::player(0),
                        },
                        Intent::default(),
                        Pose::IdleA,
                        Facing(1.0),
                        Transform::default(),
                    ))
                    .id();
                equip(&mut commands, player, armed.0, weapon_at(armed.0).ammo(), 1.0);
            })
            .add_systems(Update, (animate_weapon_icon, animate_weapon_rigs).chain());
        app.update();

        let mut quadros = Vec::new();
        for pose in [Pose::PunchWindup, Pose::PunchStrike, Pose::PunchRecover] {
            let world = app.world_mut();
            let player = world
                .query_filtered::<Entity, With<Player>>()
                .iter(world)
                .next()
                .expect("o boneco sumiu");
            world.entity_mut(player).insert((
                pose,
                MeleeAttack {
                    step,
                    style: weapon.style(),
                    kind: melee,
                    move_data: if melee == MeleeKind::Heavy {
                        weapon.heavy().unwrap_or_else(|| weapon.melee(step))
                    } else {
                        weapon.melee(step)
                    },
                    launched: false,
                },
            ));
            // O icone persegue a mao com inercia; um quadro so mostraria a arma
            // ainda a caminho da pose, que e a foto errada.
            for _ in 0..14 {
                app.world_mut()
                    .resource_mut::<Time>()
                    .advance_by(std::time::Duration::from_millis(16));
                app.update();
            }
            quadros.push(stamp_scene(&mut app));
        }
        quadros
    }

    /// Carimba o que as entidades da arma realmente tem no `Transform` agora.
    fn stamp_scene(app: &mut App) -> String {
        let world = app.world_mut();
        let (art, flip, icon) = world
            .query_filtered::<(&AsciiSprite, &Transform), With<WeaponIcon>>()
            .iter(world)
            .map(|(sprite, at)| (sprite.art.clone(), sprite.flip_x, *at))
            .next()
            .expect("a arma sumiu da mao");
        let world = app.world_mut();
        let pecas: Vec<(AsciiArt, Transform)> = world
            .query_filtered::<(&AsciiSprite, &Transform), With<WeaponPart>>()
            .iter(world)
            .map(|(sprite, at)| (sprite.art.clone(), *at))
            .collect();

        let mut sheet = Sheet::new(Rect::from_corners(
            Vec2::new(-76.0, -60.0),
            Vec2::new(112.0, 80.0),
        ));
        sheet.stamp_fighter();
        let angle = icon.rotation.to_scaled_axis().z;
        let scale = icon.scale.truncate();
        let at = icon.translation.truncate();
        sheet.stamp(&art, flip, at, angle, scale);
        for (arte, local) in pecas {
            let world_at = at + Vec2::from_angle(angle).rotate(local.translation.truncate() * scale);
            sheet.stamp(
                &arte,
                false,
                world_at,
                angle + local.rotation.to_scaled_axis().z,
                local.scale.truncate() * scale,
            );
        }
        sheet.text()
    }

    /// A arma nos tres quadros de cada golpe, com os sistemas rodando:
    /// `cargo test olhar_o_combo -- --nocapture --ignored`.
    ///
    /// Segue o modelo de `olhar_o_dragao`. Um preview desenhado pela arte
    /// parada mostraria a arma na pose de mira -- a unica em que ela nao esta
    /// quando o golpe importa -- e a corrente do nunchaku, que so existe em
    /// movimento, nao apareceria em foto nenhuma.
    #[test]
    #[ignore = "so para olhar"]
    fn olhar_o_combo() {
        for kind in 0..ARSENAL.len() as u8 {
            let weapon = weapon_at(kind);
            println!("\n########## {} ##########", weapon.name());
            for (rotulo, melee, step) in [
                ("elo 1", MeleeKind::Chain, 0u8),
                ("elo 2", MeleeKind::Chain, 1),
                ("elo 3", MeleeKind::Chain, 2),
                ("pesado", MeleeKind::Heavy, 2),
            ] {
                if melee == MeleeKind::Heavy && weapon.heavy().is_none() {
                    continue;
                }
                let golpe = crate::actor::pose::strike_for(step, melee, weapon.style()).name;
                for (fase, arte) in run_strike(kind, melee, step).into_iter().enumerate() {
                    println!("\n--- {rotulo}: {golpe}, fase {fase} ---\n{arte}");
                }
            }
        }
    }

    /// Percorre o arsenal de verdade, nao uma lista a parte: arma nova entra
    /// nas invariantes sozinha, em vez de escapar delas por esquecimento.
    #[test]
    fn cada_arma_tem_combo_e_finalizador_proprios() {
        for make in ARSENAL {
            let weapon = make();
            assert!(
                weapon.melee(2).damage > weapon.melee(0).damage,
                "{}: o finalizador nao passa do primeiro golpe",
                weapon.name()
            );
            // O combate encadeia com `(step + 1) % 3`; o combo tem que girar
            // no mesmo passo, senao o quarto golpe sai do nada.
            assert_eq!(
                weapon.melee(3).damage,
                weapon.melee(0).damage,
                "{}: o combo nao volta ao inicio",
                weapon.name()
            );
        }
    }

    /// Contato e fogo sao excludentes: se a arma tem golpe pesado no M2, ela
    /// nao pode tambem ter municao, senao um botao faria duas coisas.
    #[test]
    fn arma_de_contato_nao_atira() {
        for make in ARSENAL {
            let weapon = make();
            if weapon.heavy().is_none() {
                assert!(
                    !weapon.is_melee(),
                    "{}: marcada como contato sem golpe pesado",
                    weapon.name()
                );
                continue;
            }
            assert!(weapon.is_melee(), "{}", weapon.name());
            assert_eq!(weapon.ammo(), 0, "{} tem municao", weapon.name());
            assert!(
                weapon.shots(Vec2::X).is_empty(),
                "{} produz projetil",
                weapon.name()
            );
        }
    }

    /// O golpe pesado tem que doer mais e demorar mais que o finalizador do
    /// combo -- se nao custar nada, nao ha motivo pra jogar de outro jeito.
    #[test]
    fn o_golpe_pesado_vale_a_preparacao() {
        for make in ARSENAL {
            let weapon = make();
            let Some(heavy) = weapon.heavy() else {
                continue;
            };
            let finisher = weapon.melee(2);
            assert!(
                heavy.damage > finisher.damage,
                "{}: o pesado nao dÃ³i mais que o finalizador",
                weapon.name()
            );
            assert!(
                heavy.duration > finisher.duration,
                "{}: o pesado nao custa mais tempo",
                weapon.name()
            );
        }
    }

    /// A arma arremessada vai para onde o cursor aponta.
    ///
    /// Antes ela saia sempre no sentido do olhar, e o mouse so valia para o
    /// tiro: com o oponente pendurado numa corrente acima, jogar a arma nele
    /// era impossivel mesmo com a mira em cima dele.
    #[test]
    fn o_arremesso_vai_para_a_mira() {
        for mira in [Vec2::Y, Vec2::NEG_Y, Vec2::new(-0.6, 0.8).normalize()] {
            let arco = thrown_arc(mira, 260.0, THROW_SPEED);
            let saida = (arco - Vec2::Y * 260.0).normalize();
            assert!(
                (saida - mira).length() < 0.001,
                "o arremesso ignorou a mira {mira:?} e saiu em {saida:?}"
            );
        }
    }

    /// Mirando na horizontal, o arremesso tem que ser o de sempre.
    ///
    /// O levante nao gira junto com a direcao: e ele que faz a arma descrever
    /// um arco em vez de viajar reta, e girar tudo trocaria o arco por um
    /// empurrao para tras cada vez que alguem mirasse para cima.
    #[test]
    fn o_arremesso_reto_nao_mudou() {
        for lado in [-1.0f32, 1.0] {
            let dir = Vec2::new(lado, 0.0);
            assert_eq!(
                thrown_arc(dir, 260.0, THROW_SPEED),
                Vec2::new(lado * THROW_SPEED, 260.0)
            );
            assert_eq!(
                thrown_arc(dir, 190.0, THROW_PUSH),
                Vec2::new(lado * THROW_PUSH, 190.0)
            );
        }
        // Mirando no chao, a arma desce: o levante nao pode salvar o arremesso
        // de cima para baixo.
        assert!(thrown_arc(Vec2::NEG_Y, 260.0, THROW_SPEED).y < 0.0);
    }

    /// O arco e o que distingue a bomba. Um arremesso que sai reto e so uma
    /// bala lenta.
    #[test]
    fn a_bomba_sai_em_arco() {
        // Mirando na horizontal, a componente vertical tem que existir mesmo
        // assim -- e ela que transforma a mira reta em arco.
        let shots = PipeBomb.shots(Vec2::X);
        assert_eq!(shots.len(), 1);
        assert!(shots[0].velocity.y > 0.0, "a bomba sai reta, sem arco");
        assert!(matches!(shots[0].kind, ShotKind::Lobbed { .. }));
    }

    /// Todo o dano esta no estouro, mesmo agora que encostar detona.
    ///
    /// Sao coisas diferentes: o toque decide *quando* estoura, o estouro decide
    /// *quanto* dÃ³i. Se a granada tambem machucasse de raspao, um encostao de
    /// lado cobraria duas vezes pelo mesmo acerto.
    #[test]
    fn a_bomba_nao_machuca_ao_encostar() {
        for shot in PipeBomb.shots(Vec2::X) {
            let ShotKind::Lobbed { damage, blast, .. } = shot.kind else {
                panic!("a bomba deixou de ser arremesso");
            };
            assert_eq!(shot.damage, 0, "a bomba machuca antes de estourar");
            assert!(damage > 0, "o estouro nao machuca");
            assert!(
                blast > PipeBomb.melee(2).reach,
                "o estouro nao cobre mais area que uma paulada"
            );
        }
    }

    /// So uma arma arqueia. Se duas passassem a fazer isso sem querer, o
    /// arsenal perderia o contraste que justifica ter cinco.
    #[test]
    fn o_arsenal_tem_um_unico_arremesso() {
        let arqueiam: Vec<&str> = ARSENAL
            .iter()
            .map(|make| make())
            .filter(|weapon| {
                weapon
                    .shots(Vec2::X)
                    .iter()
                    .any(|shot| matches!(shot.kind, ShotKind::Lobbed { .. }))
            })
            .map(|weapon| weapon.name())
            .collect();
        assert_eq!(arqueiam, vec!["BOMB"], "arsenal com arremessos demais");
    }

    /// Dois nomes iguais deixariam o HUD mentindo sobre o que esta na mao.
    #[test]
    fn o_arsenal_nao_repete_nome() {
        let mut names: Vec<&str> = ARSENAL.iter().map(|make| make().name()).collect();
        let total = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), total, "arsenal com nome repetido: {names:?}");
    }

    #[test]
    fn o_livro_dispara_sigilo_sem_capsula() {
        fn conjurar(mut commands: Commands) {
            arcane_bloom(&mut commands, Vec2::ZERO, Vec2::X);
        }

        let shot = MagicBook.shots(Vec2::X).remove(0);
        assert!(magic_runes().iter().any(|rune| rune.glyph == shot.glyph));
        assert_eq!(shot.kind, ShotKind::Arcane);
        assert_eq!(MagicBook.style(), WeaponStyle::Book);
        assert_eq!(
            book_model_parts(true).len(),
            labeled_elements(book_held_scene(), "livro_magico_aberto_vista_leste").len()
        );
        assert_eq!(
            book_model_parts(false).len(),
            labeled_elements(book_spawn_scene(), "magic book").len()
        );
        assert!(gunfire(WeaponLook::Book).is_none(), "magia virou arma de fogo");

        let mut app = App::new();
        app.add_systems(Update, conjurar);
        app.update();
        let world = app.world_mut();
        let mut flames = world.query::<&ArcaneFlame>();
        assert_eq!(flames.iter(world).count(), 16, "a flor arcana perdeu petalas");
        assert_eq!(magic_runes().len(), 8);
    }

    /// So a faca crava. Se duas armas passassem a fazer isso sem querer, o
    /// arsenal perderia o contraste que justifica ter seis.
    #[test]
    fn o_arsenal_tem_um_unico_projetil_que_crava() {
        let cravam: Vec<&str> = ARSENAL
            .iter()
            .map(|make| make())
            .filter(|weapon| {
                weapon
                    .shots(Vec2::X)
                    .iter()
                    .any(|shot| shot.kind == ShotKind::Sticky)
            })
            .map(|weapon| weapon.name())
            .collect();
        assert_eq!(cravam, vec!["KNIVES"]);
    }

    /// A granada tem que estourar ao encostar num corpo, e nao so quando o
    /// pavio acaba -- e essa a diferenca entre mirar num lugar e mirar em
    /// alguem.
    #[test]
    fn a_granada_estoura_no_toque() {
        // Pavio longo de proposito: se estourar num unico quadro, foi o toque.
        fn arena(distancia: f32) -> App {
            let mut app = App::new();
            app.init_resource::<Time>()
                .add_message::<crate::fx::Shake>()
                .add_systems(Update, tick_fuses);
            let dono = app
                .world_mut()
                .spawn((
                    Player {
                        id: 0,
                        color: palette::player(0),
                    },
                    Transform::from_xyz(-400.0, 0.0, 0.0),
                    Collider::size(30.0, 60.0),
                ))
                .id();
            app.world_mut().spawn((
                Player {
                    id: 1,
                    color: palette::player(1),
                },
                Transform::from_xyz(distancia, 0.0, 0.0),
                Collider::size(30.0, 60.0),
            ));
            app.world_mut().spawn((
                Fuse {
                    timer: Timer::from_seconds(30.0, TimerMode::Once),
                    blast: 150.0,
                    damage: 30,
                    owner: dono,
                },
                Transform::default(),
            ));
            app
        }

        let mut app = arena(10.0);
        app.update();
        let world = app.world_mut();
        assert_eq!(
            world.query::<&Fuse>().iter(world).count(),
            0,
            "a granada encostou no boneco e nao estourou"
        );
        // E o estouro tem que se anunciar como estouro: e o que a camada de
        // sangue le para desmontar quem morreu nele.
        assert_eq!(
            world
                .query_filtered::<&Hitbox, With<crate::combat::Explosive>>()
                .iter(world)
                .count(),
            1,
            "o estouro nao deixou hitbox de explosao"
        );

        let mut longe = arena(600.0);
        longe.update();
        let world = longe.world_mut();
        assert_eq!(
            world.query::<&Fuse>().iter(world).count(),
            1,
            "a granada estourou sem encostar em ninguem"
        );
    }

    #[test]
    fn arma_vazia_e_pega_e_nao_desaparece_ao_disparar() {
        let mut app = App::new();
        app.init_resource::<Time>()
            .add_message::<crate::fx::Shake>()
            .add_systems(Update, (pick_up, fire).chain());
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
                Transform::default(),
                Velocity::default(),
                Collider::size(30.0, 30.0),
            ))
            .id();
        app.world_mut().spawn((
            GroundWeapon { kind: 0, ammo: 0 },
            Transform::default(),
            Collider::size(30.0, 30.0),
        ));

        app.update();

        assert_eq!(app.world().get::<Held>(player).unwrap().ammo, 0);
    }

    /// Largar uma arma e pisar noutra no mesmo quadro nao pode deixar a mao
    /// vazia na tela.
    ///
    /// `throw_weapon` tira o `Held` e `pick_up` devolve logo atras, os dois no
    /// mesmo quadro. Enquanto a limpeza do icone escutava a remocao do
    /// componente, o aviso sobrevivia a essa ida e volta e matava o icone que
    /// `equip` tinha acabado de criar: o boneco ficava armado e atirando, com
    /// arma nenhuma desenhada, ate perder a arma de novo.
    #[test]
    fn trocar_de_arma_no_mesmo_quadro_nao_apaga_o_icone() {
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<NextWeaponId>()
            .add_systems(Update, (throw_weapon, pick_up, clear_weapon_icon).chain());

        let player = app
            .world_mut()
            .spawn((
                Player {
                    id: 0,
                    color: palette::player(0),
                },
                Intent {
                    throw_weapon: true,
                    ..default()
                },
                Pose::IdleA,
                Facing(1.0),
                Transform::default(),
                Velocity::default(),
                Collider::size(30.0, 30.0),
            ))
            .id();
        // A arma que ele ja carrega, montada como `equip` a monta.
        app.world_mut().entity_mut(player).insert(held_weapon(0, 3));
        app.world_mut().spawn((
            WeaponIcon,
            AsciiSprite::new(AsciiArt::solid(weapon_at(0).held_art(), palette::GOLD)),
            Layer::Actor,
            Transform::default(),
            ChildOf(player),
        ));
        // E outra parada exatamente onde ele pisa.
        app.world_mut().spawn((
            GroundWeapon { kind: 1, ammo: 5 },
            Transform::default(),
            Collider::size(30.0, 30.0),
        ));

        app.update();

        assert!(
            app.world().get::<Held>(player).is_some(),
            "o boneco largou uma arma, pegou outra e ficou de maos vazias"
        );
        let world = app.world_mut();
        assert_eq!(
            world
                .query_filtered::<Entity, With<WeaponIcon>>()
                .iter(world)
                .count(),
            1,
            "a arma na mao ficou sem icone (ou ganhou dois empilhados)"
        );
    }

    /// Arma que cai no vao para de existir.
    ///
    /// Nada a apagava: o colisor nao encosta em nada la embaixo, entao ela caia
    /// para sempre -- invisivel, impossivel de pegar, e ainda assim anunciada
    /// em todo pacote de armas que o dono manda.
    #[test]
    fn arma_que_cai_no_vao_some() {
        let mut app = App::new();
        app.add_systems(Update, sink_lost_weapons);

        let perdida = app
            .world_mut()
            .spawn((
                GroundWeapon { kind: 0, ammo: 1 },
                Transform::from_xyz(0.0, KILL_Y - 1.0, 0.0),
            ))
            .id();
        let no_chao = app
            .world_mut()
            .spawn((
                GroundWeapon { kind: 0, ammo: 1 },
                Transform::from_xyz(0.0, KILL_Y + 1.0, 0.0),
            ))
            .id();

        app.update();

        assert!(app.world().get_entity(perdida).is_err());
        assert!(
            app.world().get_entity(no_chao).is_ok(),
            "a arma que ainda esta na arena foi levada junto"
        );
    }

    /// Arma caida cabe ao lado de quem vai pegar ela.
    ///
    /// A arte do chao existe para ser lida de longe e por isso e maior que a da
    /// mao -- so que ela ia para a tela em escala crua, sem nada equivalente ao
    /// `held_scale`. Uma katana deitada media 136 unidades e o boneco tem 64 de
    /// altura por 24 de largura: a arma era duas vezes o lutador, e cada sprite,
    /// sozinho, continuava certo. So medindo os dois na mesma regua aparece.
    ///
    /// O teto e o boneco mais uma cabeca no comprimento -- espada e rifle sao
    /// compridos por natureza -- e meio boneco na espessura.
    #[test]
    fn a_arma_caida_cabe_ao_lado_de_quem_pega() {
        let alto = BODY_ROWS as f32 * CELL.y;
        let comprimento = alto + CELL.y;
        let espessura = alto * 0.5;
        for make in ARSENAL {
            let weapon = make();
            let caixa = weapon_art(weapon.ground_art()).size() * GROUND_SCALE;
            assert!(
                caixa.x <= comprimento,
                "{}: {:.0} de comprimento no chao, e o boneco todo tem {alto:.0}",
                weapon.name(),
                caixa.x
            );
            assert!(
                caixa.y <= espessura,
                "{}: {:.0} de altura no chao, mais que meio boneco",
                weapon.name(),
                caixa.y
            );
        }
    }

    /// A respiracao da arma caida nao pode devolver ela ao tamanho cru.
    ///
    /// `spawn_ground_weapon` nasce com `GROUND_SCALE` e `pulse_pickups` reescreve
    /// a escala inteira todo quadro. Enquanto ele escrevia `1.0 + onda`, o
    /// encolhimento durava exatamente um quadro: a arma nascia do tamanho certo e
    /// pulava para o tamanho cru antes de qualquer um ver. E o pior tipo de bug
    /// de escala, porque o codigo que a encolhe continua ali, correto, e o outro
    /// so nao sabe que ele existe.
    #[test]
    fn a_respiracao_nao_devolve_a_arma_ao_tamanho_cru() {
        let mut app = App::new();
        app.init_resource::<Time>().add_systems(Update, pulse_pickups);
        let arma = app
            .world_mut()
            .spawn((
                PickupPulse(0.0),
                Transform::from_scale(Vec3::splat(GROUND_SCALE)),
            ))
            .id();

        app.update();

        let escala = app.world().get::<Transform>(arma).unwrap().scale.x;
        assert!(
            (escala - GROUND_SCALE).abs() <= GROUND_SCALE * 0.1,
            "a arma caida respirou de {GROUND_SCALE} para {escala}"
        );
    }

    /// O chao para de receber arma quando a fase ja tem o bastante.
    ///
    /// Antes so o cronometro mandava: sete segundos, mais uma arma, para sempre.
    /// Fase longa acabava carpetada, e a disputa por arma -- que e a coisa que o
    /// drop existe para criar -- sumia junto, porque sempre havia outra a um
    /// passo de qualquer um.
    ///
    /// As armas em maos contam. Contar so as caidas repoe o estoque toda vez que
    /// alguem pega uma: o chao esvazia, o contador zera, e o cronometro larga
    /// outra em cima de uma fase que ja esta armada ate os dentes.
    #[test]
    fn o_chao_para_de_receber_arma_quando_a_fase_ja_tem() {
        fn arena(caidas: usize, armados: usize, desarmados: usize) -> usize {
            let mut app = App::new();
            app.init_resource::<Time>()
                .init_resource::<NextWeaponId>()
                .insert_resource(CurrentLevel(crate::level::level_at(0)))
                .insert_resource(DropSchedule(Timer::from_seconds(
                    0.0,
                    TimerMode::Repeating,
                )))
                .add_systems(Update, drop_weapons);
            for _ in 0..caidas {
                app.world_mut()
                    .spawn((GroundWeapon { kind: 0, ammo: 1 }, Transform::default()));
            }
            for id in 0..(armados + desarmados) {
                let mut lutador = app.world_mut().spawn((
                    Player {
                        id: id as u8,
                        color: palette::player(id as u8),
                    },
                    Transform::default(),
                ));
                if id < armados {
                    lutador.insert(held_weapon(0, 1));
                }
            }

            app.update();
            let world = app.world_mut();
            world.query::<&GroundWeapon>().iter(world).count()
        }

        // Dois lutadores desarmados e nada no chao: o orcamento e tres, entao cai.
        assert_eq!(arena(0, 0, 2), 1, "a fase vazia ficou sem arma nenhuma");
        // Dois lutadores armados e uma no chao: tres de tres, nao cabe mais.
        assert_eq!(
            arena(1, 2, 0),
            1,
            "o drop ignorou as armas que ja estao em maos"
        );
        // As mesmas tres armas, so que todas no chao: o teto e o mesmo.
        assert_eq!(arena(3, 0, 2), 3, "o drop passou do teto do cenario");
        // Mais gente, mais orcamento: e a conta que faz sala cheia nao secar.
        assert_eq!(arena(3, 0, 4), 4, "sala cheia ficou sem reposicao");
    }
