"""Poe a fency_sword na mao do boneco e escreve o combo dela.

O desenho da espada e do Luis, em `creations/armas/fency_sword.glyph.json`;
este script nao mexe naquele arquivo. Ele le, encolhe para a escala do boneco,
prende na mao da frente e escreve a coreografia.

    python espada.py

Escreve `creations/bonecos/boneco_espada.glyph.json`.

O que sai daqui e diretamente portavel: as posicoes de mao de cada quadro sao
os `Arm` de um `Strike` em `actor::pose`, e o giro da espada e o que hoje o
jogo tira do angulo do braco.
"""

from __future__ import annotations

import json
import math
from pathlib import Path

import bake_actor as base
import revamp
from bake_actor import point_id, to_canvas

FORGE = Path(__file__).resolve().parent
ESPADA = FORGE / "creations" / "armas" / "fency_sword.glyph.json"

# A espada foi desenhada num canvas de 800x600 e o boneco mora num de 128. O
# punho fica entre o pomo e a guarda; e dali que todas as distancias saem.
PUNHO = (214.0, 236.0)
ENCOLHE = 0.20


def pecas_da_espada(mao: dict) -> list[dict]:
    """A espada como pecas do boneco, ja presas a mao.

    A distancia de cada peca ate o punho vira a distancia dela ate a mao: e o
    que faz a espada inteira andar junto quando so a mao se move.
    """
    desenho = json.loads(ESPADA.read_text(encoding="utf-8"))
    pecas = []
    for index, parte in enumerate(desenho["elements"]):
        dx = (parte["x"] - PUNHO[0]) * ENCOLHE
        dy = (parte["y"] - PUNHO[1]) * ENCOLHE
        pecas.append(
            {
                **parte,
                "id": f"espada_{index}",
                "x": round(mao["x"] + dx, 3),
                "y": round(mao["y"] + dy, 3),
                "font_size": 16,
                # O desenho original usa glifos de 64px; a escala do editor
                # multiplica por cima disso, entao encolher a fonte para 16
                # exige devolver o fator na escala.
                "scale_x": round(parte["scale_x"] * ENCOLHE * 4, 4),
                "scale_y": round(parte["scale_y"] * ENCOLHE * 4, 4),
                "layer": 3,
                "role": "arma",
                "span": [],
                "follow": mao["id"],
                "offset": [round(dx, 3), round(dy, 3)],
            }
        )
    return pecas


def golpe(nome: str, descricao: str, fases: list[tuple], rest: dict, pecas: list[dict]) -> dict:
    """Um elo do combo: tres quadros, cada um com bracos e giro da lamina.

    `fases` e `(nome, hold, frente, tras, giro, escala, tilt, marcas)`. O giro e
    da espada: 0 e a lamina apontando para a frente, positivo desce.
    """
    quadros = []
    for passo, hold, frente, tras, giro, escala, tilt, marcas in fases:
        quadro = revamp.quadro(
            f"{nome}_{passo}",
            hold,
            frente,
            tras,
            scale=escala,
            tilt=tilt,
            marks=marcas,
            nota=f"{nome}: {passo}",
        )
        # Girar a espada e girar *o corpo dela*, e nao cada peca no proprio
        # centro. Sem virar a distancia ate a mao junto, as sete pecas ficam na
        # mesma fileira horizontal e so se inclinam: a lamina vira uma fila de
        # tracinhos soltos em vez de uma lamina.
        angulo = math.radians(giro)
        cos, sen = math.cos(angulo), math.sin(angulo)
        for peca in pecas:
            dx, dy = peca["offset"]
            quadro["keys"][peca["id"]] = {
                "rotation": giro,
                "offset": [round(dx * cos - dy * sen, 3), round(dx * sen + dy * cos, 3)],
            }
        quadro["keys"] = base.difference(quadro["keys"], rest)
        quadros.append(quadro)
    # Duracoes de `Katana::melee`, que e a arma de contato mais lenta e mais
    # forte do arsenal -- a espada e do mesmo peso.
    return {
        "id": f"anim_{nome}",
        "name": nome,
        "fps": 100.0,
        "loop": False,
        "description": descricao,
        "frames": quadros,
    }


