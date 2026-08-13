---
name: ascii-art-modeling
description: Modelar, compor, implementar ou aprimorar arte ASCII/ANSI complexa para jogos e interfaces, tratando glifos como pincéis e sprites livres em vez de texto preso a uma grade. Usar ao criar personagens, armas, cenários, efeitos, animações, poses, paletas, sobreposições de glifos, sprites CP437 ou sistemas de renderização ASCII, especialmente neste projeto Bevy.
---

# Modelagem ASCII viva

Tratar ASCII como linguagem visual, não como limitação técnica. Usar caracteres para massa, contorno, material, luz, ritmo e gesto; permitir que cada glifo tenha posição, escala, rotação, cor, profundidade e movimento próprios.

**A orientação e o tamanho original do glifo não impõem limites.** Aqui o glifo é um sprite: o código pode girá-lo em qualquer ângulo, espelhar, esticar, achatar, aumentar até virar uma macroforma, reduzir até virar uma partícula, deslocar por frações de célula, recolorir, aplicar transparência/alpha e animar todas essas propriedades. Escolher o glifo pela pincelada que ele oferece e transformar o sprite até cumprir a forma desejada; não rejeitar uma ideia porque o caractere “aponta para o lado errado” ou “tem tamanho de uma célula”.

Antes de trabalhar neste repositório, ler [references/project-contract.md](references/project-contract.md). Para decisões de composição, materiais, camadas e animação, consultar [references/visual-language.md](references/visual-language.md). Para peças **grandes** — cenário, criatura, arquitetura — seguir o método de [references/pintura-generativa.md](references/pintura-generativa.md): ele traz o ciclo de trabalho, os geradores, a correção de proporção da célula e as armadilhas que só aparecem na tela.

## Princípio central

Não reduzir a obra a uma matriz monoespaçada pobre. Usar a grade apenas como uma ferramenta opcional para rascunhar blocos rígidos. No resultado em jogo:

- compor glifos como sprites em espaço contínuo;
- transformar cada glifo livremente — rotação, escala uniforme ou não uniforme, flip, cor e alpha;
- sobrepor glifo sobre glifo com entidades/camadas distintas;
- emendar armas, roupas, membros, faíscas e detalhes por hierarquia e pontos de encaixe;
- reservar strings `AsciiArt` para formas rígidas que realmente devam se mover juntas;
- separar partes que giram, recuam, piscam ou trocam de cor independentemente;
- misturar arte de glifo com efeitos não textuais quando isso reforçar impacto, sem apagar a identidade ASCII.

## Fluxo obrigatório

### 1. Descobrir o contrato existente

Buscar símbolos antes de abrir arquivos:

```powershell
rg -n "AsciiArt|AsciiSprite|Layer|Paint|PoseDef|Weapon|weapon_hand|palette" src/
```

Identificar:

- repertório de glifos e atlas;
- primitivas de composição e hierarquia;
- paleta e semântica de cores;
- fonte de verdade de pose/movimento;
- testes que protegem espelhamento, caixa, CP437 e sincronização.

Reutilizar contratos existentes antes de inventar outro sistema.

### 2. Escrever um micro-brief visual

Definir em poucas linhas:

- **silhueta:** o que deve ser reconhecido em miniatura;
- **gesto:** direção, peso e linha de ação;
- **materiais:** metal, osso, fumaça, carne, tecido, energia etc.;
- **foco:** 1–2 detalhes de maior contraste;
- **encaixes:** partes que dependem de mão, junta, cano, cabeça ou chão;
- **movimento:** antecipação, ação, contato e recuperação;
- **legibilidade:** o que deve continuar claro durante movimento rápido e espelhamento.

Não escolher glifos primeiro. Projetar volume, gesto e hierarquia visual primeiro.

### 3. Planejar a composição como cena

Classificar cada parte:

| Parte | Representação | Usar quando |
|---|---|---|
| Massa rígida | uma `AsciiArt` | células sempre compartilham o mesmo transform |
| Detalhe rígido colorido | `Paint`, `Accent`, `Mask` ou `stamp` | muda cor/forma sem movimento independente |
| Articulação/arma/acessório | entidade filha com `AsciiSprite` | precisa acompanhar um encaixe e ainda girar/recuar |
| Sobreposição no mesmo ponto | sprites irmãos/filhos com Z local | precisa de “glifo em cima de glifo” |
| Trilha, sangue, faísca | entidade efêmera | tem vida, deriva e cor próprias |
| Impacto além de glifo | `Effect`/sprite/malha | precisa de flash, onda ou distorção complementar |

