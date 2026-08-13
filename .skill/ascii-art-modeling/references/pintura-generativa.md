# Pintura generativa com glifos

Método de trabalho para composições grandes — cenário, criatura, arquitetura.
Cada regra vem com a falha concreta que ela evita; sem a falha, a regra vira
gosto pessoal e a próxima pessoa a ignora com razão.

## Sumário

1. A ordem do trabalho
2. Gerar em vez de desenhar
3. O preview é a ferramenta principal
4. A célula não é quadrada
5. Densidade já é luz
6. Meia célula é resolução de graça
7. Emenda entre peça gerada e peça desenhada
8. Determinismo, espelho e repetição
9. Profundidade é orçamento, não enfeite
10. Cenário precisa de acontecimento
11. Movimento tem antecipação, contato e recuperação
12. Quando a colisão não acompanha o desenho
13. O que só teste segura

## 1. A ordem do trabalho

```
silhueta → escolher gerador ou mão → preview → iterar → teste → rodar o jogo
```

Não escrever arte antes de conseguir ver arte. A parte cara desse ciclo é
montar o preview uma vez; depois disso cada iteração custa segundos, e é aí que
o desenho fica bom. Quem pula o preview desenha às cegas e entrega o primeiro
rascunho.

## 2. Gerar em vez de desenhar

Forma grande é **repetição com variação**: coluna de basalto, andar de pagode,
arco de galeria, escama, degrau, telha, onda. Escrever isso à mão é escrever
vinte linhas de quarenta caracteres e errar uma.

O que denuncia arte grande feita à mão nunca é o detalhe — é a **linha de
construção**. Uma parede sai com uma junta a menos no meio; uma serpente vira
três pedaços empilhados porque a espinha deixou de ser uma curva só; um pagode
entorta porque o quarto telhado saiu uma coluna menor que o terceiro.

Escrever à mão só o que é **gesto**: cabeça, mão, rosto, pose. Gesto não tem
regra que uma senoide cumpra.

Anatomia de um gerador:

```rust
fn peca(cols, rows, tabela_de_cor) -> AsciiArt {
    draw(cols, rows, tabela, fallback, |col, row| {
        // 1. uma conta que decide se a célula existe (silhueta)
        // 2. uma conta que decide quanto ela pesa (volume)
        // 3. escolher o glifo pela rampa de densidade
    })
}
```

Ter **um** `draw(cols, rows, …, |col,row| -> char)` compartilhado importa: sem
ele cada gerador repete oito linhas de `map`/`collect`/`join`, e é nelas — não
no desenho — que nasce o erro de uma coluna a mais.

## 3. O preview é a ferramenta principal

Construir um renderizador de texto que imprima a composição montada, e olhar
para ele a cada mudança. Requisitos, todos aprendidos errando:

- **Imprimir o glifo real, não `#`.** Uma máscara de `#` é uma mancha perfeita:
  um cone com a cratera tapada, um dragão sem olho e um pagode sem janela
  passam iguais.
- **Inverter o mapeamento do atlas corretamente.** Numa página CP437, os
  símbolos de 0x00..0x1F são gráficos (`•`, `▲`, `○`) que a tabela ingênua
  devolve como caractere de controle — invisível no terminal, presente na tela.
  O preview então mente exatamente sobre olho, crista e mostrador: os detalhes
  focais. Foi o que aconteceu, e por dois ciclos o dragão pareceu não ter olho.
- **Ordenar por profundidade**, do plano mais fundo para o mais raso. Sem isso
  o preview mostra a soma de tudo em vez de quem cobre quem, e uma peça inteira
  escondida atrás de outra passa despercebida.
- **Deixar disponível, não ligado.** Um teste `#[ignore]` que se roda de
  propósito:

  ```
  cargo test olhar_o_fundo -- --ignored --nocapture
  ```

## 4. A célula não é quadrada

A célula é 8×16. Um círculo com o mesmo número de colunas e linhas sai um **ovo
deitado**, e não há correção depois — ou se conta a proporção na hora de gerar,
ou se desenha errado.

