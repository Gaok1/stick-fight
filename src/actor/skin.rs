//! Peles: a aparencia de um boneco, separada do que ele faz.
//!
//! Uma pele responde tres perguntas -- que glifos desenham o corpo, de que cor
//! ele fica em cada estado, e como e um membro -- e nada mais. A pose continua
//! decidindo a forma, o `ActorTint` continua decidindo de quem e o boneco.
//!
//! E essa divisao que faz a coisa toda fechar: dois jogadores podem vestir a
//! mesma pele sem deixarem de ser distinguiveis, porque o acento nao vem dela;
//! e uma pele nova nao precisa saber que existe combate, dano ou morte, porque
//! ela responde em papeis de cor ([`Tone`]) e nao em situacoes.

use bevy::prelude::*;

use super::pose::Pose;
use super::rig;
use crate::ascii::{Accent, AsciiArt, palette};

/// Papel de cor do corpo numa pose.
///
/// A pose escolhe o papel, a pele escolhe a cor. Sem essa indirecao o vermelho
/// de dano estaria escrito na pose e valeria para toda pele -- uma pele de
/// pedra sangraria vermelho.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    /// Boneco inteiro, em pe.
    Body,
    /// Apanhando ou caido.
    Hurt,
    /// Morto.
    Gone,
}

/// A aparencia de um boneco.
#[derive(Debug)]
pub struct Skin {
    /// Nome, para a selecao e para as mensagens de teste.
    ///
    /// O catalogo e referencia da direcao de arte, nao inventario do que ja tem
    /// tela -- pelo mesmo motivo que a paleta lista a gama inteira.
    #[allow(dead_code)]
    pub name: &'static str,
    /// Troca de glifo aplicada a silhueta inteira, `(de, para)`.
    ///
    /// E o caminho barato: a mesma silhueta com outra cabeca ou outro tronco,
    /// sem redesenhar as vinte e uma poses.
    pub swap: &'static [(char, char)],
    /// Silhuetas proprias, para as poses que a pele redesenha de verdade.
    ///
    /// O que nao estiver aqui cai na silhueta canonica da pose. Uma pele que
    /// muda so uma expressao lista uma linha, nao vinte e uma.
    pub art: &'static [(Pose, &'static str)],
    /// Caracteres que recebem a cor do jogador.
    ///
    /// Falam nos caracteres canonicos da arte, nao nos trocados por `swap`: a
    /// troca acontece depois de a cor ja estar decidida.
    pub accent: &'static str,
    /// Glifo de um segmento de membro.
    pub limb: char,
    /// Cor do corpo em pe.
    pub body: Color,
    /// Cor de quem esta apanhando.
    pub hurt: Color,
    /// Cor de quem morreu.
    pub gone: Color,
    /// Cor dos membros.
    pub limbs: Color,
}

impl Skin {
    /// Silhueta desta pele para a pose.
    pub fn art(&self, pose: Pose) -> &'static str {
        self.art
            .iter()
            .find(|(at, _)| *at == pose)
            .map_or_else(|| rig::def(pose).art, |(_, art)| *art)
    }

    /// Cor concreta de um papel.
    pub fn tone(&self, tone: Tone) -> Color {
        match tone {
            Tone::Body => self.body,
            Tone::Hurt => self.hurt,
            Tone::Gone => self.gone,
        }
    }

    /// A arte da pose: pintada, com os glifos da pele.
    ///
    /// `accent` e a cor de quem esta dentro do boneco. `flash` sobrescreve
    /// tudo, e e por onde o piscar de invulneravel entra -- sem ele o piscar
    /// precisava de um sistema proprio escrevendo direto no sprite, e de um
    /// segundo sistema so para desfazer o que o primeiro tinha feito.
    ///
    /// A celula da cabeca sai da arte: quem a desenha e o rosto, com olho,
    /// nariz e boca em cima dela. O circulo que ficava ali era so um contorno
    /// atras da cara, e apagar aqui -- e nao nas strings -- e o que mantem a
    /// silhueta como unico lugar que diz em que linha a cabeca esta, que e a
    /// mesma linha que as pecas do rosto leem.
    pub fn draw(&self, pose: Pose, accent: Color, flash: Option<Color>) -> AsciiArt {
        let def = rig::def(pose);
        let (row, col) = rig::head_at(def.art);
        let art = AsciiArt::build(
            self.art(pose),
            &Accent {
                base: self.tone(def.tone),
                accent,
                on: self.accent,
            },
        )
        .swapped(self.swap)
        .erased(col, row);

        match flash {
            Some(color) => art.recolored(color),
            None => art,
        }
    }

    /// A arte de um segmento de membro.
    pub fn limb_art(&self, flash: Option<Color>) -> AsciiArt {
        AsciiArt::glyph(self.limb, flash.unwrap_or(self.limbs))
    }
}

