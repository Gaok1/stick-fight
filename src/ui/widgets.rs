/// Caracteres de moldura, pintados mais apagados que o texto.
const FRAME_CHARS: &str = "\u{2554}\u{2550}\u{2557}\u{2551}\u{255A}\u{255D}\u{2560}\u{2563}";
/// Linha de conteudo que vira separador horizontal.
const SEPARATOR: &str = "---";
/// Celulas da barra de vida.
const BAR_CELLS: u16 = 18;
/// Seta para a esquerda dos seletores.
const LEFT: &str = "\u{25C4}";
/// Seta para a direita.
const RIGHT: &str = "\u{25BA}";

/// Marca o HUD para atualizacao.
#[derive(Component)]
struct HudBar(u8);

/// Envolve as linhas numa moldura de linha dupla, alinhando tudo pela mais
/// longa. Gerar em vez de desenhar a mao e o que garante que a caixa fecha.
fn framed(lines: &[&str]) -> String {
    let width = lines
        .iter()
        .filter(|l| **l != SEPARATOR)
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(0);

    let mut out = String::new();
    out.push('\u{2554}');
    out.extend(std::iter::repeat_n('\u{2550}', width + 2));
    out.push_str("\u{2557}\n");

    for line in lines {
        if *line == SEPARATOR {
            out.push('\u{2560}');
            out.extend(std::iter::repeat_n('\u{2550}', width + 2));
            out.push_str("\u{2563}\n");
        } else {
            let pad = width - line.chars().count();
            out.push_str("\u{2551} ");
            out.push_str(line);
            out.extend(std::iter::repeat_n(' ', pad));
            out.push_str(" \u{2551}\n");
        }
    }

    out.push('\u{255A}');
    out.extend(std::iter::repeat_n('\u{2550}', width + 2));
    out.push('\u{255D}');
    out
}

/// Arte de uma caixa: moldura apagada, texto em branco.
fn box_art(lines: &[&str]) -> AsciiArt {
    AsciiArt::build(
        &framed(lines),
        &Accent {
            base: palette::BONE,
            accent: palette::ASH,
            on: FRAME_CHARS,
        },
    )
}

/// Repinta um trecho que ja esta escrito na caixa.
///
/// A caixa nasce de uma cor so e o realce vem depois, por busca. A alternativa
/// -- uma segunda string de cores, alinhada caractere a caractere com a
/// primeira -- e o tipo de par que sai de sincronia na primeira edicao do
/// texto, e sai calado.
fn highlight(art: AsciiArt, lines: &[&str], needle: &str, color: Color) -> AsciiArt {
    let Some((col, row)) = locate(lines, needle) else {
        return art;
    };
    art.stamp(&AsciiArt::solid(needle, color), col, row)
}

/// Onde um trecho cai dentro da caixa ja emoldurada, em coluna e linha.
///
/// Separada do realce para poder ser conferida: um alvo que deixou de existir
/// no texto nao quebra nada, so apaga a cor -- o menu volta a ser branco e
/// ninguem fica sabendo.
fn locate(lines: &[&str], needle: &str) -> Option<(u16, u16)> {
    lines.iter().enumerate().find_map(|(row, line)| {
        // A moldura desloca tudo: uma linha em cima, dois caracteres a esquerda.
        line.find(needle)
            .map(|at| (line[..at].chars().count() as u16 + 2, row as u16 + 1))
    })
}

// --- botoes -----------------------------------------------------------------

/// O que uma tela pode pedir.
///
/// Mouse e teclado escrevem isto; um sistema so decide o que fazer. E o que
/// permite os dois existirem sem duas copias das regras de navegacao.
#[derive(Message, Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuAction {
    /// Escolhe o modo de jogo.
    PickMode(GameMode),
    /// Gira a fase escolhida.
    Stage(i32),
    /// Sai da tela inicial para a escolha de lutador.
    Play,
    /// Volta uma tela.
    Back,
    /// Mexe numa linha do seletor de lutador. Passo zero so move o cursor.
    Fighter { seat: u8, row: usize, step: i32 },
    /// Confirma o lutador.
    Confirm,
    /// Manda um pedido para a sala online.
    Room(LobbyCommand),
    /// Comeca o round seguinte.
    NextRound,
}

/// O cursor esta em cima de algum botao?
///
/// A sala e uma arena viva: sem isto, clicar em COMECAR tambem daria um soco,
/// porque o M1 e golpe la dentro.
#[derive(Resource, Default)]
pub struct PointerOverUi(pub bool);

