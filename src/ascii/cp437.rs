//! Fonte CP437 e atlas de glifos.
//!
//! A ROM de 4096 bytes e a fonte 8x16 do BIOS VGA do IBM PC: 256 glifos, 16
//! bytes cada, 1 bit por pixel, MSB a esquerda. E o mesmo repertorio que o
//! Dwarf Fortress usa (`curses_*.png`), so que como bitmap embutido no binario
//! em vez de PNG externo -- nao ha asset pra carregar nem licenca pra carregar
//! junto (a forma de um glifo bitmap nao e copyrightavel nos EUA).

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageSampler, TextureAtlasLayout};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use std::collections::HashMap;

use super::cp437_table::CP437_TO_UNICODE;

/// Largura de um glifo em pixels/unidades de mundo.
pub const GLYPH_W: u32 = 8;
/// Altura de um glifo em pixels/unidades de mundo.
pub const GLYPH_H: u32 = 16;
/// Colunas do atlas (16x16 = 256 glifos).
pub const ATLAS_COLS: u32 = 16;
/// Linhas do atlas.
pub const ATLAS_ROWS: u32 = 16;

/// Tamanho de uma celula como `Vec2`, para conta de layout.
pub const CELL: Vec2 = Vec2::new(GLYPH_W as f32, GLYPH_H as f32);

/// ROM da fonte VGA 8x16 do IBM PC (256 glifos * 16 bytes).
const FONT_ROM: &[u8; 4096] = include_bytes!("../../assets/fonts/ibm_vga_8x16.bin");

/// Glifos graficos que o DOS desenha em 0x00..0x1F.
///
/// A tabela `CP437_TO_UNICODE` mapeia essa faixa para caracteres de controle
/// (e o que o codec `cp437` faz), entao os simbolos que a IBM realmente
/// desenhava ali precisam de um mapeamento explicito para ficarem acessiveis.
const DOS_GRAPHICS: &[(char, u8)] = &[
    ('\u{263A}', 0x01), // smiley vazado
    ('\u{263B}', 0x02), // smiley cheio
    ('\u{2665}', 0x03), // copas
    ('\u{2666}', 0x04), // ouros
    ('\u{2663}', 0x05), // paus
    ('\u{2660}', 0x06), // espadas
    ('\u{2022}', 0x07), // bullet
    ('\u{25D8}', 0x08),
    ('\u{25CB}', 0x09),
    ('\u{2642}', 0x0B), // marte
    ('\u{2640}', 0x0C), // venus
    ('\u{266A}', 0x0D), // nota
    ('\u{266B}', 0x0E),
    ('\u{263C}', 0x0F), // sol
    ('\u{25BA}', 0x10), // seta cheia direita
    ('\u{25C4}', 0x11), // seta cheia esquerda
    ('\u{2195}', 0x12),
    ('\u{203C}', 0x13),
    ('\u{25AC}', 0x16),
    ('\u{2191}', 0x18),
    ('\u{2193}', 0x19),
    ('\u{2192}', 0x1A),
    ('\u{2190}', 0x1B),
    ('\u{2194}', 0x1D),
    ('\u{25B2}', 0x1E), // triangulo cima
    ('\u{25BC}', 0x1F), // triangulo baixo
];

/// Handles do atlas de glifos, prontos para montar um `Sprite`.
#[derive(Resource, Debug, Clone)]
pub struct GlyphAtlas {
    /// Textura 128x256 com os 256 glifos em branco sobre transparente.
    pub image: Handle<Image>,
    /// Layout 16x16 que indexa a textura por codigo CP437.
    pub layout: Handle<TextureAtlasLayout>,
}

