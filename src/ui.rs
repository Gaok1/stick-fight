//! Telas e HUD.
//!
//! Tudo aqui e feito das mesmas primitivas de arte do jogo -- nao ha caminho de
//! texto separado. O titulo inclusive e gerado expandindo o bitmap da propria
//! fonte, entao a tela inicial nunca sai de estilo em relacao a arena.
//!
//! Mouse e teclado nao tem caminhos separados: os dois escrevem a mesma
//! [`MenuAction`], e um so lugar sabe o que cada acao significa. Enquanto o
//! menu foi so teclado, cada tela tinha o proprio `if keys.just_pressed`
//! espalhado, e ligar o mouse teria significado escrever tudo de novo do lado.

use bevy::prelude::*;

use crate::actor::face::{Face, Part};
use crate::actor::skin;
use crate::actor::{
    ActorSkin, ActorTint, DummyBehavior, Facing, Health, Intent, MAX_PLAYERS, Player, Pose,
    SkinSelections, TrainingDummy,
};
use crate::ascii::{Accent, AsciiArt, AsciiSprite, Layer, palette};
use crate::combat::{ComboMeter, MATCH_WINS, RoundResult, ShowBoxes};
use crate::level::{CATALOG as LEVEL_CATALOG, LevelPick, level_name};
use crate::online::{LobbyCommand, OnlineSession};
use crate::state::{AppSet, GameMode, GameState};
use crate::weapon::Held;

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

/// Painel de teclas da tela inicial.
#[derive(Component)]
struct MenuPanel;

/// Linha do modo escolhido, abaixo dos botoes.
#[derive(Component)]
struct MenuBlurb;

