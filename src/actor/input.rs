//! Entrada dos jogadores.
//!
//! O gameplay nunca le teclado: ele le `Intent`. Quem produz o `Intent` fica
//! atras de um trait, entao trocar um jogador humano por IA ou por um replay de
//! rede e trocar o `Box<dyn InputSource>` -- nenhum sistema de movimento,
//! combate ou arma precisa mudar.

use bevy::prelude::*;

use crate::state::AppSet;

/// O que um ator quer fazer neste frame, ja abstraido do dispositivo.
#[derive(Component, Default, Debug, Clone, Copy)]
pub struct Intent {
    /// Eixo horizontal em [-1, 1].
    pub move_x: f32,
    /// Segurando cima (pular / escalar).
    pub up: bool,
    /// Segurando baixo (descer corrente / atravessar plataforma).
    pub down: bool,
    /// Cima foi apertado neste frame.
    pub jump: bool,
    /// Golpe principal / combo (M1 para o jogador 1).
    pub attack: bool,
    /// Especial da arma / disparo (M2 para o jogador 1).
    pub special: bool,
    /// Defesa de precisao.
    pub parry: bool,
    /// Agarrar ou soltar da corrente.
    pub grapple: bool,
    /// Arremessar a arma equipada.
    pub throw_weapon: bool,
    /// Direcao da mira, normalizada. `ZERO` quando o ator nao mira (usa o lado
    /// para o qual olha) -- so quem tem mouse preenche isso.
    pub aim: Vec2,
}

/// O que ha do lado de quem esta pensando em andar para la.
///
/// Nao e pathfinding: e o minimo para nao caminhar para dentro de um buraco.
#[derive(Debug, Clone, Copy, Default)]
pub struct Footing {
    /// Ha chao logo a frente, um passo adiante.
    pub step: bool,
    /// Ha chao do outro lado, ao alcance de um pulo.
    pub across: bool,
}

/// A arma na mao, do ponto de vista de quem decide o que fazer com ela.
#[derive(Debug, Clone, Copy)]
pub struct Armed {
    /// Ainda tem com que atirar.
    pub loaded: bool,
    /// E arma de contato: o M2 dela e uma pancada, nao um disparo.
    pub melee: bool,
}

/// O que um ator percebe do mundo neste quadro.
///
/// Existe porque `poll` so recebia o teclado. Dava para plugar outro
/// dispositivo, mas nao uma IA: ela nao tinha como saber onde o oponente esta.
/// O contrato prometia IA e nao entregava o que uma precisa para decidir.
#[derive(Debug, Clone, Copy)]
pub struct Sense {
    /// Onde o proprio ator esta.
    pub at: Vec2,
    /// Onde esta o adversario, se houver um.
    pub foe: Option<Vec2>,
    /// Com os pes no chao.
    pub grounded: bool,
    /// Fracao de vida propria, de 0 a 1.
    pub health: f32,
    /// Tempo de jogo, para quem precisa de cadencia.
    ///
    /// Sem isto uma fonte sem estado so consegue reagir, nunca ritmar -- e
    /// um adversario que ataca todo quadro vira metralhadora.
    pub time: f32,
    /// Chao a esquerda.
    pub left: Footing,
    /// Chao a direita.
    pub right: Footing,
    /// O que ele tem na mao, se tem alguma coisa.
    pub armed: Option<Armed>,
}

impl Sense {
    /// Chao no sentido de `dir` (negativo esquerda, positivo direita).
    pub fn footing(&self, dir: f32) -> Footing {
        if dir < 0.0 { self.left } else { self.right }
    }
}

/// Fonte de intencao. Implemente para plugar IA, gamepad ou rede.
pub trait InputSource: Send + Sync + 'static {
    /// Produz a intencao deste frame.
    fn poll(&self, keys: &ButtonInput<KeyCode>, sense: &Sense) -> Intent;
}

/// Liga um ator a uma fonte de entrada.
#[derive(Component)]
pub struct Controller(pub Box<dyn InputSource>);

/// Mapeamento de teclas de um jogador humano.
pub struct KeyBindings {
    /// Anda para a esquerda.
    pub left: KeyCode,
    /// Anda para a direita.
    pub right: KeyCode,
    /// Pula e escala.
    pub up: KeyCode,
    /// Desce corrente e atravessa plataforma.
    pub down: KeyCode,
    /// Soco ou disparo. Varias teclas porque teclado numerico nem sempre existe.
    pub attack: &'static [KeyCode],
    /// Especial da arma.
    pub special: &'static [KeyCode],
    /// Aparar.
    pub parry: &'static [KeyCode],
    /// Agarra ou solta da corrente.
    pub grapple: &'static [KeyCode],
    /// Arremessa a arma equipada.
    pub throw_weapon: &'static [KeyCode],
}

