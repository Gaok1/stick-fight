/// Quantas vertebras o corpo tem, e quanto de arco separa uma da outra.
///
/// O produto e o comprimento do bicho: quatrocentos e tantos, quase meia tela.
/// Fita curta le como minhoca, e dragao chines e antes de tudo comprimento.
const SCALES: usize = 26;
const SCALE_SPAN: f32 = 18.0;
/// Onde a primeira vertebra fica, medida do centro da cabeca para tras.
const NECK: f32 = 74.0;

/// Passo do rastro, em unidades de arco.
///
/// Metade do vao entre vertebras: cada uma cai entre dois pontos gravados, e a
/// interpolacao faz o resto. Passo igual ao vao economizaria memoria e faria o
/// corpo inteiro andar de dezoito em dezoito -- ondulacao viraria tremor.
const TRAIL_STEP: f32 = 9.0;
/// Quantos pontos guardar: o corpo inteiro mais a folga da tangente do rabo.
const TRAIL_LEN: usize = 66;

/// A batida do peito, em voltas por segundo.
///
/// Lenta de proposito. Respiracao rapida nao le como respiracao -- le como
/// vibracao, que e exatamente o que a versao anterior fazia.
const BREATH_HZ: f32 = 0.42;

/// Onde nasce cada bigode na cabeca, quantos nos ele tem e quanto mede cada
/// elo.
///
/// Coordenada da arte, nao do mundo: [`Dragon::on_head`] resolve giro e
/// espelho. Os dois primeiros sao o bigode do focinho; o terceiro e a barba do
/// queixo, mais curta e mais pesada.
const WHISKERS: [(Vec2, usize, f32); 3] = [
    (Vec2::new(74.0, -2.0), 11, 13.0),
    (Vec2::new(70.0, -16.0), 10, 12.0),
    (Vec2::new(46.0, -52.0), 6, 10.0),
];
/// Peso do fio, em aceleracao. Baixo: bigode nao e corrente.
const WHISKER_WEIGHT: f32 = 210.0;
/// Quanto da inercia sobrevive a um quadro.
const WHISKER_DRAG: f32 = 0.94;
/// Amplitude do bafejo que ondula o fio parado.
///
/// Sem ele o bigode fica pendurado e imovel quando o bicho para, e fio imovel
/// le como vareta. Com ele o jardim inteiro parece ter ar.
const WHISKER_WAVE: f32 = 320.0;
/// Passadas de restricao por quadro. Duas ja seguram; tres nao deixam o fio
/// esticar visivelmente quando a cabeca arranca para a rasante.
const WHISKER_PASSES: usize = 3;

/// A boca, em coordenada da arte da cabeca: o vao entre as duas mandibulas.
const MAW_LOCAL: Vec2 = Vec2::new(74.0, -24.0);

/// O que o dragao esta fazendo.
///
/// A ordem e o ciclo: enrolado no ceu, sobe, varre a pista com fogo e volta.
/// Sem os dois estados de transicao o bicho apareceria e sumiria da rasante
/// como se tivesse sido teletransportado -- e o que faz a passada valer nao e
/// o fogo, e a subida antes dele.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mood {
    /// Desenhando o oito no alto, respirando e olhando quem briga.
    Coiled,
    /// Ganhando altura na ponta oposta da arena, juntando luz na boca.
    Rise,
    /// Descendo a arena de ponta a ponta, cuspindo fogo na pista.
    Dive,
    /// Voltando para o oito.
    Roost,
}

impl Mood {
    /// Quanto o estado dura, em segundos.
    ///
    /// A rasante e longa porque metade dela e a curva de cima: o bicho sobe
    /// para um lado, vira em cima de si mesmo e so entao atravessa. Cortar
    /// esse tempo corta a travessia, nao a curva.
    const fn span(self) -> f32 {
        match self {
            Self::Coiled => 15.0,
            Self::Rise => 2.4,
            Self::Dive => 4.2,
            Self::Roost => 5.5,
        }
    }

    /// Quao rapido a cabeca anda, e quao fechado ela consegue virar.
    const fn flight(self) -> (f32, f32) {
        match self {
            Self::Coiled => (128.0, 2.6),
            Self::Rise => (250.0, 2.2),
            Self::Dive => (360.0, 2.1),
            Self::Roost => (215.0, 2.4),
        }
    }
}