/// As linhas de teclas da tela inicial.
///
/// Separadas do desenho para que o teste dos realces trabalhe sobre o texto de
/// verdade. Enquanto ele tinha uma copia das linhas, a copia podia envelhecer
/// e o teste continuava passando sobre um menu que nao existia mais.
fn menu_lines() -> Vec<String> {
    [
        "                 PLAYER 1        PLAYER 2",
        "  MOVE            A / D        LEFT / RIGHT",
        "  JUMP              W               UP",
        "  CLIMB           W / S         UP / DOWN",
        "  GRAB / RELEASE     F           NUMPAD 1",
        "  DROP THRU          S              DOWN",
        "  COMBO M1       MOUSE 1          NUMPAD 0",
        "  SWEEP        DOWN + M1      DOWN + NUMPAD 0",
        "  DIVE KICK     M1 IN AIR      NUMPAD 0 IN AIR",
        "  AIM            MOUSE MOVE        FACING",
        "  SHOOT          MOUSE 2          NUMPAD 2",
        "  THROW WEAPON       G           NUMPAD 4",
        "  PARRY              Q           NUMPAD 3",
        SEPARATOR,
        "  FISTS FIRST. WEAPONS START DROPPING LATER.",
        "  A MATCH IS THE FIRST TO 3 ROUNDS.",
        "  THE STAGE ROLLS TO A RANDOM ONE AFTER EVERY ROUND.",
        "  ONLINE HOLDS TWO TO FOUR. WARM UP IN THE LOBBY.",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

/// Conteudo do painel de teclas.
fn menu_art() -> AsciiArt {
    let owned = menu_lines();
    let lines: Vec<&str> = owned.iter().map(String::as_str).collect();
    let art = box_art(&lines);
    let art = highlight(art, &lines, "PLAYER 1", palette::player(0));
    highlight(art, &lines, "PLAYER 2", palette::player(1))
}

/// Largura da maior etiqueta de modo, que e o que fixa a celula do seletor.
fn widest_label() -> usize {
    GameMode::ALL
        .iter()
        .map(|m| m.label().chars().count())
        .max()
        .unwrap_or(0)
}

/// Largura da maior etiqueta de fase.
fn widest_stage() -> usize {
    (0..LEVEL_CATALOG.len())
        .map(|i| level_name(i).chars().count())
        .max()
        .unwrap_or(0)
}

/// Onde cada botao de modo fica.
fn mode_slot(at: usize) -> Vec2 {
    let step = (widest_label() + 4) as f32 * crate::ascii::CELL.x + 16.0;
    let span = step * (GameMode::ALL.len() - 1) as f32;
    Vec2::new(at as f32 * step - span * 0.5, 108.0)
}

/// Tela inicial: titulo, seletores e controles.
fn spawn_controls_screen(mut commands: Commands, mode: Res<GameMode>, pick: Res<LevelPick>) {
    commands.spawn((
        AsciiSprite::new(AsciiArt::banner("STICK FIGHT", '\u{2588}', palette::BONE)),
        Layer::Hud,
        Transform::from_translation(Vec3::new(0.0, 250.0, 0.0)).with_scale(Vec3::splat(0.42)),
        DespawnOnExit(GameState::Controls),
    ));

    for (at, option) in GameMode::ALL.into_iter().enumerate() {
        Button::new(option.label(), MenuAction::PickMode(option))
            .width(widest_label())
            .chosen(option == *mode)
            .spawn(&mut commands, GameState::Controls, mode_slot(at));
    }

    let stage_x = (widest_stage() as f32 * crate::ascii::CELL.x) * 0.5 + 32.0;
    Button::new(LEFT, MenuAction::Stage(-1)).spawn(
        &mut commands,
        GameState::Controls,
        Vec2::new(-stage_x, 64.0),
    );
    Button::new(level_name(pick.0), MenuAction::Stage(1))
        .width(widest_stage())
        .chosen(true)
        .spawn(&mut commands, GameState::Controls, Vec2::new(0.0, 64.0));
    Button::new(RIGHT, MenuAction::Stage(1)).spawn(
        &mut commands,
        GameState::Controls,
        Vec2::new(stage_x, 64.0),
    );

    commands.spawn((
        MenuBlurb,
        AsciiSprite::new(AsciiArt::solid(mode.blurb().trim(), palette::MOSS)),
        Layer::Hud,
        Transform::from_translation(Vec3::new(0.0, 32.0, 0.0)),
        DespawnOnExit(GameState::Controls),
    ));

    Button::new("FIGHT", MenuAction::Play)
        .width(12)
        .accent(palette::BONE)
        .spawn(&mut commands, GameState::Controls, Vec2::new(0.0, -8.0));

    commands.spawn((
        MenuPanel,
        AsciiSprite::new(menu_art()),
        Layer::Hud,
        Transform::from_translation(Vec3::new(0.0, -195.0, 0.0)),
        DespawnOnExit(GameState::Controls),
    ));
}

/// Mantem os botoes da tela inicial em dia com o que esta escolhido.
fn update_controls_screen(
    mode: Res<GameMode>,
    pick: Res<LevelPick>,
    mut buttons: Query<(&mut Button, &mut AsciiSprite), Without<MenuBlurb>>,
    mut blurb: Query<&mut AsciiSprite, With<MenuBlurb>>,
) {
    for (mut button, mut sprite) in &mut buttons {
        let next = match button.action {
            MenuAction::PickMode(option) => Button::new(option.label(), button.action)
                .width(widest_label())
                .chosen(option == *mode),
            MenuAction::Stage(1) if button.width > 1 => {
                Button::new(level_name(pick.0), button.action)
                    .width(widest_stage())
                    .chosen(true)
            }
            _ => continue,
        };
        restyle(&mut button, &mut sprite, next);
    }
    for mut sprite in &mut blurb {
        let art = AsciiArt::solid(mode.blurb().trim(), palette::MOSS);
        if sprite.art != art {
            sprite.art = art;
        }
    }
}

// --- sala online ------------------------------------------------------------

#[derive(Component)]
struct LobbyPanel;

/// Painel da sala: quem esta dentro e o que o jogo esta esperando.
///
/// Ele mora encostado num canto porque a arena esta viva atras: o lobby e
/// jogavel, e um painel no meio da tela ficaria em cima do lugar onde se
/// briga enquanto os amigos nao chegam.
fn lobby_art(session: &OnlineSession) -> AsciiArt {
    let lobby_id = session
        .lobby
        .map(|lobby| lobby.raw().to_string())
        .unwrap_or_else(|| "NONE".into());
    let local = session.local_player_id();

    let mut lines = vec![
        format!("STEAM ONLINE      {lobby_id}"),
        SEPARATOR.to_string(),
        format!("{:<44}", session.status),
        SEPARATOR.to_string(),
    ];

    // Uma linha por lugar, cheio ou nao: ver a vaga aberta e o que diz que
    // ainda cabe gente, e por isso a sala mostra os quatro mesmo com dois.
    let roster = lines.len();
    lines.extend((0..MAX_PLAYERS).map(|slot| {
        let who = session
            .members
            .get(slot)
            .map(String::as_str)
            .unwrap_or("---");
        let you = if slot as u8 == local && session.seated() {
            RIGHT
        } else {
            " "
        };
        let host = if slot == 0 { "HOST" } else { "    " };
        format!("{you} P{}  {:<28} {host}", slot + 1, who)
    }));

    lines.extend([
        SEPARATOR.to_string(),
        "  WARM UP WHILE YOU WAIT".to_string(),
    ]);

    let text: Vec<&str> = lines.iter().map(String::as_str).collect();
    let mut art = box_art(&text);
    // Cada lugar leva a cor do boneco dele -- a mesma que esta correndo na
    // arena atras do painel. E o que liga um nome na lista a um boneco na tela.
    for slot in 0..MAX_PLAYERS {
        art = art.stamp(
            &AsciiArt::solid(&format!("P{}", slot + 1), palette::player(slot as u8)),
            4,
            (roster + slot + 1) as u16,
        );
    }
    art = highlight(art, &text, RIGHT, palette::GOLD);
    highlight(art, &text, "WARM UP WHILE YOU WAIT", palette::MOSS)
}

/// Os botoes da sala, de cima para baixo.
///
/// A lista e a mesma sempre; o que muda e quem esta aceso. Uma sala que
/// esconde botoes conforme o estado faz a fileira dancar, e o jogador clica no
/// lugar onde o botao estava um instante atras.
fn lobby_buttons(session: &OnlineSession) -> [(MenuAction, &'static str, bool); 5] {
    let idle = !session.in_lobby();
    [
        (
            MenuAction::Room(LobbyCommand::Create),
            "CREATE ROOM",
            idle,
        ),
        (MenuAction::Room(LobbyCommand::Find), "FIND ROOM", idle),
        (
            MenuAction::Room(LobbyCommand::Invite),
            "INVITE FRIEND",
            session.in_lobby(),
        ),
        (
            MenuAction::Room(LobbyCommand::Start),
            "START MATCH",
            session.can_start(),
        ),
        (MenuAction::Room(LobbyCommand::Leave), "LEAVE", true),
    ]
}

/// Onde o botao `at` da sala fica.
fn lobby_slot(at: usize) -> Vec2 {
    Vec2::new(430.0, 300.0 - at as f32 * 34.0)
}

fn spawn_lobby_screen(mut commands: Commands, session: Res<OnlineSession>, pick: Res<LevelPick>) {
    commands.spawn((
        LobbyPanel,
        // Ancorado pelo canto superior esquerdo: a caixa cresce para dentro da
        // tela conforme o status muda de tamanho, em vez de escorregar de lado
        // a cada palavra -- e desce a partir do topo, deixando livre a faixa do
        // chao, que e onde os bonecos ficam.
        AsciiSprite::pivoted(lobby_art(&session), Vec2::new(-0.5, 0.5)),
        Layer::Hud,
        Transform::from_translation(Vec3::new(-628.0, 300.0, 0.0)),
        DespawnOnExit(GameState::Lobby),
    ));

    for (at, (action, label, enabled)) in lobby_buttons(&session).into_iter().enumerate() {
        Button::new(label, action)
            .width(13)
            .enabled(enabled)
            .spawn(&mut commands, GameState::Lobby, lobby_slot(at));
    }

    // A fase e do dono: quem entrou ve qual e, e so ele a troca.
    let stage_y = lobby_slot(lobby_buttons(&session).len()).y - 12.0;
    Button::new(LEFT, MenuAction::Stage(-1))
        .enabled(session.is_host())
        .spawn(
            &mut commands,
            GameState::Lobby,
            Vec2::new(430.0 - widest_stage() as f32 * 4.0 - 24.0, stage_y),
        );
    Button::new(level_name(pick.0), MenuAction::Stage(1))
        .width(widest_stage())
        .chosen(true)
        .enabled(session.is_host())
        .spawn(&mut commands, GameState::Lobby, Vec2::new(430.0, stage_y));
    Button::new(RIGHT, MenuAction::Stage(1))
        .enabled(session.is_host())
        .spawn(
            &mut commands,
            GameState::Lobby,
            Vec2::new(430.0 + widest_stage() as f32 * 4.0 + 24.0, stage_y),
        );
}

fn update_lobby_screen(
    session: Res<OnlineSession>,
    pick: Res<LevelPick>,
    mut panels: Query<&mut AsciiSprite, (With<LobbyPanel>, Without<Button>)>,
    mut buttons: Query<(&mut Button, &mut AsciiSprite)>,
) {
    for mut panel in &mut panels {
        let art = lobby_art(&session);
        if panel.art != art {
            panel.art = art;
        }
    }
    let states = lobby_buttons(&session);
    for (mut button, mut sprite) in &mut buttons {
        let next = match button.action {
            MenuAction::Room(_) => {
                let Some((action, label, enabled)) =
                    states.iter().find(|(action, ..)| *action == button.action)
                else {
                    continue;
                };
                Button::new(label, *action).width(13).enabled(*enabled)
            }
            MenuAction::Stage(1) if button.width > 1 => {
                Button::new(level_name(pick.0), button.action)
                    .width(widest_stage())
                    .chosen(true)
                    .enabled(session.is_host())
            }
            MenuAction::Stage(_) => Button::new(&button.label.clone(), button.action)
                .enabled(session.is_host()),
            _ => continue,
        };
        restyle(&mut button, &mut sprite, next);
    }
}

// --- escolha de lutador -----------------------------------------------------

/// Painel e atores da tela de escolha cosmetica.
#[derive(Component)]
struct SkinPanel;

#[derive(Component)]
struct SkinPreview(u8);

/// Linha destacada no seletor de lutador.
///
/// Zero e a pele; o resto sao as pecas do rosto, na ordem de [`Part::CHOSEN`].
/// Pele e rosto sao a mesma pergunta -- com que cara este boneco entra --
/// entao eles moram no mesmo cursor.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
struct FighterRow(usize);

/// Quantas linhas o seletor tem.
fn fighter_rows() -> usize {
    1 + Part::CHOSEN.len()
}

fn row_label(row: usize) -> &'static str {
    match row {
        0 => "SKIN",
        at => Part::CHOSEN[(at - 1) % Part::CHOSEN.len()].label(),
    }
}

/// Nome da opcao escolhida agora por este lugar.
fn row_value(row: usize, skin_pick: usize, face: Face) -> &'static str {
    match row {
        0 => skin::skin(skin_pick).name,
        at => face.look(Part::CHOSEN[(at - 1) % Part::CHOSEN.len()]).name,
    }
}

/// Gira a escolha desta linha.
fn row_cycle(row: usize, skin_pick: &mut usize, face: &mut Face, step: i32) {
    match row {
        0 => *skin_pick = cycle(*skin_pick, step, skin::CATALOG.len()),
        at => face.cycle(Part::CHOSEN[(at - 1) % Part::CHOSEN.len()], step),
    }
}

/// Largura fixa da celula de valor, para o seletor nao mudar de tamanho ao
/// navegar -- um botao que encolhe leva a area de clique junto.
fn fighter_cell() -> usize {
    skin::CATALOG
        .iter()
        .map(|s| s.name.chars().count())
        .chain(
            Part::CHOSEN
                .iter()
                .flat_map(|part| part.catalog())
                .map(|look| look.name.chars().count()),
        )
        .max()
        .unwrap_or(0)
}

/// A escolha de um lugar do seletor: pele e rosto.
fn seat_choice(picks: &SkinSelections, mode: GameMode, seat: u8) -> (usize, Face) {
    if mode == GameMode::Online {
        (picks.online_local, picks.online_face)
    } else {
        (
            picks.players[seat as usize],
            picks.faces[seat as usize],
        )
    }
}

/// Altura da linha `row` do seletor.
fn fighter_row_y(row: usize) -> f32 {
    190.0 - row as f32 * 34.0
}

/// Onde a coluna de um lugar comeca.
fn fighter_column_x(mode: GameMode, seat: u8) -> f32 {
    if mode == GameMode::Online {
        60.0
    } else if seat == 0 {
        -150.0
    } else {
        230.0
    }
}

/// Quantos lugares esta tela deixa escolher.
///
/// Online so ha um -- o seu. No treino o segundo lugar e do dummy, e mostrar
/// um seletor la prometeria uma escolha que nao existe.
fn fighter_seats(mode: GameMode) -> u8 {
    match mode {
        GameMode::Online | GameMode::Training => 1,
        _ => 2,
    }
}

fn spawn_skin_select_screen(
    mut commands: Commands,
    mode: Res<GameMode>,
    picks: Res<SkinSelections>,
    row: Res<FighterRow>,
) {
    let title = if fighter_seats(*mode) == 1 {
        "CHOOSE YOUR FIGHTER"
    } else {
        "CHOOSE YOUR FIGHTERS"
    };
    commands.spawn((
        SkinPanel,
        AsciiSprite::new(AsciiArt::solid(title, palette::GOLD)),
        Layer::Hud,
        Transform::from_translation(Vec3::new(0.0, 250.0, 0.0)),
        DespawnOnExit(GameState::SkinSelect),
    ));

    let cell = fighter_cell();
    for line in 0..fighter_rows() {
        let y = fighter_row_y(line);
        Button::new(
            row_label(line),
            MenuAction::Fighter {
                seat: 0,
                row: line,
                step: 0,
            },
        )
        .width(6)
        .chosen(line == row.0)
        .spawn(
            &mut commands,
            GameState::SkinSelect,
            Vec2::new(fighter_column_x(*mode, 0) - 150.0, y),
        );

        for seat in 0..fighter_seats(*mode) {
            let x = fighter_column_x(*mode, seat);
            let (pick, face) = seat_choice(&picks, *mode, seat);
            let step = |step| MenuAction::Fighter {
                seat,
                row: line,
                step,
            };
            let half = cell as f32 * 4.0 + 20.0;
            Button::new(LEFT, step(-1)).spawn(
                &mut commands,
                GameState::SkinSelect,
                Vec2::new(x - half, y),
            );
            Button::new(row_value(line, pick, face), step(1))
                .width(cell)
                .accent(palette::player(seat))
                .spawn(&mut commands, GameState::SkinSelect, Vec2::new(x, y));
            Button::new(RIGHT, step(1)).spawn(
                &mut commands,
                GameState::SkinSelect,
                Vec2::new(x + half, y),
            );
        }
    }

    Button::new("BACK", MenuAction::Back)
        .width(10)
        .spawn(
            &mut commands,
            GameState::SkinSelect,
            Vec2::new(-140.0, -300.0),
        );
    Button::new("CONFIRM", MenuAction::Confirm)
        .width(10)
        .accent(palette::BONE)
        .spawn(
            &mut commands,
            GameState::SkinSelect,
            Vec2::new(140.0, -300.0),
        );

    if fighter_seats(*mode) == 1 {
        let (pick, face) = seat_choice(&picks, *mode, 0);
        spawn_skin_preview(&mut commands, 0, skin::skin(pick), face, Vec2::new(-330.0, -60.0));
    } else {
        spawn_skin_preview(
            &mut commands,
            0,
            skin::skin(picks.players[0]),
            picks.faces[0],
            Vec2::new(-430.0, -60.0),
        );
        spawn_skin_preview(
            &mut commands,
            1,
            skin::skin(picks.players[1]),
            picks.faces[1],
            Vec2::new(430.0, -60.0),
        );
    }
}

fn spawn_skin_preview(
    commands: &mut Commands,
    id: u8,
    chosen: &'static skin::Skin,
    face: Face,
    at: Vec2,
) {
    let color = palette::player(id);
    let root = commands
        .spawn((
            SkinPreview(id),
            Pose::IdleA,
            ActorTint(color),
            Facing(if id == 0 { 1.0 } else { -1.0 }),
            Intent::default(),
            Transform::from_translation(at.extend(0.0)).with_scale(Vec3::splat(1.7)),
            Visibility::default(),
            DespawnOnExit(GameState::SkinSelect),
        ))
        .id();
    crate::actor::spawn_actor_body(
        commands,
        root,
        chosen,
        color,
        if id == 0 { 1.0 } else { -1.0 },
        id as f32 * 1.7,
        face,
    );
}

/// Mantem os botoes e os bonecos de amostra em dia com o que foi escolhido.
fn update_skin_select_screen(
    mode: Res<GameMode>,
    picks: Res<SkinSelections>,
    row: Res<FighterRow>,
    mut buttons: Query<(&mut Button, &mut AsciiSprite)>,
    mut previews: Query<(&SkinPreview, &mut ActorSkin, &mut Face)>,
) {
    let cell = fighter_cell();
    for (mut button, mut sprite) in &mut buttons {
        let MenuAction::Fighter {
            seat,
            row: line,
            step,
        } = button.action
        else {
            continue;
        };
        let next = if step == 0 {
            Button::new(row_label(line), button.action)
                .width(6)
                .chosen(line == row.0)
        } else if button.width > 1 {
            let (pick, face) = seat_choice(&picks, *mode, seat);
            Button::new(row_value(line, pick, face), button.action)
                .width(cell)
                .accent(palette::player(seat))
        } else {
            continue;
        };
        restyle(&mut button, &mut sprite, next);
    }

    for (preview, mut actor_skin, mut face) in &mut previews {
        let (pick, next) = seat_choice(&picks, *mode, preview.0);
        actor_skin.0 = skin::skin(pick);
        face.set_if_neq(next);
    }
}

/// Pequena demonstracao feita das poses reais; nao existe rig paralelo no menu.
fn animate_skin_previews(time: Res<Time>, mut previews: Query<(&SkinPreview, &mut Pose)>) {
    for (preview, mut pose) in &mut previews {
        let t = (time.elapsed_secs() + preview.0 as f32 * 0.18).rem_euclid(4.8);
        let next = match t {
            x if x < 0.8 => Pose::idling((x * 3.0) as usize),
            x if x < 2.1 => Pose::running(((x - 0.8) * 9.0) as usize),
            x if x < 2.55 => Pose::Jump,
            x if x < 2.95 => Pose::Fall,
            x if x < 3.35 => Pose::PunchWindup,
            x if x < 3.65 => Pose::PunchStrike,
            x if x < 4.05 => Pose::PunchRecover,
            _ => Pose::IdleA,
        };
        if *pose != next {
            *pose = next;
        }
    }
}

// --- fim de round -----------------------------------------------------------

/// Rounds vencidos, desenhados como blocos cheios sobre os que faltam.
///
/// Numero exige leitura; bloco se conta de relance, que e o que se quer entre
/// um round e o proximo.
fn pips(wins: u32) -> String {
    (0..MATCH_WINS)
        .map(|i| if i < wins { '\u{2588}' } else { '\u{2591}' })
        .collect()
}

/// Placar de fim de round -- ou de fim de partida, quando alguem chega la.
pub(crate) fn spawn_round_over_screen(
    mut commands: Commands,
    result: Res<RoundResult>,
    mode: Res<GameMode>,
    pick: Res<LevelPick>,
    online: Res<OnlineSession>,
) {
    let champion = result.match_winner();

    let (title, color) = match (champion, result.winner) {
        (Some(id), _) => (format!("PLAYER {} TAKES IT", id + 1), palette::player(id)),
        (None, Some(id)) => (format!("PLAYER {} WINS", id + 1), palette::player(id)),
        // Ninguem de pe. Com quatro em campo "DOUBLE" mentiria sobre quantos
        // cairam juntos, entao a palavra some.
        (None, None) => ("EVERYBODY DIES".to_string(), palette::BONE),
    };

    commands.spawn((
        AsciiSprite::new(AsciiArt::banner(&title, '\u{2588}', color)),
        Layer::Hud,
        Transform::from_translation(Vec3::new(0.0, 130.0, 0.0)).with_scale(Vec3::splat(0.45)),
        DespawnOnExit(GameState::RoundOver),
    ));

    // Uma linha por lugar em jogo. Enquanto foi uma linha so com os dois lados
    // frente a frente, nao havia onde por o terceiro -- e mostrar os quatro
    // sempre encheria a tela de zeros numa briga de dois.
    let mut lines: Vec<String> = (0..result.seats())
        .map(|id| {
            format!(
                "  P{}  {}{}",
                id + 1,
                pips(result.score[id]),
                if result.winner == Some(id as u8) {
                    "  <"
                } else {
                    ""
                }
            )
        })
        .collect();
    lines.insert(0, format!("  MATCH   -   FIRST TO {MATCH_WINS}"));

    let progress = match champion {
        Some(_) => format!("  TOOK {MATCH_WINS} ROUNDS IN {}", result.rounds),
        None => format!("  ROUND {} NEXT", result.rounds + 1),
    };
    lines.extend([
        SEPARATOR.to_string(),
        progress,
        format!("  NEXT STAGE   {}", level_name(pick.0)),
    ]);

    commands.spawn((
        AsciiSprite::new(box_art(
            &lines.iter().map(String::as_str).collect::<Vec<_>>(),
        )),
        Layer::Hud,
        Transform::from_translation(Vec3::new(0.0, -20.0, 0.0)),
        DespawnOnExit(GameState::RoundOver),
    ));

    // Quem nao manda na partida ve o botao apagado em vez de nao ver botao
    // nenhum: e assim que fica claro que a espera e pelo dono, e nao um bug.
    let waiting = *mode == GameMode::Online && !online.is_host();
    let label = if champion.is_some() {
        "NEW MATCH"
    } else {
        "NEXT ROUND"
    };
    Button::new(if waiting { "WAITING HOST" } else { label }, MenuAction::NextRound)
        .width(12)
        .accent(palette::BONE)
        .enabled(!waiting)
        .spawn(
            &mut commands,
            GameState::RoundOver,
            Vec2::new(-120.0, -190.0),
        );
    Button::new("LEAVE", MenuAction::Back).width(12).spawn(
        &mut commands,
        GameState::RoundOver,
        Vec2::new(120.0, -190.0),
    );
}

// --- navegacao --------------------------------------------------------------

/// Traduz teclado em acao de tela.
fn keyboard_actions(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<GameState>>,
    mode: Res<GameMode>,
    session: Res<OnlineSession>,
    row: Res<FighterRow>,
    mut actions: MessageWriter<MenuAction>,
) {
    let enter = keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter]);
    let back = keys.just_pressed(KeyCode::Escape);
    let vertical = keys.any_just_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) as i32
        - keys.any_just_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) as i32;

    match state.get() {
        GameState::Controls => {
            let horizontal = keys.any_just_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) as i32
                - keys.any_just_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) as i32;
            if horizontal != 0 {
                actions.write(MenuAction::Stage(horizontal));
            }
            if vertical != 0 {
                let at = GameMode::ALL.iter().position(|m| *m == *mode).unwrap_or(0);
                actions.write(MenuAction::PickMode(
                    GameMode::ALL[cycle(at, vertical, GameMode::ALL.len())],
                ));
            }
            if enter {
                actions.write(MenuAction::Play);
            }
        }
        GameState::SkinSelect => {
            // Cada jogador anda no proprio par de teclas, mas a linha e uma so:
            // os dois escolhem a mesma peca ao mesmo tempo, e a tela nao precisa
            // de dois cursores disputando espaco.
            let p1 = keys.just_pressed(KeyCode::KeyD) as i32
                - keys.just_pressed(KeyCode::KeyA) as i32;
            let p2 = keys.just_pressed(KeyCode::ArrowRight) as i32
                - keys.just_pressed(KeyCode::ArrowLeft) as i32;
            if vertical != 0 {
                actions.write(MenuAction::Fighter {
                    seat: 0,
                    row: cycle(row.0, vertical, fighter_rows()),
                    step: 0,
                });
            }
            let solo = fighter_seats(*mode) == 1;
            let first = if solo && p1 == 0 { p2 } else { p1 };
            if first != 0 {
                actions.write(MenuAction::Fighter {
                    seat: 0,
                    row: row.0,
                    step: first,
                });
            }
            if !solo && p2 != 0 {
                actions.write(MenuAction::Fighter {
                    seat: 1,
                    row: row.0,
                    step: p2,
                });
            }
            if enter {
                actions.write(MenuAction::Confirm);
            }
            if back {
                actions.write(MenuAction::Back);
            }
        }
        GameState::Lobby => {
            if enter {
                actions.write(MenuAction::Room(if session.in_lobby() {
                    LobbyCommand::Start
                } else {
                    LobbyCommand::Create
                }));
            }
            if keys.just_pressed(KeyCode::KeyF) {
                actions.write(MenuAction::Room(LobbyCommand::Find));
            }
            if keys.just_pressed(KeyCode::KeyI) {
                actions.write(MenuAction::Room(LobbyCommand::Invite));
            }
            if back {
                actions.write(MenuAction::Back);
            }
        }
        GameState::RoundOver => {
            if enter {
                actions.write(MenuAction::NextRound);
            }
            if back {
                actions.write(MenuAction::Back);
            }
        }
        GameState::Fighting => {
            if back {
                actions.write(MenuAction::Back);
            }
        }
    }
}

