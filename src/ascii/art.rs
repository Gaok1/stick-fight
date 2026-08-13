//! Arte ASCII: da string crua para celulas com cor resolvida.
//!
//! Uma `AsciiArt` e so um punhado de celulas com coordenada de grid local. Ela
//! nao sabe nada de posicao no mundo, escala ou rotacao -- isso e o `Transform`
//! da entidade que carrega. O grid existe so como conveniencia de autoria; o
//! sprite final vive em espaco continuo.

use bevy::prelude::*;

use super::cp437::{CELL, GLYPH_H, GLYPH_W, glyph_index, rom_pixel};
use super::palette;

/// Uma celula ja resolvida: onde, qual glifo, que cor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cell {
    /// Coluna no grid local (0 = esquerda).
    pub col: u16,
    /// Linha no grid local (0 = topo).
    pub row: u16,
    /// Indice CP437 do glifo.
    pub glyph: u8,
    /// Cor final do glifo.
    pub color: Color,
}

/// Decide a cor de cada celula de uma arte.
///
/// Separar cor de forma e o que deixa a mesma silhueta servir dois jogadores,
/// um estado de dano e uma versao fantasma sem duplicar a arte.
pub trait Paint: Send + Sync {
    /// Cor da celula `(col, row)`, cujo caractere e `ch`.
    fn color_at(&self, ch: char, col: u16, row: u16) -> Color;
}

/// Pinta a arte inteira de uma cor so.
pub struct Solid(pub Color);

impl Paint for Solid {
    fn color_at(&self, _ch: char, _col: u16, _row: u16) -> Color {
        self.0
    }
}

/// Pinta por mascara: uma segunda string, do mesmo tamanho da arte, onde cada
/// caractere e uma chave de [`palette::key`]. Espaco na mascara cai no
/// `fallback`.
///
/// E o caminho para arte de coloracao irregular -- cenario, decoracao. Os
/// bonecos usam [`Accent`], que e mais barato.
#[allow(dead_code)]
pub struct Mask<'a> {
    /// String de chaves de cor, linha a linha.
    pub mask: &'a str,
    /// Cor usada onde a mascara nao diz nada.
    pub fallback: Color,
}

impl Paint for Mask<'_> {
    fn color_at(&self, _ch: char, col: u16, row: u16) -> Color {
        self.mask
            .lines()
            .nth(row as usize)
            .and_then(|l| l.chars().nth(col as usize))
            .filter(|c| !c.is_whitespace())
            .map_or(self.fallback, palette::key)
    }
}

/// Pinta um caractere-chave com uma cor de destaque e o resto com a cor base.
///
/// E como o boneco ganha identidade: mesma silhueta, cabeca na cor do jogador.
pub struct Accent {
    /// Cor do corpo.
    pub base: Color,
    /// Cor dos caracteres listados em `on`.
    pub accent: Color,
    /// Caracteres que recebem o destaque.
    pub on: &'static str,
}

impl Paint for Accent {
    fn color_at(&self, ch: char, _col: u16, _row: u16) -> Color {
        if self.on.contains(ch) {
            self.accent
        } else {
            self.base
        }
    }
}

/// Um bloco de arte ASCII pronto para virar glifos.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AsciiArt {
    /// Celulas nao-vazias. Espacos sao descartados (ficam transparentes).
    pub cells: Vec<Cell>,
    /// Largura em colunas (linha mais longa).
    pub cols: u16,
    /// Altura em linhas.
    pub rows: u16,
}

impl AsciiArt {
    /// Monta a arte aplicando um [`Paint`] celula a celula.
    pub fn build(art: &str, paint: &dyn Paint) -> Self {
        let mut cells = Vec::new();
        let mut cols = 0u16;
        let mut rows = 0u16;

        for (row, line) in art.lines().enumerate() {
            let row = row as u16;
            rows = rows.max(row + 1);
            for (col, ch) in line.chars().enumerate() {
                let col = col as u16;
                cols = cols.max(col + 1);
                if ch == ' ' {
                    continue;
                }
                cells.push(Cell {
                    col,
                    row,
                    glyph: glyph_index(ch) as u8,
                    color: paint.color_at(ch, col, row),
                });
            }
        }

        Self { cells, cols, rows }
    }

    /// Atalho: arte inteira numa cor so.
    pub fn solid(art: &str, color: Color) -> Self {
        Self::build(art, &Solid(color))
    }

    /// Atalho: arte com mascara de cor.
    #[allow(dead_code)]
    pub fn masked(art: &str, mask: &str, fallback: Color) -> Self {
        Self::build(art, &Mask { mask, fallback })
    }

