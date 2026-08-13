//! Tabela visual das poses: uma linha por pose, e nada fora dela.
//!
//! Uma pose e quatro coisas -- a silhueta, como o corpo se deforma, o que os
//! membros fazem e de que cor ela pinta o boneco. Isso vivia espalhado por sete
//! lugares: tres `match` sobre `Pose` em dois arquivos e tres listas que
//! precisavam concordar entre si. Uma pose nova so ficava completa se quem a
//! escreveu lembrasse de todos, e a que esquecesse um saia calada -- foi assim
//! que `Downed` nasceu fora dos testes de caixa.
//!
//! Aqui pose nova e uma variante do enum e uma linha em [`POSES`]. O tamanho da
//! tabela e `Pose::COUNT`, que por sua vez e o discriminante da ultima pose:
//! mexer no enum quebra a compilacao da tabela ate ela receber a linha.

use bevy::prelude::*;

use super::pose::{Arm, BODY_COLS, BODY_ROWS, DOWNED_ARM, PARRY_ARM, Pose, Strike};
use super::skin::Tone;
use crate::ascii::CELL;

// --- clipes -----------------------------------------------------------------

/// Uma animacao: os quadros na ordem em que tocam.
///
/// Existe para que um ciclo seja um conceito, e nao um punhado de variantes
/// irmas. O ciclo de corrida ja foi tres listas para uma animacao so -- as seis
/// poses, uma `running()` que as ordenava e uma `run_frame()` que desfazia a
/// ordem. `Clip` e a lista unica: quem toca usa [`Clip::at`], quem pergunta em
/// que quadro esta usa [`Clip::index_of`], e as duas nunca podem divergir.
pub struct Clip(pub &'static [Pose]);

impl Clip {
    /// Quantos quadros o clipe tem.
    pub const fn len(&self) -> usize {
        self.0.len()
    }

    /// Quadro `i`, girando.
    pub fn at(&self, i: usize) -> Pose {
        self.0[i % self.0.len()]
    }

    /// Posicao da pose dentro do clipe, se ela pertence a ele.
    pub fn index_of(&self, pose: Pose) -> Option<usize> {
        self.0.iter().position(|frame| *frame == pose)
    }
}

/// Respiracao parada: dois quadros que se alternam devagar.
pub const IDLE: Clip = Clip(&[Pose::IdleA, Pose::IdleB]);

/// Ciclo de corrida.
pub const RUN: Clip = Clip(&[
    Pose::RunA,
    Pose::RunB,
    Pose::RunC,
    Pose::RunD,
    Pose::RunE,
    Pose::RunF,
]);

/// Bracadas na corrente.
pub const CLIMB: Clip = Clip(&[Pose::ClimbA, Pose::ClimbB]);

/// Preparo, contato e recuperacao de um golpe corpo-a-corpo.
pub const PUNCH: Clip = Clip(&[Pose::PunchWindup, Pose::PunchStrike, Pose::PunchRecover]);

/// Todos os clipes, para quem precisa saber em que quadro uma pose esta sem
/// saber a que animacao ela pertence.
#[cfg(test)]
pub const CLIPS: [&Clip; 4] = [&IDLE, &RUN, &CLIMB, &PUNCH];

/// Quantos quadros tem o ciclo de corrida.
#[cfg(test)]
pub const RUN_FRAMES: usize = RUN.len();

/// Fase da passada no quadro `frame`, de -1 a 1 e de volta.
///
/// Gerada, e nao listada a mao. A tabela escrita a mao que existia aqui andava
/// -1, -0.6, 0, 0.6, 1, 0.35 -- e ao fechar o ciclo pulava 1.35 de uma vez,
/// mais que o dobro de qualquer outro degrau. Isso e um tranco na perna toda
/// vez que a passada da a volta.
///
/// Uma onda triangular fecha com degraus iguais por construcao: nao ha como
/// escrever um valor fora do lugar.
#[cfg(test)]
pub fn run_gait(frame: usize) -> f32 {
    let t = (frame % RUN_FRAMES) as f32 / RUN_FRAMES as f32;
    if t < 0.5 {
        -1.0 + 4.0 * t
    } else {
        3.0 - 4.0 * t
    }
}

// --- deformacao do corpo ----------------------------------------------------

/// O que faz a deformacao de um quadro respirar em vez de ficar parada.
///
/// Sao tres moduladores nomeados, e nao uma funcao livre, porque cada um
/// responde a uma coisa diferente do ator e o nome e a documentacao.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sway {
    /// Numeros fixos.
    None,
    /// Escala oscila com a fase de respiracao do ator.
    Breath,
    /// Inclina proporcional a velocidade horizontal, nao ao olhar.
    Lean,
    /// Estica conforme o `rise` do golpe em curso.
    Rise,
}

/// Como o corpo inteiro se deforma numa pose.
///
/// Squash e stretch vivem so aqui: a fisica nunca ve estes numeros.
#[derive(Clone, Copy, Debug)]
pub struct Body {
    /// Escala do tronco.
    pub scale: Vec2,
    /// Inclinacao em radianos, no sentido em que o boneco olha.
    pub tilt: f32,
    /// O que modula os dois acima.
    pub sway: Sway,
}

impl Body {
    const fn new(sx: f32, sy: f32, tilt: f32, sway: Sway) -> Self {
        Self {
            scale: Vec2::new(sx, sy),
            tilt,
            sway,
        }
    }