/// Faz o que a tela pediu. Um lugar so, para mouse e teclado nao divergirem.
fn apply_menu_action(
    mut actions: MessageReader<MenuAction>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
    mut mode: ResMut<GameMode>,
    mut pick: ResMut<LevelPick>,
    mut picks: ResMut<SkinSelections>,
    mut row: ResMut<FighterRow>,
    mut room: MessageWriter<LobbyCommand>,
) {
    for action in actions.read() {
        match *action {
            MenuAction::PickMode(chosen) => *mode = chosen,
            MenuAction::Stage(step) => pick.0 = cycle(pick.0, step, LEVEL_CATALOG.len()),
            MenuAction::Play => next.set(GameState::SkinSelect),
            MenuAction::Confirm => next.set(if *mode == GameMode::Online {
                GameState::Lobby
            } else {
                GameState::Fighting
            }),
            MenuAction::Fighter {
                seat,
                row: line,
                step,
            } => {
                row.0 = line % fighter_rows();
                if step != 0 {
                    let solo = *mode == GameMode::Online;
                    let (mut chosen, mut face) = seat_choice(&picks, *mode, seat);
                    row_cycle(line, &mut chosen, &mut face, step);
                    if solo {
                        picks.online_local = chosen;
                        picks.online_face = face;
                    } else {
                        picks.players[seat as usize] = chosen;
                        picks.faces[seat as usize] = face;
                    }
                }
            }
            MenuAction::Room(command) => {
                room.write(command);
            }
            MenuAction::NextRound => {
                if *mode == GameMode::Online {
                    room.write(LobbyCommand::Start);
                } else {
                    next.set(GameState::Fighting);
                }
            }
            MenuAction::Back => match state.get() {
                GameState::SkinSelect => next.set(GameState::Controls),
                GameState::Lobby => {
                    room.write(LobbyCommand::Leave);
                }
                GameState::Fighting | GameState::RoundOver => {
                    next.set(if *mode == GameMode::Online {
                        GameState::Lobby
                    } else {
                        GameState::Controls
                    })
                }
                GameState::Controls => {}
            },
        }
    }
}

