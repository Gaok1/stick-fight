# Glyph Forge

Editor local de composicoes e animacoes feitas com glyphs ASCII. A fonte padrao e a mesma ROM IBM VGA 8x16 CP437 de `assets/fonts/ibm_vga_8x16.bin` usada pelo jogo. Cada glyph permanece como um objeto com posicao, cor, escala, rotacao e camada, para que uma LLM possa entender o modelo sem depender apenas de uma imagem.

Uma composicao pode virar um boneco articulado: pecas ligadas a pontos do rig, animacoes que guardam so a diferenca em relacao a pose de repouso, e peles que repintam tudo sem duplicar uma pose sequer -- que e exatamente como o jogo separa `pose`, `rig` e `skin`.

## Executar

```powershell
cd glyph_forge
python -m pip install -r requirements.txt
python app.py
```

Atalhos: `Ctrl+N` novo, `Ctrl+O` abrir, `Ctrl+S` salvar, `Ctrl+Shift+S` salvar como, `Ctrl+E` exportar tudo, `Ctrl+Z/Y` desfazer/refazer, `Ctrl+C/V` copiar/colar pecas, `Ctrl+D` duplicar, `Ctrl+A` selecionar tudo no canvas, `Ctrl+scroll` aplicar zoom sob o cursor, `Ctrl+0` voltar a 100%, setas mover, `Shift+setas` mover mais rapido, `Esc` limpar e `Delete` excluir. Com o foco no canvas, `espaco` toca ou para a animacao e `,` / `.` andam um quadro.

No canvas: **arrastar com o botao do meio** empurra a vista, a roda rola, `Shift+roda` rola de lado e `Ctrl+roda` aplica zoom sob o cursor.

## O boneco do jogo

```powershell
cd glyph_forge
python bake_actor.py
```

Escreve `creations/bonecos/boneco.glyph.json` com o boneco inteiro: as 21 poses de `src/actor/rig.rs` em 13 animacoes, as 6 peles de `src/actor/skin.rs` e o rig de 12 pontos. Abra no editor, ajuste, e o que voltar para o Rust sao as coordenadas dos pontos -- elas sao os mesmos numeros de `Joints` em `rig.rs`, so deslocadas pela origem do canvas:

```
x_ator = x_canvas - 56   |   y_ator = 64 - y_canvas
```

`python bake_actor.py --self-test` confere a transcricao contra os numeros do Rust: se uma articulacao discordar, o teste diz qual. Quando `rig.rs` ou `pose.rs` mudarem, atualize as tabelas no topo de `bake_actor.py` e rode de novo.

## A janela

A barra de cima agrupa os comandos pelo que eles fazem -- **arquivo** (novo, abrir, salvar, importar, exportar, conferir) na primeira linha; **criar**, **editar**, **ver** na segunda. **Exportar** e um botao so: ele abre com as duas saidas, o pacote inteiro e a peca selecionada. Passar o mouse por qualquer botao mostra o que ele faz, o atalho e, quando existe, o que precisa estar selecionado.

A direita ficam seis abas:

- **Objetos** -- a lista do que existe na cena: glyphs, pontos, destaques e rotulos, com um filtro no topo. Clicar numa linha seleciona a peca no canvas. E o jeito de achar o cotovelo de tras sem cacar entre vinte e cinco pecas sobrepostas num boneco de 64px.
- **Peca** -- o que estiver selecionado agora, em tres secoes: **Desenho** (o que a peca e), **Transformar** (onde ela esta) e **No rig** (como ela se prende). Ela troca de formulario sozinha entre glyph, articulacao e ponto de atencao, e **nao** rouba o foco das outras abas: da para ficar na aba Animacao clicando em pontos do rig o dia inteiro.
- **Animacao** -- criar, renomear, reordenar animacoes e quadros.
- **Rotulos** -- os conjuntos nomeados que o jogo le.
- **Peles** -- as variantes de glifo e cor.
- **Projeto** -- canvas, acento e notas.