    /// Corpo em repouso, sem deformacao nenhuma.
    const STILL: Body = Body::new(1.0, 1.0, 0.0, Sway::None);

    /// Escala e angulo finais, dado o estado do ator neste frame.
    pub fn resolve(&self, at: &Swaying) -> (Vec2, f32) {
        match self.sway {
            Sway::None => (self.scale, at.facing * self.tilt),
            Sway::Breath => (
                Vec2::new(
                    self.scale.x - at.pulse * 0.006,
                    self.scale.y + at.pulse * 0.008,
                ),
                at.facing * self.tilt,
            ),
            // A inclinacao da corrida segue a velocidade, e nao o olhar: quem
            // freia andando de costas se inclina para o outro lado sozinho.
            Sway::Lean => (self.scale, -at.speed * self.tilt),
            // O gancho estica pra cima, a rasteira afunda, a pancada desce. E o
            // `rise` do golpe em curso que decide, entao um quadro so serve os
            // quatro.
            Sway::Rise => (
                Vec2::new(self.scale.x / at.rise, self.scale.y * at.rise),
                at.facing * self.tilt / at.rise,
            ),
        }
    }
}

/// O que a deformacao do corpo le do ator neste frame.
pub struct Swaying {
    /// Seno da respiracao, de -1 a 1.
    pub pulse: f32,
    /// Para que lado o boneco olha.
    pub facing: f32,
    /// Velocidade horizontal, em fracao da corrida cheia.
    pub speed: f32,
    /// `rise` do golpe em curso, ou 1 quando nao ha um.
    pub rise: f32,
}

// --- membros ----------------------------------------------------------------

/// Os pontos articulados de um lado do corpo, em coordenadas locais.
#[derive(Clone, Copy, Debug)]
pub struct Joints {
    pub shoulder: Vec2,
    pub elbow: Vec2,
    pub hand: Vec2,
    pub hip: Vec2,
    pub knee: Vec2,
    pub foot: Vec2,
}

impl Joints {
    /// A passada padrao: e dela que toda pose parte.
    ///
    /// Uma pose que nao diz nada sobre um membro deixa ele aqui, entao um
    /// quadro novo escreve so o que muda -- e nao os seis pontos de novo.
    pub fn gait(r: &Rigging) -> Self {
        let swing = r.gait * r.side * r.facing;
        Self {
            shoulder: Vec2::new(r.side * 2.5, 15.0),
            elbow: Vec2::new(r.side * 7.0 - swing * 5.0, 6.0),
            hand: Vec2::new(r.side * 5.0 - swing * 10.0, -2.0),
            hip: Vec2::new(r.side * 2.2, -7.0),
            knee: Vec2::new(r.side * 5.0 + swing * 7.0, -18.0),
            foot: Vec2::new(r.side * 7.0 + swing * 13.0, -31.0),
        }
    }

    /// Poe os dois bracos numa posicao escrita no sentido do olhar.
    fn reach(&mut self, arm: Arm, facing: f32) {
        self.elbow = Vec2::new(facing * arm.elbow.x, arm.elbow.y);
        self.hand = Vec2::new(facing * arm.hand.x, arm.hand.y);
    }
}

