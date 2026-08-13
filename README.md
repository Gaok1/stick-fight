# Stick Fight ASCII

Jogo de briga PVP local, 2D, com arte ASCII em página de código 437 — o mesmo
repertório de glifos do Dwarf Fortress (`█▓▒░`, `─│┌┐└┘`, `☺♦▲`).

**ASCII aqui é estilo de arte, não meio de saída.** O jogo não roda em terminal
e não está preso a um grid de caracteres: cada glifo é um sprite com `Transform`
próprio em espaço contínuo. Posição, escala e rotação são float livre, e a
camada de efeitos desenha por cima sem passar por glifo nenhum.

```
       O                     O
      /|\        ╫          /|=
       |         ╫           |
      / \        ╫          / \
  ════════      ╫       ══════════
█████████████   ╫    ▄▄▄▄▄▄▄▄▄▄▄▄▄▄
▒▒▒▒▒▒▒▒▒▒▒▒▒        ▒▒▒▒▒▒▒▒▒▒▒▒▒▒
▒▒▒▒▒▒▒▒▒▒▒▒▒  ← buraco →  ▒▒▒▒▒▒▒▒
```

## Rodar

```bash
cargo run --release
```

O primeiro build compila o Bevy inteiro e demora. Os seguintes são rápidos.

## Releases

Um push de tag `v*` cria uma GitHub Release com bundles jogáveis para Windows e
Linux, incluindo executável, músicas, `steam_appid.txt` e a biblioteca da Steam:

```bash
git tag v0.1.0
git push origin v0.1.0
```

Baixe o arquivo da sua plataforma, extraia a pasta inteira e execute
`stick-fightt.exe` (Windows) ou `./stick-fightt` (Linux).

## Cooperação entre agentes

Antes de **ler ou editar qualquer arquivo**, registre a intenção em
[`agents.lock`](agents.lock), usando um identificador único no formato
`agente-tarefa`. O arquivo é YAML e cada entrada pode declarar:

```yaml
version: 1
locks:
  agente-tarefa-tal:
    read:
      - src/main.rs
    writing:
      - src/foo.rs
    future_writing:
      - src/bar.rs
```

- `read`: arquivos sendo consultados. Vários agentes podem compartilhar leitura;
- `writing`: arquivos sendo alterados. É exclusivo e conflita com qualquer
  `read`, `writing` ou `future_writing` de outro agente no mesmo caminho;
- `future_writing`: reserva exclusiva de arquivos que serão alterados mais
  adiante na tarefa, evitando trabalho paralelo incompatível.

Use caminhos relativos à raiz do repositório, com `/`. Um caminho de diretório
terminado em `/` cobre tudo abaixo dele. Antes de adicionar uma entrada,
verifique todas as entradas existentes e não prossiga enquanto houver conflito.
Ao atualizar `agents.lock`, preserve os locks dos outros agentes; o próprio
arquivo é a única exceção à exigência de lock, somente para registrar ou remover
a sua entrada. Remova seus locks imediatamente ao terminar, cancelar ou entregar
a tarefa para outro agente. Locks sem tarefa ativa só podem ser removidos depois
de confirmar que o agente proprietário não está mais trabalhando.

### Steam / online

O modo `ONLINE` usa lobby da Steam e `ISteamNetworkingMessages` P2P (com relay
da Valve quando necessario). Para testar, abra a Steam antes do jogo; o
`steam_appid.txt` usa o App ID de desenvolvimento `480` (Spacewar). No lobby:

- `Enter` cria uma sala publica ou, para o host, inicia a luta;
- `F` procura e entra na primeira sala aberta deste jogo;
- `I` abre o convite de amigos no overlay da Steam;
- `Esc` sai da partida/sala.

Para publicar, troque `480` pelo App ID real no `steam_appid.txt`, configure
lobbies/overlay no Steamworks e distribua o redistribuivel `steam_api64.dll`
ao lado do executavel no Windows. Os dois jogadores precisam usar a mesma
build e contas Steam diferentes.

## Controles

