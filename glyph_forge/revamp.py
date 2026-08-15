"""Peles e animacoes novas, propostas por cima do boneco transcrito do jogo.

`bake_actor.py` e transcricao: o que sai dele tem que bater com `src/actor/`.
Este aqui e autoral -- peles que o jogo ainda nao tem e animacoes que ainda
nao existem como pose no Rust. Por isso escreve num arquivo proprio: abrir,
conferir, e o que sobreviver vira `skin.rs` e `rig.rs`.

    python revamp.py

Escreve `creations/bonecos/boneco_novo.glyph.json`.
"""

from __future__ import annotations

import json
import math
from pathlib import Path

import bake_actor as base
from bake_actor import BONE, ASH, IRON, COAL, BLOOD, STANDING, cell_id, point_id, to_canvas

# --- peles ------------------------------------------------------------------
#
# A direcao de arte do jogo e monocromatica: cor so entra como informacao de
# gameplay. Entao pele nova se separa por *glifo* e por *valor*, nunca por
# matiz -- senao o acento do jogador deixa de ser a coisa mais colorida da tela
# e para de dizer de quem e o boneco.
#
# Duas regras que sairam de olhar as candidatas rodando:
#   - o glifo do membro precisa de forma vertical solida. `·`, `!` e `§`
#     esticados viram tracinho picado e o membro some.
#   - o glifo do tronco precisa de continuidade vertical. `▄` e `♦` quebram a
#     silhueta e o boneco parece desmontado.

PELES_NOVAS = [
    {
        "id": "osso",
        "name": "OSSO",
        # `≡` sao as costelas e `:` esticado vira osso em dois blocos. E a unica
        # pele em que o tronco le como *estrutura* e nao como massa.
        "swap": [["O", "Ω"], ["o", "Ω"], ["|", "≡"], ["_", "═"]],
        "accent": "Oo<>Ω",
        "limb": ":",
        "body": BONE,
        "hurt": BLOOD,
        "gone": ASH,
        "limbs": BONE,
        "art": {
            # A caixa toracica enche e esvazia: costela cheia, costela vazia.
            "q_parado_a": {"cel_r1c1": "≡", "cel_r2c1": "═"},
            "q_parado_b": {"cel_r1c1": "═", "cel_r2c1": "≡"},
        },
        # Ponto fraco conhecido: parado ele e o mais caracteristico do catalogo,
        # mas na corrida os segmentos encurtam e o `:` esticado vira dois
        # pontinhos -- o boneco quase some justamente quando esta em movimento.
        # Trocar o membro por `│` resolve e custa a identidade. Decisao de quem
        # olhar rodando.
        "description": (
            "Esqueleto. Costelas no tronco, ossos partidos nos membros. "
            "ATENCAO: na corrida os membros encurtam e quase somem -- conferir rodando."
        ),
    },
    {
        "id": "vulto",
        "name": "VULTO",
        # Massa solida inteira. E a leitura mais forte de longe: nao tem
        # detalhe nenhum, so silhueta -- o oposto de OSSO.
        "swap": [["O", "◙"], ["o", "◙"], ["|", "█"], ["_", "█"]],
        "accent": "Oo<>◙",
        "limb": "█",
        "body": IRON,
        "hurt": BLOOD,
        "gone": COAL,
        "limbs": ASH,
        "art": {},
        "description": "Sombra macica. Sem detalhe: so silhueta, e por isso le de longe.",
    },
    {
        "id": "lamina",
        "name": "LAMINA",
        # Meios-blocos opostos no tronco e no membro: tudo fica deslocado para
        # um lado, e o boneco parece cortante mesmo parado.
        "swap": [["O", "▼"], ["o", "▼"], ["|", "▐"], ["_", "▄"]],
        "accent": "Oo<>▼",
        "limb": "▌",
        "body": BONE,
        "hurt": BLOOD,
        "gone": IRON,
        "limbs": ASH,
        "art": {
            # O tronco troca de lado entre os dois quadros: le como uma lamina
            # girando de leve, sem sair do lugar.
            "q_parado_a": {"cel_r1c1": "▐", "cel_r2c1": "▌"},
            "q_parado_b": {"cel_r1c1": "▌", "cel_r2c1": "▐"},
        },
        "description": "Meios-blocos opostos: tudo deslocado, angular.",
    },
    {
        "id": "coracao",
        "name": "CORACAO",
        # A cabeca e a coisa mais identificavel do catalogo inteiro -- um
        # coracao na cor do jogador se acha numa tela cheia de gente.
        "swap": [["O", "♥"], ["o", "♥"], ["|", "▓"], ["_", "═"]],
        "accent": "Oo<>♥",
        "limb": "║",
        "body": BONE,
        "hurt": BLOOD,
        "gone": COAL,
        "limbs": IRON,
        "art": {
            # O tronco pulsa: cheio, vazio. E o batimento.
            "q_parado_a": {"cel_r1c1": "▓", "cel_r2c1": "▒"},
            "q_parado_b": {"cel_r1c1": "▒", "cel_r2c1": "▓"},
        },
        "description": "Cabeca de coracao na cor do jogador; o tronco bate.",
    },
    {
        "id": "sigilo",
        "name": "SIGILO",
        "swap": [["O", "Φ"], ["o", "Φ"], ["|", "≡"], ["_", "═"]],
        "accent": "Oo<>Φ",
        "limb": "│",
        "body": ASH,
        "hurt": BLOOD,
        "gone": IRON,
        "limbs": BONE,
        "art": {
            "q_parado_a": {"cel_r1c1": "≡", "cel_r2c1": "─"},
            "q_parado_b": {"cel_r1c1": "─", "cel_r2c1": "≡"},
        },
        "description": "Enfaixado. Bandagens no tronco, membros finos e claros.",
    },
]