/// O que a pose dos membros le do ator neste frame.
pub struct Rigging {
    /// +1 para o membro da frente, -1 para o de tras.
    pub side: f32,
    /// Para que lado o boneco olha.
    pub facing: f32,
    /// Fase da passada, de -1 a 1.
    pub gait: f32,
    /// Direcao da mira.
    pub aim: Vec2,
    /// Coreografia e fase do golpe em curso.
    pub strike: Option<(&'static Strike, usize)>,
    /// Alcance que a arma na mao acrescenta ao quadro de contato.
    pub reach: f32,
    /// Fase continua da locomocao, em radianos.
    pub cycle: f32,
    /// Velocidade vertical normalizada: positiva subindo, negativa caindo.
    pub air: f32,
}

impl Rigging {
    /// E o membro da frente?
    pub fn front(&self) -> bool {
        self.side > 0.0
    }
}

/// Alcance de um braco totalmente estendido num golpe.
///
/// Serve de referencia para medir o quanto um braco esta comprometido com o
/// soco em curso -- e o que separa o punho que sai do punho que fica na guarda.
const FULL_EXTENT: f32 = 26.0;

/// Nenhum ajuste: os membros ficam onde a passada os colocou.
fn gait(_j: &mut Joints, _r: &Rigging) {}

/// A passada da corrida: joelho alto, calcanhar atras e bracos bombeando.
///
/// A passada padrao balanca os membros so na horizontal -- o pe nunca sai da
/// linha do chao e a mao nunca sobe. Isso e exatamente o desenho de uma
/// caminhada, e era o que a corrida herdava por nao ter ajuste proprio. O que
/// separa correr de andar e a altura: joelho que sobe, pe que recolhe, mao que
/// passa na altura do peito.
fn running(j: &mut Joints, r: &Rigging) {
    // Quanto este lado avanca agora: -1 totalmente atras, +1 totalmente a
    // frente. Nao depende do olhar -- e a perna em relacao ao corpo.
    let leg = r.gait * r.side;
    // O braco do mesmo lado vai ao contrario da perna. E o contrapeso que
    // impede o tronco de girar junto, e sem ele a corrida vira marcha.
    let arm = -leg;
    // Os mesmos avancos, ja em deslocamento horizontal no mundo.
    let stride = leg * r.facing;
    let pump = -stride;

    // Perna da frente sobe o joelho e recolhe o pe; a de tras estica e joga o
    // calcanhar pra cima.
    j.knee = Vec2::new(r.side * 4.0 + stride * 9.0, -17.0 + leg.max(0.0) * 8.0);
    j.foot = Vec2::new(
        r.side * 6.0 + stride * 15.0,
        -31.0 + leg.max(0.0) * 13.0 + (-leg).max(0.0) * 6.0,
    );

    // Cotovelo dobrado junto ao tronco; a mao sobe ate o peito na frente e
    // passa atras do quadril na volta.
    j.elbow = Vec2::new(r.side * 5.0 + pump * 6.0, 6.0 + arm.max(0.0) * 3.0);
    j.hand = Vec2::new(
        r.side * 3.0 + pump * 13.0,
        1.0 + arm.max(0.0) * 9.0 - (-arm).max(0.0) * 4.0,
    );
}

/// Agachado: o corpo inteiro desce, e nao so as pernas.
///
/// A silhueta agachada desenha a cabeca uma linha abaixo e o tronco encolhe
/// para 0,82 da altura -- mas os bracos saiam da passada de pe, entao o boneco
/// agachava com o ombro na altura do peito de quem esta em pe, isto e, acima da
/// propria cabeca. Ombro, cotovelo, mao e quadril descem junto com o tronco.
///
/// Os pes ficam na mesma linha de chao da passada de pe: agachar dobra as
/// pernas, nao levanta o boneco do chao.
fn crouch(j: &mut Joints, r: &Rigging) {
    j.shoulder = Vec2::new(r.side * 2.5, -4.0);
    j.elbow = Vec2::new(r.side * 6.5, -12.0);
    j.hand = Vec2::new(r.side * 5.0 + r.facing * 3.0, -19.0);
    j.hip = Vec2::new(r.side * 2.2, -16.0);
    j.knee = Vec2::new(r.side * 10.0, -20.0);
    j.foot = Vec2::new(r.side * 13.0, -31.0);
}

/// Pernas recolhidas no impulso do salto.
fn tuck(j: &mut Joints, r: &Rigging) {
    let apex = 1.0 - r.air.clamp(0.0, 1.0);
    j.knee = Vec2::new(r.side * (9.0 + apex * 2.0), -13.0 - apex * 3.0);
    j.foot = Vec2::new(r.side * (3.0 + apex * 4.0), -21.0 - apex * 3.0);
}

/// Bracos para cima e joelhos abertos: o corpo se arma para aterrissar.
fn falling(j: &mut Joints, r: &Rigging) {
    let fall = (-r.air).clamp(0.0, 1.0);
    j.elbow.y = 17.0 + fall * 4.0;
    j.hand.y = 24.0 + fall * 7.0;
    j.knee.x *= 1.15 + fall * 0.35;
}

/// Maos alternadas na corrente, uma alta e uma baixa.
///
/// Qual das duas sobe depende do quadro: e a troca entre eles que le como
/// bracada em vez de um boneco pendurado e parado.
fn climb(j: &mut Joints, r: &Rigging) {
    let phase = (r.cycle + if r.front() { 0.0 } else { std::f32::consts::PI }).sin();
    j.elbow = Vec2::new(r.side * 6.0, 16.5 + phase * 7.5);
    j.hand = Vec2::new(r.side * 3.0, 24.5 + phase * 7.5);
    j.knee.x = r.side * (7.0 - phase * 2.0);
    j.foot.x = r.side * (4.0 + phase * 2.0);
}

/// A coreografia do golpe em curso.
///
/// Um `Strike` descreve os dois bracos e, quando o golpe sai da perna, tambem
/// as pernas -- entao golpe novo e uma entrada em `pose`, e esta funcao nao
/// muda uma linha.
fn choreo(j: &mut Joints, r: &Rigging) {
    let Some((strike, phase)) = r.strike else {
        return;
    };
    let frame = if r.front() {
        strike.front[phase]
    } else {
        strike.back[phase]
    };

    // Arma na mao alonga so o quadro de contato, e so o braco que avanca:
    // somar no de guarda esticaria ele para tras.
    let extra = if phase == 1 { r.reach } else { 0.0 };

    // O golpe gira no ombro para acompanhar a mira -- e a metade visual de
    // "o soco segue o mouse": sem ela a hitbox subiria e o braco continuaria
    // deitado, e os dois desenhos discordariam.
    //
    // Cada braco gira o tanto que estiver comprometido com o golpe: o que
    // avanca acompanha o cursor inteiro, o que fica na guarda quase nao se
    // mexe. Sai da propria coreografia, entao o cruzado -- que soca com o braco
    // de tras -- funciona sem a pose precisar dizer qual dos dois esta batendo.
    // O giro sai do ombro de verdade, e nao do meio do peito: a coreografia
    // vive no espaco em que `x` e "para a frente", entao o ombro entra aqui
    // desespelhado. Fora do proprio ombro, girar mudaria o comprimento do
    // braco -- e um membro que estica ao mirar le como erro de desenho.
    let pivot = Vec2::new(j.shoulder.x * r.facing, j.shoulder.y);
    let commitment = (frame.hand.x / FULL_EXTENT).clamp(0.0, 1.0);
    // Gira o angulo pela metade, e nao o ponto: interpolar entre o braco reto e
    // o braco girado encurtaria o membro no meio do caminho, e com a mira para
    // tras ele chegaria a sumir dentro do tronco.
    let swing = Vec2::from_angle(Vec2::new(r.aim.x * r.facing, r.aim.y).to_angle() * commitment);

    let reach = |at: Vec2, scale: f32| {
        let forward = if at.x > 0.0 { extra * scale } else { 0.0 };
        let turned = pivot + swing.rotate(Vec2::new(at.x + forward, at.y) - pivot);
        Vec2::new(r.facing * turned.x, turned.y)
    };
    j.elbow = reach(frame.elbow, 0.5);
    j.hand = reach(frame.hand, 1.0);

    // Golpe de perna: so a da frente varre, a de tras fica de apoio. Sem isso a
    // rasteira seria um soco fraco agachado.
    if let Some(legs) = &strike.legs
        && r.front()
    {
        j.knee = Vec2::new(r.facing * legs[phase].knee.x, legs[phase].knee.y);
        j.foot = Vec2::new(r.facing * legs[phase].foot.x, legs[phase].foot.y);
    }
}

/// Bracos na linha da mira: o da frente estendido, o de tras apoiando mais
/// curto, como quem segura o coice.
fn aiming(j: &mut Joints, r: &Rigging) {
    let anchor = Vec2::new(0.0, if r.front() { 11.0 } else { 9.0 });
    let extent = if r.front() { 21.0 } else { 14.0 };
    j.elbow = anchor + r.aim * extent * 0.5;
    j.hand = anchor + r.aim * extent;
}

/// Guarda alta.
fn parry(j: &mut Joints, r: &Rigging) {
    j.reach(PARRY_ARM, r.facing);
}

/// Bracos jogados para tras pelo impacto.
fn recoil(j: &mut Joints, r: &Rigging) {
    j.reach(Arm::new(-8.0, 20.0, -15.0, 27.0), r.facing);
}

/// Esparramado: tudo desce para perto da linha do chao e os membros abrem para
/// os lados em vez de ficarem sob o corpo.
fn sprawl(j: &mut Joints, r: &Rigging) {
    j.reach(DOWNED_ARM, r.facing);
    j.knee = Vec2::new(r.facing * 12.0 + r.side * 4.0, -26.0);
    j.foot = Vec2::new(r.facing * 24.0 + r.side * 8.0, -31.0);
}

/// Pernas abertas de quem caiu e nao levanta.
fn limp(j: &mut Joints, r: &Rigging) {
    j.knee = Vec2::new(r.side * 13.0, -26.0);
    j.foot = Vec2::new(r.side * 22.0, -29.0);
}

// --- a tabela ---------------------------------------------------------------

/// Tudo que define visualmente uma pose.
pub struct PoseDef {
    /// Silhueta canonica do tronco. Uma pele pode substitui-la.
    pub art: &'static str,
    /// Deformacao do corpo inteiro.
    pub body: Body,
    /// Ajuste dos membros, por cima da passada padrao.
    pub rig: fn(&mut Joints, &Rigging),
    /// Papel de cor do corpo nesta pose. A pele escolhe a cor concreta.
    pub tone: Tone,
    /// A pose tira o controle de quem esta nela?
    pub locks: bool,
    /// A pose manda nos bracos?
    ///
    /// Quando manda, arma na mao nao os puxa para a linha da mira. Sem esta
    /// coluna quem estava caido segurando uma arma tinha o braco na linha da
    /// mira e a arma na mao do chao -- os dois desenhos discordavam.
    pub owns_arms: bool,
}

/// Silhueta de pe, usada pela maioria das poses.
const STANDING: &str = " O>\n | \n | \n   ";

/// Atalho para as poses que so diferem na deformacao do corpo.
const fn standing(body: Body) -> PoseDef {
    PoseDef {
        art: STANDING,
        body,
        rig: gait,
        tone: Tone::Body,
        locks: false,
        owns_arms: false,
    }
}

/// Deformacao do tronco na corrida.
///
/// A inclinacao e o que se ve primeiro: um corpo que corre cai pra frente e as
/// pernas correm atras dele. Enquanto ela foi de tres graus, o boneco podia
/// estar passeando.
const RUNNING_BODY: Body = Body::new(1.04, 0.97, 0.20, Sway::Lean);

/// Um quadro do ciclo de corrida.
const fn sprinting() -> PoseDef {
    PoseDef {
        rig: running,
        ..standing(RUNNING_BODY)
    }
}

/// A tabela, na ordem do enum. `def` indexa por discriminante.
const POSES: [(Pose, PoseDef); Pose::COUNT] = [
    (
        Pose::IdleA,
        standing(Body::new(1.0, 1.0, 0.0, Sway::Breath)),
    ),
    (
        Pose::IdleB,
        standing(Body::new(1.0, 1.0, 0.0, Sway::Breath)),
    ),
    (Pose::RunA, sprinting()),
    (Pose::RunB, sprinting()),
    (Pose::RunC, sprinting()),
    (Pose::RunD, sprinting()),
    (Pose::RunE, sprinting()),
    (Pose::RunF, sprinting()),
    (
        Pose::Crouch,
        PoseDef {
            art: "   \n O>\n_|_\n   ",
            body: Body::new(1.10, 0.82, 0.0, Sway::None),
            rig: crouch,
            ..standing(Body::STILL)
        },
    ),
    (
        Pose::Jump,
        PoseDef {
            body: Body::new(0.93, 1.10, -0.035, Sway::None),
            rig: tuck,
            ..standing(Body::STILL)
        },
    ),
    (
        Pose::Fall,
        PoseDef {
            body: Body::new(1.08, 0.94, 0.025, Sway::None),
            rig: falling,
            ..standing(Body::STILL)
        },
    ),
    (
        Pose::ClimbA,
        PoseDef {
            rig: climb,
            owns_arms: true,
            ..standing(Body::STILL)
        },
    ),
    (
        Pose::ClimbB,
        PoseDef {
            rig: climb,
            owns_arms: true,
            ..standing(Body::STILL)
        },
    ),
    (
        Pose::PunchWindup,
        PoseDef {
            body: Body::new(0.96, 1.03, -0.10, Sway::None),
            rig: choreo,
            owns_arms: true,
            ..standing(Body::STILL)
        },
    ),
    (
        Pose::PunchStrike,
        PoseDef {
            body: Body::new(1.13, 0.94, 0.075, Sway::Rise),
            rig: choreo,
            owns_arms: true,
            ..standing(Body::STILL)
        },
    ),
    (
        Pose::PunchRecover,
        PoseDef {
            rig: choreo,
            owns_arms: true,
            ..standing(Body::STILL)
        },
    ),
    (
        Pose::Shoot,
        PoseDef {
            body: Body::new(1.13, 0.94, 0.075, Sway::None),
            rig: aiming,
            owns_arms: true,
            ..standing(Body::STILL)
        },
    ),
    (
        Pose::Parry,
        PoseDef {
            art: " O>\n[| \n | \n   ",
            body: Body::new(1.08, 0.96, -0.06, Sway::None),
            rig: parry,
            locks: true,
            owns_arms: true,
            ..standing(Body::STILL)
        },
    ),
    (
        Pose::Hit,
        PoseDef {
            art: " o<\n | \n | \n   ",
            body: Body::new(1.10, 0.91, -0.12, Sway::None),
            rig: recoil,
            tone: Tone::Hurt,
            locks: true,
            ..standing(Body::STILL)
        },
    ),
    (
        // Caido, mas ainda por cima da linha do chao: o que separa isto de
        // `Dead` e uma linha, e e a linha que diz que ele vai levantar.
        Pose::Downed,
        PoseDef {
            art: "   \n   \n o_\n\u{2500}\u{2500}\u{2500}",
            body: Body::new(1.32, 0.62, -0.18, Sway::None),
            rig: sprawl,
            tone: Tone::Hurt,
            locks: true,
            owns_arms: true,
        },
    ),
    (
        Pose::Dead,
        PoseDef {
            art: "   \n   \n   \n_o_",
            body: Body::new(1.08, 0.88, 0.0, Sway::None),
            rig: limp,
            tone: Tone::Gone,
            locks: true,
            ..standing(Body::STILL)
        },
    ),
];

/// A linha da tabela desta pose.
pub fn def(pose: Pose) -> &'static PoseDef {
    &POSES[pose as usize].1
}

