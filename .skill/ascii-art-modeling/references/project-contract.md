# Contrato visual de Stick Fight ASCII

## Regra de leitura

Confirmar os detalhes no código antes de editar; este mapa serve para orientar buscas pontuais.

## Arquitetura existente

- `src/ascii/cp437.rs`: converte a ROM VGA 8×16 embutida em atlas e mapeia caracteres com `glyph_index`.
- `src/ascii/art.rs`: define `AsciiArt`, `Cell`, `Paint`, `Solid`, `Mask`, `Accent`, `Tint`, `stamp`, espelhamento e recoloração.
- `src/ascii/sprite.rs`: transforma cada célula não vazia em um `Sprite` filho e aplica `Transform`, pivô, flip e `Layer`.
- `src/ascii/palette.rs`: concentra a paleta semântica. Reutilizar constantes antes de criar cores.
- `src/actor/rig.rs`: guarda poses, clipes, pivôs e geometria visual do corpo.
- `src/actor/pose.rs`: expõe a coreografia compartilhada, incluindo `weapon_hand`.
- `src/actor/motion.rs`: anima membros e movimentos visuais sem contaminar física.
- `src/actor/face.rs`: demonstra composição de peças faciais como sprites filhos independentes.
- `src/actor/skin.rs`: separa silhueta, glifos e pintura de jogador.
- `src/weapon.rs`: define `Weapon`, artes na mão/chão, projéteis, arma filha, recoil e arremesso.
- `src/fx.rs`, `src/gore.rs`, `src/backdrop.rs`: exemplos de partículas, efeitos, cor e movimento em camadas.

## Primitivas preferidas

### `AsciiArt`

Usar para um bloco rígido de células. Espaços são transparentes; `cols` e `rows` preservam a caixa. Aplicar:

- `AsciiArt::solid` para cor única;
- `AsciiArt::tinted` (`Tint`) para material por glifo — a rampa `░▒▓█` vira rampa de luz numa string só, sem máscara paralela para sair defasada;
- `AsciiArt::build` com `Accent` ou outro `Paint` para cor por regra;
- `AsciiArt::masked` só quando a cor não seguir o glifo;
- `stamp` para fundir subarte rígida em uma composição estática;
- `mirrored`/`flip_x` para direção;
- `recolored` para flashes integrais.

Não usar `stamp` para uma peça que precisa de transform ou ciclo de vida independente.

### Geradores de forma grande (`src/backdrop.rs`)

Antes de escrever arte grande à mão, conferir se um destes já resolve — e, se não, escrever mais um em vez de desenhar linha a linha:

`draw` (a base: `(col,row) -> char`), `shade` (rampa de densidade), `ridge`, `cone`, `smog`, `basalt`, `vault`, `arcade`, `terrace`, `cascade`, `current`, `disc` (círculo com a proporção 8×16 corrigida), `furnace`, `vat`, `pipeline`, `gantry`, `drains`, `suspension`, `pagoda`, `gate`, `jade_pillar`, `bamboo`, `serpent`.

As tabelas de material (`STONE`, `STEEL`, `MAGMA_FALL`, `ACID_FALL`, `JADE_SKIN`, `TIMBER`, `LACQUER`, …) são o que faz o mesmo gerador servir temas diferentes: trocar a tabela, não o gerador.

### Peças que se mexem

- `Panel` — composição parada, devolvida por `panels(scene)`.
- `Reel`/`Flipbook` — quadros em ciclo (queda, cascata, núcleo pulsando), descritos por `reels(scene)`.
- `Landmark`/`Show` — o acontecimento periódico de uma cena (erupção, martelo, sopro, purga).
- `Parallax`/`Sway` — profundidade e bamboleio; **só** `drift_planes` escreve `Transform`.

`Panel` e `Reel` existem como dado antes de virarem entidade justamente para os testes conseguirem varrê-los. Peça nova que nasce direto em `Commands` escapa da varredura de CP437, de cor reservada e de composição.

### `AsciiSprite`

Usar como unidade posicionável no mundo. `new` ancora no centro; `footed` ancora na base. Mudar `art`, `pivot` ou `flip_x` provoca rebuild dos glifos filhos. Portanto:

- evitar reatribuir uma arte idêntica;
- escolher pivô pela função física/visual;
- pendurar acessórios em entidades apropriadas com `ChildOf`;
- aplicar pequeno Z local para sobreposição controlada.

### `Layer`

Usar os planos existentes para fundo, cenário, pickups, atores, efeitos e UI. Para glyph-on-glyph dentro do mesmo plano, usar offsets locais pequenos e determinísticos em Z, sem criar competição entre sistemas.

## Padrão para armas emendadas

1. Manter a arte da arma em `Weapon::held_art`/`ground_art` quando for rígida.
2. Gerar a arma empunhada como `WeaponIcon`, filha do ator.
3. Ler posição e ângulo da mão por `pose::weapon_hand`.
4. Espelhar arte e transform a partir do mesmo facing.
5. Aplicar recoil sobre esse resultado, não sobre coordenadas paralelas.
6. Separar partes móveis adicionais — corrente, mira, chama, carregador — em filhos da arma quando necessário.

## Padrão para glifo sobre glifo

Não depender de combining marks Unicode. Gerar duas ou mais entidades de glifo com:

- mesmo pai ou relação pai/filho explícita;
- offsets `Vec2` livres, inclusive frações de `CELL`;
- escala e rotação independentes;
- cores e alpha próprios;
- Z local ordenado;
- autoridade de atualização clara.

Isso permite olho sobre rosto, brilho sobre lâmina, sangue sobre corpo, chama sobre cano e runa sobre arma sem fingir que existe uma única célula textual.

## Invariantes a preservar

- Todo glifo de conteúdo deve resolver no atlas CP437, nunca no fallback `?`.
- Poses equivalentes devem manter caixas estáveis para não saltar ao virar ou animar.
- Glifos direcionais devem trocar corretamente no espelho.
- Arma e mão devem compartilhar a mesma coreografia.
- Mudança visual não deve alterar hitbox, gravidade ou timing sem intenção explícita.
- Conteúdo novo deve entrar nos catálogos/tabelas percorridos pelos testes.
- Duas peças no mesmo plano não podem se sobrepor: mesmo Z é sorteio da ordem de spawn.
- Gerador não sorteia — variação vem de conta sobre `(col, row, quadro)`, senão a arte muda entre partidas e entre execuções do teste.
- Cenário não usa as cores reservadas ao gameplay; evento momentâneo pode romper isso de propósito, e só ele.

## Buscas úteis

```powershell
rg -n "pub struct AsciiArt|pub fn stamp|fn mirror_glyph|struct Tint" src/ascii/art.rs
rg -n "fn draw|fn shade|struct Panel|struct Reel|struct Landmark" src/backdrop.rs
rg -n "pub struct AsciiSprite|pub enum Layer|rebuild_glyphs" src/ascii/sprite.rs
rg -n "pub trait Weapon|weapon_hand|WeaponIcon|Recoiling" src/weapon.rs src/actor/
rg -n "glyph_index|fallback|CP437" src/ docs/
rg -n "cargo test|espelh|arma|pose|glifo" README.md src/
```

## Validação mínima

```powershell
cargo fmt --check && cargo test
```

Para composição de fundo, olhar antes de rodar o jogo — o preview imprime os
planos montados, com o glifo real e ordenados por profundidade:

```powershell
cargo test olhar_o_fundo -- --ignored --nocapture
```

Para mudanças visuais, executar também `cargo run` e observar composição, timing, sobreposição e contraste.
