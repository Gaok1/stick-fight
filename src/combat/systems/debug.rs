/// Contorno de caixa desenhado no treino.
#[derive(Component)]
struct DebugBox;

/// Ligado ou desligado pelo `H` durante o treino.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShowBoxes(pub bool);

/// Moldura vazada de `cols` x `rows`, em linha simples.
fn outline(cols: u16, rows: u16) -> String {
    let cols = cols.max(2) as usize;
    let rows = rows.max(2) as usize;
    let mut out = String::new();
    for row in 0..rows {
        for col in 0..cols {
            let borda_h = row == 0 || row == rows - 1;
            let borda_v = col == 0 || col == cols - 1;
            out.push(match (borda_h, borda_v) {
                (true, true) => '\u{253C}',
                (true, false) => '\u{2500}',
                (false, true) => '\u{2502}',
                // Vazado: a moldura nao pode esconder o boneco que ela mede.
                (false, false) => ' ',
            });
        }
        if row + 1 < rows {
            out.push('\n');
        }
    }
    out
}

/// Desenha as caixas de dano e de dano-recebido.
///
/// Existe porque quase todo erro de alcance que apareceu neste jogo foi achado
/// por aritmetica, nao por olhar a tela: dummy flutuando fora de alcance, arma
/// descolada da mao, golpe abrindo na altura errada. Com as caixas a vista,
/// esses erros viram obvios em vez de invisiveis.
///
/// Redesenha do zero a cada quadro: sao poucas caixas, e diferenciar custaria
/// mais que refazer.
fn draw_debug_boxes(
    mut commands: Commands,
    show: Res<ShowBoxes>,
    antigas: Query<Entity, With<DebugBox>>,
    hitboxes: Query<(&Transform, &Collider), With<Hitbox>>,
    corpos: Query<
        (&Transform, &Collider, Option<&Hurtbox>),
        Or<(With<Player>, With<TrainingDummy>)>,
    >,
) {
    for antiga in &antigas {
        commands.entity(antiga).despawn();
    }
    if !show.0 {
        return;
    }

    let mut desenhar = |centro: Vec2, half: Vec2, cor: Color| {
        let cols = (half.x * 2.0 / crate::ascii::CELL.x).round() as u16;
        let rows = (half.y * 2.0 / crate::ascii::CELL.y).round() as u16;
        commands.spawn((
            DebugBox,
            AsciiSprite::new(AsciiArt::solid(&outline(cols, rows), cor)),
            Layer::Fx,
            Transform::from_translation(centro.extend(0.0)),
            DespawnOnExit(GameState::Fighting),
        ));
    };

    for (transform, collider) in &hitboxes {
        desenhar(
            transform.translation.truncate(),
            collider.half,
            palette::BLOOD,
        );
    }
    for (transform, collider, hurtbox) in &corpos {
        let at = transform.translation.truncate();
        match hurtbox {
            Some(hurtbox) => desenhar(at + hurtbox.offset, hurtbox.half, palette::MOSS),
            None => desenhar(at, collider.half, palette::MOSS),
        }
    }
}