/// O palito de sempre: branco, fino, cabeca na cor do jogador.
pub const STICK: Skin = Skin {
    name: "STICK",
    swap: &[],
    art: &[],
    accent: "Oo<>",
    limb: '|',
    body: palette::BONE,
    hurt: palette::BLOOD,
    gone: palette::IRON,
    limbs: palette::BONE,
};

/// Pesado: mesma silhueta, glifos grossos.
///
/// Nao redesenha pose nenhuma -- so troca caracteres. E a prova de que uma pele
/// nova custa uma linha de tabela, e nao vinte e uma silhuetas.
pub const HEAVY: Skin = Skin {
    name: "HEAVY",
    swap: &[('O', '0'), ('o', '0'), ('|', '\u{2551}'), ('_', '\u{2550}')],
    art: &[],
    accent: "Oo<>",
    limb: '\u{2551}',
    body: palette::BONE,
    hurt: palette::BLOOD,
    gone: palette::IRON,
    limbs: palette::ASH,
};

/// Espectro: corpo esmaecido, membros de sombra.
///
/// Redesenha a respiracao parada para o tronco desvanecer, que e o unico lugar
/// onde a diferenca aparece parada na tela.
pub const WRAITH: Skin = Skin {
    name: "WRAITH",
    swap: &[],
    art: &[
        (Pose::IdleA, " O>\n \u{2593} \n \u{2592} \n   "),
        (Pose::IdleB, " O>\n \u{2592} \n \u{2591} \n   "),
    ],
    accent: "Oo<>",
    limb: '\u{2502}',
    body: palette::ASH,
    hurt: palette::BLOOD,
    gone: palette::COAL,
    limbs: palette::IRON,
};

/// Ninja: mascara fechada e corpo quase preto, com olhos na cor do jogador.
pub const NINJA: Skin = Skin {
    name: "NINJA",
    swap: &[('O', '\u{263B}'), ('o', '\u{263B}'), ('|', '\u{2502}')],
    art: &[
        (Pose::IdleA, " \u{263B}>\n \u{2593} \n \u{2502} \n   "),
        (Pose::IdleB, " \u{263B}>\n \u{2592} \n \u{2502} \n   "),
    ],
    accent: "Oo<>\u{263B}",
    limb: '\u{2502}',
    body: palette::COAL,
    hurt: palette::BLOOD,
    gone: palette::IRON,
    limbs: palette::IRON,
};

/// Robo: glifos duplos e cabeca quadrada, deliberadamente rigido.
pub const ROBOT: Skin = Skin {
    name: "ROBOT",
    swap: &[
        ('O', '\u{25A0}'),
        ('o', '\u{25A0}'),
        ('|', '\u{256C}'),
        ('_', '\u{2550}'),
    ],
    art: &[],
    accent: "Oo<>\u{25A0}",
    limb: '\u{2551}',
    body: palette::ASH,
    hurt: palette::BLOOD,
    gone: palette::IRON,
    limbs: palette::BONE,
};