# Respiracao para as peles do jogo que ainda nao tinham nenhuma. Parado e o
# estado em que o boneco passa mais tempo, e as tres abaixo ficavam totalmente
# congeladas nele -- os dois quadros de `parado` sao identicos sem isto.
RESPIRACAO = {
    "stick": {
        "q_parado_a": {"cel_r2c1": "|"},
        "q_parado_b": {"cel_r2c1": "¦" if False else ":"},
    },
    "heavy": {
        "q_parado_a": {"cel_r1c1": "║", "cel_r2c1": "║"},
        "q_parado_b": {"cel_r1c1": "║", "cel_r2c1": "│"},
    },
    "robot": {
        "q_parado_a": {"cel_r1c1": "╬", "cel_r2c1": "║"},
        "q_parado_b": {"cel_r1c1": "║", "cel_r2c1": "╬"},
    },
}


# --- poses autorais ---------------------------------------------------------

GAIT = {
    "shoulder": (2.5, 15.0),
    "elbow": (7.0, 6.0),
    "hand": (5.0, -2.0),
    "hip": (2.2, -7.0),
    "knee": (5.0, -18.0),
    "foot": (7.0, -31.0),
}


def lado(sinal: float, **mudou) -> dict[str, tuple[float, float]]:
    """Um lado do corpo: o que nao for dito fica na passada parada.

    Mesma economia da tabela do jogo -- um quadro escreve so o que muda, e nao
    os seis pontos de novo.
    """
    pontos = {}
    for nome, (x, y) in GAIT.items():
        base_x, base_y = sinal * x, y
        pontos[nome] = mudou.get(nome, (base_x, base_y))
    return pontos


