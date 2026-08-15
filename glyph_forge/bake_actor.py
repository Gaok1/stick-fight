"""Gera o projeto Glyph Forge do boneco a partir do rig do jogo.

O jogo descreve um boneco em tres tabelas de Rust -- `actor::rig` diz a silhueta
e onde ficam as articulacoes de cada pose, `actor::pose` diz a coreografia dos
golpes e `actor::skin` diz os glifos e as cores de cada pele. Este script le
essas tabelas transcritas aqui e escreve `creations/bonecos/boneco.glyph.json`, que abre
no editor com as peles e as animacoes prontas para ajustar a mao.

O que sai daqui e comparavel com o jogo por construcao: os quadros guardam as
coordenadas das articulacoes, que sao os mesmos numeros de `Joints` em Rust. Um
ajuste feito no editor volta para `rig.rs` por transcricao, e nao por conta de
angulo e escala.

    python bake_actor.py

Os numeros abaixo sao copia dos do jogo. Quando `rig.rs` ou `pose.rs` mudarem,
mude aqui tambem -- e por isso que o rodape do JSON registra de onde cada
tabela veio.
"""

from __future__ import annotations

import json
import math
import sys
from dataclasses import dataclass
from pathlib import Path

# --- medidas do jogo --------------------------------------------------------

# Um glyph da ROM CP437 do jogo.
CELL_W, CELL_H = 8, 16
# A silhueta do tronco, em celulas.
BODY_COLS, BODY_ROWS = 3, 4
# O sprite do corpo e ancorado nos pes, meia altura abaixo da origem do ator.
BODY_BASE = -BODY_ROWS * CELL_H / 2

# Velocidades, de `actor::motion`.
RUN_SPEED, CLIMB_SPEED, JUMP_SPEED = 265.0, 175.0, 510.0
# A cada 9px de corrida o quadro muda; a cada 16px de subida, a bracada.
RUN_STRIDE, CLIMB_STRIDE = 9.0, 16.0
# Alcance de um braco totalmente estendido num golpe, de `rig::FULL_EXTENT`.
FULL_EXTENT = 26.0

# Onde os pes do boneco ficam no canvas.
#
# O canvas e apertado de proposito: nas 33 poses o boneco ocupa 64x68px, e
# sobra e area morta que so faz o desenho abrir pequeno. A origem cai em
# multiplos de 8 para que as celulas do tronco fiquem na grade.
CANVAS = (128, 128)
ORIGIN = (56.0, 64.0)

# Cores de `ascii::palette`, ja em hexadecimal.
BONE, ASH, IRON, COAL, BLOOD = "#ebebe0", "#75787f", "#4a4d57", "#26292f", "#e02138"
P1 = "#33d9f2"


def to_canvas(x: float, y: float) -> tuple[float, float]:
    """Do espaco do ator (y para cima) para o do canvas (y para baixo)."""
    return (round(ORIGIN[0] + x, 3), round(ORIGIN[1] - y, 3))


# --- coreografia dos golpes, de `actor::pose` -------------------------------


@dataclass(frozen=True)
class Strike:
    """Um golpe corpo-a-corpo: preparo, contato e recuperacao.

    Cada braco e `(cotovelo_x, cotovelo_y, mao_x, mao_y)` e cada perna e
    `(joelho_x, joelho_y, pe_x, pe_y)`, sempre com x positivo para a frente.
    """

    name: str
    front: tuple
    back: tuple
    legs: tuple | None = None
    rise: float = 1.0
    # Duracao real do golpe, de `combat::unarmed_move`.
    duration: float = 0.18


JAB = Strike(
    name="JAB",
    front=((2, 13, -4, 10), (13, 14, 26, 14), (8, 12, 11, 8)),
    back=((-4, 11, -2, 18), (-5, 11, -3, 18), (-4, 11, -2, 17)),
    duration=0.18,
)
CROSS = Strike(
    name="CROSS",
    front=((10, 12, 16, 10), (-5, 12, -9, 16), (4, 11, 6, 7)),
    back=((-8, 12, -14, 9), (16, 13, 33, 12), (9, 10, 12, 5)),
    duration=0.20,
)
UPPERCUT = Strike(
    name="UPPERCUT",
    front=((6, 2, 9, -6), (12, 14, 18, 34), (10, 16, 14, 24)),
    back=((-6, 10, -4, 16), (-9, 6, -14, -2), (-7, 9, -6, 14)),
    rise=1.18,
    duration=0.28,
)
SWEEP = Strike(
    name="SWEEP",
    front=((4, 4, 8, -8), (-6, -2, -12, -14), (2, 6, 4, -2)),
    back=((-6, 4, -10, -6), (-10, -4, -16, -16), (-5, 6, -8, 0)),
    legs=((8, -22, 4, -33), (20, -30, 40, -34), (12, -24, 16, -32)),
    rise=0.74,
    duration=0.30,
)
DIVE_KICK = Strike(
    name="DIVE",
    front=((-4, 14, -12, 18), (-9, 16, -20, 22), (-5, 13, -11, 15)),
    back=((-6, 12, -14, 14), (-11, 14, -22, 17), (-6, 11, -12, 12)),
    legs=((10, -16, 16, -26), (18, -20, 34, -30), (12, -18, 20, -28)),
    rise=1.06,
    duration=0.50,
)

