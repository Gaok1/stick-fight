/// Escadaria de pedra: degraus que abrem para baixo.
fn terrace(steps: u16, top: u16, spread: u16, map: &'static [(char, Color)]) -> AsciiArt {
    let widest = top + (steps - 1) * spread * 2;
    draw(widest, steps, map, palette::IRON, |col, row| {
        let half = (top + row * spread * 2) / 2;
        let middle = (widest - 1) / 2;
        if col.abs_diff(middle) > half {
            ' '
        } else if row == 0 || col.abs_diff(middle) > (top + (row - 1) * spread * 2) / 2 {
            '▄'
        } else {
            '█'
        }
    })
}

/// Bambuzal: colmos com no, alturas diferentes e folhagem nos nos.
fn bamboo(stalks: u16, rows: u16, map: &'static [(char, Color)]) -> AsciiArt {
    let head = |stalk: u16| (stalk * 5 % 4) + 1;
    // Folha so nos nos, e so em um no a cada dois: em todos, o bambuzal fecha
    // numa cerca viva e os colmos somem dentro dela.
    let leafy = |stalk: u16, row: u16| {
        row >= head(stalk) && (row - head(stalk)).is_multiple_of(4) && (row / 4).is_multiple_of(2)
    };
    draw(stalks * 4, rows, map, palette::SCENE_JADE, |col, row| {
        let stalk = col / 4;
        match col % 4 {
            // O colmo: no a cada quatro linhas, que e o que separa bambu de
            // vareta.
            1 if row >= head(stalk) => {
                if (row - head(stalk)).is_multiple_of(4) {
                    '╪'
                } else {
                    '║'
                }
            }
            0 if leafy(stalk, row) => '\\',
            2 if leafy(stalk, row) => '/',
            _ => ' ',
        }
    })
}

/// Teto de galeria: laje macica com dente pendurado.
///
/// A borda de baixo nunca sai reta -- e ela, e nao a laje, que faz o teto ler
/// como pedra escavada em vez de faixa preta colada no topo da tela.
fn vault(span: u16, rows: u16, map: &'static [(char, Color)]) -> AsciiArt {
    draw(span, rows, map, palette::COAL, |col, row| {
        let hang = (col as f32 * 0.31).sin() * 1.5 + (col as f32 * 0.11).cos() * 1.7 + 2.2;
        match row as f32 {
            r if r < 1.0 => '█',
            r if r < hang => {
                if col.is_multiple_of(2) {
                    '▓'
                } else {
                    '█'
                }
            }
            r if r < hang + 0.65 => '▀',
            _ => ' ',
        }
    })
}

/// A cabeca do dragao de jade, olhando para a direita.
///
/// A unica peca grande deste arquivo desenhada a mao, e por um motivo: cabeca
/// e gesto, nao repeticao. O que a faz ler como dragao -- e nao como cobra com
/// chifre -- sao tres coisas que nenhuma senoide entrega: os chifres partindo
/// do craneo para tras, a boca aberta com as presas penduradas no vazio entre
/// as duas mandibulas, e o focinho comprido que os bigodes vao pendurar.
///
/// Ela e desenhada olhando para a direita, e nao espelhada como antes, porque
/// agora ela gira: `facing` manda direto na `Transform`, e uma arte que ja
/// nasce virada obrigaria toda conta de angulo a carregar meia volta de
/// correcao. O bigode nao esta aqui -- ele e fio com fisica, nao celula.
const DRAGON_HEAD: &str = "   ▲   ▲
  ▄█▄▄▄█▄▄
 ░▒▓████████▄▄
░▒▓█████•███████▄▄
▒▓███████████████████▄
▒▓██████████▀▀▀▀▀▀▀▀▀▀
░▒▓████████▄  ▼   ▼
 ░▒▓███████▄▄▄▄▄▄▄▄▄
  ░▒▓████████▀▀▀▀▀▀
   ░▒▓▒░              ";

/// Uma vertebra do corpo. Ela nasce apontando para a direita, como a cabeca:
/// quem gira e a `Transform`, seguindo a tangente do rastro.
///
/// Tres linhas e o minimo que separa um dragao de um cordao: crista dourada em
/// cima, massa no meio, barriga acesa embaixo. A silhueta do bicho e a soma de
/// vinte e tantas delas, entao o que se desenha aqui aparece vinte e tantas
/// vezes -- e um detalhe a mais vira ruido, nao riqueza.
const DRAGON_SCALE: &str = "▲▲▲
▓█▓
 ▒ ";

/// A vertebra que tem perna. Duas delas ao longo do corpo bastam: dragao
/// chines tem garra, mas ela e pontuacao, nao padrao.
const DRAGON_LIMB: &str = "▲▲▲
▓█▓
/ \\";

/// A ponta do rabo: a nadadeira que fecha o corpo.
///
/// Sem ela o bicho acaba num toco, e um corpo que afina ate sumir le como erro
/// de desenho -- parece que faltou carregar o resto.
const DRAGON_TAIL: &str = "▲▲
▒░
▼▼";

/// Malho da forja: a haste guia em cima, a massa embaixo.
const HAMMER: &str = "    ████
    ████
▄██████████▄
████████████
▀██████████▀";

/// Bigorna que recebe a pancada.
const ANVIL: &str = " ▄▄▄▄▄▄▄▄▄▄▄▄▄
▐█████████████▌
 ▀▀▀▀██████▀▀▀▀
     ██████
  ▄▄██████████▄▄";

