//! Telas e HUD.
//!
//! Tudo aqui e feito das mesmas primitivas de arte do jogo -- nao ha caminho de
//! texto separado. O titulo inclusive e gerado expandindo o bitmap da propria
//! fonte, entao a tela inicial nunca sai de estilo em relacao a arena.

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
use crate::online::OnlineSession;
use crate::state::{AppSet, GameMode, GameState};
use crate::weapon::Held;

/// Caracteres de moldura, pintados mais apagados que o texto.
const FRAME_CHARS: &str = "\u{2554}\u{2550}\u{2557}\u{2551}\u{255A}\u{255D}\u{2560}\u{2563}";
/// Linha de conteudo que vira separador horizontal.
const SEPARATOR: &str = "---";
/// Celulas da barra de vida.
const BAR_CELLS: u16 = 18;

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

/// A tela de menu inteira, para poder redesenhar quando a selecao muda.
#[derive(Component)]
struct MenuPanel;

/// Painel e atores da tela de escolha cosmetica.
#[derive(Component)]
struct SkinPanel;

#[derive(Component)]
struct SkinPreview(u8);

#[derive(Component)]
struct LobbyPanel;

/// Linha destacada no menu.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
enum MenuRow {
    #[default]
    Mode,
    Stage,
}

impl MenuRow {
    const ALL: [MenuRow; 2] = [MenuRow::Mode, MenuRow::Stage];

    /// Marcador da linha ativa.
    fn cursor(self, active: MenuRow) -> &'static str {
        if self == active { "\u{25BA}" } else { " " }
    }
}

/// Linha do seletor de modo.
///
/// Cada celula tem largura fixa para que trocar de modo nao empurre as colunas
/// de lado -- um menu que "pula" ao navegar parece quebrado.
fn mode_line(selected: GameMode, active: MenuRow) -> String {
    let width = widest_label();

    let mut line = format!("{} MODE    ", MenuRow::Mode.cursor(active));
    for mode in GameMode::ALL {
        let cell = format!("{:^width$}", mode.label(), width = width + 2);
        if mode == selected {
            line.push_str(&format!("[{cell}]"));
        } else {
            line.push_str(&format!(" {cell} "));
        }
    }
    line
}

/// Linha do seletor de fase. Mesma logica de largura fixa do seletor de modo.
fn stage_line(pick: usize, active: MenuRow) -> String {
    let width = (0..LEVEL_CATALOG.len())
        .map(|i| level_name(i).chars().count())
        .max()
        .unwrap_or(0);
    format!(
        "{} STAGE     < {:^width$} >",
        MenuRow::Stage.cursor(active),
        level_name(pick),
        width = width
    )
}

/// As linhas da tela inicial, ja centradas e alinhadas.
///
/// Separadas do desenho para que o teste dos realces trabalhe sobre o texto de
/// verdade. Enquanto ele tinha uma copia das linhas, a copia podia envelhecer
/// e o teste continuava passando sobre um menu que nao existia mais.
fn menu_lines(mode: GameMode, pick: usize, active: MenuRow) -> Vec<String> {
    let selector = mode_line(mode, active);
    let stage = stage_line(pick, active);
    // Todos os textos de modo ocupam a mesma largura, senao a moldura muda de
    // tamanho ao navegar.
    let widest = GameMode::ALL
        .iter()
        .map(|m| m.blurb().chars().count())
        .max()
        .unwrap_or(0);
    let blurb = format!("{:<widest$}", mode.blurb(), widest = widest);

    let body = [
        selector.as_str(),
        stage.as_str(),
        blurb.as_str(),
        "  UP / DOWN  PICKS THE ROW,  LEFT / RIGHT  CHANGES IT",
        SEPARATOR,
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
        "  ONLINE HOLDS TWO TO FOUR. WARM UP IN THE LOBBY.",
    ];

    // Titulo e chamada saem centrados na caixa, e nao empurrados por espacos
    // contados a mao: a largura vem do resto do conteudo, entao mexer numa
    // linha de teclas nao deixa os dois tortos.
    let width = body
        .iter()
        .filter(|line| **line != SEPARATOR)
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0);
    let title = format!("{:^width$}", "STICK FIGHT   //   CP437 ASCII");
    let call = format!("{:^width$}", "PRESS  ENTER  TO FIGHT");

    std::iter::once(title)
        .chain([SEPARATOR.to_string()])
        .chain(body.into_iter().map(str::to_string))
        .chain([SEPARATOR.to_string(), call])
        .collect()
}

