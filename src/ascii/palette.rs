//! Paleta.
//!
//! A direcao de arte e monocromatica: cenario e corpos em tons de cinza, e a
//! cor entra so como informacao de jogo (quem e quem, o que machuca, o que da
//! pra pegar). Manter os acentos escassos e o que faz eles lerem.
//!
//! A paleta define a direcao de arte, entao lista a gama inteira mesmo que nem
//! toda cor tenha uso hoje. E referencia, nao inventario do que ja foi gasto.
#![allow(dead_code)]

use bevy::prelude::*;

// --- base monocromatica ---

/// Fundo da tela.
pub const VOID: Color = Color::srgb(0.03, 0.03, 0.04);
/// Branco principal: corpos, texto.
pub const BONE: Color = Color::srgb(0.92, 0.92, 0.88);
/// Cinza medio: superficie do chao.
pub const ASH: Color = Color::srgb(0.46, 0.47, 0.51);
/// Cinza escuro: massa do terreno, correntes.
pub const IRON: Color = Color::srgb(0.29, 0.30, 0.34);
/// Quase preto: preenchimento profundo, sombra.
pub const COAL: Color = Color::srgb(0.15, 0.16, 0.19);

// --- acentos (usar com parcimonia) ---

/// Jogador 1.
pub const P1: Color = Color::srgb(0.20, 0.85, 0.95);
/// Jogador 2.
pub const P2: Color = Color::srgb(1.00, 0.47, 0.15);
/// Jogador 3.
///
/// Violeta e verde-limao fecham o circulo com o ciano e o laranja: quatro
/// acentos que nao se confundem entre si nem com o vermelho do dano ou o
/// dourado das armas, que e o que a tela ja gasta de cor.
pub const P3: Color = Color::srgb(0.78, 0.40, 0.95);
/// Jogador 4.
pub const P4: Color = Color::srgb(0.62, 0.92, 0.30);
/// Dano, morte.
pub const BLOOD: Color = Color::srgb(0.88, 0.13, 0.22);
/// Armas no chao, municao.
pub const GOLD: Color = Color::srgb(1.00, 0.80, 0.22);
/// Corrente escalavel (sinaliza "interagivel").
pub const MOSS: Color = Color::srgb(0.38, 0.72, 0.40);
/// Nucleo branco-amarelo da lava.
pub const MAGMA: Color = Color::srgb(1.00, 0.86, 0.24);
/// Corpo laranja da lava.
pub const EMBER: Color = Color::srgb(1.00, 0.32, 0.06);
/// Brilho toxico do acido.
pub const TOXIC: Color = Color::srgb(0.65, 1.00, 0.12);
/// Profundidade esverdeada dos pocos de acido.
pub const SLUDGE: Color = Color::srgb(0.12, 0.40, 0.18);
/// Fogo de jade: o sopro do dragao e as fontes do jardim.
///
/// Mora entre os acentos de gameplay, e nao entre as cores de cenario, porque
/// e isso que ele e: a chama que sai da boca de pedra machuca. O jade parado --
/// escultura, escama, pilar -- fica em [`SCENE_JADE`], bem abaixo desta.
pub const JADE: Color = Color::srgb(0.24, 0.94, 0.62);
/// Laranja queimado do ceu oriental.
pub const SUNSET: Color = Color::srgb(0.95, 0.28, 0.12);
/// Roxo distante para silhuetas no poente.
pub const HAZE: Color = Color::srgb(0.30, 0.18, 0.34);

// --- cenario colorido ------------------------------------------------------

/// Acentos do fundo ficam deliberadamente abaixo das cores de gameplay.
/// Assim P2 continua laranja sobre lava/poente e P4 continua verde na fabrica.
pub const SCENE_FIRE: Color = Color::srgb(0.48, 0.13, 0.06);
pub const SCENE_GOLD: Color = Color::srgb(0.48, 0.34, 0.10);
pub const SCENE_RED: Color = Color::srgb(0.46, 0.09, 0.12);
pub const SCENE_TOXIC: Color = Color::srgb(0.17, 0.38, 0.23);
pub const SCENE_BLUE: Color = Color::srgb(0.14, 0.31, 0.38);
pub const SCENE_HAZE: Color = Color::srgb(0.24, 0.14, 0.28);
/// Brasa fria: a rocha que ainda guarda calor, entre o carvao e o fogo.
///
/// Sem este degrau o vulcao so tem cinza e chama, e toda encosta acaba tendo
/// que escolher entre parecer apagada ou parecer em brasa.
pub const SCENE_CINDER: Color = Color::srgb(0.31, 0.14, 0.09);
/// Ferrugem: o material da fabrica, ao lado do aco.
pub const SCENE_RUST: Color = Color::srgb(0.37, 0.21, 0.11);
/// Verde vivo do acido no cenario -- o topo da rampa, nao o brilho do perigo.
pub const SCENE_ACID: Color = Color::srgb(0.29, 0.52, 0.17);
/// Jade na sombra: a massa das esculturas do jardim.
pub const SCENE_JADE: Color = Color::srgb(0.10, 0.30, 0.25);
/// Jade na luz: escama, borda e veio das mesmas esculturas.
pub const SCENE_JADE_LIT: Color = Color::srgb(0.20, 0.52, 0.39);

/// Cor de cada jogador por indice, para HUD e bonecos baterem.
pub fn player(id: u8) -> Color {
    match id {
        0 => P1,
        1 => P2,
        2 => P3,
        _ => P4,
    }
}

/// Resolve uma chave de mascara de cor para uma cor concreta.
///
/// Usado por [`super::art::Mask`]: a arte carrega a forma, uma segunda string
/// do mesmo tamanho carrega a cor, celula a celula.
pub fn key(c: char) -> Color {
    match c {
        'w' => BONE,
        'a' => ASH,
        'i' => IRON,
        'c' => COAL,
        '1' => P1,
        '2' => P2,
        '3' => P3,
        '4' => P4,
        'r' => BLOOD,
        'g' => GOLD,
        'm' => MOSS,
        'h' => MAGMA,
        'e' => EMBER,
        't' => TOXIC,
        's' => SLUDGE,
        _ => BONE,
    }
}