PARRY_ARM = (9, 18, 13, 27)
DOWNED_ARM = (-12, -20, -24, -27)

# Onde cada fase do golpe comeca, em fracao da duracao, de `motion::drive_pose`.
PHASE_SPLIT = (0.26, 0.38, 0.36)


# --- ajuste dos membros, de `actor::rig` ------------------------------------


@dataclass
class Rigging:
    """O que a pose dos membros le do ator neste quadro."""

    side: float = 1.0
    facing: float = 1.0
    gait: float = 0.0
    aim: tuple = (1.0, 0.0)
    strike: tuple | None = None
    reach: float = 0.0
    cycle: float = 0.0
    air: float = 0.0

    def front(self) -> bool:
        return self.side > 0.0


Joints = dict[str, tuple[float, float]]


def gait_joints(r: Rigging) -> Joints:
    """A passada padrao: e dela que toda pose parte."""
    swing = r.gait * r.side * r.facing
    return {
        "shoulder": (r.side * 2.5, 15.0),
        "elbow": (r.side * 7.0 - swing * 5.0, 6.0),
        "hand": (r.side * 5.0 - swing * 10.0, -2.0),
        "hip": (r.side * 2.2, -7.0),
        "knee": (r.side * 5.0 + swing * 7.0, -18.0),
        "foot": (r.side * 7.0 + swing * 13.0, -31.0),
    }


def reach_arm(j: Joints, arm: tuple, facing: float) -> None:
    j["elbow"] = (facing * arm[0], arm[1])
    j["hand"] = (facing * arm[2], arm[3])


def gait(j: Joints, r: Rigging) -> None:
    """Nenhum ajuste: os membros ficam onde a passada os colocou."""


def running(j: Joints, r: Rigging) -> None:
    leg = r.gait * r.side
    arm = -leg
    stride = leg * r.facing
    pump = -stride
    j["knee"] = (r.side * 4.0 + stride * 9.0, -17.0 + max(leg, 0.0) * 8.0)
    j["foot"] = (
        r.side * 6.0 + stride * 15.0,
        -31.0 + max(leg, 0.0) * 13.0 + max(-leg, 0.0) * 6.0,
    )
    j["elbow"] = (r.side * 5.0 + pump * 6.0, 6.0 + max(arm, 0.0) * 3.0)
    j["hand"] = (
        r.side * 3.0 + pump * 13.0,
        1.0 + max(arm, 0.0) * 9.0 - max(-arm, 0.0) * 4.0,
    )


def crouch(j: Joints, r: Rigging) -> None:
    j["shoulder"] = (r.side * 2.5, -4.0)
    j["elbow"] = (r.side * 6.5, -12.0)
    j["hand"] = (r.side * 5.0 + r.facing * 3.0, -19.0)
    j["hip"] = (r.side * 2.2, -16.0)
    j["knee"] = (r.side * 10.0, -20.0)
    j["foot"] = (r.side * 13.0, -31.0)


def tuck(j: Joints, r: Rigging) -> None:
    apex = 1.0 - min(max(r.air, 0.0), 1.0)
    j["knee"] = (r.side * (9.0 + apex * 2.0), -13.0 - apex * 3.0)
    j["foot"] = (r.side * (3.0 + apex * 4.0), -21.0 - apex * 3.0)


def falling(j: Joints, r: Rigging) -> None:
    fall = min(max(-r.air, 0.0), 1.0)
    j["elbow"] = (j["elbow"][0], 17.0 + fall * 4.0)
    j["hand"] = (j["hand"][0], 24.0 + fall * 7.0)
    j["knee"] = (j["knee"][0] * (1.15 + fall * 0.35), j["knee"][1])


def climb(j: Joints, r: Rigging) -> None:
    phase = math.sin(r.cycle + (0.0 if r.front() else math.pi))
    j["elbow"] = (r.side * 6.0, 16.5 + phase * 7.5)
    j["hand"] = (r.side * 3.0, 24.5 + phase * 7.5)
    j["knee"] = (r.side * (7.0 - phase * 2.0), j["knee"][1])
    j["foot"] = (r.side * (4.0 + phase * 2.0), j["foot"][1])


def choreo(j: Joints, r: Rigging) -> None:
    """A coreografia do golpe em curso, girada para acompanhar a mira."""
    if r.strike is None:
        return
    strike, phase = r.strike
    frame = (strike.front if r.front() else strike.back)[phase]
    extra = r.reach if phase == 1 else 0.0
    pivot = (j["shoulder"][0] * r.facing, j["shoulder"][1])
    commitment = min(max(frame[2] / FULL_EXTENT, 0.0), 1.0)
    angle = math.atan2(r.aim[1], r.aim[0] * r.facing) * commitment
    cosine, sine = math.cos(angle), math.sin(angle)

    def turned(x: float, y: float, scale: float) -> tuple[float, float]:
        forward = extra * scale if x > 0.0 else 0.0
        local_x, local_y = x + forward - pivot[0], y - pivot[1]
        return (
            r.facing * (pivot[0] + local_x * cosine - local_y * sine),
            pivot[1] + local_x * sine + local_y * cosine,
        )

    j["elbow"] = turned(frame[0], frame[1], 0.5)
    j["hand"] = turned(frame[2], frame[3], 1.0)
    if strike.legs and r.front():
        knee_x, knee_y, foot_x, foot_y = strike.legs[phase]
        j["knee"] = (r.facing * knee_x, knee_y)
        j["foot"] = (r.facing * foot_x, foot_y)


