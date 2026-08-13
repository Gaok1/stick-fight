# Regenerar `src/ascii/cp437_table.rs`

A tabela mapeia cada um dos 256 bytes da página de código 437 para o caractere
Unicode equivalente. Ela é **gerada**, não escrita à mão — digitar 256
mapeamentos é um convite a erro silencioso, do tipo que só aparece quando um
glifo de moldura sai trocado no meio da arena.

A fonte da verdade é o codec `cp437` da biblioteca padrão do Python.

```bash
python - <<'EOF'
import io
BS = chr(92)
out = io.StringIO()
out.write("//! Tabela CP437 -> Unicode, gerada a partir do codec `cp437` do Python.\n")
out.write("//! NAO EDITE A MAO -- veja `docs/regen-cp437.md` para regenerar.\n\n")
out.write("/// Caractere Unicode correspondente a cada um dos 256 bytes da CP437.\n")
out.write("pub const CP437_TO_UNICODE: [char; 256] = [\n")
for row in range(16):
    items = []
    for col in range(16):
        ch = bytes([row * 16 + col]).decode('cp437')
        items.append("'" + BS + "u{%04X}" % ord(ch) + "'")
    out.write("    " + ", ".join(items) + ",\n")
out.write("];\n")
open("src/ascii/cp437_table.rs", "w", encoding="utf-8").write(out.getvalue())
EOF

cargo test -p stick-fightt cp437
```

## Sobre a faixa 0x00–0x1F

O codec do Python mapeia esses bytes para caracteres de controle (`U+0001`,
`U+0002`, …), porque é isso que eles significam como *dados*. Mas o hardware da
IBM desenhava símbolos ali — `☺☻♥♦♣♠►◄▲▼` — e a ROM da fonte tem os bitmaps
correspondentes.

A tabela gerada preserva o comportamento do codec. Quem corrige isso é a
constante `DOS_GRAPHICS` em `src/ascii/cp437.rs`, que sobrepõe esses códigos na
busca reversa. Se você precisar de mais símbolos dessa faixa, adicione lá — não
na tabela gerada, que seria sobrescrita na próxima regeneração.

## Sobre a fonte

`assets/fonts/ibm_vga_8x16.bin` são 4096 bytes: 256 glifos × 16 linhas × 1 byte,
1 bit por pixel, MSB à esquerda. É o dump da fonte 8×16 da ROM de BIOS VGA do IBM
PC, obtido de [spacerace/romfont](https://github.com/spacerace/romfont)
(`font-bin/IBM_VGA_8x16.bin`, SHA-256
`a8bad6fd78475a6bc2a05438c19207a9bb8c0f4f4099f60384e158cdc3eba580`).

A forma de um glifo bitmap não é sujeita a copyright nos EUA — é a mesma base
sobre a qual DOSBox e emuladores de PC embutem essas fontes. Não há arquivo de
licença para carregar junto.