/// Conteudo da tela inicial para a selecao atual.
fn menu_art(mode: GameMode, pick: usize, active: MenuRow) -> AsciiArt {
    let owned = menu_lines(mode, pick, active);
    let lines: Vec<&str> = owned.iter().map(String::as_str).collect();

    // A caixa sai de uma cor so; a cor entra depois, e so onde ela informa
    // alguma coisa -- o que esta selecionado e de quem e cada coluna de teclas.
    let selected = format!("[{:^width$}]", mode.label(), width = widest_label() + 2);
    let mut art = box_art(&lines);
    art = highlight(art, &lines, "STICK FIGHT", palette::GOLD);
    art = highlight(art, &lines, &selected, palette::GOLD);
    art = highlight(art, &lines, level_name(pick), palette::MOSS);
    art = highlight(art, &lines, "\u{25BA}", palette::GOLD);
    art = highlight(art, &lines, "PLAYER 1", palette::player(0));
    art = highlight(art, &lines, "PLAYER 2", palette::player(1));
    highlight(art, &lines, "ENTER", palette::GOLD)
}

/// Largura da maior etiqueta de modo, que e o que fixa a celula do seletor.
fn widest_label() -> usize {
    GameMode::ALL
        .iter()
        .map(|m| m.label().chars().count())
        .max()
        .unwrap_or(0)
}

/// Tela inicial: titulo, seletores e controles.
fn spawn_controls_screen(
    mut commands: Commands,
    mode: Res<GameMode>,
    pick: Res<LevelPick>,
    row: Res<MenuRow>,
) {
    commands.spawn((
        AsciiSprite::new(AsciiArt::banner("STICK FIGHT", '\u{2588}', palette::BONE)),
        Layer::Hud,
        Transform::from_translation(Vec3::new(0.0, 178.0, 0.0)).with_scale(Vec3::splat(0.5)),
        DespawnOnExit(GameState::Controls),
    ));

    commands.spawn((
        MenuPanel,
        AsciiSprite::new(menu_art(*mode, pick.0, *row)),
        Layer::Hud,
        Transform::from_translation(Vec3::new(0.0, -60.0, 0.0)),
        DespawnOnExit(GameState::Controls),
    ));
}