def combo(rest: dict, pecas: list[dict]) -> list[dict]:
    """O combo do estoque: tudo e ponta, nada e fio.

    Numa arma de estocada a forca nao sai do giro da lamina -- ela sai da mao
    indo para a frente e da perna que joga o corpo atras dela. Por isso o giro
    fica sempre perto de zero (a lamina aponta para onde vai) e o que muda de
    quadro para quadro e o alcance da mao e o afundo.

    O braco de tras sobe e vai para tras em vez de acompanhar: e o contrapeso
    do esgrimista, e e ele que faz a pose ler como estocada e nao como soco com
    uma barra na mao.
    """
    lado = revamp.lado

    # A perna: guarda com o pe da frente adiantado, afundo com ela esticada.
    guarda_frente = {"knee": (9.0, -19.0), "foot": (14.0, -31.0)}
    guarda_tras = {"knee": (-8.0, -20.0), "foot": (-14.0, -31.0)}
    afundo_frente = {"knee": (20.0, -24.0), "foot": (34.0, -31.0)}
    afundo_tras = {"knee": (-13.0, -25.0), "foot": (-21.0, -31.0)}
    fundo_frente = {"knee": (25.0, -26.0), "foot": (43.0, -31.0)}
    fundo_tras = {"knee": (-16.0, -27.0), "foot": (-26.0, -31.0)}

    return [
        golpe(
            "espada_guarda",
            "Em guarda. Corpo de lado, lamina na linha, braco de tras erguido de "
            "contrapeso. E a pose de espera com o estoque na mao.",
            [
                ("respira_a", 40, lado(1, elbow=(9.0, 11.0), hand=(19.0, 13.0), **guarda_frente),
                 lado(-1, elbow=(-9.0, 17.0), hand=(-15.0, 25.0), **guarda_tras),
                 -7.0, (0.98, 1.02), -0.03, ()),
                ("respira_b", 40, lado(1, elbow=(9.0, 10.0), hand=(19.0, 12.0), **guarda_frente),
                 lado(-1, elbow=(-9.0, 16.0), hand=(-15.0, 24.0), **guarda_tras),
                 -5.0, (0.99, 1.01), -0.03, ()),
            ],
            rest,
            pecas,
        ),
        golpe(
            "espada_estocada",
            "Elo 1: a estocada basica. A mao recolhe ate o quadril e explode para a "
            "frente; a perna da frente afunda atras dela.",
            [
                ("preparo", 7, lado(1, elbow=(1.0, 10.0), hand=(-6.0, 9.0), knee=(7.0, -20.0), foot=(11.0, -31.0)),
                 lado(-1, elbow=(-8.0, 16.0), hand=(-13.0, 24.0), **guarda_tras),
                 -6.0, (0.95, 1.04), -0.06, ()),
                ("contato", 10, lado(1, elbow=(21.0, 11.0), hand=(44.0, 11.0), **afundo_frente),
                 lado(-1, elbow=(-11.0, 19.0), hand=(-19.0, 27.0), **afundo_tras),
                 0.0, (1.24, 0.86), 0.14, ("contato",)),
                ("recuperacao", 9, lado(1, elbow=(12.0, 11.0), hand=(24.0, 12.0), knee=(12.0, -21.0), foot=(19.0, -31.0)),
                 lado(-1, elbow=(-9.0, 17.0), hand=(-15.0, 25.0), **guarda_tras),
                 -4.0, (1.06, 0.96), 0.05, ()),
            ],
            rest,
            pecas,
        ),
        golpe(
            "espada_bote",
            "Elo 2: o bote alto. A ponta sobe para o rosto -- mesma mecanica, outra "
            "linha, e e a linha que o adversario tem que ler.",
            [
                ("preparo", 8, lado(1, elbow=(2.0, 17.0), hand=(-4.0, 21.0), knee=(7.0, -20.0), foot=(11.0, -31.0)),
                 lado(-1, elbow=(-8.0, 15.0), hand=(-13.0, 22.0), **guarda_tras),
                 13.0, (0.96, 1.05), -0.09, ()),
                ("contato", 11, lado(1, elbow=(20.0, 17.0), hand=(42.0, 23.0), **afundo_frente),
                 lado(-1, elbow=(-12.0, 20.0), hand=(-20.0, 28.0), **afundo_tras),
                 -11.0, (1.20, 0.89), 0.11, ("contato",)),
                ("recuperacao", 11, lado(1, elbow=(12.0, 14.0), hand=(24.0, 17.0), knee=(12.0, -21.0), foot=(19.0, -31.0)),
                 lado(-1, elbow=(-9.0, 17.0), hand=(-15.0, 25.0), **guarda_tras),
                 -6.0, (1.05, 0.97), 0.04, ()),
            ],
            rest,
            pecas,
        ),
        golpe(
            "espada_finta",
            "Elo 3: finta e afundo. A mao arrisca meia extensao, recolhe e vai fundo -- "
            "e o finalizador, e o alcance dele e o maior do arsenal.",
            [
                ("preparo", 11, lado(1, elbow=(13.0, 12.0), hand=(26.0, 13.0), knee=(10.0, -20.0), foot=(16.0, -31.0)),
                 lado(-1, elbow=(-8.0, 16.0), hand=(-13.0, 23.0), **guarda_tras),
                 -3.0, (1.04, 0.97), 0.04, ("finta",)),
                ("contato", 16, lado(1, elbow=(25.0, 9.0), hand=(52.0, 8.0), **fundo_frente),
                 lado(-1, elbow=(-14.0, 21.0), hand=(-24.0, 29.0), **fundo_tras),
                 3.0, (1.34, 0.79), 0.20, ("contato",)),
                ("recuperacao", 19, lado(1, elbow=(14.0, 11.0), hand=(28.0, 12.0), **afundo_frente),
                 lado(-1, elbow=(-10.0, 18.0), hand=(-17.0, 26.0), **afundo_tras),
                 -2.0, (1.14, 0.92), 0.09, ()),
            ],
            rest,
            pecas,
        ),
    ]