// --- pontos que outros sistemas leem da pose --------------------------------

/// Os glifos que desenham a cabeca nas silhuetas canonicas.
///
/// Sao dois porque ela muda de desenho quando o boneco apanha -- `O` vira `o`
/// --, e nao porque haja escolha: uma pele troca o glifo da cabeca por `swap`,
/// depois de a geometria ja estar decidida.
const HEAD_GLYPHS: [char; 2] = ['O', 'o'];

/// Linha e coluna da cabeca numa silhueta.
///
/// Cai na celula de cima, no meio, quando a arte nao tem cabeca nenhuma -- que
/// e onde um boneco de pe a tem. Nenhuma silhueta do jogo cai nesse caso, e
/// `toda_silhueta_tem_uma_cabeca` cobra isso.
pub fn head_at(art: &str) -> (u16, u16) {
    art.lines()
        .enumerate()
        .find_map(|(row, line)| {
            line.chars()
                .position(|ch| HEAD_GLYPHS.contains(&ch))
                .map(|col| (row as u16, col as u16))
        })
        .unwrap_or((0, BODY_COLS / 2))
}

/// Centro da cabeca desta pose, no espaco do sprite do corpo.
///
/// Sai da propria silhueta em vez de uma coluna a parte da tabela: a pose que
/// desenha a cabeca uma linha abaixo -- agachar, cair, morrer -- leva o rosto
/// junto sem escrever numero nenhum, e nao ha um segundo numero para sair de
/// sincronia com a arte.
///
/// O sprite do corpo e ancorado nos pes, entao a conta sobe da base da arte.
pub fn head_cell(pose: Pose) -> Vec2 {
    let (row, col) = head_at(def(pose).art);
    Vec2::new(
        (col as f32 - (BODY_COLS - 1) as f32 * 0.5) * CELL.x,
        (BODY_ROWS - row) as f32 * CELL.y - CELL.y * 0.5,
    )
}