def aiming(j: Joints, r: Rigging) -> None:
    anchor_y = 11.0 if r.front() else 9.0
    extent = 21.0 if r.front() else 14.0
    j["elbow"] = (r.aim[0] * extent * 0.5, anchor_y + r.aim[1] * extent * 0.5)
    j["hand"] = (r.aim[0] * extent, anchor_y + r.aim[1] * extent)


def parry(j: Joints, r: Rigging) -> None:
    reach_arm(j, PARRY_ARM, r.facing)


def recoil(j: Joints, r: Rigging) -> None:
    reach_arm(j, (-8, 20, -15, 27), r.facing)


def sprawl(j: Joints, r: Rigging) -> None:
    reach_arm(j, DOWNED_ARM, r.facing)
    j["knee"] = (r.facing * 12.0 + r.side * 4.0, -26.0)
    j["foot"] = (r.facing * 24.0 + r.side * 8.0, -31.0)


def limp(j: Joints, r: Rigging) -> None:
    j["knee"] = (r.side * 13.0, -26.0)
    j["foot"] = (r.side * 22.0, -29.0)


# --- a tabela das poses, de `rig::POSES` ------------------------------------

STANDING = " O>\n | \n | \n   "


@dataclass(frozen=True)
class PoseDef:
    art: str = STANDING
    scale: tuple = (1.0, 1.0)
    tilt: float = 0.0
    sway: str = "none"
    rig: object = gait
    tone: str = "body"


RUNNING_BODY = {"scale": (1.04, 0.97), "tilt": 0.20, "sway": "lean"}

POSES: dict[str, PoseDef] = {
    "IdleA": PoseDef(sway="breath"),
    "IdleB": PoseDef(sway="breath"),
    **{f"Run{letter}": PoseDef(rig=running, **RUNNING_BODY) for letter in "ABCDEF"},
    "Crouch": PoseDef(art="   \n O>\n_|_\n   ", scale=(1.10, 0.82), rig=crouch),
    "Jump": PoseDef(scale=(0.93, 1.10), tilt=-0.035, rig=tuck),
    "Fall": PoseDef(scale=(1.08, 0.94), tilt=0.025, rig=falling),
    "ClimbA": PoseDef(rig=climb),
    "ClimbB": PoseDef(rig=climb),
    "PunchWindup": PoseDef(scale=(0.96, 1.03), tilt=-0.10, rig=choreo),
    "PunchStrike": PoseDef(scale=(1.13, 0.94), tilt=0.075, sway="rise", rig=choreo),
    "PunchRecover": PoseDef(rig=choreo),
    "Shoot": PoseDef(scale=(1.13, 0.94), tilt=0.075, rig=aiming),
    "Parry": PoseDef(art=" O>\n[| \n | \n   ", scale=(1.08, 0.96), tilt=-0.06, rig=parry),
    "Hit": PoseDef(art=" o<\n | \n | \n   ", scale=(1.10, 0.91), tilt=-0.12, rig=recoil, tone="hurt"),
    "Downed": PoseDef(
        art="   \n   \n o_\n───",
        scale=(1.32, 0.62),
        tilt=-0.18,
        rig=sprawl,
        tone="hurt",
    ),
    "Dead": PoseDef(art="   \n   \n   \n_o_", scale=(1.08, 0.88), rig=limp, tone="gone"),
}


def body_transform(pose: PoseDef, speed: float = 1.0) -> tuple[tuple[float, float], float]:
    """Escala e angulo do tronco nesta pose, com o ator em repouso.

    A respiracao entra em zero e a corrida a velocidade cheia: um quadro parado
    precisa de um valor so, e estes sao os que o jogo mostra na maior parte do
    tempo.
    """
    scale, tilt = pose.scale, pose.tilt
    if pose.sway == "lean":
        return scale, -speed * tilt
    if pose.sway == "rise":
        rise = 1.0
        return (scale[0] / rise, scale[1] * rise), tilt / rise
    return scale, tilt


# --- peles, de `actor::skin` ------------------------------------------------