impl KeyBindings {
    /// Jogador 1: WASD + F.
    pub fn player_one() -> Self {
        Self {
            left: KeyCode::KeyA,
            right: KeyCode::KeyD,
            up: KeyCode::KeyW,
            down: KeyCode::KeyS,
            attack: &[],
            special: &[KeyCode::KeyR],
            parry: &[KeyCode::KeyQ],
            grapple: &[KeyCode::KeyF],
            throw_weapon: &[KeyCode::KeyG],
        }
    }

    /// Jogador 2: setas + Numpad0 / Ctrl direito.
    pub fn player_two() -> Self {
        Self {
            left: KeyCode::ArrowLeft,
            right: KeyCode::ArrowRight,
            up: KeyCode::ArrowUp,
            down: KeyCode::ArrowDown,
            attack: &[KeyCode::Numpad0, KeyCode::ControlRight, KeyCode::Period],
            special: &[KeyCode::Numpad2],
            parry: &[KeyCode::Numpad3],
            grapple: &[KeyCode::Numpad1, KeyCode::ShiftRight, KeyCode::Slash],
            throw_weapon: &[KeyCode::Numpad4],
        }
    }
}

impl InputSource for KeyBindings {
    /// Teclado nao percebe nada: quem joga e que percebe.
    fn poll(&self, keys: &ButtonInput<KeyCode>, _sense: &Sense) -> Intent {
        let axis = |neg: KeyCode, pos: KeyCode| {
            (keys.pressed(pos) as i32 - keys.pressed(neg) as i32) as f32
        };
        Intent {
            move_x: axis(self.left, self.right),
            up: keys.pressed(self.up),
            down: keys.pressed(self.down),
            jump: keys.just_pressed(self.up),
            attack: self.attack.iter().any(|k| keys.just_pressed(*k)),
            special: self.special.iter().any(|k| keys.just_pressed(*k)),
            parry: self.parry.iter().any(|k| keys.just_pressed(*k)),
            grapple: self.grapple.iter().any(|k| keys.just_pressed(*k)),
            throw_weapon: self.throw_weapon.iter().any(|k| keys.just_pressed(*k)),
            // Teclado nao mira: quem preenche isso e o mouse, em `aim_at_cursor`.
            aim: Vec2::ZERO,
        }
    }
}

/// Alcance em que o adversario para de andar e comeca a bater.
const CPU_REACH: f32 = 46.0;
/// Distancia que ele tenta manter quando tem arma de fogo carregada.
const CPU_RANGE: f32 = 260.0;

/// Adversario controlado pelo jogo.
///
/// Deliberadamente legivel: aproxima, bate no alcance, sobe atras de quem esta
/// em cima e abre distancia quando esta ferido. Nao existe para ser dificil --
/// existe para dar pra jogar sozinho, e para provar que o `InputSource` aceita
/// mesmo outra fonte que nao o teclado.
///
/// Sem estado proprio: tudo que ele decide sai da percepcao do quadro. Isso o
/// torna uma funcao pura, que e o que permite testa-lo sem subir um `App`.
pub struct CpuBrain {
    /// Defasagem da cadencia. Dois adversarios com a mesma fase atacariam no
    /// mesmo instante.
    pub phase: f32,
}