|          | Jogador 1 | Jogador 2      |
| -------- | --------- | -------------- |
| Andar    | `A` / `D` | `←` / `→`      |
| Pular    | `W`       | `↑`            |
| Agarrar/soltar corrente | `F` | `Numpad 1` |
| Escalar/balançar | `W` / `S`, `A` / `D` | `↑` / `↓`, `←` / `→` |
| Descer plataforma | `S` | `↓`         |
| Combo corpo-a-corpo | `Mouse 1` | `Numpad 0` |
| Rasteira | `S` + `Mouse 1` | `↓` + `Numpad 0` |
| Voadora | `Mouse 1` no ar | `Numpad 0` no ar |
| Especial / disparar | `Mouse 2` (ou `R`) | `Numpad 2` |
| Arremessar arma | `G` | `Numpad 4` |
| Parry | `Q` | `Numpad 3` |

Na tela inicial, `↑`/`↓` escolhem a linha (modo ou fase), `←`/`→` mudam o valor
e `Enter` abre a seleção de lutadores. Nela, `A`/`D` troca a skin do P1,
`←`/`→` troca a do P2 e `Enter` confirma. No online cada cliente escolhe a
própria skin; no treino o dummy permanece `WRAITH`. `Esc` volta.

## Combate

`Mouse 1` encadeia três golpes distintos, não o mesmo soco repetido:

| Elo | Golpe      | O que ele faz                                          |
| --- | ---------- | ------------------------------------------------------ |
| 1   | `JAB`      | Rápido e curto. Só o braço da frente sai.               |
| 2   | `CROSS`    | O braço da frente recolhe pra guarda e o de trás passa. |
| 3   | `UPPERCUT` | Finalizador: sobe, dobra o dano e lança pra cima.       |

Cada elo tem dano, alcance e empurrão próprios, e a coreografia dos oito
membros é escrita como dado em `pose::UNARMED_COMBO` — três quadros (preparo,
contato, recuperação) por braço, e por perna quando o golpe usa as pernas.
Inventar um golpe é escrever mais uma entrada naquela lista; `animate_limbs`
não muda. `x` positivo é sempre "para a frente", então quem desenha nunca pensa
em espelhamento.

O ciclo de corrida é contínuo e amarrado à distância: uma senoide fecha a
passada sem salto entre o último e o primeiro quadro. Escalada usa o mesmo
princípio na altura. As mudanças de pose são misturadas nos `Transform`s dos
membros; salto abre perto do ápice, queda prepara o corpo e o pouso aplica um
squash curto, tudo visual e sem alterar collider ou timing de combate.

A arma e os membros passam pela mesma montagem, `rig::armed_joints`. Durante o
golpe ela lê a coreografia de `pose::strike_for`; fora dele, o estilo escolhe a
empunhadura: pistola, faca, bomba e nunchaku deixam a mão livre em guarda,
enquanto rifle, escopeta, cano e katana usam apoio de duas mãos. Corrida acrescenta
o balanço da passada e o coice recolhe e levanta braço e arma pelo mesmo vetor.
Assim escalada, queda, espelhamento e mira inclinada não deixam a arma flutuando
fora da mão.

Perder a janela de encadeamento derruba o combo de volta pro `JAB`. No modo
treino o painel nomeia o elo que acabou de acertar, que é o que ensina a
sequência — sem isso os três golpes são um contador subindo.

### Alto, baixo e aéreo

O mesmo botão dá três golpes diferentes conforme onde você está:

| Entrada | Golpe | Troca |
| --- | --- | --- |
| M1 no chão | Combo `JAB → CROSS → UPPERCUT` | O caminho normal |
| Baixo + M1 | `SWEEP` | Dano baixo (7 contra 8), mas põe no chão **0,62s** em vez de 0,26 |
| M1 no ar | `DIVE` | Troca o controle da queda pela chance de acertar |

O que faz as três serem leituras de verdade é a **altura em que a hitbox abre**.
A rasteira nasce na canela (`-18`) e por isso passa por baixo de quem está no
ar; o `UPPERCUT` nasce no alto (`+8`) e é a resposta pra esse caso; a voadora
nasce em `-12` e desce junto com o corpo. Sem essa diferença seriam três nomes
para o mesmo golpe.

