//! Fisica de plataforma: AABB, gravidade, resolucao eixo a eixo.
//!
//! Deliberadamente burra e deterministica. Nada de motor fisico -- num jogo de
//! briga o que importa e o controle previsivel, e resolucao separada por eixo
//! da exatamente isso (permite raspar parede sem grudar e subir quina sem
//! travar).

use bevy::prelude::*;

use crate::state::AppSet;

/// Aceleracao da gravidade em unidades/s^2.
pub const GRAVITY: f32 = 1400.0;
/// Velocidade terminal de queda.
pub const MAX_FALL: f32 = 900.0;
/// Abaixo disso a entidade caiu num buraco e morre.
pub const KILL_Y: f32 = -420.0;

/// Velocidade linear em unidades/s.
#[derive(Component, Default, Debug, Clone, Copy, Deref, DerefMut)]
pub struct Velocity(pub Vec2);

/// Caixa de colisao centrada na `Transform` da entidade.
#[derive(Component, Debug, Clone, Copy)]
pub struct Collider {
    /// Metade da largura e da altura.
    pub half: Vec2,
}

impl Collider {
    /// Colisor a partir do tamanho total.
    pub fn size(w: f32, h: f32) -> Self {
        Self {
            half: Vec2::new(w, h) * 0.5,
        }
    }

    /// AABB no mundo, dado o centro.
    pub fn aabb(&self, center: Vec2) -> Rect {
        Rect::from_center_half_size(center, self.half)
    }
}

/// Geometria solida nos quatro lados.
#[derive(Component)]
pub struct Solid;

/// Plataforma atravessavel por baixo: so bloqueia queda.
#[derive(Component)]
pub struct OneWay;

/// A entidade sofre gravidade.
#[derive(Component)]
pub struct Falls;

/// Suspende a gravidade neste frame (escalando corrente, por exemplo).
#[derive(Component)]
pub struct Weightless;

/// Estado de contato com o chao, preenchido pela resolucao de colisao.
#[derive(Component, Default, Debug, Clone, Copy)]
pub struct Grounded(pub bool);

/// Pede para a entidade ignorar plataformas one-way por um instante
/// (pular para baixo).
#[derive(Component, Debug)]
pub struct DropThrough(pub Timer);

/// Move mas nao colide com nada.
///
/// Projeteis usam isso: quem decide o que acontece no impacto e o sistema de
/// combate, nao a resolucao de colisao -- uma bala tem que sumir, nao parar.
#[derive(Component)]
pub struct Ghost;

/// Sobreposicao de dois retangulos, ou `None` se nao se tocam.
pub fn overlap(a: Rect, b: Rect) -> bool {
    a.min.x < b.max.x && a.max.x > b.min.x && a.min.y < b.max.y && a.max.y > b.min.y
}

/// Aplica gravidade a tudo que cai e nao esta suspenso.
fn apply_gravity(time: Res<Time>, mut q: Query<&mut Velocity, (With<Falls>, Without<Weightless>)>) {
    let dt = time.delta_secs();
    for mut vel in &mut q {
        vel.y = (vel.y - GRAVITY * dt).max(-MAX_FALL);
    }
}

/// Integra velocidade e resolve colisao contra a geometria estatica.
///
/// Move em X, corrige em X, move em Y, corrige em Y. A ordem importa: resolver
/// os dois de uma vez faz o corpo escorregar pela diagonal em quinas.
fn integrate_and_resolve(
    time: Res<Time>,
    mut movers: Query<(
        &mut Transform,
        &mut Velocity,
        &Collider,
        Option<&mut Grounded>,
        Option<&DropThrough>,
        Has<Ghost>,
    )>,
    solids: Query<(&Transform, &Collider, Option<&OneWay>), (With<Solid>, Without<Velocity>)>,
) {
    let dt = time.delta_secs();
    // Snapshot da geometria estatica: ela nao muda durante a resolucao.
    let world: Vec<(Rect, bool)> = solids
        .iter()
        .map(|(t, c, one_way)| (c.aabb(t.translation.truncate()), one_way.is_some()))
        .collect();

    for (mut transform, mut vel, collider, grounded, drop_through, ghost) in &mut movers {
        // Sem colisao: so integra e sai.
        if ghost {
            let step = vel.0 * dt;
            transform.translation.x += step.x;
            transform.translation.y += step.y;
            continue;
        }

        let mut pos = transform.translation.truncate();
        let ignore_one_way = drop_through.is_some_and(|d| !d.0.is_finished());

        // --- eixo X ---
        pos.x += vel.x * dt;
        for &(block, one_way) in &world {
            if one_way {
                continue; // one-way nunca bloqueia lateralmente
            }
            let body = collider.aabb(pos);
            if !overlap(body, block) {
                continue;
            }
            pos.x = if body.center().x < block.center().x {
                block.min.x - collider.half.x
            } else {
                block.max.x + collider.half.x
            };
            vel.x = 0.0;
        }

        // --- eixo Y ---
        let prev_bottom = collider.aabb(pos).min.y;
        pos.y += vel.y * dt;
        let mut on_ground = false;

        for &(block, one_way) in &world {
            let body = collider.aabb(pos);
            if !overlap(body, block) {
                continue;
            }
            if one_way {
                // So conta se estava caindo e vinha de cima da plataforma.
                if ignore_one_way || vel.y > 0.0 || prev_bottom < block.max.y - 1.0 {
                    continue;
                }
                pos.y = block.max.y + collider.half.y;
                vel.y = 0.0;
                on_ground = true;
                continue;
            }
            if body.center().y < block.center().y {
                pos.y = block.min.y - collider.half.y;
                vel.y = vel.y.min(0.0);
            } else {
                pos.y = block.max.y + collider.half.y;
                vel.y = vel.y.max(0.0);
                on_ground = true;
            }
        }

        if let Some(mut grounded) = grounded {
            grounded.0 = on_ground;
        }
        transform.translation.x = pos.x;
        transform.translation.y = pos.y;
    }
}

/// Particulas sao fantasmas sem colisor; ainda assim precisam integrar a
/// velocidade. Separar esta query mantem o caminho de colisao enxuto.
fn integrate_uncollidable_ghosts(
    time: Res<Time>,
    mut ghosts: Query<(&mut Transform, &Velocity), (With<Ghost>, Without<Collider>)>,
) {
    let dt = time.delta_secs();
    for (mut transform, velocity) in &mut ghosts {
        transform.translation += (velocity.0 * dt).extend(0.0);
    }
}

/// Conta o tempo de "atravessar plataforma" e remove o pedido quando expira.
fn tick_drop_through(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut DropThrough)>,
) {
    for (entity, mut drop) in &mut q {
        if drop.0.tick(time.delta()).is_finished() {
            commands.entity(entity).remove::<DropThrough>();
        }
    }
}

/// Gravidade, integracao e colisao, nessa ordem.
pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                tick_drop_through,
                apply_gravity,
                integrate_and_resolve,
                integrate_uncollidable_ghosts,
            )
                .chain()
                .in_set(AppSet::Physics),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aabb_centra_no_ponto_dado() {
        let c = Collider::size(10.0, 20.0);
        let r = c.aabb(Vec2::new(5.0, 5.0));
        assert_eq!(r.min, Vec2::new(0.0, -5.0));
        assert_eq!(r.max, Vec2::new(10.0, 15.0));
    }

    #[test]
    fn retangulos_que_so_encostam_nao_sobrepoem() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(10.0, 0.0, 20.0, 10.0);
        assert!(!overlap(a, b));
        assert!(overlap(a, Rect::new(9.0, 0.0, 20.0, 10.0)));
    }
}