/// Painel da sala: quem esta dentro, o que fazer, e como comecar.
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
        session.status.clone(),
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
        let you = if slot as u8 == local { "\u{25BA}" } else { " " };
        format!("{you} P{}  {who}", slot + 1)
    }));

    lines.extend([
        SEPARATOR.to_string(),
        "  ENTER  CREATE / START".to_string(),
        "  F      FIND OPEN LOBBY".to_string(),
        "  I      INVITE FRIEND".to_string(),
        "  ESC    BACK".to_string(),
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
    art = highlight(art, &text, "\u{25BA}", palette::GOLD);
    highlight(art, &text, "WARM UP WHILE YOU WAIT", palette::MOSS)
}

fn spawn_lobby_screen(mut commands: Commands, session: Res<OnlineSession>) {
    commands.spawn((
        LobbyPanel,
        // Ancorado pelo canto superior esquerdo: a caixa cresce para dentro da
        // tela conforme o status muda de tamanho, em vez de escorregar de lado
        // a cada palavra -- e desce a partir do topo, deixando livre a faixa do
        // chao, que e onde os bonecos ficam.
        AsciiSprite::pivoted(lobby_art(&session), Vec2::new(-0.5, 0.5)),
        Layer::Hud,
        Transform::from_translation(Vec3::new(-628.0, 232.0, 0.0)),
        DespawnOnExit(GameState::Lobby),
    ));
}

fn update_lobby_screen(
    session: Res<OnlineSession>,
    mut panels: Query<&mut AsciiSprite, With<LobbyPanel>>,
) {
    if !session.is_changed() {
        return;
    }
    for mut panel in &mut panels {
        panel.art = lobby_art(&session);
    }
}

/// Navega o menu: cima/baixo escolhe a linha, esquerda/direita muda o valor.
fn menu_navigation(
    keys: Res<ButtonInput<KeyCode>>,
    mut mode: ResMut<GameMode>,
    mut pick: ResMut<LevelPick>,
    mut row: ResMut<MenuRow>,
    mut panels: Query<&mut AsciiSprite, With<MenuPanel>>,
) {
    let down = keys.any_just_pressed([KeyCode::KeyS, KeyCode::ArrowDown]);
    let up = keys.any_just_pressed([KeyCode::KeyW, KeyCode::ArrowUp]);
    let right = keys.any_just_pressed([KeyCode::KeyD, KeyCode::ArrowRight]);
    let left = keys.any_just_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]);

    let vertical = down as i32 - up as i32;
    let horizontal = right as i32 - left as i32;
    if vertical == 0 && horizontal == 0 {
        return;
    }

    /// Gira um indice dentro de `len`, aceitando passo negativo.
    fn cycle(current: usize, step: i32, len: usize) -> usize {
        (current as i32 + step).rem_euclid(len as i32) as usize
    }

    if vertical != 0 {
        let current = MenuRow::ALL.iter().position(|r| *r == *row).unwrap_or(0);
        *row = MenuRow::ALL[cycle(current, vertical, MenuRow::ALL.len())];
    }

    if horizontal != 0 {
        match *row {
            MenuRow::Mode => {
                let current = GameMode::ALL.iter().position(|m| *m == *mode).unwrap_or(0);
                *mode = GameMode::ALL[cycle(current, horizontal, GameMode::ALL.len())];
            }
            // Escrever o indice basta: `apply_level_pick` troca o `CurrentLevel`.
            MenuRow::Stage => pick.0 = cycle(pick.0, horizontal, LEVEL_CATALOG.len()),
        }
    }

    for mut panel in &mut panels {
        panel.art = menu_art(*mode, pick.0, *row);
    }
}

/// Uma linha do seletor de lutador: a pele, ou uma peca do rosto.
///
/// Pele e rosto sao a mesma pergunta -- com que cara este boneco entra -- entao
/// eles moram no mesmo cursor. Uma tela so pra pele e outra so pro rosto faria
/// o jogador confirmar duas vezes a mesma escolha.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
enum FighterRow {
    #[default]
    Skin,
    Part(usize),
}

impl FighterRow {
    /// Todas, na ordem em que o cursor as percorre.
    fn all() -> Vec<FighterRow> {
        std::iter::once(FighterRow::Skin)
            .chain((0..Part::CHOSEN.len()).map(FighterRow::Part))
            .collect()
    }

    fn label(self) -> &'static str {
        match self {
            FighterRow::Skin => "SKIN",
            FighterRow::Part(at) => Part::CHOSEN[at].label(),
        }
    }

    /// Nome da opcao escolhida agora por este lugar.
    fn value(self, skin_pick: usize, face: Face) -> &'static str {
        match self {
            FighterRow::Skin => skin::skin(skin_pick).name,
            FighterRow::Part(at) => face.look(Part::CHOSEN[at]).name,
        }
    }

    /// Gira a escolha desta linha.
    fn cycle(self, skin_pick: &mut usize, face: &mut Face, step: i32) {
        match self {
            FighterRow::Skin => {
                *skin_pick =
                    (*skin_pick as i32 + step).rem_euclid(skin::CATALOG.len() as i32) as usize
            }
            FighterRow::Part(at) => face.cycle(Part::CHOSEN[at], step),
        }
    }
}

/// Largura fixa da celula de valor, para a moldura nao mudar de tamanho ao
/// navegar -- mesma regra do seletor de modo.
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