A voadora é comprometida de propósito: ela **substitui** a velocidade
(`300, -560`) em vez de somar, e é uma por pulo — o `DiveSpent` só sai ao
aterrissar, então não dá pra picotar o golpe no ar pra planar. Errar significa
pousar onde o oponente escolher. A hitbox dela acompanha o corpo em vez de
ficar onde nasceu, senão erraria todo mundo que não estivesse exatamente no
ponto de contato.

Nenhum dos três golpes especiais encadeia — todos encerram a sequência. Dá pra
fazer `JAB → CROSS → SWEEP`, mas não `SWEEP → SWEEP`.

### Agachar

Segurar baixo sem atacar **passa por baixo do `UPPERCUT`**. É a metade
defensiva do mixup: antes disso a altura só importava pra quem atacava.

| Golpe | Contra quem agacha |
| --- | --- |
| `JAB` / `CROSS` | Acerta — nenhuma postura ganha de tudo |
| `UPPERCUT` | **Passa por cima** |
| `SWEEP` | Acerta — é o castigo de quem agacha demais |
| `SMASH` (cano) | Acerta — a pancada desce, e o punho não |

Agachar encolhe a **hurtbox**, não o `Collider`: mexer no colisor faria o boneco
afundar ou flutuar, porque a resolução de colisão o usa centrado na `Transform`.
Separar caixa de dano de caixa de terreno é o que permite ter postura sem tocar
na física. A caixa desce junto com o encolhimento, então os pés ficam onde
estavam e o que some é o topo — que é exatamente o que o gancho procura.

Quem toma a rasteira vai ao chão de verdade: pose própria, membros esparramados
e o corpo achatado, até o atordoamento acabar. O contraste com o recuo em pé é
o que diz na tela que a rasteira comprou tempo — sem ele a vantagem existiria só
na tabela de números.

O round começa mão a mão. Depois de cinco segundos as armas começam a cair —
encostar pega. Armas vazias continuam equipadas e podem ser arremessadas; quem
as pegar recebe a munição que restava.

### Arsenal

| Arma      | M1                          | M2                              |
| --------- | --------------------------- | ------------------------------- |
| `PISTOL`  | Combo curto                 | Tiro reto, preciso              |
| `SHOTGUN` | Combo pesado                | Leque de chumbos, forte de perto |
| `RIFLE`   | Combo longo                 | Cadência alta                   |
| `PIPE`    | Investida, gancho e uppercut de duas mãos | `SMASH`: pancada de cima pra baixo |
| `NUNCHAKU`| Combo circular mais rápido  | `CYCLONE`: giro amplo           |
| `KATANA`  | Saque e cortes longos       | `EXECUTE`: corte vertical       |
| `KNIFE`   | Estocadas curtas e rápidas  | `LUNGE`: avanço perfurante      |
| `KNIVES`  | Combo de faca               | Arremessa facas que ficam cravadas |
| `BOMB`    | Combo fraco (é pra jogar, não pra bater) | Arremesso em arco, estoura em área |

O `PIPE` é a primeira arma que não atira. Ela existe pra dar a quem pega uma
escolha diferente de "aponte e clique": alcance e dano de arma com o risco de
ter que chegar perto. Não gasta nada, então nunca vira peso morto — só sai da
mão arremessada.

O `SMASH` causa 31 de dano com 0,56s de preparação, e
corta qualquer combo em andamento: ele é o fim da sequência, não mais um elo
dela. Encadeá-lo faria dele a única coisa que vale apertar.

Uma arma de contato declara `heavy()` no trait `Weapon`; o padrão é `None`, que
significa "o M2 dispara". Nenhuma arma de fogo mudou uma linha, e `fire` não
sabe que o cano existe.

A `BOMB` é a primeira que não mira em linha reta. As outras resolvem no instante
do disparo — apontou, acertou ou errou. Esta troca precisão por área e obriga a
prever onde o outro vai estar daqui a **1,15s**. Ela **não machuca ao encostar**:
o dano inteiro (30) está no estouro, então errar o arremesso é errar o golpe.