SKINS = [
    {
        "id": "stick",
        "name": "STICK",
        "swap": [],
        "accent": "Oo<>",
        "limb": "|",
        "body": BONE,
        "hurt": BLOOD,
        "gone": IRON,
        "limbs": BONE,
        "description": "O palito de sempre: branco, fino, cabeca na cor do jogador.",
    },
    {
        "id": "heavy",
        "name": "HEAVY",
        "swap": [["O", "0"], ["o", "0"], ["|", "║"], ["_", "═"]],
        "accent": "Oo<>",
        "limb": "║",
        "body": BONE,
        "hurt": BLOOD,
        "gone": IRON,
        "limbs": ASH,
        "description": "Mesma silhueta, glifos grossos. Uma pele nova custa uma linha de tabela.",
    },
    {
        "id": "wraith",
        "name": "WRAITH",
        "swap": [],
        "accent": "Oo<>",
        "limb": "│",
        "body": ASH,
        "hurt": BLOOD,
        "gone": COAL,
        "limbs": IRON,
        "description": "Espectro: corpo esmaecido, membros de sombra. Redesenha a respiracao parada.",
    },
    {
        "id": "ninja",
        "name": "NINJA",
        "swap": [["O", "☻"], ["o", "☻"], ["|", "│"]],
        "accent": "Oo<>☻",
        "limb": "│",
        "body": COAL,
        "hurt": BLOOD,
        "gone": IRON,
        "limbs": IRON,
        "description": "Mascara fechada e corpo quase preto, com olhos na cor do jogador.",
    },
    {
        "id": "robot",
        "name": "ROBOT",
        "swap": [["O", "■"], ["o", "■"], ["|", "╬"], ["_", "═"]],
        "accent": "Oo<>■",
        "limb": "║",
        "body": ASH,
        "hurt": BLOOD,
        "gone": IRON,
        "limbs": BONE,
        "description": "Glifos duplos e cabeca quadrada, deliberadamente rigido.",
    },
    {
        "id": "inferno",
        "name": "INFERNO",
        "swap": [["O", "♦"], ["o", "♦"], ["|", "▒"]],
        "accent": "Oo<>♦",
        "limb": "░",
        "body": BONE,
        "hurt": BLOOD,
        "gone": COAL,
        "limbs": ASH,
        "description": "Tronco em blocos que pulsa como chama sem gastar cor de gameplay.",
    },
]

# Silhuetas que uma pele redesenha, de `skin::art`.
#
# Sao as poses que a pele desenha de verdade, e nao as que ela so troca de
# caractere: tres peles redesenham a respiracao parada para o tronco desvanecer
# ou pulsar, que e o unico lugar onde a diferenca aparece com o boneco parado.
SKIN_ART = {
    "wraith": {"IdleA": " O>\n ▓ \n ▒ \n   ", "IdleB": " O>\n ▒ \n ░ \n   "},
    "ninja": {"IdleA": " ☻>\n ▓ \n │ \n   ", "IdleB": " ☻>\n ▒ \n │ \n   "},
    "inferno": {"IdleA": " ♦>\n ▓ \n ░ \n   ", "IdleB": " ♦>\n ▒ \n ▓ \n   "},
}

# Em que quadro do projeto cada pose redesenhada cai.
ART_FRAMES = {"IdleA": "q_parado_a", "IdleB": "q_parado_b"}


def skin_redraws(art: dict[str, str]) -> dict[str, dict[str, str]]:
    """Traduz uma silhueta de pele para as celulas que ela muda.

    So o que difere da silhueta canonica entra: uma pele que muda dois glifos
    guarda duas linhas, e nao a grade inteira -- que e a mesma economia que o
    `art` do jogo faz ao listar so as poses redesenhadas.
    """
    redraws = {}
    for pose_name, silhouette in art.items():
        canonical = POSES[pose_name].art.split("\n")
        lines = silhouette.split("\n")
        cells = {}
        for row in range(BODY_ROWS):
            for col in range(BODY_COLS):
                line = lines[row] if row < len(lines) else ""
                base = canonical[row] if row < len(canonical) else ""
                glyph = line[col] if col < len(line) else " "
                if glyph != (base[col] if col < len(base) else " "):
                    cells[cell_id(row, col)] = glyph
        if cells:
            redraws[ART_FRAMES[pose_name]] = cells
    return redraws


# --- montagem do projeto ----------------------------------------------------

SIDES = (("frente", 1.0), ("tras", -1.0))
# Os quatro segmentos de um lado, e os dois pontos que cada um liga.
SEGMENTS = (
    ("braco", "shoulder", "elbow"),
    ("antebraco", "elbow", "hand"),
    ("coxa", "hip", "knee"),
    ("canela", "knee", "foot"),
)
POINT_NAMES = {
    "shoulder": "ombro",
    "elbow": "cotovelo",
    "hand": "mao",
    "hip": "quadril",
    "knee": "joelho",
    "foot": "pe",
}


def point_id(side: str, name: str) -> str:
    return f"pt_{side}_{POINT_NAMES[name]}"


def cell_id(row: int, col: int) -> str:
    return f"cel_r{row}c{col}"


def segment_id(side: str, name: str) -> str:
    return f"seg_{side}_{name}"


def pose_joints(pose: PoseDef, r: Rigging) -> Joints:
    joints = gait_joints(r)
    pose.rig(joints, r)
    return joints