/// Inferno: tronco em blocos que pulsa como chama sem gastar cor de gameplay.
pub const INFERNO: Skin = Skin {
    name: "INFERNO",
    swap: &[('O', '\u{2666}'), ('o', '\u{2666}'), ('|', '\u{2592}')],
    art: &[
        (Pose::IdleA, " \u{2666}>\n \u{2593} \n \u{2591} \n   "),
        (Pose::IdleB, " \u{2666}>\n \u{2592} \n \u{2593} \n   "),
    ],
    accent: "Oo<>\u{2666}",
    limb: '\u{2591}',
    body: palette::BONE,
    hurt: palette::BLOOD,
    gone: palette::COAL,
    limbs: palette::ASH,
};

/// As peles disponiveis, na ordem em que a selecao as percorre.
pub const CATALOG: [&Skin; 6] = [&STICK, &HEAVY, &WRAITH, &NINJA, &ROBOT, &INFERNO];

/// A pele de indice `i`, girando.
pub fn skin(i: usize) -> &'static Skin {
    CATALOG[i % CATALOG.len()]
}

/// Escolhas cosmeticas persistentes. `players` vale para os ids da partida;
/// `online_local` existe antes de a Steam decidir se este cliente sera P1/P2.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkinSelections {
    pub players: [usize; super::MAX_PLAYERS],
    pub online_local: usize,
    /// Rosto de cada lugar. Mora aqui, junto da pele, porque as duas coisas sao
    /// a mesma pergunta -- com que cara este boneco entra em campo -- e viajam
    /// juntas no pacote de inicio.
    pub faces: [super::face::Face; super::MAX_PLAYERS],
    pub online_face: super::face::Face,
}

impl Default for SkinSelections {
    fn default() -> Self {
        // Uma pele e um rosto diferentes por lugar: numa sala cheia ninguem
        // precisa escolher pra ja dar pra separar os quatro bonecos.
        Self {
            players: [0, 1, 2, 3],
            online_local: 0,
            faces: [
                super::face::Face::variety(0),
                super::face::Face::variety(1),
                super::face::Face::variety(2),
                super::face::Face::variety(3),
            ],
            online_face: super::face::Face::variety(0),
        }
    }
}

impl SkinSelections {
    pub fn for_player(&self, id: u8) -> &'static Skin {
        skin(self.players[(id as usize).min(super::MAX_PLAYERS - 1)])
    }

    /// O rosto escolhido para um lugar.
    pub fn face_for(&self, id: u8) -> super::face::Face {
        self.faces[(id as usize).min(super::MAX_PLAYERS - 1)]
    }
}

