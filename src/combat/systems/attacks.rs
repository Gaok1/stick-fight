fn unarmed_move(step: u8) -> MeleeMove {
    match step % 3 {
        0 => MeleeMove {
            damage: 8,
            reach: 28.0,
            knockback: Vec2::new(225.0, 100.0),
            duration: 0.18,
            contact: 0.27,
        },
        1 => MeleeMove {
            damage: 9,
            reach: 31.0,
            knockback: Vec2::new(255.0, 145.0),
            duration: 0.20,
            contact: 0.25,
        },
        _ => MeleeMove {
            damage: 14,
            reach: 35.0,
            knockback: Vec2::new(360.0, 235.0),
            duration: 0.28,
            contact: 0.39,
        },
    }
}

fn begin_melee(commands: &mut Commands, entity: Entity, step: u8, held: Option<&Held>) {
    let (style, move_data) = held
        .map(|held| (held.weapon.style(), held.weapon.melee(step)))
        .unwrap_or((WeaponStyle::Unarmed, unarmed_move(step)));
    commands.entity(entity).insert((
        Attacking(Timer::from_seconds(move_data.duration, TimerMode::Once)),
        MeleeAttack {
            step,
            style,
            kind: MeleeKind::Chain,
            move_data,
            launched: false,
        },
        ComboChain {
            next: (step + 1) % 3,
            grace: Timer::from_seconds(COMBO_GRACE, TimerMode::Once),
        },
    ));
}

/// Rasteira do combate desarmado.
///
/// Dano baixo, mas o alvo fica no chao o dobro do tempo -- ela compra
/// vantagem, nao vida. Somada ao combo em pe, e o que da ao ataque uma leitura
/// alta e uma baixa em vez de um botao so.
const SWEEP_MOVE: MeleeMove = MeleeMove {
    damage: 7,
    reach: 34.0,
    knockback: Vec2::new(160.0, 60.0),
    duration: 0.30,
    contact: 0.42,
};

/// Quanto tempo a rasteira deixa o alvo no chao.
const SWEEP_STUN: f32 = 0.62;

/// Voadora: o golpe do ar.
///
/// Ela e comprometida de proposito -- quem aperta troca o controle da queda
/// pela chance de acertar. Errar significa aterrissar onde o oponente escolher.
const DIVE_MOVE: MeleeMove = MeleeMove {
    damage: 11,
    reach: 26.0,
    knockback: Vec2::new(240.0, -280.0),
    duration: 0.50,
    contact: 0.20,
};

/// Impulso da voadora. Substitui a velocidade em vez de somar: e o mergulho
/// que faz o golpe valer o risco.
const DIVE_LAUNCH: Vec2 = Vec2::new(300.0, -560.0);

/// Voadora ja gasta neste pulo. Sai ao encostar no chao.
///
/// Sem isso a voadora se cancelaria e se repetiria no ar, e o jogador nunca
/// perderia a altura que o golpe deveria custar.
#[derive(Component)]
struct DiveSpent;

/// Baixo + M1 com os pes no chao: rasteira.
///
/// Nao encadeia, igual a pancada pesada -- ela e o fim de uma sequencia
/// ("jab, cross, rasteira"), nao mais um elo dela.
fn start_sweep(
    mut commands: Commands,
    q: Query<
        (Entity, &Intent, &Pose, &Grounded),
        (With<Player>, Without<Attacking>, Without<Parrying>),
    >,
) {
    for (entity, intent, pose, grounded) in &q {
        if !intent.attack || !intent.down || !grounded.0 || pose.locks_control() {
            continue;
        }
        commands.entity(entity).insert((
            Attacking(Timer::from_seconds(SWEEP_MOVE.duration, TimerMode::Once)),
            MeleeAttack {
                step: 0,
                style: WeaponStyle::Unarmed,
                kind: MeleeKind::Sweep,
                move_data: SWEEP_MOVE,
                launched: false,
            },
        ));
        commands
            .entity(entity)
            .remove::<(ComboChain, QueuedAttack)>();
    }
}