Os campos aplicam ao sair deles, e nao so no `Enter`: digitar um numero e clicar em outro lugar vale. Os paineis rolam quando nao cabem.

Sob o canvas, a barra de transporte escolhe a animacao e toca; abaixo dela, a **tira de quadros** e o mapa: um botao por quadro, o atual afundado, amarelo para quadro que muda alguma coisa e `*` para quadro com marca.

No grupo VER, **Nomes** desliga os nomes dos pontos quando eles se sobrepoem -- o nome do ponto selecionado continua aparecendo.

## Peles e animacoes novas

```powershell
python revamp.py
```

Escreve `creations/bonecos/boneco_novo.glyph.json`: o boneco transcrito **mais** o que o jogo ainda nao tem -- 5 peles e 5 animacoes autorais (provocacao, aterrissagem, vitoria, nocaute, esquiva).

Fica num arquivo separado de proposito. `bake_actor.py` e transcricao e tem que continuar batendo com `src/actor/`; `revamp.py` e autoral e nao tem com o que bater. Abrir, conferir rodando, apagar o que nao prestar -- o que sobrar vira `skin.rs` e `rig.rs`.

Duas regras que sairam de renderizar candidatas e olhar:

- o glifo do **membro** precisa de forma vertical solida. `·`, `!` e `§` esticados viram tracinho picado e o membro some;
- o glifo do **tronco** precisa de continuidade vertical. `▄` e `♦` quebram a silhueta e o boneco parece desmontado.

E `¦`, `†`, `¤` e `Ξ` **nao existem na CP437**: viram `?` na tela sem avisar.

## Fluxo

1. Clique em **Glyph**, no grupo CRIAR da barra.
2. Use **Abrir tabela CP437** para escolher visualmente um dos 256 glyphs da fonte do jogo, ou edite o texto manualmente. A tabela permanece aberta para escolhas repetidas e se minimiza ao perder o foco; restaure-a pelo `Alt+Tab` ou pelo mesmo botao.
3. Selecione o objeto e edite posicao, tamanho, escalas, rotacao, cor e camada. O color picker possui campo 2D de saturacao/luminosidade, faixa de matiz, cores rapidas e hexadecimal; tudo atualiza ao vivo, sem botao Aplicar.
4. Arraste objetos diretamente no canvas. Um glyph unico exibe uma caixa orientada, quatro quadrados de escala e uma alca circular de rotacao. O canto oposto fica fixo e a alca acompanha o mouse; segure `Shift` para manter a proporcao. Os cursores mudam sobre escala e rotacao, e cruzar o canto oposto espelha a peca automaticamente. **Espelhar X/Y** tambem pode ser acionado na aba **Peca**.
5. Arraste no fundo para criar uma selecao multipla; `Shift+clique` adiciona ou remove itens. Ative **Grade** e **Snap** para alinhar as pecas ao espacamento definido na aba **Projeto**.
6. Selecione duas pecas e clique em **Ponto**. Ele conecta automaticamente Peca A e Peca B.
7. Com o ponto selecionado, na aba **Peca**, escolha entre pivo com giro ou ligacao fixa, nomeie/descreva e marque se tambem e uma ancora no mundo.
8. Repita para montar a estrutura. Linhas tracejadas azuis mostram quais duas pecas cada articulacao conecta.
9. Escreva instrucoes na aba **Projeto > Notas para a LLM** e exporte.

Articulacoes nao pertencem a um glyph e nao o acompanham automaticamente: selecione as pecas e o ponto juntos quando quiser mover todo o conjunto. O botao **Rig**, no grupo VER, alterna a sobreposicao estrutural.

## Articulacoes e pontos de atencao

Para montar um rig, selecione os dois glyphs que representam as pecas e clique em **Ponto**. O ponto nasce entre elas; arraste-o ate o pivo real. **Peca A** e **Peca B** registram os dois corpos conectados. **Pivo** prende a posicao mas permite giro relativo; **Fixa** representa uma solda. **Ancora fixa no mundo** prende o ponto tambem ao cenario. Nome e descricao sao exportados para a LLM.