/// A pele que um boneco esta vestindo.
///
/// Trocar este componente repinta o boneco inteiro -- silhueta e membros -- no
/// mesmo frame: os sistemas de animacao escutam `Changed<ActorSkin>` do mesmo
/// jeito que escutam mudanca de pose.
#[derive(Component, Clone, Copy, Debug)]
pub struct ActorSkin(pub &'static Skin);

impl Default for ActorSkin {
    fn default() -> Self {
        Self(&STICK)
    }
}

/// Cor que sobrescreve o boneco inteiro neste instante.
///
/// E um `Option` dentro de um componente sempre presente, e nao um componente
/// que aparece e some, de proposito: assim um filtro `Changed` so pega tanto o
/// inicio quanto o fim do efeito. Enquanto isto era insercao e remocao, o fim
/// nao disparava nada e precisava de um sistema inteiro (`restore_after_blink`)
/// que existia so para desfazer o efeito do outro.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq)]
pub struct Flash(pub Option<Color>);

#[cfg(test)]
mod tests {
    use super::*;

    /// Toda pele tem que caber na mesma caixa de arte que a pose canonica.
    ///
    /// Se uma silhueta de pele for mais larga, o boneco escorrega de lado ao
    /// virar -- o pivo e calculado a partir do tamanho da arte.
    #[test]
    fn toda_pele_cabe_na_caixa_do_corpo() {
        for skin in CATALOG {
            for pose in rig::all() {
                let art = AsciiArt::solid(skin.art(pose), palette::BONE);
                assert_eq!(
                    (art.cols, art.rows),
                    (super::super::pose::BODY_COLS, super::super::pose::BODY_ROWS),
                    "{}: pose {pose:?} fora da caixa",
                    skin.name
                );
            }
        }
    }

    /// O acento tem que sobreviver a troca de glifos.
    ///
    /// `swap` roda depois de a cor estar decidida justamente para isso: se a
    /// troca acontecesse na string, uma pele que muda um caractere acentuado
    /// perderia a cor do jogador e os dois lutadores ficariam iguais. Desde que
    /// a cabeca saiu da silhueta, quem carrega o acento nela e a faixa.
    #[test]
    fn a_troca_de_glifo_nao_apaga_a_cor_do_jogador() {
        let art = HEAVY.draw(Pose::IdleA, palette::P2, None);
        let faixa = art
            .cells
            .iter()
            .find(|c| c.glyph == b'>')
            .expect("a faixa sumiu da silhueta");
        assert_eq!(faixa.color, palette::P2);
        // E o tronco continua na cor do corpo, ja com o glifo grosso da pele:
        // senao o acento deixou de ser escasso e parou de carregar informacao.
        let tronco = art
            .cells
            .iter()
            .find(|c| c.glyph == 0xBA)
            .expect("HEAVY deveria trocar o tronco");
        assert_eq!(tronco.color, HEAVY.body);
    }

    #[test]
    fn a_faixa_usa_a_cor_do_jogador() {
        let art = STICK.draw(Pose::IdleA, palette::P2, None);
        let faixa = art.cells.iter().find(|c| c.glyph == b'>').unwrap();
        assert_eq!(faixa.color, palette::P2);
    }

    /// Nenhuma pele pode desenhar na celula da cabeca.
    ///
    /// E ali que ficam olho, nariz e boca -- pecas menores que uma celula,
    /// posicionadas por cima. Um glifo de pele naquela celula voltaria a
    /// aparecer como contorno atras da cara, que e exatamente o circulo que
    /// saiu da silhueta.
    #[test]
    fn nenhuma_pele_desenha_na_celula_da_cabeca() {
        for skin in CATALOG {
            for pose in rig::all() {
                let (row, col) = rig::head_at(rig::def(pose).art);
                let art = skin.draw(pose, palette::P1, None);
                assert!(
                    !art.cells.iter().any(|c| (c.col, c.row) == (col, row)),
                    "{}: pose {pose:?} desenha na celula da cabeca",
                    skin.name
                );
            }
        }
    }

    /// A pose escolhe o papel, a pele escolhe a cor: duas peles tem que pintar
    /// a mesma pose de cores diferentes.
    #[test]
    fn o_papel_de_cor_passa_pela_pele() {
        assert_eq!(
            STICK.draw(Pose::Dead, palette::P1, None).cells[0].color,
            STICK.gone
        );
        assert_eq!(
            WRAITH.draw(Pose::Dead, palette::P1, None).cells[0].color,
            WRAITH.gone
        );
        assert_ne!(STICK.gone, WRAITH.gone, "as duas peles morrem igual");
    }

    /// O flash sobrescreve tudo, inclusive o acento -- e o que faz o piscar de
    /// invulneravel ler como uma coisa so, e nao como um boneco malhado.
    #[test]
    fn o_flash_pinta_o_boneco_inteiro() {
        let art = STICK.draw(Pose::IdleA, palette::P2, Some(palette::BLOOD));
        assert!(art.cells.iter().all(|c| c.color == palette::BLOOD));
        assert_eq!(
            STICK.limb_art(Some(palette::BLOOD)).cells[0].color,
            palette::BLOOD
        );
    }

    /// Peles tem que ser distinguiveis: duas que desenhem igual e pintem igual
    /// nao sao duas peles.
    #[test]
    fn cada_pele_desenha_diferente() {
        for (i, a) in CATALOG.iter().enumerate() {
            for b in CATALOG.iter().skip(i + 1) {
                let diferem = rig::all()
                    .any(|pose| a.draw(pose, palette::P1, None) != b.draw(pose, palette::P1, None))
                    || a.limb_art(None) != b.limb_art(None);
                assert!(diferem, "{} e {} sao a mesma pele", a.name, b.name);
            }
        }
    }
}