// --- HUD --------------------------------------------------------------------

/// Barra de vida + arma de um jogador.
fn hud_art(id: u8, color: Color, health: &Health, held: Option<&Held>) -> AsciiArt {
    let filled = (health.fraction() * BAR_CELLS as f32).round() as u16;
    let label = AsciiArt::solid(&format!("P{}", id + 1), color);

    let mut art = label.stamp(&AsciiArt::fill('\u{2588}', filled, 1, color), 3, 0);
    art = art.stamp(
        &AsciiArt::fill('\u{2591}', BAR_CELLS - filled, 1, palette::IRON),
        3 + filled,
        0,
    );
    art = art.stamp(
        &AsciiArt::solid(&format!("{:>4}", health.hp.max(0)), palette::BONE),
        4 + BAR_CELLS,
        0,
    );

    let weapon = match held {
        // Arma de contato nao tem municao; mostrar "x0" a faria passar por
        // arma de fogo vazia, que e o oposto do que ela e.
        Some(held) if held.weapon.is_melee() => held.weapon.name().to_string(),
        Some(held) => format!("{} x{}", held.weapon.name(), held.ammo),
        None => "FISTS".to_string(),
    };
    art.stamp(&AsciiArt::solid(&weapon, palette::GOLD), 3, 1)
}