impl InputSource for CpuBrain {
    fn poll(&self, _keys: &ButtonInput<KeyCode>, sense: &Sense) -> Intent {
        let Some(foe) = sense.foe else {
            return Intent::default();
        };

        let delta = foe - sense.at;
        let dist = delta.x.abs();
        let mut intent = Intent::default();

        // Pensa em batidas, nao em quadros. Sem isso ele ataca 60 vezes por
        // segundo e nao ha jogo.
        let beat = (sense.time * 2.6 + self.phase).sin();

        // Arma de fogo carregada muda a distancia que vale a pena manter.
        // Sem isto ele caminhava ate o alcance de soco segurando uma escopeta,
        // e pegar arma o deixava mais fraco.
        let atirando = sense.armed.is_some_and(|arma| arma.loaded && !arma.melee);
        let ideal = if atirando { CPU_RANGE } else { CPU_REACH };

        // --- para onde ir ---
        let quer = if dist > ideal * 1.15 {
            delta.x.signum()
        } else if dist < ideal * 0.7 || (sense.health < 0.35 && beat < -0.5) {
            -delta.x.signum()
        } else {
            0.0
        };
        if quer != 0.0 {
            let footing = sense.footing(quer);
            if footing.step {
                intent.move_x = quer;
            } else if footing.across && quer == delta.x.signum() {
                // Pula vao so para perseguir. Recuar para dentro de um buraco
                // e pior que ficar encurralado.
                intent.move_x = quer;
                intent.jump = true;
                intent.up = true;
            }
            // Sem chao dos dois jeitos ele fica onde esta. Perseguir ate cair
            // no buraco e como ele perdia sozinho em dois dos tres mapas.
        }

        // --- o que fazer ---
        if atirando {
            // Mira de verdade, e nao so pra frente: o oponente costuma estar
            // numa altura diferente.
            intent.aim = delta.normalize_or_zero();
            intent.special = beat > 0.55;
        } else if dist <= CPU_REACH {
            intent.attack = beat > 0.4;
            // Mistura o golpe baixo: previsivel demais e de graca pra ler.
            intent.down = beat < -0.86;
            intent.parry = beat < -0.97;
            // Com arma de contato, a pancada do M2 vale a preparacao de vez
            // em quando.
            if sense.armed.is_some_and(|arma| arma.melee) {
                intent.special = beat > 0.93;
            }
        }

        // Sobe atras de quem esta numa plataforma acima.
        if delta.y > 40.0 && sense.grounded {
            intent.jump = true;
            intent.up = true;
        }

        intent
    }
}

/// Cursor do mouse em coordenadas de mundo, ou `None` se ele saiu da janela.
///
/// Fica num recurso porque mais de um sistema quer o mesmo ponto (a mira dos
/// tiros e o retículo desenhado na tela) e a conversao e a mesma para todos.
#[derive(Resource, Default)]
pub struct CursorWorld(pub Option<Vec2>);

/// Converte a posicao do cursor na janela para o mundo.
fn track_cursor(
    mut cursor: ResMut<CursorWorld>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((camera, camera_transform)) = cameras.single() else {
        return;
    };
    cursor.0 = window
        .cursor_position()
        .and_then(|at| camera.viewport_to_world_2d(camera_transform, at).ok());
}

