//! Rosto: as pecas soltas que ocupam a celula da cabeca.
//!
//! A cabeca e uma celula so -- 8x16 -- e olho, nariz e boca nao cabem dentro
//! dela como texto. Mas glifo aqui nao e celula de terminal: e sprite com
//! `Transform` proprio. Entao o rosto e montado do mesmo jeito que os oito
//! membros ja sao, com pecas **menores que uma celula** posicionadas em espaco
//! continuo dentro dela. O circulo que desenhava a cabeca saiu: o rosto *e* a
//! cabeca agora, e quem desenha aquela celula e so ele -- por isso
//! [`Skin::draw`](super::skin::Skin::draw) nao pinta o glifo de cabeca.
//!
//! As pecas sao filhas do sprite do corpo, e nao da raiz do ator: e o corpo que
//! carrega inclinacao, respiracao e squash/stretch da pose, e o rosto tem que
//! ir junto. Enquanto elas penderam da raiz, o boneco corria inclinado com a
//! cara reta e agachava com o rosto parado onde a cabeca estava de pe.
//!
//! A divisao e a mesma que separa pele de pose: o **catalogo** e gosto de quem
//! joga (que olho, que cabelo), a **expressao** e do momento (batendo,
//! apanhando, morto). Uma expressao so sobrescreve as pecas que ela realmente
//! mexe -- do mesmo jeito que uma pose que nao fala de um membro deixa ele na
//! passada padrao. Por isso expressao nova nao precisa reescrever o rosto
//! inteiro, e cabelo escolhido nao vira sorriso sem querer.

use bevy::prelude::*;

use super::pose::Pose;
use super::rig;
use super::skin::ActorSkin;
use super::{ActorLimb, Flash};
use crate::ascii::{AsciiArt, AsciiSprite, Layer};

/// Z do rosto, medido a partir do sprite do corpo.
///
/// O corpo esta na camada dos atores (z 30), e os membros da frente em 30.4:
/// esta folga poe o rosto acima dos dois sem tirar o boneco da camada dele.
const FACE_Z: f32 = 0.6;

/// Uma peca do rosto.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct FacePart {
    pub owner: Entity,
    pub part: Part,
    /// -1 e +1 nas pecas duplas, 0 nas unicas.
    pub side: f32,
}

/// Que peca do rosto e esta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Hair,
    Brow,
    Eye,
    Nose,
    Mouth,
}

impl Part {
    pub const ALL: [Part; 5] = [Part::Hair, Part::Brow, Part::Eye, Part::Nose, Part::Mouth];

    /// As pecas que o jogador escolhe, na ordem em que o seletor as lista.
    ///
    /// A sobrancelha fica de fora: ela e da expressao, nao do gosto.
    pub const CHOSEN: [Part; 4] = [Part::Hair, Part::Eye, Part::Nose, Part::Mouth];

    /// Nome exibido no seletor.
    pub fn label(self) -> &'static str {
        match self {
            Part::Hair => "HAIR",
            Part::Brow => "BROW",
            Part::Eye => "EYES",
            Part::Nose => "NOSE",
            Part::Mouth => "MOUTH",
        }
    }

    /// As opcoes desta peca.
    pub fn catalog(self) -> &'static [Look] {
        match self {
            Part::Hair => &HAIR,
            Part::Eye => &EYES,
            Part::Nose => &NOSE,
            Part::Mouth => &MOUTH,
            Part::Brow => std::slice::from_ref(&BROW),
        }
    }

    /// Peca dupla tem uma copia de cada lado; peca unica, so uma no meio.
    pub fn sides(self) -> &'static [f32] {
        match self {
            Part::Brow | Part::Eye => &[-1.0, 1.0],
            _ => &[0.0],
        }
    }

    /// Onde ela fica, medida do centro da cabeca.
    ///
    /// Em peca dupla o `x` e a distancia ate o meio, espelhada pelo lado. Em
    /// peca unica ele acompanha o olhar, que e o que faz o nariz apontar pra
    /// frente em vez de ficar no centro do rosto de perfil.
    pub fn anchor(self) -> Vec2 {
        match self {
            Part::Hair => Vec2::new(0.0, 5.2),
            Part::Brow => Vec2::new(1.9, 2.4),
            Part::Eye => Vec2::new(1.9, 0.6),
            Part::Nose => Vec2::new(1.2, -1.6),
            Part::Mouth => Vec2::new(0.3, -3.6),
        }
    }
}

