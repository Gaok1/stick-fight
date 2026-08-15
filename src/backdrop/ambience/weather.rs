/// O que cai do ceu num tema, e como.
///
/// Uma tabela e nao quatro sistemas: chuva, cinza, fuligem e petala so diferem
/// em glifo, cor e ritmo. Enquanto foram codigo separado, so a que alguem estava
/// olhando ganhava conserto.
struct Weather {
    glyphs: &'static [char],
    colors: &'static [Color],
    count: usize,
    /// Queda por segundo, ja negativa.
    fall: f32,
    /// Deriva lateral constante -- vento.
    slant: f32,
    /// Amplitude do bamboleio de quem cai devagar.
    sway: f32,
    depth: f32,
}

/// Uma particula de clima. Ela nunca morre: ao sair da moldura volta pelo topo.
///
/// Sem isto, chuva e cinza seriam spawn e despawn a cada quadro -- centenas de
/// entidades nascendo e morrendo por segundo para desenhar o que nunca muda.
#[derive(Component)]
struct Drift {
    fall: f32,
    slant: f32,
    sway: f32,
    phase: f32,
}

fn weather(theme: Theme) -> Weather {
    match theme {
        Theme::City => Weather {
            glyphs: &['│', '!', '\'', '│'],
            colors: &[palette::IRON, palette::ASH],
            count: 54,
            fall: -520.0,
            slant: -70.0,
            sway: 0.0,
            depth: NEAR,
        },
        Theme::Volcano => Weather {
            glyphs: &['·', '°', '∙', ','],
            colors: &[palette::ASH, palette::IRON, palette::SCENE_FIRE],
            count: 46,
            fall: -46.0,
            slant: -14.0,
            sway: 22.0,
            depth: MID,
        },
        Theme::Industrial => Weather {
            glyphs: &['·', '.', '∙'],
            colors: &[palette::IRON, palette::COAL, palette::SCENE_TOXIC],
            count: 34,
            fall: -62.0,
            slant: 18.0,
            sway: 14.0,
            depth: MID,
        },
        Theme::Oriental => Weather {
            glyphs: &['*', ',', '°', '·'],
            colors: &[palette::SCENE_RED, palette::SCENE_GOLD, palette::SCENE_HAZE],
            count: 40,
            fall: -54.0,
            slant: -26.0,
            sway: 30.0,
            depth: MID,
        },
    }
}

/// Semeia o clima ja espalhado pela tela inteira.
///
/// Nascer tudo no topo faria a primeira leva descer em bloco, como uma cortina.
fn seed_weather(commands: &mut Commands, sky: &Weather) {
    for i in 0..sky.count {
        let at = Vec2::new(
            (fastrand::f32() - 0.5) * (ARENA_HALF_W * 2.0 + 120.0),
            (fastrand::f32() - 0.5) * (ARENA_HALF_H * 2.0 + 80.0),
        );
        commands.spawn((
            LevelGeometry,
            Parallax {
                home: at,
                depth: sky.depth,
            },
            Drift {
                fall: sky.fall * (0.75 + fastrand::f32() * 0.5),
                slant: sky.slant,
                sway: sky.sway,
                phase: i as f32 * 0.77,
            },
            AsciiSprite::new(AsciiArt::glyph(
                sky.glyphs[i % sky.glyphs.len()],
                sky.colors[i % sky.colors.len()],
            )),
            Layer::Background,
            Transform::from_translation(at.extend(-sky.depth)),
        ));
    }
}

/// Quanto o vento leva uma particula neste quadro.
///
/// Chao comum do clima da arena e da ventania do menu: as duas coisas sao a
/// mesma folha caindo, e a segunda so multiplica o vento. Enquanto cada uma
/// tivesse a sua conta, so a que alguem estivesse olhando ganharia conserto --
/// que e a mesma razao de [`Weather`] ser uma tabela e nao quatro sistemas.
///
/// Com `gust` em 1 a conta e exatamente a da brisa da arena, e e de proposito:
/// o menu e que e o caso extremo, e nao o contrario.
fn gust_step(drift: &Drift, gust: f32, now: f32, dt: f32) -> Vec2 {
    Vec2::new(
        (drift.slant * gust + (now * 0.9 + drift.phase).sin() * drift.sway) * dt,
        // A rajada empurra de lado e segura a queda: folha em vento forte plana.
        // Sem isto ela desce no mesmo ritmo enquanto e varrida, e o campo inteiro
        // le como confete soprado, nao como ar.
        drift.fall / (1.0 + (gust - 1.0) * 0.55) * dt,
    )
}