/// Altura da segunda fileira de barras.
///
/// A arte de uma barra tem duas linhas de celula; o passo abre uma linha de
/// folga entre as fileiras para elas nao lerem como um bloco so.
const HUD_ROW: f32 = 3.0 * crate::ascii::CELL.y;

/// Onde a barra de um lugar fica: canto e fileira.
///
/// Impares a direita, pares a esquerda, descendo de dois em dois. Assim uma
/// briga de dois mantem exatamente o HUD de sempre e o terceiro jogador
/// aparece sem empurrar ninguem.
fn hud_anchor(id: u8) -> (Vec3, f32) {
    let side = if id.is_multiple_of(2) { -1.0 } else { 1.0 };
    let row = (id / 2) as f32;
    (
        Vec3::new(630.0 * side, 228.0 - row * HUD_ROW, 0.0),
        side * 0.5,
    )
}

/// Cria uma barra por lutador ao comecar a luta.
fn spawn_hud(mut commands: Commands, players: Query<(&Player, &Health)>) {
    for (player, health) in &players {
        let (at, pivot_x) = hud_anchor(player.id);

        commands.spawn((
            HudBar(player.id),
            AsciiSprite::pivoted(
                hud_art(player.id, player.color, health, None),
                Vec2::new(pivot_x, 0.5),
            ),
            Layer::Hud,
            Transform::from_translation(at),
            DespawnOnExit(GameState::Fighting),
        ));
    }
}