/// Uma opcao de catalogo para uma peca do rosto.
pub struct Look {
    /// Nome exibido no seletor.
    pub name: &'static str,
    /// Glifo em repouso. Espaco desenha nada, que e como "nenhum" existe.
    pub glyph: char,
    /// Tamanho em relacao a celula cheia. O rosto inteiro cabe numa celula,
    /// entao toda peca e uma fracao dela.
    pub scale: f32,
}

const fn look(name: &'static str, glyph: char, scale: f32) -> Look {
    Look { name, glyph, scale }
}

/// Cabelos disponiveis.
///
/// E a peca que veste a cor do jogador. O circulo da cabeca fazia esse papel
/// enquanto era desenhado; com ele fora, o cabelo e a unica peca larga o
/// bastante para dizer de quem e o boneco a distancia. Quem escolhe `BALD`
/// abre mao disso e fica so com a faixa da silhueta.
pub const HAIR: [Look; 5] = [
    look("BALD", ' ', 0.5),
    look("SPIKE", '^', 0.62),
    look("MOP", '\u{2580}', 0.5),
    look("CREST", '\u{2248}', 0.55),
    look("MANE", '\u{2593}', 0.5),
];

/// Olhos disponiveis.
pub const EYES: [Look; 5] = [
    look("DOTS", '\u{00B7}', 0.55),
    look("WIDE", '\u{00B0}', 0.5),
    look("BEADY", '\u{2219}', 0.45),
    look("SHARP", '\u{00AC}', 0.45),
    look("TIRED", '-', 0.45),
];

/// Narizes disponiveis.
pub const NOSE: [Look; 4] = [
    look("NONE", ' ', 0.45),
    look("TICK", '\u{00B7}', 0.45),
    look("BEAK", '>', 0.45),
    look("SNUB", 'v', 0.4),
];

/// Bocas disponiveis.
pub const MOUTH: [Look; 5] = [
    look("LINE", '-', 0.5),
    look("GRIN", 'u', 0.45),
    look("FLAT", '_', 0.5),
    look("OPEN", 'o', 0.45),
    look("SMIRK", '\u{2310}', 0.45),
];

/// Sobrancelha em repouso.
///
/// Nao tem catalogo de proposito: ela e do momento, nao do gosto de quem joga
/// -- e a peca que carrega quase toda a expressao. Um jogador que escolhesse a
/// propria sobrancelha ficaria bravo o tempo todo, ou nunca.
const BROW: Look = look("BROW", '-', 0.5);

/// A combinacao escolhida por um jogador.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Face {
    pub hair: usize,
    pub eyes: usize,
    pub nose: usize,
    pub mouth: usize,
}

impl Default for Face {
    fn default() -> Self {
        Self {
            hair: 1,
            eyes: 0,
            nose: 1,
            mouth: 0,
        }
    }
}

impl Face {
    /// Um rosto diferente por lugar, para uma sala cheia ja nascer distinguivel
    /// sem ninguem precisar mexer no seletor.
    ///
    /// O cabelo comeca no seguinte da lista para ninguem nascer careca: como e
    /// ele quem veste a cor do jogador, o lugar 0 entraria em campo sem acento
    /// nenhum -- e quem visse isso leria como boneco sem cor, e nao como
    /// escolha. Careca continua sendo uma opcao, so nao o padrao de ninguem.
    pub fn variety(id: u8) -> Self {
        let id = id as usize;
        Self {
            hair: (id + 1) % HAIR.len(),
            eyes: id % EYES.len(),
            nose: id % NOSE.len(),
            mouth: id % MOUTH.len(),
        }
    }