/// Indice CP437 de um caractere Unicode.
///
/// Cai em `?` (0x3F) para qualquer coisa fora da pagina de codigo, entao arte
/// com caractere invalido aparece visivelmente errada em vez de sumir.
pub fn glyph_index(c: char) -> usize {
    static ONCE: std::sync::OnceLock<HashMap<char, u8>> = std::sync::OnceLock::new();
    let map = ONCE.get_or_init(|| {
        let mut m = HashMap::default();
        // Faixa imprimivel + alta: a tabela gerada e autoritativa.
        for (code, &ch) in CP437_TO_UNICODE.iter().enumerate().skip(0x20) {
            m.entry(ch).or_insert(code as u8);
        }
        // Os graficos do DOS sobrescrevem os controles de 0x00..0x1F.
        for &(ch, code) in DOS_GRAPHICS {
            m.insert(ch, code);
        }
        m
    });
    map.get(&c).copied().unwrap_or(b'?') as usize
}

/// Le um pixel do glifo direto da ROM. `true` = aceso.
///
/// Usado tanto para montar o atlas quanto para o banner gigante (que redesenha
/// cada pixel de um glifo como uma celula inteira).
pub fn rom_pixel(code: usize, x: u32, y: u32) -> bool {
    if x >= GLYPH_W || y >= GLYPH_H || code >= 256 {
        return false;
    }
    FONT_ROM[code * GLYPH_H as usize + y as usize] & (0x80 >> x) != 0
}

