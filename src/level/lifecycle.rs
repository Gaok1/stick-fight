/// Fase destacada no menu.
///
/// O gameplay so conhece [`CurrentLevel`]; o indice existe apenas para o menu
/// poder girar a lista, e um sistema mantem os dois em sincronia.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct LevelPick(pub usize);

/// Qual fase a geometria que esta no ar representa.
///
/// Sem isso nao ha como perguntar "a arena montada ainda e a fase escolhida?",
/// e a resposta importa: online a fase muda por pacote, no meio de qualquer
/// quadro, e nao so na porta de entrada do estado.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltStage(pub Option<usize>);

/// Traduz o indice escolhido no `CurrentLevel` que o resto do jogo le.
///
/// Roda no `PreUpdate`, logo depois dos pacotes, e nao no `Update`: a troca de
/// estado do Bevy acontece entre os dois. Enquanto isto rodava depois, o
/// cliente que recebia o pacote de inicio entrava na luta com o mapa **antigo**
/// -- o pacote dizia a fase, a transicao levantava a geometria no mesmo quadro,
/// e a fase nova so chegava ao `CurrentLevel` um quadro tarde demais. Era esse
/// o "mapa que nao atualiza" de quem entrava na sala.
fn apply_level_pick(pick: Res<LevelPick>, mut current: ResMut<CurrentLevel>) {
    if pick.is_changed() {
        current.0 = level_at(pick.0);
    }
}

/// Sorteia uma fase diferente da atual.
///
/// Diferente de proposito: repetir o mapa que acabou de ser jogado le como
/// sorteio quebrado, mesmo sendo um resultado legitimo.
pub fn roll_stage(current: usize) -> usize {
    if CATALOG.len() < 2 {
        return current;
    }
    let mut next = fastrand::usize(..CATALOG.len() - 1);
    if next >= current % CATALOG.len() {
        next += 1;
    }
    next
}

/// Gira a fase depois de cada round.
///
/// Quem decide o round tambem decide o mapa seguinte: online a escolha viaja no
/// pacote de inicio, entao os clientes seguem sem precisar sortear nada -- dois
/// sorteios independentes dariam dois mapas.
fn rotate_stage(mut pick: ResMut<LevelPick>) {
    pick.0 = roll_stage(pick.0);
}

/// Monta a fase ao entrar em `Fighting`.
///
/// Um unico lugar traduz peca em entidade. As fases nao spawnam nada por conta
/// propria, entao nao existe mapa com regra de colisao ou de camada diferente
/// dos outros.
fn build_level(
    mut commands: Commands,
    level: Res<CurrentLevel>,
    pick: Res<LevelPick>,
    mut built: ResMut<BuiltStage>,
) {
    built.0 = Some(pick.0);
    raise_level(&mut commands, &level);
}

/// Reergue a arena quando a fase muda com ela ja de pe.
///
/// A porta de entrada do estado nao basta: online a fase chega por pacote e
/// pode trocar com a sala aberta -- e quem estivesse no aquecimento continuaria
/// correndo na geometria da fase anterior, atravessando chao que so existe na
/// tela dele.
fn rebuild_on_stage_change(
    mut commands: Commands,
    mut level: ResMut<CurrentLevel>,
    pick: Res<LevelPick>,
    mut built: ResMut<BuiltStage>,
    geometry: Query<Entity, With<LevelGeometry>>,
) {
    if built.0 == Some(pick.0) {
        return;
    }
    // A fase e relida aqui, e nao herdada de `apply_level_pick`: quem troca o
    // mapa com a sala aberta -- o dono pelo menu, o cliente por `stage` do
    // lobby -- escreve o indice no `Update`, e a traducao para `CurrentLevel`
    // so acontece no `PreUpdate` do quadro seguinte. Sem esta linha a arena
    // subia com a geometria da fase **anterior** e `built` ja ficava marcado
    // como a nova, entao nada corrigia depois: o nome na tela dizia um mapa e o
    // chao era de outro.
    level.0 = level_at(pick.0);
    for entity in &geometry {
        commands.entity(entity).despawn();
    }
    built.0 = Some(pick.0);
    raise_level(&mut commands, &level);
}

fn raise_level(commands: &mut Commands, level: &CurrentLevel) {
    crate::backdrop::build(
        commands,
        level.0.skyline(),
        level.0.signs(),
        level.0.scene(),
    );

    let mut next_chain = 0u8;
    for piece in level.0.pieces() {
        match *piece {
            Piece::Terrain { top, cols, rows } => {
                let height = rows as f32 * CELL.y;
                terrain(commands, Vec2::new(top.x, top.y - height * 0.5), cols, rows);
            }
            Piece::Ceiling { bottom, cols, rows } => {
                let height = rows as f32 * CELL.y;
                terrain(
                    commands,
                    Vec2::new(bottom.x, bottom.y + height * 0.5),
                    cols,
                    rows,
                );
            }
            Piece::Platform { at, cols } => platform(commands, at, cols),
            Piece::Chain { top, links } => {
                chain(commands, next_chain, top, links);
                next_chain += 1;
            }
            Piece::Hazard { at, cols, kind } => hazard(commands, at, cols, kind),
            Piece::Geyser {
                at,
                cols,
                rows,
                period,
                phase,
                kind,
            } => geyser(commands, at, cols, rows, period, phase, kind),
            Piece::Tide {
                at,
                cols,
                rise,
                period,
                phase,
                kind,
            } => tide(commands, at, cols, rise, period, phase, kind),
            Piece::Drip {
                from,
                cols,
                floor,
                period,
                phase,
                kind,
            } => spout(commands, from, cols, floor, period, phase, kind),
        }
    }
}

/// Limpa a geometria ao sair de `Fighting`.
fn clear_level(
    mut commands: Commands,
    mut built: ResMut<BuiltStage>,
    q: Query<Entity, With<LevelGeometry>>,
) {
    built.0 = None;
    for entity in &q {
        commands.entity(entity).despawn();
    }
}

/// Alcance horizontal de um pulo que sobe `rise`, ou `None` se for alto demais.
///
/// E a solucao do lancamento obliquo com os mesmos numeros que a fisica usa em
/// jogo, entao mexer em `JUMP_SPEED` ou `GRAVITY` reprova mapas que passaram a
/// nao fechar -- que e exatamente o aviso que se quer.
#[cfg(test)]
fn jump_reach(rise: f32) -> Option<f32> {
    use crate::actor::motion::{JUMP_SPEED, RUN_SPEED};
    use crate::physics::GRAVITY;

    let discriminant = JUMP_SPEED * JUMP_SPEED - 2.0 * GRAVITY * rise;
    if discriminant < 0.0 {
        return None;
    }
    let airtime = (JUMP_SPEED + discriminant.sqrt()) / GRAVITY;
    Some(RUN_SPEED * airtime)
}