    /// O visual escolhido para uma peca. A sobrancelha nao tem escolha.
    pub fn look(&self, part: Part) -> &'static Look {
        let catalog = part.catalog();
        &catalog[self.pick(part) % catalog.len()]
    }

    /// Indice escolhido para a peca.
    pub fn pick(&self, part: Part) -> usize {
        match part {
            Part::Hair => self.hair,
            Part::Eye => self.eyes,
            Part::Nose => self.nose,
            Part::Mouth => self.mouth,
            Part::Brow => 0,
        }
    }

    /// Escolhe a opcao da peca, girando o indice dentro do catalogo.
    pub fn set(&mut self, part: Part, at: usize) {
        let at = at % part.catalog().len();
        match part {
            Part::Hair => self.hair = at,
            Part::Eye => self.eyes = at,
            Part::Nose => self.nose = at,
            Part::Mouth => self.mouth = at,
            Part::Brow => {}
        }
    }

    /// Gira a escolha da peca, aceitando passo negativo.
    pub fn cycle(&mut self, part: Part, step: i32) {
        let len = part.catalog().len() as i32;
        self.set(
            part,
            (self.pick(part) as i32 + step).rem_euclid(len) as usize,
        );
    }

    /// Um indice por peca escolhida, para caber num pacote.
    pub fn to_bytes(self) -> [u8; FACE_BYTES] {
        let mut out = [0; FACE_BYTES];
        for (byte, part) in out.iter_mut().zip(Part::CHOSEN) {
            *byte = self.pick(part) as u8;
        }
        out
    }

    /// Le um rosto de um pacote.
    ///
    /// Indice fora do catalogo gira em vez de ser recusado: um cliente de outra
    /// build manda numero maior, e nascer com um cabelo estranho e melhor que
    /// derrubar a partida por causa de cosmetico.
    pub fn from_bytes(data: &[u8]) -> Face {
        let mut face = Face::default();
        for (part, byte) in Part::CHOSEN.into_iter().zip(data) {
            face.set(part, *byte as usize);
        }
        face
    }
}

/// Bytes que um rosto ocupa num pacote: um indice por peca escolhida.
pub const FACE_BYTES: usize = Part::CHOSEN.len();

/// O que o rosto esta fazendo agora.
///
/// Vem do estado do jogo, e nao de escolha. `Pose` ja e a leitura canonica do
/// que o boneco esta fazendo, entao a expressao sai dela: um lugar so decide, e
/// nenhum sistema precisa perguntar de novo se o jogador esta apanhando.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expression {
    /// Parado, andando, correndo.
    Calm,
    /// Batendo ou atirando.
    Fierce,
    /// Esforco sem agressao: aparar, pular, escalar.
    Strain,
    /// Apanhando ou no chao.
    Hurt,
    /// Morto.
    Gone,
}

impl Expression {
    /// Quantas existem. E o discriminante da ultima mais um, entao a tabela
    /// quebra a compilacao se uma expressao nova nao receber a linha dela.
    pub const COUNT: usize = Expression::Gone as usize + 1;

    /// Todas, na ordem da tabela.
    ///
    /// So os testes precisam dela: o jogo sempre tem uma expressao especifica
    /// em mao, e quem varre o repertorio inteiro e a conferencia de glifos.
    #[cfg(test)]
    pub const ALL: [Expression; Expression::COUNT] = [
        Expression::Calm,
        Expression::Fierce,
        Expression::Strain,
        Expression::Hurt,
        Expression::Gone,
    ];

    /// A expressao que esta pose pede.
    pub fn of(pose: Pose) -> Expression {
        match pose {
            Pose::Dead => Expression::Gone,
            Pose::Hit | Pose::Downed => Expression::Hurt,
            Pose::PunchWindup | Pose::PunchStrike | Pose::PunchRecover | Pose::Shoot => {
                Expression::Fierce
            }
            Pose::Parry | Pose::Jump | Pose::Fall | Pose::ClimbA | Pose::ClimbB => {
                Expression::Strain
            }
            _ => Expression::Calm,
        }
    }
}