Use **Atencao** quando quiser apenas destacar uma regiao sem criar um osso. O losango rosa possui nome, descricao opcional e pode ser fixado a um glyph para acompanha-lo. Esses destaques sao exportados em `attention_points` e aparecem em `rig_preview.png`.

Para nomear uma peca logica inteira, selecione um ou varios glyphs e clique em **Rotulo**. Na aba **Rotulos**, informe nome e descricao opcional. Em **Sub-rotulos**, um conjunto pode conter outros conjuntos; a selecao, o contorno e a exportacao resolvem todos os membros recursivamente. No JSON, `element_ids` guarda glyphs diretos e `label_ids` referencia os conjuntos internos. Projetos antigos em que um rotulo era um subconjunto exato de outro sao reconhecidos e migrados automaticamente ao abrir.

## Props: uma peca presa a um ponto

Selecione a peca e **um** ponto e use **Prender a um ponto**, na aba **Peca**. A distancia de agora vira a empunhadura: a peca nao se mexe ao ser presa, mas dali em diante o ponto a carrega.

E o que faz animar golpe com arma ser possivel a mao. Voce move a mao e a arma vai junto; o que sobra para desenhar quadro a quadro e o **giro** dela, que e justamente a metade autoral do golpe. No JSON, `follow` e o ponto e `offset` e a distancia -- a posicao nunca e gravada, entao o quadro guarda a mao (o numero que o jogo tem) e a empunhadura, e nunca os dois discordam.

Arrastar uma peca presa muda a empunhadura, e nao o lugar. O mesmo botao, sem nenhum ponto selecionado, solta.

### Receita do melee

1. Modele a arma no proprio arquivo (`.glyph.json`), com rotulos e pontos de atencao nomeados (`pegada`, `boca`, `ponta`).
2. Abra `creations/bonecos/boneco.glyph.json` e **Importar** a arma.
3. Selecione as pecas dela e o ponto `mao_frente`, e **Prender a um ponto**.
4. Crie a animacao, e em cada quadro mova os pontos do rig e gire a arma.
5. Marque o quadro do impacto com `contato` em **Marcas**.
6. **Conferir**, e exportar.

As animacoes proprias da arma nao viajam no Importar -- so a geometria. Se a arma tiver clipes proprios (um livro que pulsa), eles ficam no arquivo dela.

## Papel da peca

O campo **Papel da peca** e texto livre. `corpo` e `membro` sao os que a pele entende; qualquer outro (`bolt`, `sight`, `pump`, `muzzle`, `arma`) e para o jogo ler e decidir o que se mexe sozinho. Sem ele, toda peca importada chega no Rust como decoracao parada.

## Conferir

O botao **Conferir** procura o que o jogo nao vai conseguir ler: nome repetido entre pontos, rotulos ou animacoes (o Rust acha peca por nome, e nome repetido pega o errado), segmento ou prop apontando para ponto que sumiu, rotulo vazio, e quadro guardando peca que nao existe mais. Exportar tambem avisa antes.

## Segmentos: uma peca entre dois pontos

Selecione um glyph e dois pontos (`Shift+clique`) e use **Virar segmento entre 2 pontos**, na aba **Peca**. A peca vira um segmento: posicao, giro e altura passam a sair dos dois pontos, e arrastar um deles dobra o membro. As alcas de escala e giro somem porque nao mandam mais em nada; `Escala X` continua sendo a espessura.

E isso que faz um quadro guardar a coordenada da articulacao em vez de meio-caminho, angulo e escala -- que sao numeros que ninguem reverte a mao. O mesmo botao, com nenhum ponto selecionado, desfaz a ligacao. Arrastar o segmento leva as duas pontas junto; excluir um ponto solta o segmento em vez de deixa-lo preso a um id morto.

## Animacao