```rust
let squash = CELL.y / CELL.x;         // 2.0
let ry = (radius / squash).round();   // raio vertical em linhas
let d = dx.hypot((row - ry) * squash) / radius;
```

Vale para tudo que é redondo: lua, sol, anel, arco, boca de forno.

## 5. Densidade já é luz

Escolher o glifo pelo volume que ele ocupa é escolher o tom junto:

```
vazio → · → ░ → ▒ → ▓ → █
```

Pintar por **tabela de glifo** (`Tint`), e não por máscara paralela. Máscara é
uma segunda string que precisa ficar alinhada caractere a caractere; numa
escultura de trinta linhas ela sai defasada em uma coluna no meio, o que
ninguém enxerga lendo o código e todo mundo enxerga na tela.

A mesma rampa lida com materiais diferentes só trocando a tabela — basalto,
jade, magma e ácido saem do mesmo gerador. É isso que faz um gerador servir
três temas sem virar três geradores.

**Inverter a rampa para material translúcido.** Em rocha opaca o miolo pega
luz. Em jade, âmbar ou gelo, a casca fina é que acende:

```rust
const JADE: [(char, Color); 4] = [
    ('█', JADE_FUNDO), ('▓', JADE_FUNDO),      // miolo escuro
    ('▒', JADE_ACESO), ('░', JADE_ACESO),      // borda acesa
];
```

Essa inversão — e só ela — separa uma escultura de jade de uma escultura de
granito pintada de verde.

## 6. Meia célula é resolução de graça

`▀` desenha na metade de cima da célula e `▄` na metade de baixo. Usar isso
**dentro da mesma linha** dá meia célula de resolução sem gastar linha:

- crista de serra com altura fracionária → `▄` onde a encosta chega no meio;
- beiral de telhado virado → `▀` nas duas pontas e `▄` no meio, tudo na linha
  da telha. Numa linha própria acima, a ponta descola do telhado e vira dois
  riscos soltos no céu, com um vão de célula e meia entre eles;
- aresta iluminada no topo de uma parede → `▄` na linha da crista.

## 7. Emenda entre peça gerada e peça desenhada

Quando um corpo gerado encontra uma cabeça desenhada, a emenda acontece numa
coluna e numa linha que **ninguém enxerga lendo o código** — são contas em
constantes de arquivos diferentes.

Fazer as duas coordenadas serem **derivadas**, não digitadas:

- coluna: `fim_da_cabeça == começo_do_corpo`, calculado a partir das larguras;
- linha: a altura em que o pescoço sai do desenho tem que ser a altura em que a
  espinha do gerador chega naquele ponto — resolver a equação, não estimar.

E travar as duas com teste. Mexer na amplitude da onda, na altura do corpo ou
no tamanho da cabeça descola o pescoço, e **nada quebra**: o jogo roda, os
outros testes passam, e na tela há uma cabeça flutuando ao lado de uma cobra.

Cuidado com espelhamento: espelhar a peça troca qual ponta é grossa. Se a
cabeça encaixa em `t = 1`, a espessura tem que crescer até `t = 1` — senão o
bicho fica com a cabeça espetada na ponta fina do rabo.

## 8. Determinismo, espelho e repetição

- **Gerador não sorteia.** Aleatoriedade em tempo de construção faz a arte
  mudar entre um spawn e outro e entre um teste e outro. Variação vem de conta
  sobre `(col, row, quadro)`, não de `random()`. Sorteio fica para partícula,
  que é efêmera por natureza.
- **Espelhar a cópia.** Duas peças geradas iguais nas duas pontas da tela leem
  como moldura de cartaz. Uma linha (`.mirrored()`) resolve.
- **Duas frequências incomensuráveis** onde o padrão atravessa a tela. Um rio
  de mil e seiscentas unidades com uma senoide só repete o mesmo desenho a cada
  quinze colunas, e listra regular é a primeira coisa que o olho pega.