/// Lanterna de pedra do jardim: chapeu, caixa de luz, fuste e base.
const STONE_LANTERN: &str = "   ▄▄▄▄▄
  ▐█████▌
  ▄▀▀▀▀▀▄
 ▐░░███░░▌
  ▀▄▄▄▄▄▀
    ███
   ▄███▄
  ███████";

/// Grou em voo: corpo e duas asas abertas, o bastante a essa distancia.
const CRANE: &str = " ▄ \n▀ ▀";

/// Agua do jardim: jade correndo, nao lodo.
const JADE_FLOW: [(char, Color); 4] = [
    ('≈', palette::SCENE_JADE_LIT),
    ('~', palette::SCENE_JADE),
    ('-', palette::SCENE_JADE),
    ('_', palette::COAL),
];

// --- onde as pecas grandes moram --------------------------------------------
//
// Landmark que anda -- malho, sopro, nucleo -- precisa do mesmo ponto em dois
// lugares: a composicao parada, que desenha o que fica em volta dele, e o
// numero, que o move. Enquanto os dois foram numeros digitados em funcoes
// diferentes, mexer na bigorna deixava o malho batendo no ar ao lado dela.

/// Onde o malho da forja descansa, no alto dos trilhos.
const HAMMER_AT: Vec2 = Vec2::new(250.0, GROUND + 200.0);
/// Quanto ele desce ate encostar na bigorna.
const HAMMER_DROP: f32 = 80.0;

/// Centro do anel de contencao do reator.
const CORE_AT: Vec2 = Vec2::new(0.0, 30.0);

/// O meio do oito que o dragao desenha no ceu do jardim.
///
/// Alto o bastante para passar por cima do tabuado mais alto da arena e baixo
/// o bastante para nao cruzar os letreiros, que moram em y = 164 e y = 188.
const DRAGON_SKY: Vec2 = Vec2::new(-30.0, 112.0);
/// Meias-medidas do oito: largo e raso.
///
/// Largo porque um dragao chines e uma fita, e fita curta le como minhoca;
/// raso porque a altura util entre o tabuado e o letreiro e pouca. O `y` sobe
/// e desce duas vezes por volta -- e a lemniscata, e nao uma senoide, que faz
/// o bicho passar por cima do proprio corpo.
const DRAGON_LOOP: Vec2 = Vec2::new(330.0, 33.0);
/// Altura da rasante: entre o tabuado do meio e as plataformas das alas.
///
/// E o numero que faz o sopro chegar na pista. Mais alto e o fogo morre no ar;
/// mais baixo e o bicho passa por tras do terreno, que desenha na frente, e a
/// passada inteira acontece atras do chao onde ninguem ve.
const DRAGON_DIVE: f32 = 30.0;
/// Quanto o alvo da rasante corre na frente da propria cabeca.
///
/// A rasante persegue um ponto que anda com ela, e nao um ponto fixo no outro
/// canto: alvo fixo faz o bicho mergulhar direto para ele e atravessar o chao,
/// porque no meio do caminho o alvo ja esta em cima e a curva so aponta para
/// baixo. Correndo na frente, a altura puxa o voo para o nivel e ele nivela.
const DIVE_LEAD: f32 = 460.0;

// --- a composicao de cada tema ----------------------------------------------

/// Serra distante do vulcao. Os numeros sao alturas em linhas.
const VOLCANO_RIDGE: [u16; 15] = [2, 5, 3, 8, 4, 6, 3, 9, 5, 3, 7, 4, 8, 3, 2];
/// Borda interna da caldeira: mais perto, mais alta, e ja morna.
///
/// Duas bandas de serra em profundidades diferentes sao o que faz a montanha
/// ler como cordilheira. Com uma so, o cone fica num palco vazio.
const CALDERA_RIM: [u16; 11] = [3, 8, 5, 11, 6, 4, 9, 5, 10, 4, 3];
/// Cristas partidas do desfiladeiro.
const CHASM_RIDGE: [u16; 11] = [3, 7, 4, 11, 6, 2, 8, 3, 9, 4, 3];
/// Serra do tema oriental: mais baixa e mais longa, para o sol ficar por cima.
const EAST_RIDGE: [u16; 13] = [2, 4, 7, 3, 5, 9, 4, 6, 3, 8, 4, 5, 2];
/// Morros suaves do jardim, atras do dragao.
const GARDEN_RIDGE: [u16; 9] = [2, 5, 3, 4, 2, 6, 3, 5, 2];
/// Colinas do portao, quase planas: o gesto ali e o portao, nao a paisagem.
const GATE_RIDGE: [u16; 8] = [2, 3, 5, 4, 6, 3, 5, 2];
/// Torres distantes da fabrica.
const STACK_RIDGE: [u16; 11] = [3, 9, 4, 10, 3, 6, 11, 4, 9, 3, 5];
/// Morros atras da cidade: baixos, para aparecerem so entre os predios.
const CITY_RIDGE: [u16; 12] = [2, 4, 3, 6, 2, 5, 3, 6, 2, 4, 5, 2];

/// Predios distantes da cidade: `(x, y, colunas, linhas)`, ja no plano de tras.
const CITY_FAR: [Building; 9] = [
    (-600.0, -30.0, 9, 12),
    (-450.0, -10.0, 7, 16),
    (-300.0, -40.0, 11, 10),
    (-160.0, 0.0, 8, 18),
    (-20.0, -25.0, 12, 13),
    (140.0, -5.0, 7, 17),
    (290.0, -35.0, 10, 11),
    (450.0, -12.0, 8, 15),
    (600.0, -32.0, 11, 12),
];