/// M1 fora do chao: voadora.
///
/// Uma por pulo. O `DiveSpent` so sai ao aterrissar, entao nao da pra ficar
/// picotando o golpe no ar pra planar.
fn start_dive(
    mut commands: Commands,
    q: Query<
        (Entity, &Intent, &Pose, &Grounded),
        (
            With<Player>,
            Without<Attacking>,
            Without<Parrying>,
            Without<DiveSpent>,
        ),
    >,
) {
    for (entity, intent, pose, grounded) in &q {
        if !intent.attack || grounded.0 || pose.locks_control() {
            continue;
        }
        commands.entity(entity).insert((
            DiveSpent,
            Attacking(Timer::from_seconds(DIVE_MOVE.duration, TimerMode::Once)),
            MeleeAttack {
                step: 0,
                style: WeaponStyle::Unarmed,
                kind: MeleeKind::Dive,
                move_data: DIVE_MOVE,
                launched: false,
            },
        ));
        commands
            .entity(entity)
            .remove::<(ComboChain, QueuedAttack)>();
    }
}

/// Aterrissar encerra a voadora e devolve o golpe para o proximo pulo.
///
/// Sem isso o boneco pousaria e ficaria parado na pose de chute ate o timer
/// acabar, sem conseguir se defender.
fn land_dive(
    mut commands: Commands,
    q: Query<(Entity, &Grounded, Option<&MeleeAttack>), With<DiveSpent>>,
) {
    for (entity, grounded, melee) in &q {
        if !grounded.0 {
            continue;
        }
        commands.entity(entity).remove::<DiveSpent>();
        if melee.is_some_and(|m| m.kind == MeleeKind::Dive) {
            commands.entity(entity).remove::<(MeleeAttack, Attacking)>();
        }
    }
}

/// M2 de uma arma de contato: um golpe unico, caro, que nao entra no combo.
///
/// Nao insere `ComboChain` de proposito -- encadear a pancada pesada faria dela
/// a unica coisa que vale apertar.
fn start_heavy(
    mut commands: Commands,
    mut q: Query<
        (Entity, &Intent, &Pose, &mut Facing, &mut Held),
        (With<Player>, Without<Attacking>, Without<Parrying>),
    >,
) {
    for (entity, intent, pose, mut facing, mut held) in &mut q {
        if !intent.special || pose.locks_control() {
            continue;
        }
        let Some(move_data) = held.weapon.heavy() else {
            continue;
        };
        // Quem tica o timer e `fire`, que roda para toda arma equipada; aqui
        // ele so e consumido, senao a cadencia correria em dobro.
        if !held.cooldown.is_finished() {
            continue;
        }
        held.cooldown.reset();
        crate::weapon::turn_to_aim(&mut facing, intent);
        commands.entity(entity).insert((
            Attacking(Timer::from_seconds(move_data.duration, TimerMode::Once)),
            MeleeAttack {
                step: 2,
                style: held.weapon.style(),
                kind: MeleeKind::Heavy,
                move_data,
                launched: false,
            },
        ));
        // Um golpe pesado corta qualquer combo em andamento: ele e o fim da
        // sequencia, nao mais um elo dela.
        commands
            .entity(entity)
            .remove::<(ComboChain, QueuedAttack)>();
    }
}

/// M1 sempre e corpo-a-corpo; cada arma troca alcance, ritmo e finalizador.
fn start_melee(
    mut commands: Commands,
    mut q: Query<
        (
            Entity,
            &Intent,
            &Pose,
            &Grounded,
            &mut Facing,
            Option<&Held>,
            Option<&ComboChain>,
        ),
        (With<Player>, Without<Attacking>, Without<Parrying>),
    >,
) {
    for (entity, intent, pose, grounded, mut facing, held, combo) in &mut q {
        // Baixo + M1 e a rasteira e fora do chao e a voadora; os dois tem
        // sistema proprio. Aqui fica so o combo em pe.
        if !intent.attack || intent.down || !grounded.0 || pose.locks_control() {
            continue;
        }
        crate::weapon::turn_to_aim(&mut facing, intent);
        let step = combo
            .filter(|c| !c.grace.is_finished())
            .map_or(0, |c| c.next);
        begin_melee(&mut commands, entity, step, held);
    }
}