def cells_of(pose: PoseDef, speed: float = 1.0) -> dict[str, dict]:
    """Onde cada celula da silhueta cai, ja com a deformacao do tronco.

    O tronco inteiro escala e gira em torno da base da arte -- que e onde o
    sprite do corpo e ancorado no jogo --, entao a celula guarda a posicao ja
    transformada, mais a escala e o giro que ela herda.
    """
    (scale_x, scale_y), angle = body_transform(pose, speed)
    cosine, sine = math.cos(angle), math.sin(angle)
    lines = pose.art.split("\n")
    cells = {}
    for row in range(BODY_ROWS):
        line = lines[row] if row < len(lines) else ""
        for col in range(BODY_COLS):
            local_x = (col - (BODY_COLS - 1) / 2) * CELL_W * scale_x
            local_y = ((BODY_ROWS - row) * CELL_H - CELL_H / 2) * scale_y
            x, y = to_canvas(
                local_x * cosine - local_y * sine,
                BODY_BASE + local_x * sine + local_y * cosine,
            )
            cells[cell_id(row, col)] = {
                "glyph": line[col] if col < len(line) else " ",
                "x": x,
                "y": y,
                "scale_x": round(scale_x, 4),
                "scale_y": round(scale_y, 4),
                # O angulo do jogo gira no sentido anti-horario com y para cima;
                # no canvas, y desce, entao o mesmo giro troca de sinal.
                "rotation": round(-math.degrees(angle), 4),
            }
    return cells


def points_of(pose: PoseDef, **rigging) -> dict[str, dict]:
    """Onde cada articulacao cai nesta pose, dos dois lados."""
    points = {}
    for side, sign in SIDES:
        joints = pose_joints(pose, Rigging(side=sign, **rigging))
        for name, (x, y) in joints.items():
            canvas_x, canvas_y = to_canvas(x, y)
            points[point_id(side, name)] = {"x": canvas_x, "y": canvas_y}
    return points


def state_of(pose_name: str, **rigging) -> dict[str, dict]:
    """O estado completo de uma pose: celulas do tronco e articulacoes."""
    pose = POSES[pose_name]
    speed = rigging.pop("speed", 1.0)
    return cells_of(pose, speed) | points_of(pose, **rigging)


def difference(state: dict[str, dict], rest: dict[str, dict]) -> dict[str, dict]:
    """So o que mudou em relacao ao repouso -- que e o que um quadro guarda."""
    keys = {}
    for item_id, fields in state.items():
        changed = {
            key: value for key, value in fields.items() if rest.get(item_id, {}).get(key) != value
        }
        if changed:
            keys[item_id] = changed
    return keys


def frame(name: str, pose_name: str, rest: dict, hold: int = 1, **rigging) -> dict:
    pose = POSES[pose_name]
    return {
        "id": f"q_{name}",
        "name": name,
        "hold": hold,
        "tone": pose.tone,
        "keys": difference(state_of(pose_name, **rigging), rest),
        "note": f"Pose::{pose_name} de actor::rig",
    }


def punch_clip(clip_name: str, strike: Strike, rest: dict) -> dict:
    """Um golpe: as tres fases, com a duracao real de cada uma.

    Um tempo vale 10ms, entao `hold` le direto como centesimo de segundo e o
    total bate com `duration` de `combat::unarmed_move`.
    """
    holds = [max(1, round(strike.duration * share * 100)) for share in PHASE_SPLIT]
    phases = ("PunchWindup", "PunchStrike", "PunchRecover")
    return {
        "id": f"anim_{clip_name}",
        "name": clip_name,
        "fps": 100.0,
        "loop": False,
        "description": (
            f"{strike.name}, de pose::{strike.name.replace(' ', '_')}. "
            f"{round(strike.duration * 1000)}ms no total; cada tempo vale 10ms."
        ),
        "frames": [
            frame(
                f"{clip_name}_{step}",
                phase,
                rest,
                hold=hold,
                strike=(strike, index),
            )
            for index, (step, phase, hold) in enumerate(
                zip(("preparo", "contato", "recuperacao"), phases, holds)
            )
        ],
    }


