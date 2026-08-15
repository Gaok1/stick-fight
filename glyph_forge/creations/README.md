# creations

Os sprites do jogo, na forma que o Glyph Forge edita. Abra o `.glyph.json`, mexa, salve -- e o jogo pega na proxima compilacao.

```
bonecos/
  boneco.glyph.json     o lutador: 21 poses, 13 animacoes, 6 peles
  boneco/               pacote exportado (previews e GIFs)

armas/
  nunchako.glyph.json              lido pelo jogo
  magic_book.glyph.json            lido pelo jogo -- o livro no chao
  magic_book_open_side.glyph.json  lido pelo jogo -- o livro na mao
  fency_sword.glyph.json           ainda nao entrou no jogo
  <nome>/                          pacote exportado de cada um

antigos/                sobras de exportacoes soltas, guardadas por precaucao
```

## Quem le o que

O jogo compila estes arquivos para dentro do binario com `include_str!`, em `src/weapon.rs`. Nao ha leitura em tempo de execucao: mudar um sprite custa um rebuild, igual a mudar uma string de Rust.

**Mover ou renomear um `.glyph.json` daqui quebra a compilacao** ate `weapon.rs` apontar para o lugar novo. Os caminhos vivem em `nunchaku_scene`, `book_spawn_scene` e `book_held_scene`.

O `boneco.glyph.json` e gerado por `bake_actor.py` a partir de `src/actor/`, e nao o contrario: rodar o script por cima de ajustes feitos a mao os desfaz. O caminho de volta e transcrever as coordenadas dos pontos para `rig.rs`.

## Antes de salvar

O Rust acha as pecas por **nome** de rotulo e de ponto de atencao -- nome trocado vira `panic!` na primeira vez que a arma aparece em campo. Use o botao **Conferir** do editor antes de fechar; ele avisa sobre nome repetido, peca solta e rotulo vazio.

Hoje o `nunchako.glyph.json` tem dois pontos de atencao com o mesmo nome (`ponto de pega do nunchako`) e o Rust pega o primeiro do array, entao a pega certa depende da ordem. Vale renomear um deles quando mexer nele.