/// Painel de treino: vida do dummy, combo em andamento e recorde da sessao.
#[derive(Component)]
struct TrainingPanel;

fn training_art(
    dummy: &Health,
    meter: &ComboMeter,
    behavior: DummyBehavior,
    boxes: ShowBoxes,
) -> AsciiArt {
    let filled = (dummy.fraction() * BAR_CELLS as f32).round() as u16;
    let mut art = AsciiArt::solid("DUMMY", palette::GOLD);
    art = art.stamp(&AsciiArt::fill('\u{2588}', filled, 1, palette::GOLD), 6, 0);
    art = art.stamp(
        &AsciiArt::fill('\u{2591}', BAR_CELLS - filled, 1, palette::IRON),
        6 + filled,
        0,
    );

    // Enquanto o combo corre ele fica em vermelho; parado, apagado. A cor e o
    // que diz se os golpes ainda estao encadeando.
    let live = meter.hits > 0;
    art = art.stamp(
        &AsciiArt::solid(
            &format!("COMBO {:>2} HITS   {:>3} DMG", meter.hits, meter.damage),
            if live { palette::BLOOD } else { palette::IRON },
        ),
        6,
        1,
    );
    // Nomear o elo que acabou de acertar e o que ensina o combo: sem isso os
    // tres golpes sao um contador subindo.
    art = art.stamp(
        &AsciiArt::solid(
            &format!("{:<8}", if live { meter.last_move } else { "" }),
            palette::GOLD,
        ),
        6 + BAR_CELLS + 2,
        1,
    );
    art = art.stamp(
        &AsciiArt::solid(
            &format!(
                "BEST  {:>2} HITS   {:>3} DMG",
                meter.best_hits, meter.best_damage
            ),
            palette::ASH,
        ),
        6,
        2,
    );
    art.stamp(
        &AsciiArt::solid(
            &format!(
                "TAB {:<6}  H BOXES {}",
                behavior.label(),
                if boxes.0 { "ON " } else { "OFF" }
            ),
            palette::MOSS,
        ),
        6 + BAR_CELLS + 2,
        2,
    )
}