/// De onde sai um membro que segue a mira, nesta pose.
///
/// E o ombro da propria pose, montado com a passada parada e descido ate o
/// peito. A arma que a mao segura le o mesmo ponto: enquanto os dois foram um
/// `8.0` escrito a mao em dois arquivos, agachar descia o braco e deixava a
/// arma flutuando na altura do peito de quem esta em pe.
pub fn aim_anchor(pose: Pose) -> Vec2 {
    let rigging = Rigging {
        side: 1.0,
        facing: 1.0,
        gait: 0.0,
        aim: Vec2::X,
        strike: None,
        reach: 0.0,
        cycle: 0.0,
        air: 0.0,
    };
    let mut joints = Joints::gait(&rigging);
    (def(pose).rig)(&mut joints, &rigging);
    Vec2::new(0.0, joints.shoulder.y - 7.0)
}

/// Todas as poses, na ordem da tabela.
///
/// Vem da tabela, e nao de uma lista escrita a mao: enquanto as duas eram
/// separadas, uma pose nova podia existir no jogo e faltar nos testes de
/// varredura -- que foi o que aconteceu com `Downed`. O jogo nunca precisa da
/// lista porque ele sempre tem uma pose especifica em mao; quem varre o
/// repertorio inteiro sao os testes.
#[cfg(test)]
pub fn all() -> impl Iterator<Item = Pose> {
    POSES.iter().map(|(pose, _)| *pose)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `def` indexa a tabela pelo discriminante da pose. Uma linha fora de
    /// lugar daria a uma pose a deformacao e os membros de outra, calada.
    #[test]
    fn a_tabela_segue_a_ordem_do_enum() {
        for (i, (pose, _)) in POSES.iter().enumerate() {
            assert_eq!(*pose as usize, i, "{pose:?} fora de lugar na tabela");
        }
    }

    /// Uma pose nao pode estar em dois clipes: `frame` devolveria o quadro de
    /// um deles e a animacao andaria pelo outro.
    #[test]
    fn nenhuma_pose_esta_em_dois_clipes() {
        for pose in all() {
            let clipes = CLIPS
                .iter()
                .filter(|clip| clip.index_of(pose).is_some())
                .count();
            assert!(clipes <= 1, "{pose:?} aparece em {clipes} clipes");
        }
    }

    /// Todo quadro de um clipe tem que estar na tabela, e o clipe tem que dar a
    /// volta no lugar certo.
    #[test]
    fn os_clipes_giram_no_proprio_tamanho() {
        for clip in CLIPS {
            assert!(clip.len() > 0);
            assert_eq!(clip.at(clip.len()), clip.at(0));
            for i in 0..clip.len() {
                assert_eq!(clip.index_of(clip.at(i)), Some(i));
            }
        }
    }

    /// O ciclo de corrida tem que fechar com degraus iguais.
    ///
    /// A tabela escrita a mao que existia aqui andava -1, -0.6, 0, 0.6, 1, 0.35
    /// e pulava 1.35 ao dar a volta -- mais que o dobro do maior degrau normal,
    /// o que faz a perna dar um tranco toda passada. Nada no jogo reclamava
    /// disso; era so feio.
    #[test]
    fn o_ciclo_de_corrida_fecha_com_passo_uniforme() {
        let fases: Vec<f32> = (0..RUN_FRAMES).map(run_gait).collect();
        let degraus: Vec<f32> = (0..RUN_FRAMES)
            .map(|i| (fases[(i + 1) % RUN_FRAMES] - fases[i]).abs())
            .collect();
        let maior = degraus.iter().cloned().fold(f32::MIN, f32::max);
        let menor = degraus.iter().cloned().fold(f32::MAX, f32::min);
        assert!(
            (maior - menor).abs() < 0.01,
            "degraus desiguais: {degraus:?} para {fases:?}"
        );
    }

    /// A passada tem que varrer o balanco inteiro, nos dois sentidos: um ciclo
    /// que so vai pra frente anima um boneco mancando.
    #[test]
    fn a_passada_vai_e_volta() {
        let fases: Vec<f32> = (0..RUN_FRAMES).map(run_gait).collect();
        assert!(fases.iter().cloned().fold(f32::MIN, f32::max) >= 0.99);
        assert!(fases.iter().cloned().fold(f32::MAX, f32::min) <= -0.99);
        assert_eq!(run_gait(RUN_FRAMES), run_gait(0));
    }

    /// So quem tira o controle pode pintar de dano ou de morte: se uma pose
    /// jogavel virasse `Hurt`, o jogador ficaria vermelho podendo agir e a cor
    /// pararia de significar "esta apanhando".
    #[test]
    fn a_cor_de_dano_so_aparece_em_pose_travada() {
        for pose in all() {
            let def = def(pose);
            if def.tone != Tone::Body {
                assert!(def.locks, "{pose:?} pinta de dano mas deixa agir");
            }
        }
    }

    /// A passada so pode mexer o corpo em quem esta correndo. Uma pose parada
    /// que lesse a fase da passada moveria a perna sozinha.
    #[test]
    fn so_a_corrida_se_inclina_com_a_velocidade() {
        for pose in all() {
            assert_eq!(
                def(pose).body.sway == Sway::Lean,
                RUN.index_of(pose).is_some(),
                "{pose:?} discorda sobre estar correndo"
            );
        }
    }

    /// Toda silhueta tem que ter uma cabeca, e uma so.
    ///
    /// E a celula dela que diz onde vai o rosto e qual celula a silhueta nao
    /// desenha. Uma arte sem cabeca cairia no palpite de [`head_at`] -- a
    /// celula de cima -- e o rosto de quem esta caido ficaria flutuando onde
    /// estaria a cabeca de quem esta de pe, com o circulo de volta no chao.
    #[test]
    fn toda_silhueta_tem_uma_cabeca() {
        for pose in all() {
            let cabecas = def(pose)
                .art
                .chars()
                .filter(|ch| HEAD_GLYPHS.contains(ch))
                .count();
            assert_eq!(cabecas, 1, "{pose:?} tem {cabecas} cabecas na silhueta");
        }
    }

    /// O rosto acompanha a cabeca que a silhueta desenha.
    ///
    /// Agachar, cair e morrer baixam a cabeca uma linha ou mais. Enquanto o
    /// rosto teve altura propria, ele ficava parado onde a cabeca de quem esta
    /// de pe fica -- o boneco agachava e a cara continuava no ar.
    #[test]
    fn a_cabeca_desce_com_a_silhueta() {
        let y = |pose| head_cell(pose).y;
        assert!(y(Pose::Crouch) < y(Pose::IdleA), "agachar nao baixa a cabeca");
        assert!(y(Pose::Downed) < y(Pose::Crouch), "cair nao baixa a cabeca");
        assert!(y(Pose::Dead) < y(Pose::Downed), "morrer nao baixa a cabeca");

        // E ela cai dentro da caixa da arte, medida a partir dos pes.
        for pose in all() {
            let head = head_cell(pose);
            assert!(
                head.y > 0.0 && head.y < BODY_ROWS as f32 * CELL.y,
                "{pose:?} poe a cabeca fora da arte: {head:?}"
            );
            assert!(
                head.x.abs() < BODY_COLS as f32 * CELL.x * 0.5,
                "{pose:?} poe a cabeca fora da arte: {head:?}"
            );
        }
    }

    /// Agachar tem que baixar o corpo inteiro, e nao so as pernas.
    ///
    /// Enquanto `crouch` escrevia so joelho e pe, o ombro ficava na altura de
    /// quem esta em pe -- acima da cabeca agachada -- e os bracos pairavam ao
    /// lado do boneco. Os pes ficam onde a passada parada os deixa: agachar
    /// dobra a perna, nao levanta o boneco do chao.
    #[test]
    fn agachar_abaixa_o_corpo_inteiro() {
        for side in [-1.0, 1.0] {
            let rigging = Rigging {
                side,
                facing: 1.0,
                gait: 0.0,
                aim: Vec2::X,
                strike: None,
                reach: 0.0,
                cycle: 0.0,
                air: 0.0,
            };
            let de_pe = Joints::gait(&rigging);
            let mut agachado = de_pe;
            crouch(&mut agachado, &rigging);

            for (nome, em_pe, baixo) in [
                ("ombro", de_pe.shoulder.y, agachado.shoulder.y),
                ("cotovelo", de_pe.elbow.y, agachado.elbow.y),
                ("mao", de_pe.hand.y, agachado.hand.y),
                ("quadril", de_pe.hip.y, agachado.hip.y),
            ] {
                assert!(
                    baixo < em_pe - 2.0,
                    "o {nome} nao desce ao agachar: {em_pe} -> {baixo}"
                );
            }
            assert_eq!(
                agachado.foot.y, de_pe.foot.y,
                "os pes sairam do chao ao agachar"
            );
            // O joelho de um agachamento nao desce: ele abre pra fora,
            // enquanto o quadril e que vem ao encontro dele.
            assert!(
                agachado.knee.x.abs() > de_pe.knee.x.abs() + 2.0,
                "o joelho nao abre ao agachar: {} -> {}",
                de_pe.knee.x,
                agachado.knee.x
            );
        }
    }

    /// Monta o quadro de contato de um golpe, mirando em `aim`.
    fn socando(aim: Vec2, side: f32) -> Joints {
        let rigging = Rigging {
            side,
            facing: 1.0,
            gait: 0.0,
            aim,
            strike: Some((crate::actor::pose::strike(0), 1)),
            reach: 0.0,
            cycle: 0.0,
            air: 0.0,
        };
        let mut joints = Joints::gait(&rigging);
        choreo(&mut joints, &rigging);
        joints
    }

    /// O punho tem que ir para onde a mira aponta.
    ///
    /// E a metade visual de uma regra que o combate ja aplica na hitbox. Sem
    /// ela o golpe abre no alto e o braco continua deitado -- dois desenhos do
    /// mesmo soco discordando na mesma tela.
    #[test]
    fn o_soco_acompanha_a_mira() {
        let reto = socando(Vec2::X, 1.0);
        let alto = socando(Vec2::Y, 1.0);
        let baixo = socando(Vec2::NEG_Y, 1.0);

        assert!(reto.hand.x > 20.0, "o jab parou de sair para a frente");
        assert!(
            alto.hand.y > reto.hand.y + 15.0,
            "mirar para cima nao levantou o punho: {:?}",
            alto.hand
        );
        assert!(
            baixo.hand.y < reto.hand.y - 15.0,
            "mirar para baixo nao baixou o punho: {:?}",
            baixo.hand
        );

        // Girar no ombro nao pode esticar nem encolher o braco: o membro e um
        // sprite de comprimento fixo, e um braco que cresce le como erro.
        let braco = |j: &Joints| (j.hand - j.shoulder).length();
        for (nome, girado) in [("alto", &alto), ("baixo", &baixo)] {
            assert!(
                (braco(girado) - braco(&reto)).abs() < 0.5,
                "o braco mudou de tamanho mirando {nome}"
            );
        }
    }

    /// So o braco que soca acompanha a mira; o da guarda fica onde esta.
    ///
    /// O quanto cada braco gira sai da propria coreografia -- de o punho estar
    /// avancado ou recolhido -- e nao de ser o da frente ou o de tras. E o que
    /// faz o cruzado, que soca com o braco de tras, funcionar sem uma segunda
    /// regra escrita a parte.
    #[test]
    fn o_braco_de_guarda_nao_gira_junto() {
        let guarda_reta = socando(Vec2::X, -1.0);
        let guarda_alta = socando(Vec2::Y, -1.0);
        assert!(
            (guarda_alta.hand - guarda_reta.hand).length() < 1.0,
            "a guarda seguiu o cursor junto com o soco"
        );
    }

    /// A linha da mira sai do ombro da pose, e nao de um numero fixo: quem
    /// agacha com arma na mao segura ela na altura do proprio peito.
    #[test]
    fn a_mira_desce_junto_com_o_ombro() {
        assert!(
            aim_anchor(Pose::Crouch).y < aim_anchor(Pose::IdleA).y - 5.0,
            "agachado, a arma continua na altura de quem esta em pe"
        );
    }

    /// A passada padrao tem que ser simetrica: com a fase zerada, os dois lados
    /// do corpo caem no mesmo lugar espelhado. Sem isso o boneco nasce torto.
    #[test]
    fn a_passada_parada_e_simetrica() {
        let make = |side: f32| {
            Joints::gait(&Rigging {
                side,
                facing: 1.0,
                gait: 0.0,
                aim: Vec2::X,
                strike: None,
                reach: 0.0,
                cycle: 0.0,
                air: 0.0,
            })
        };
        let (frente, tras) = (make(1.0), make(-1.0));
        assert_eq!(frente.hand.x, -tras.hand.x);
        assert_eq!(frente.foot.x, -tras.foot.x);
        assert_eq!(frente.hand.y, tras.hand.y);
    }

    /// Correr nao e andar depressa: o que separa os dois e a altura.
    ///
    /// A passada padrao balanca os membros so na horizontal. Enquanto a
    /// corrida herdou esse ajuste, o pe nunca saia da linha do chao e a mao
    /// nunca subia -- seis quadros de uma caminhada acelerada.
    #[test]
    fn a_corrida_tira_o_pe_do_chao() {
        let quadro = |gait: f32, side: f32| {
            let rig = Rigging {
                side,
                facing: 1.0,
                gait,
                aim: Vec2::X,
                strike: None,
                reach: 0.0,
                cycle: 0.0,
                air: 0.0,
            };
            let mut joints = Joints::gait(&rig);
            running(&mut joints, &rig);
            joints
        };

        // Varre o ciclo inteiro e mede o quanto cada ponta sobe e desce.
        let ciclo: Vec<Joints> = (0..24)
            .map(|i| quadro(-(i as f32 / 24.0 * std::f32::consts::TAU).cos(), 1.0))
            .collect();
        let curso = |ponta: fn(&Joints) -> f32| {
            let alto = ciclo.iter().map(ponta).fold(f32::MIN, f32::max);
            let baixo = ciclo.iter().map(ponta).fold(f32::MAX, f32::min);
            alto - baixo
        };
        assert!(curso(|j| j.foot.y) > 8.0, "o pe nao sai do chao");
        assert!(curso(|j| j.hand.y) > 8.0, "a mao nao sobe");

        // Uma perna na frente e a outra atras: se as duas ficam na mesma
        // altura, o boneco corre de pernas juntas.
        assert!(
            quadro(1.0, 1.0).knee.y > quadro(1.0, -1.0).knee.y,
            "as duas pernas correm iguais"
        );
    }

    /// Quem corre cai pra frente e deixa as pernas alcancarem o corpo.
    #[test]
    fn a_corrida_cai_pra_frente() {
        let (_, angle) = def(Pose::RunA).body.resolve(&Swaying {
            pulse: 0.0,
            facing: 1.0,
            speed: 1.0,
            rise: 1.0,
        });
        assert!(angle.abs() > 0.12, "a corrida mal se inclina: {angle}");
    }

    #[test]
    fn escalada_fecha_um_ciclo_continuo() {
        let hand = |cycle: f32| {
            let rig = Rigging {
                side: 1.0,
                facing: 1.0,
                gait: 0.0,
                aim: Vec2::Y,
                strike: None,
                reach: 0.0,
                cycle,
                air: 0.0,
            };
            let mut joints = Joints::gait(&rig);
            climb(&mut joints, &rig);
            joints.hand.y
        };
        assert!((hand(0.0) - hand(std::f32::consts::TAU)).abs() < 0.001);
        assert!(hand(0.1) > hand(0.0), "a bracada deveria subir sem degrau");
    }
}