    /// Uma celula so.
    ///
    /// E o formato mais comum do jogo -- particula, projetil, segmento de
    /// membro -- e sem ele cada chamador montava uma `String` de um caractere
    /// para `solid` desmontar na linha seguinte.
    pub fn glyph(ch: char, color: Color) -> Self {
        Self {
            cells: vec![Cell {
                col: 0,
                row: 0,
                glyph: glyph_index(ch) as u8,
                color,
            }],
            cols: 1,
            rows: 1,
        }
    }

    /// Um retangulo cheio de um mesmo glifo -- terreno, barras de vida, etc.
    pub fn fill(ch: char, cols: u16, rows: u16, color: Color) -> Self {
        let glyph = glyph_index(ch) as u8;
        let cells = (0..rows)
            .flat_map(|row| {
                (0..cols).map(move |col| Cell {
                    col,
                    row,
                    glyph,
                    color,
                })
            })
            .collect();
        Self { cells, cols, rows }
    }

    /// Texto gigante: cada pixel aceso da fonte ROM vira uma celula inteira.
    ///
    /// Reaproveita o bitmap que ja esta embutido em vez de exigir arte de
    /// titulo desenhada a mao, entao o banner nunca sai de estilo com o resto.
    pub fn banner(text: &str, ch: char, color: Color) -> Self {
        let glyph = glyph_index(ch) as u8;
        let mut cells = Vec::new();

        for (i, c) in text.chars().enumerate() {
            let code = glyph_index(c);
            let ox = i as u16 * GLYPH_W as u16;
            for y in 0..GLYPH_H {
                for x in 0..GLYPH_W {
                    if rom_pixel(code, x, y) {
                        cells.push(Cell {
                            col: ox + x as u16,
                            row: y as u16,
                            glyph,
                            color,
                        });
                    }
                }
            }
        }

        Self {
            cells,
            cols: text.chars().count() as u16 * GLYPH_W as u16,
            rows: GLYPH_H as u16,
        }
    }

    /// Empilha outra arte por cima desta, deslocada em celulas.
    ///
    /// Deixa compor HUD e telas sem precisar de uma entidade por pedaco.
    pub fn stamp(mut self, other: &AsciiArt, col: u16, row: u16) -> Self {
        self.cells.extend(other.cells.iter().map(|c| Cell {
            col: c.col + col,
            row: c.row + row,
            ..*c
        }));
        self.cols = self.cols.max(other.cols + col);
        self.rows = self.rows.max(other.rows + row);
        self
    }

    /// Espelha horizontalmente, trocando os glifos que tem par direcional.
    ///
    /// Sem a troca de glifo um `/` espelhado continuaria `/` e o boneco ficaria
    /// com a perna quebrada ao virar.
    pub fn mirrored(&self) -> Self {
        let cells = self
            .cells
            .iter()
            .map(|c| Cell {
                col: self.cols.saturating_sub(1) - c.col,
                glyph: mirror_glyph(c.glyph),
                ..*c
            })
            .collect();
        Self {
            cells,
            cols: self.cols,
            rows: self.rows,
        }
    }

    /// Tamanho em unidades de mundo, na escala 1:1 do glifo.
    pub fn size(&self) -> Vec2 {
        Vec2::new(self.cols as f32, self.rows as f32) * CELL
    }

    /// Troca glifos por um mapa `(de, para)`, mantendo posicao e cor.
    ///
    /// E o que deixa uma pele vestir a silhueta inteira -- outra cabeca, outro
    /// tronco -- sem redesenhar pose por pose. Trocar aqui, e nao na string
    /// antes de construir, e o que preserva o `on` do [`Accent`]: ele fala nos
    /// caracteres canonicos da arte, que continuam sendo os que decidiram a
    /// cor.
    pub fn swapped(&self, pairs: &[(char, char)]) -> Self {
        if pairs.is_empty() {
            return self.clone();
        }
        let cells = self
            .cells
            .iter()
            .map(|cell| {
                match pairs
                    .iter()
                    .find(|(from, _)| glyph_index(*from) as u8 == cell.glyph)
                {
                    Some((_, to)) => Cell {
                        glyph: glyph_index(*to) as u8,
                        ..*cell
                    },
                    None => *cell,
                }
            })
            .collect();
        Self { cells, ..*self }
    }

    /// Apaga uma celula, se houver alguma ali.
    ///
    /// E como a silhueta do boneco abre espaco para a cabeca sem que a string
    /// precise mentir sobre onde a cabeca esta: quem desenha aquela celula sao
    /// as pecas do rosto, por cima, e a arte continua sendo o unico lugar que
    /// diz em que linha ela fica.
    pub fn erased(&self, col: u16, row: u16) -> Self {
        Self {
            cells: self
                .cells
                .iter()
                .filter(|cell| (cell.col, cell.row) != (col, row))
                .copied()
                .collect(),
            ..*self
        }
    }