fn spawn_training_panel(
    mut commands: Commands,
    dummies: Query<&Health, With<TrainingDummy>>,
    meter: Res<ComboMeter>,
    behavior: Res<DummyBehavior>,
    boxes: Res<ShowBoxes>,
) {
    let Ok(dummy) = dummies.single() else {
        return;
    };
    commands.spawn((
        TrainingPanel,
        AsciiSprite::pivoted(
            training_art(dummy, &meter, *behavior, *boxes),
            Vec2::new(0.5, 0.5),
        ),
        Layer::Hud,
        Transform::from_translation(Vec3::new(630.0, 228.0, 0.0)),
        DespawnOnExit(GameState::Fighting),
    ));
}

fn update_training_panel(
    dummies: Query<&Health, With<TrainingDummy>>,
    healed: Query<(), (With<TrainingDummy>, Changed<Health>)>,
    meter: Res<ComboMeter>,
    behavior: Res<DummyBehavior>,
    boxes: Res<ShowBoxes>,
    mut panels: Query<&mut AsciiSprite, With<TrainingPanel>>,
) {
    // Redesenhar todo frame respawnaria os ~70 glifos do painel a toa.
    if !meter.is_changed() && !behavior.is_changed() && !boxes.is_changed() && healed.is_empty() {
        return;
    }
    let Ok(dummy) = dummies.single() else {
        return;
    };
    for mut panel in &mut panels {
        panel.art = training_art(dummy, &meter, *behavior, *boxes);
    }
}

