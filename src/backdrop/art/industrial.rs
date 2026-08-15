/// Monta a silhueta de uma serra a partir de alturas de controle.
///
/// Gerada, e nao desenhada a mao: uma cordilheira de cento e tantas colunas
/// escrita caractere a caractere sai com degrau flutuando e encosta furada, e
/// nada no jogo reclama. Aqui a encosta e reta entre dois controles por
/// construcao, e a altura fracionaria vira meio bloco (`▄`) -- resolucao de meia
/// celula de graca, que e o que tira o serrilhado da crista.
fn ridge(controls: &[u16], span: u16, color: Color) -> AsciiArt {
    let rows = controls.iter().copied().max().unwrap_or(1).max(1);
    let last = controls.len() - 1;

    // A altura de cada coluna, com casa decimal: e ela que vira meio bloco.
    let crest: Vec<f32> = (0..span)
        .map(|col| {
            let t = col as f32 * last as f32 / (span.max(2) - 1) as f32;
            let low = (t.floor() as usize).min(last);
            let high = (low + 1).min(last);
            controls[low] as f32 + (controls[high] as f32 - controls[low] as f32) * t.fract()
        })
        .collect();

    let text = (0..rows)
        .map(|row| {
            // Quantas linhas de encosta esta altura precisa ter para chegar aqui.
            let needed = (rows - row) as f32;
            crest
                .iter()
                .map(|height| match *height {
                    h if h >= needed => '█',
                    h if h >= needed - 0.55 => '▄',
                    _ => ' ',
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    AsciiArt::solid(&text, color)
}

/// Teto de fumaca esfarrapado, pendurado no alto do ceu.
///
/// E onde a coluna do vulcao vai dar. Sem ele a fumaca sobe e some no preto, e
/// a erupcao perde o teto que a faz parecer uma erupcao e nao uma fogueira.
///
/// A densidade rareia para baixo e serpenteia com a coluna, entao a borda de
/// baixo nunca sai reta -- que e o que denuncia faixa desenhada com `fill`.
fn smog(span: u16, rows: u16, color: Color) -> AsciiArt {
    let text = (0..rows)
        .map(|row| {
            (0..span)
                .map(|col| {
                    let wave = ((col as f32 * 0.21).sin() + (col as f32 * 0.07).cos()) * 0.45;
                    match 1.15 - row as f32 / rows as f32 + wave {
                        d if d > 1.05 => '▓',
                        d if d > 0.75 => '▒',
                        d if d > 0.45 => '░',
                        _ => ' ',
                    }
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    AsciiArt::solid(&text, color)
}

// --- o vulcao ---------------------------------------------------------------

/// Altura do cone, em linhas.
const CONE_ROWS: u16 = 13;
/// Largura do topo do cone, em colunas.
const CONE_TOP: u16 = 14;
/// Quanto ele alarga por linha, em colunas por lado.
const CONE_FLARE: u16 = 2;
/// Largura da boca da cratera.
const CRATER_COLS: u16 = 6;
/// Quantas linhas do topo ficam abertas.
const CRATER_ROWS: u16 = 2;

/// Onde o vulcao se apoia. Fora do eixo de proposito: montanha centrada na tela
/// vira logotipo, e o mapa e assimetrico.
const VOLCANO_FOOT: Vec2 = Vec2::new(120.0, GROUND);

/// A boca da cratera, em coordenadas de mundo.
///
/// Sai da propria arte -- base mais altura, menos a linha aberta -- e nao de um
/// numero escrito a mao. Fumaca, brilho e bomba de lava saem daqui: mexer na
/// altura do cone leva os tres junto.
const CRATER: Vec2 = Vec2::new(
    VOLCANO_FOOT.x,
    VOLCANO_FOOT.y + (CONE_ROWS as f32 - CRATER_ROWS as f32 * 0.5) * CELL.y,
);

/// Os glifos por onde a lava desce. Sao eles que o [`Accent`] acende: a
/// montanha inteira e uma arte so, e a cor separa rocha de brasa.
const LAVA: &str = "≈~";

/// Onde os dois veios passam nesta linha, medidos do meio para fora.
///
/// Fracao da meia-largura, e nao coluna fixa: o veio desce pela encosta, que
/// abre a cada linha, entao ele tem que abrir junto. Reto para baixo, ele leria
/// como cano descendo por dentro da montanha.
const VEINS: [f32; 2] = [-0.42, 0.30];

/// Um caractere da encosta: rocha, lava ou vazio.
///
/// Os tres saem da mesma funcao porque so assim a lava cabe exatamente onde a
/// rocha nao esta. Enquanto foram duas artes carimbadas uma sobre a outra, cada
/// celula de brasa desenhava em cima de uma pedra -- duas celulas no mesmo
/// lugar, no mesmo Z, e quem aparecia era sorteio da ordem de spawn.
fn face(col: u16, width: u16, row: u16) -> char {
    // Bordo duro dos dois lados: e ele que recorta a silhueta contra o ceu.
    if col == 0 || col + 1 == width {
        return '█';
    }
    if col == 1 || col + 2 == width {
        return '▓';
    }
    // A boca fica aberta: o brilho da lava entra por ela, por baixo.
    let mouth = (width - CRATER_COLS) / 2;
    if row < CRATER_ROWS && col >= mouth && col < mouth + CRATER_COLS {
        return ' ';
    }
    let middle = (width as f32 - 1.0) * 0.5;
    let meander = (row as f32 * 1.3).sin() * 1.6;
    for lean in VEINS {
        let channel = (middle * (1.0 + lean) + meander).round();
        if col as f32 == channel {
            return if row.is_multiple_of(2) { '≈' } else { '~' };
        }
    }
    // Motivo curto que anda com a linha, entao a textura nunca se repete na
    // mesma coluna e a encosta nao vira papel de parede listrado.
    const ROCK: [char; 6] = ['▒', '▒', '░', '▓', '▒', '▓'];
    ROCK[(col as usize * 3 + row as usize * 5) % ROCK.len()]
}

/// A montanha inteira, linha a linha.
fn cone(stone: Color, lava: Color) -> AsciiArt {
    let base = CONE_TOP + (CONE_ROWS - 1) * CONE_FLARE * 2;
    let mut text = String::new();
    for row in 0..CONE_ROWS {
        let width = CONE_TOP + row * CONE_FLARE * 2;
        text.push_str(&" ".repeat(((base - width) / 2) as usize));
        text.extend((0..width).map(|col| face(col, width, row)));
        text.push('\n');
    }
    AsciiArt::build(
        &text,
        &Accent {
            base: stone,
            accent: lava,
            on: LAVA,
        },
    )
}

/// A lava vista pela boca da cratera.
const CRATER_GLOW: &str = "░▒▓▓▒░
▒▓██▓▒";

/// O clarao do instante em que ela estoura.
const BLAST_FLASH: &str = " ▄▄▄▄
▄████▄
██████
▀████▀";

// --- o resto do repertorio --------------------------------------------------

/// Lua cheia com mar de sombra.
const MOON: &str = "  ▄▄▄▄▄▄
 ████████
██▓▓▒████
██▓░▒▓███
 ███▓████
  ▀▀▀▀▀▀";

/// Antena de radio da cidade: a coisa mais alta do horizonte.
const MAST: &str = "  ▄
 ▄█▄
  █
 ▄█▄
▄▀█▀▄
  █
 ▄█▄
▄███▄";

/// Caixa d'agua no telhado.
const TANK: &str = "▄████▄
██████
▀█▀▀█▀
 █  █";

/// Um vao da trelica de aco. O portico e este desenho repetido.
const BAY: [&str; 6] = [
    "o===============",
    "|\\             /",
    "| \\           / ",
    "|  o=========o  ",
    "| /           \\ ",
    "o===============",
];

/// O portico que atravessa a fabrica, vao a vao.
///
/// Gerado porque trelica e repeticao pura: doze vaos escritos a mao sao seis
/// linhas de duzentos caracteres cada, e basta um traco a menos em uma delas
/// para a estrutura nascer torta.
fn gantry(bays: usize, color: Color) -> AsciiArt {
    let text = BAY
        .iter()
        .map(|row| {
            // A coluna que fecha o ultimo vao e a primeira do proprio desenho:
            // montante vira montante, junta vira junta.
            let mut line = row.repeat(bays);
            line.push(row.chars().next().unwrap_or(' '));
            line
        })
        .collect::<Vec<_>>()
        .join("\n");
    AsciiArt::solid(&text, color)
}

/// Cano de escoamento correndo na linha do chao, com as saidas em pe.
fn drains(span: u16, color: Color) -> AsciiArt {
    let vent = |col: u16| col % 17 == 5;
    let cano: String = (0..span)
        .map(|col| if vent(col) { '╦' } else { '═' })
        .collect();
    let saida: String = (0..span)
        .map(|col| if vent(col) { '╨' } else { ' ' })
        .collect();
    AsciiArt::solid(&format!("{cano}\n{saida}"), color)
}

// --- geradores de cenario ---------------------------------------------------
//
// Tudo aqui e desenhado por conta, e nao escrito a mao, pelo mesmo motivo que a
// serra e o cone ja eram: as formas grandes do fundo sao repeticao com variacao
// -- coluna de basalto, andar de pagode, arco de galeria, escama de dragao. Uma
// parede de vinte linhas por quarenta colunas escrita caractere a caractere sai
// com uma junta a menos no meio, e o que denuncia nao e a junta: e a linha da
// crista, que deixa de ser uma curva so.

/// Rampa de densidade padrao, do vazio ao macico.
///
/// A ordem importa: os geradores escolhem por indice, entao trocar a rampa
/// muda o material inteiro de uma vez.
const RAMP: [char; 4] = ['░', '▒', '▓', '█'];

/// Escolhe o degrau da rampa para uma densidade em `0..1`.
fn shade(density: f32) -> char {
    match density {
        d if d >= 0.80 => RAMP[3],
        d if d >= 0.58 => RAMP[2],
        d if d >= 0.36 => RAMP[1],
        d if d >= 0.16 => RAMP[0],
        _ => ' ',
    }
}

/// Monta uma arte celula a celula a partir de uma funcao `(col, row) -> char`.
///
/// Todo gerador daqui repetia as mesmas oito linhas de `map`/`collect`/`join`,
/// e era nelas -- nao no desenho -- que nascia o erro de uma coluna a mais.
fn draw(
    cols: u16,
    rows: u16,
    map: &'static [(char, Color)],
    fallback: Color,
    mut cell: impl FnMut(u16, u16) -> char,
) -> AsciiArt {
    let text = (0..rows)
        .map(|row| (0..cols).map(|col| cell(col, row)).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    AsciiArt::tinted(&text, map, fallback)
}

/// Uma cortina de liquido despencando, num quadro do ciclo.
///
/// Os veios saem por coluna e o padrao anda para baixo com o quadro: e o que
/// faz a queda escorrer em vez de piscar. O pe se desmancha em respingo, porque
/// queda com a base reta le como fita colada na parede.
fn cascade(cols: u16, rows: u16, frame: u16, map: &'static [(char, Color)]) -> AsciiArt {
    draw(cols, rows, map, palette::COAL, |col, row| {
        let vein = (col as f32 * 1.9).sin() * 0.32 + 0.62;
        let flow = ((row + frame) as f32 * 0.85 + col as f32 * 2.4).sin() * 0.28;
        let spray = 1.0 - (row as f32 / rows.max(2) as f32).powi(3) * 0.85;
        shade((vein + flow) * spray)
    })
}

/// Parede de basalto: colunas verticais com junta, fratura e topo quebrado.
fn basalt(cols: u16, rows: u16, map: &'static [(char, Color)]) -> AsciiArt {
    draw(cols, rows, map, palette::COAL, |col, row| {
        // A parede nao termina numa reta: a crista sobe e desce com a coluna.
        let crest = (col as f32 * 0.53).sin() * 1.3 + (col as f32 * 0.17).cos() * 1.7 + 2.6;
        let depth = row as f32 - crest;
        match depth {
            d if d < -0.5 => ' ',
            d if d < 0.5 => '▄',
            _ if col.is_multiple_of(5) => '▓',
            _ if (row * 7 + col * 3) % 23 == 0 => '▒',
            _ => '█',
        }
    })
}

/// Ponte pensil arruinada: torres, cabo que peca e tabuado partido no meio.
///
/// O vao no meio nao e enfeite -- e o que diz que essa travessia ja caiu, e
/// que a que os jogadores estao pisando e a ultima que sobrou.
fn suspension(cols: u16, gap: u16, map: &'static [(char, Color)]) -> AsciiArt {
    const ROWS: u16 = 9;
    let deck = ROWS - 1;
    let (left, right) = (3u16, cols.saturating_sub(4));
    let void = |col: u16| (col as i32 - cols as i32 / 2).unsigned_abs() as u16 * 2 < gap;

    draw(cols, ROWS, map, palette::IRON, |col, row| {
        if col == left || col == right {
            return if row == 0 { '╥' } else { '║' };
        }
        if void(col) {
            return ' ';
        }
        if row == deck {
            return if col < left || col > right {
                ' '
            } else {
                '═'
            };
        }
        if col < left || col > right {
            return ' ';
        }
        // Cabo: parabola presa no topo das duas torres, pecando no meio do vao.
        let u = (col - left) as f32 / (right - left).max(1) as f32;
        let sag = 1.0 + (deck - 3) as f32 * 4.0 * u * (1.0 - u);
        if row == sag.round() as u16 {
            return match u {
                u if u < 0.45 => '\\',
                u if u > 0.55 => '/',
                _ => '_',
            };
        }
        // Pendural: so entre o cabo e o tabuado, senao ele fura a ponte.
        if (col - left) % 4 == 2 && row as f32 > sag && row < deck {
            return '│';
        }
        ' '
    })
}

/// Tanque de pressao: casco abaulado, cintas e visor de nivel.
fn vat(cols: u16, rows: u16, map: &'static [(char, Color)]) -> AsciiArt {
    let glass = cols / 2;
    draw(cols, rows, map, palette::IRON, |col, row| {
        // O casco fecha em abobada: duas linhas de recuo no topo.
        let inset = match row {
            0 => 2,
            1 => 1,
            _ => 0,
        };
        if col < inset || col + inset >= cols {
            return ' ';
        }
        if row == 0 {
            return '▄';
        }
        if col == glass && row > 1 {
            // Visor: a coluna de liquido some antes do topo, entao o tanque
            // le como meio cheio em vez de solido.
            return if row * 3 > rows * 2 { '▒' } else { '░' };
        }
        if row % 4 == 3 {
            return '═';
        }
        if col == inset || col + inset + 1 == cols {
            return '▓';
        }
        '█'
    })
}

/// Corrida de canos com valvulas, manometros e pernas de descida.
fn pipeline(span: u16, map: &'static [(char, Color)]) -> AsciiArt {
    draw(span, 3, map, palette::IRON, |col, row| {
        let (valve, leg) = (col % 13 == 4, col % 13 == 10);
        match row {
            0 if valve => '○',
            1 if valve => '╪',
            1 if leg => '╤',
            1 => '═',
            2 if leg => '║',
            _ => ' ',
        }
    })
}

/// Galeria de arcos: o subsolo lido de uma vez so.
///
/// O arco e meia-circunferencia de verdade, e nao dois riscos e um traco: e a
/// curva que separa galeria de porta quadrada.
fn arcade(bays: u16, bay: u16, rows: u16, map: &'static [(char, Color)]) -> AsciiArt {
    let peak = (rows as f32 - 1.5).max(1.0);
    draw(bays * bay, rows, map, palette::IRON, |col, row| {
        let u = (col % bay) as f32 / (bay - 1).max(1) as f32 * 2.0 - 1.0;
        // Fora do vao e pilar macico; dentro, a altura do arco vem do circulo.
        let open = if u.abs() < 0.68 {
            peak * (1.0 - (u / 0.68).powi(2)).max(0.0).sqrt()
        } else {
            0.0
        };
        let mouth = rows as f32 - 1.0 - open;
        match row as f32 {
            r if r > mouth => ' ',
            r if r > mouth - 1.0 => '▓',
            0.0 => '▄',
            _ if (row * 5 + col * 3) % 19 == 0 => '▒',
            _ => '█',
        }
    })
}

/// Pagode de `tiers` andares: telhado que abre, corpo que estreita.
///
/// Cada andar sao tres linhas -- ponta de beiral, telha e corpo -- e a de cima
/// e sempre a mais estreita. Escrito a mao, o quarto telhado sai uma coluna
/// menor que o terceiro e o predio inteiro entorta sem ninguem saber por que.
fn pagoda(tiers: u16, map: &'static [(char, Color)]) -> AsciiArt {
    const TOP_ROOF: u16 = 11;
    const ROOF_STEP: u16 = 5;
    const BODY_INSET: u16 = 3;

    let widest = TOP_ROOF + (tiers - 1) * ROOF_STEP;
    let mut text = String::new();
    let pad = |n: u16| " ".repeat(n as usize);

    // Pinaculo: a joia e a haste que fecham o predio por cima.
    text.push_str(&format!("{}○\n{}║\n", pad(widest / 2), pad(widest / 2)));

    for tier in 0..tiers {
        let roof = TOP_ROOF + tier * ROOF_STEP;
        let body = roof - BODY_INSET * 2;
        let side = (widest - roof) / 2;

        // Beiral virado na propria linha da telha: `▀` desenha na metade de
        // cima da celula e `▄` na de baixo, entao a ponta sobe meia celula sem
        // gastar linha nenhuma. Numa linha propria acima, a ponta descolava do
        // telhado e virava dois riscos soltos no ceu.
        text.push_str(&format!(
            "{}▀{}▀\n",
            pad(side),
            "▄".repeat(roof.saturating_sub(2) as usize)
        ));
        // Os andares de baixo sao mais altos: pagode com todos os corpos da
        // mesma altura le como bolo de casamento, nao como torre.
        for _ in 0..1 + tier / 2 {
            text.push_str(&format!(
                "{}{}\n",
                pad(side + BODY_INSET),
                (0..body)
                    .map(|col| match col as i32 - body as i32 / 2 {
                        0 => '╬',
                        -1 | 1 => '│',
                        _ => '█',
                    })
                    .collect::<String>()
            ));
        }
    }
    AsciiArt::tinted(text.trim_end(), map, palette::IRON)
}

/// Portao de dois pilares e duas travessas, com a de cima virada nas pontas.
fn gate(cols: u16, rows: u16, map: &'static [(char, Color)]) -> AsciiArt {
    let pillar = |col: u16| {
        let inner = cols / 5;
        (inner..inner + 3).contains(&col) || (cols - inner - 3..cols - inner).contains(&col)
    };
    draw(cols, rows, map, palette::SCENE_RED, |col, row| match row {
        // Kasagi: a viga de cima, com as duas pontas levantadas.
        0 => {
            if col < 3 || col + 3 >= cols {
                '▀'
            } else {
                '▄'
            }
        }
        1 => '█',
        // Shimaki: a segunda viga, recuada, que da espessura ao topo.
        2 => {
            if col < 4 || col + 4 >= cols {
                ' '
            } else {
                '▄'
            }
        }
        // Nuki: a travessa que amarra os pilares.
        4 => {
            let inner = cols / 5;
            if (inner..cols - inner).contains(&col) {
                '▄'
            } else {
                ' '
            }
        }
        // Gakuzuka: o pilarete curto entre as duas travessas.
        3 if col == cols / 2 => '║',
        _ if pillar(col) => '█',
        _ => ' ',
    })
}

/// Pilar de jade com dragao em relevo: a espiral sobe uma coluna por linha.
fn jade_pillar(rows: u16, map: &'static [(char, Color)]) -> AsciiArt {
    const COLS: u16 = 5;
    draw(COLS, rows, map, palette::SCENE_JADE, |col, row| {
        if row == 0 || row + 1 == rows {
            return '▄';
        }
        if col == 0 || col + 1 == COLS {
            return '▓';
        }
        // O relevo anda de lado a cada linha: e a espiral que sobe o fuste.
        if col == 1 + (row % 3) {
            return '▒';
        }
        '█'
    })
}

/// Disco ou anel desenhado no grid retangular da fonte.
///
/// A celula e 8x16: um circulo escrito a mao com o mesmo numero de colunas e
/// linhas sai um ovo deitado, e nao ha correcao possivel depois -- ou se conta
/// a proporcao ou se desenha errado. Aqui o raio vertical ja entra corrigido,
/// entao a lua e redonda na tela, nao no arquivo.
fn disc(radius: u16, hollow: bool, map: &'static [(char, Color)]) -> AsciiArt {
    let squash = CELL.y / CELL.x;
    let ry = (radius as f32 / squash).round().max(1.0);
    let (cols, rows) = (radius * 2 + 1, ry as u16 * 2 + 1);

    draw(cols, rows, map, palette::ASH, |col, row| {
        let dx = col as f32 - radius as f32;
        let dy = (row as f32 - ry) * squash;
        let d = dx.hypot(dy) / radius as f32;
        match (hollow, d) {
            (_, d) if d > 1.04 => ' ',
            // Anel: so a casca, e ela engrossa para dentro.
            (true, d) if d < 0.70 => ' ',
            (true, d) => shade(1.0 - (d - 0.87).abs() * 3.4),
            // Disco: denso no meio, esfumado na borda -- volume, nao recorte.
            // A rampa tem que cruzar os quatro degraus dentro do raio, senao o
            // sol sai chapado em dois tons e vira um circulo pintado.
            (false, d) => shade(1.62 - d * 1.32),
        }
    })
}

/// Faixa de liquido correndo na horizontal, num quadro do ciclo.
///
/// Duas frequencias incomensuraveis, e nao uma: com uma so, o rio atravessa a
/// tela repetindo o mesmo desenho a cada quinze colunas, e listra regular numa
/// faixa de mil e seiscentas unidades e a primeira coisa que o olho pega.
fn current(span: u16, rows: u16, frame: u16, map: &'static [(char, Color)]) -> AsciiArt {
    draw(span, rows, map, palette::COAL, |col, row| {
        let t = (col + frame * 2) as f32;
        let wave = (t * 0.42 + row as f32 * 2.1).sin() * 0.68 + (t * 0.17).sin() * 0.32;
        match wave {
            w if w > 0.62 => '≈',
            w if w > 0.05 => '~',
            w if w > -0.55 => '-',
            _ => '_',
        }
    })
}

/// Alto-forno: massa que abre para baixo, com a boca em arco e o nucleo aceso.
fn furnace(cols: u16, rows: u16, map: &'static [(char, Color)]) -> AsciiArt {
    let mouth_w = cols as f32 * 0.26;
    let mouth_h = (rows as f32 - 2.0) * 0.55;

    draw(cols, rows, map, palette::IRON, |col, row| {
        // O corpo estreita para cima: forno com a parede reta le como caixa.
        let inset = ((rows - 1 - row) as f32 * 0.5).round() as u16;
        if col < inset || col + inset >= cols {
            return ' ';
        }
        // A boca e o mesmo arco da galeria, so que cheia de fogo.
        let u = (col as f32 - (cols as f32 - 1.0) * 0.5) / mouth_w;
        let arch = if u.abs() < 1.0 {
            mouth_h * (1.0 - u * u).sqrt()
        } else {
            0.0
        };
        let lip = rows as f32 - 1.0 - arch;
        match row as f32 {
            r if r > lip => {
                if row.is_multiple_of(2) {
                    '≈'
                } else {
                    '~'
                }
            }
            r if r > lip - 1.0 => '▓',
            0.0 => '▄',
            _ if (row * 5 + col * 7) % 17 == 0 => '▒',
            _ => '█',
        }
    })
}

/// Onde os tres tanques da fabrica se apoiam.
const VAT_FOOT: f32 = GROUND + 30.0;
/// Largura de um tanque de processo, em celulas.
const VAT_COLS: u16 = 13;

/// Eixo e altura de cada tanque -- e, por tabela, de cada chamine.
///
/// Alturas diferentes de proposito: tres tanques do mesmo tamanho, igualmente
/// espacados, leem como icone repetido tres vezes, nao como patio.
///
/// As bocas de fumaca sao entidades a parte da arte. Enquanto os dois foram
/// numeros escritos em lugares diferentes, o vapor subia de tres pontos do ceu
/// ao lado dos canos e a arte continuava intacta: nada para um teste pegar, e
/// obvio na tela.
const STACKS: [(f32, u16); 3] = [(-190.0, 12), (0.0, 10), (190.0, 14)];

/// Altura da boca de uma chamine, tirada da propria altura do tanque.
fn stack_top(rows: u16) -> f32 {
    VAT_FOOT + rows as f32 * CELL.y
}

/// Placa de risco da fabrica: o unico texto do cenario, e por isso ele conta.
const PLACARD: &str = "╔════════════╗
║  TOX-03 !! ║
║ CORROSIVE  ║
╚════════════╝";

// --- materiais --------------------------------------------------------------
//
// Cada tabela e um material: a mesma rampa de densidade lida como basalto,
// como jade ou como acido dependendo so de qual delas pinta. E o que deixa um
// gerador servir tres temas sem virar tres geradores.

/// Rocha fria: basalto, penhasco, muro de pedra.
const STONE: [(char, Color); 5] = [
    ('▄', palette::ASH),
    ('█', palette::IRON),
    ('▓', palette::COAL),
    ('▒', palette::COAL),
    ('░', palette::COAL),
];
/// Aco de fabrica, com a ferrugem nas juntas.
const STEEL: [(char, Color); 10] = [
    ('▄', palette::ASH),
    ('█', palette::IRON),
    ('▓', palette::COAL),
    ('▒', palette::SCENE_RUST),
    ('░', palette::COAL),
    ('═', palette::SCENE_RUST),
    ('║', palette::IRON),
    ('╪', palette::ASH),
    ('╤', palette::ASH),
    ('○', palette::SCENE_TOXIC),
];
/// Queda de magma, do nucleo claro a borda apagada.
const MAGMA_FALL: [(char, Color); 4] = [
    ('█', palette::SCENE_GOLD),
    ('▓', palette::SCENE_FIRE),
    ('▒', palette::SCENE_RED),
    ('░', palette::SCENE_CINDER),
];
/// Rio de lava correndo na linha do horizonte.
const MAGMA_FLOW: [(char, Color); 4] = [
    ('≈', palette::SCENE_GOLD),
    ('~', palette::SCENE_FIRE),
    ('-', palette::SCENE_RED),
    ('_', palette::SCENE_CINDER),
];
/// Rocha do forno: massa fria com a boca acesa.
const FURNACE: [(char, Color); 6] = [
    ('▄', palette::ASH),
    ('█', palette::IRON),
    ('▓', palette::SCENE_CINDER),
    ('▒', palette::COAL),
    ('≈', palette::SCENE_GOLD),
    ('~', palette::SCENE_FIRE),
];
/// Queda de acido.
const ACID_FALL: [(char, Color); 4] = [
    ('█', palette::SCENE_ACID),
    ('▓', palette::SCENE_TOXIC),
    ('▒', palette::SCENE_JADE),
    ('░', palette::COAL),
];
/// Canal de esgoto correndo no fundo da galeria.
const ACID_FLOW: [(char, Color); 4] = [
    ('≈', palette::SCENE_ACID),
    ('~', palette::SCENE_TOXIC),
    ('-', palette::SCENE_JADE),
    ('_', palette::COAL),
];
/// Queda de refrigerante do reator: fria onde a de magma e quente.
const COOLANT_FALL: [(char, Color); 4] = [
    ('█', palette::ASH),
    ('▓', palette::SCENE_BLUE),
    ('▒', palette::IRON),
    ('░', palette::COAL),
];
/// Casco de tanque, com o visor de nivel esverdeado.
const TANK_SKIN: [(char, Color); 6] = [
    ('▄', palette::ASH),
    ('█', palette::IRON),
    ('▓', palette::COAL),
    ('═', palette::SCENE_RUST),
    ('▒', palette::SCENE_ACID),
    ('░', palette::SCENE_TOXIC),
];
/// Jade: massa profunda com a borda acesa, que e como pedra translucida le.
///
/// A rampa e invertida de proposito. Em rocha opaca o miolo e o que pega luz;
/// no jade e a casca fina que acende, e e essa inversao -- e so ela -- que
/// separa uma escultura de jade de uma escultura de granito pintada de verde.
const JADE_SKIN: [(char, Color); 11] = [
    ('█', palette::SCENE_JADE),
    ('▓', palette::SCENE_JADE),
    ('▒', palette::SCENE_JADE_LIT),
    ('░', palette::SCENE_JADE_LIT),
    ('▄', palette::SCENE_JADE_LIT),
    ('▀', palette::SCENE_JADE_LIT),
    ('▲', palette::SCENE_GOLD),
    ('•', palette::SCENE_GOLD),
    ('~', palette::SCENE_JADE_LIT),
    ('\\', palette::SCENE_GOLD),
    ('/', palette::SCENE_GOLD),
];
/// Telhado de telha e madeira lacada do pagode.
const TIMBER: [(char, Color); 7] = [
    ('▄', palette::SCENE_RED),
    ('▀', palette::SCENE_RED),
    ('█', palette::SCENE_GOLD),
    ('│', palette::SCENE_FIRE),
    ('╬', palette::SCENE_FIRE),
    ('○', palette::SCENE_GOLD),
    ('║', palette::SCENE_GOLD),
];
/// Laca vermelha do portao, com as ferragens douradas.
const LACQUER: [(char, Color); 4] = [
    ('█', palette::SCENE_RED),
    ('▄', palette::SCENE_RED),
    ('▀', palette::SCENE_FIRE),
    ('║', palette::SCENE_GOLD),
];
/// Sol baixo do poente.
const SUNSET: [(char, Color); 4] = [
    ('█', palette::SCENE_RED),
    ('▓', palette::SCENE_RED),
    ('▒', palette::SCENE_FIRE),
    ('░', palette::SCENE_FIRE),
];
/// Lua: pedra fria, com o mar de sombra nos degraus de baixo.
const LUNAR: [(char, Color); 4] = [
    ('█', palette::ASH),
    ('▓', palette::IRON),
    ('▒', palette::IRON),
    ('░', palette::COAL),
];
/// Anel de contencao do reator.
const CONTAINMENT: [(char, Color); 4] = [
    ('█', palette::IRON),
    ('▓', palette::SCENE_BLUE),
    ('▒', palette::SCENE_BLUE),
    ('░', palette::COAL),
];
/// Nucleo do reator, do repouso ao pico de carga.
const CORE_HEAT: [(char, Color); 4] = [
    ('█', palette::SCENE_BLUE),
    ('▓', palette::SCENE_TOXIC),
    ('▒', palette::SCENE_ACID),
    ('░', palette::SCENE_BLUE),
];

/// Galho de cerejeira entrando pelo canto de cima.
const BRANCH: &str = "▄▄▄▄▄▄▄▄▄▄
 ░ ▀▀▄▄ ░ ▀▀▄▄▄
░ ░  ░ ▀▄  ░  ▀▀▄
      ░  ░ ░";

/// Lanterna de papel, com o miolo vazado: a brasa que mora ali dentro e uma
/// entidade a parte, que balanca sozinha.
const LANTERN: &str = "┌─┐\n│ │\n└─┘";