/// Roda toda fonte de entrada e escreve o `Intent` de cada ator.
fn gather_intents(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Res<CursorWorld>,
    level: Res<crate::level::CurrentLevel>,
    mode: Res<crate::state::GameMode>,
    online: Res<crate::online::OnlineSession>,
    lutadores: Query<(Entity, &GlobalTransform), With<super::Player>>,
    mut q: Query<(
        Entity,
        &super::Player,
        &Controller,
        &GlobalTransform,
        &crate::physics::Grounded,
        &super::Health,
        Option<&crate::weapon::Held>,
        &mut Intent,
    )>,
) {
    let posicoes: Vec<(Entity, Vec2)> = lutadores
        .iter()
        .map(|(entity, transform)| (entity, transform.translation().truncate()))
        .collect();

    // Um passo a frente e um vao inteiro adiante. O segundo cabe no arco do
    // pulo com folga, entao "tem chao la" significa mesmo "da pra chegar".
    const PASSO: f32 = 46.0;
    const VAO: f32 = 140.0;
    /// Queda maior que isto conta como buraco, mesmo tendo chao la embaixo.
    const QUEDA_ACEITAVEL: f32 = 140.0;

    for (entity, player, controller, transform, grounded, health, held, mut intent) in &mut q {
        let at = transform.translation().truncate();
        let firme = |dx: f32| {
            level
                .0
                .ground_under(at + Vec2::new(dx, 0.0))
                .is_some_and(|chao| at.y - chao < QUEDA_ACEITAVEL)
        };
        let footing = |dir: f32| Footing {
            step: firme(dir * PASSO),
            across: firme(dir * VAO),
        };

        let sense = Sense {
            at,
            // O adversario mais proximo. Com dois lutadores e sempre o outro,
            // mas assim a percepcao nao assume que sao so dois.
            foe: posicoes
                .iter()
                .filter(|(outro, _)| *outro != entity)
                .min_by(|a, b| {
                    a.1.distance_squared(at)
                        .total_cmp(&b.1.distance_squared(at))
                })
                .map(|(_, pos)| *pos),
            grounded: grounded.0,
            health: health.fraction(),
            time: time.elapsed_secs(),
            left: footing(-1.0),
            right: footing(1.0),
            armed: held.map(|held| Armed {
                loaded: held.ammo > 0,
                melee: held.weapon.is_melee(),
            }),
        };
        *intent = controller.0.poll(&keys, &sense);
        let local = if *mode == crate::state::GameMode::Online {
            online.local_player_id()
        } else {
            0
        };
        if player.id != local {
            continue;
        }
        intent.attack |= mouse.just_pressed(MouseButton::Left);
        intent.special |= mouse.just_pressed(MouseButton::Right);
        if let Some(at) = cursor.0 {
            // Mira sai do ombro, nao do centro do corpo: e de la que o cano sai.
            let from = transform.translation().truncate() + Vec2::new(0.0, 8.0);
            intent.aim = (at - from).normalize_or_zero();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Chao solido dos dois lados: o caso comum, sem buraco por perto.
    const FIRME: Footing = Footing {
        step: true,
        across: true,
    };
    /// Beirada: um passo a frente e o vazio, e nao ha nada do outro lado.
    const ABISMO: Footing = Footing {
        step: false,
        across: false,
    };
    /// Vao curto: da pra pular por cima.
    const VAO: Footing = Footing {
        step: false,
        across: true,
    };

    fn percebendo(foe: Option<Vec2>, health: f32, time: f32) -> Sense {
        Sense {
            at: Vec2::ZERO,
            foe,
            grounded: true,
            health,
            time,
            left: FIRME,
            right: FIRME,
            armed: None,
        }
    }

    /// Varre um ciclo inteiro de cadencia e devolve o que o adversario faz.
    fn ao_longo_do_tempo(sense: impl Fn(f32) -> Sense) -> Vec<Intent> {
        let brain = CpuBrain { phase: 0.0 };
        let keys = ButtonInput::<KeyCode>::default();
        (0..240)
            .map(|passo| passo as f32 * 0.01)
            .map(|t| brain.poll(&keys, &sense(t)))
            .collect()
    }

    /// Longe, ele tem que andar na direcao do oponente -- nos dois lados.
    #[test]
    fn o_adversario_persegue_de_qualquer_lado() {
        let brain = CpuBrain { phase: 0.0 };
        let keys = ButtonInput::<KeyCode>::default();

        for lado in [-1.0f32, 1.0] {
            let longe = Vec2::new(lado * (CPU_REACH + 200.0), 0.0);
            let intent = brain.poll(&keys, &percebendo(Some(longe), 1.0, 0.0));
            assert_eq!(
                intent.move_x, lado,
                "com o oponente em {longe:?} ele andou pro lado errado"
            );
        }
    }

    /// Perto, ele para de andar e passa a bater.
    #[test]
    fn o_adversario_ataca_no_alcance() {
        let perto = Vec2::new(CPU_REACH - 6.0, 0.0);
        let quadros = ao_longo_do_tempo(|t| percebendo(Some(perto), 1.0, t));

        assert!(
            quadros.iter().any(|intent| intent.attack),
            "no alcance ele nunca ataca"
        );
        assert!(
            quadros.iter().all(|intent| intent.move_x == 0.0),
            "no alcance ele continua andando por cima do oponente"
        );
    }

    /// Mas nao pode atacar todo quadro: sem cadencia nao ha jogo.
    #[test]
    fn o_adversario_respira_entre_os_golpes() {
        let perto = Vec2::new(CPU_REACH - 6.0, 0.0);
        let quadros = ao_longo_do_tempo(|t| percebendo(Some(perto), 1.0, t));
        let atacando = quadros.iter().filter(|intent| intent.attack).count();

        assert!(
            atacando > 0 && atacando < quadros.len() * 2 / 3,
            "ele ataca em {atacando} de {} quadros",
            quadros.len()
        );
    }

    /// Ferido e encurralado, ele tem que tentar sair, nao so trocar golpe.
    #[test]
    fn o_adversario_recua_quando_ferido() {
        let perto = Vec2::new(CPU_REACH - 6.0, 0.0);
        let quadros = ao_longo_do_tempo(|t| percebendo(Some(perto), 0.15, t));

        assert!(
            quadros.iter().any(|intent| intent.move_x < 0.0),
            "ferido, ele nunca abre distancia"
        );
    }

    /// Com arma de fogo carregada ele tem que atirar de longe.
    ///
    /// Antes disto ele nunca apertava o M2: pegar uma escopeta o deixava mais
    /// fraco, porque ele caminhava ate o alcance de soco segurando ela.
    #[test]
    fn o_adversario_atira_em_vez_de_colar() {
        let longe = Vec2::new(CPU_RANGE, 40.0);
        let armado = |t: f32| Sense {
            armed: Some(Armed {
                loaded: true,
                melee: false,
            }),
            ..percebendo(Some(longe), 1.0, t)
        };
        let quadros = ao_longo_do_tempo(armado);

        assert!(
            quadros.iter().any(|intent| intent.special),
            "com arma carregada ele nunca dispara"
        );
        assert!(
            quadros.iter().all(|intent| !intent.attack),
            "ele socou o ar a {CPU_RANGE} de distancia"
        );
        // Mira de verdade: apontar so pra frente erraria quem esta mais alto.
        let mirando = quadros[0].aim;
        assert!(
            mirando.y > 0.0,
            "ele nao mira pra cima do alvo: {mirando:?}"
        );
    }

    /// Arma vazia nao muda nada: ele volta a fechar a distancia, porque so o
    /// soco resolve.
    #[test]
    fn arma_vazia_nao_mantem_o_adversario_longe() {
        let brain = CpuBrain { phase: 0.0 };
        let keys = ButtonInput::<KeyCode>::default();
        let sense = Sense {
            armed: Some(Armed {
                loaded: false,
                melee: false,
            }),
            ..percebendo(Some(Vec2::new(CPU_RANGE, 0.0)), 1.0, 0.0)
        };

        assert_eq!(
            brain.poll(&keys, &sense).move_x,
            1.0,
            "com a arma vazia ele ficou parado esperando"
        );
    }

    /// Arma de contato tambem nao: o M2 dela e uma pancada, que so vale de
    /// perto.
    #[test]
    fn arma_de_contato_mantem_o_adversario_perto() {
        let perto = Vec2::new(CPU_REACH - 6.0, 0.0);
        let com_cano = |t: f32| Sense {
            armed: Some(Armed {
                loaded: false,
                melee: true,
            }),
            ..percebendo(Some(perto), 1.0, t)
        };
        let quadros = ao_longo_do_tempo(com_cano);

        assert!(
            quadros.iter().any(|intent| intent.attack),
            "com o cano ele parou de bater"
        );
        assert!(
            quadros.iter().any(|intent| intent.special),
            "ele nunca usa a pancada do cano"
        );
        let pancadas = quadros.iter().filter(|intent| intent.special).count();
        assert!(
            pancadas < quadros.len() / 4,
            "a pancada virou o golpe padrao: {pancadas} de {}",
            quadros.len()
        );
    }

    /// Perseguir ate cair no buraco foi como o adversario perdeu sozinho em
    /// dois dos tres mapas: na Arena 01 ele nasce no trecho da direita e anda
    /// reto para dentro do vao; na Arena 02 sai da torre para o vazio.
    #[test]
    fn o_adversario_para_na_beirada() {
        let brain = CpuBrain { phase: 0.0 };
        let keys = ButtonInput::<KeyCode>::default();

        let mut sense = percebendo(Some(Vec2::new(-600.0, 0.0)), 1.0, 0.0);
        sense.left = ABISMO;
        let intent = brain.poll(&keys, &sense);

        assert_eq!(
            intent.move_x, 0.0,
            "ele andou para dentro do buraco atras do oponente"
        );
        assert!(!intent.jump, "ele pulou para dentro do buraco");
    }

    /// Mas parar em toda beirada o deixaria preso no proprio trecho de chao.
    /// Vao curto ele atravessa.
    #[test]
    fn o_adversario_pula_o_vao_curto() {
        let brain = CpuBrain { phase: 0.0 };
        let keys = ButtonInput::<KeyCode>::default();

        let mut sense = percebendo(Some(Vec2::new(-600.0, 0.0)), 1.0, 0.0);
        sense.left = VAO;
        let intent = brain.poll(&keys, &sense);

        assert_eq!(intent.move_x, -1.0, "ele nao foi na direcao do vao");
        assert!(intent.jump, "ele nao pulou o vao que da pra pular");
    }

    /// Sem oponente ele fica parado, em vez de andar pra algum lugar
    /// arbitrario -- e o caso do round que acabou de terminar.
    #[test]
    fn o_adversario_sem_alvo_fica_parado() {
        let brain = CpuBrain { phase: 0.0 };
        let keys = ButtonInput::<KeyCode>::default();
        let intent = brain.poll(&keys, &percebendo(None, 1.0, 3.0));

        assert_eq!(intent.move_x, 0.0);
        assert!(!intent.attack && !intent.jump);
    }
}

/// Coleta de entrada, no inicio do frame.
pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CursorWorld>().add_systems(
            Update,
            (track_cursor, gather_intents).chain().in_set(AppSet::Input),
        );
    }
}