O arco não precisou de física nova. Um projétil `Lobbed` nasce sem `Ghost` (bate
no cenário e para), com `Falls` (arqueia) e sem `Hitbox` (não fere ao tocar). O
estouro é só mais um `Hitbox` — grande, curto e parado — então a mesma resolução
de dano que trata soco e bala trata explosão, sem uma linha nova em `combat`.

`Shot::kind` não tem valor padrão de propósito: uma arma nova é obrigada pelo
compilador a escolher entre reto e arremesso, em vez de herdar "reto" sem
perceber.

## Modos

`ONLINE` cria ou encontra um lobby Steam para dois jogadores, usa P2P para as
entradas e deixa o dono da sala como autoridade do resultado da luta.

| Modo       | O que muda                                                        |
| ---------- | ----------------------------------------------------------------- |
| `VERSUS`   | Dois jogadores, **primeiro a 3 rounds** leva a partida.            |
| `CPU`      | P1 contra o jogo. Mesma partida, um teclado.                       |
| `TRAINING` | Só o P1, contra um dummy no lugar do P2. Ninguém morre.            |

No `VERSUS` perde o round quem zerar a barra **ou** cair num buraco. A tela
entre rounds mostra o placar em blocos (`███` contra `░░░`) em vez de números —
bloco se conta de relance, número exige leitura. Quem chega a 3 leva a partida,
e o `Enter` seguinte começa outra do zero. Voltar ao menu abandona a partida.

O modo `CPU` troca **só a fonte de entrada** do segundo boneco. Nenhum sistema
de movimento, combate ou arma sabe a diferença — era a promessa do trait
`InputSource` desde o começo, e ela não se sustentava: `poll` só recebia o
teclado, então uma IA não tinha como saber onde o oponente está. Agora recebe
um `Sense` (posição própria, posição do adversário, se está no chão, vida,
tempo), e o adversário é uma **função pura** daquilo — o que permite testar
cada decisão dele sem subir um `App`.

Ele não existe pra ser difícil: aproxima, bate no alcance, sobe atrás de quem
está numa plataforma acima e abre distância quando está ferido. O `time` no
`Sense` é o que dá cadência — sem ele uma fonte sem estado só consegue reagir,
e um adversário que ataca todo quadro é uma metralhadora.

O `Sense` também diz **se há chão de cada lado**, um passo à frente e um vão
adiante. Sem isso ele perseguia em linha reta e caía no buraco: na Arena 01
nasce no trecho da direita e anda para dentro do vão em `143..235`; na Arena 02
sai da torre para o vazio. Agora ele para na beirada e pula o vão quando dá pra
pular. Não é pathfinding — é o mínimo pra não perder sozinho.

E diz **o que ele tem na mão**. Com arma de fogo carregada ele mantém distância
de tiro (260) e mira de verdade, em vez de apontar só pra frente; com arma vazia
ou de contato ele volta a fechar a distância, porque aí só o soco resolve. Sem
isso ele nunca apertava o M2 — pegar uma escopeta o deixava **mais fraco**,
porque ele caminhava até o alcance de soco segurando ela.

Quem decide que a partida acabou é o placar, em `combat`, não a UI: a tela só
pergunta `match_winner()`. Empate gasta um round sem dar ponto a ninguém, e por
isso a contagem de rounds é um campo à parte da soma do placar — senão o
"ROUND N" travaria no mesmo número depois de um duplo KO.

O modo escolhe **quem entra na arena e o que encerra o round** — nunca as regras
de golpe. Combo, arma, parry e física são idênticos nos dois, senão o treino
ensinaria um jogo que não existe.

No treino o dummy não leva knockback nem ganha janela de invulnerabilidade: um
saco de pancada que sai voando, ou que fica imune por 0,34 s entre golpes,
tornaria impossível medir um combo até o fim. O painel da direita conta acertos
e dano do combo corrente e guarda o recorde da sessão; parou de bater por um
segundo, o combo fecha e o dummy volta com a vida cheia. Cair num buraco só
devolve o jogador ao ponto de partida.