- **Variar o que a natureza varia.** Três tanques do mesmo tamanho igualmente
  espaçados leem como ícone repetido três vezes, não como pátio.

## 9. Profundidade é orçamento, não enfeite

Duas peças no mesmo plano que se cruzam disputam quem cobre quem, e o que
aparece vira sorteio da ordem de spawn — muda entre partidas. Portanto:

- ou separar os planos, ou não deixar as peças se tocarem;
- peça grande nova empurra as antigas para trás: quando o pátio de tanques
  ocupou o plano médio inteiro, os prédios tiveram que recuar de plano;
- apoiar uma peça **em cima** da outra, não dentro: o pagode pousa no último
  degrau do terraço, e não a quinze unidades dentro dele.

## 10. Cenário precisa de acontecimento

Pintura parada é bonita no primeiro round e muda no décimo. Cada cena ganha
**uma** coisa que ela faz de tempos em tempos — erupção, martelo, sopro, purga
— e isso é o que separa cenário de papel de parede.

Reduzir a um componente só com um relógio e um enum de número, não a um sistema
por cena: enquanto foram três sistemas, só o que alguém estava olhando ganhava
conserto.

O mesmo vale para o que escorre: queda de magma, cascata de ácido e núcleo que
pulsa são um punhado de quadros e um relógio — um `Flipbook`, não três.

## 11. Movimento tem antecipação, contato e recuperação

Dirigir por uma batida normalizada (`0..1`), não por uma pilha de timers:

```
0.00 .. 0.30  antecipação — o peso vai contra a direção da ação
0.30 .. 0.42  ação acelerada
0.42          quadro de contato — tremor, faísca, som
0.42 .. 0.58  pausa no contato
0.58 .. 1.00  recuperação amortecida
```

Sem a antecipação o martelo parece cair sozinho; sem a pausa, ele quica. O
quadro de contato acontece uma vez por número — marcar com um booleano que
zera no início, senão a faísca sai todo frame enquanto a batida está no
intervalo.

## 12. Quando a colisão não acompanha o desenho

Redimensionar collider por quadro costuma ser caro demais para um efeito que
dura um quarto de segundo. Nesse caso a zona cobre a forma inteira e **abre numa
janela mais estreita que a do desenho**.

O erro tem que cair para o lado certo: fogo visível que ainda não machuca
ensina o jogador; dano vindo do nada, não. Armar a zona junto com o desenho
cobra dano no topo enquanto a coluna ainda mede um degrau — hitbox invisível.

E anunciar antes: uma fonte sem aviso é armadilha, e morrer para ela não ensina
nada. O aviso olha para a janela do **desenho**, não para a da colisão.

## 13. O que só teste segura

Composição não quebra nada quando erra — é isso que a torna perigosa. Estes
testes pegam o que nenhum compilador pega:

| Teste                                   | O que ele impede                             |
| --------------------------------------- | -------------------------------------------- |
| Nada nasce enterrado                    | peça inteira abaixo da linha do chão          |
| Todo plano encosta na tela              | trabalho jogado fora onde ninguém vê          |
| Plano largo cobre a tela no deslize máx. | buraco de vazio no canto quando a atenção anda |
| Nenhum carimbo pisa em outro            | duas células no mesmo lugar e mesmo Z         |
| Toda a arte cabe no atlas               | glifo virando `?` com o jogo rodando          |
| Cenário não usa cor de gameplay         | silhueta ilegível cruzando um landmark        |
| A emenda das peças fecha                | cabeça flutuando ao lado do corpo             |
| A cena inteira monta sem quebrar        | índice fora de vetor de quadros, largura zero  |

A varredura tem que passar pela **composição montada**, não pela lista de
strings: só assim ela cobre o que é gerado — serra, cone, prédio, queda — que é
justamente o que ninguém pensa em conferir. E tem que incluir o que se mexe: a
arte animada nasce direto em `Commands` e escapa de toda conferência a menos
que também seja **descrita antes de existir**.

No fim, rodar o jogo. Teste unitário não prova composição.