/// O que uma expressao faz com o rosto.
///
/// Ela nao lista o rosto inteiro: so sobrescreve o que realmente mexe. Uma
/// expressao que nao fala de boca deixa a que o jogador escolheu.
pub struct FaceDef {
    /// Sobrancelha -- sempre definida, porque e ela que carrega a expressao.
    pub brow: char,
    /// Inclinacao da sobrancelha, em radianos. Positivo franze para dentro.
    pub brow_tilt: f32,
    /// Altura extra da sobrancelha.
    pub brow_lift: f32,
    /// Trocam o glifo escolhido, quando a expressao manda na peca.
    pub eye: Option<char>,
    pub mouth: Option<char>,
}

const CALM: FaceDef = FaceDef {
    brow: '-',
    brow_tilt: 0.0,
    brow_lift: 0.0,
    eye: None,
    mouth: None,
};

/// A tabela, na ordem do enum. [`def`] indexa por discriminante.
const FACES: [(Expression, FaceDef); Expression::COUNT] = [
    (Expression::Calm, CALM),
    (
        // Franzido e boca aberta: quem esta batendo grita.
        Expression::Fierce,
        FaceDef {
            brow_tilt: 0.42,
            brow_lift: -0.7,
            mouth: Some('o'),
            ..CALM
        },
    ),
    (
        // Sobrancelha alta e boca fechada: esforco, nao raiva.
        Expression::Strain,
        FaceDef {
            brow_tilt: -0.28,
            brow_lift: 0.6,
            mouth: Some('-'),
            ..CALM
        },
    ),
    (
        // Olho apertado e boca aberta.
        Expression::Hurt,
        FaceDef {
            brow: '~',
            brow_tilt: -0.5,
            brow_lift: 0.3,
            eye: Some('-'),
            mouth: Some('o'),
        },
    ),
    (
        // Sem sobrancelha, olhos riscados.
        Expression::Gone,
        FaceDef {
            brow: ' ',
            brow_tilt: 0.0,
            brow_lift: 0.0,
            eye: Some('x'),
            mouth: Some('_'),
        },
    ),
];

/// A linha da tabela desta expressao.
pub fn def(expression: Expression) -> &'static FaceDef {
    &FACES[expression as usize].1
}

/// Monta as pecas do rosto de um ator.
///
/// Elas nascem penduradas no `body` -- o sprite da silhueta -- e nao na raiz:
/// e assim que o rosto herda de graca a inclinacao e o squash/stretch que a
/// pose da ao corpo. O `owner` continua sendo a raiz, que e quem carrega pose,
/// olhar e pele.
pub(crate) fn spawn_face(commands: &mut Commands, root: Entity, body: Entity, face: Face) {
    commands.entity(root).insert(face);
    for part in Part::ALL {
        for side in part.sides().iter().copied() {
            commands.spawn((
                FacePart {
                    owner: root,
                    part,
                    side,
                },
                AsciiSprite::new(AsciiArt::glyph(' ', Color::NONE)),
                Layer::Actor,
                Transform::default(),
                ChildOf(body),
            ));
        }
    }
}