`Tab` troca o que o dummy faz:

| Modo     | Pra que serve                                                 |
| -------- | ------------------------------------------------------------- |
| `STILL`  | Parado. Medir combo e dano.                                    |
| `HOP`    | Pula no lugar. Treinar antiaéreo: a rasteira passa por baixo.  |
| `GUARD`  | Apara a cada 1,1 s. Treinar a espera.                           |
| `CROUCH` | Fica agachado. Ver o `UPPERCUT` passar por cima.                |

`H` liga o **visualizador de caixas**: contorno vermelho nas áreas de dano,
verde nas áreas atingíveis. Ele existe porque quase todo erro de alcance deste
jogo foi achado por aritmética, não por olhar a tela — dummy flutuando fora de
alcance, arma descolada da mão, golpe abrindo na altura errada. Com as caixas à
vista esses erros ficam óbvios em vez de invisíveis. A moldura é vazada de
propósito: cheia, ela esconderia o boneco que está medindo.

A altura do pulo (56) não é decorativa: ela precisa ser alta o bastante pra
rasteira passar por baixo e baixa o bastante pro `UPPERCUT` ainda alcançar. Essa
janela tem menos de 10 unidades, então um teste a tranca — mexer em
`DUMMY_HOP`, na altura das hitboxes ou no tamanho do corpo reprova.

O dummy continua sem `Velocity`: o pulo é escrito direto na `Transform`, pra ele
seguir imune a knockback.

Ele também **mostra o que está fazendo** — guarda quando apara, recuo quando
apanha, e tem os mesmos oito membros articulados dos jogadores.

Isso custou três correções da mesma causa: a camada visual exigia `Player` pra
desenhar. Agora ela lê `ActorTint` (cor), `BreathPhase` (defasagem da
respiração) e trata `Velocity` como opcional — o dummy não tem uma, de
propósito. Nenhum desses sistemas precisa mais saber *o que* está desenhando.

O corpo dos dois é montado pela mesma função. Enquanto o spawn do dummy tinha
cópia própria, ele nascia sem membro nenhum, e um teste agora exige que todo
corpo nasça com os oito.

## Fases

| Fase                    | Ideia                                                     |
| ----------------------- | --------------------------------------------------------- |
| `ARENA 01 - THE GAP`    | Briga no chão, com dois buracos e plataformas altas.       |
| `ARENA 02 - THE STACKS` | Não existe chão: três torres e vãos largos entre elas.     |
| `ARENA 03 - THE VAULT`  | Chão inteiro, zero buracos, teto nos dois lados.           |

As duas primeiras decidem a briga pela queda; a `VAULT` decide pelo dano. O teto
divide o espaço: encostado na parede sobram 40 de folga contra os 93 do arco do
pulo, então o jogo aéreo morre e só resta combo e rasteira. No meio a sala abre
e o gancho e a voadora voltam a valer. **Onde você está decide que golpes você
tem** — e um teste garante que o teto realmente corta o pulo, senão ele seria
decoração.

Uma fase é **dado, não código**. Ela devolve um `&'static [Piece]` — terreno,
teto, plataforma, corrente, perigo — e um único `build_level` traduz peça em
entidade. Nenhum
mapa spawna nada por conta própria, então não dá pra existir fase com regra de
colisão ou de camada diferente das outras. Adicionar um mapa é escrever o
`Level` e pôr o construtor no `CATALOG`.

### Perigo com hora marcada

Uma poça só cobra atenção uma vez: o jogador aprende onde ela está e nunca mais
pisa ali. Por isso três peças têm relógio, e o lugar seguro passa a ser um lugar
**e uma hora**:

| Peça     | O que faz                                                        |
| -------- | ---------------------------------------------------------------- |
| `Geyser` | Jorra de tempos em tempos; avisa borbulhando antes de abrir.      |
| `Tide`   | A poça sobe e desce, engolindo os patamares mais baixos.          |
| `Drip`   | Uma boca no alto pinga, e a gota é um perigo que anda.            |

