# Modelo Glyph Forge

Descreva aqui o que esta composicao representa e como deve ser usada.

Analise `scene.json` como fonte de verdade para posicoes, cores e transformacoes. A fonte padrao e a ROM IBM VGA 8x16 CP437 descrita em `default_font`. Nos elementos, `flip_x` e `flip_y` registram o espelhamento local. Em `rig.joints`, `part_a_element_id` e `part_b_element_id` sao as duas pecas ligadas por um ponto independente; `constraint_type` define pivo ou solda e `fixed` ancora ao mundo. `attention_points` contem destaques com nome e descricao opcional que nao sao ossos. `labels` nomeia e descreve conjuntos: `element_ids` sao membros diretos e `label_ids` contem outros rotulos, herdando seus glyphs recursivamente. Use `rig_preview.png` para visualizar o esqueleto, `preview.png` para a arte limpa e `flattened.txt` apenas como aproximacao ASCII.