def build() -> dict:
    # O repouso e o primeiro quadro da respiracao parada, sem passada nenhuma.
    rest = state_of("IdleA", gait=0.0)

    elements = []
    for row in range(BODY_ROWS):
        for col in range(BODY_COLS):
            item_id = cell_id(row, col)
            elements.append(
                {
                    "id": item_id,
                    "glyph": rest[item_id]["glyph"],
                    "x": rest[item_id]["x"],
                    "y": rest[item_id]["y"],
                    "font_size": CELL_H,
                    "scale_x": rest[item_id]["scale_x"],
                    "scale_y": rest[item_id]["scale_y"],
                    "flip_x": False,
                    "flip_y": False,
                    "rotation": rest[item_id]["rotation"],
                    "color": BONE,
                    "layer": 1,
                    "font_path": "assets/fonts/ibm_vga_8x16.bin",
                    "role": "body",
                    "span": [],
                }
            )
    for side, sign in SIDES:
        for name, start, end in SEGMENTS:
            elements.append(
                {
                    "id": segment_id(side, name),
                    "glyph": "|",
                    "x": 0.0,
                    "y": 0.0,
                    "font_size": CELL_H,
                    # O membro do jogo e um glyph esticado ate 0,72 da largura;
                    # a altura sai do comprimento e por isso nao mora aqui.
                    "scale_x": 0.72,
                    "scale_y": 1.0,
                    "flip_x": False,
                    "flip_y": False,
                    "rotation": 0.0,
                    "color": BONE,
                    # O lado de tras passa atras do tronco e o da frente por
                    # cima: e o que da profundidade a um boneco chapado.
                    "layer": 2 if sign > 0 else 0,
                    "font_path": "assets/fonts/ibm_vga_8x16.bin",
                    "role": "limb",
                    "span": [point_id(side, start), point_id(side, end)],
                }
            )

    joints = []
    for side, _ in SIDES:
        for name in ("shoulder", "elbow", "hand", "hip", "knee", "foot"):
            item_id = point_id(side, name)
            linked = [
                segment_id(side, segment)
                for segment, start, end in SEGMENTS
                if name in (start, end)
            ]
            joints.append(
                {
                    "id": item_id,
                    "name": f"{POINT_NAMES[name]}_{side}",
                    "x": rest[item_id]["x"],
                    "y": rest[item_id]["y"],
                    "parent_id": "",
                    "attached_element_id": "",
                    "part_a_element_id": linked[0] if linked else "",
                    "part_b_element_id": linked[1] if len(linked) > 1 else "",
                    "constraint_type": "pivot",
                    "fixed": False,
                    "color": "#ffcc33" if side == "frente" else "#c98f2a",
                    "kind": "joint",
                    "description": f"Joints::{name}, lado {side}, de actor::rig.",
                }
            )

    attention = [
        {
            "id": "at_rosto",
            "name": "rosto",
            "x": rest[cell_id(0, 1)]["x"],
            "y": rest[cell_id(0, 1)]["y"],
            "parent_id": "",
            "attached_element_id": cell_id(0, 1),
            "part_a_element_id": "",
            "part_b_element_id": "",
            "constraint_type": "pivot",
            "fixed": False,
            "color": "#ff4dc4",
            "kind": "attention",
            "description": (
                "No jogo esta celula e apagada da silhueta e o rosto (olho, nariz e boca) "
                "e desenhado por cima dela, por actor::face. O 'O' aqui e so referencia."
            ),
        }
    ]

    labels = [
        {
            "id": "lb_tronco",
            "name": "tronco",
            "element_ids": [cell_id(r, c) for r in range(BODY_ROWS) for c in range(BODY_COLS)],
            "description": "A silhueta 3x4 do corpo. Cada pose reescreve os glifos dela.",
            "label_ids": [],
        }
    ]
    for side, _ in SIDES:
        for limb, parts in (("braco", ("braco", "antebraco")), ("perna", ("coxa", "canela"))):
            labels.append(
                {
                    "id": f"lb_{limb}_{side}",
                    "name": f"{limb}_{side}",
                    "element_ids": [segment_id(side, part) for part in parts],
                    "description": f"Os dois segmentos do {limb} de {side}.",
                    "label_ids": [],
                }
            )
    labels.append(
        {
            "id": "lb_boneco",
            "name": "boneco",
            "element_ids": [],
            "description": "O ator inteiro: tronco e os quatro membros.",
            "label_ids": [label["id"] for label in labels],
        }
    )

    clips = [
        {
            "id": "anim_parado",
            "name": "parado",
            "fps": 2.2,
            "loop": True,
            "description": "Respiracao parada. O jogo alterna os dois quadros a 2,2 por segundo.",
            "frames": [
                frame("parado_a", "IdleA", rest, gait=0.0),
                frame("parado_b", "IdleB", rest, gait=0.0),
            ],
        },
        {
            "id": "anim_corrida",
            "name": "corrida",
            "fps": round(RUN_SPEED / RUN_STRIDE, 2),
            "loop": True,
            "description": (
                "Ciclo de corrida. O quadro e amarrado a distancia percorrida -- um a cada 9px "
                f"-- entao a {RUN_SPEED:g}px/s a cadencia e a fps deste clipe."
            ),
            "frames": [
                # A passada vem de `-cos(x/54 * TAU)`, medida no meio do trecho
                # em que cada quadro fica na tela.
                frame(
                    f"corrida_{index + 1}",
                    f"Run{letter}",
                    rest,
                    gait=round(-math.cos(math.tau * (index + 0.5) / 6), 4),
                )
                for index, letter in enumerate("ABCDEF")
            ],
        },
        {
            "id": "anim_escalada",
            "name": "escalada",
            "fps": round(CLIMB_SPEED / CLIMB_STRIDE, 2),
            "loop": True,
            "description": "Bracadas na corrente, uma a cada 16px de subida.",
            "frames": [
                frame("escalada_a", "ClimbA", rest, cycle=round(math.pi / 2, 4)),
                frame("escalada_b", "ClimbB", rest, cycle=round(3 * math.pi / 2, 4)),
            ],
        },
        {
            "id": "anim_agachado",
            "name": "agachado",
            "fps": 1.0,
            "loop": True,
            "description": "Pose unica. O corpo inteiro desce, e nao so as pernas.",
            "frames": [frame("agachado", "Crouch", rest)],
        },
        {
            "id": "anim_ar",
            "name": "ar",
            "fps": 2.0,
            "loop": False,
            "description": (
                "Impulso e queda. O salto e medido no lancamento (air=1) e a queda na descida "
                "cheia (air=-1); no jogo os dois variam com a velocidade vertical."
            ),
            "frames": [
                frame("salto", "Jump", rest, air=1.0),
                frame("queda", "Fall", rest, air=-1.0),
            ],
        },
        {
            "id": "anim_mira",
            "name": "mira",
            "fps": 1.0,
            "loop": True,
            "description": "Tiro com a mira na horizontal, para a frente.",
            "frames": [frame("mira", "Shoot", rest)],
        },
        {
            "id": "anim_guarda",
            "name": "guarda",
            "fps": 1.0,
            "loop": True,
            "description": "Guarda alta. Tira o controle de quem esta nela.",
            "frames": [frame("guarda", "Parry", rest)],
        },
        {
            "id": "anim_dano",
            "name": "dano",
            "fps": 2.0,
            "loop": False,
            "description": (
                "Apanhar, cair e morrer. Cada quadro tem o seu papel de cor: os dois primeiros "
                "usam `hurt` e o ultimo `gone`."
            ),
            "frames": [
                frame("apanhando", "Hit", rest),
                frame("caido", "Downed", rest),
                frame("morto", "Dead", rest),
            ],
        },
    ]
    clips += [
        punch_clip("soco_jab", JAB, rest),
        punch_clip("soco_cross", CROSS, rest),
        punch_clip("soco_gancho", UPPERCUT, rest),
        punch_clip("rasteira", SWEEP, rest),
        punch_clip("voadora", DIVE_KICK, rest),
    ]

    return {
        "app": "Glyph Forge",
        "version": 7,
        "canvas": {
            "width": CANVAS[0],
            "height": CANVAS[1],
            "background": "#181818",
            "grid_size": 8,
            "accent": P1,
            # O boneco tem 64px de altura: a 100% ele cabe num polegar e os
            # doze pontos do rig viram um borrao de nomes sobrepostos.
            "zoom": 4.0,
        },
        "default_font": {
            "kind": "bitmap_rom",
            "asset": "assets/fonts/ibm_vga_8x16.bin",
            "encoding": "CP437",
            "glyph_size": [CELL_W, CELL_H],
        },
        "elements": elements,
        "rig": {
            "joints": joints,
            "semantics": {
                "part_a_element_id": "Primeira peca conectada pelo ponto independente.",
                "part_b_element_id": "Segunda peca conectada pelo ponto independente.",
                "constraint_type": "pivot permite giro relativo; fixed solda as duas pecas.",
                "fixed": "Quando verdadeiro, o ponto tambem esta ancorado ao mundo.",
            },
        },
        "attention_points": attention,
        "labels": labels,
        "animation": {
            "clips": clips,
            "semantics": {
                "elements": "A lista `elements` e a pose de repouso; um quadro guarda so a diferenca.",
                "keys": "id da peca -> campo -> valor.",
                "hold": "Por quantos tempos o quadro fica na tela.",
                "tone": "Papel de cor do corpo neste quadro: body, hurt ou gone.",
            },
        },
        "skins": [
            skin | {"art": skin_redraws(SKIN_ART.get(skin["id"], {}))} for skin in SKINS
        ],
        "active_skin": "stick",
        "skin_semantics": {
            "role": "body usa as cores da pele; limb e um segmento de membro.",
            "swap": "Trocas de glifo [de, para], aplicadas depois de a cor estar decidida.",
            "accent": "Glifos que recebem canvas.accent, a cor de quem veste o boneco.",
            "art": "quadro -> peca -> glifo, para a pele que redesenha um quadro em vez de "
            "so trocar caracteres.",
        },
        "source": {
            "poses": "src/actor/rig.rs (POSES, Joints::gait e os ajustes por pose)",
            "strikes": "src/actor/pose.rs (UNARMED_COMBO, SWEEP, DIVE_KICK)",
            "skins": "src/actor/skin.rs (CATALOG)",
            "palette": "src/ascii/palette.rs",
            "speeds": "src/actor/motion.rs (RUN_SPEED, CLIMB_SPEED, JUMP_SPEED)",
            "generated_by": "glyph_forge/bake_actor.py",
        },
        "notes": (
            "Boneco do jogo, gerado de src/actor/. A pose de repouso e IdleA sem passada.\n\n"
            "Cada segmento de membro (role=limb) e derivado dos dois pontos que ele liga: "
            "arraste os pontos para dobrar o braco. Os numeros dos pontos, em coordenadas do "
            "ator (x para a frente, y para cima, pes na origem), sao os mesmos de `Joints` em "
            "rig.rs -- diminua ORIGIN do canvas para converter de volta:\n"
            f"  x_ator = x_canvas - {ORIGIN[0]:g}   |   y_ator = {ORIGIN[1]:g} - y_canvas\n\n"
            "O tronco e uma grade 3x4 de celulas; cada pose reescreve os glifos e a deformacao "
            "delas. A celula da cabeca (cel_r0c1) e apagada no jogo, onde o rosto e desenhado "
            "por cima -- veja o ponto de atencao 'rosto'.\n\n"
            "As poses paradas foram medidas com respiracao zero, corrida a velocidade cheia e "
            "mira na horizontal. Ajuste o que quiser: o que voltar para o Rust sao as "
            "coordenadas dos pontos."
        ),
    }