/// Poe cada peca no lugar e escolhe o glifo dela.
///
/// A arte so e reescrita quando muda de verdade: escrever todo quadro marcaria
/// o sprite como sujo e faria o rebuild de glifos rodar a toa para seis pecas
/// vezes quatro bonecos.
pub(crate) fn animate_face(
    actors: Query<
        (
            &Pose,
            &super::Facing,
            &Face,
            &ActorSkin,
            &super::ActorTint,
            &Flash,
        ),
        Without<ActorLimb>,
    >,
    mut parts: Query<(&FacePart, &mut Transform, &mut AsciiSprite)>,
) {
    for (piece, mut transform, mut sprite) in &mut parts {
        let Ok((pose, facing, face, skin, tint, flash)) = actors.get(piece.owner) else {
            continue;
        };
        let def = def(Expression::of(*pose));
        let look = face.look(piece.part);
        let anchor = piece.part.anchor();
        // Onde a silhueta desta pose poe a cabeca. Agachar, cair e morrer
        // desenham ela mais embaixo, e o rosto vai junto sem numero proprio.
        // O `facing` espelha a coluna do mesmo jeito que o sprite do corpo
        // espelha a arte ao virar.
        let head = rig::head_cell(*pose) * Vec2::new(facing.0, 1.0);

        // Peca dupla espelha pelo lado e ignora o olhar; peca unica acompanha
        // o olhar, senao o nariz ficaria no meio do rosto de perfil.
        let x = if piece.side == 0.0 {
            anchor.x * facing.0
        } else {
            anchor.x * piece.side
        };
        let lift = if piece.part == Part::Brow {
            def.brow_lift
        } else {
            0.0
        };

        let glyph = match piece.part {
            Part::Brow => def.brow,
            Part::Eye => def.eye.unwrap_or(look.glyph),
            Part::Mouth => def.mouth.unwrap_or(look.glyph),
            _ => look.glyph,
        };
        let tilt = if piece.part == Part::Brow {
            def.brow_tilt * piece.side * facing.0
        } else {
            0.0
        };

        transform.translation = (head + Vec2::new(x, anchor.y + lift)).extend(FACE_Z);
        transform.rotation = Quat::from_rotation_z(tilt);
        transform.scale = Vec3::splat(look.scale);

        // Mesma regra que pinta a silhueta: o acento vai na peca escassa, o
        // resto veste o papel de cor da pose. Aqui a peca escassa e o cabelo --
        // era o circulo da cabeca ate ele sair --, e o papel e o que faz a cara
        // apagar junto com o corpo em vez de o cadaver ficar de rosto vivo.
        // `Flash` continua por cima de tudo: o piscar de invulneravel tem que
        // ler como uma coisa so, e nao como um boneco malhado.
        let paint = if piece.part == Part::Hair {
            tint.0
        } else {
            skin.0.tone(rig::def(*pose).tone)
        };
        let art = AsciiArt::glyph(glyph, flash.0.unwrap_or(paint));
        if sprite.art != art {
            sprite.art = art;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::actor::{ActorTint, Facing, MAX_PLAYERS, skin};
    use crate::ascii::palette;

    /// Nenhum lugar nasce careca.
    ///
    /// O cabelo e quem veste a cor do jogador. Um lugar que nascesse `BALD`
    /// entraria em campo sem acento nenhum, e quem olhasse leria isso como um
    /// boneco sem cor em vez de uma escolha de quem joga.
    #[test]
    fn nenhum_lugar_nasce_careca() {
        for id in 0..MAX_PLAYERS as u8 {
            assert_ne!(
                Face::variety(id).look(Part::Hair).glyph,
                ' ',
                "o lugar {id} nasce careca"
            );
        }
    }

    /// Monta so o rosto de um boneco: `animate_face` nao le corpo nem membros.
    fn montar(mut commands: Commands) {
        let root = commands
            .spawn((
                Pose::IdleA,
                Facing(1.0),
                ActorSkin(&skin::STICK),
                ActorTint(palette::P2),
                Flash::default(),
                Transform::default(),
            ))
            .id();
        spawn_face(&mut commands, root, root, Face::default());
    }

    /// Cor de cada peca do rosto, com o boneco nesta pose.
    fn cores(pose: Pose) -> Vec<(Part, Color)> {
        let mut app = App::new();
        app.add_systems(Startup, montar)
            .add_systems(Update, animate_face);
        app.update();

        let world = app.world_mut();
        let root = world
            .query_filtered::<Entity, With<Face>>()
            .single(world)
            .unwrap();
        world.entity_mut(root).insert(pose);
        app.update();

        let world = app.world_mut();
        world
            .query::<(&FacePart, &AsciiSprite)>()
            .iter(world)
            .map(|(piece, sprite)| (piece.part, sprite.art.cells[0].color))
            .collect()
    }

    /// Cor de uma peca, na pose dada.
    fn cor(pose: Pose, part: Part) -> Color {
        cores(pose)
            .into_iter()
            .find(|(at, _)| *at == part)
            .unwrap_or_else(|| panic!("{part:?} nao foi desenhada"))
            .1
    }

    /// O cabelo veste a cor do jogador.
    ///
    /// Desde que o circulo da cabeca saiu da silhueta, e ele quem diz de quem e
    /// o boneco. Se nascesse na cor do corpo, dois jogadores com a mesma pele
    /// voltariam a ficar indistinguiveis a distancia.
    #[test]
    fn o_cabelo_veste_a_cor_do_jogador() {
        assert_eq!(
            cor(Pose::IdleA, Part::Hair),
            palette::P2,
            "o cabelo perdeu o acento"
        );
        assert_eq!(
            cor(Pose::IdleA, Part::Eye),
            skin::STICK.body,
            "o acento vazou para o resto do rosto e deixou de ser escasso"
        );
    }

    /// O rosto apaga junto com o corpo.
    ///
    /// A cara e a cabeca agora: se ela ficasse na cor de quem esta de pe, o
    /// cadaver cinza teria um rosto vivo em cima.
    #[test]
    fn o_rosto_apaga_junto_com_o_corpo() {
        assert_eq!(
            cor(Pose::Dead, Part::Eye),
            skin::STICK.gone,
            "o cadaver ficou de rosto vivo"
        );
        assert_eq!(
            cor(Pose::Hit, Part::Eye),
            skin::STICK.hurt,
            "apanhar nao aparece na cara"
        );
    }

    /// `def` indexa a tabela pelo discriminante. Uma linha fora de lugar daria
    /// a uma expressao o rosto de outra, calada.
    #[test]
    fn a_tabela_segue_a_ordem_do_enum() {
        for (i, (expression, _)) in FACES.iter().enumerate() {
            assert_eq!(*expression as usize, i, "{expression:?} fora de lugar");
        }
    }

    /// Toda pose tem que ter uma expressao, e as que tiram o controle nao podem
    /// cair na cara de quem esta tranquilo.
    #[test]
    fn apanhar_e_morrer_nao_ficam_com_cara_de_calmo() {
        assert_eq!(Expression::of(Pose::Dead), Expression::Gone);
        assert_eq!(Expression::of(Pose::Hit), Expression::Hurt);
        assert_eq!(Expression::of(Pose::Downed), Expression::Hurt);
        assert_eq!(Expression::of(Pose::PunchStrike), Expression::Fierce);
        assert_eq!(Expression::of(Pose::IdleA), Expression::Calm);
        assert_eq!(Expression::of(Pose::RunA), Expression::Calm);
    }

    /// Cada expressao precisa mudar alguma coisa no rosto.
    ///
    /// Uma linha copiada da anterior compila, roda e nao aparece na tela --
    /// o jogador leva um soco e a cara nao muda.
    #[test]
    fn cada_expressao_desenha_diferente() {
        let assinatura = |e: Expression| {
            let d = def(e);
            format!(
                "{}{:.2}{:.2}{:?}{:?}",
                d.brow, d.brow_tilt, d.brow_lift, d.eye, d.mouth
            )
        };
        let mut vistas: Vec<String> = Vec::new();
        for (expression, _) in FACES.iter() {
            let atual = assinatura(*expression);
            assert!(
                !vistas.contains(&atual),
                "{expression:?} desenha igual a uma anterior"
            );
            vistas.push(atual);
        }
    }

    /// O rosto tem que caber na cabeca.
    ///
    /// A cabeca e uma celula: 8 de largura por 16 de altura, centrada em
    /// `HEAD`. Uma peca ancorada fora disso flutua ao lado do boneco, e nada
    /// no jogo reclama.
    #[test]
    fn toda_peca_cai_dentro_da_cabeca() {
        for part in Part::ALL {
            let anchor = part.anchor();
            assert!(
                anchor.x.abs() <= 4.0,
                "{part:?} nasce fora da cabeca em x: {anchor:?}"
            );
            assert!(
                anchor.y.abs() <= 8.0,
                "{part:?} nasce fora da cabeca em y: {anchor:?}"
            );
        }
    }

    /// Um catalogo vazio deixaria a peca sem opcao e o seletor sem o que girar.
    #[test]
    fn todo_catalogo_tem_opcao() {
        assert!(!HAIR.is_empty() && !EYES.is_empty() && !NOSE.is_empty() && !MOUTH.is_empty());
        for id in 0..8u8 {
            let face = Face::variety(id);
            for part in Part::ALL {
                // Nao pode entrar em panico por indice fora do catalogo.
                let _ = face.look(part);
            }
        }
    }
}