/// O bicho: onde a cabeca esta, para onde ela aponta, e o rastro que o corpo
/// inteiro le.
#[derive(Component)]
struct Dragon {
    /// Rastro da cabeca em passo fixo de arco, do mais novo para o mais velho.
    trail: Vec<Vec2>,
    /// Arco andado desde o ultimo ponto gravado. E ele que tira o degrau da
    /// amostragem.
    spun: f32,
    at: Vec2,
    /// Direcao de viagem.
    heading: f32,
    /// Para onde a cabeca aponta -- que nem sempre e para onde ela vai: no
    /// oito ele torce o pescoco para acompanhar a briga.
    facing: f32,
    /// A arte olha para a esquerda? Guardado, e nao recalculado, porque o
    /// sinal do cosseno oscila quando a cabeca sobe reta e o bicho piscaria.
    flip: bool,
    mood: Mood,
    /// Segundos no estado atual.
    age: f32,
    /// Parametro do oito. Anda sempre, inclusive fora do oito: e ele que diz
    /// onde voltar a entrar na curva.
    lap: f32,
    /// De que lado a rasante comeca. `-1` esquerda, `+1` direita.
    lane: f32,
    /// Emissao fracionaria: deixa o sopro independente do framerate.
    puff: f32,
}

/// Uma vertebra, que so precisa saber a que altura do rastro ela mora.
#[derive(Component)]
struct DragonScale(usize);

/// Um no de bigode. A posicao mora no [`Parallax`]; aqui fica o que a fisica
/// precisa saber alem dela.
#[derive(Component)]
struct Whisker {
    /// Qual fio.
    chain: u8,
    /// Distancia do focinho, em nos. O zero e o proprio focinho.
    index: u16,
    link: f32,
    /// Onde este no estava no quadro passado: em Verlet a velocidade nao se
    /// guarda, se mede.
    prev: Vec2,
}

/// O ponto do oito no parametro `lap`.
///
/// Lemniscata de Gerono, e nao uma senoide: a senoide faz o bicho ir e voltar
/// pelo mesmo caminho, e o que da o gesto de dragao chines e ele cruzar por
/// cima do proprio corpo no meio da tela.
fn loop_at(lap: f32) -> Vec2 {
    DRAGON_SKY + Vec2::new(DRAGON_LOOP.x * lap.sin(), DRAGON_LOOP.y * (lap * 2.0).sin())
}