def self_test() -> None:
    """Confere a transcricao contra os numeros que estao no Rust.

    Uma porta e sempre uma copia, e uma copia sempre pode ter um sinal trocado.
    Cada valor aqui foi lido direto de `rig.rs` ou `pose.rs`, entao um erro de
    transcricao para de sair calado -- ele diz qual articulacao discorda.
    """

    def actor(item_id: str, state: dict) -> tuple[float, float]:
        """De volta ao espaco do ator, que e onde os numeros do Rust vivem."""
        return (
            round(state[item_id]["x"] - ORIGIN[0], 3),
            round(ORIGIN[1] - state[item_id]["y"], 3),
        )

    # A passada padrao, de `Joints::gait` com a passada em zero.
    rest = state_of("IdleA", gait=0.0)
    for name, expected in (
        ("shoulder", (2.5, 15.0)),
        ("elbow", (7.0, 6.0)),
        ("hand", (5.0, -2.0)),
        ("hip", (2.2, -7.0)),
        ("knee", (5.0, -18.0)),
        ("foot", (7.0, -31.0)),
    ):
        got = actor(point_id("frente", name), rest)
        assert got == expected, f"gait: {name} deu {got}, esperado {expected}"
        mirrored = actor(point_id("tras", name), rest)
        assert mirrored == (-expected[0], expected[1]), f"gait: {name} de tras deu {mirrored}"

    # Agachar desce o corpo inteiro, e nao so as pernas.
    crouching = state_of("Crouch")
    assert actor(point_id("frente", "shoulder"), crouching) == (2.5, -4.0)
    assert actor(point_id("frente", "foot"), crouching) == (13.0, -31.0)

    # Morrer abre as pernas e nao mexe nos bracos.
    dead = state_of("Dead")
    assert actor(point_id("frente", "knee"), dead) == (13.0, -26.0)
    assert actor(point_id("frente", "foot"), dead) == (22.0, -29.0)

    # Com a mira na horizontal o golpe nao gira, entao a mao do contato cai
    # exatamente onde a coreografia a escreveu.
    jab = state_of("PunchStrike", strike=(JAB, 1))
    assert actor(point_id("frente", "hand"), jab) == (26.0, 14.0)
    assert actor(point_id("tras", "hand"), jab) == (-3.0, 18.0)
    # No cruzado quem avanca e o braco de tras.
    cross = state_of("PunchStrike", strike=(CROSS, 1))
    assert actor(point_id("tras", "hand"), cross) == (33.0, 12.0)
    # A rasteira e o unico golpe que manda na perna, e so na da frente.
    sweep = state_of("PunchStrike", strike=(SWEEP, 1))
    assert actor(point_id("frente", "foot"), sweep) == (40.0, -34.0)
    assert actor(point_id("tras", "foot"), sweep) != (40.0, -34.0)

    # A cabeca da silhueta de pe cai no meio da linha de cima; agachado, uma
    # linha abaixo. E dai que o rosto sai no jogo.
    assert actor(cell_id(0, 1), rest) == (0.0, 24.0)
    assert rest[cell_id(0, 1)]["glyph"] == "O"
    assert crouching[cell_id(1, 1)]["glyph"] == "O"
    assert crouching[cell_id(0, 1)]["glyph"] == " "

    # A corrida inclina o tronco para a frente; parado nao inclina nada.
    assert rest[cell_id(0, 1)]["rotation"] == 0.0
    lean = state_of("RunA", gait=0.0)[cell_id(0, 1)]["rotation"]
    assert lean > 0, f"a corrida deveria cair para a frente, deu {lean}"
    assert round(lean, 2) == round(math.degrees(0.20), 2)

    # As peles redesenham so o que muda: duas celulas do tronco parado.
    wraith = skin_redraws(SKIN_ART["wraith"])
    assert wraith == {
        "q_parado_a": {"cel_r1c1": "▓", "cel_r2c1": "▒"},
        "q_parado_b": {"cel_r1c1": "▒", "cel_r2c1": "░"},
    }, wraith

    scene = build()
    assert len(scene["elements"]) == BODY_COLS * BODY_ROWS + 8
    assert len(scene["rig"]["joints"]) == 12
    poses_used = {
        frame["note"] for clip in scene["animation"]["clips"] for frame in clip["frames"]
    }
    assert len(poses_used) == len(POSES), (
        f"{len(POSES) - len(poses_used)} pose(s) do jogo ficaram sem quadro"
    )
    # Um segmento nunca guarda a propria posicao: ela sai dos dois pontos.
    for clip in scene["animation"]["clips"]:
        for frame in clip["frames"]:
            for side, _ in SIDES:
                for name, _, _ in SEGMENTS:
                    assert segment_id(side, name) not in frame["keys"]
    json.dumps(scene)
    print("self-test OK")


def main() -> None:
    target = Path(__file__).resolve().parent / "creations" / "bonecos" / "boneco.glyph.json"
    target.parent.mkdir(parents=True, exist_ok=True)
    scene = build()
    target.write_text(json.dumps(scene, ensure_ascii=False, indent=2), encoding="utf-8")
    frames = sum(len(clip["frames"]) for clip in scene["animation"]["clips"])
    print(
        f"{target}\n"
        f"  {len(scene['elements'])} pecas, {len(scene['rig']['joints'])} pontos, "
        f"{len(scene['skins'])} peles, {len(scene['animation']['clips'])} animacoes, "
        f"{frames} quadros"
    )


if __name__ == "__main__":
    if "--self-test" in sys.argv:
        self_test()
    else:
        main()