/// Um botao de tela.
#[derive(Component, Clone)]
struct Button {
    action: MenuAction,
    label: String,
    /// Largura fixa da etiqueta: sem ela, trocar o texto empurra o botao de
    /// lado e a area de clique passa a nao coincidir com o que se ve.
    width: usize,
    /// Cor em repouso.
    accent: Color,
    /// Este e o valor em uso agora.
    chosen: bool,
    hovered: bool,
    /// Aceita clique. Um botao apagado continua na tela para dizer que a acao
    /// existe -- some-lo faria a fileira de botoes dancar a cada mudanca.
    enabled: bool,
}

impl Button {
    fn new(label: &str, action: MenuAction) -> Self {
        Self {
            action,
            label: label.to_string(),
            width: label.chars().count(),
            accent: palette::ASH,
            chosen: false,
            hovered: false,
            enabled: true,
        }
    }

    fn width(mut self, width: usize) -> Self {
        self.width = width.max(self.label.chars().count());
        self
    }

    fn accent(mut self, color: Color) -> Self {
        self.accent = color;
        self
    }

    fn chosen(mut self, on: bool) -> Self {
        self.chosen = on;
        self
    }

    fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }

    fn spawn(self, commands: &mut Commands, until: GameState, at: Vec2) {
        commands.spawn((
            AsciiSprite::new(button_art(&self)),
            self,
            Layer::Hud,
            Transform::from_translation(at.extend(0.0)),
            DespawnOnExit(until),
        ));
    }
}

/// Como um botao aparece. Colchete diz "e este"; cor diz "o cursor esta aqui".
fn button_art(button: &Button) -> AsciiArt {
    let text = format!("{:^width$}", button.label, width = button.width);
    let text = if button.chosen {
        format!("[{text}]")
    } else {
        format!(" {text} ")
    };
    let color = match (button.enabled, button.hovered, button.chosen) {
        (false, ..) => palette::IRON,
        (_, true, _) => palette::GOLD,
        (_, _, true) => palette::BONE,
        _ => button.accent,
    };
    AsciiArt::solid(&text, color)
}

/// Area de clique de um botao.
///
/// Uma linha de texto tem dezesseis unidades de altura, que e pouco para mirar
/// com o mouse: a folga aqui e o que separa um botao clicavel de um botao
/// tecnicamente correto.
fn button_rect(transform: &Transform, sprite: &AsciiSprite) -> Rect {
    let size = sprite.art.size() * transform.scale.truncate();
    let center = transform.translation.truncate() - size * sprite.pivot;
    Rect::from_center_size(center, size + Vec2::new(8.0, 10.0))
}

/// Acende o botao sob o cursor e dispara o que ele faz.
fn point_at_buttons(
    cursor: Res<crate::actor::input::CursorWorld>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut over: ResMut<PointerOverUi>,
    mut actions: MessageWriter<MenuAction>,
    mut buttons: Query<(&mut Button, &Transform, &mut AsciiSprite)>,
) {
    let mut touching = false;
    for (mut button, transform, mut sprite) in &mut buttons {
        let hit = cursor
            .0
            .is_some_and(|at| button_rect(transform, &sprite).contains(at));
        touching |= hit;
        if button.hovered != hit {
            button.hovered = hit;
            sprite.art = button_art(&button);
        }
        if hit && button.enabled && mouse.just_pressed(MouseButton::Left) {
            actions.write(button.action);
        }
    }
    over.0 = touching;
}

/// Reescreve um botao sem repintar a tela a toa.
///
/// Comparar antes de atribuir importa: o `AsciiSprite` respawna todos os
/// glifos filhos a cada escrita, e escrever igual todo quadro seria refazer a
/// tela inteira sessenta vezes por segundo.
fn restyle(button: &mut Mut<Button>, sprite: &mut Mut<AsciiSprite>, next: Button) {
    if button.label != next.label
        || button.chosen != next.chosen
        || button.enabled != next.enabled
        || button.accent != next.accent
    {
        button.label = next.label;
        button.chosen = next.chosen;
        button.enabled = next.enabled;
        button.accent = next.accent;
        sprite.art = button_art(button);
    }
}

/// Gira um indice dentro de `len`, aceitando passo negativo.
fn cycle(current: usize, step: i32, len: usize) -> usize {
    (current as i32 + step).rem_euclid(len as i32) as usize
}

// --- tela inicial -----------------------------------------------------------