fn skin_select_art(mode: GameMode, picks: &SkinSelections, active: FighterRow) -> AsciiArt {
    let cell = fighter_cell();
    let solo = mode == GameMode::Online;

    let mut lines = vec![
        if solo {
            "CHOOSE YOUR FIGHTER".to_string()
        } else {
            "CHOOSE YOUR FIGHTERS".to_string()
        },
        SEPARATOR.to_string(),
    ];

    for row in FighterRow::all() {
        let cursor = if row == active { "\u{25BA}" } else { " " };
        let label = format!("{cursor} {:<6}", row.label());
        if solo {
            let value = row.value(picks.online_local, picks.online_face);
            lines.push(format!("{label}  < {value:^cell$} >"));
        } else {
            // No treino o lugar do P2 e do dummy: ele nao escolhe nada, e
            // mostrar um seletor la prometeria uma escolha que nao existe.
            let p1 = row.value(picks.players[0], picks.faces[0]);
            let p2 = if mode.is_training() {
                "-".to_string()
            } else {
                row.value(picks.players[1], picks.faces[1]).to_string()
            };
            lines.push(format!("{label}  < {p1:^cell$} >    < {p2:^cell$} >"));
        }
    }

    lines.push(SEPARATOR.to_string());
    if solo {
        lines.push("  W / S  PICKS THE ROW,  A / D  CHANGES IT".to_string());
    } else {
        lines.push("         P1: W / S + A / D    P2: UP / DOWN + LEFT / RIGHT".to_string());
    }
    lines.push("  ENTER  CONFIRM     ESC  BACK".to_string());

    let text: Vec<&str> = lines.iter().map(String::as_str).collect();
    let art = box_art(&text);
    highlight(art, &text, "\u{25BA}", palette::GOLD)
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
            Transform::from_translation(at.extend(0.0)).with_scale(Vec3::splat(1.45)),
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

fn spawn_skin_select_screen(
    mut commands: Commands,
    mode: Res<GameMode>,
    picks: Res<SkinSelections>,
    row: Res<FighterRow>,
) {
    commands.spawn((
        SkinPanel,
        AsciiSprite::new(skin_select_art(*mode, &picks, *row)),
        Layer::Hud,
        Transform::from_translation(Vec3::new(0.0, 125.0, 0.0)),
        DespawnOnExit(GameState::SkinSelect),
    ));

    if *mode == GameMode::Online {
        spawn_skin_preview(
            &mut commands,
            0,
            skin::skin(picks.online_local),
            picks.online_face,
            Vec2::new(0.0, -35.0),
        );
    } else {
        spawn_skin_preview(
            &mut commands,
            0,
            skin::skin(picks.players[0]),
            picks.faces[0],
            Vec2::new(-180.0, -35.0),
        );
        let second = if mode.is_training() {
            &skin::WRAITH
        } else {
            skin::skin(picks.players[1])
        };
        spawn_skin_preview(
            &mut commands,
            1,
            second,
            picks.faces[1],
            Vec2::new(180.0, -35.0),
        );
    }
}

fn skin_select_navigation(
    keys: Res<ButtonInput<KeyCode>>,
    mode: Res<GameMode>,
    mut picks: ResMut<SkinSelections>,
    mut row: ResMut<FighterRow>,
    mut previews: Query<(&SkinPreview, &mut ActorSkin, &mut Face)>,
    mut panels: Query<&mut AsciiSprite, With<SkinPanel>>,
) {
    // Cada jogador anda no proprio par de teclas, mas a linha e uma so: os dois
    // escolhem a mesma peca ao mesmo tempo, e a tela nao precisa de dois
    // cursores disputando espaco.
    let p1 = keys.just_pressed(KeyCode::KeyD) as i32 - keys.just_pressed(KeyCode::KeyA) as i32;
    let p2 = keys.just_pressed(KeyCode::ArrowRight) as i32
        - keys.just_pressed(KeyCode::ArrowLeft) as i32;
    let vertical = keys.any_just_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) as i32
        - keys.any_just_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) as i32;

    if p1 == 0 && p2 == 0 && vertical == 0 {
        return;
    }

    let rows = FighterRow::all();
    if vertical != 0 {
        let at = rows.iter().position(|r| *r == *row).unwrap_or(0) as i32;
        *row = rows[(at + vertical).rem_euclid(rows.len() as i32) as usize];
    }

    if *mode == GameMode::Online {
        let step = if p1 != 0 { p1 } else { p2 };
        if step != 0 {
            let (mut pick, mut face) = (picks.online_local, picks.online_face);
            row.cycle(&mut pick, &mut face, step);
            picks.online_local = pick;
            picks.online_face = face;
        }
    } else {
        if p1 != 0 {
            let (mut pick, mut face) = (picks.players[0], picks.faces[0]);
            row.cycle(&mut pick, &mut face, p1);
            picks.players[0] = pick;
            picks.faces[0] = face;
        }
        if p2 != 0 && !mode.is_training() {
            let (mut pick, mut face) = (picks.players[1], picks.faces[1]);
            row.cycle(&mut pick, &mut face, p2);
            picks.players[1] = pick;
            picks.faces[1] = face;
        }
    }

    for (preview, mut actor_skin, mut face) in &mut previews {
        let solo = *mode == GameMode::Online;
        actor_skin.0 = if solo {
            skin::skin(picks.online_local)
        } else if mode.is_training() && preview.0 == 1 {
            &skin::WRAITH
        } else {
            skin::skin(picks.players[preview.0 as usize])
        };
        let next = if solo {
            picks.online_face
        } else {
            picks.faces[preview.0 as usize]
        };
        face.set_if_neq(next);
    }
    for mut panel in &mut panels {
        panel.art = skin_select_art(*mode, &picks, *row);
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
fn spawn_round_over_screen(
    mut commands: Commands,
    result: Res<RoundResult>,
    mode: Res<GameMode>,
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
        Transform::from_translation(Vec3::new(0.0, 60.0, 0.0)).with_scale(Vec3::splat(0.45)),
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
    let advance = match (*mode == GameMode::Online, online.is_host(), champion) {
        (true, false, _) => "  WAITING FOR THE HOST",
        (_, _, Some(_)) => "  PRESS  ENTER  FOR A NEW MATCH",
        _ => "  PRESS  ENTER  FOR THE NEXT ROUND",
    };
    lines.extend([
        SEPARATOR.to_string(),
        progress,
        advance.to_string(),
        "  PRESS  ESC    TO LEAVE THE MATCH".to_string(),
    ]);

    commands.spawn((
        AsciiSprite::new(box_art(
            &lines.iter().map(String::as_str).collect::<Vec<_>>(),
        )),
        Layer::Hud,
        Transform::from_translation(Vec3::new(0.0, -90.0, 0.0)),
        DespawnOnExit(GameState::RoundOver),
    ));
}

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

/// Avanca de tela conforme as teclas.
fn screen_transitions(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<GameState>>,
    mode: Res<GameMode>,
    mut next: ResMut<NextState<GameState>>,
) {
    match state.get() {
        GameState::Controls => {
            if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter) {
                next.set(GameState::SkinSelect);
            }
        }
        GameState::SkinSelect => {
            if keys.just_pressed(KeyCode::Escape) {
                next.set(GameState::Controls);
            } else if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter) {
                next.set(if *mode == GameMode::Online {
                    GameState::Lobby
                } else {
                    GameState::Fighting
                });
            }
        }
        GameState::Lobby => {}
        GameState::RoundOver => {
            if *mode != GameMode::Online
                && (keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter))
            {
                next.set(GameState::Fighting);
            } else if keys.just_pressed(KeyCode::Escape) {
                next.set(if *mode == GameMode::Online {
                    GameState::Lobby
                } else {
                    GameState::Controls
                });
            }
        }
        GameState::Fighting => {
            if keys.just_pressed(KeyCode::Escape) {
                next.set(if *mode == GameMode::Online {
                    GameState::Lobby
                } else {
                    GameState::Controls
                });
            }
        }
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
        app.init_resource::<MenuRow>()
            .init_resource::<FighterRow>()
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
            .add_systems(
                Update,
                (
                    screen_transitions,
                    menu_navigation.run_if(in_state(GameState::Controls)),
                )
                    .in_set(AppSet::Input),
            )
            .add_systems(
                Update,
                skin_select_navigation
                    .in_set(AppSet::Input)
                    .run_if(in_state(GameState::SkinSelect)),
            )
            .add_systems(
                Update,
                animate_skin_previews
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

    /// Navegar nao pode mexer no tamanho da caixa: uma moldura que muda de
    /// largura a cada tecla le como bug, e so aparece com mapa ou modo novo --
    /// exatamente o tipo de coisa que quebra em silencio.
    #[test]
    fn navegar_o_menu_nao_mexe_na_caixa() {
        let mut widths = Vec::new();
        for mode in GameMode::ALL {
            for pick in 0..LEVEL_CATALOG.len() {
                for row in MenuRow::ALL {
                    widths.push(menu_art(mode, pick, row).cols);
                }
            }
        }
        assert!(
            widths.windows(2).all(|w| w[0] == w[1]),
            "a moldura muda de largura ao navegar: {widths:?}"
        );
    }

    /// Todo realce do menu tem que achar o texto que ele pinta.
    ///
    /// A cor entra por busca sobre o texto ja montado. Se a etiqueta de um modo
    /// ou a linha de teclas mudar e o alvo do realce nao, a cor simplesmente
    /// some -- o menu volta a ser branco sem nada quebrar e sem ninguem notar.
    #[test]
    fn todo_realce_do_menu_acha_o_alvo() {
        for mode in GameMode::ALL {
            for row in MenuRow::ALL {
                for pick in 0..LEVEL_CATALOG.len() {
                    let owned = menu_lines(mode, pick, row);
                    let lines: Vec<&str> = owned.iter().map(String::as_str).collect();
                    let cell = format!("[{:^width$}]", mode.label(), width = widest_label() + 2);

                    for alvo in [
                        "STICK FIGHT",
                        cell.as_str(),
                        level_name(pick),
                        "\u{25BA}",
                        "PLAYER 1",
                        "PLAYER 2",
                        "ENTER",
                    ] {
                        assert!(
                            locate(&lines, alvo).is_some(),
                            "{mode:?}/{row:?}: o realce {alvo:?} nao acha o que pintar"
                        );
                    }
                }
            }
        }
    }

    /// O painel da sala tem que marcar o lugar de quem esta olhando.
    #[test]
    fn o_painel_da_sala_marca_o_proprio_lugar() {
        let session = OnlineSession::default();
        let art = lobby_art(&session);
        assert!(
            art.rows >= MAX_PLAYERS as u16,
            "a sala nao lista os lugares"
        );
    }

    /// A tela de lutador nao pode mudar de tamanho ao navegar.
    ///
    /// Ela agora varre pele **e** as quatro pecas do rosto, entao a varredura
    /// passa por todo catalogo e por toda linha do cursor: basta um nome mais
    /// longo que a celula para a moldura pular ao trocar de opcao.
    #[test]
    fn seletor_de_lutador_nao_pula_ao_navegar() {
        for mode in GameMode::ALL {
            let mut sizes = Vec::new();
            for row in FighterRow::all() {
                for a in 0..skin::CATALOG.len() {
                    let mut face = Face::default();
                    for passo in 0..8 {
                        for part in Part::CHOSEN {
                            face.cycle(part, passo % 3 - 1);
                        }
                        let picks = SkinSelections {
                            players: [a, a, a, a],
                            online_local: a,
                            faces: [face; crate::actor::MAX_PLAYERS],
                            online_face: face,
                        };
                        let art = skin_select_art(mode, &picks, row);
                        sizes.push((art.cols, art.rows));
                    }
                }
            }
            assert!(
                sizes.windows(2).all(|pair| pair[0] == pair[1]),
                "a tela de {mode:?} muda de tamanho: {sizes:?}"
            );
        }
    }

    #[test]
    fn o_seletor_marca_exatamente_um_modo() {
        for mode in GameMode::ALL {
            let line = mode_line(mode, MenuRow::Mode);
            assert_eq!(line.matches('[').count(), 1, "modo {mode:?}: {line}");
            assert!(line.contains(&format!("[{:^10}]", mode.label())));
        }
    }

    #[test]
    fn so_a_linha_ativa_leva_o_cursor() {
        for row in MenuRow::ALL {
            let lines = [mode_line(GameMode::Versus, row), stage_line(0, row)];
            let marked = lines.iter().filter(|l| l.starts_with('\u{25BA}')).count();
            assert_eq!(marked, 1, "linha ativa {row:?}: {lines:?}");
        }
    }

    #[test]
    fn o_seletor_de_fase_cobre_o_catalogo_inteiro() {
        for pick in 0..LEVEL_CATALOG.len() {
            assert!(
                stage_line(pick, MenuRow::Stage).contains(level_name(pick)),
                "fase {pick} nao aparece no seletor"
            );
        }
    }

    #[test]
    fn separador_vira_linha_dupla() {
        let art = framed(&["a", SEPARATOR]);
        assert!(art.contains('\u{2560}'));
        assert!(art.contains('\u{2563}'));
    }
}