def quadro(
    nome: str,
    hold: int,
    frente: dict,
    tras: dict,
    *,
    art: str = STANDING,
    scale: tuple = (1.0, 1.0),
    tilt: float = 0.0,
    tone: str = "body",
    marks: tuple = (),
    nota: str = "",
) -> dict:
    """Um quadro autoral: os doze pontos e a deformacao do tronco."""
    pose = base.PoseDef(art=art, scale=scale, tilt=tilt, tone=tone)
    estado = base.cells_of(pose)
    for sinal, pontos in ((("frente"), frente), (("tras"), tras)):
        for ponto, (x, y) in pontos.items():
            canvas_x, canvas_y = to_canvas(x, y)
            estado[point_id(sinal, ponto)] = {"x": canvas_x, "y": canvas_y}
    return {
        "id": f"q_{nome}",
        "name": nome,
        "hold": hold,
        "tone": tone,
        "keys": estado,
        "marks": list(marks),
        "note": nota,
    }


def clipe(nome: str, fps: float, loop: bool, descricao: str, quadros: list) -> dict:
    return {
        "id": f"anim_{nome}",
        "name": nome,
        "fps": fps,
        "loop": loop,
        "description": descricao,
        "frames": quadros,
    }


def animacoes_novas(rest: dict) -> list[dict]:
    """As animacoes que o jogo ainda nao tem.

    Todas partem de um buraco real: o boneco aterrissa sem quadro de impacto,
    morre sem cair, ganha sem comemorar e nao tem nada a dizer entre um round e
    outro. Sao propostas -- cada uma vira uma pose em `rig.rs` se sobreviver.
    """

    def limpo(estado: dict) -> dict:
        """So a diferenca em relacao ao repouso, como todo quadro guarda."""
        return base.difference(estado, rest)

    def enxuga(clip: dict) -> dict:
        for frame in clip["frames"]:
            frame["keys"] = limpo(frame["keys"])
        return clip

    provocacao = enxuga(
        clipe(
            "provocacao",
            4.0,
            True,
            "Chamar para a briga. Os bracos abrem, a mao da frente chama, o corpo infla.",
            [
                quadro(
                    "provocacao_abre",
                    1,
                    lado(1, elbow=(11.0, 13.0), hand=(17.0, 19.0)),
                    lado(-1, elbow=(-11.0, 13.0), hand=(-17.0, 19.0)),
                    scale=(0.97, 1.05),
                    tilt=-0.05,
                    nota="bracos abertos, peito estufado",
                ),
                quadro(
                    "provocacao_chama",
                    1,
                    lado(1, elbow=(9.0, 15.0), hand=(3.0, 21.0)),
                    lado(-1, elbow=(-12.0, 12.0), hand=(-18.0, 17.0)),
                    scale=(1.0, 1.02),
                    tilt=-0.02,
                    marks=("provoca",),
                    nota="a mao da frente chama",
                ),
                quadro(
                    "provocacao_volta",
                    1,
                    lado(1, elbow=(11.0, 13.0), hand=(17.0, 19.0)),
                    lado(-1, elbow=(-11.0, 13.0), hand=(-17.0, 19.0)),
                    scale=(0.97, 1.05),
                    tilt=-0.05,
                ),
                quadro(
                    "provocacao_baixa",
                    2,
                    lado(1, elbow=(8.0, 9.0), hand=(11.0, 4.0)),
                    lado(-1, elbow=(-8.0, 9.0), hand=(-11.0, 4.0)),
                    scale=(1.04, 0.96),
                    nota="afunda antes de repetir",
                ),
            ],
        )
    )

    aterrissagem = enxuga(
        clipe(
            "aterrissagem",
            14.0,
            False,
            "O impacto que faltava. O jogo ja achata o corpo ao pousar, mas os "
            "membros continuavam na pose de queda.",
            [
                quadro(
                    "pouso_impacto",
                    1,
                    lado(1, elbow=(10.0, 1.0), hand=(14.0, -7.0), knee=(9.0, -23.0), foot=(11.0, -31.0)),
                    lado(-1, elbow=(-10.0, 1.0), hand=(-14.0, -7.0), knee=(-9.0, -23.0), foot=(-11.0, -31.0)),
                    art="   \n O>\n_|_\n   ",
                    scale=(1.22, 0.74),
                    marks=("poeira",),
                    nota="joelhos absorvem, maos quase no chao",
                ),
                quadro(
                    "pouso_sobe",
                    1,
                    lado(1, elbow=(8.0, 5.0), hand=(10.0, 1.0), knee=(6.0, -21.0), foot=(8.0, -31.0)),
                    lado(-1, elbow=(-8.0, 5.0), hand=(-10.0, 1.0), knee=(-6.0, -21.0), foot=(-8.0, -31.0)),
                    scale=(1.08, 0.92),
                ),
                quadro("pouso_pe", 2, lado(1), lado(-1), scale=(0.99, 1.01)),
            ],
        )
    )

    vitoria = enxuga(
        clipe(
            "vitoria",
            3.0,
            True,
            "Ganhou. Bracos para cima, corpo esticado, um pulinho no lugar.",
            [
                # Os bracos abrem para fora antes de subir: colados no eixo eles
                # passam por cima da cabeca e o rosto -- que e onde a cara do
                # boneco e desenhada -- fica atras de dois membros.
                quadro(
                    "vitoria_sobe",
                    1,
                    lado(1, elbow=(11.0, 18.0), hand=(16.0, 28.0)),
                    lado(-1, elbow=(-11.0, 18.0), hand=(-16.0, 28.0)),
                    scale=(0.94, 1.09),
                    nota="bracos ao alto, abertos",
                ),
                quadro(
                    "vitoria_alto",
                    1,
                    lado(1, elbow=(14.0, 21.0), hand=(21.0, 34.0), knee=(5.0, -16.0), foot=(7.0, -27.0)),
                    lado(-1, elbow=(-14.0, 21.0), hand=(-21.0, 34.0), knee=(-5.0, -16.0), foot=(-7.0, -27.0)),
                    scale=(0.90, 1.14),
                    marks=("pulo",),
                    nota="sai do chao",
                ),
                quadro(
                    "vitoria_desce",
                    2,
                    lado(1, elbow=(12.0, 16.0), hand=(17.0, 25.0)),
                    lado(-1, elbow=(-12.0, 16.0), hand=(-17.0, 25.0)),
                    scale=(0.97, 1.05),
                ),
            ],
        )
    )

    nocaute = enxuga(
        clipe(
            "nocaute",
            9.0,
            False,
            "Morrer com queda, e nao de uma vez. Hoje o boneco pula direto para "
            "a pose deitada; aqui ele e arrancado do chao antes.",
            [
                quadro(
                    "ko_estala",
                    1,
                    lado(1, elbow=(-7.0, 19.0), hand=(-16.0, 25.0)),
                    lado(-1, elbow=(-11.0, 17.0), hand=(-21.0, 22.0)),
                    art=" o<\n | \n | \n   ",
                    scale=(1.12, 0.90),
                    tilt=-0.30,
                    tone="hurt",
                    marks=("impacto",),
                    nota="a cabeca vai primeiro",
                ),
                quadro(
                    "ko_voa",
                    1,
                    lado(1, elbow=(-12.0, 12.0), hand=(-24.0, 14.0), knee=(11.0, -12.0), foot=(19.0, -8.0)),
                    lado(-1, elbow=(-15.0, 8.0), hand=(-27.0, 6.0), knee=(6.0, -16.0), foot=(13.0, -14.0)),
                    art=" o<\n | \n | \n   ",
                    scale=(1.26, 0.72),
                    tilt=-0.55,
                    tone="hurt",
                    nota="fora do chao, membros para tras",
                ),
                quadro(
                    "ko_bate",
                    1,
                    lado(1, elbow=(-13.0, -14.0), hand=(-25.0, -22.0), knee=(13.0, -24.0), foot=(24.0, -29.0)),
                    lado(-1, elbow=(-16.0, -17.0), hand=(-28.0, -25.0), knee=(8.0, -26.0), foot=(18.0, -30.0)),
                    art="   \n   \n o_\n───",
                    scale=(1.34, 0.60),
                    tilt=-0.20,
                    tone="hurt",
                    marks=("baque",),
                    nota="chega no chao",
                ),
                quadro(
                    "ko_para",
                    3,
                    lado(1, elbow=(-12.0, -19.0), hand=(-23.0, -26.0), knee=(13.0, -26.0), foot=(22.0, -29.0)),
                    lado(-1, elbow=(-14.0, -20.0), hand=(-25.0, -27.0), knee=(-13.0, -26.0), foot=(-22.0, -29.0)),
                    art="   \n   \n   \n_o_",
                    scale=(1.10, 0.86),
                    tone="gone",
                    nota="mesma pose de Dead, para emendar",
                ),
            ],
        )
    )

    esquiva = enxuga(
        clipe(
            "esquiva",
            16.0,
            False,
            "Jogar o corpo para tras sem sair do lugar. Falta ao jogo uma leitura "
            "defensiva que nao seja a guarda parada.",
            [
                quadro(
                    "esquiva_inclina",
                    1,
                    lado(1, elbow=(-2.0, 12.0), hand=(-9.0, 14.0), knee=(9.0, -19.0), foot=(15.0, -31.0)),
                    lado(-1, elbow=(-9.0, 10.0), hand=(-16.0, 11.0), knee=(-7.0, -20.0), foot=(-11.0, -31.0)),
                    scale=(1.06, 0.95),
                    tilt=0.22,
                    nota="peso para tras",
                ),
                quadro(
                    "esquiva_fundo",
                    1,
                    lado(1, elbow=(-6.0, 9.0), hand=(-15.0, 9.0), knee=(13.0, -20.0), foot=(21.0, -31.0)),
                    lado(-1, elbow=(-13.0, 6.0), hand=(-22.0, 5.0), knee=(-9.0, -22.0), foot=(-13.0, -31.0)),
                    scale=(1.14, 0.88),
                    tilt=0.40,
                    marks=("invulneravel",),
                    nota="o quadro que devia sair ileso",
                ),
                quadro("esquiva_volta", 2, lado(1), lado(-1), scale=(1.02, 0.98), tilt=0.08),
            ],
        )
    )

    return [provocacao, aterrissagem, vitoria, nocaute, esquiva]