As três saem de `cycle(now, period, phase)`, e não de um `Timer` por entidade: a
coluna que aparece e a zona que machuca são entidades diferentes, e dois
relógios que começam juntos terminam desencontrados depois de um round — o jorro
aparece e não fere, ou fere invisível. A zona de contato da fonte ainda arma
numa janela **mais estreita** que a do desenho, porque o erro tem que cair para
o lado certo: fogo visível que ainda não machuca ensina; dano vindo do nada,
não. Um teste cobra exatamente isso.

O ganho real disso é poder **testar geometria sem subir um `App`**:

```
cargo test todo_patamar_e_alcancavel
```

O teste monta um grafo de travessia (patamares e correntes), resolve o alcance
do pulo com `JUMP_SPEED` e `GRAVITY` — os mesmos números que a física usa — e
reprova qualquer mapa com um patamar ilhado. Ele achou duas plataformas
inalcançáveis na Arena 01, que recebiam metade dos drops de arma; as correntes
em `x = -340` e `x = 250` existem por causa disso. Mexer no pulo agora reprova
os mapas que deixaram de fechar, em vez de deixar o problema pra quem joga.

## Por que não ratatui

A ideia inicial era usar ratatui pra montar o buffer e renderizar numa malha.
Não fizemos isso: o modelo do ratatui é um grid fixo de células, que é
exatamente a limitação que queríamos evitar. Um sprite ASCII livre no espaço não
tem coluna e linha — ele tem um `Vec2`.

Também não usamos a fonte padrão do Bevy. A `FiraMono-subset` embutida cobre os
95 caracteres ASCII imprimíveis e **nada** acima de 127: sem blocos, sem
molduras. Para arte ASCII isso é castrante.

O que usamos é a fonte 8×16 da ROM de BIOS VGA do IBM PC, 4 KB embutidos no
binário, convertida num atlas de textura no boot. Zero asset em runtime, CP437
completo, e a proporção 2:1 favorece stick figure. Detalhes e procedência em
[`docs/regen-cp437.md`](docs/regen-cp437.md).

## Arquitetura

Cada módulo é um `Plugin` do Bevy e define o próprio contrato. Os pontos de
extensão são traits e tabelas de dados, não `match` espalhado:

| Ponto          | Onde                | Trocar isso permite                                  |
| -------------- | ------------------- | ---------------------------------------------------- |
| `Paint`        | `ascii::art`        | Recolorir a mesma silhueta (jogador, dano, fantasma)  |
| `PoseDef`      | `actor::rig`        | Pose ou quadro de animação novo numa linha de tabela  |
| `Clip`         | `actor::rig`        | Ciclo de animação novo como lista de quadros          |
| `Skin`         | `actor::skin`       | Outro boneco — glifos e cores — sem tocar em pose     |
| `Level`        | `level`             | Mapa novo como dado, sem tocar em nenhum sistema      |
| `InputSource`  | `actor::input`      | IA ou rede no lugar do teclado, sem mexer no gameplay |
| `Sense`        | `actor::input`      | O que uma fonte percebe: mudar isso muda o que dá pra decidir |
| `Weapon`       | `weapon`            | Arma nova sem mexer em pegar/mirar/dano               |
| `Effect`       | `fx`                | Efeito não-ASCII (shader, malha) na camada de cima    |

```
src/
  ascii/          camada de arte: CP437, atlas, sprites de glifo
    cp437.rs      ROM da fonte -> atlas de textura
    art.rs        string -> células com cor (trait Paint)
    sprite.rs     AsciiSprite -> entidades Sprite filhas
    palette.rs    mono + acentos escassos
  physics.rs      AABB, gravidade, resolução eixo a eixo
  level.rs        trait Level, Arena01
  online.rs       lobby Steam, convites, P2P e sincronizacao dos lutadores
  actor/          jogadores: input, movimento, poses
    rig.rs        tabela visual das poses + clipes de animação
    skin.rs       peles: glifos, cores e catálogo
    pose.rs       o que uma pose é pro jogo + coreografia dos golpes
  combat.rs       hitboxes, dano, fim de round
  weapon.rs       trait Weapon, arsenal, drops, projéteis
  fx.rs           trait Effect, partículas acima do ASCII
  ui.rs           tela de controles, HUD, placar
```