fn queue_melee(
    mut commands: Commands,
    q: Query<(Entity, &Intent), (With<Player>, With<Attacking>, With<MeleeAttack>)>,
) {
    for (entity, intent) in &q {
        if intent.attack {
            commands.entity(entity).insert(QueuedAttack);
        }
    }
}

/// Altura em que a hitbox abre, relativa ao centro do atacante.
///
/// E o que o oponente le. A rasteira nasce na canela e por isso passa por
/// baixo de quem esta no ar; o gancho nasce no alto e e a resposta pra esse
/// caso. Sem essa diferenca, alto e baixo seriam o mesmo golpe com nomes
/// distintos.
fn melee_height(kind: MeleeKind, step: u8) -> f32 {
    match kind {
        MeleeKind::Sweep => -18.0,
        // A voadora desce em cima do alvo: o pe chega abaixo do centro dela.
        MeleeKind::Dive => -12.0,
        // A pancada termina com o cano embaixo, entao ela abre onde a mao
        // para -- e nao no alto, como o gancho.
        MeleeKind::Heavy => -6.0,
        // O gancho sobe de verdade: alto o bastante para passar por cima de
        // quem esta agachado, que e a resposta defensiva a ele.
        _ if step == 2 => 22.0,
        _ => 2.0,
    }
}

/// Quanto tempo a hitbox fica no ar.
///
/// A voadora precisa de uma janela longa porque ela viaja com o corpo -- com
/// os 0,075 s dos outros golpes ela so acertaria quem estivesse exatamente no
/// ponto de contato.
fn melee_active(kind: MeleeKind) -> f32 {
    match kind {
        MeleeKind::Dive => 0.26,
        _ => MELEE_ACTIVE,
    }
}

/// O golpe sai na direcao da mira, ou apenas para o lado em que o boneco olha?
///
/// Soco e pancada saem do braco, e o braco aponta para onde o jogador esta
/// olhando -- o mesmo contrato que o tiro ja tinha. Rasteira e voadora saem da
/// perna e do corpo inteiro: a rasteira varre o chao e a voadora e a propria
/// queda, entao mandar as duas para o cursor tiraria delas o que as define.
fn follows_aim(kind: MeleeKind) -> bool {
    matches!(kind, MeleeKind::Chain | MeleeKind::Heavy)
}

/// Para onde este golpe vai.
fn strike_dir(kind: MeleeKind, intent: &Intent, facing: &Facing) -> Vec2 {
    if follows_aim(kind) {
        crate::weapon::aim_dir(intent, facing)
    } else {
        Vec2::new(facing.0, 0.0)
    }
}