def build() -> dict:
    scene = revamp.build()
    rest = base.state_of("IdleA", gait=0.0)
    mao = next(j for j in scene["rig"]["joints"] if j["name"] == "mao_frente")

    pecas = pecas_da_espada(mao)
    scene["elements"] += pecas
    scene["labels"].append(
        {
            "id": "lb_espada",
            "name": "espada",
            "element_ids": [peca["id"] for peca in pecas],
            "description": "A fency_sword presa a mao da frente. Giro autorado por quadro.",
            "label_ids": [],
        }
    )
    # A ponta da lamina: e dela que o alcance do golpe sai, e e o que o jogo
    # precisa para saber onde o corte acerta.
    ponta = max(pecas, key=lambda peca: peca["x"])
    scene["attention_points"].append(
        {
            "id": "at_ponta_espada",
            "name": "ponta_espada",
            "x": ponta["x"],
            "y": ponta["y"],
            "parent_id": "",
            "attached_element_id": ponta["id"],
            "part_a_element_id": "",
            "part_b_element_id": "",
            "constraint_type": "pivot",
            "fixed": False,
            "color": "#ff4dc4",
            "kind": "attention",
            "description": "Ponta da lamina: e daqui que sai o alcance do golpe.",
        }
    )

    # O repouso precisa conhecer as pecas novas, senao o primeiro quadro grava
    # a espada inteira como diferenca em vez de so o giro dela.
    for peca in pecas:
        rest[peca["id"]] = {
            "x": peca["x"],
            "y": peca["y"],
            "rotation": peca["rotation"],
            "offset": peca["offset"],
        }
    scene["animation"]["clips"] += combo(rest, pecas)
    scene["notes"] = (
        "Boneco com a fency_sword na mao. O desenho da espada vem de "
        "creations/armas/fency_sword.glyph.json e nao foi alterado aqui.\n\n"
        "A espada e presa a `mao_frente` (`follow`), entao a posicao dela nunca "
        "e gravada: o quadro guarda a mao e o giro da lamina. Sao esses dois "
        "numeros que viram um `Strike` em actor::pose.\n\n"
        "Os quatro clipes de espada sao propostas: nao existe arma correspondente "
        "no Rust ainda."
    )
    return scene


def main() -> None:
    target = FORGE / "creations" / "bonecos" / "boneco_espada.glyph.json"
    scene = build()
    target.write_text(json.dumps(scene, ensure_ascii=False, indent=2), encoding="utf-8")
    espada = sum(1 for e in scene["elements"] if e.get("role") == "arma")
    print(f"{target}\n  {espada} pecas de espada, {len(scene['animation']['clips'])} animacoes")


if __name__ == "__main__":
    main()
