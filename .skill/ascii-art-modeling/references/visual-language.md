# Linguagem visual para pintura com glifos

## Sumário

1. Unidade visual
2. Vocabulário de pincéis
3. Modelagem de volume
4. Sobreposição e hierarquia
5. Cor e luz
6. Movimento
7. Escala de complexidade
8. Fontes da pesquisa

## 1. Unidade visual

Tratar “glifo” como amostra de forma proveniente de um atlas, não como caractere semântico. Um glifo pode ser:

- pincel de contorno;
- mancha de valor;
- textura de material;
- articulação;
- partícula;
- decalque;
- luz ou reflexo;
- marcador de movimento.

Uma composição pode manter origem em arte ASCII mesmo quando os glifos deixam de compartilhar linhas, colunas, tamanho ou orientação.

Não tratar a forma impressa do caractere como pose final. Depois de escolhido no atlas, ele é um sprite comum: girar por qualquer ângulo, espelhar, aplicar escala uniforme ou diferente em X/Y, torná-lo enorme ou minúsculo, usar offsets subcélula, recolorir e variar transparência. Um `─` pode virar haste diagonal; um `█` ampliado pode virar uma massa; um `·` reduzido e translúcido pode virar poeira. A célula de origem é apenas o molde do pincel.

## 2. Vocabulário de pincéis

### Estrutura e direção

- `-`, `─`, `═`: tensão e comprimento horizontal;
- `|`, `│`, `║`: suporte vertical;
- `/`, `\`: diagonais, membros, lâminas e gesto;
- cantos e junções: encaixes mecânicos, coronhas, guarda, arquitetura;
- setas e triângulos: ponta, direção e agressão.

### Massa e valor

- `█`: núcleo sólido, sombra fechada, impacto;
- `▓`: massa densa e material pesado;
- `▒`: meia-luz, ruído controlado, fumaça densa;
- `░`: atmosfera, desgaste, transição;
- espaço/transparência: recorte, respiração e silhueta negativa.

### Energia e matéria solta

- `.`, `·`, `∙`, `°`: poeira, distância, fumaça e brilho pequeno;
- `*`, `+`, `×`: faísca, contato e explosão;
- `'`, `,`, `` ` ``: lasca, gotejo e cauda de movimento;
- círculos/diamantes disponíveis no atlas: núcleos, olhos, projéteis e joias.

Escolher apenas glifos que existam no repertório do projeto.

## 3. Modelagem de volume

Construir do grande para o pequeno:

1. bloquear a silhueta;
2. estabelecer eixo e linha de ação;
3. dividir planos frontal, médio e traseiro;
4. definir luz/sombra com densidade;
5. indicar material por borda e textura;
6. acrescentar detalhes focais por último.

Para metal, usar bordas duras, retas, junções e highlight curto. Para tecido, quebrar contorno e deixar cauda/folga. Para osso, usar segmentos claros e articulações pontuais. Para fumaça, dissolver densidade e borda. Para energia, inverter a lógica: núcleo claro, halo colorido e partículas divergentes.

## 4. Sobreposição e hierarquia

Sobrepor sprites em vez de sobreimprimir texto. A sobreposição histórica de caracteres demonstra que múltiplas marcas podem produzir um tom/forma mais rico, mas em renderização por atlas a versão robusta é uma pilha explícita de entidades.

Ordenar camadas por função:

1. sombra/halo traseiro;
2. massa principal;
3. articulações e acessórios;
4. contorno seletivo;
5. highlight/runa/olho;
6. partículas e flash frontal.

Evitar duplicar toda a forma em cada plano. Fazer cada camada contribuir algo que as outras não conseguem.

## 5. Cor e luz

Separar forma de pintura. Aplicar uma paleta curta:

- neutro escuro para recuo;
- tom base para material/identidade;
- sombra ou densidade para volume;
- acento saturado para foco/estado;
- highlight quente ou frio para contato e energia.

Usar alpha e brilho transitórios para eventos. Conservar acento: se tudo brilha, nada brilha.

## 6. Movimento

### Princípios

- **antecipação:** deslocar peso contra a direção da ação;
- **arco:** girar membros/armas ao redor de pivôs reais;
- **spacing:** aumentar distância entre amostras durante aceleração;
- **impacto:** comprimir duração, elevar contraste e congelar por instantes quando apropriado;
- **follow-through:** deixar acessórios, tecido, fumaça e sangue atrasarem;
- **overshoot:** ultrapassar o destino e retornar;
- **secondary motion:** mover detalhes depois da massa principal.

### Pincelada temporal

Não limitar animação a trocar strings. Variar `Transform`, escala, rotação, cor, alpha, Z e existência. Uma arma pode conservar sua arte rígida e ainda ganhar vida por recoil, giro, atraso de corrente, brilho de cano e trilha.

### Continuidade

Em loops, fazer posição e velocidade visual fecharem. Em ataque, garantir que antecipação e recuperação não ocupem o mesmo desenho que o contato. Em espelhamento, conservar “frente” sem duplicar tabelas.

## 7. Escala de complexidade

### Ícone pequeno

Priorizar silhueta, uma diagonal dominante e um acento. Não tentar representar toda peça real.

### Personagem/arma jogável

Usar massa reconhecível, 2–4 peças articuladas, cor semântica, ponto focal e poses extremas distintas.

### Cenário ou chefe

Combinar macroformas, parallax, planos tonais, detalhes repetidos com variação, partículas ambientais e movimentos secundários. Manter zonas de descanso para o jogador continuar legível.

## 8. Fontes da pesquisa

- Unicode Standard, capítulo 22: Block Elements representam frações/preenchimentos e níveis de sombreamento; quadrantes complementam meios-blocos. https://www.unicode.org/versions/Unicode17.0.0/core-spec/chapter-22/
- Unicode Standard, capítulo 21: Braille oferece 256 padrões de oito pontos, útil como referência histórica de densidade, embora este projeto deva respeitar seu atlas CP437. https://www.unicode.org/versions/Unicode17.0.0/core-spec/chapter-21/
- Unicode, Character Encoding Mappings: registra a tabela CP437 para interoperabilidade entre código legado e Unicode. https://www.unicode.org/L2/L1999/99325-N.htm
- Bevy, exemplo Sprite Sheet: demonstra atlas, `Transform`, filtro nearest e troca temporal de índices. https://bevy.org/examples/2d-rendering/sprite-sheet/
- Bevy `Sprite`: suporta imagem de atlas e tamanho/transform visual por sprite. https://docs.rs/bevy/latest/bevy/sprite/struct.Sprite.html
- Bevy `ChildOf`: a hierarquia mantém relações pai/filho e propaga composição de transform. https://docs.rs/bevy/latest/bevy/ecs/hierarchy/struct.ChildOf.html

Conclusão aplicada: blocos e caracteres fornecem vocabulário visual, mas atlas + hierarquia + transforms removem a obrigação de grade e tornam sobreposição, articulação e animação propriedades explícitas da pintura.