/// Diferenca entre dois angulos, sempre pelo lado curto.
fn shortest(from: f32, to: f32) -> f32 {
    (to - from + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

impl Dragon {
    /// Nasce ja esticado sobre o oito, e nao numa reta.
    ///
    /// Sem isto o bicho comeca como um pau e leva a volta inteira para virar
    /// cobra -- e a volta inteira e o primeiro round.
    fn new() -> Self {
        let lap = 0.35;
        Self {
            trail: seeded(lap),
            spun: 0.0,
            at: loop_at(lap),
            heading: 0.0,
            facing: 0.0,
            flip: false,
            mood: Mood::Coiled,
            age: 0.0,
            lap,
            lane: -1.0,
            puff: 0.0,
        }
    }

    /// Um ponto do desenho da cabeca, ja no mundo.
    ///
    /// Espelhar e girar meia volta nao dao no mesmo, e e por isso que o `y`
    /// inverte junto quando o bicho vira para a esquerda: sem essa troca o
    /// fogo sairia da nuca dele em metade do oito, e os bigodes nasceriam
    /// dentro do craneo.
    fn on_head(&self, local: Vec2) -> Vec2 {
        let local = if self.flip {
            Vec2::new(local.x, -local.y)
        } else {
            local
        };
        self.at + Vec2::from_angle(self.facing).rotate(local)
    }

    /// A boca, de onde sai tudo que o bicho cospe.
    fn maw(&self) -> Vec2 {
        self.on_head(MAW_LOCAL)
    }

    /// O ponto do rastro a `back` de arco atras da cabeca.
    ///
    /// A cabeca esta sempre entre dois pontos gravados, e e o `spun` que diz
    /// onde: sem ele o corpo saltaria um passo inteiro a cada gravacao.
    fn sample(&self, back: f32) -> Vec2 {
        let steps = (back - self.spun) / TRAIL_STEP;
        if steps <= 0.0 {
            // Ainda dentro do pedaco que a cabeca acabou de andar.
            let first = self.trail.first().copied().unwrap_or(self.at);
            let t = if self.spun > 0.0 { back / self.spun } else { 0.0 };
            return self.at.lerp(first, t.clamp(0.0, 1.0));
        }
        let low = (steps.floor() as usize).min(self.trail.len() - 1);
        let high = (low + 1).min(self.trail.len() - 1);
        self.trail[low].lerp(self.trail[high], steps.fract())
    }

    /// Onde mora a vertebra `n`, e para onde ela aponta.
    ///
    /// A tangente sai do vizinho de tras, e nao de um angulo guardado a parte:
    /// angulo guardado sai de sincronia com a posicao na primeira curva
    /// fechada, e a vertebra passa a apontar para fora do proprio corpo.
    fn joint(&self, n: usize) -> (Vec2, f32) {
        let back = NECK + n as f32 * SCALE_SPAN;
        let at = self.sample(back);
        let behind = self.sample(back + SCALE_SPAN);
        let along = at - behind;
        let angle = if along.length_squared() > 0.0001 {
            along.to_angle()
        } else {
            self.heading
        };
        (at, angle)
    }

    /// Para onde a cabeca esta indo agora.
    fn target(&self) -> Vec2 {
        match self.mood {
            // O alvo corre na frente pelo proprio oito: a cabeca persegue e,
            // perseguindo, corta a curva por dentro. E esse atraso -- e nao
            // uma senoide somada por cima -- que da a nado da serpente.
            Mood::Coiled | Mood::Roost => loop_at(self.lap + 0.55),
            // Sobe pelo lado de onde a rasante vai comecar.
            Mood::Rise => Vec2::new(self.lane * 560.0, DRAGON_SKY.y + 66.0),
            // E atravessa a arena na altura da rasante, atras de um ponto que
            // corre com ele. A volta la em cima sai de graca: enquanto ele
            // ainda aponta para fora, esse ponto esta as costas dele.
            Mood::Dive => Vec2::new(self.at.x - self.lane * DIVE_LEAD, DRAGON_DIVE),
        }
    }

    /// Guarda o rastro depois que a cabeca andou `walked`.
    fn record(&mut self, walked: f32) {
        self.spun += walked;
        while self.spun >= TRAIL_STEP {
            self.spun -= TRAIL_STEP;
            // O ponto entra onde a cabeca estava um passo atras, e nao onde
            // ela esta: a cabeca ja passou dali.
            let back = self.at - Vec2::from_angle(self.heading) * self.spun;
            self.trail.insert(0, back);
            self.trail.truncate(TRAIL_LEN);
        }
    }
}

/// O rastro de partida: o oito ja percorrido, medido de tras para frente.
fn seeded(lap: f32) -> Vec<Vec2> {
    let mut trail = Vec::with_capacity(TRAIL_LEN);
    let mut probe = lap;
    let mut last = loop_at(lap);
    let mut spun = 0.0;
    while trail.len() < TRAIL_LEN {
        probe -= 0.002;
        let at = loop_at(probe);
        spun += at.distance(last);
        last = at;
        if spun >= TRAIL_STEP {
            spun -= TRAIL_STEP;
            trail.push(at);
        }
    }
    trail
}

/// Poe o dragao no ar. So o jardim tem um.
fn hatch_dragon(commands: &mut Commands, scene: Scene) {
    if scene != Scene::DragonGarden {
        return;
    }
    let dragon = Dragon::new();

    for n in 0..SCALES {
        let (at, angle) = dragon.joint(n);
        // Perna em duas vertebras e nadadeira na ultima: garra e pontuacao,
        // nao padrao. Repetida em todas, o corpo vira uma lagarta.
        let art = match n {
            n if n + 1 == SCALES => DRAGON_TAIL,
            3 | 11 => DRAGON_LIMB,
            _ => DRAGON_SCALE,
        };
        commands.spawn((
            LevelGeometry,
            Parallax {
                home: at,
                depth: MID,
            },
            DragonScale(n),
            AsciiSprite::new(AsciiArt::tinted(art, &JADE_SKIN, palette::SCENE_JADE)),
            Layer::Background,
            Transform::from_translation(at.extend(-MID))
                .with_rotation(Quat::from_rotation_z(angle)),
        ));
    }

    for (chain, &(root, nodes, link)) in WHISKERS.iter().enumerate() {
        for index in 0..nodes {
            // Todo no nasce no focinho: a primeira restricao ja os espalha, e
            // um quadro de bigode encolhido ninguem ve.
            let at = dragon.on_head(root);
            commands.spawn((
                LevelGeometry,
                Parallax {
                    home: at,
                    depth: MID,
                },
                Whisker {
                    chain: chain as u8,
                    index: index as u16,
                    link,
                    prev: at,
                },
                AsciiSprite::new(whisker_art(index, nodes)),
                Layer::Background,
                Transform::from_translation(at.extend(-MID)),
            ));
        }
    }

    commands.spawn((
        LevelGeometry,
        Parallax {
            home: dragon.at,
            depth: MID,
        },
        AsciiSprite::new(AsciiArt::tinted(
            DRAGON_HEAD,
            &JADE_SKIN,
            palette::SCENE_JADE,
        )),
        Layer::Background,
        Transform::from_translation(dragon.at.extend(-MID)),
        dragon,
    ));
}

/// O glifo de um no de bigode: o fio acende da raiz para a ponta.
fn whisker_art(index: usize, nodes: usize) -> AsciiArt {
    let tip = index + 1 >= nodes;
    let far = index * 3 >= nodes * 2;
    match (tip, far) {
        (true, _) => AsciiArt::glyph('°', palette::SCENE_GOLD),
        (_, true) => AsciiArt::glyph('~', palette::SCENE_GOLD),
        _ => AsciiArt::glyph('~', palette::SCENE_JADE_LIT),
    }
}

/// Conduz a cabeca: escolhe o que fazer, vira, anda e sopra.
///
/// Vira antes de andar, e com limite de giro, porque e o limite que faz a
/// trajetoria virar arco. Sem ele a cabeca aponta para o alvo no primeiro
/// quadro e a curva inteira -- que e o que o corpo vai copiar -- desaparece.
fn fly_dragon(
    time: Res<Time>,
    focus: Res<Focus>,
    mut commands: Commands,
    mut shake: MessageWriter<Shake>,
    mut heads: Query<(&mut Dragon, &mut Parallax, &mut Transform, &mut AsciiSprite)>,
) {
    let dt = time.delta_secs();
    let now = time.elapsed_secs();
    let swell = (now * BREATH_HZ * std::f32::consts::TAU).sin();

    for (mut dragon, mut plane, mut transform, mut sprite) in &mut heads {
        dragon.age += dt;
        dragon.lap += dt * 0.55;

        // --- o que fazer agora ---
        let done = match dragon.mood {
            // Volta a se enrolar assim que reencontra o oito, e nao quando o
            // relogio manda: chegar em cima da curva e o que fecha o gesto.
            Mood::Roost => {
                dragon.at.distance(loop_at(dragon.lap)) < 70.0 || dragon.age > Mood::Roost.span()
            }
            mood => dragon.age > mood.span(),
        };
        if done {
            dragon.age = 0.0;
            dragon.mood = match dragon.mood {
                Mood::Coiled => {
                    // Comeca a subida pelo lado em que ele ja esta: subir
                    // atravessando a arena inteira antes da rasante gasta o
                    // suspense duas vezes.
                    dragon.lane = if dragon.at.x < 0.0 { -1.0 } else { 1.0 };
                    Mood::Rise
                }
                Mood::Rise => {
                    shake.write(Shake(0.30));
                    Mood::Dive
                }
                Mood::Dive => Mood::Roost,
                Mood::Roost => Mood::Coiled,
            };
        }

        // --- virar e andar ---
        let (speed, turn) = dragon.mood.flight();
        let want = (dragon.target() - dragon.at).to_angle();
        dragon.heading += shortest(dragon.heading, want).clamp(-turn * dt, turn * dt);
        let walked = speed * dt;
        let step = Vec2::from_angle(dragon.heading) * walked;
        dragon.at += step;
        dragon.record(walked);
        plane.home = dragon.at;

        // --- para onde olhar ---
        // Enrolado, o pescoco torce para acompanhar a briga; em voo, a cabeca
        // segue a viagem. Um bicho que olha para o jogador enquanto desliza e
        // a diferenca entre cenario e presenca -- mas so ate meio radiano: o
        // pescoco que gira sozinho para tras vira coruja, nao dragao.
        let watch = if dragon.mood == Mood::Coiled {
            let eye = shortest(dragon.heading, (focus.0 - dragon.at).to_angle());
            eye.clamp(-0.55, 0.55)
        } else {
            0.0
        };
        let aim = dragon.heading + watch;
        dragon.facing += shortest(dragon.facing, aim) * (1.0 - (-dt * 6.0).exp());

        // Espelha com folga: perto da vertical o cosseno troca de sinal a toa,
        // e a cabeca piscaria de um lado para o outro no meio da subida.
        let cos = dragon.facing.cos();
        if cos.abs() > 0.18 {
            let flip = cos < 0.0;
            if dragon.flip != flip {
                dragon.flip = flip;
                sprite.flip_x = flip;
            }
        }
        transform.rotation = Quat::from_rotation_z(if dragon.flip {
            dragon.facing + std::f32::consts::PI
        } else {
            dragon.facing
        });
        // O peito enche e esvazia. Cinco por cento de uma cabeca de cento e
        // setenta pixels sao oito pixels indo e vindo: isso se ve.
        transform.scale = Vec3::new(1.0, 1.0 + swell * 0.05, 1.0);

        // --- o que sai da boca ---
        let maw = dragon.maw();
        // `facing` ja e a direcao no mundo: o espelho e coisa do desenho, e
        // quem resolve ele e o `on_head`. Descontar o espelho aqui tambem
        // apontaria o sopro para tras em metade do oito.
        let ahead = Vec2::from_angle(dragon.facing);
        match dragon.mood {
            // Fogo para a frente e para baixo: e a pista que ele quer varrer,
            // nao o ceu. E so depois de ja ter descido -- na volta la em cima
            // ele ainda esta virando, e fogo cuspido ali cai fora da arena e
            // gasta o numero antes de ele comecar.
            Mood::Dive if dragon.at.y < DRAGON_SKY.y + 20.0 => {
                let aim = (ahead * 0.55 + Vec2::NEG_Y).normalize_or_zero();
                dragon.puff += dt * 150.0;
                while dragon.puff >= 1.0 {
                    dragon.puff -= 1.0;
                    let speed = 320.0 + fastrand::f32() * 190.0;
                    jade_flame(&mut commands, maw, aim, speed, MID);
                }
            }
            Mood::Rise => {
                // Junta luz na boca antes de soltar. A antecipacao e o que faz
                // a rasante ler como decisao do bicho e nao como acidente.
                dragon.puff += dt * 26.0;
                while dragon.puff >= 1.0 {
                    dragon.puff -= 1.0;
                    let angle = fastrand::f32() * std::f32::consts::TAU;
                    let at = maw + Vec2::from_angle(angle) * (24.0 + fastrand::f32() * 46.0);
                    ember(&mut commands, at, MID, palette::JADE);
                }
            }
            _ => {
                // Parado, ele respira: um fio de vapor sai do focinho na
                // virada do peito. E isto, e nao o sobe-e-desce do lombo, que
                // faz a respiracao aparecer de longe.
                dragon.puff += dt;
                if swell > 0.72 && dragon.puff > 1.0 {
                    dragon.puff = 0.0;
                    for _ in 0..3 {
                        let drift = Vec2::from_angle(fastrand::f32() * 0.7 - 0.15);
                        jade_flame(
                            &mut commands,
                            maw,
                            (ahead * 0.7 + drift * 0.5).normalize_or_zero(),
                            42.0 + fastrand::f32() * 26.0,
                            MID,
                        );
                    }
                }
            }
        }
    }
}

/// Poe cada vertebra no ponto do rastro que e dela.
///
/// O corpo nunca decide nada: ele so le. E isso que garante que ele passe
/// exatamente por onde a cabeca passou, sem emenda aberta nem cotovelo.
fn coil_dragon(
    time: Res<Time>,
    dragon: Query<&Dragon>,
    mut scales: Query<(&DragonScale, &mut Parallax, &mut Transform)>,
) {
    let Ok(dragon) = dragon.single() else {
        return;
    };
    let now = time.elapsed_secs();

    for (scale, mut plane, mut transform) in &mut scales {
        let (at, angle) = dragon.joint(scale.0);
        plane.home = at;
        transform.rotation = Quat::from_rotation_z(angle);

        // A onda do peito desce pelo corpo com atraso, em vez de inflar tudo
        // junto: e a diferenca entre um bicho que respira e um bicho que pisca
        // de tamanho.
        let wave =
            (now * BREATH_HZ * std::f32::consts::TAU - scale.0 as f32 * 0.24).sin() * 0.11 + 1.0;
        transform.scale = Vec3::new(1.0, girth(scale.0) * wave, 1.0);
    }
}

/// Quanto corpo tem a vertebra `n`: grossa no pescoco, fina no rabo.
///
/// A curva nao e reta porque dragao chines nao afina de forma constante --
/// engrossa logo depois do pescoco e so entrega o rabo no fim.
fn girth(n: usize) -> f32 {
    let t = n as f32 / (SCALES - 1) as f32;
    (1.0 - t).powf(0.62) * 0.86 + 0.2
}

/// Os bigodes: Verlet livre, depois a corda encurtada de volta ao tamanho.
///
/// Fio pendurado nao precisa de mola nem de massa: basta cada no lembrar onde
/// estava e alguem, depois, obrigar o elo a medir o que mede. O que o olho le
/// como peso e o atraso entre a cabeca arrancar e o fio saber disso.
fn wave_whiskers(
    time: Res<Time>,
    dragon: Query<&Dragon>,
    mut nodes: Query<(Entity, &mut Whisker, &mut Parallax, &mut Transform)>,
) {
    let Ok(dragon) = dragon.single() else {
        return;
    };
    // Travado: um quadro longo -- carga de fase, janela arrastada -- manda o
    // bigode para o outro lado da tela, e ele nunca mais volta.
    let dt = time.delta_secs().min(1.0 / 30.0);
    let now = time.elapsed_secs();

    for (_, mut node, mut plane, _) in &mut nodes {
        let drift = (plane.home - node.prev) * WHISKER_DRAG;
        node.prev = plane.home;
        // O bafejo e o que faz o fio parecer submerso em vez de pendurado.
        let breeze = (now * 2.9 + node.index as f32 * 0.75 + node.chain as f32 * 2.1).sin();
        plane.home += drift + Vec2::new(breeze * WHISKER_WAVE, -WHISKER_WEIGHT) * dt * dt;
    }

    // Do focinho para a ponta, e sempre nessa ordem: quem esta mais perto da
    // cabeca manda, quem esta mais longe cede. Ao contrario o bigode puxaria a
    // cabeca junto e o bicho andaria de re.
    let mut order: Vec<(u8, u16, Entity)> = nodes
        .iter()
        .map(|(entity, node, ..)| (node.chain, node.index, entity))
        .collect();
    order.sort_unstable();

    for _ in 0..WHISKER_PASSES {
        let mut lead = Vec2::ZERO;
        for &(chain, index, entity) in &order {
            let Ok((_, node, mut plane, mut transform)) = nodes.get_mut(entity) else {
                continue;
            };
            if index == 0 {
                // O no zero e o proprio focinho: ele nao cede nunca.
                lead = dragon.on_head(WHISKERS[chain as usize].0);
                plane.home = lead;
                transform.rotation = Quat::from_rotation_z(dragon.facing);
                continue;
            }
            let mut along = (plane.home - lead).normalize_or_zero();
            if along == Vec2::ZERO {
                along = Vec2::NEG_Y;
            }
            lead += along * node.link;
            plane.home = lead;
            // O til aponta pelo fio: sem isso a ponta do bigode le como uma
            // fileira de tracinhos soltos, e nao como um fio so.
            transform.rotation = Quat::from_rotation_z(along.to_angle());
        }
    }
}

// --- clima ------------------------------------------------------------------