/// Faz o clima cair, e o devolve ao topo quando ele sai por baixo.
fn blow_weather(time: Res<Time>, mut drops: Query<(&Drift, &mut Parallax)>) {
    let dt = time.delta_secs();
    let now = time.elapsed_secs();
    let edge = ARENA_HALF_W + 80.0;

    for (drift, mut plane) in &mut drops {
        plane.home += gust_step(drift, 1.0, now, dt);

        if plane.home.y < -ARENA_HALF_H - 40.0 {
            plane.home.y = ARENA_HALF_H + 30.0;
            plane.home.x = (fastrand::f32() - 0.5) * (edge * 2.0);
        }
        if plane.home.x < -edge {
            plane.home.x = edge;
        } else if plane.home.x > edge {
            plane.home.x = -edge;
        }
    }
}

// --- ventania da tela inicial -----------------------------------------------

/// Meia moldura que a ventania cobre.
///
/// Nao e a da arena, e por isso e uma constante propria. A camera abre 1300 por
/// 730 no formato de tela comum e o menu desenha do titulo em +250 ate a base do
/// painel de teclas perto de -340; semear pela caixa da arena, que tem 240 de
/// meia altura, deixaria a metade de baixo do menu sem uma folha sequer.
pub const GALE_HALF: Vec2 = Vec2::new(720.0, 440.0);

/// Quantas folhas voam na tela inicial.
///
/// Tres vezes as 40 da brisa do jardim: ali elas pontuam um cenario que ja tem
/// o que olhar, e aqui elas *sao* o cenario. A conta e por area -- a camera do
/// menu abre quase o dobro da caixa em que aquelas 40 vivem --, e no meio do
/// caminho o campo lia como chuvisco: folha suficiente para ver uma passar,
/// nunca para ver o vento.
const GALE_COUNT: usize = 124;

/// Glifos de folha, do broto a petala.
///
/// Os quatro primeiros sao os do clima oriental, que e o campo do jardim. Os
/// outros existem porque folha varrida vira de lado: `~` e a lamina de perfil e
/// `\` e `/` sao a folhagem do bambuzal solta do colmo.
const LEAVES: [char; 8] = ['*', ',', '°', '·', '~', '\\', '/', '\''];

/// Tons da folhagem, do bordo seco a bruma.
const LEAF_TONES: [Color; 4] = [
    palette::SCENE_RED,
    palette::SCENE_GOLD,
    palette::SCENE_CINDER,
    palette::SCENE_HAZE,
];

/// Uma folha da ventania.
#[derive(Component)]
struct Leaf {
    drift: Drift,
    /// Profundidade falsa, de 0 (fundo) a 1 (colada na tela).
    ///
    /// Nao ha camera se mexendo num menu, entao `Parallax` nao tem de onde tirar
    /// profundidade. Sem ela o campo inteiro anda no mesmo tamanho e na mesma
    /// velocidade, e um campo assim le como papel de parede rolando -- que e
    /// exatamente a coisa que uma ventania nao e.
    near: f32,
    /// Giro por segundo no pico da rajada, com sinal: metade das folhas roda
    /// para cada lado.
    spin: f32,
}