/// Abre a hitbox exatamente no quadro visual de contato.
fn launch_melee(
    mut commands: Commands,
    mut q: Query<
        (
            Entity,
            &Transform,
            &Facing,
            &Intent,
            &Attacking,
            &mut MeleeAttack,
            &mut Velocity,
        ),
        With<Player>,
    >,
) {
    for (entity, transform, facing, intent, attacking, mut attack, mut velocity) in &mut q {
        if attack.launched
            || attacking.0.elapsed_secs() < attack.move_data.duration * attack.move_data.contact
        {
            continue;
        }
        attack.launched = true;
        let dir = strike_dir(attack.kind, intent, facing);
        match attack.kind {
            // A voadora troca a velocidade inteira: quem aperta abre mao do
            // controle da queda em troca da chance de acertar.
            MeleeKind::Dive => velocity.0 = Vec2::new(facing.0 * DIVE_LAUNCH.x, DIVE_LAUNCH.y),
            // O passo do golpe acompanha so o quanto a mira aponta para o lado:
            // socar para cima nao pode empurrar o corpo para frente.
            _ => velocity.x += dir.x * (70.0 + attack.step as f32 * 28.0),
        }
        // O punho vai parar onde o cursor esta; a altura continua sendo o que
        // separa gancho de rasteira, e por isso ela e somada, nao girada.
        let offset =
            dir * attack.move_data.reach + Vec2::Y * melee_height(attack.kind, attack.step);
        let at = transform.translation.truncate() + offset;
        let mut hitbox = commands.spawn((
            Hitbox {
                owner: entity,
                damage: attack.move_data.damage,
                // O empurrao sai na linha do golpe, e o levante continua sendo
                // para cima: quem soca de cima para baixo prega o outro no
                // chao, e quem soca para cima o joga para o alto.
                knockback: dir * attack.move_data.knockback.x
                    + Vec2::Y * attack.move_data.knockback.y,
                stun: if attack.kind == MeleeKind::Sweep {
                    SWEEP_STUN
                } else {
                    HIT_STUN
                },
            },
            Lifetime(Timer::from_seconds(
                melee_active(attack.kind),
                TimerMode::Once,
            )),
            Collider::size(32.0 + attack.move_data.reach * 0.18, MELEE_BOX_H),
            Transform::from_translation(at.extend(0.0)),
            DespawnOnExit(GameState::Fighting),
        ));
        if attack.kind == MeleeKind::Dive {
            hitbox.insert(FollowsOwner(offset));
        }
        let arc = match attack.style {
            WeaponStyle::Unarmed => {
                if attack.step == 2 {
                    "))"
                } else {
                    ")"
                }
            }
            WeaponStyle::Pistol => "-)>",
            // Cortes curtos, arco longo de lamina e giro de corrente tem
            // silhuetas proprias para o estilo ser legivel no contato.
            WeaponStyle::Knife => "/)",
            WeaponStyle::Katana => "====/)",
            // Nao e arco: e linha. O rastro do estoque aponta para onde a
            // ponta foi, e e a unica silhueta de contato sem curva.
            WeaponStyle::FencySword => "-----\u{25ba}",
            WeaponStyle::Nunchaku => "o~)))",
            WeaponStyle::Shotgun => "==)>",
            WeaponStyle::Rifle => "---)>",
            WeaponStyle::Book => "\u{03a6}))",
            WeaponStyle::Bomb => ")",
            // A pancada pesada desce, entao o rastro dela e mais largo que o
            // do combo normal do cano.
            WeaponStyle::Pipe => {
                if attack.kind == MeleeKind::Heavy {
                    "=))))"
                } else {
                    "=))"
                }
            }
        };
        // O rastro e desenhado apontando para a direita: espelha quando o golpe
        // vai para o outro lado e gira ate a linha da mira. Sem o giro, um
        // gancho para cima abria a hitbox no alto e desenhava o arco deitado.
        let flipped = dir.x < 0.0;
        let angle = dir.to_angle() - if flipped { std::f32::consts::PI } else { 0.0 };
        commands.spawn((
            AsciiSprite::new(AsciiArt::solid(arc, palette::GOLD)).flipped(flipped),
            Layer::Fx,
            Transform::from_translation(at.extend(0.0))
                .with_rotation(Quat::from_rotation_z(angle)),
            Lifetime(Timer::from_seconds(0.11, TimerMode::Once)),
            DespawnOnExit(GameState::Fighting),
        ));
    }
}

fn continue_combo(
    mut commands: Commands,
    q: Query<
        (
            Entity,
            Option<&Held>,
            Option<&ComboChain>,
            Has<QueuedAttack>,
        ),
        (With<MeleeAttack>, Without<Attacking>),
    >,
) {
    for (entity, held, combo, queued) in &q {
        commands.entity(entity).remove::<MeleeAttack>();
        // Pancada, rasteira e voadora nao encadeiam, entao nao tem
        // `ComboChain`. Sem este caminho o `MeleeAttack` delas ficaria
        // pendurado no jogador depois do golpe acabar.
        let Some(combo) = combo else {
            continue;
        };
        if queued && !combo.grace.is_finished() {
            commands.entity(entity).remove::<QueuedAttack>();
            begin_melee(&mut commands, entity, combo.next, held);
        }
    }
}

fn start_parry(
    mut commands: Commands,
    q: Query<
        (Entity, &Intent, &Pose),
        (
            With<Player>,
            Without<Attacking>,
            Without<Parrying>,
            Without<ParryCooldown>,
        ),
    >,
) {
    for (entity, intent, pose) in &q {
        if intent.parry && !pose.locks_control() {
            commands.entity(entity).insert((
                Parrying(Timer::from_seconds(PARRY_TOTAL, TimerMode::Once)),
                ParryCooldown(Timer::from_seconds(PARRY_COOLDOWN, TimerMode::Once)),
            ));
        }
    }
}