    /// Sobrescreve a cor de todas as celulas -- usado para flash de dano.
    pub fn recolored(&self, color: Color) -> Self {
        Self {
            cells: self.cells.iter().map(|c| Cell { color, ..*c }).collect(),
            ..*self
        }
    }
}

/// Par espelhado de um glifo CP437, ou ele mesmo se for simetrico.
fn mirror_glyph(g: u8) -> u8 {
    const PAIRS: &[(u8, u8)] = &[
        (b'/', b'\\'),
        (b'(', b')'),
        (b'[', b']'),
        (b'{', b'}'),
        (b'<', b'>'),
        (b'd', b'b'),
        (b'p', b'q'),
        (b'\\', b'/'),
        (b')', b'('),
        (b']', b'['),
        (b'}', b'{'),
        (b'>', b'<'),
        (b'b', b'd'),
        (b'q', b'p'),
        (0x10, 0x11), // seta cheia direita/esquerda
        (0x11, 0x10),
        (0xDD, 0xDE), // meio-bloco esquerdo/direito
        (0xDE, 0xDD),
        (0xC9, 0xBB), // cantos duplos superiores
        (0xBB, 0xC9),
        (0xC8, 0xBC), // cantos duplos inferiores
        (0xBC, 0xC8),
        (0xCC, 0xB9), // T duplo esquerda/direita
        (0xB9, 0xCC),
        (0xDA, 0xBF), // cantos simples superiores
        (0xBF, 0xDA),
        (0xC0, 0xD9), // cantos simples inferiores
        (0xD9, 0xC0),
        (0xC3, 0xB4), // T simples esquerda/direita
        (0xB4, 0xC3),
    ];
    PAIRS
        .iter()
        .find(|(from, _)| *from == g)
        .map_or(g, |(_, to)| *to)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn espacos_viram_transparencia() {
        let art = AsciiArt::solid("a b", palette::BONE);
        assert_eq!(art.cells.len(), 2);
        assert_eq!(art.cols, 3);
        assert_eq!(art.rows, 1);
    }

    #[test]
    fn dimensao_segue_a_linha_mais_longa() {
        let art = AsciiArt::solid("ab\nabcd\na", palette::BONE);
        assert_eq!((art.cols, art.rows), (4, 3));
        assert_eq!(art.size(), Vec2::new(32.0, 48.0));
    }

    #[test]
    fn espelhar_troca_diagonais_e_inverte_colunas() {
        let art = AsciiArt::solid("/ \\", palette::BONE);
        let flipped = art.mirrored();
        // '/' na coluna 0 vira '\' na coluna 2, e vice-versa.
        let at = |c: u16| {
            flipped
                .cells
                .iter()
                .find(|x| x.col == c)
                .map(|x| x.glyph)
                .unwrap()
        };
        assert_eq!(at(2), b'\\');
        assert_eq!(at(0), b'/');
    }

    #[test]
    fn espelhar_duas_vezes_volta_ao_original() {
        let art = AsciiArt::solid(" O \n/|\\\n/ \\", palette::BONE);
        let mut twice = art.mirrored().mirrored();
        let mut once = art.clone();
        let sort = |v: &mut Vec<Cell>| v.sort_by_key(|c| (c.row, c.col));
        sort(&mut twice.cells);
        sort(&mut once.cells);
        assert_eq!(twice, once);
    }

    #[test]
    fn mascara_colore_por_celula() {
        let art = AsciiArt::masked("OO", "1r", palette::BONE);
        assert_eq!(art.cells[0].color, palette::P1);
        assert_eq!(art.cells[1].color, palette::BLOOD);
    }

    #[test]
    fn accent_pega_so_os_caracteres_listados() {
        let paint = Accent {
            base: palette::BONE,
            accent: palette::P2,
            on: "O",
        };
        let art = AsciiArt::build("O|", &paint);
        assert_eq!(art.cells[0].color, palette::P2);
        assert_eq!(art.cells[1].color, palette::BONE);
    }

    #[test]
    fn banner_expande_o_bitmap_da_fonte() {
        let art = AsciiArt::banner("A", '\u{2588}', palette::BONE);
        assert_eq!((art.cols, art.rows), (8, 16));
        // 'A' tem pixel aceso, mas nao e um bloco solido.
        assert!(!art.cells.is_empty());
        assert!(art.cells.len() < (8 * 16));
    }

    #[test]
    fn stamp_expande_os_limites() {
        let base = AsciiArt::fill('#', 2, 2, palette::BONE);
        let combined = base.stamp(&AsciiArt::fill('#', 2, 2, palette::BONE), 5, 1);
        assert_eq!((combined.cols, combined.rows), (7, 3));
        assert_eq!(combined.cells.len(), 8);
    }
}
