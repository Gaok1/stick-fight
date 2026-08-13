//! Ponte entre arte ASCII e o renderer do Bevy.
//!
//! Cada celula acesa vira uma entidade `Sprite` filha, indexando o atlas CP437.
//! Isso da posicao, rotacao, escala e tint por glifo de graca, com batching de
//! GPU, e mantem o pipeline de layout de texto fora do caminho. O grid so
//! existe na hora de autorar a arte -- o `Transform` do pai e float livre.

use bevy::prelude::*;
use bevy::sprite::Anchor;

use super::art::AsciiArt;
use super::cp437::{CELL, GlyphAtlas};

/// Camada de profundidade. Vira `translation.z` na hora do spawn.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Layer {
    /// Parallax e decoracao ao fundo.
    #[allow(dead_code)]
    Background,
    /// Chao, plataformas, correntes.
    #[default]
    Terrain,
    /// Armas caidas.
    Pickup,
    /// Bonecos.
    Actor,
    /// Projeteis.
    Projectile,
    /// Efeitos acima do ASCII.
    Fx,
    /// HUD e telas.
    Hud,
}

impl Layer {
    /// Z correspondente. O espacamento de 10 deixa espaco para sub-ordenacao.
    pub fn z(self) -> f32 {
        10.0 * match self {
            Layer::Background => 0.0,
            Layer::Terrain => 1.0,
            Layer::Pickup => 2.0,
            Layer::Actor => 3.0,
            Layer::Projectile => 4.0,
            Layer::Fx => 5.0,
            Layer::Hud => 6.0,
        }
    }
}

/// Uma arte ASCII posicionada no mundo.
///
/// Mudar `art`, `pivot` ou `flip_x` reconstroi os glifos filhos no mesmo frame.
#[derive(Component, Clone, Debug)]
#[require(Transform, Visibility, Layer)]
pub struct AsciiSprite {
    /// A arte a desenhar.
    pub art: AsciiArt,
    /// Ancora normalizada: `(0,0)` centro, `(0,-0.5)` base, `(-0.5,0.5)` topo-esq.
    pub pivot: Vec2,
    /// Espelha horizontalmente na hora de desenhar.
    pub flip_x: bool,
}

impl AsciiSprite {
    /// Sprite ancorado no centro.
    pub fn new(art: AsciiArt) -> Self {
        Self {
            art,
            pivot: Vec2::ZERO,
            flip_x: false,
        }
    }

    /// Sprite ancorado na base -- o normal para qualquer coisa que pisa no chao.
    pub fn footed(art: AsciiArt) -> Self {
        Self {
            pivot: Vec2::new(0.0, -0.5),
            ..Self::new(art)
        }
    }

    /// Sprite com ancora arbitraria -- HUD encostado numa borda, por exemplo.
    pub fn pivoted(art: AsciiArt, pivot: Vec2) -> Self {
        Self {
            pivot,
            ..Self::new(art)
        }
    }

    /// Versao espelhada.
    pub fn flipped(mut self, flip: bool) -> Self {
        self.flip_x = flip;
        self
    }
}

/// Marca as entidades-glifo geradas, para poder limpa-las sem tocar em outros
/// filhos que o gameplay tenha pendurado na mesma entidade.
#[derive(Component)]
pub struct Glyph;

/// Reconstroi os glifos de todo `AsciiSprite` que mudou neste frame.
///
/// Refaz tudo em vez de fazer diff: as artes tem dezenas de celulas, nao
/// milhares, e o custo de um diff correto passaria o custo do respawn.
pub fn rebuild_glyphs(
    mut commands: Commands,
    atlas: Res<GlyphAtlas>,
    changed: Query<(Entity, &AsciiSprite, Option<&Children>), Changed<AsciiSprite>>,
    glyphs: Query<Entity, With<Glyph>>,
) {
    for (entity, sprite, children) in &changed {
        if let Some(children) = children {
            for child in children.iter() {
                if glyphs.contains(child) {
                    commands.entity(child).despawn();
                }
            }
        }

        let art = if sprite.flip_x {
            sprite.art.mirrored()
        } else {
            sprite.art.clone()
        };

        let size = art.size();
        // Canto superior esquerdo da arte, relativo a origem da entidade.
        let origin = Vec2::new(
            -size.x * (0.5 + sprite.pivot.x),
            size.y * (0.5 - sprite.pivot.y),
        );

        for cell in &art.cells {
            let offset = origin
                + Vec2::new(
                    (cell.col as f32 + 0.5) * CELL.x,
                    -(cell.row as f32 + 0.5) * CELL.y,
                );

            commands.spawn((
                Glyph,
                Sprite {
                    image: atlas.image.clone(),
                    texture_atlas: Some(TextureAtlas {
                        layout: atlas.layout.clone(),
                        index: cell.glyph as usize,
                    }),
                    color: cell.color,
                    ..default()
                },
                Anchor::CENTER,
                Transform::from_translation(offset.extend(0.0)),
                ChildOf(entity),
            ));
        }
    }
}

/// Aplica o `Layer` de cada sprite no eixo Z assim que ele muda.
///
/// Fica separado do rebuild porque camada e posicao mudam por motivos
/// diferentes: a arte anima todo frame, a camada quase nunca.
pub fn apply_layer_z(mut q: Query<(&Layer, &mut Transform), Changed<Layer>>) {
    for (layer, mut transform) in &mut q {
        transform.translation.z = layer.z();
    }
}