/// Constroi a textura do atlas a partir da ROM: branco opaco onde o bit esta
/// aceso, transparente no resto. O tint fica por conta do `Sprite`.
fn build_atlas_image() -> Image {
    let w = ATLAS_COLS * GLYPH_W;
    let h = ATLAS_ROWS * GLYPH_H;
    let mut data = vec![0u8; (w * h * 4) as usize];

    for code in 0..256usize {
        let gx = (code as u32 % ATLAS_COLS) * GLYPH_W;
        let gy = (code as u32 / ATLAS_COLS) * GLYPH_H;
        for y in 0..GLYPH_H {
            for x in 0..GLYPH_W {
                if rom_pixel(code, x, y) {
                    let px = (((gy + y) * w + gx + x) * 4) as usize;
                    data[px..px + 4].copy_from_slice(&[255, 255, 255, 255]);
                }
            }
        }
    }

    let mut image = Image::new(
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    // Pixel art: nada de filtro linear borrando a borda do glifo.
    image.sampler = ImageSampler::nearest();
    image
}

/// Gera o atlas e publica os handles. Roda uma vez, antes de qualquer spawn.
pub fn init_glyph_atlas(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let image = images.add(build_atlas_image());
    let layout = layouts.add(TextureAtlasLayout::from_grid(
        UVec2::new(GLYPH_W, GLYPH_H),
        ATLAS_COLS,
        ATLAS_ROWS,
        None,
        None,
    ));
    commands.insert_resource(GlyphAtlas { image, layout });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rom_tem_tamanho_de_uma_fonte_8x16() {
        assert_eq!(FONT_ROM.len(), 256 * GLYPH_H as usize);
    }

    #[test]
    fn ascii_mapeia_para_o_proprio_byte() {
        for c in ' '..='~' {
            assert_eq!(glyph_index(c), c as usize, "ASCII {c:?} fora do lugar");
        }
    }

    #[test]
    fn glifos_de_bloco_e_moldura_existem() {
        assert_eq!(glyph_index('\u{2588}'), 0xDB); // bloco cheio
        assert_eq!(glyph_index('\u{2591}'), 0xB0); // sombra leve
        assert_eq!(glyph_index('\u{2500}'), 0xC4); // linha horizontal
        assert_eq!(glyph_index('\u{2554}'), 0xC9); // canto duplo sup-esq
        assert_eq!(glyph_index('\u{25B2}'), 0x1E); // triangulo (override DOS)
    }

    /// Todo glifo que a arte do jogo usa tem que existir na pagina de codigo.
    ///
    /// Fora dela o `glyph_index` cai em `?`: a arte aparece errada mas o jogo
    /// roda, o teste passa e ninguem descobre ate olhar a tela. Foi assim que
    /// a `BOMB` nasceu desenhada como uma interrogacao, no chao, na mao e no
    /// arremesso, e sobreviveu duas iteracoes.
    ///
    /// A varredura cobre as fontes de arte que sao dado -- poses, arsenal e
    /// fases -- que e por onde conteudo novo entra.
    #[test]
    fn a_arte_do_jogo_cabe_na_cp437() {
        let fallback = glyph_index('?');
        // O detector tem que detectar. Sem esta linha o teste passaria mesmo
        // se `glyph_index` parasse de cair no fallback, e nao provaria nada.
        // U+25CF e justamente o caractere que a `BOMB` usava.
        assert_eq!(
            glyph_index('\u{25CF}'),
            fallback,
            "U+25CF deveria estar fora da pagina"
        );

        let mut fora: Vec<String> = Vec::new();

        let mut conferir = |origem: String, texto: &str| {
            // `\n` separa linhas da arte, nao e glifo.
            for ch in texto.chars().filter(|c| *c != '\n') {
                // `?` de verdade e legitimo; qualquer outro que caia no
                // fallback e caractere que a pagina nao tem.
                if ch != '?' && glyph_index(ch) == fallback {
                    fora.push(format!("{origem}: U+{:04X} {ch:?}", ch as u32));
                }
            }
        };

        // Peles vezes poses: uma pele desenha com glifos proprios, entao a
        // varredura tem que passar por cada uma delas -- olhar so a silhueta
        // canonica deixaria a arte de uma pele inteira sem conferencia.
        for skin in crate::actor::skin::CATALOG {
            let nome = skin.name;
            conferir(format!("{nome} (membro)"), &skin.limb.to_string());
            for pose in crate::actor::rig::all() {
                conferir(format!("{nome}, pose {pose:?}"), skin.art(pose));
            }
            for (de, para) in skin.swap {
                conferir(format!("{nome} (troca de {de:?})"), &para.to_string());
            }
        }

        // Rosto: catalogo de cada peca mais os glifos que as expressoes usam.
        // E por aqui que conteudo novo entra, entao a varredura tem que cobrir
        // os dois -- um cabelo fora da pagina viraria uma interrogacao na testa
        // do boneco, e o jogo rodaria assim mesmo.
        for part in crate::actor::face::Part::ALL {
            for look in part.catalog() {
                conferir(
                    format!("rosto {} ({})", part.label(), look.name),
                    &look.glyph.to_string(),
                );
            }
        }
        for expression in crate::actor::face::Expression::ALL {
            let def = crate::actor::face::def(expression);
            let mut glifos = String::from(def.brow);
            glifos.extend(def.eye);
            glifos.extend(def.mouth);
            conferir(format!("expressao {expression:?}"), &glifos);
        }

        for make in crate::weapon::ARSENAL {
            let weapon = make();
            let nome = weapon.name();
            conferir(format!("{nome} (nome)"), nome);
            conferir(format!("{nome} (na mao)"), weapon.held_art());
            conferir(format!("{nome} (no chao)"), weapon.ground_art());
            for shot in weapon.shots(Vec2::X) {
                conferir(format!("{nome} (projetil)"), &shot.glyph.to_string());
            }
        }

        for build in crate::level::CATALOG {
            let level = build();
            let nome = level.name();
            conferir(format!("fase {nome}"), nome);
            for (texto, ..) in level.signs() {
                conferir(format!("letreiro de {nome}"), texto);
            }
        }

        assert!(
            fora.is_empty(),
            "arte fora da CP437:\n  {}",
            fora.join("\n  ")
        );
    }

    #[test]
    fn bloco_cheio_tem_todos_os_pixels_acesos() {
        for y in 0..GLYPH_H {
            for x in 0..GLYPH_W {
                assert!(rom_pixel(0xDB, x, y), "bloco cheio furado em {x},{y}");
            }
        }
    }

    #[test]
    fn espaco_nao_tem_pixel_aceso() {
        for y in 0..GLYPH_H {
            for x in 0..GLYPH_W {
                assert!(!rom_pixel(0x20, x, y));
            }
        }
    }
}