Desenhar mentalmente um grafo de peças, não uma tela de terminal.

Decidir também **quem gera e quem se desenha**. Forma grande é repetição com
variação — coluna, andar, arco, escama, degrau, onda — e escrita à mão ela sai
com a linha de construção quebrada: uma junta a menos no meio da parede, um
corpo que vira três pedaços empilhados. Reservar a mão para o que é gesto:
cabeça, rosto, pose, silhueta de personagem. Ver
[references/pintura-generativa.md](references/pintura-generativa.md) §2.

### 4. Montar o preview antes da arte

Construir — uma vez — um renderizador de texto que imprima a composição já
montada, com o **glifo real** (nunca `#`) e **ordenada por profundidade**, atrás
de um teste `#[ignore]`. É a parte cara do ciclo, e depois dela cada iteração
custa segundos.

Conferir que o preview inverte o mapeamento do atlas corretamente: na CP437 os
gráficos de 0x00..0x1F voltam como caractere de controle na tabela ingênua, e
aí o preview fica invisível justamente em olho, crista e mostrador — os pontos
focais. Um preview que mente sobre o foco é pior que preview nenhum.

### 5. Pintar por função

Escolher cada glifo pelo trabalho visual que executa:

- linhas e barras para direção, estrutura e tensão;
- cantos, junções e cruzamentos para mecânica e encaixe;
- blocos e sombras para massa, oclusão e valor tonal;
- pontuação para poeira, brilho, fragmentos e transições;
- símbolos fortes para olhos, núcleo, lâmina, projétil ou ponto focal.

Usar densidade de glifo como valor: vazio → pontuação → `░` → `▒` → `▓` → `█`. Não preencher tudo; espaço negativo também modela.

A rampa de densidade **já é** a rampa de luz: escolher o glifo pelo volume que ele ocupa escolhe o tom junto. Pintar por tabela de glifo (`Tint`/`AsciiArt::tinted`), não por máscara paralela — máscara de trinta linhas sai defasada em uma coluna no meio, e isso não se lê no código, só na tela. Trocar a tabela faz o mesmo gerador virar basalto, jade, magma ou ácido.

Inverter a rampa para material translúcido: em rocha opaca o miolo pega luz; em jade, âmbar ou gelo, é a casca fina que acende. Sem essa inversão, jade é granito pintado de verde.

Lembrar que a célula é 8×16: círculo com o mesmo número de colunas e linhas sai ovo. E que `▀`/`▄` na mesma linha dão meia célula de resolução de graça — é assim que um beiral vira para cima sem gastar linha.

Validar todo glifo contra o atlas real. Neste projeto, não presumir que “Unicode visível” significa “CP437 disponível”.

### 6. Usar cor como iluminação e significado

Começar por base escura/neutra, acrescentar cor de material e terminar com acento escasso. Manter funções consistentes: jogador, arma, perigo, sangue, calor, veneno, interação.

Criar profundidade por contraste e temperatura, não por arco-íris. Fazer o foco ter maior contraste; rebaixar fundo e partes secundárias. Preferir a paleta do projeto a novos valores soltos.

### 7. Animar intenção, não apenas quadros

Construir movimento com:

1. pose legível de repouso;
2. antecipação curta;
3. arco principal com aceleração;
4. quadro de contato forte;
5. overshoot/recuo;
6. recuperação com amortecimento.

Mover peças por pivôs e encaixes. Fazer arma e mão consumirem a mesma fonte de verdade. Usar rotação, escala, offset subcélula, squash, recoil e trilhas; não redesenhar a sprite inteira quando só uma peça muda.

Para animação cíclica, garantir fechamento contínuo. Para locomoção, preferir fase ligada à distância quando isso evitar deslizamento dos pés.