/// Redesenha a barra so quando vida ou arma mudam.
fn update_hud(
    players: Query<(&Player, &Health, Option<&Held>), Or<(Changed<Health>, Changed<Held>)>>,
    mut bars: Query<(&HudBar, &mut AsciiSprite)>,
) {
    for (player, health, held) in &players {
        for (bar, mut sprite) in &mut bars {
            if bar.0 == player.id {
                sprite.art = hud_art(player.id, player.color, health, held);
            }
        }
    }
}

/// Redesenha a barra quando a arma acaba (o `Changed` nao ve remocao).
fn refresh_hud_on_weapon_loss(
    mut removed: RemovedComponents<Held>,
    players: Query<(&Player, &Health)>,
    mut bars: Query<(&HudBar, &mut AsciiSprite)>,
) {
    for entity in removed.read() {
        let Ok((player, health)) = players.get(entity) else {
            continue;
        };
        for (bar, mut sprite) in &mut bars {
            if bar.0 == player.id {
                sprite.art = hud_art(player.id, player.color, health, None);
            }
        }
    }
}

fn pulse_low_health(
    time: Res<Time>,
    players: Query<(&Player, &Health)>,
    mut bars: Query<(&HudBar, &mut Transform)>,
) {
    for (bar, mut transform) in &mut bars {
        let low = players
            .iter()
            .find(|(player, _)| player.id == bar.0)
            .is_some_and(|(_, health)| health.fraction() < 0.3);
        let scale = if low {
            1.0 + (time.elapsed_secs() * 9.0).sin().max(0.0) * 0.06
        } else {
            1.0
        };
        transform.scale = Vec3::splat(scale);
    }
}

/// Troca o que o dummy faz, sem sair da luta.
///
/// Fica na camada de UI porque e um controle de tela, nao de personagem: o
/// gameplay continua sem ler teclado.
fn cycle_dummy_behavior(
    keys: Res<ButtonInput<KeyCode>>,
    mut behavior: ResMut<DummyBehavior>,
    mut boxes: ResMut<ShowBoxes>,
) {
    if keys.just_pressed(KeyCode::KeyH) {
        boxes.0 = !boxes.0;
    }
    if keys.just_pressed(KeyCode::Tab) {
        *behavior = behavior.next();
    }
}

/// Telas, HUD e navegacao.
pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FighterRow>()
            .init_resource::<PointerOverUi>()
            .add_message::<MenuAction>()
            .add_systems(OnEnter(GameState::Controls), spawn_controls_screen)
            .add_systems(OnEnter(GameState::SkinSelect), spawn_skin_select_screen)
            .add_systems(OnEnter(GameState::Lobby), spawn_lobby_screen)
            .add_systems(OnEnter(GameState::RoundOver), spawn_round_over_screen)
            // O HUD le os jogadores, entao precisa entrar depois do spawn deles.
            .add_systems(
                OnEnter(GameState::Fighting),
                spawn_hud.after(crate::actor::spawn_players),
            )
            .add_systems(
                OnEnter(GameState::Fighting),
                spawn_training_panel
                    .after(crate::actor::spawn_training_dummy)
                    .run_if(resource_equals(GameMode::Training)),
            )
            // O mouse antes do teclado, e os dois antes de quem decide: assim
            // um clique e uma tecla no mesmo quadro chegam juntos.
            .add_systems(
                Update,
                (point_at_buttons, keyboard_actions)
                    .in_set(AppSet::Input)
                    .before(crate::actor::input::gather_intents),
            )
            .add_systems(Update, apply_menu_action.in_set(AppSet::Logic))
            .add_systems(
                Update,
                update_controls_screen
                    .in_set(AppSet::Animate)
                    .run_if(in_state(GameState::Controls)),
            )
            .add_systems(
                Update,
                (update_skin_select_screen, animate_skin_previews)
                    .in_set(AppSet::Animate)
                    .before(crate::actor::motion::apply_pose)
                    .run_if(in_state(GameState::SkinSelect)),
            )
            .add_systems(
                Update,
                update_lobby_screen
                    .in_set(AppSet::Animate)
                    .run_if(in_state(GameState::Lobby)),
            )
            .add_systems(
                Update,
                (update_hud, refresh_hud_on_weapon_loss, pulse_low_health)
                    .in_set(AppSet::Animate)
                    .run_if(in_state(GameState::Fighting)),
            )
            .add_systems(
                Update,
                (
                    update_training_panel.in_set(AppSet::Animate),
                    cycle_dummy_behavior.in_set(AppSet::Input),
                )
                    .run_if(in_state(GameState::Fighting))
                    .run_if(resource_equals(GameMode::Training)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
                let button = Button::new(GameMode::ALL[at].label(), MenuAction::Play)
                    .width(widest_label());
                caixa(button_art(&button).size(), mode_slot(at))
            })
            .collect();
        caixas.push(caixa(
            button_art(&Button::new(level_name(0), MenuAction::Stage(1)).width(widest_stage()))
                .size(),
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

        assert!(rect.contains(Vec2::new(430.0, 300.0)), "o centro nao acerta");
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
}