def build() -> dict:
    scene = base.build()
    rest = base.state_of("IdleA", gait=0.0)

    for skin in scene["skins"]:
        if skin["id"] in RESPIRACAO:
            skin["art"] = {**skin.get("art", {}), **RESPIRACAO[skin["id"]]}
    scene["skins"] += PELES_NOVAS
    scene["animation"]["clips"] += animacoes_novas(rest)
    scene["source"]["revamp"] = "glyph_forge/revamp.py -- peles e animacoes autorais"
    scene["notes"] = (
        "Boneco do jogo mais o que ainda nao existe nele.\n\n"
        "As 13 primeiras animacoes e as 6 primeiras peles sao transcricao de "
        "src/actor/ (veja bake_actor.py). Da 14a animacao em diante e da 7a pele "
        "em diante e proposta: nao existe pose nem pele correspondente no Rust.\n\n"
        "Conferir olhando, apagar o que nao prestar, e o que sobrar vira "
        "skin.rs e rig.rs. As coordenadas dos pontos, em espaco do ator, saem "
        f"de x_ator = x_canvas - {base.ORIGIN[0]:g} e y_ator = {base.ORIGIN[1]:g} - y_canvas."
    )
    return scene


def main() -> None:
    target = Path(__file__).resolve().parent / "creations" / "bonecos" / "boneco_novo.glyph.json"
    target.parent.mkdir(parents=True, exist_ok=True)
    scene = build()
    target.write_text(json.dumps(scene, ensure_ascii=False, indent=2), encoding="utf-8")
    novos = len(scene["animation"]["clips"]) - 13
    print(
        f"{target}\n"
        f"  {len(scene['skins'])} peles ({len(PELES_NOVAS)} novas), "
        f"{len(scene['animation']['clips'])} animacoes ({novos} novas)"
    )


if __name__ == "__main__":
    main()