### Animações e peles

Uma pose é **uma linha** de `POSES`, em `actor::rig` — a silhueta, como o corpo
se deforma, o que os membros fazem e de que papel de cor ela pinta. Nenhum
sistema tem `match` sobre `Pose`: `animate_body` e `animate_limbs` leem a tabela.

```rust
(Pose::Taunt, PoseDef {
    art: " O>\n/| \n | \n   ",
    body: Body::new(1.04, 0.98, 0.05, Sway::Breath),
    rig: |joints, r| joints.hand = Vec2::new(r.facing * 18.0, 30.0),
    ..standing(Body::STILL)
}),
```

Um ciclo é um `Clip` — a lista de quadros na ordem em que tocam. `Clip::at`
toca e `Clip::index_of` pergunta em que quadro se está, então as duas pontas
nunca divergem. `Pose::COUNT` é o discriminante da última pose mais um, então
uma pose nova **quebra a compilação** da tabela até receber a linha dela.

Uma pele é uma entrada em `skin::CATALOG`. Ela tem dois níveis: `swap` troca
glifos na silhueta inteira (cabeça e tronco novos por uma linha), e `art`
substitui as poses que ela realmente redesenha. As cores vêm em papéis
(`Tone::Body`/`Hurt`/`Gone`), não em situações — por isso uma pele de pedra não
precisa sangrar vermelho. O acento (quem é quem) fica de fora da pele de
propósito: dois jogadores podem vestir a mesma e continuar distinguíveis.
Trocar o componente `ActorSkin` repinta silhueta e membros no frame seguinte.
O catálogo inclui `STICK`, `HEAVY`, `WRAITH`, `NINJA`, `ROBOT` e `INFERNO`;
todas preservam a caixa 3×4 e o acento de cor que identifica o jogador.

### Ordenação

`AppSet` (em `state.rs`) fixa a ordem dentro de `Update`:

```
Input → Logic → Physics → Animate → Render
```

`Render` reconstrói os glifos das artes que mudaram. Ele roda por último de
propósito: se rodasse antes da lógica, toda animação sairia um frame atrasada.

### Direção de arte

Base monocromática, cor só como informação de jogo. Cenário em cinza, corpos em
branco, e o acento entra apenas onde carrega significado: cabeça na cor do
jogador, armas em dourado, corrente escalável em verde, dano em vermelho.
Acento escasso é o que faz ele ler.

## Testes

Por padrão, todos os testes do projeto devem ficar no diretório `tests/`, na
raiz do repositório. Não escreva testes dentro de `src/`, nem mesmo em módulos
marcados com `#[cfg(test)]`.

```bash
cargo test
```

Cobrem o que quebra em silêncio. O caso mais literal: **todo glifo que a arte
usa tem que existir na CP437**. Fora dela o `glyph_index` cai em `?` — a arte
sai errada mas o jogo roda e nenhum teste reclama. Foi assim que a `BOMB` nasceu
desenhada como uma interrogação, no chão, na mão e no arremesso, e sobreviveu
duas iterações. A varredura cobre poses, arsenal e fases, que é por onde
conteúdo novo entra, e o teste ainda confere que o próprio detector detecta.

Além disso: cobertura da tabela CP437, integridade dos
bitmaps da ROM, invariantes da caixa de arte (toda pose tem que ter a mesma
largura, senão o boneco escorrega de lado ao virar), espelhamento de glifos
direcionais, o fechamento da moldura da UI (e que ela não muda de largura ao
navegar o menu), a geometria das fases (todo patamar alcançável, todo spawn e
todo drop de arma com chão embaixo), a coreografia dos golpes (todo golpe
avança no quadro de contato, o finalizador sobe, a pancada desce, e os elos do
combo não podem desenhar a mesma coisa nem sair de sincronia com o combate), e
o arsenal — que os testes percorrem de verdade, via `ARSENAL`, pra que arma
nova entre nas invariantes sozinha em vez de escapar por esquecimento.