A aba **Animacao** e a barra sob o canvas trabalham juntas: a barra escolhe a animacao e o quadro e toca; a aba cria, renomeia e reordena.

Uma animacao e uma lista de quadros. **A cena salva e sempre a pose de repouso** e cada quadro guarda so o que muda em relacao a ela -- entao mexer no repouso alcanca todos os quadros de uma vez, e um quadro que so levanta um braco tem duas linhas de JSON.

1. **Repouso** volta a pose base. Editar aqui vale para toda a animacao.
2. Escolha ou crie uma animacao e clique em **+ Quadro**: o quadro nasce com a pose que esta na tela.
3. Com o quadro selecionado, arraste as pecas e os pontos. A diferenca fica gravada nele sozinha -- nao ha botao de gravar.
4. **Sombra** desenha o quadro anterior atras do atual, para comparar.
5. **Tocar** roda a animacao no proprio canvas, respeitando `Quadros por segundo` e a `Duracao` de cada quadro.

`Duracao (tempos)` e por quantos tempos o quadro fica na tela: dobrar um quadro nao exige duplica-lo. `Papel de cor` diz se o corpo, neste quadro, usa a cor de vivo, ferida ou morte da pele. `Marcas` diz o que acontece ali -- `contato`, `brilho`, `som` -- em nomes livres; e assim que "o golpe acerta no segundo quadro" chega ao Rust como dado em vez de palpite.

No JSON, `animation.clips[].frames[].keys` e `id da peca -> campo -> valor`. Os campos possiveis sao `x`, `y`, `rotation`, `scale_x`, `scale_y`, `flip_x`, `flip_y`, `glyph`, `color`, `font_size` e `layer`. Pontos do rig entram nas mesmas chaves, pelo id deles.

## Peles

Uma pele decide glifo e cor das pecas conforme o **Papel da peca** de cada uma, na aba **Peca**:

- **corpo** -- a cor sai da pele (`Corpo`, `Ferido` ou `Morto`, conforme o papel de cor do quadro), ou de `canvas.accent` quando o glifo esta em **Glifos com acento**;
- **membro** -- o glifo vira **Glifo do membro** e a cor, `Membros`;
- **(cor propria)** -- a peca fica intacta. Um projeto sem peles funciona como antes.

**Trocas** substitui caracteres na silhueta inteira (`O=0, |=║`) e roda *depois* de a cor estar decidida, entao uma pele que troque um caractere acentuado nao perde a cor de quem veste. Para a pele que redesenha um quadro so, `art` no JSON aceita `quadro -> peca -> glifo`.

Trocar de pele repinta tudo sem tocar na cena: a mesma animacao serve todas elas.

**Exportar selecao** salva somente a peca selecionada em `.glyph-piece.json`; **Importar** mescla uma peca ou cena JSON ao projeto atual, gerando IDs novos, ou transforma um `.txt`/`.asc` em um objeto ASCII editavel. Copiar, colar e duplicar preservam as articulacoes associadas e suas relacoes internas.

A exportacao cria:

- `scene.json`: modelo exato e fonte de verdade;
- `preview.png`: arte limpa;
- `rig_preview.png`: arte com pontos, nomes, ossos e ancoras;
- `animacao/<nome>.gif`: cada animacao rodando, e um PNG por quadro ao lado;
- `flattened.txt`: aproximacao em uma grade ASCII;
- `prompt.md`: notas e instrucao de leitura.

As imagens saem com a pele que estiver vestida na hora.

**Exportar como** pede somente o nome e cria a pasta completa ao lado do projeto salvo. Se o projeto ainda nao foi salvo, usa `glyph_forge/exports/<nome>`. **Salvar como** cria outra copia editavel e passa a trabalhar nela.

Em `scene.json`, cada item de `rig.joints` possui `parent_id` para a hierarquia, `attached_element_id` para o glyph que usa o pivo e `fixed` para ancoras presas ao mundo.

Para validar as funcoes de renderizacao sem abrir a interface:

```powershell
python app.py --self-test
```