/// Forca do vento agora, em multiplos da brisa de base.
///
/// Uma senoide sozinha da vaivem constante, e vento constante nao e ventania --
/// e ventilador. Elevada a potencia alta ela passa quase o ciclo inteiro perto
/// de zero e sobe num pico curto, que e o desenho de uma rajada. Sao duas, em
/// ritmos que nao fecham, para tirar o metronomo: as rajadas se somam e se
/// cancelam sem nunca repetir o intervalo.
fn gale(now: f32) -> f32 {
    let swell = ((now * 0.84).sin() * 0.5 + 0.5).powi(5);
    let surge = ((now * 0.51 + 2.1).sin() * 0.5 + 0.5).powi(7);
    1.0 + 5.0 * swell + 3.5 * surge + 0.4 * (now * 2.3).sin()
}

/// Semeia a ventania da tela inicial, ja espalhada pela moldura inteira.
///
/// Publica porque quem monta o menu e a `ui`: o fundo sabe desenhar folha ao
/// vento, e a tela sabe que quer uma.
pub fn seed_gale(commands: &mut Commands) {
    for i in 0..GALE_COUNT {
        let near = fastrand::f32();
        let at = Vec2::new(
            (fastrand::f32() - 0.5) * GALE_HALF.x * 2.0,
            (fastrand::f32() - 0.5) * GALE_HALF.y * 2.0,
        );
        commands.spawn((
            Leaf {
                drift: Drift {
                    // A de perto cai e deriva mais depressa que a do fundo. E a
                    // unica coisa que separa as duas alem do tamanho, e e o que
                    // faz o campo ter ar entre uma camada e outra.
                    fall: -20.0 - near * 44.0,
                    slant: -30.0 - near * 50.0,
                    sway: 16.0 + near * 26.0,
                    phase: i as f32 * 0.77,
                },
                near,
                spin: (0.35 + fastrand::f32() * 0.75) * if i % 2 == 0 { 1.0 } else { -1.0 },
            },
            AsciiSprite::new(AsciiArt::glyph(
                // O glifo anda pelo indice, para as oito formas aparecerem
                // todas; o tom e sorteado, porque oito formas contra quatro
                // tons casam certinho -- pelo indice, cada folha teria uma cor
                // so, e `~` seria sempre vermelho ate o fim do jogo.
                LEAVES[i % LEAVES.len()],
                // A do fundo desbota. Todas no mesmo tom achatam as camadas de
                // volta num plano so, que e o problema que `near` existe para
                // resolver.
                LEAF_TONES[fastrand::usize(..LEAF_TONES.len())].with_alpha(0.34 + near * 0.66),
            )),
            Layer::Background,
            Transform::from_translation(at.extend(Layer::Background.z()))
                .with_scale(Vec3::splat(0.55 + near * 0.7)),
            DespawnOnExit(GameState::Controls),
        ));
    }
}

/// Sopra a ventania e devolve a folha pela borda oposta.
fn blow_gale(time: Res<Time>, mut leaves: Query<(&Leaf, &mut Transform)>) {
    let dt = time.delta_secs();
    let now = time.elapsed_secs();
    let wind = gale(now);

    for (leaf, mut transform) in &mut leaves {
        // A folha de perto pega mais rajada que a do fundo -- a de tras esta
        // atras de alguma coisa. Se todas pegassem a mesma, a tela inteira
        // acelerava e freava em bloco, como uma cortina sendo puxada.
        let gust = 1.0 + (wind - 1.0) * (0.45 + leaf.near * 0.85);
        let mut at = transform.translation.truncate() + gust_step(&leaf.drift, gust, now, dt);

        if at.y < -GALE_HALF.y {
            at.y = GALE_HALF.y;
            at.x = (fastrand::f32() - 0.5) * GALE_HALF.x * 2.0;
        }
        // Sai pela esquerda, volta pela direita na mesma altura: e o que faz a
        // rajada parecer uma corrente atravessando a tela em vez de folhas
        // sumindo numa borda e nascendo do nada na outra.
        if at.x < -GALE_HALF.x {
            at.x = GALE_HALF.x;
        } else if at.x > GALE_HALF.x {
            at.x = -GALE_HALF.x;
        }

        transform.translation.x = at.x;
        transform.translation.y = at.y;
        transform.rotate_z(leaf.spin * gust * dt);
    }
}

// --- fumaca e erupcao -------------------------------------------------------