Dirigir por uma **batida normalizada** (`0..1`) derivada de um relógio, não por uma pilha de timers por entidade: arte e zona de contato costumam ser entidades diferentes, e dois relógios que começam juntos terminam desencontrados depois de alguns minutos — a coluna aparece e não fere, ou fere invisível. O quadro de contato dispara uma vez por número, marcado por um booleano que zera no início; sem isso a faísca sai todo frame enquanto a batida está no intervalo.

Cenário parado é bonito na primeira vez e mudo na décima: dar a cada cena **uma** coisa que ela faz de tempos em tempos, e reduzir todas a um componente com relógio — enquanto forem um sistema por cena, só a que alguém estiver olhando ganha conserto.

Quando a colisão não puder acompanhar o desenho de graça, abrir a zona numa janela **mais estreita** que a do desenho, nunca igual: forma visível que ainda não machuca ensina; dano vindo do nada, não.

### 8. Implementar com parcimônia estrutural

- Reutilizar `AsciiArt`, `AsciiSprite`, `Layer`, `Paint`, poses e traits existentes.
- Atualizar arte apenas quando mudou; evitar reconstruir filhos todo frame sem necessidade.
- Manter física e apresentação desacopladas: inclinação, squash e recoil visuais não devem alterar collider por acidente.
- Dar a cada sistema uma única responsabilidade e uma única autoridade de escrita.
- Adicionar uma abstração nova apenas quando duas ou mais peças reais exigirem o mesmo contrato.

### 9. Verificar em movimento

Executar, quando aplicável:

```powershell
cargo fmt --check && cargo test
```

Também conferir:

- leitura da silhueta parada e em velocidade;
- espelhamento de `/`, `\`, setas, cantos e pivôs;
- arma realmente colada à mão em todas as poses;
- sobreposições sem z-fighting;
- caixa/pivô sem salto entre quadros;
- contraste em fundos claros e escuros;
- ausência de fallback `?` para glifos novos;
- trilhas e partículas sem esconder o quadro de contato.

Para composição grande, cobrir com teste o que não quebra nada quando erra —
peça enterrada, peça fora da tela, plano largo que descobre a borda no deslize
máximo, carimbo em cima de carimbo, cor de gameplay roubada pelo cenário,
emenda de peças que se solta, cena que não monta. A varredura tem que passar
pela **composição montada**, e não por uma lista de strings: é isso que a faz
cobrir o que é gerado, que é justamente o que ninguém pensa em conferir. Arte
animada precisa ser **descrita antes de existir** pelo mesmo motivo — o que
nasce direto em `Commands` escapa de toda conferência.

Se a mudança visual for importante, rodar o jogo e inspecionar o resultado; testes unitários não provam composição.

## Critério de qualidade

Entregar uma pintura articulada, não um boneco de palitos genérico. O resultado deve ter:

- silhueta reconhecível;
- gesto e peso;
- pelo menos três níveis de valor/densidade quando a escala permitir;
- hierarquia de cor;
- camadas e oclusão deliberadas;
- detalhes ligados a pivôs corretos;
- movimento com antecipação e consequência;
- consistência com atlas, paleta e arquitetura do projeto.

## Proibições

- Não propor terminal, `ratatui` ou buffer de células para este projeto.
- Não entregar apenas um bloco de texto estático quando o pedido exige integração no jogo.
- Não achatar arma, rosto, roupa e FX numa única string se eles precisam se mover separadamente.
- Não usar caracteres combinantes como atalho para sobreposição; sobrepor sprites reais.
- Não adicionar glifos fora do atlas, emoji ou fallback silencioso.
- Não espalhar cores sem semântica nem tornar todos os planos igualmente contrastados.
- Não duplicar coordenadas de mão/arma ou pose em sistemas diferentes.
- Não confundir detalhe com ruído: cada marca deve explicar forma, material, luz ou movimento.
- Não escrever à mão o que é repetição com variação — parede, telhado, arco, escama, onda. Gerar.
- Não desenhar às cegas: sem preview que imprima o glifo real, não há iteração, só primeiro rascunho.
- Não digitar a coordenada de uma emenda entre duas peças; derivar dos tamanhos e travar com teste.
- Não sortear dentro de gerador: arte que muda entre partidas não é arte, é ruído.
- Não deixar duas peças se cruzarem no mesmo plano de profundidade.
