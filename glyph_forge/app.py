from __future__ import annotations

import copy
import colorsys
import json
import math
import os
import sys
import tempfile
import uuid
from dataclasses import asdict, dataclass, field
from functools import lru_cache
from pathlib import Path
import tkinter as tk
from tkinter import filedialog, messagebox, simpledialog, ttk

try:
    from PIL import Image, ImageChops, ImageColor, ImageDraw, ImageFont, ImageTk
except ImportError as exc:  # pragma: no cover - only used on machines without Pillow
    raise SystemExit("Pillow nao encontrado. Rode: python -m pip install Pillow") from exc


APP_NAME = "Glyph Forge"
PROJECT_VERSION = 7

# Campos que um quadro de animacao pode sobrescrever.
#
# Sao os que descrevem *onde e como* a peca aparece, e nenhum que diga *que
# peca ela e*: `id`, `font_path` e o resto ficam de fora de proposito, para que
# um quadro nunca consiga transformar uma peca em outra.
ANIMATABLE = (
    "x",
    "y",
    "rotation",
    "scale_x",
    "scale_y",
    "flip_x",
    "flip_y",
    "glyph",
    "color",
    "font_size",
    "layer",
    "offset",
)

# Campos que um segmento nao autora: eles saem dos dois pontos que ele liga.
DERIVED = ("x", "y", "rotation", "scale_y")

# Campos que uma peca presa a um ponto nao autora: eles saem do ponto mais o
# `offset`. O giro fica de fora de proposito -- e o que o animador desenha.
CARRIED = ("x", "y")

# Papeis de cor de um quadro, na ordem em que o seletor os percorre.
TONES = ("body", "hurt", "gone")

# Cor de quem esta dentro do boneco, quando o projeto nao diz outra.
DEFAULT_ACCENT = "#33d9f2"

# Sombra do quadro anterior. Escura o bastante para nao competir com o quadro
# corrente, clara o bastante para se ver contra o fundo padrao.
ONION_COLOR = "#4a4a58"

DOS_GRAPHICS = {
    0x00: " ", 0x01: "☺", 0x02: "☻", 0x03: "♥", 0x04: "♦", 0x05: "♣", 0x06: "♠",
    0x07: "•", 0x08: "◘", 0x09: "○", 0x0A: "◙", 0x0B: "♂", 0x0C: "♀",
    0x0D: "♪", 0x0E: "♫", 0x0F: "☼", 0x10: "►", 0x11: "◄", 0x12: "↕",
    0x13: "‼", 0x14: "¶", 0x15: "§", 0x16: "▬", 0x17: "↨", 0x18: "↑",
    0x19: "↓", 0x1A: "→", 0x1B: "←", 0x1C: "∟", 0x1D: "↔", 0x1E: "▲",
    0x1F: "▼", 0x7F: "⌂",
}
CHAR_TO_DOS = {character: code for code, character in DOS_GRAPHICS.items()}


def default_font_path() -> str:
    project_font = Path(__file__).resolve().parent.parent / "assets" / "fonts" / "ibm_vga_8x16.bin"
    if project_font.is_file():
        return str(project_font)
    candidates = [
        Path(os.environ.get("WINDIR", "")) / "Fonts" / "consola.ttf",
        Path(os.environ.get("WINDIR", "")) / "Fonts" / "lucon.ttf",
        Path("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf"),
        Path("/System/Library/Fonts/Menlo.ttc"),
    ]
    return str(next((path for path in candidates if path.is_file()), ""))


def code_to_char(code: int) -> str:
    return DOS_GRAPHICS.get(code, bytes([code]).decode("cp437"))


def char_to_code(character: str) -> int:
    if character in CHAR_TO_DOS:
        return CHAR_TO_DOS[character]
    try:
        return character.encode("cp437")[0]
    except (UnicodeEncodeError, IndexError):
        return ord("?")


def grid_line_color(background: str) -> str:
    try:
        red, green, blue = ImageColor.getrgb(background)[:3]
    except ValueError:
        red, green, blue = (24, 24, 24)
    target = 0 if red + green + blue > 382 else 255
    mixed = tuple(round(channel * 0.88 + target * 0.12) for channel in (red, green, blue))
    return "#{:02x}{:02x}{:02x}".format(*mixed)


def safe_export_name(value: str) -> str:
    invalid = '<>:"/\\|?*'
    name = "".join("_" if character in invalid or ord(character) < 32 else character for character in value)
    name = name.strip().rstrip(".")
    reserved = {"CON", "PRN", "AUX", "NUL", *(f"COM{i}" for i in range(1, 10)), *(f"LPT{i}" for i in range(1, 10))}
    return f"_{name}" if name.upper() in reserved else name


@lru_cache(maxsize=4)
def read_rom(path: str) -> bytes:
    try:
        data = Path(path).read_bytes()
    except OSError:
        return b""
    return data if len(data) == 4096 else b""


def rom_code_image(code: int, path: str, scale: int = 1, color: str = "#ffffff") -> Image.Image:
    rom = read_rom(path)
    image = Image.new("RGBA", (8, 16))
    if rom:
        pixels = image.load()
        rgba = Image.new("RGBA", (1, 1), color).getpixel((0, 0))
        for y in range(16):
            row = rom[(code & 0xFF) * 16 + y]
            for x in range(8):
                if row & (0x80 >> x):
                    pixels[x, y] = rgba
    if scale != 1:
        image = image.resize((8 * scale, 16 * scale), Image.Resampling.NEAREST)
    return image


def rom_text_image(text: str, path: str, font_size: int, color: str) -> Image.Image:
    lines = (text or " ").splitlines() or [" "]
    width = max(1, max(len(line) for line in lines) * 8)
    height = max(1, len(lines) * 16)
    image = Image.new("RGBA", (width, height))
    for row, line in enumerate(lines):
        for column, character in enumerate(line):
            glyph = rom_code_image(char_to_code(character), path, color=color)
            image.alpha_composite(glyph, (column * 8, row * 16))
    ratio = max(1, font_size) / 16
    size = (max(1, round(width * ratio)), max(1, round(height * ratio)))
    return image.resize(size, Image.Resampling.NEAREST) if size != image.size else image


@dataclass
class Glyph:
    id: str
    glyph: str = "O"
    x: float = 400
    y: float = 300
    font_size: int = 64
    scale_x: float = 1.0
    scale_y: float = 1.0
    flip_x: bool = False
    flip_y: bool = False
    rotation: float = 0.0
    color: str = "#ffffff"
    layer: int = 0
    font_path: str = ""
    # Papel da peca dentro de uma pele: `body` e silhueta, `limb` e segmento de
    # membro, vazio e um glyph solto que guarda a propria cor. Sem isso trocar
    # de pele exigiria reescrever a cor de cada peca, e um projeto que nao usa
    # peles precisaria saber o que uma pele e.
    role: str = ""
    # Os dois pontos que esta peca liga, quando ela e um segmento.
    #
    # Um segmento nao tem posicao propria: `x`, `y`, `rotation` e `scale_y` sao
    # calculados dos dois pontos toda vez que se desenha. E o que faz arrastar o
    # cotovelo dobrar o braco, e o que faz um quadro guardar as coordenadas das
    # articulacoes -- os mesmos numeros que o jogo tem no codigo -- em vez de
    # meio-caminho, angulo e escala, que ninguem sabe reverter a mao.
    span: list[str] = field(default_factory=list)
    # O ponto que carrega esta peca, quando ela e um prop preso a alguem.
    #
    # Uma arma na mao nao tem posicao propria: ela tem uma distancia ate a mao.
    # Sem isto, animar um golpe seria mover a mao e depois mover a arma atras
    # dela em todo quadro, e os dois desenhos discordariam no primeiro que
    # alguem esquecesse. O giro continua sendo autorado quadro a quadro -- e a
    # metade do golpe que o animador quer na mao.
    follow: str = ""
    # Distancia ate o ponto que a carrega, `[dx, dy]`. So vale com `follow`.
    offset: list[float] = field(default_factory=list)

    @classmethod
    def create(cls, x: float, y: float, font_path: str) -> "Glyph":
        return cls(id=uuid.uuid4().hex[:10], x=x, y=y, font_path=font_path)

    @classmethod
    def from_dict(cls, value: dict) -> "Glyph":
        allowed = cls.__dataclass_fields__.keys()
        glyph = cls(**{key: value[key] for key in allowed if key in value})
        if not glyph.font_path or (
            glyph.font_path.lower().endswith("ibm_vga_8x16.bin") and not Path(glyph.font_path).is_file()
        ):
            glyph.font_path = default_font_path()
        return glyph


@dataclass
class Joint:
    id: str
    name: str
    x: float
    y: float
    parent_id: str = ""
    attached_element_id: str = ""
    part_a_element_id: str = ""
    part_b_element_id: str = ""
    constraint_type: str = "pivot"
    fixed: bool = False
    color: str = "#ffcc33"
    kind: str = "joint"
    description: str = ""

    @classmethod
    def create(cls, x: float, y: float, number: int) -> "Joint":
        return cls(id=uuid.uuid4().hex[:10], name=f"joint_{number}", x=x, y=y)

    @classmethod
    def create_attention(cls, x: float, y: float, number: int) -> "Joint":
        return cls(
            id=uuid.uuid4().hex[:10],
            name=f"atencao_{number}",
            x=x,
            y=y,
            color="#ff4dc4",
            kind="attention",
        )

    @classmethod
    def from_dict(cls, value: dict) -> "Joint":
        allowed = cls.__dataclass_fields__.keys()
        return cls(**{key: value[key] for key in allowed if key in value})


@dataclass
class SemanticLabel:
    id: str
    name: str
    element_ids: list[str]
    description: str = ""
    label_ids: list[str] = field(default_factory=list)

    @classmethod
    def create(cls, element_ids: set[str], number: int) -> "SemanticLabel":
        return cls(
            id=uuid.uuid4().hex[:10],
            name=f"rotulo_{number}",
            element_ids=sorted(element_ids),
        )

    @classmethod
    def from_dict(cls, value: dict) -> "SemanticLabel":
        allowed = cls.__dataclass_fields__.keys()
        return cls(**{key: value[key] for key in allowed if key in value})


def resolved_label_elements(
    label_id: str,
    labels_by_id: dict[str, SemanticLabel],
    seen: set[str] | None = None,
) -> set[str]:
    if seen is None:
        seen = set()
    if label_id in seen or label_id not in labels_by_id:
        return set()
    seen = seen | {label_id}
    label = labels_by_id[label_id]
    resolved = set(label.element_ids)
    for child_id in label.label_ids:
        resolved.update(resolved_label_elements(child_id, labels_by_id, seen))
    return resolved


def label_reaches(
    start_id: str,
    target_id: str,
    labels_by_id: dict[str, SemanticLabel],
    seen: set[str] | None = None,
) -> bool:
    if start_id == target_id:
        return True
    if start_id in (seen or set()) or start_id not in labels_by_id:
        return False
    seen = (seen or set()) | {start_id}
    return any(
        label_reaches(child_id, target_id, labels_by_id, seen)
        for child_id in labels_by_id[start_id].label_ids
    )


def infer_nested_labels(labels: list[SemanticLabel]) -> bool:
    """Migrate legacy subset labels into explicit, non-redundant nesting."""
    labels_by_id = {label.id: label for label in labels}
    original = {
        label.id: resolved_label_elements(label.id, labels_by_id)
        for label in labels
    }
    changed = False
    for parent in labels:
        if parent.label_ids or not original[parent.id]:
            continue
        candidates = [
            child
            for child in labels
            if child.id != parent.id
            and original[child.id]
            and original[child.id] < original[parent.id]
        ]
        immediate = [
            child
            for child in candidates
            if not any(
                original[child.id] < original[other.id]
                for other in candidates
                if other.id != child.id
            )
        ]
        if not immediate:
            continue
        parent.label_ids = sorted(child.id for child in immediate)
        nested_elements = set().union(*(original[child.id] for child in immediate))
        parent.element_ids = sorted(set(parent.element_ids) - nested_elements)
        changed = True
    return changed


@dataclass
class Skin:
    """A aparencia de um conjunto, separada da forma dele.

    Uma pele responde tres perguntas -- que glifos desenham o corpo, de que cor
    ele fica em cada papel, e como e um membro -- e nada mais. Quem decide a
    forma continua sendo o quadro. E essa divisao que faz duas peles caberem no
    mesmo projeto sem duplicar uma pose sequer.
    """

    id: str
    name: str
    # Troca de glifo aplicada a silhueta inteira, `[de, para]`.
    swap: list[list[str]] = field(default_factory=list)
    # Caracteres que recebem a cor de quem esta dentro do boneco. Falam nos
    # glifos originais da arte, nao nos trocados por `swap`.
    accent: str = ""
    # Glifo de um segmento de membro.
    limb: str = "|"
    body: str = "#ebebe0"
    hurt: str = "#e02138"
    gone: str = "#4a4d57"
    limbs: str = "#ebebe0"
    # Glifos que esta pele redesenha em quadros especificos:
    # `id do quadro` -> `id da peca` -> glifo.
    #
    # E o caminho caro, ao lado de `swap`: serve para a pele que muda o desenho
    # de um quadro so, e nao o mesmo caractere em toda parte. Uma pele que nao
    # redesenha nada deixa isto vazio.
    art: dict[str, dict[str, str]] = field(default_factory=dict)
    description: str = ""

    @classmethod
    def create(cls, number: int) -> "Skin":
        return cls(id=uuid.uuid4().hex[:10], name=f"pele_{number}")

    @classmethod
    def from_dict(cls, value: dict) -> "Skin":
        allowed = cls.__dataclass_fields__.keys()
        skin = cls(**{key: value[key] for key in allowed if key in value})
        skin.swap = [list(pair)[:2] for pair in skin.swap if len(pair) >= 2]
        skin.art = {
            str(frame_id): {str(item_id): str(glyph) for item_id, glyph in redraws.items()}
            for frame_id, redraws in skin.art.items()
            if isinstance(redraws, dict)
        }
        return skin

    def tone(self, tone: str) -> str:
        return {"body": self.body, "hurt": self.hurt, "gone": self.gone}.get(tone, self.body)

    def swapped(self, text: str) -> str:
        table = {pair[0]: pair[1] for pair in self.swap if pair[0]}
        return "".join(table.get(character, character) for character in text)


@dataclass
class Frame:
    """Um quadro: o que muda em relacao ao repouso, e nada mais.

    Guardar so a diferenca e o que mantem o JSON editavel a mao -- um quadro que
    move um braco tem duas linhas, e nao a cena inteira de novo -- e o que faz
    mexer na pose de repouso alcancar todos os quadros de uma vez.
    """

    id: str
    name: str
    # Quantos tempos o quadro fica na tela. Dobrar um quadro nao exige duplicar.
    hold: int = 1
    # Papel de cor do corpo neste quadro; a pele escolhe a cor concreta.
    tone: str = "body"
    # `id da peca` -> `campo` -> valor. So os campos de `ANIMATABLE`.
    keys: dict[str, dict] = field(default_factory=dict)
    # O que acontece neste quadro, para quem for implementar o golpe: `contato`,
    # `brilho`, `som`. Sao nomes livres de proposito -- o editor nao sabe o que
    # e um hitbox, e nao deveria: ele marca o quadro e quem le decide.
    #
    # Sem isso, "o golpe acerta no segundo quadro" e uma coisa que so existe na
    # cabeca de quem animou, e chega ao Rust como um numero adivinhado.
    marks: list[str] = field(default_factory=list)
    note: str = ""

    @classmethod
    def create(cls, number: int) -> "Frame":
        return cls(id=uuid.uuid4().hex[:10], name=f"quadro_{number}")

    @classmethod
    def from_dict(cls, value: dict) -> "Frame":
        allowed = cls.__dataclass_fields__.keys()
        frame = cls(**{key: value[key] for key in allowed if key in value})
        frame.hold = max(1, int(frame.hold))
        frame.tone = frame.tone if frame.tone in TONES else "body"
        frame.keys = {
            str(item_id): {key: value for key, value in fields.items() if key in ANIMATABLE}
            for item_id, fields in frame.keys.items()
            if isinstance(fields, dict)
        }
        frame.marks = [str(mark).strip() for mark in frame.marks if str(mark).strip()]
        return frame


@dataclass
class Clip:
    """Uma animacao: os quadros na ordem em que tocam."""

    id: str
    name: str
    fps: float = 8.0
    loop: bool = True
    description: str = ""
    frames: list[Frame] = field(default_factory=list)

    @classmethod
    def create(cls, number: int) -> "Clip":
        return cls(id=uuid.uuid4().hex[:10], name=f"animacao_{number}")

    @classmethod
    def from_dict(cls, value: dict) -> "Clip":
        allowed = set(cls.__dataclass_fields__.keys()) - {"frames"}
        clip = cls(**{key: value[key] for key in allowed if key in value})
        clip.fps = max(0.5, float(clip.fps))
        clip.frames = [Frame.from_dict(frame) for frame in value.get("frames", [])]
        return clip

    def frame(self, index: int | None) -> Frame | None:
        if index is None or not 0 <= index < len(self.frames):
            return None
        return self.frames[index]


def scene_skins(scene: dict) -> list[Skin]:
    return [Skin.from_dict(value) for value in scene.get("skins", [])]


def scene_clips(scene: dict) -> list[Clip]:
    return [Clip.from_dict(value) for value in scene.get("animation", {}).get("clips", [])]


def scene_accent(scene: dict) -> str:
    return scene.get("canvas", {}).get("accent", DEFAULT_ACCENT)


def active_skin(scene: dict) -> Skin | None:
    skins = scene_skins(scene)
    if not skins:
        return None
    chosen = scene.get("active_skin", "")
    return next((skin for skin in skins if skin.id == chosen), skins[0])


def dressed(
    element: Glyph, skin: Skin | None, tone: str, accent: str, frame_id: str = ""
) -> Glyph:
    """A peca como ela deve aparecer vestindo esta pele.

    Um glyph sem papel sai intacto: e o que deixa um projeto sem peles funcionar
    exatamente como antes, e o que permite misturar cenario solto e boneco
    vestido na mesma cena.
    """
    if skin is None or element.role not in ("body", "limb"):
        return element
    shown = copy.copy(element)
    if element.role == "limb":
        shown.glyph = skin.limb
        shown.color = skin.limbs
        return shown
    # A troca de glifo roda depois de a cor estar decidida, e o acento fala nos
    # caracteres originais: uma pele que troque um caractere acentuado continua
    # deixando ele com a cor do jogador.
    shown.color = (
        accent
        if skin.accent and any(character in skin.accent for character in element.glyph)
        else skin.tone(tone)
    )
    redrawn = skin.art.get(frame_id, {}).get(element.id)
    shown.glyph = redrawn if redrawn is not None else skin.swapped(element.glyph)
    return shown


def solve_spans(elements: list[Glyph], joints: list[Joint]) -> None:
    """Poe cada peca presa no lugar que o rig manda.

    Sao duas presas diferentes: um segmento fica *entre* dois pontos e um prop
    fica *a uma distancia de* um ponto. O glyph e desenhado em pe, entao o
    angulo mede a partir da vertical da tela -- e por isso que `atan2` recebe os
    argumentos trocados e o x negado.
    """
    by_id = {joint.id: joint for joint in joints}
    for element in elements:
        carrier = by_id.get(element.follow) if element.follow else None
        if carrier is not None:
            offset = element.offset if len(element.offset) == 2 else [0.0, 0.0]
            element.x = round(carrier.x + offset[0], 4)
            element.y = round(carrier.y + offset[1], 4)
        if len(element.span) != 2:
            continue
        start, end = by_id.get(element.span[0]), by_id.get(element.span[1])
        if start is None or end is None:
            continue
        delta_x, delta_y = end.x - start.x, end.y - start.y
        element.x = round((start.x + end.x) / 2, 4)
        element.y = round((start.y + end.y) / 2, 4)
        element.rotation = round(math.degrees(math.atan2(-delta_x, delta_y)), 4)
        element.scale_y = round(
            max(0.02, math.hypot(delta_x, delta_y) / max(1, element.font_size)), 4
        )


def staged_scene(scene: dict, clip_index: int | None = None, frame_index: int | None = None) -> dict:
    """A cena como ela deve ser desenhada: quadro aplicado e pele resolvida.

    Existe para que `render_scene` e `flatten_scene` nao precisem saber o que
    uma animacao e: quem quiser o quadro 3 pede a cena daquele quadro, e desenha
    a cena de sempre.
    """
    staged = copy.deepcopy(scene)
    clips = scene_clips(scene)
    frame = None
    if clip_index is not None and 0 <= clip_index < len(clips):
        frame = clips[clip_index].frame(frame_index)
    keys = frame.keys if frame else {}
    for group in (
        staged.get("elements", []),
        staged.get("rig", {}).get("joints", []),
        staged.get("attention_points", []),
    ):
        for item in group:
            item.update(keys.get(str(item.get("id", "")), {}))
    tone = frame.tone if frame else "body"
    skin = active_skin(scene)
    accent = scene_accent(scene)
    elements = [Glyph.from_dict(value) for value in staged.get("elements", [])]
    solve_spans(elements, scene_joints(staged) + scene_attention_points(staged))
    frame_id = frame.id if frame else ""
    staged["elements"] = [
        asdict(dressed(element, skin, tone, accent, frame_id)) for element in elements
    ]
    return staged


def scene_joints(scene: dict) -> list[Joint]:
    rig = scene.get("rig", {})
    values = rig.get("joints", scene.get("joints", []))
    joints = [Joint.from_dict(value) for value in values]
    for joint in joints:
        joint.kind = "joint"
        if not joint.part_a_element_id and joint.attached_element_id:
            joint.part_a_element_id = joint.attached_element_id
        joint.attached_element_id = ""
    return joints


def scene_attention_points(scene: dict) -> list[Joint]:
    points = [Joint.from_dict(value) for value in scene.get("attention_points", [])]
    for point in points:
        point.kind = "attention"
        point.parent_id = ""
        point.fixed = False
    return points


def portable_scene(scene: dict) -> dict:
    portable = copy.deepcopy(scene)
    for element in portable.get("elements", []):
        if str(element.get("font_path", "")).lower().endswith("ibm_vga_8x16.bin"):
            element["font_path"] = "assets/fonts/ibm_vga_8x16.bin"
    return portable


def load_font(path: str, size: int) -> ImageFont.ImageFont:
    size = max(1, int(size))
    try:
        return ImageFont.truetype(path, size) if path else ImageFont.load_default(size)
    except (OSError, ValueError):
        return ImageFont.load_default(size)


def glyph_image(glyph: Glyph, include_rotation: bool = True) -> Image.Image:
    text = glyph.glyph or " "
    is_rom = glyph.font_path.lower().endswith(".bin") and bool(read_rom(glyph.font_path))
    if is_rom:
        image = rom_text_image(text, glyph.font_path, glyph.font_size, glyph.color)
    else:
        font = load_font(glyph.font_path, glyph.font_size)
        probe = Image.new("RGBA", (1, 1))
        draw = ImageDraw.Draw(probe)
        bbox = draw.multiline_textbbox((0, 0), text, font=font, spacing=0)
        padding = 4
        width = max(1, bbox[2] - bbox[0] + padding * 2)
        height = max(1, bbox[3] - bbox[1] + padding * 2)
        image = Image.new("RGBA", (width, height))
        ImageDraw.Draw(image).multiline_text(
            (padding - bbox[0], padding - bbox[1]), text, font=font, fill=glyph.color, spacing=0
        )
    width, height = image.size
    scaled = (
        max(1, round(width * max(0.02, glyph.scale_x))),
        max(1, round(height * max(0.02, glyph.scale_y))),
    )
    if scaled != image.size:
        image = image.resize(scaled, Image.Resampling.NEAREST if is_rom else Image.Resampling.LANCZOS)
    if glyph.flip_x:
        image = image.transpose(Image.Transpose.FLIP_LEFT_RIGHT)
    if glyph.flip_y:
        image = image.transpose(Image.Transpose.FLIP_TOP_BOTTOM)
    if include_rotation and glyph.rotation % 360:
        image = image.rotate(
            -glyph.rotation,
            expand=True,
            resample=Image.Resampling.NEAREST if is_rom else Image.Resampling.BICUBIC,
        )
    return image


def render_scene(scene: dict, show_rig: bool = False) -> Image.Image:
    canvas = scene.get("canvas", {})
    width = max(1, int(canvas.get("width", 800)))
    height = max(1, int(canvas.get("height", 600)))
    result = Image.new("RGBA", (width, height), canvas.get("background", "#181818"))
    elements = [Glyph.from_dict(value) for value in scene.get("elements", [])]
    joints = scene_joints(scene)
    attention_points = scene_attention_points(scene)
    if show_rig:
        draw = ImageDraw.Draw(result)
        by_id = {joint.id: joint for joint in joints}
        element_by_id = {element.id: element for element in elements}
        for joint in joints:
            parent = by_id.get(joint.parent_id)
            if parent:
                draw.line((parent.x, parent.y, joint.x, joint.y), fill="#4db7ff", width=2)
            for part_id in (joint.part_a_element_id, joint.part_b_element_id):
                part = element_by_id.get(part_id)
                if part and (part.x != joint.x or part.y != joint.y):
                    draw.line((part.x, part.y, joint.x, joint.y), fill="#4db7ff", width=1)
        for point in attention_points:
            attached = element_by_id.get(point.attached_element_id)
            if attached and (attached.x != point.x or attached.y != point.y):
                draw.line((attached.x, attached.y, point.x, point.y), fill=point.color, width=1)
    for glyph in sorted(enumerate(elements), key=lambda pair: (pair[1].layer, pair[0])):
        element = glyph[1]
        image = glyph_image(element)
        position = (round(element.x - image.width / 2), round(element.y - image.height / 2))
        result.alpha_composite(image, position)
    if show_rig:
        draw = ImageDraw.Draw(result)
        label_font = ImageFont.load_default(12)
        for joint in joints:
            radius = 7
            box = (joint.x - radius, joint.y - radius, joint.x + radius, joint.y + radius)
            if joint.fixed or joint.constraint_type == "fixed":
                draw.rectangle(box, fill=joint.color, outline="#000000", width=2)
            else:
                draw.ellipse(box, fill=joint.color, outline="#000000", width=2)
            if joint.fixed:
                draw.line((joint.x - 10, joint.y, joint.x + 10, joint.y), fill=joint.color, width=2)
                draw.line((joint.x, joint.y - 10, joint.x, joint.y + 10), fill=joint.color, width=2)
            draw.text((joint.x + 10, joint.y - 8), joint.name, font=label_font, fill="#ffffff", stroke_width=2, stroke_fill="#000000")
        for point in attention_points:
            radius = 8
            draw.polygon(
                ((point.x, point.y - radius), (point.x + radius, point.y), (point.x, point.y + radius), (point.x - radius, point.y)),
                fill=point.color,
                outline="#000000",
            )
            draw.text((point.x + 11, point.y - 8), point.name, font=label_font, fill="#ffffff", stroke_width=2, stroke_fill="#000000")
    return result


def problems(scene: dict) -> list[str]:
    """O que o jogo nao vai conseguir ler nesta cena.

    O Rust acha as pecas por *nome* de rotulo e de ponto, e um nome repetido ou
    trocado vira `panic!` na primeira vez que a arma aparece -- longe daqui, e
    quase sempre no meio de uma partida. Conferir antes custa um clique.
    """
    found = []
    elements = [Glyph.from_dict(value) for value in scene.get("elements", [])]
    joints = scene_joints(scene) + scene_attention_points(scene)
    ids = {item.id for item in (*elements, *joints)}

    for kind, group in (
        ("ponto", [joint for joint in joints if joint.kind == "joint"]),
        ("ponto de atencao", [joint for joint in joints if joint.kind == "attention"]),
        ("rotulo", [SemanticLabel.from_dict(value) for value in scene.get("labels", [])]),
        ("animacao", scene_clips(scene)),
    ):
        seen: dict[str, int] = {}
        for item in group:
            seen[item.name] = seen.get(item.name, 0) + 1
        found += [
            f"{kind}: o nome '{name}' aparece {count} vezes -- quem le por nome pega o errado"
            for name, count in seen.items()
            if count > 1
        ]

    for element in elements:
        if element.span and not set(element.span).issubset(ids):
            found.append(f"peca '{element.id}': segmento aponta para um ponto que nao existe")
        if element.follow and element.follow not in ids:
            found.append(f"peca '{element.id}': presa a um ponto que nao existe")
        if element.follow and len(element.offset) != 2:
            found.append(f"peca '{element.id}': presa a um ponto mas sem distancia gravada")

    for clip in scene_clips(scene):
        if not clip.frames:
            found.append(f"animacao '{clip.name}': nenhum quadro")
        for frame in clip.frames:
            orphans = sorted(set(frame.keys) - ids)
            if orphans:
                found.append(
                    f"animacao '{clip.name}', quadro '{frame.name}': "
                    f"guarda peca que nao existe ({', '.join(orphans)})"
                )

    labels = [SemanticLabel.from_dict(value) for value in scene.get("labels", [])]
    labels_by_id = {label.id: label for label in labels}
    for label in labels:
        if not resolved_label_elements(label.id, labels_by_id):
            found.append(f"rotulo '{label.name}': nao contem glyph nenhum")
        missing = sorted(set(label.element_ids) - ids)
        if missing:
            found.append(
                f"rotulo '{label.name}': aponta para peca que nao existe ({', '.join(missing)})"
            )
    return found


def export_clips(scene: dict, target: Path) -> list[str]:
    """Grava cada animacao como GIF e como um PNG por quadro.

    O GIF e para olhar -- e a unica forma de saber se um ciclo fecha sem abrir o
    editor. Os PNGs existem porque um quadro parado e o que se compara lado a
    lado quando dois deles discordam.
    """
    clips = scene_clips(scene)
    if not clips:
        return []
    folder = target / "animacao"
    folder.mkdir(parents=True, exist_ok=True)
    written = []
    for clip_index, clip in enumerate(clips):
        if not clip.frames:
            continue
        name = safe_export_name(clip.name) or f"animacao_{clip_index + 1}"
        images = []
        for frame_index, frame in enumerate(clip.frames):
            image = render_scene(staged_scene(scene, clip_index, frame_index)).convert("RGB")
            image.save(folder / f"{name}_{frame_index + 1:02d}.png")
            images.append(image)
        step = round(1000 / max(0.5, clip.fps))
        images[0].save(
            folder / f"{name}.gif",
            save_all=True,
            append_images=images[1:],
            duration=[max(20, step * max(1, frame.hold)) for frame in clip.frames],
            loop=0 if clip.loop else 1,
            disposal=2,
        )
        written.append(f"{name}.gif")
    return written


def flatten_scene(scene: dict, cell_width: int = 12, cell_height: int = 24) -> str:
    """Approximate the free canvas as a plain-text grid; JSON remains authoritative."""
    canvas = scene.get("canvas", {})
    columns = max(1, min(200, int(canvas.get("width", 800)) // cell_width))
    rows = max(1, min(120, int(canvas.get("height", 600)) // cell_height))
    grid = [[" " for _ in range(columns)] for _ in range(rows)]
    elements = [Glyph.from_dict(value) for value in scene.get("elements", [])]
    for _, glyph in sorted(enumerate(elements), key=lambda pair: (pair[1].layer, pair[0])):
        lines = (glyph.glyph or " ").splitlines() or [" "]
        center_row = round(glyph.y / cell_height)
        for line_index, line in enumerate(lines):
            row = center_row - len(lines) // 2 + line_index
            start = round(glyph.x / cell_width) - len(line) // 2
            if not 0 <= row < rows:
                continue
            for offset, character in enumerate(line):
                column = start + offset
                if 0 <= column < columns and character != " ":
                    grid[row][column] = character
    lines = ["".join(row).rstrip() for row in grid]
    while lines and not lines[-1]:
        lines.pop()
    return "\n".join(lines) + ("\n" if lines else "")


def scrollable(parent: ttk.Frame) -> ttk.Frame:
    """Um painel que rola quando o conteudo passa da altura da janela.

    Sem isto o fim de um formulario longo simplesmente nao existe para quem tem
    a tela menor, e o jeito de descobrir e procurar um botao que nunca aparece.
    Devolve o quadro interno: quem chama empacota dentro dele e esquece o resto.
    """
    holder = ttk.Frame(parent)
    holder.pack(fill=tk.BOTH, expand=True)
    view = tk.Canvas(holder, highlightthickness=0, borderwidth=0)
    bar = ttk.Scrollbar(holder, orient=tk.VERTICAL, command=view.yview)
    inner = ttk.Frame(view)
    window = view.create_window((0, 0), window=inner, anchor="nw")
    view.configure(yscrollcommand=bar.set)
    view.pack(side=tk.LEFT, fill=tk.BOTH, expand=True)

    def fit(_event: tk.Event | None = None) -> None:
        view.configure(scrollregion=view.bbox("all"))
        # A barra so aparece quando ha o que rolar: uma barra morta encostada
        # no formulario e ruido que sugere conteudo escondido onde nao ha.
        needed = inner.winfo_reqheight() > view.winfo_height()
        bar.pack(side=tk.RIGHT, fill=tk.Y) if needed else bar.pack_forget()

    inner.bind("<Configure>", fit)
    view.bind("<Configure>", lambda event: (view.itemconfigure(window, width=event.width), fit()))
    view.bind("<MouseWheel>", lambda event: view.yview_scroll(-event.delta // 120, "units"))
    return inner


class Tip:
    """Balao de ajuda de um botao.

    Existe porque metade dos comandos tem atalho e um terco tem precondicao
    ("selecione uma peca e dois pontos"), e ate agora as duas coisas so
    apareciam depois de errar, na barra de status. Aqui elas ficam onde o
    ponteiro ja esta.
    """

    def __init__(self, widget: tk.Widget, text: str) -> None:
        self.widget, self.text, self.window = widget, text, None
        widget.bind("<Enter>", self.show, add="+")
        widget.bind("<Leave>", self.hide, add="+")
        widget.bind("<ButtonPress>", self.hide, add="+")

    def show(self, _event: tk.Event | None = None) -> None:
        if self.window or not self.text:
            return
        self.window = tk.Toplevel(self.widget)
        self.window.wm_overrideredirect(True)
        self.window.wm_geometry(
            f"+{self.widget.winfo_rootx() + 10}"
            f"+{self.widget.winfo_rooty() + self.widget.winfo_height() + 4}"
        )
        tk.Label(
            self.window,
            text=self.text,
            justify=tk.LEFT,
            background="#ffffe8",
            foreground="#222222",
            relief=tk.SOLID,
            borderwidth=1,
            font=("TkDefaultFont", 8),
            padx=6,
            pady=3,
        ).pack()

    def hide(self, _event: tk.Event | None = None) -> None:
        if self.window:
            self.window.destroy()
            self.window = None


class GlyphForge:
    def __init__(self, root: tk.Tk) -> None:
        self.root = root
        self.root.title(APP_NAME)
        self.root.geometry("1280x860")
        self.root.minsize(980, 660)

        self.canvas_width = 800
        self.canvas_height = 600
        self.background = "#181818"
        self.grid_size = 16
        self.zoom = 1.0
        self.font_path = default_font_path()
        self.elements: list[Glyph] = []
        self.joints: list[Joint] = []
        self.labels: list[SemanticLabel] = []
        self.clips: list[Clip] = []
        self.skins: list[Skin] = []
        self.active_skin_id: str = ""
        self.accent = DEFAULT_ACCENT
        # As pecas sao editadas ja posadas; o repouso fica aqui, para que um
        # quadro guarde a diferenca e nao uma copia da cena.
        self.rest: dict[str, dict] = {}
        self.clip_index: int | None = None
        self.frame_index: int | None = None
        self.playing = False
        self.play_job: str | None = None
        self.selected_label_id: str | None = None
        self.labels_migrated = False
        self.selected_id: str | None = None
        self.selected_joint_id: str | None = None
        self.selected_element_ids: set[str] = set()
        self.selected_joint_ids: set[str] = set()
        self.project_path: Path | None = None
        self.dirty = False
        self.undo_stack: list[dict] = []
        self.redo_stack: list[dict] = []
        self.photos: dict[str, ImageTk.PhotoImage] = {}
        self.drag_offset: tuple[float, float] | None = None
        self.drag_checkpointed = False
        self.marquee_start: tuple[float, float] | None = None
        self.marquee_additive = False
        self.transform_mode: str | None = None
        self.transform_start: dict[str, object] | None = None
        self.transform_checkpointed = False
        self.origin = (30, 30)
        self.loading_notes = False
        self.glyph_palette: tk.Toplevel | None = None
        self.color_picker: tk.Toplevel | None = None
        self.show_rig = tk.BooleanVar(value=True)
        self.show_grid = tk.BooleanVar(value=True)
        self.show_names = tk.BooleanVar(value=True)
        self.snap_to_grid = tk.BooleanVar(value=False)
        self.onion_skin = tk.BooleanVar(value=False)
        self.zoom_text = tk.StringVar(value="Zoom 100%")
        self.frame_text = tk.StringVar(value="repouso")
        self.clip_choice = tk.StringVar(value="(nenhuma)")

        self._build_ui()
        self._bind_shortcuts()
        self.new_project(force=True)
        self.root.protocol("WM_DELETE_WINDOW", self.on_close)

    @staticmethod
    def _group(parent: ttk.Frame, title: str, first: bool = False) -> ttk.Frame:
        """Um grupo de comandos com o nome do que ele faz.

        A barra tinha treze botoes iguais em fila. Agrupar por intencao -- criar,
        editar, ver -- e o que deixa achar um comando sem ler os treze.
        """
        if not first:
            ttk.Separator(parent, orient=tk.VERTICAL).pack(side=tk.LEFT, fill=tk.Y, padx=7)
        holder = ttk.Frame(parent)
        holder.pack(side=tk.LEFT)
        ttk.Label(holder, text=title.upper(), foreground="#8a8a8a", font=("TkDefaultFont", 7)).pack(
            anchor=tk.W
        )
        row = ttk.Frame(holder)
        row.pack(anchor=tk.W)
        return row

    @staticmethod
    def _tool(parent: ttk.Frame, label: str, command, hint: str, width: int = 0) -> ttk.Button:
        button = ttk.Button(parent, text=label, command=command)
        if width:
            button.configure(width=width)
        button.pack(side=tk.LEFT, padx=1)
        Tip(button, hint)
        return button

    def _build_ui(self) -> None:
        toolbar = ttk.Frame(self.root, padding=(6, 5))
        toolbar.pack(fill=tk.X)
        top = ttk.Frame(toolbar)
        top.pack(fill=tk.X, pady=(0, 5))
        bottom = ttk.Frame(toolbar)
        bottom.pack(fill=tk.X)

        # Um grupo so para o arquivo: salvar, trazer e entregar sao a mesma
        # pergunta -- o que acontece com este projeto. Estavam em tres grupos,
        # com dois botoes chamados "Exportar" em lugares diferentes, e escolher
        # entre eles exigia lembrar qual dos dois fazia o que.
        arquivo = self._group(top, "arquivo", first=True)
        for label, command, hint in (
            ("Novo", self.new_project, "Comeca um projeto vazio  (Ctrl+N)"),
            ("Abrir", self.open_project, "Abre um .glyph.json  (Ctrl+O)"),
            ("Salvar", self.save_project, "Salva por cima  (Ctrl+S)"),
            ("Salvar como", self.save_project_as, "Salva uma copia e passa a trabalhar nela  (Ctrl+Shift+S)"),
        ):
            self._tool(arquivo, label, command, hint)
        ttk.Separator(arquivo, orient=tk.VERTICAL).pack(side=tk.LEFT, fill=tk.Y, padx=5)
        self._tool(
            arquivo,
            "Importar",
            self.import_piece,
            "Traz outra cena, peca ou .txt para dentro desta.\nSo a geometria: animacoes da peca ficam no arquivo dela.",
        )
        # Um botao de exportar, com as duas saidas dentro dele: quem exporta ja
        # sabe se quer o pacote ou a peca, e ve as duas escolhas lado a lado no
        # momento de escolher, em vez de decorar dois botoes distantes.
        export = ttk.Menubutton(arquivo, text="Exportar")
        menu = tk.Menu(export, tearoff=0)
        menu.add_command(
            label="Tudo: pacote com previews e GIFs...", command=self.export_bundle
        )
        menu.add_command(
            label="So a selecao: peca reaproveitavel...", command=self.export_selection
        )
        export["menu"] = menu
        export.pack(side=tk.LEFT, padx=1)
        Tip(export, "O pacote inteiro (Ctrl+E) ou so a peca selecionada")
        self._tool(
            arquivo,
            "Conferir",
            self.check_scene,
            "Procura o que o jogo nao vai conseguir ler:\nnome repetido, peca solta, rotulo vazio",
        )

        criar = self._group(bottom, "criar", first=True)
        for label, command, hint in (
            ("Glyph", self.add_glyph, "Um caractere novo no centro do canvas"),
            ("Ponto", self.add_joint, "Uma articulacao. Com duas pecas selecionadas,\nja nasce ligando as duas"),
            ("Atencao", self.add_attention_point, "Um destaque nomeado: pegada, boca do cano, ponta"),
            ("Rotulo", self.create_label_from_selection, "Nomeia o conjunto selecionado.\nE por este nome que o jogo acha a peca"),
        ):
            self._tool(criar, label, command, hint)

        editar = self._group(bottom, "editar")
        for label, command, hint in (
            ("↶", self.undo, "Desfazer  (Ctrl+Z)"),
            ("↷", self.redo, "Refazer  (Ctrl+Y)"),
        ):
            self._tool(editar, label, command, hint, width=3)
        for label, command, hint in (
            ("Copiar", self.copy_selected, "Copiar  (Ctrl+C)"),
            ("Colar", self.paste_clipboard, "Colar  (Ctrl+V)"),
            ("Duplicar", self.duplicate_selected, "Duplicar no lugar  (Ctrl+D)"),
            ("Excluir", self.delete_selected, "Excluir  (Delete)"),
        ):
            self._tool(editar, label, command, hint)

        ver = self._group(bottom, "ver")
        for label, variable, hint in (
            ("Rig", self.show_rig, "Mostra pontos e ligacoes por cima do desenho"),
            (
                "Nomes",
                self.show_names,
                "Mostra o nome de cada ponto.\nDesligue quando os nomes se sobrepuserem --\no nome do ponto selecionado continua aparecendo.",
            ),
            ("Grade", self.show_grid, "Mostra a grade do projeto"),
            ("Snap", self.snap_to_grid, "Prende o que voce arrasta a grade"),
        ):
            check = ttk.Checkbutton(
                ver,
                text=label,
                variable=variable,
                command=self.redraw if variable is not self.snap_to_grid else None,
            )
            check.pack(side=tk.LEFT, padx=2)
            Tip(check, hint)
        zoom = ttk.Label(ver, textvariable=self.zoom_text, foreground="#666666")
        zoom.pack(side=tk.LEFT, padx=(8, 2))
        Tip(zoom, "Ctrl+scroll aplica zoom sob o cursor; Ctrl+0 volta a 100%")

        body = ttk.Panedwindow(self.root, orient=tk.HORIZONTAL)
        body.pack(fill=tk.BOTH, expand=True)

        canvas_frame = ttk.Frame(body)
        inspector = ttk.Frame(body, width=280, padding=10)
        body.add(canvas_frame, weight=1)
        body.add(inspector, weight=0)

        self.canvas = tk.Canvas(canvas_frame, bg="#303030", highlightthickness=0)
        x_scroll = ttk.Scrollbar(canvas_frame, orient=tk.HORIZONTAL, command=self.canvas.xview)
        y_scroll = ttk.Scrollbar(canvas_frame, orient=tk.VERTICAL, command=self.canvas.yview)
        self.canvas.configure(xscrollcommand=x_scroll.set, yscrollcommand=y_scroll.set)
        self.canvas.grid(row=0, column=0, sticky="nsew")
        y_scroll.grid(row=0, column=1, sticky="ns")
        x_scroll.grid(row=1, column=0, sticky="ew")
        canvas_frame.rowconfigure(0, weight=1)
        canvas_frame.columnconfigure(0, weight=1)

        # Barra de transporte: fica sob o canvas, e nao dentro da aba, porque e
        # olhando o desenho que se decide se o quadro esta certo.
        transport = ttk.Frame(canvas_frame, padding=(4, 5))
        transport.grid(row=2, column=0, columnspan=2, sticky="ew")
        ttk.Label(transport, text="Animacao").pack(side=tk.LEFT, padx=(0, 4))
        self.clip_combo = ttk.Combobox(
            transport, textvariable=self.clip_choice, state="readonly", width=18
        )
        self.clip_combo.pack(side=tk.LEFT, padx=2)
        self.clip_combo.bind("<<ComboboxSelected>>", self.on_clip_chosen)
        Tip(self.clip_combo, "Qual animacao esta aberta")
        self._tool(transport, "<", lambda: self.step_frame(-1), "Quadro anterior  (,)", width=3)
        self.play_button = ttk.Button(transport, text="▶ Tocar", width=9, command=self.toggle_play)
        self.play_button.pack(side=tk.LEFT, padx=1)
        Tip(self.play_button, "Toca a animacao no proprio canvas  (espaco)")
        self._tool(transport, ">", lambda: self.step_frame(1), "Proximo quadro  (.)", width=3)
        self._tool(
            transport,
            "+ Quadro",
            self.add_frame,
            "Grava a pose que esta na tela como um quadro novo,\ndepois do quadro atual",
        )
        onion = ttk.Checkbutton(
            transport, text="Sombra", variable=self.onion_skin, command=self.redraw
        )
        onion.pack(side=tk.LEFT, padx=(8, 2))
        Tip(onion, "Desenha o quadro anterior esmaecido atras do atual")
        ttk.Label(transport, textvariable=self.frame_text, foreground="#666666").pack(
            side=tk.RIGHT, padx=8
        )

        # Tira de quadros: e o mapa da animacao. Numa lista vertical na aba, ir
        # do quadro 2 ao 5 e procurar uma linha; aqui e um clique num numero,
        # sem tirar os olhos do desenho.
        self.strip = ttk.Frame(canvas_frame, padding=(4, 0, 4, 5))
        self.strip.grid(row=3, column=0, columnspan=2, sticky="ew")
        self.strip_buttons: list[tk.Widget] = []

        self.canvas.bind("<Button-1>", self.on_canvas_press)
        self.canvas.bind("<B1-Motion>", self.on_canvas_drag)
        self.canvas.bind("<ButtonRelease-1>", self.on_canvas_release)
        self.canvas.bind("<Motion>", self.on_canvas_motion)
        self.canvas.bind(
            "<Leave>",
            lambda _event: self.canvas.configure(cursor="") if not self.transform_mode else None,
        )
        self.canvas.bind("<Control-MouseWheel>", self.on_zoom_wheel)
        self.canvas.bind("<Control-Button-4>", lambda event: self.change_zoom(event, 1))
        self.canvas.bind("<Control-Button-5>", lambda event: self.change_zoom(event, -1))
        # Arrastar com o botao do meio empurra a vista, e nao as pecas: com o
        # zoom em 700% a barra de rolagem anda a cena inteira num arrasto, e a
        # unica forma de chegar num canto era acertar a barra no pixel certo.
        self.canvas.bind("<Button-2>", self.on_pan_press)
        self.canvas.bind("<B2-Motion>", self.on_pan_drag)
        self.canvas.bind("<ButtonRelease-2>", self.on_pan_release)
        # Roda sem modificador rola; com Shift, rola de lado. O zoom continua no
        # Ctrl+roda, que e onde todo editor o coloca.
        self.canvas.bind("<MouseWheel>", lambda event: self.canvas.yview_scroll(-event.delta // 120, "units"))
        self.canvas.bind(
            "<Shift-MouseWheel>",
            lambda event: self.canvas.xview_scroll(-event.delta // 120, "units"),
        )

        self._build_inspector(inspector)

        self.status = tk.StringVar(value="Pronto")
        ttk.Label(self.root, textvariable=self.status, anchor=tk.W, padding=(8, 3)).pack(fill=tk.X)

    def _field(self, parent: ttk.Frame, row: int, column: int, key: str, width: int = 10) -> ttk.Entry:
        """Um campo que aplica ao sair dele, e nao so no Enter.

        Digitar um numero e clicar em outro lugar tem que valer: exigir Enter e
        um contrato invisivel, e o modo de falha e o pior possivel -- o valor
        fica na tela, parecendo aplicado, e some no proximo redesenho.
        """
        entry = ttk.Entry(parent, textvariable=self.vars[key], width=width)
        entry.grid(row=row, column=column, sticky=tk.EW, pady=2)
        entry.bind("<Return>", lambda _event: self.apply_properties())
        entry.bind("<FocusOut>", lambda _event: self.apply_properties())
        return entry

    def _build_inspector(self, parent: ttk.Frame) -> None:
        self.inspector_tabs = ttk.Notebook(parent)
        self.inspector_tabs.pack(fill=tk.BOTH, expand=True)

        # Uma aba so para o que esta selecionado, e nao uma por tipo de objeto.
        #
        # Eram tres -- Glyph, Rig, Atencao -- e o editor pulava sozinho entre
        # elas a cada clique. Isso e a confissao de que sempre foram um painel
        # so: nunca ha dois tipos selecionados como peca principal. E o pulo
        # atrapalhava justamente quem anima, que clica em ponto o tempo todo e
        # era arrancado da lista de quadros a cada clique.
        outline_tab = ttk.Frame(self.inspector_tabs, padding=(8, 8))
        piece_holder = ttk.Frame(self.inspector_tabs, padding=(10, 8))
        piece_tab = scrollable(piece_holder)
        self.piece_hint = ttk.Label(
            piece_tab, text="", foreground="#666666", wraplength=240, justify=tk.LEFT
        )
        self.piece_hint.pack(fill=tk.X, pady=(0, 4))
        self.piece_forms = ttk.Frame(piece_tab)
        self.piece_forms.pack(fill=tk.BOTH, expand=True)
        self.glyph_form = ttk.Frame(self.piece_forms)
        self.rig_tab = ttk.Frame(self.piece_forms)
        self.attention_tab = ttk.Frame(self.piece_forms)
        glyph_tab = self.glyph_form

        self.label_tab = ttk.Frame(self.inspector_tabs, padding=10)
        animation_holder = ttk.Frame(self.inspector_tabs, padding=(10, 8))
        self.animation_tab = scrollable(animation_holder)
        self.skin_tab = ttk.Frame(self.inspector_tabs, padding=10)
        project_tab = ttk.Frame(self.inspector_tabs, padding=10)
        self.inspector_tabs.add(outline_tab, text="Objetos")
        self.inspector_tabs.add(piece_holder, text="Peca")
        self.inspector_tabs.add(animation_holder, text="Animacao")
        self.inspector_tabs.add(self.label_tab, text="Rotulos")
        self.inspector_tabs.add(self.skin_tab, text="Peles")
        self.inspector_tabs.add(project_tab, text="Projeto")
        self._build_outline_tab(outline_tab)

        form = ttk.Frame(glyph_tab)
        form.pack(fill=tk.X, pady=(2, 10))

        self.vars = {
            "glyph": tk.StringVar(),
            "x": tk.DoubleVar(),
            "y": tk.DoubleVar(),
            "font_size": tk.IntVar(value=64),
            "scale_x": tk.DoubleVar(value=1),
            "scale_y": tk.DoubleVar(value=1),
            "flip_x": tk.BooleanVar(value=False),
            "flip_y": tk.BooleanVar(value=False),
            "rotation": tk.DoubleVar(),
            "layer": tk.IntVar(),
            "font_path": tk.StringVar(value=self.font_path),
            "role": tk.StringVar(value="(cor propria)"),
        }
        form.destroy()

        # Tres secoes, e nao dezesseis controles em fila: o que a peca *e*, onde
        # ela *esta*, e como ela se prende ao rig. Sao as tres perguntas que se
        # faz olhando uma peca, e antes disto respondia-se as tres varrendo a
        # mesma lista de cima a baixo.
        drawing = ttk.LabelFrame(glyph_tab, text=" Desenho ", padding=(8, 6))
        drawing.pack(fill=tk.X, pady=(2, 8))
        ttk.Label(drawing, text="Glyph / texto").grid(row=0, column=0, sticky=tk.W, pady=2)
        self._field(drawing, 0, 1, "glyph", width=12)
        ttk.Label(drawing, text="Cor").grid(row=1, column=0, sticky=tk.W, pady=2)
        self.color_button = tk.Button(
            drawing, text="      ", command=self.choose_color, relief=tk.RAISED
        )
        self.color_button.grid(row=1, column=1, sticky=tk.E, pady=2)
        drawing.columnconfigure(1, weight=1)
        table = ttk.Button(drawing, text="Tabela CP437...", command=self.open_glyph_table)
        table.grid(row=2, column=0, columnspan=2, sticky=tk.EW, pady=(6, 2))
        Tip(table, "Escolhe visualmente entre os 256 glifos da fonte do jogo")
        font_button = ttk.Button(drawing, text="Outra fonte...", command=self.choose_font)
        font_button.grid(row=3, column=0, columnspan=2, sticky=tk.EW, pady=1)
        self.font_label = ttk.Label(drawing, text="Fonte padrao", foreground="#666666")
        self.font_label.grid(row=4, column=0, columnspan=2, sticky=tk.W)

        transform = ttk.LabelFrame(glyph_tab, text=" Transformar ", padding=(8, 6))
        transform.pack(fill=tk.X, pady=(0, 8))
        pairs = (
            (("X", "x"), ("Y", "y")),
            (("Escala X", "scale_x"), ("Escala Y", "scale_y")),
            (("Rotacao", "rotation"), ("Camada", "layer")),
            (("Tamanho", "font_size"), None),
        )
        for row, (left, right) in enumerate(pairs):
            for column, pair in ((0, left), (2, right)):
                if pair is None:
                    continue
                label, key = pair
                ttk.Label(transform, text=label).grid(row=row, column=column, sticky=tk.W, pady=2)
                self._field(transform, row, column + 1, key, width=7)
        for column in (1, 3):
            transform.columnconfigure(column, weight=1)
        mirror_row = ttk.Frame(transform)
        mirror_row.grid(row=4, column=0, columnspan=4, sticky=tk.EW, pady=(6, 0))
        ttk.Checkbutton(
            mirror_row, text="Espelhar X", variable=self.vars["flip_x"], command=self.apply_properties
        ).pack(side=tk.LEFT)
        ttk.Checkbutton(
            mirror_row, text="Espelhar Y", variable=self.vars["flip_y"], command=self.apply_properties
        ).pack(side=tk.RIGHT)

        glyph_tab = ttk.LabelFrame(self.glyph_form, text=" No rig ", padding=(8, 6))
        glyph_tab.pack(fill=tk.X)
        role_row = ttk.Frame(glyph_tab)
        role_row.pack(fill=tk.X, pady=2)
        ttk.Label(role_row, text="Papel da peca").pack(side=tk.LEFT)
        # Editavel, e nao uma lista fechada: `corpo` e `membro` sao o que a pele
        # entende, mas o jogo le o papel para saber que peca se mexe sozinha --
        # ferrolho, alca, bomba. Fechar a lista aqui obrigaria a inventar um
        # papel novo no editor toda vez que uma arma ganhasse uma peca movel.
        self.role_combo = ttk.Combobox(
            role_row,
            textvariable=self.vars["role"],
            values=("(cor propria)", "corpo", "membro", "bolt", "sight", "pump", "muzzle"),
            width=14,
        )
        self.role_combo.pack(side=tk.RIGHT)
        self.role_combo.bind("<<ComboboxSelected>>", lambda _event: self.apply_properties())
        self.role_combo.bind("<Return>", lambda _event: self.apply_properties())
        self.role_combo.bind("<FocusOut>", lambda _event: self.apply_properties())

        self.span_button = ttk.Button(
            glyph_tab, text="Virar segmento entre 2 pontos", command=self.link_span
        )
        self.span_button.pack(fill=tk.X, pady=(6, 0))
        Tip(
            self.span_button,
            "Selecione a peca e dois pontos (Shift+clique).\n"
            "A peca passa a se esticar entre eles: arrastar um ponto dobra o membro.\n"
            "Sem nenhum ponto selecionado, desfaz.",
        )
        self.carry_button = ttk.Button(
            glyph_tab, text="Prender a um ponto", command=self.carry_element
        )
        self.carry_button.pack(fill=tk.X, pady=(2, 0))
        Tip(
            self.carry_button,
            "Selecione a peca e um ponto (Shift+clique).\n"
            "A peca passa a ser carregada por ele -- e assim que a arma fica na mao.\n"
            "Sem nenhum ponto selecionado, solta.",
        )
        self.span_label = ttk.Label(glyph_tab, text="", foreground="#666666", wraplength=240)
        self.span_label.pack(fill=tk.X, pady=(4, 0))

        ttk.Label(self.rig_tab, text="Ponto de articulacao", font=("TkDefaultFont", 11, "bold")).pack(
            anchor=tk.W
        )
        rig_form = ttk.Frame(self.rig_tab)
        rig_form.pack(fill=tk.X, pady=(6, 8))
        self.joint_vars = {
            "name": tk.StringVar(),
            "x": tk.DoubleVar(),
            "y": tk.DoubleVar(),
            "parent": tk.StringVar(value="Nenhuma"),
            "part_a": tk.StringVar(value="Nenhum"),
            "part_b": tk.StringVar(value="Nenhum"),
            "constraint": tk.StringVar(value="Pivo (permite girar)"),
            "fixed": tk.BooleanVar(value=False),
        }
        for row, (label, key) in enumerate((("Nome", "name"), ("X", "x"), ("Y", "y"))):
            ttk.Label(rig_form, text=label).grid(row=row, column=0, sticky=tk.W, pady=2)
            entry = ttk.Entry(rig_form, textvariable=self.joint_vars[key], width=18)
            entry.grid(row=row, column=1, sticky=tk.EW, pady=2)
            entry.bind("<Return>", lambda _event: self.apply_joint_properties())
        ttk.Label(rig_form, text="Pai / osso").grid(row=3, column=0, sticky=tk.W, pady=2)
        self.parent_combo = ttk.Combobox(
            rig_form, textvariable=self.joint_vars["parent"], state="readonly", width=17
        )
        self.parent_combo.grid(row=3, column=1, sticky=tk.EW, pady=2)
        self.parent_combo.configure(postcommand=self.refresh_rig_choices)
        ttk.Label(rig_form, text="Peca A").grid(row=4, column=0, sticky=tk.W, pady=2)
        self.part_a_combo = ttk.Combobox(
            rig_form, textvariable=self.joint_vars["part_a"], state="readonly", width=17
        )
        self.part_a_combo.grid(row=4, column=1, sticky=tk.EW, pady=2)
        self.part_a_combo.configure(postcommand=self.refresh_rig_choices)
        ttk.Label(rig_form, text="Peca B").grid(row=5, column=0, sticky=tk.W, pady=2)
        self.part_b_combo = ttk.Combobox(
            rig_form, textvariable=self.joint_vars["part_b"], state="readonly", width=17
        )
        self.part_b_combo.grid(row=5, column=1, sticky=tk.EW, pady=2)
        self.part_b_combo.configure(postcommand=self.refresh_rig_choices)
        ttk.Label(rig_form, text="Comportamento").grid(row=6, column=0, sticky=tk.W, pady=2)
        self.constraint_combo = ttk.Combobox(
            rig_form,
            textvariable=self.joint_vars["constraint"],
            state="readonly",
            values=("Pivo (permite girar)", "Fixa (solda as pecas)"),
            width=17,
        )
        self.constraint_combo.grid(row=6, column=1, sticky=tk.EW, pady=2)
        ttk.Checkbutton(
            rig_form,
            text="Ancora fixa no mundo",
            variable=self.joint_vars["fixed"],
        ).grid(row=7, column=0, columnspan=2, sticky=tk.W, pady=5)
        rig_form.columnconfigure(1, weight=1)

        joint_color_row = ttk.Frame(self.rig_tab)
        joint_color_row.pack(fill=tk.X, pady=2)
        ttk.Label(joint_color_row, text="Cor do ponto").pack(side=tk.LEFT)
        self.joint_color_button = tk.Button(
            joint_color_row, text="      ", bg="#ffcc33", command=self.choose_joint_color
        )
        self.joint_color_button.pack(side=tk.RIGHT)
        ttk.Label(self.rig_tab, text="Descricao opcional").pack(anchor=tk.W, pady=(6, 2))
        self.joint_description = tk.Text(self.rig_tab, height=4, width=28, wrap=tk.WORD)
        self.joint_description.pack(fill=tk.X)
        self.joint_description.bind("<FocusOut>", lambda _event: self.apply_joint_properties())
        ttk.Button(self.rig_tab, text="Aplicar articulacao", command=self.apply_joint_properties).pack(
            fill=tk.X, pady=(8, 10)
        )
        ttk.Label(
            self.rig_tab,
            text=(
                "A articulacao e independente: ela conecta Peca A e Peca B naquele ponto. Pivo permite "
                "giro relativo; Fixa solda as pecas. Ancora fixa prende o ponto ao mundo."
            ),
            wraplength=250,
            foreground="#666666",
        ).pack(fill=tk.X, pady=4)

        ttk.Label(
            self.attention_tab, text="Ponto de atencao", font=("TkDefaultFont", 11, "bold")
        ).pack(anchor=tk.W)
        attention_form = ttk.Frame(self.attention_tab)
        attention_form.pack(fill=tk.X, pady=(6, 8))
        self.attention_vars = {
            "name": tk.StringVar(),
            "x": tk.DoubleVar(),
            "y": tk.DoubleVar(),
            "attachment": tk.StringVar(value="Nenhum"),
        }
        for row, (label, key) in enumerate((("Nome", "name"), ("X", "x"), ("Y", "y"))):
            ttk.Label(attention_form, text=label).grid(row=row, column=0, sticky=tk.W, pady=2)
            entry = ttk.Entry(attention_form, textvariable=self.attention_vars[key], width=18)
            entry.grid(row=row, column=1, sticky=tk.EW, pady=2)
            entry.bind("<Return>", lambda _event: self.apply_attention_properties())
        ttk.Label(attention_form, text="Fixado ao glyph").grid(row=3, column=0, sticky=tk.W, pady=2)
        self.attention_attachment_combo = ttk.Combobox(
            attention_form,
            textvariable=self.attention_vars["attachment"],
            state="readonly",
            width=17,
        )
        self.attention_attachment_combo.grid(row=3, column=1, sticky=tk.EW, pady=2)
        self.attention_attachment_combo.configure(postcommand=self.refresh_rig_choices)
        attention_form.columnconfigure(1, weight=1)
        attention_color_row = ttk.Frame(self.attention_tab)
        attention_color_row.pack(fill=tk.X, pady=4)
        ttk.Label(attention_color_row, text="Cor do destaque").pack(side=tk.LEFT)
        self.attention_color_button = tk.Button(
            attention_color_row,
            text="      ",
            bg="#ff4dc4",
            command=self.choose_attention_color,
        )
        self.attention_color_button.pack(side=tk.RIGHT)
        ttk.Label(self.attention_tab, text="Descricao opcional").pack(anchor=tk.W, pady=(6, 2))
        self.attention_description = tk.Text(self.attention_tab, height=8, width=28, wrap=tk.WORD)
        self.attention_description.pack(fill=tk.BOTH, expand=True)
        self.attention_description.bind(
            "<FocusOut>", lambda _event: self.apply_attention_properties()
        )
        ttk.Button(
            self.attention_tab,
            text="Aplicar ponto de atencao",
            command=self.apply_attention_properties,
        ).pack(fill=tk.X, pady=(8, 8))
        ttk.Label(
            self.attention_tab,
            text="Use para destacar uma parte importante para a LLM sem criar um osso no rig.",
            wraplength=250,
            foreground="#666666",
        ).pack(fill=tk.X)

        ttk.Label(
            self.label_tab, text="Rotulos semanticos", font=("TkDefaultFont", 11, "bold")
        ).pack(anchor=tk.W)
        self.label_list = tk.Listbox(self.label_tab, height=7, exportselection=False)
        self.label_list.pack(fill=tk.X, pady=(6, 6))
        self.label_list.bind("<<ListboxSelect>>", self.on_label_selected)
        label_buttons = ttk.Frame(self.label_tab)
        label_buttons.pack(fill=tk.X)
        ttk.Button(
            label_buttons, text="Novo da selecao", command=self.create_label_from_selection
        ).pack(side=tk.LEFT, expand=True, fill=tk.X, padx=(0, 2))
        ttk.Button(label_buttons, text="Excluir", command=self.delete_selected_label).pack(
            side=tk.LEFT, expand=True, fill=tk.X, padx=(2, 0)
        )
        self.label_name_var = tk.StringVar()
        self.label_count_var = tk.StringVar(value="Nenhum rotulo selecionado")
        ttk.Label(self.label_tab, text="Nome").pack(anchor=tk.W, pady=(10, 2))
        label_name_entry = ttk.Entry(self.label_tab, textvariable=self.label_name_var)
        label_name_entry.pack(fill=tk.X)
        label_name_entry.bind("<Return>", lambda _event: self.apply_label_properties())
        ttk.Label(self.label_tab, textvariable=self.label_count_var, foreground="#666666").pack(
            anchor=tk.W, pady=(4, 6)
        )
        ttk.Label(self.label_tab, text="Sub-rotulos (conjuntos dentro deste)").pack(anchor=tk.W)
        self.label_children_list = tk.Listbox(
            self.label_tab,
            height=5,
            selectmode=tk.MULTIPLE,
            exportselection=False,
        )
        self.label_children_list.pack(fill=tk.X, pady=(2, 6))
        ttk.Label(self.label_tab, text="Descricao opcional").pack(anchor=tk.W)
        self.label_description = tk.Text(self.label_tab, height=5, width=28, wrap=tk.WORD)
        self.label_description.pack(fill=tk.BOTH, expand=True, pady=(2, 6))
        self.label_description.bind("<FocusOut>", lambda _event: self.apply_label_properties())
        ttk.Button(
            self.label_tab, text="Aplicar rotulo", command=self.apply_label_properties
        ).pack(fill=tk.X)
        ttk.Button(
            self.label_tab, text="Selecionar membros", command=self.select_label_members
        ).pack(fill=tk.X, pady=(4, 0))
        ttk.Button(
            self.label_tab, text="Usar selecao atual", command=self.update_label_members
        ).pack(fill=tk.X, pady=(4, 0))

        ttk.Label(project_tab, text="Canvas", font=("TkDefaultFont", 11, "bold")).pack(
            anchor=tk.W, pady=(4, 2)
        )
        canvas_form = ttk.Frame(project_tab)
        canvas_form.pack(fill=tk.X)
        self.width_var = tk.IntVar(value=self.canvas_width)
        self.height_var = tk.IntVar(value=self.canvas_height)
        self.grid_var = tk.IntVar(value=self.grid_size)
        for row, (label, variable) in enumerate(
            (("Largura", self.width_var), ("Altura", self.height_var), ("Grade", self.grid_var))
        ):
            ttk.Label(canvas_form, text=label).grid(row=row, column=0, sticky=tk.W, pady=2)
            ttk.Entry(canvas_form, textvariable=variable, width=12).grid(row=row, column=1, sticky=tk.E, pady=2)
        canvas_form.columnconfigure(1, weight=1)
        bg_row = ttk.Frame(project_tab)
        bg_row.pack(fill=tk.X, pady=4)
        ttk.Label(bg_row, text="Fundo").pack(side=tk.LEFT)
        self.background_button = tk.Button(bg_row, text="      ", command=self.choose_background)
        self.background_button.pack(side=tk.RIGHT)
        accent_row = ttk.Frame(project_tab)
        accent_row.pack(fill=tk.X, pady=4)
        ttk.Label(accent_row, text="Acento (cor de quem veste)").pack(side=tk.LEFT)
        self.accent_button = tk.Button(
            accent_row, text="      ", command=lambda: self.open_live_color_picker("accent")
        )
        self.accent_button.pack(side=tk.RIGHT)
        ttk.Button(project_tab, text="Aplicar canvas", command=self.apply_canvas).pack(
            fill=tk.X, pady=(4, 10)
        )

        ttk.Label(project_tab, text="Notas para a LLM", font=("TkDefaultFont", 11, "bold")).pack(
            anchor=tk.W
        )
        self.notes = tk.Text(project_tab, height=7, width=30, wrap=tk.WORD)
        self.notes.pack(fill=tk.BOTH, expand=True, pady=(4, 0))
        self.notes.bind("<<Modified>>", self.on_notes_modified)

        self._build_animation_tab(self.animation_tab)
        self._build_skin_tab(self.skin_tab)

    def _build_outline_tab(self, parent: ttk.Frame) -> None:
        """A lista do que existe na cena.

        Ate agora, achar uma peca era cacar no canvas: com vinte e cinco pecas
        sobrepostas num boneco de 64px, algumas so davam para pegar por
        tentativa. Uma lista com filtro resolve a pergunta "cade o cotovelo de
        tras" em duas teclas.
        """
        top = ttk.Frame(parent)
        top.pack(fill=tk.X, pady=(0, 6))
        ttk.Label(top, text="Filtrar").pack(side=tk.LEFT)
        self.outline_filter = tk.StringVar()
        entry = ttk.Entry(top, textvariable=self.outline_filter)
        entry.pack(side=tk.LEFT, fill=tk.X, expand=True, padx=(4, 0))
        self.outline_filter.trace_add("write", lambda *_: self.sync_outline())
        Tip(entry, "Filtra por nome, id, glifo ou papel")

        self.outline = ttk.Treeview(parent, columns=("what",), show="tree headings", height=18)
        self.outline.heading("#0", text="Peca")
        self.outline.heading("what", text="O que e")
        self.outline.column("#0", width=150, stretch=True)
        self.outline.column("what", width=90, stretch=False)
        bar = ttk.Scrollbar(parent, orient=tk.VERTICAL, command=self.outline.yview)
        self.outline.configure(yscrollcommand=bar.set)
        self.outline.pack(side=tk.LEFT, fill=tk.BOTH, expand=True)
        bar.pack(side=tk.RIGHT, fill=tk.Y)
        self.outline.bind("<<TreeviewSelect>>", self.on_outline_selected)
        self.syncing_outline = False

    def sync_outline(self) -> None:
        if not hasattr(self, "outline") or self.syncing_outline:
            return
        self.syncing_outline = True
        try:
            self.outline.delete(*self.outline.get_children())
            needle = self.outline_filter.get().strip().lower()

            def matches(*fields: str) -> bool:
                return not needle or any(needle in field.lower() for field in fields if field)

            groups = {}
            for kind, title in (
                ("glyph", "Glyphs"),
                ("joint", "Pontos"),
                ("attention", "Atencao"),
                ("label", "Rotulos"),
            ):
                groups[kind] = self.outline.insert("", tk.END, text=title, open=True, tags=("group",))

            counts = dict.fromkeys(groups, 0)
            for element in self.elements:
                role = element.role or ("segmento" if element.span else "")
                if element.follow:
                    role = role or "preso"
                shown = (element.glyph or " ").splitlines()[0][:8]
                if not matches(element.id, shown, element.role):
                    continue
                self.outline.insert(
                    groups["glyph"],
                    tk.END,
                    iid=f"e:{element.id}",
                    text=f"{shown or '·'}   {element.id}",
                    values=(role or "livre",),
                )
                counts["glyph"] += 1
            for joint in self.joints:
                kind = "attention" if joint.kind == "attention" else "joint"
                if not matches(joint.name, joint.id):
                    continue
                carried = sum(
                    1
                    for element in self.elements
                    if element.follow == joint.id or joint.id in element.span
                )
                self.outline.insert(
                    groups[kind],
                    tk.END,
                    iid=f"j:{joint.id}",
                    text=joint.name,
                    values=(f"{carried} peca(s)" if carried else "solto",),
                )
                counts[kind] += 1
            labels_by_id = {label.id: label for label in self.labels}
            for label in self.labels:
                if not matches(label.name, label.id):
                    continue
                total = len(resolved_label_elements(label.id, labels_by_id))
                self.outline.insert(
                    groups["label"],
                    tk.END,
                    iid=f"l:{label.id}",
                    text=label.name,
                    values=(f"{total} glyph(s)",),
                )
                counts["label"] += 1

            for kind, node in groups.items():
                title = self.outline.item(node, "text")
                self.outline.item(node, text=f"{title}  ({counts[kind]})")
                if not counts[kind]:
                    self.outline.delete(node)

            chosen = [f"e:{item}" for item in self.selected_element_ids]
            chosen += [f"j:{item}" for item in self.selected_joint_ids]
            present = [item for item in chosen if self.outline.exists(item)]
            if present:
                self.outline.selection_set(present)
                self.outline.see(present[0])
        finally:
            self.syncing_outline = False

    def on_outline_selected(self, _event: tk.Event | None = None) -> None:
        if self.syncing_outline:
            return
        chosen = self.outline.selection()
        elements = {item[2:] for item in chosen if item.startswith("e:")}
        joints = {item[2:] for item in chosen if item.startswith("j:")}
        labels = [item[2:] for item in chosen if item.startswith("l:")]
        # A propria sincronizacao marca linhas na arvore, e marcar dispara este
        # callback de volta. Sair quando nada mudou fecha o ciclo sem depender
        # de o Tk entregar o evento antes ou depois da flag.
        if elements == self.selected_element_ids and joints == self.selected_joint_ids:
            return
        if labels:
            self.selected_label_id = labels[0]
            self.select_label_members()
            return
        if not elements and not joints:
            return
        self.clear_selection()
        self.selected_element_ids = elements
        self.selected_joint_ids = joints
        self.selected_id = next(iter(elements), None)
        self.selected_joint_id = next(iter(joints), None)
        if self.selected_joint_id:
            self.selected_id = None
        self.draw_selection()
        self.sync_inspector()
        self.update_selection_status()

    def _build_animation_tab(self, holder: ttk.Frame) -> None:
        parent = ttk.LabelFrame(holder, text=" Animacoes ", padding=(8, 6))
        parent.pack(fill=tk.X, pady=(2, 8))
        self.clip_list = tk.Listbox(parent, height=5, exportselection=False)
        self.clip_list.pack(fill=tk.X, pady=(0, 4))
        self.clip_list.bind("<<ListboxSelect>>", self.on_clip_selected)
        clip_buttons = ttk.Frame(parent)
        clip_buttons.pack(fill=tk.X)
        for label, command in (
            ("Nova", self.add_clip),
            ("Duplicar", self.duplicate_clip),
            ("Excluir", self.delete_clip),
        ):
            ttk.Button(clip_buttons, text=label, command=command).pack(
                side=tk.LEFT, expand=True, fill=tk.X, padx=1
            )

        clip_form = ttk.Frame(parent)
        clip_form.pack(fill=tk.X, pady=(8, 4))
        self.clip_vars = {
            "name": tk.StringVar(),
            "fps": tk.DoubleVar(value=8.0),
            "loop": tk.BooleanVar(value=True),
        }
        for row, (label, key) in enumerate((("Nome", "name"), ("Quadros por segundo", "fps"))):
            ttk.Label(clip_form, text=label).grid(row=row, column=0, sticky=tk.W, pady=2)
            entry = ttk.Entry(clip_form, textvariable=self.clip_vars[key], width=12)
            entry.grid(row=row, column=1, sticky=tk.EW, pady=2)
            entry.bind("<Return>", lambda _event: self.apply_clip_properties())
            entry.bind("<FocusOut>", lambda _event: self.apply_clip_properties())
        ttk.Checkbutton(
            clip_form,
            text="Repetir em ciclo",
            variable=self.clip_vars["loop"],
            command=self.apply_clip_properties,
        ).grid(row=2, column=0, columnspan=2, sticky=tk.W, pady=2)
        clip_form.columnconfigure(1, weight=1)

        parent = ttk.LabelFrame(holder, text=" Quadros ", padding=(8, 6))
        parent.pack(fill=tk.X)
        self.frame_list = tk.Listbox(parent, height=7, exportselection=False)
        self.frame_list.pack(fill=tk.X, pady=(0, 4))
        self.frame_list.bind("<<ListboxSelect>>", self.on_frame_selected)
        frame_buttons = ttk.Frame(parent)
        frame_buttons.pack(fill=tk.X)
        for label, command in (
            ("+ Quadro", self.add_frame),
            ("Duplicar", self.duplicate_frame),
            ("Excluir", self.delete_frame),
        ):
            ttk.Button(frame_buttons, text=label, command=command).pack(
                side=tk.LEFT, expand=True, fill=tk.X, padx=1
            )
        order_buttons = ttk.Frame(parent)
        order_buttons.pack(fill=tk.X, pady=(2, 0))
        ttk.Button(order_buttons, text="Subir", command=lambda: self.move_frame(-1)).pack(
            side=tk.LEFT, expand=True, fill=tk.X, padx=1
        )
        ttk.Button(order_buttons, text="Descer", command=lambda: self.move_frame(1)).pack(
            side=tk.LEFT, expand=True, fill=tk.X, padx=1
        )

        frame_form = ttk.Frame(parent)
        frame_form.pack(fill=tk.X, pady=(8, 4))
        self.frame_vars = {
            "name": tk.StringVar(),
            "hold": tk.IntVar(value=1),
            "tone": tk.StringVar(value="corpo"),
            "marks": tk.StringVar(),
        }
        for row, (label, key) in enumerate((("Nome", "name"), ("Duracao (tempos)", "hold"))):
            ttk.Label(frame_form, text=label).grid(row=row, column=0, sticky=tk.W, pady=2)
            entry = ttk.Entry(frame_form, textvariable=self.frame_vars[key], width=12)
            entry.grid(row=row, column=1, sticky=tk.EW, pady=2)
            entry.bind("<Return>", lambda _event: self.apply_frame_properties())
            entry.bind("<FocusOut>", lambda _event: self.apply_frame_properties())
        ttk.Label(frame_form, text="Papel de cor").grid(row=2, column=0, sticky=tk.W, pady=2)
        tone_combo = ttk.Combobox(
            frame_form,
            textvariable=self.frame_vars["tone"],
            state="readonly",
            values=("corpo", "ferido", "morto"),
            width=10,
        )
        tone_combo.grid(row=2, column=1, sticky=tk.EW, pady=2)
        tone_combo.bind("<<ComboboxSelected>>", lambda _event: self.apply_frame_properties())
        ttk.Label(frame_form, text="Marcas").grid(row=3, column=0, sticky=tk.W, pady=2)
        marks_entry = ttk.Entry(frame_form, textvariable=self.frame_vars["marks"], width=12)
        marks_entry.grid(row=3, column=1, sticky=tk.EW, pady=2)
        marks_entry.bind("<Return>", lambda _event: self.apply_frame_properties())
        marks_entry.bind("<FocusOut>", lambda _event: self.apply_frame_properties())
        frame_form.columnconfigure(1, weight=1)
        ttk.Button(parent, text="Limpar quadro (voltar ao repouso)", command=self.clear_frame).pack(
            fill=tk.X, pady=(4, 0)
        )
        ttk.Label(
            holder,
            text="Um quadro guarda so a diferenca em relacao ao repouso.",
            wraplength=240,
            foreground="#666666",
        ).pack(fill=tk.X, pady=(8, 0))

    def _build_skin_tab(self, parent: ttk.Frame) -> None:
        ttk.Label(parent, text="Peles", font=("TkDefaultFont", 11, "bold")).pack(anchor=tk.W)
        self.skin_list = tk.Listbox(parent, height=6, exportselection=False)
        self.skin_list.pack(fill=tk.X, pady=(6, 4))
        self.skin_list.bind("<<ListboxSelect>>", self.on_skin_selected)
        skin_buttons = ttk.Frame(parent)
        skin_buttons.pack(fill=tk.X)
        for label, command in (
            ("Nova", self.add_skin),
            ("Duplicar", self.duplicate_skin),
            ("Excluir", self.delete_skin),
        ):
            ttk.Button(skin_buttons, text=label, command=command).pack(
                side=tk.LEFT, expand=True, fill=tk.X, padx=1
            )

        skin_form = ttk.Frame(parent)
        skin_form.pack(fill=tk.X, pady=(8, 4))
        self.skin_vars = {
            "name": tk.StringVar(),
            "accent": tk.StringVar(),
            "limb": tk.StringVar(value="|"),
            "swap": tk.StringVar(),
        }
        rows = (
            ("Nome", "name"),
            ("Glifos com acento", "accent"),
            ("Glifo do membro", "limb"),
            ("Trocas (O=0, |=║)", "swap"),
        )
        for row, (label, key) in enumerate(rows):
            ttk.Label(skin_form, text=label).grid(row=row, column=0, sticky=tk.W, pady=2)
            entry = ttk.Entry(skin_form, textvariable=self.skin_vars[key], width=16)
            entry.grid(row=row, column=1, sticky=tk.EW, pady=2)
            entry.bind("<Return>", lambda _event: self.apply_skin_properties())
        skin_form.columnconfigure(1, weight=1)

        self.skin_color_buttons: dict[str, tk.Button] = {}
        for key, label in (
            ("body", "Corpo"),
            ("hurt", "Ferido"),
            ("gone", "Morto"),
            ("limbs", "Membros"),
        ):
            row = ttk.Frame(parent)
            row.pack(fill=tk.X, pady=1)
            ttk.Label(row, text=label).pack(side=tk.LEFT)
            button = tk.Button(
                row,
                text="      ",
                command=lambda target=key: self.open_live_color_picker(f"skin:{target}"),
            )
            button.pack(side=tk.RIGHT)
            self.skin_color_buttons[key] = button

        ttk.Button(parent, text="Aplicar pele", command=self.apply_skin_properties).pack(
            fill=tk.X, pady=(8, 4)
        )
        self.skin_art_label = ttk.Label(parent, text="", foreground="#666666", wraplength=250)
        self.skin_art_label.pack(fill=tk.X, pady=(0, 4))
        ttk.Label(parent, text="Descricao opcional").pack(anchor=tk.W)
        self.skin_description = tk.Text(parent, height=4, width=28, wrap=tk.WORD)
        self.skin_description.pack(fill=tk.BOTH, expand=True, pady=(2, 6))
        self.skin_description.bind("<FocusOut>", lambda _event: self.apply_skin_properties())
        ttk.Label(
            parent,
            text=(
                "A pele decide glifo e cor das pecas marcadas como corpo ou membro, na aba "
                "Glyph. Um glyph sem papel guarda a propria cor e nao muda de pele."
            ),
            wraplength=250,
            foreground="#666666",
        ).pack(fill=tk.X)

    def _bind_shortcuts(self) -> None:
        bindings = {
            "<Control-n>": self.new_project,
            "<Control-o>": self.open_project,
            "<Control-s>": self.save_project,
            "<Control-Shift-S>": self.save_project_as,
            "<Control-e>": self.export_bundle,
            "<Control-Shift-E>": self.export_bundle,
            "<Control-z>": self.undo,
            "<Control-y>": self.redo,
            "<Control-d>": self.duplicate_selected,
            "<Control-c>": self.copy_selected,
            "<Control-v>": self.paste_clipboard,
            "<Delete>": self.delete_selected,
        }
        for sequence, command in bindings.items():
            self.root.bind(sequence, lambda _event, callback=command: callback())
        self.canvas.bind("<Control-a>", lambda _event: self.select_all())
        self.canvas.bind("<Escape>", lambda _event: self.deselect_all())
        self.canvas.bind("<Control-0>", lambda _event: self.reset_zoom())
        # No canvas, e nao na janela: senao um espaco digitado no nome de um
        # rotulo tocaria a animacao.
        self.canvas.bind("<space>", lambda _event: self.toggle_play())
        self.canvas.bind("<comma>", lambda _event: self.step_frame(-1))
        self.canvas.bind("<period>", lambda _event: self.step_frame(1))
        for sequence, delta in {
            "<Left>": (-1, 0),
            "<Right>": (1, 0),
            "<Up>": (0, -1),
            "<Down>": (0, 1),
            "<Shift-Left>": (-10, 0),
            "<Shift-Right>": (10, 0),
            "<Shift-Up>": (0, -10),
            "<Shift-Down>": (0, 10),
        }.items():
            self.canvas.bind(sequence, lambda _event, move=delta: self.nudge_selection(*move))

    def on_pan_press(self, event: tk.Event) -> None:
        self.canvas.scan_mark(event.x, event.y)
        self.canvas.configure(cursor="fleur")

    def on_pan_drag(self, event: tk.Event) -> None:
        self.canvas.scan_dragto(event.x, event.y, gain=1)

    def on_pan_release(self, event: tk.Event) -> None:
        self.canvas.configure(cursor="")
        self.on_canvas_motion(event)

    def on_zoom_wheel(self, event: tk.Event) -> str:
        direction = 1 if event.delta > 0 else -1
        return self.change_zoom(event, direction)

    def change_zoom(self, event: tk.Event, direction: int) -> str:
        old_zoom = self.zoom
        new_zoom = max(0.25, min(8.0, old_zoom * (1.15 if direction > 0 else 1 / 1.15)))
        if new_zoom == old_zoom:
            return "break"
        pointer_x = self.canvas.canvasx(event.x)
        pointer_y = self.canvas.canvasy(event.y)
        scene_x = (pointer_x - self.origin[0]) / old_zoom
        scene_y = (pointer_y - self.origin[1]) / old_zoom
        self.zoom = new_zoom
        self.zoom_text.set(f"Zoom {round(self.zoom * 100)}%")
        self.redraw()
        target_x = self.origin[0] + scene_x * self.zoom
        target_y = self.origin[1] + scene_y * self.zoom
        scroll_width = self.canvas_width * self.zoom + self.origin[0] * 2
        scroll_height = self.canvas_height * self.zoom + self.origin[1] * 2
        self.canvas.xview_moveto(max(0.0, (target_x - event.x) / scroll_width))
        self.canvas.yview_moveto(max(0.0, (target_y - event.y) / scroll_height))
        self.status.set(f"Zoom: {round(self.zoom * 100)}%")
        return "break"

    def reset_zoom(self) -> None:
        self.zoom = 1.0
        self.zoom_text.set("Zoom 100%")
        self.redraw()
        self.canvas.xview_moveto(0)
        self.canvas.yview_moveto(0)
        self.status.set("Zoom restaurado para 100%")

    def at_rest(self, item: Glyph | Joint) -> dict:
        """Os valores de repouso de uma peca, e nao os que estao na tela.

        A cena salva e sempre a de repouso: e o quadro que guarda a diferenca.
        Sem isto, salvar com um quadro aberto gravaria a pose daquele quadro
        como se fosse o repouso, e todos os outros quadros escorregariam junto.
        """
        return asdict(item) | self.rest.get(item.id, {})

    def current_scene(self) -> dict:
        self.sync_pose()
        return {
            "app": APP_NAME,
            "version": PROJECT_VERSION,
            "canvas": {
                "width": self.canvas_width,
                "height": self.canvas_height,
                "background": self.background,
                "grid_size": self.grid_size,
                "accent": self.accent,
                # O zoom viaja com o projeto porque ele e parte do enquadramento:
                # uma cena de 320px abre inutilizavel a 100% e util a 300%, e
                # nao ha por que quem a abre ter que descobrir isso de novo.
                "zoom": round(self.zoom, 3),
            },
            "default_font": {
                "kind": "bitmap_rom",
                "asset": "assets/fonts/ibm_vga_8x16.bin",
                "encoding": "CP437",
                "glyph_size": [8, 16],
            },
            "elements": [self.at_rest(element) for element in self.elements],
            "rig": {
                "joints": [self.at_rest(joint) for joint in self.joints if joint.kind == "joint"],
                "semantics": {
                    "parent_id": "Joint pai; a linha entre ambos representa um osso.",
                    "part_a_element_id": "Primeira peca conectada pelo ponto independente.",
                    "part_b_element_id": "Segunda peca conectada pelo ponto independente.",
                    "constraint_type": "pivot permite giro relativo; fixed solda as duas pecas.",
                    "fixed": "Quando verdadeiro, o ponto tambem esta ancorado ao mundo.",
                },
            },
            "attention_points": [
                self.at_rest(point) for point in self.joints if point.kind == "attention"
            ],
            "attention_semantics": {
                "description": "Destaques nomeados para a LLM; nao fazem parte do esqueleto.",
                "attached_element_id": "Glyph que carrega o destaque quando e movido.",
            },
            "labels": [asdict(label) for label in self.labels],
            "label_semantics": {
                "element_ids": "Glyphs diretamente contidos neste rotulo.",
                "label_ids": "Sub-rotulos contidos; seus glyphs sao herdados recursivamente.",
            },
            "animation": {
                "clips": [asdict(clip) for clip in self.clips],
                "semantics": {
                    "elements": "A lista `elements` e a pose de repouso; um quadro guarda so a diferenca.",
                    "keys": "id da peca -> campo -> valor. Campos possiveis: "
                    + ", ".join(ANIMATABLE)
                    + ".",
                    "hold": "Por quantos tempos o quadro fica na tela.",
                    "tone": "Papel de cor do corpo neste quadro: body, hurt ou gone.",
                    "fps": "Tempos por segundo do clipe.",
                    "marks": "O que acontece neste quadro (contato, brilho, som). Nomes livres.",
                },
            },
            "skins": [asdict(skin) for skin in self.skins],
            "active_skin": self.active_skin_id,
            "skin_semantics": {
                "role": "Cada glyph tem `role`: body usa as cores da pele, limb e um segmento "
                "de membro, vazio guarda a propria cor.",
                "swap": "Trocas de glifo `[de, para]` aplicadas depois de a cor estar decidida.",
                "accent": "Glifos que recebem `canvas.accent`, a cor de quem veste o boneco.",
            },
            "notes": self.notes.get("1.0", "end-1c") if hasattr(self, "notes") else "",
        }

    def load_scene(self, scene: dict, reset_history: bool = False) -> None:
        canvas = scene.get("canvas", {})
        self.canvas_width = max(1, int(canvas.get("width", 800)))
        self.canvas_height = max(1, int(canvas.get("height", 600)))
        self.background = canvas.get("background", "#181818")
        self.grid_size = max(2, int(canvas.get("grid_size", 16)))
        self.accent = canvas.get("accent", DEFAULT_ACCENT)
        self.zoom = max(0.25, min(8.0, float(canvas.get("zoom", self.zoom))))
        self.zoom_text.set(f"Zoom {round(self.zoom * 100)}%")
        self.elements = [Glyph.from_dict(value) for value in scene.get("elements", [])]
        self.joints = scene_joints(scene) + scene_attention_points(scene)
        self.labels = [SemanticLabel.from_dict(value) for value in scene.get("labels", [])]
        self.labels_migrated = infer_nested_labels(self.labels)
        self.clips = scene_clips(scene)
        self.skins = scene_skins(scene)
        self.active_skin_id = scene.get("active_skin", "")
        if self.skins and not any(skin.id == self.active_skin_id for skin in self.skins):
            self.active_skin_id = self.skins[0].id
        # A cena que chega e a de repouso, por definicao.
        self.rest = {item.id: self.animatable(item) for item in self.animated()}
        self.clamp_playhead()
        self.prune_animation()
        self.apply_pose()
        self.selected_label_id = None
        self.selected_id = None
        self.selected_joint_id = None
        self.selected_element_ids.clear()
        self.selected_joint_ids.clear()
        self.width_var.set(self.canvas_width)
        self.height_var.set(self.canvas_height)
        self.grid_var.set(self.grid_size)
        self.loading_notes = True
        self.notes.delete("1.0", tk.END)
        self.notes.insert("1.0", scene.get("notes", ""))
        self.notes.edit_modified(False)
        self.loading_notes = False
        if reset_history:
            self.undo_stack.clear()
            self.redo_stack.clear()
        self.redraw()
        self.sync_inspector()
        self.sync_label_list()
        self.sync_animation_lists()
        self.sync_skin_list()

    # --- animacao ------------------------------------------------------------

    def animated(self) -> list[Glyph | Joint]:
        """Tudo que um quadro pode mover: as pecas e os pontos do rig."""
        return [*self.elements, *self.joints]

    @staticmethod
    def animatable(item: Glyph | Joint) -> dict:
        values = asdict(item)
        return {key: values[key] for key in ANIMATABLE if key in values}

    def current_clip(self) -> Clip | None:
        if self.clip_index is None or not 0 <= self.clip_index < len(self.clips):
            return None
        return self.clips[self.clip_index]

    def current_frame(self) -> Frame | None:
        clip = self.current_clip()
        return clip.frame(self.frame_index) if clip else None

    def clamp_playhead(self) -> None:
        if not self.clips:
            self.clip_index = None
            self.frame_index = None
            return
        if self.clip_index is None or self.clip_index >= len(self.clips):
            self.clip_index = 0
        frames = self.clips[self.clip_index].frames
        if self.frame_index is not None and self.frame_index >= len(frames):
            self.frame_index = len(frames) - 1 if frames else None

    def sync_pose(self) -> None:
        """Grava o que esta na tela onde ele pertence.

        No repouso, o que esta na tela e o repouso. Com um quadro aberto, o que
        esta na tela e o quadro -- e o que se grava e so a diferenca. E o unico
        ponto do editor que escreve animacao, entao arrastar, digitar, colar e
        empurrar com as setas alimentam o quadro sem nenhum deles saber disso.
        """
        if self.playing:
            return
        frame = self.current_frame()
        for item in self.animated():
            self.rest.setdefault(item.id, self.animatable(item))
        if frame is None:
            self.rest = {item.id: self.animatable(item) for item in self.animated()}
            return
        keys = {}
        for item in self.animated():
            rest = self.rest[item.id]
            live = self.animatable(item)
            # O que o segmento deriva dos pontos nao vira chave: senao o quadro
            # guardaria a mesma pose duas vezes, e a copia derivada venceria a
            # proxima vez que alguem arrastasse um cotovelo. O mesmo vale para
            # a peca carregada por um ponto, que guarda a distancia e nao o
            # lugar.
            derived = ()
            if getattr(item, "span", None):
                derived = DERIVED
            elif getattr(item, "follow", ""):
                derived = CARRIED
            changed = {
                key: value
                for key, value in live.items()
                if key not in derived and rest.get(key) != value
            }
            if changed:
                keys[item.id] = changed
        frame.keys = keys

    def apply_pose(self) -> None:
        """Poe na tela o repouso somado ao quadro corrente."""
        frame = self.current_frame()
        keys = frame.keys if frame else {}
        for item in self.animated():
            for key, value in (self.rest.get(item.id, {}) | keys.get(item.id, {})).items():
                setattr(item, key, value)
        self.frame_text.set(self.playhead_label())

    def playhead_label(self) -> str:
        clip = self.current_clip()
        if clip is None:
            return "sem animacao"
        if self.frame_index is None:
            return f"{clip.name}: repouso"
        return f"{clip.name}: {self.frame_index + 1}/{len(clip.frames)}"

    def prune_animation(self) -> None:
        """Quadro nao guarda peca que nao existe mais."""
        alive = {item.id for item in self.animated()}
        self.rest = {key: value for key, value in self.rest.items() if key in alive}
        for clip in self.clips:
            for frame in clip.frames:
                frame.keys = {key: value for key, value in frame.keys.items() if key in alive}
        for element in self.elements:
            if element.span and not set(element.span).issubset(alive):
                element.span = []
            if element.follow and element.follow not in alive:
                element.follow = ""
                element.offset = []

    def onion_keys(self) -> dict[str, dict]:
        """O quadro anterior, para desenhar de sombra atras do atual."""
        clip = self.current_clip()
        if not self.onion_skin.get() or clip is None or self.frame_index is None:
            return {}
        if self.frame_index == 0 and not clip.loop:
            return {}
        previous = clip.frame((self.frame_index - 1) % max(1, len(clip.frames)))
        return previous.keys if previous else {}

    def goto(self, clip_index: int | None, frame_index: int | None) -> None:
        self.sync_pose()
        self.clip_index = clip_index
        self.frame_index = frame_index
        self.clamp_playhead()
        self.apply_pose()
        self.redraw()
        self.sync_inspector()
        self.sync_animation_lists()

    def goto_frame(self, index: int | None) -> None:
        self.goto(self.clip_index, index)

    def goto_rest(self) -> None:
        self.stop_play()
        self.goto(self.clip_index, None)
        self.status.set("Pose de repouso: o que voce editar aqui vale para todos os quadros")

    def step_frame(self, delta: int) -> None:
        clip = self.current_clip()
        if clip is None or not clip.frames:
            return
        current = self.frame_index if self.frame_index is not None else -1 if delta > 0 else 0
        self.goto_frame((current + delta) % len(clip.frames))

    def toggle_play(self) -> None:
        self.stop_play() if self.playing else self.start_play()

    def start_play(self) -> None:
        clip = self.current_clip()
        if clip is None or not clip.frames:
            self.status.set("Crie uma animacao com pelo menos um quadro para tocar")
            return
        self.sync_pose()
        if self.frame_index is None:
            self.frame_index = 0
        self.playing = True
        self.play_button.configure(text="■ Parar")
        self.tick_play()

    def stop_play(self) -> None:
        if self.play_job is not None:
            self.root.after_cancel(self.play_job)
            self.play_job = None
        if self.playing:
            self.playing = False
            self.play_button.configure(text="▶ Tocar")
            # A tela ficou no quadro em que o play parou: as listas e o
            # inspetor tem que concordar com ela antes da proxima edicao.
            self.sync_animation_lists()
            self.sync_inspector()

    def tick_play(self) -> None:
        clip = self.current_clip()
        if not self.playing or clip is None or not clip.frames:
            self.stop_play()
            return
        self.apply_pose()
        self.redraw()
        self.highlight_strip()
        frame = clip.frame(self.frame_index)
        delay = max(20, round(1000 * max(1, frame.hold if frame else 1) / max(0.5, clip.fps)))
        last = self.frame_index is not None and self.frame_index >= len(clip.frames) - 1
        if last and not clip.loop:
            self.play_job = self.root.after(delay, self.stop_play)
            return
        self.play_job = self.root.after(delay, self.advance_play)

    def advance_play(self) -> None:
        """Anda um quadro e desenha.

        O avanco tem que vir *antes* do desenho, e nao depois: enquanto ele
        ficava no fim do tick, `frame_index` apontava para o quadro seguinte
        enquanto a tela mostrava o anterior. Parar o play no meio disso deixava
        o playhead num quadro que ninguem estava vendo -- e a proxima peca
        arrastada era gravada nele.
        """
        clip = self.current_clip()
        if clip and clip.frames:
            self.frame_index = ((self.frame_index or 0) + 1) % len(clip.frames)
        self.tick_play()

    def add_clip(self) -> None:
        self.stop_play()
        self.checkpoint()
        clip = Clip.create(len(self.clips) + 1)
        self.clips.append(clip)
        self.goto(len(self.clips) - 1, None)
        self.status.set(f"Animacao criada: {clip.name}")

    def duplicate_clip(self) -> None:
        clip = self.current_clip()
        if clip is None:
            return
        self.stop_play()
        self.checkpoint()
        copied = Clip.from_dict(asdict(clip))
        copied.id = uuid.uuid4().hex[:10]
        copied.name = f"{clip.name}_copia"
        for frame in copied.frames:
            frame.id = uuid.uuid4().hex[:10]
        self.clips.insert(self.clip_index + 1, copied)
        self.goto(self.clip_index + 1, None)
        self.status.set(f"Animacao duplicada: {copied.name}")

    def delete_clip(self) -> None:
        clip = self.current_clip()
        if clip is None:
            return
        self.stop_play()
        self.checkpoint()
        self.clips.pop(self.clip_index)
        self.goto(self.clip_index - 1 if self.clip_index else None, None)
        self.status.set(f"Animacao removida: {clip.name}")

    def add_frame(self) -> None:
        """Grava a pose que esta na tela como um quadro novo."""
        self.stop_play()
        if self.current_clip() is None:
            self.add_clip()
        clip = self.current_clip()
        self.checkpoint()
        self.sync_pose()
        frame = Frame.create(len(clip.frames) + 1)
        source = self.current_frame()
        if source is not None:
            frame.keys = copy.deepcopy(source.keys)
            frame.tone = source.tone
        at = len(clip.frames) if self.frame_index is None else self.frame_index + 1
        clip.frames.insert(at, frame)
        self.goto(self.clip_index, at)
        self.status.set(f"Quadro criado: {frame.name}")

    def duplicate_frame(self) -> None:
        if self.current_frame() is None:
            return
        self.add_frame()

    def delete_frame(self) -> None:
        clip = self.current_clip()
        frame = self.current_frame()
        if clip is None or frame is None:
            return
        self.stop_play()
        self.checkpoint()
        clip.frames.pop(self.frame_index)
        self.goto(self.clip_index, self.frame_index - 1 if self.frame_index else None)
        self.status.set(f"Quadro removido: {frame.name}")

    def move_frame(self, delta: int) -> None:
        clip = self.current_clip()
        if clip is None or self.frame_index is None:
            return
        target = self.frame_index + delta
        if not 0 <= target < len(clip.frames):
            return
        self.stop_play()
        self.checkpoint()
        self.sync_pose()
        clip.frames.insert(target, clip.frames.pop(self.frame_index))
        self.goto(self.clip_index, target)

    def clear_frame(self) -> None:
        frame = self.current_frame()
        if frame is None:
            return
        self.checkpoint()
        frame.keys.clear()
        self.apply_pose()
        self.redraw()
        self.sync_inspector()
        self.status.set(f"Quadro {frame.name} voltou ao repouso")

    def apply_clip_properties(self) -> None:
        """Grava nome, cadencia e ciclo no clipe -- se algum deles mudou.

        A saida antecipada nao e economia: sem ela, cada clique fora do campo
        empilharia um passo de desfazer identico ao anterior, e `Ctrl+Z` viraria
        uma fila de nada.
        """
        clip = self.current_clip()
        if clip is None:
            return
        name = self.clip_vars["name"].get().strip() or clip.name
        try:
            fps = max(0.5, float(self.clip_vars["fps"].get()))
        except (tk.TclError, ValueError):
            fps = clip.fps
        loop = bool(self.clip_vars["loop"].get())
        if (name, fps, loop) == (clip.name, clip.fps, clip.loop):
            return
        self.checkpoint()
        clip.name, clip.fps, clip.loop = name, fps, loop
        self.sync_animation_lists()
        self.frame_text.set(self.playhead_label())
        self.status.set(f"Animacao atualizada: {clip.name}")

    def apply_frame_properties(self) -> None:
        frame = self.current_frame()
        if frame is None:
            return
        name = self.frame_vars["name"].get().strip() or frame.name
        try:
            hold = max(1, int(self.frame_vars["hold"].get()))
        except (tk.TclError, ValueError):
            hold = frame.hold
        tone = {"corpo": "body", "ferido": "hurt", "morto": "gone"}.get(
            self.frame_vars["tone"].get(), "body"
        )
        marks = [
            mark.strip() for mark in self.frame_vars["marks"].get().split(",") if mark.strip()
        ]
        if (name, hold, tone, marks) == (frame.name, frame.hold, frame.tone, frame.marks):
            return
        self.checkpoint()
        frame.name, frame.hold, frame.tone, frame.marks = name, hold, tone, marks
        self.sync_animation_lists()
        self.redraw()
        self.status.set(f"Quadro atualizado: {frame.name}")

    def sync_frame_strip(self) -> None:
        """Redesenha a tira de quadros sob o canvas."""
        if not hasattr(self, "strip"):
            return
        for button in self.strip_buttons:
            button.destroy()
        self.strip_buttons = []
        clip = self.current_clip()
        rest = tk.Button(
            self.strip,
            text="repouso",
            font=("TkDefaultFont", 8),
            relief=tk.SUNKEN if self.frame_index is None else tk.RAISED,
            bg="#d7e8ff" if self.frame_index is None else "#f0f0f0",
            command=self.goto_rest,
        )
        rest.pack(side=tk.LEFT, padx=(0, 6))
        Tip(rest, "A pose base. O que voce editar aqui vale para todos os quadros.")
        self.strip_buttons.append(rest)
        for index, frame in enumerate(clip.frames if clip else []):
            current = index == self.frame_index
            button = tk.Button(
                self.strip,
                text=f"{index + 1}{'•' if frame.marks else ''}",
                width=3,
                font=("TkDefaultFont", 8),
                relief=tk.SUNKEN if current else tk.RAISED,
                bg="#d7e8ff" if current else ("#fffbe0" if frame.keys else "#f0f0f0"),
                command=lambda at=index: self.goto_frame(at),
            )
            button.pack(side=tk.LEFT, padx=1)
            marks = f"\nMarcas: {', '.join(frame.marks)}" if frame.marks else ""
            Tip(button, f"{frame.name}  ({frame.hold} tempo(s)){marks}")
            self.strip_buttons.append(button)

    def highlight_strip(self) -> None:
        """So repinta qual quadro esta aceso, sem refazer a tira.

        Tocar precisa acender o botao do quadro corrente, mas refazer os botoes
        a cada quadro pisca a tira inteira -- e num clipe de 100 tempos por
        segundo isso e trabalho a toa cinquenta vezes por segundo.
        """
        clip = self.current_clip()
        for index, button in enumerate(self.strip_buttons):
            at = None if index == 0 else index - 1
            current = at == self.frame_index
            frame = clip.frame(at) if clip and at is not None else None
            button.configure(
                relief=tk.SUNKEN if current else tk.RAISED,
                bg="#d7e8ff"
                if current
                else ("#fffbe0" if frame and frame.keys else "#f0f0f0"),
            )

    def sync_animation_lists(self) -> None:
        self.sync_frame_strip()
        if not hasattr(self, "clip_list"):
            return
        self.clip_list.delete(0, tk.END)
        for clip in self.clips:
            self.clip_list.insert(tk.END, f"{clip.name} ({len(clip.frames)})")
        names = [clip.name for clip in self.clips]
        self.clip_combo.configure(values=names or ["(nenhuma)"])
        clip = self.current_clip()
        if clip is not None:
            self.clip_list.selection_clear(0, tk.END)
            self.clip_list.selection_set(self.clip_index)
            self.clip_choice.set(clip.name)
            self.clip_vars["name"].set(clip.name)
            self.clip_vars["fps"].set(clip.fps)
            self.clip_vars["loop"].set(clip.loop)
        else:
            self.clip_choice.set("(nenhuma)")
            self.clip_vars["name"].set("")

        self.frame_list.delete(0, tk.END)
        for index, frame in enumerate(clip.frames if clip else []):
            mark = "*" if frame.keys else " "
            marks = f"  [{', '.join(frame.marks)}]" if frame.marks else ""
            self.frame_list.insert(
                tk.END, f"{index + 1:>2}{mark} {frame.name} x{frame.hold}{marks}"
            )
        frame = self.current_frame()
        if frame is not None:
            self.frame_list.selection_clear(0, tk.END)
            self.frame_list.selection_set(self.frame_index)
            self.frame_list.see(self.frame_index)
            self.frame_vars["name"].set(frame.name)
            self.frame_vars["hold"].set(frame.hold)
            self.frame_vars["tone"].set(
                {"body": "corpo", "hurt": "ferido", "gone": "morto"}[frame.tone]
            )
            self.frame_vars["marks"].set(", ".join(frame.marks))
        else:
            self.frame_vars["name"].set("")
            self.frame_vars["marks"].set("")
        self.frame_text.set(self.playhead_label())

    def on_clip_selected(self, _event: tk.Event | None = None) -> None:
        selection = self.clip_list.curselection()
        if selection and selection[0] != self.clip_index:
            self.stop_play()
            self.goto(selection[0], None)

    def on_clip_chosen(self, _event: tk.Event | None = None) -> None:
        name = self.clip_choice.get()
        index = next((i for i, clip in enumerate(self.clips) if clip.name == name), None)
        if index is not None and index != self.clip_index:
            self.stop_play()
            self.goto(index, None)

    def on_frame_selected(self, _event: tk.Event | None = None) -> None:
        selection = self.frame_list.curselection()
        if selection and selection[0] != self.frame_index:
            self.stop_play()
            self.goto_frame(selection[0])

    # --- peles ---------------------------------------------------------------

    def current_skin(self) -> Skin | None:
        return next((skin for skin in self.skins if skin.id == self.active_skin_id), None)

    def add_skin(self) -> None:
        self.checkpoint()
        skin = Skin.create(len(self.skins) + 1)
        self.skins.append(skin)
        self.active_skin_id = skin.id
        self.sync_skin_list()
        self.redraw()
        self.status.set(f"Pele criada: {skin.name}")

    def duplicate_skin(self) -> None:
        skin = self.current_skin()
        if skin is None:
            return
        self.checkpoint()
        copied = Skin.from_dict(asdict(skin))
        copied.id = uuid.uuid4().hex[:10]
        copied.name = f"{skin.name}_copia"
        self.skins.append(copied)
        self.active_skin_id = copied.id
        self.sync_skin_list()
        self.redraw()
        self.status.set(f"Pele duplicada: {copied.name}")

    def delete_skin(self) -> None:
        skin = self.current_skin()
        if skin is None:
            return
        self.checkpoint()
        self.skins.remove(skin)
        self.active_skin_id = self.skins[0].id if self.skins else ""
        self.sync_skin_list()
        self.redraw()
        self.status.set(f"Pele removida: {skin.name}")

    def apply_skin_properties(self) -> None:
        skin = self.current_skin()
        if skin is None:
            return
        self.checkpoint()
        skin.name = self.skin_vars["name"].get().strip() or skin.name
        skin.accent = self.skin_vars["accent"].get()
        skin.limb = (self.skin_vars["limb"].get() or "|")[0]
        skin.swap = [
            [pair.split("=", 1)[0].strip()[:1], pair.split("=", 1)[1].strip()[:1]]
            for pair in self.skin_vars["swap"].get().split(",")
            if "=" in pair and pair.split("=", 1)[0].strip()
        ]
        skin.description = self.skin_description.get("1.0", "end-1c").strip()
        self.sync_skin_list()
        self.redraw()
        self.status.set(f"Pele atualizada: {skin.name}")

    def sync_skin_list(self) -> None:
        if not hasattr(self, "skin_list"):
            return
        self.skin_list.delete(0, tk.END)
        for skin in self.skins:
            self.skin_list.insert(tk.END, skin.name)
        skin = self.current_skin()
        if skin is None:
            self.skin_vars["name"].set("")
            return
        self.skin_list.selection_clear(0, tk.END)
        self.skin_list.selection_set(self.skins.index(skin))
        self.skin_vars["name"].set(skin.name)
        self.skin_vars["accent"].set(skin.accent)
        self.skin_vars["limb"].set(skin.limb)
        self.skin_vars["swap"].set(", ".join(f"{a}={b}" for a, b in skin.swap))
        self.skin_description.delete("1.0", tk.END)
        self.skin_description.insert("1.0", skin.description)
        for key, button in self.skin_color_buttons.items():
            button.configure(bg=getattr(skin, key))
        redrawn = sum(len(cells) for cells in skin.art.values())
        self.skin_art_label.configure(
            text=f"Redesenha {redrawn} glifo(s) em {len(skin.art)} quadro(s); edite em `art` no JSON."
            if redrawn
            else "Nao redesenha quadro nenhum: so trocas e cores."
        )

    def on_skin_selected(self, _event: tk.Event | None = None) -> None:
        selection = self.skin_list.curselection()
        if not selection:
            return
        chosen = self.skins[selection[0]]
        if chosen.id != self.active_skin_id:
            self.active_skin_id = chosen.id
            self.sync_skin_list()
            self.redraw()
            self.status.set(f"Vestindo: {chosen.name}")

    def checkpoint(self) -> None:
        self.undo_stack.append(copy.deepcopy(self.current_scene()))
        self.undo_stack = self.undo_stack[-100:]
        self.redo_stack.clear()
        self.mark_dirty()

    def mark_dirty(self) -> None:
        self.dirty = True
        self.update_title()

    def update_title(self) -> None:
        name = self.project_path.name if self.project_path else "sem-titulo.glyph.json"
        self.root.title(f"{'*' if self.dirty else ''}{name} - {APP_NAME}")

    def redraw(self) -> None:
        # Antes de qualquer coisa: um segmento so sabe onde esta depois que os
        # pontos dele sabem. Fica aqui, e nao em cada lugar que move um ponto,
        # porque desenhar e o unico caminho por onde toda mudanca passa.
        solve_spans(self.elements, self.joints)
        self.canvas.delete("all")
        ox, oy = self.origin
        self.canvas.create_rectangle(
            ox,
            oy,
            ox + self.canvas_width * self.zoom,
            oy + self.canvas_height * self.zoom,
            fill=self.background,
            outline="#777777",
            tags=("stage",),
        )
        if self.show_grid.get() and self.grid_size >= 2:
            color = grid_line_color(self.background)
            for grid_x in range(self.grid_size, self.canvas_width, self.grid_size):
                self.canvas.create_line(
                    ox + grid_x * self.zoom,
                    oy,
                    ox + grid_x * self.zoom,
                    oy + self.canvas_height * self.zoom,
                    fill=color,
                    tags=("grid",),
                )
            for grid_y in range(self.grid_size, self.canvas_height, self.grid_size):
                self.canvas.create_line(
                    ox,
                    oy + grid_y * self.zoom,
                    ox + self.canvas_width * self.zoom,
                    oy + grid_y * self.zoom,
                    fill=color,
                    tags=("grid",),
                )
        if self.show_rig.get():
            joints_by_id = {joint.id: joint for joint in self.joints}
            elements_by_id = {element.id: element for element in self.elements}
            for joint in self.joints:
                parent = joints_by_id.get(joint.parent_id)
                if joint.kind == "joint" and parent and parent.kind == "joint":
                    self.canvas.create_line(
                        ox + parent.x * self.zoom,
                        oy + parent.y * self.zoom,
                        ox + joint.x * self.zoom,
                        oy + joint.y * self.zoom,
                        fill="#4db7ff",
                        width=2,
                        tags=("rig-line",),
                    )
                linked_ids = (
                    (joint.attached_element_id,)
                    if joint.kind == "attention"
                    else (joint.part_a_element_id, joint.part_b_element_id)
                )
                for linked_id in linked_ids:
                    linked = elements_by_id.get(linked_id)
                    if linked and (linked.x != joint.x or linked.y != joint.y):
                        self.canvas.create_line(
                            ox + linked.x * self.zoom,
                            oy + linked.y * self.zoom,
                            ox + joint.x * self.zoom,
                            oy + joint.y * self.zoom,
                            fill=joint.color if joint.kind == "attention" else "#4db7ff",
                            dash=(3, 3),
                            tags=("rig-attachment",),
                        )
        self.photos.clear()
        if self.onion_skin.get() and self.current_frame() is not None:
            onion = self.onion_keys()
            for element in self.elements:
                ghost = Glyph.from_dict(
                    asdict(element) | self.rest.get(element.id, {}) | onion.get(element.id, {})
                )
                ghost.color = ONION_COLOR
                self.place_glyph(ghost, f"onion:{element.id}", ("onion",))
        for _, element in sorted(enumerate(self.elements), key=lambda pair: (pair[1].layer, pair[0])):
            self.place_glyph(
                self.dressed_view(element),
                element.id,
                ("element", f"element:{element.id}"),
            )
        selected_label = next(
            (label for label in self.labels if label.id == self.selected_label_id), None
        )
        if selected_label:
            labels_by_id = {label.id: label for label in self.labels}
            boxes = [
                self.canvas.bbox(f"element:{element_id}")
                for element_id in resolved_label_elements(selected_label.id, labels_by_id)
            ]
            boxes = [box for box in boxes if box]
            if boxes:
                left = min(box[0] for box in boxes) - 10
                top = min(box[1] for box in boxes) - 22
                right = max(box[2] for box in boxes) + 10
                bottom = max(box[3] for box in boxes) + 10
                self.canvas.create_rectangle(
                    left,
                    top,
                    right,
                    bottom,
                    outline="#a970ff",
                    dash=(6, 4),
                    width=2,
                    tags=("semantic-label",),
                )
                self.canvas.create_text(
                    left + 5,
                    top + 2,
                    text=selected_label.name,
                    fill="#d7b8ff",
                    anchor=tk.NW,
                    font=("TkDefaultFont", 9, "bold"),
                    tags=("semantic-label",),
                )
        if self.show_rig.get():
            for joint in self.joints:
                x, y = ox + joint.x * self.zoom, oy + joint.y * self.zoom
                if joint.kind == "attention":
                    self.canvas.create_polygon(
                        x,
                        y - 8,
                        x + 8,
                        y,
                        x,
                        y + 8,
                        x - 8,
                        y,
                        fill=joint.color,
                        outline="#000000",
                        width=2,
                        tags=("joint", "attention", f"joint:{joint.id}"),
                    )
                else:
                    marker = (
                        self.canvas.create_rectangle
                        if joint.fixed or joint.constraint_type == "fixed"
                        else self.canvas.create_oval
                    )
                    marker(
                        x - 7,
                        y - 7,
                        x + 7,
                        y + 7,
                        fill=joint.color,
                        outline="#000000",
                        width=2,
                        tags=("joint", f"joint:{joint.id}"),
                    )
                if joint.kind == "joint" and joint.fixed:
                    self.canvas.create_line(
                        x - 10,
                        y,
                        x + 10,
                        y,
                        fill=joint.color,
                        width=2,
                        tags=("joint", f"joint:{joint.id}"),
                    )
                    self.canvas.create_line(
                        x,
                        y - 10,
                        x,
                        y + 10,
                        fill=joint.color,
                        width=2,
                        tags=("joint", f"joint:{joint.id}"),
                    )
                # O nome do ponto selecionado aparece mesmo com os nomes
                # desligados: quem desliga quer parar de ver os doze, e nao
                # perder de vista o que esta mexendo.
                if self.show_names.get() or joint.id in self.selected_joint_ids:
                    self.canvas.create_text(
                        x + 11,
                        y - 10,
                        text=joint.name,
                        fill="#ffffff",
                        anchor=tk.W,
                        font=("TkDefaultFont", 9, "bold"),
                        tags=("joint", f"joint:{joint.id}"),
                    )
        self.canvas.configure(
            scrollregion=(
                0,
                0,
                self.canvas_width * self.zoom + ox * 2,
                self.canvas_height * self.zoom + oy * 2,
            )
        )
        self.draw_selection()
        self.background_button.configure(bg=self.background)
        self.accent_button.configure(bg=self.accent)

    def dressed_view(self, element: Glyph) -> Glyph:
        """A peca como ela aparece agora: pele corrente e papel de cor do quadro."""
        frame = self.current_frame()
        return dressed(
            element,
            self.current_skin(),
            frame.tone if frame else "body",
            self.accent,
            frame.id if frame else "",
        )

    def place_glyph(self, element: Glyph, key: str, tags: tuple[str, ...]) -> None:
        image = glyph_image(element)
        if self.zoom != 1.0:
            image = image.resize(
                (
                    max(1, round(image.width * self.zoom)),
                    max(1, round(image.height * self.zoom)),
                ),
                Image.Resampling.NEAREST
                if element.font_path.lower().endswith(".bin")
                else Image.Resampling.LANCZOS,
            )
        photo = ImageTk.PhotoImage(image)
        self.photos[key] = photo
        self.canvas.create_image(
            self.origin[0] + element.x * self.zoom,
            self.origin[1] + element.y * self.zoom,
            image=photo,
            tags=tags,
        )

    def transform_geometry(self, element: Glyph) -> dict[str, object]:
        image = glyph_image(element, include_rotation=False)
        width = max(1.0, round(image.width * self.zoom))
        height = max(1.0, round(image.height * self.zoom))
        center = (
            self.origin[0] + element.x * self.zoom,
            self.origin[1] + element.y * self.zoom,
        )
        angle = math.radians(element.rotation)
        cosine, sine = math.cos(angle), math.sin(angle)

        def point(local_x: float, local_y: float) -> tuple[float, float]:
            return (
                center[0] + local_x * cosine - local_y * sine,
                center[1] + local_x * sine + local_y * cosine,
            )

        corners = {
            "nw": point(-width / 2, -height / 2),
            "ne": point(width / 2, -height / 2),
            "se": point(width / 2, height / 2),
            "sw": point(-width / 2, height / 2),
        }
        return {
            "center": center,
            "width": width,
            "height": height,
            "corners": corners,
            "top": point(0, -height / 2),
            "rotate": point(0, -height / 2 - 28),
        }

    def draw_selection(self) -> None:
        self.canvas.delete("selection")
        # Um segmento nao ganha alcas: escala e giro dele saem dos dois pontos,
        # entao a alca prometeria um controle que o proximo redraw desfaz.
        single_transform = (
            len(self.selected_element_ids) == 1
            and not self.selected_joint_ids
            and bool(self.selected_id)
            and not (self.selected() and self.selected().span)
        )
        tags = [(f"element:{item_id}", item_id == self.selected_id) for item_id in self.selected_element_ids]
        tags += [(f"joint:{item_id}", item_id == self.selected_joint_id) for item_id in self.selected_joint_ids]
        for tag, primary in tags:
            if single_transform and primary and tag.startswith("element:"):
                continue
            bbox = self.canvas.bbox(tag)
            if bbox:
                self.canvas.create_rectangle(
                    bbox[0] - 3,
                    bbox[1] - 3,
                    bbox[2] + 3,
                    bbox[3] + 3,
                    outline="#4db7ff" if primary else "#75e69a",
                    dash=(4, 3),
                    width=2,
                    tags=("selection",),
                )
        if single_transform:
            element = self.selected()
            if not element:
                return
            geometry = self.transform_geometry(element)
            corners = geometry["corners"]
            ordered = [corners[name] for name in ("nw", "ne", "se", "sw", "nw")]
            self.canvas.create_line(
                *(coordinate for point in ordered for coordinate in point),
                fill="#4db7ff",
                dash=(4, 3),
                width=2,
                tags=("selection",),
            )
            handle_size = 5
            for name, (x, y) in corners.items():
                self.canvas.create_rectangle(
                    x - handle_size,
                    y - handle_size,
                    x + handle_size,
                    y + handle_size,
                    fill="#ffffff",
                    outline="#1479b8",
                    width=2,
                    tags=("selection", "transform-handle", f"handle:scale:{name}"),
                )
            middle, top = geometry["top"]
            rotate_x, rotate_y = geometry["rotate"]
            self.canvas.create_line(
                middle,
                top,
                rotate_x,
                rotate_y,
                fill="#4db7ff",
                width=2,
                tags=("selection",),
            )
            self.canvas.create_oval(
                rotate_x - 6,
                rotate_y - 6,
                rotate_x + 6,
                rotate_y + 6,
                fill="#ffcc33",
                outline="#1479b8",
                width=2,
                tags=("selection", "transform-handle", "handle:rotate"),
            )

    def clear_selection(self) -> None:
        self.selected_id = None
        self.selected_joint_id = None
        self.selected_element_ids.clear()
        self.selected_joint_ids.clear()

    def deselect_all(self) -> None:
        self.clear_selection()
        self.draw_selection()
        self.sync_inspector()
        self.status.set("Selecao limpa")

    def select_all(self) -> None:
        self.selected_element_ids = {element.id for element in self.elements}
        self.selected_joint_ids = {joint.id for joint in self.joints}
        self.selected_id = self.elements[-1].id if self.elements else None
        self.selected_joint_id = self.joints[-1].id if self.joints else None
        if self.selected_joint_id:
            self.selected_id = None
        self.draw_selection()
        self.sync_inspector()
        self.update_selection_status()

    def update_selection_status(self) -> None:
        glyphs = len(self.selected_element_ids)
        selected_points = [point for point in self.joints if point.id in self.selected_joint_ids]
        joints = sum(point.kind == "joint" for point in selected_points)
        attention = sum(point.kind == "attention" for point in selected_points)
        self.status.set(
            f"Selecionados: {glyphs} glyph(s), {joints} articulacao(oes), {attention} destaque(s)"
        )

    @staticmethod
    def shift_element(element: Glyph, dx: float, dy: float) -> None:
        """Move a peca -- pela distancia ate quem a carrega, se ela e carregada.

        Existe para que arrastar seja uma coisa so no editor e continue sendo
        duas no modelo: mexer numa arma na mao muda a empunhadura, e nao a
        posicao dela no mundo, que quem decide e a mao.
        """
        if element.follow:
            offset = element.offset if len(element.offset) == 2 else [0.0, 0.0]
            element.offset = [round(offset[0] + dx, 4), round(offset[1] + dy, 4)]
            return
        element.x += dx
        element.y += dy

    def dragged_joint_ids(self) -> set[str]:
        """Os pontos que acompanham a selecao quando ela se move.

        Sao tres motivos diferentes: o ponto esta selecionado, ele esta preso a
        um glyph selecionado, ou ele e uma das pontas de um segmento
        selecionado. O ultimo e o que faz arrastar um braco levar o braco
        inteiro em vez de deixa-lo grudado nos pontos antigos.
        """
        moving = set(self.selected_joint_ids)
        for element in self.elements:
            if element.id in self.selected_element_ids:
                moving.update(element.span)
        moving.update(
            joint.id
            for joint in self.joints
            if joint.kind == "attention"
            and joint.attached_element_id in self.selected_element_ids
        )
        return moving

    def snap_value(self, value: float) -> float:
        return round(value / self.grid_size) * self.grid_size if self.snap_to_grid.get() else value

    def nudge_selection(self, dx: float, dy: float) -> None:
        if not self.selected_element_ids and not self.selected_joint_ids:
            return
        if self.snap_to_grid.get():
            dx = (self.grid_size * (5 if abs(dx) >= 10 else 1)) * (1 if dx > 0 else -1) if dx else 0
            dy = (self.grid_size * (5 if abs(dy) >= 10 else 1)) * (1 if dy > 0 else -1) if dy else 0
        self.checkpoint()
        for element in self.elements:
            if element.id in self.selected_element_ids:
                self.shift_element(element, dx, dy)
        moving = self.dragged_joint_ids()
        for joint in self.joints:
            if joint.id in moving:
                joint.x += dx
                joint.y += dy
        self.redraw()
        self.sync_inspector()
        self.update_selection_status()

    def select_element(self, item_id: str, additive: bool = False) -> None:
        if not additive:
            self.clear_selection()
        if additive and item_id in self.selected_element_ids:
            self.selected_element_ids.remove(item_id)
            if self.selected_id == item_id:
                self.selected_id = next(iter(self.selected_element_ids), None)
            return
        self.selected_element_ids.add(item_id)
        self.selected_id = item_id
        self.selected_joint_id = None

    def select_joint(self, item_id: str, additive: bool = False) -> None:
        if not additive:
            self.clear_selection()
        if additive and item_id in self.selected_joint_ids:
            self.selected_joint_ids.remove(item_id)
            if self.selected_joint_id == item_id:
                self.selected_joint_id = next(iter(self.selected_joint_ids), None)
            return
        self.selected_joint_ids.add(item_id)
        self.selected_joint_id = item_id
        self.selected_id = None

    def selected(self) -> Glyph | None:
        return next((element for element in self.elements if element.id == self.selected_id), None)

    def selected_joint(self) -> Joint | None:
        return next((joint for joint in self.joints if joint.id == self.selected_joint_id), None)

    def selected_label(self) -> SemanticLabel | None:
        return next((label for label in self.labels if label.id == self.selected_label_id), None)

    def sync_label_list(self) -> None:
        self.label_list.delete(0, tk.END)
        selected_index = None
        labels_by_id = {label.id: label for label in self.labels}
        for index, label in enumerate(self.labels):
            total = len(resolved_label_elements(label.id, labels_by_id))
            nested = f", {len(label.label_ids)} conjunto(s)" if label.label_ids else ""
            self.label_list.insert(tk.END, f"{label.name} ({total} glyph(s){nested})")
            if label.id == self.selected_label_id:
                selected_index = index
        if selected_index is not None:
            self.label_list.selection_set(selected_index)
            self.label_list.see(selected_index)
        self.sync_label_editor()

    def sync_label_editor(self) -> None:
        label = self.selected_label()
        self.label_name_var.set(label.name if label else "")
        self.label_description.delete("1.0", tk.END)
        self.label_children_list.delete(0, tk.END)
        self.label_child_ids = []
        if label:
            self.label_description.insert("1.0", label.description)
            labels_by_id = {item.id: item for item in self.labels}
            total = len(resolved_label_elements(label.id, labels_by_id))
            self.label_count_var.set(
                f"{len(label.element_ids)} direto(s) + {len(label.label_ids)} conjunto(s) = {total} glyph(s)"
            )
            for item in self.labels:
                if item.id == label.id:
                    continue
                index = self.label_children_list.size()
                child_total = len(resolved_label_elements(item.id, labels_by_id))
                self.label_children_list.insert(tk.END, f"{item.name} ({child_total})")
                self.label_child_ids.append(item.id)
                if item.id in label.label_ids:
                    self.label_children_list.selection_set(index)
        else:
            self.label_count_var.set("Nenhum rotulo selecionado")

    def on_label_selected(self, _event: tk.Event | None = None) -> None:
        selection = self.label_list.curselection()
        if not selection or selection[0] >= len(self.labels):
            return
        self.selected_label_id = self.labels[selection[0]].id
        self.sync_label_editor()
        self.redraw()

    def create_label_from_selection(self) -> None:
        if not self.selected_element_ids:
            messagebox.showinfo(APP_NAME, "Selecione um ou mais glyphs antes de criar o rotulo.")
            return
        self.checkpoint()
        label = SemanticLabel.create(self.selected_element_ids, len(self.labels) + 1)
        self.labels.append(label)
        infer_nested_labels(self.labels)
        self.selected_label_id = label.id
        self.sync_label_list()
        self.inspector_tabs.select(3)
        self.redraw()
        total = len(resolved_label_elements(label.id, {item.id: item for item in self.labels}))
        self.status.set(f"Rotulo criado para {total} glyph(s)")

    def apply_label_properties(self) -> None:
        label = self.selected_label()
        if not label:
            return
        name = self.label_name_var.get().strip() or label.id
        description = self.label_description.get("1.0", "end-1c").strip()
        child_ids = [
            self.label_child_ids[index]
            for index in self.label_children_list.curselection()
            if index < len(self.label_child_ids)
        ]
        labels_by_id = {item.id: item for item in self.labels}
        if any(label_reaches(child_id, label.id, labels_by_id) for child_id in child_ids):
            messagebox.showerror(APP_NAME, "Um sub-rotulo nao pode criar um ciclo de conjuntos.")
            self.sync_label_editor()
            return
        nested_elements: set[str] = set()
        for child_id in child_ids:
            nested_elements.update(resolved_label_elements(child_id, labels_by_id))
        direct_elements = sorted(set(label.element_ids) - nested_elements)
        if (name, description, direct_elements, child_ids) == (
            label.name,
            label.description,
            label.element_ids,
            label.label_ids,
        ):
            return
        self.checkpoint()
        label.name = name
        label.description = description
        label.element_ids = direct_elements
        label.label_ids = child_ids
        self.sync_label_list()
        self.redraw()

    def delete_selected_label(self) -> None:
        deleted = self.selected_label()
        if not deleted:
            return
        self.checkpoint()
        labels_by_id = {label.id: label for label in self.labels}
        promoted = resolved_label_elements(deleted.id, labels_by_id)
        for parent in self.labels:
            if deleted.id in parent.label_ids:
                parent.label_ids = [child_id for child_id in parent.label_ids if child_id != deleted.id]
                parent.element_ids = sorted(set(parent.element_ids) | promoted)
        self.labels = [label for label in self.labels if label.id != deleted.id]
        self.selected_label_id = None
        self.sync_label_list()
        self.redraw()

    def select_label_members(self) -> None:
        label = self.selected_label()
        if not label:
            return
        valid_ids = {element.id for element in self.elements}
        labels_by_id = {item.id: item for item in self.labels}
        self.clear_selection()
        self.selected_element_ids = resolved_label_elements(label.id, labels_by_id) & valid_ids
        self.selected_id = next(iter(self.selected_element_ids), None)
        self.draw_selection()
        self.sync_inspector()
        self.inspector_tabs.select(3)
        self.update_selection_status()

    def update_label_members(self) -> None:
        label = self.selected_label()
        if not label or not self.selected_element_ids:
            return
        labels_by_id = {item.id: item for item in self.labels}
        selected = set(self.selected_element_ids)
        child_ids = [
            child_id
            for child_id in label.label_ids
            if resolved_label_elements(child_id, labels_by_id).issubset(selected)
        ]
        nested_elements: set[str] = set()
        for child_id in child_ids:
            nested_elements.update(resolved_label_elements(child_id, labels_by_id))
        members = sorted(selected - nested_elements)
        if (members, child_ids) == (label.element_ids, label.label_ids):
            return
        self.checkpoint()
        label.element_ids = members
        label.label_ids = child_ids
        self.sync_label_list()
        self.redraw()

    @staticmethod
    def choice_id(value: str) -> str:
        return value.rsplit("[", 1)[-1].rstrip("]") if "[" in value else ""

    def refresh_rig_choices(self) -> None:
        selected_joint = self.selected_joint()
        parent_values = ["Nenhuma"] + [
            f"{joint.name} [{joint.id}]"
            for joint in self.joints
            if joint.kind == "joint" and joint.id != self.selected_joint_id
        ]
        attachment_values = ["Nenhum"] + [
            f"{(element.glyph or 'glyph')[:12]} [{element.id}]" for element in self.elements
        ]
        self.parent_combo.configure(values=parent_values)
        self.part_a_combo.configure(values=attachment_values)
        self.part_b_combo.configure(values=attachment_values)
        self.attention_attachment_combo.configure(values=attachment_values)
        if selected_joint:
            parent = next((value for value in parent_values if value.endswith(f"[{selected_joint.parent_id}]")), "Nenhuma")
            part_a = next(
                (value for value in attachment_values if value.endswith(f"[{selected_joint.part_a_element_id}]")),
                "Nenhum",
            )
            part_b = next(
                (value for value in attachment_values if value.endswith(f"[{selected_joint.part_b_element_id}]")),
                "Nenhum",
            )
            attachment = next(
                (value for value in attachment_values if value.endswith(f"[{selected_joint.attached_element_id}]")),
                "Nenhum",
            )
            self.joint_vars["parent"].set(parent)
            self.joint_vars["part_a"].set(part_a)
            self.joint_vars["part_b"].set(part_b)
            self.attention_vars["attachment"].set(attachment)

    def show_piece_form(self) -> None:
        """Mostra o formulario do que esta selecionado, e o resto nenhum.

        Tambem acende ou apaga os botoes que dependem da selecao: prender e
        ligar so funcionam com uma combinacao exata de pecas e pontos, e antes
        disto so davam para descobrir errando e lendo a barra de status.
        """
        for widget in (self.glyph_form, self.rig_tab, self.attention_tab):
            widget.pack_forget()
        joint = self.selected_joint()
        glyphs = len(self.selected_element_ids)
        points = len(self.selected_joint_ids)
        if joint and joint.kind == "attention":
            self.attention_tab.pack(fill=tk.BOTH, expand=True)
            hint = f"Ponto de atencao: {joint.name}"
        elif joint:
            self.rig_tab.pack(fill=tk.BOTH, expand=True)
            hint = f"Articulacao: {joint.name}"
        elif self.selected():
            self.glyph_form.pack(fill=tk.BOTH, expand=True)
            hint = "Glyph selecionado"
        else:
            hint = "Nada selecionado. Clique numa peca do canvas."
        if glyphs + points > 1:
            hint += f"   ({glyphs} glyph(s), {points} ponto(s) na selecao)"
        self.piece_hint.configure(text=hint)

        if hasattr(self, "span_button"):
            # Com zero pontos o botao so serve para desfazer, entao ele so
            # acende se houver o que desfazer: um botao aceso que nao faz nada
            # ensina a ignorar os botoes acesos.
            piece = next(
                (item for item in self.elements if item.id in self.selected_element_ids), None
            )
            single = glyphs == 1 and piece is not None
            self.span_button.state(
                ["!disabled" if single and (points == 2 or (points == 0 and piece.span)) else "disabled"]
            )
            self.carry_button.state(
                ["!disabled" if single and (points == 1 or (points == 0 and piece.follow)) else "disabled"]
            )

    def sync_inspector(self) -> None:
        element = self.selected()
        joint = self.selected_joint()
        if element:
            for key in (
                "glyph", "x", "y", "font_size", "scale_x", "scale_y", "flip_x", "flip_y",
                "rotation", "layer", "font_path",
            ):
                self.vars[key].set(getattr(element, key))
            self.vars["role"].set(
                {"body": "corpo", "limb": "membro", "": "(cor propria)"}.get(
                    element.role, element.role
                )
            )
            self.color_button.configure(bg=element.color)
            self.font_label.configure(
                text=Path(element.font_path).name if element.font_path else "Fonte padrao do Pillow"
            )
            names = {joint.id: joint.name for joint in self.joints}
            if element.span:
                bound = "Segmento entre " + " e ".join(
                    names.get(joint_id, "?") for joint_id in element.span
                )
            elif element.follow:
                offset = element.offset if len(element.offset) == 2 else [0.0, 0.0]
                bound = (
                    f"Presa a {names.get(element.follow, '?')}, a "
                    f"({offset[0]:g}, {offset[1]:g}). Arrastar muda a empunhadura."
                )
            else:
                bound = "Peca livre: posicao, giro e escala sao dela."
            self.span_label.configure(text=bound)
        else:
            self.vars["glyph"].set("")
            self.vars["flip_x"].set(False)
            self.vars["flip_y"].set(False)
            self.color_button.configure(bg="#d9d9d9")
            self.font_label.configure(text="Nenhum elemento selecionado")
            self.span_label.configure(text="")
        if joint and joint.kind == "joint":
            self.joint_vars["name"].set(joint.name)
            self.joint_vars["x"].set(joint.x)
            self.joint_vars["y"].set(joint.y)
            self.joint_vars["fixed"].set(joint.fixed)
            self.joint_vars["constraint"].set(
                "Fixa (solda as pecas)" if joint.constraint_type == "fixed" else "Pivo (permite girar)"
            )
            self.joint_color_button.configure(bg=joint.color)
            self.joint_description.delete("1.0", tk.END)
            self.joint_description.insert("1.0", joint.description)
            self.refresh_rig_choices()
        else:
            self.joint_vars["name"].set("")
            self.joint_vars["parent"].set("Nenhuma")
            self.joint_vars["part_a"].set("Nenhum")
            self.joint_vars["part_b"].set("Nenhum")
            self.joint_color_button.configure(bg="#d9d9d9")
            self.joint_description.delete("1.0", tk.END)
        if joint and joint.kind == "attention":
            self.attention_vars["name"].set(joint.name)
            self.attention_vars["x"].set(joint.x)
            self.attention_vars["y"].set(joint.y)
            self.attention_color_button.configure(bg=joint.color)
            self.attention_description.delete("1.0", tk.END)
            self.attention_description.insert("1.0", joint.description)
            self.refresh_rig_choices()
        else:
            self.attention_vars["name"].set("")
            self.attention_vars["attachment"].set("Nenhum")
            self.attention_color_button.configure(bg="#d9d9d9")
            self.attention_description.delete("1.0", tk.END)
        self.show_piece_form()
        self.sync_outline()

    def role_value(self) -> str:
        """O papel escrito na aba, ja no vocabulario do arquivo."""
        typed = self.vars["role"].get().strip()
        return {"corpo": "body", "membro": "limb", "(cor propria)": ""}.get(typed, typed)

    def carry_element(self) -> None:
        """Prende a peca selecionada ao ponto selecionado, ou a solta.

        A distancia atual vira a empunhadura, entao prender nao move nada: a
        peca fica onde esta e passa a acompanhar o ponto dali em diante.
        """
        chosen_elements = [
            element for element in self.elements if element.id in self.selected_element_ids
        ]
        if len(chosen_elements) != 1:
            self.status.set("Selecione exatamente uma peca para prender")
            return
        element = chosen_elements[0]
        if element.span:
            self.status.set("Um segmento ja e preso pelos dois pontos; solte-o antes")
            return
        chosen = [joint.id for joint in self.joints if joint.id in self.selected_joint_ids]
        if not chosen and element.follow:
            self.checkpoint()
            element.follow = ""
            element.offset = []
            self.redraw()
            self.sync_inspector()
            self.status.set("Peca solta: ela volta a ter posicao propria")
            return
        if len(chosen) != 1:
            self.status.set("Selecione a peca e exatamente um ponto (Shift+clique)")
            return
        carrier = next(joint for joint in self.joints if joint.id == chosen[0])
        self.checkpoint()
        element.follow = carrier.id
        element.offset = [round(element.x - carrier.x, 4), round(element.y - carrier.y, 4)]
        self.redraw()
        self.sync_inspector()
        self.status.set(f"Peca presa a {carrier.name}: mover o ponto leva ela junto")

    def link_span(self) -> None:
        """Transforma o glyph selecionado num segmento entre dois pontos.

        Com nenhum ponto selecionado, desfaz a ligacao e devolve a peca ao
        controle manual -- e o mesmo botao, porque e a mesma decisao.
        """
        # Sai de `selected_element_ids`, e nao de `selected()`: assim que se
        # marca um ponto ele vira a peca principal, e a peca principal deixaria
        # de ser o glyph exatamente quando o botao e usado.
        chosen_elements = [
            element for element in self.elements if element.id in self.selected_element_ids
        ]
        if len(chosen_elements) != 1:
            self.status.set("Selecione exatamente um glyph para virar segmento")
            return
        element = chosen_elements[0]
        chosen = [joint.id for joint in self.joints if joint.id in self.selected_joint_ids]
        if not chosen and element.span:
            self.checkpoint()
            element.span = []
            self.redraw()
            self.sync_inspector()
            self.status.set("Segmento solto: a peca volta a ter posicao propria")
            return
        if len(chosen) != 2:
            self.status.set("Selecione o glyph e exatamente dois pontos (Shift+clique)")
            return
        self.checkpoint()
        element.span = chosen
        self.redraw()
        self.sync_inspector()
        self.status.set("Segmento ligado: arraste os pontos para dobrar")

    def add_glyph(self) -> None:
        self.checkpoint()
        element = Glyph.create(
            self.snap_value(self.canvas_width / 2),
            self.snap_value(self.canvas_height / 2),
            self.font_path,
        )
        element.layer = max((item.layer for item in self.elements), default=-1) + 1
        self.elements.append(element)
        self.clear_selection()
        self.select_element(element.id)
        self.redraw()
        self.sync_inspector()
        self.status.set("Glyph adicionado")

    def add_joint(self) -> None:
        selected_parts = [
            element for element in self.elements if element.id in self.selected_element_ids
        ]
        self.checkpoint()
        if len(selected_parts) >= 2:
            x = self.snap_value((selected_parts[0].x + selected_parts[1].x) / 2)
            y = self.snap_value((selected_parts[0].y + selected_parts[1].y) / 2)
        elif selected_parts:
            x, y = selected_parts[0].x, selected_parts[0].y
        else:
            x = self.snap_value(self.canvas_width / 2)
            y = self.snap_value(self.canvas_height / 2)
        joint = Joint.create(x, y, sum(item.kind == "joint" for item in self.joints) + 1)
        if selected_parts:
            joint.part_a_element_id = selected_parts[0].id
        if len(selected_parts) >= 2:
            joint.part_b_element_id = selected_parts[1].id
        self.joints.append(joint)
        self.clear_selection()
        self.select_joint(joint.id)
        self.show_rig.set(True)
        self.redraw()
        self.sync_inspector()
        self.status.set("Articulacao adicionada")

    def add_attention_point(self) -> None:
        attached = self.selected()
        self.checkpoint()
        x = attached.x if attached else self.snap_value(self.canvas_width / 2)
        y = attached.y if attached else self.snap_value(self.canvas_height / 2)
        point = Joint.create_attention(
            x,
            y,
            sum(item.kind == "attention" for item in self.joints) + 1,
        )
        if attached:
            point.attached_element_id = attached.id
        self.joints.append(point)
        self.select_joint(point.id)
        self.show_rig.set(True)
        self.redraw()
        self.sync_inspector()
        self.status.set("Ponto de atencao adicionado")

    def duplicate_selected(self) -> None:
        piece = self.selection_piece()
        if piece:
            self.paste_piece(piece, offset=20)

    def selection_piece(self) -> dict | None:
        element_ids = set(self.selected_element_ids)
        joint_ids = set(self.selected_joint_ids)
        # Um segmento sem as duas pontas nao e um segmento: os pontos dele vao
        # junto, sempre.
        for element in self.elements:
            if element.id in element_ids:
                joint_ids.update(element.span)
                if element.follow:
                    joint_ids.add(element.follow)
        for joint in self.joints:
            if joint.kind == "attention" and joint.attached_element_id in element_ids:
                joint_ids.add(joint.id)
            elif joint.kind == "joint":
                linked = {
                    value
                    for value in (joint.part_a_element_id, joint.part_b_element_id)
                    if value
                }
                if linked and linked.issubset(element_ids):
                    joint_ids.add(joint.id)
        if not element_ids and not joint_ids:
            return None
        elements = [asdict(element) for element in self.elements if element.id in element_ids]
        for value in elements:
            span = [joint_id for joint_id in value.get("span", []) if joint_id in joint_ids]
            value["span"] = span if len(span) == 2 else []
            if value.get("follow") not in joint_ids:
                value["follow"], value["offset"] = "", []
        labels_by_id = {label.id: label for label in self.labels}
        included_label_ids = {
            label.id
            for label in self.labels
            if resolved_label_elements(label.id, labels_by_id)
            and resolved_label_elements(label.id, labels_by_id).issubset(element_ids)
        }
        labels = []
        for label in self.labels:
            if label.id not in included_label_ids:
                continue
            value = asdict(label)
            value["label_ids"] = [
                child_id for child_id in label.label_ids if child_id in included_label_ids
            ]
            labels.append(value)
        joints = [asdict(joint) for joint in self.joints if joint.id in joint_ids]
        for joint in joints:
            if joint["parent_id"] not in joint_ids:
                joint["parent_id"] = ""
            if joint["attached_element_id"] not in element_ids:
                joint["attached_element_id"] = ""
            if joint["part_a_element_id"] not in element_ids:
                joint["part_a_element_id"] = ""
            if joint["part_b_element_id"] not in element_ids:
                joint["part_b_element_id"] = ""
        return {
            "app": APP_NAME,
            "version": PROJECT_VERSION,
            "kind": "glyph_forge_piece",
            "default_font": {
                "kind": "bitmap_rom",
                "asset": "assets/fonts/ibm_vga_8x16.bin",
                "encoding": "CP437",
                "glyph_size": [8, 16],
            },
            "elements": elements,
            "rig": {"joints": [joint for joint in joints if joint.get("kind") == "joint"]},
            "attention_points": [
                joint for joint in joints if joint.get("kind") == "attention"
            ],
            "labels": labels,
        }

    def paste_piece(self, piece: dict, offset: float = 20, checkpoint: bool = True) -> bool:
        element_values = piece.get("elements", [])
        joint_values = piece.get("rig", {}).get("joints", piece.get("joints", [])) + piece.get(
            "attention_points", []
        )
        label_values = piece.get("labels", [])
        if not element_values and not joint_values:
            return False
        if checkpoint:
            self.checkpoint()
        element_ids: dict[str, str] = {}
        joint_ids: dict[str, str] = {}
        label_ids: dict[str, str] = {}
        new_elements: list[Glyph] = []
        new_joints: list[Joint] = []
        new_labels: list[SemanticLabel] = []
        for value in element_values:
            element = Glyph.from_dict(value)
            old_id = element.id
            element.id = uuid.uuid4().hex[:10]
            element.x += offset
            element.y += offset
            if element.font_path.lower().endswith("ibm_vga_8x16.bin") and not Path(element.font_path).is_file():
                element.font_path = default_font_path()
            element_ids[old_id] = element.id
            new_elements.append(element)
        for value in joint_values:
            joint = Joint.from_dict(value)
            old_id = joint.id
            joint.id = uuid.uuid4().hex[:10]
            joint.x += offset
            joint.y += offset
            joint_ids[old_id] = joint.id
            new_joints.append(joint)
        for joint in new_joints:
            joint.parent_id = joint_ids.get(joint.parent_id, "")
            joint.attached_element_id = element_ids.get(joint.attached_element_id, "")
            joint.part_a_element_id = element_ids.get(joint.part_a_element_id, "")
            joint.part_b_element_id = element_ids.get(joint.part_b_element_id, "")
        for element in new_elements:
            span = [joint_ids[old] for old in element.span if old in joint_ids]
            element.span = span if len(span) == 2 else []
            element.follow = joint_ids.get(element.follow, "")
            if not element.follow:
                element.offset = []
        for value in label_values:
            old_id = str(value.get("id", ""))
            if old_id:
                label_ids[old_id] = uuid.uuid4().hex[:10]
        for value in label_values:
            label = SemanticLabel.from_dict(value)
            label.id = label_ids.get(label.id, uuid.uuid4().hex[:10])
            label.element_ids = [
                element_ids[element_id]
                for element_id in label.element_ids
                if element_id in element_ids
            ]
            label.label_ids = [
                label_ids[child_id] for child_id in label.label_ids if child_id in label_ids
            ]
            if label.element_ids or label.label_ids:
                new_labels.append(label)
        new_labels_by_id = {label.id: label for label in new_labels}
        for label in new_labels:
            label.label_ids = [
                child_id
                for child_id in label.label_ids
                if child_id != label.id
                and not label_reaches(child_id, label.id, new_labels_by_id)
            ]
        self.elements.extend(new_elements)
        self.joints.extend(new_joints)
        self.labels.extend(new_labels)
        self.clear_selection()
        self.selected_element_ids.update(element.id for element in new_elements)
        self.selected_joint_ids.update(joint.id for joint in new_joints)
        if new_elements:
            self.selected_id = new_elements[-1].id
        elif new_joints:
            self.selected_joint_id = new_joints[-1].id
        self.redraw()
        self.sync_inspector()
        self.sync_label_list()
        return True

    def copy_selected(self) -> None:
        piece = self.selection_piece()
        if not piece:
            return
        payload = json.dumps(piece, ensure_ascii=False, indent=2)
        self.root.clipboard_clear()
        self.root.clipboard_append(payload)
        self.status.set("Selecao copiada")

    def paste_clipboard(self) -> None:
        try:
            piece = json.loads(self.root.clipboard_get())
        except (tk.TclError, json.JSONDecodeError, TypeError):
            messagebox.showerror(APP_NAME, "A area de transferencia nao contem uma peca do Glyph Forge.")
            return
        if not self.paste_piece(piece):
            messagebox.showerror(APP_NAME, "A peca copiada esta vazia ou e invalida.")

    def export_selection(self) -> None:
        piece = self.selection_piece()
        if not piece:
            messagebox.showinfo(APP_NAME, "Selecione ao menos um glyph ou articulacao.")
            return
        path = filedialog.asksaveasfilename(
            title="Exportar selecao",
            defaultextension=".glyph-piece.json",
            filetypes=(("Peca Glyph Forge", "*.glyph-piece.json"), ("JSON", "*.json")),
        )
        if not path:
            return
        try:
            Path(path).write_text(
                json.dumps(portable_scene(piece), ensure_ascii=False, indent=2), encoding="utf-8"
            )
        except OSError as exc:
            messagebox.showerror(APP_NAME, f"Nao foi possivel exportar a selecao:\n{exc}")
            return
        self.status.set(f"Selecao exportada: {path}")

    def import_piece(self) -> None:
        path = filedialog.askopenfilename(
            title="Importar peca, cena ou arte ASCII",
            filetypes=(
                ("Glyph Forge / ASCII", "*.json *.txt *.asc"),
                ("Glyph Forge JSON", "*.json"),
                ("Arte ASCII", "*.txt *.asc"),
                ("Todos os arquivos", "*.*"),
            ),
        )
        if not path:
            return
        if Path(path).suffix.lower() in {".txt", ".asc"}:
            self.import_ascii_art(Path(path))
            return
        try:
            piece = json.loads(Path(path).read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError, TypeError) as exc:
            messagebox.showerror(APP_NAME, f"Nao foi possivel importar:\n{exc}")
            return
        if not self.paste_piece(piece):
            messagebox.showerror(APP_NAME, "O arquivo nao contem glyphs ou articulacoes.")
            return
        self.status.set(f"Importado: {path}")

    def import_ascii_art(self, path: Path) -> None:
        try:
            raw = path.read_bytes()
            try:
                content = raw.decode("utf-8")
            except UnicodeDecodeError:
                content = raw.decode("cp437")
        except OSError as exc:
            messagebox.showerror(APP_NAME, f"Nao foi possivel importar a arte ASCII:\n{exc}")
            return
        content = content.replace("\r\n", "\n").replace("\r", "\n").rstrip("\n")
        if not content:
            messagebox.showerror(APP_NAME, "O arquivo ASCII esta vazio.")
            return
        self.checkpoint()
        element = Glyph.create(
            self.snap_value(self.canvas_width / 2),
            self.snap_value(self.canvas_height / 2),
            default_font_path(),
        )
        element.glyph = content.expandtabs(4)
        element.font_size = 16
        element.layer = max((item.layer for item in self.elements), default=-1) + 1
        self.elements.append(element)
        self.select_element(element.id)
        self.redraw()
        self.sync_inspector()
        self.status.set(f"Arte ASCII importada: {path}")

    def delete_selected(self) -> None:
        element_ids = set(self.selected_element_ids)
        joint_ids = set(self.selected_joint_ids)
        if not element_ids and not joint_ids:
            return
        self.checkpoint()
        self.elements = [item for item in self.elements if item.id not in element_ids]
        for label in self.labels:
            label.element_ids = [element_id for element_id in label.element_ids if element_id not in element_ids]
        while True:
            valid_label_ids = {label.id for label in self.labels}
            for label in self.labels:
                label.label_ids = [
                    child_id for child_id in label.label_ids if child_id in valid_label_ids
                ]
            empty_ids = {
                label.id for label in self.labels if not label.element_ids and not label.label_ids
            }
            if not empty_ids:
                break
            self.labels = [label for label in self.labels if label.id not in empty_ids]
        if not self.selected_label():
            self.selected_label_id = None
        self.joints = [item for item in self.joints if item.id not in joint_ids]
        for item in self.joints:
            if item.attached_element_id in element_ids:
                item.attached_element_id = ""
            if item.part_a_element_id in element_ids:
                item.part_a_element_id = ""
            if item.part_b_element_id in element_ids:
                item.part_b_element_id = ""
            if item.parent_id in joint_ids:
                item.parent_id = ""
        self.clear_selection()
        self.prune_animation()
        self.redraw()
        self.sync_inspector()
        self.sync_label_list()
        self.sync_animation_lists()

    def apply_properties(self) -> None:
        element = self.selected()
        if not element:
            return
        try:
            values = {
                "glyph": self.vars["glyph"].get(),
                "x": self.snap_value(float(self.vars["x"].get())),
                "y": self.snap_value(float(self.vars["y"].get())),
                "font_size": max(1, int(self.vars["font_size"].get())),
                "scale_x": max(0.02, float(self.vars["scale_x"].get())),
                "scale_y": max(0.02, float(self.vars["scale_y"].get())),
                "flip_x": bool(self.vars["flip_x"].get()),
                "flip_y": bool(self.vars["flip_y"].get()),
                "rotation": float(self.vars["rotation"].get()),
                "layer": int(self.vars["layer"].get()),
                "font_path": self.vars["font_path"].get(),
                "role": self.role_value(),
            }
        except (tk.TclError, ValueError):
            messagebox.showerror(APP_NAME, "Alguma propriedade possui um numero invalido.")
            return
        if all(getattr(element, key) == value for key, value in values.items()):
            return
        self.checkpoint()
        delta_x = values["x"] - element.x
        delta_y = values["y"] - element.y
        for key, value in values.items():
            setattr(element, key, value)
        # Numa peca carregada, X e Y sao o lugar onde ela esta, e nao o que a
        # coloca ali: digitar um deles tem que virar empunhadura.
        if element.follow:
            element.x, element.y = element.x - delta_x, element.y - delta_y
            self.shift_element(element, delta_x, delta_y)
        for joint in self.joints:
            if joint.kind == "attention" and joint.attached_element_id == element.id:
                joint.x += delta_x
                joint.y += delta_y
        self.font_path = element.font_path
        self.redraw()
        self.sync_inspector()

    def apply_joint_properties(self) -> None:
        joint = self.selected_joint()
        if not joint or joint.kind != "joint":
            return
        try:
            parent_id = self.choice_id(self.joint_vars["parent"].get())
            part_a_id = self.choice_id(self.joint_vars["part_a"].get())
            part_b_id = self.choice_id(self.joint_vars["part_b"].get())
            values = {
                "name": self.joint_vars["name"].get().strip() or joint.id,
                "x": self.snap_value(float(self.joint_vars["x"].get())),
                "y": self.snap_value(float(self.joint_vars["y"].get())),
                "parent_id": parent_id,
                "attached_element_id": "",
                "part_a_element_id": part_a_id,
                "part_b_element_id": part_b_id,
                "constraint_type": (
                    "fixed"
                    if self.joint_vars["constraint"].get().startswith("Fixa")
                    else "pivot"
                ),
                "fixed": bool(self.joint_vars["fixed"].get()),
                "description": self.joint_description.get("1.0", "end-1c").strip(),
            }
        except (tk.TclError, ValueError):
            messagebox.showerror(APP_NAME, "A articulacao possui uma coordenada invalida.")
            return
        if part_a_id and part_a_id == part_b_id:
            messagebox.showerror(APP_NAME, "Peca A e Peca B precisam ser diferentes.")
            return
        joints_by_id = {item.id: item for item in self.joints if item.kind == "joint"}
        cursor = parent_id
        while cursor:
            if cursor == joint.id:
                messagebox.showerror(APP_NAME, "Essa relacao criaria um ciclo no esqueleto.")
                return
            cursor = joints_by_id.get(cursor).parent_id if joints_by_id.get(cursor) else ""
        if all(getattr(joint, key) == value for key, value in values.items()):
            return
        self.checkpoint()
        for key, value in values.items():
            setattr(joint, key, value)
        self.redraw()
        self.sync_inspector()

    def apply_attention_properties(self) -> None:
        point = self.selected_joint()
        if not point or point.kind != "attention":
            return
        try:
            values = {
                "name": self.attention_vars["name"].get().strip() or point.id,
                "x": self.snap_value(float(self.attention_vars["x"].get())),
                "y": self.snap_value(float(self.attention_vars["y"].get())),
                "attached_element_id": self.choice_id(self.attention_vars["attachment"].get()),
                "description": self.attention_description.get("1.0", "end-1c").strip(),
            }
        except (tk.TclError, ValueError):
            messagebox.showerror(APP_NAME, "O ponto de atencao possui uma coordenada invalida.")
            return
        if all(getattr(point, key) == value for key, value in values.items()):
            return
        self.checkpoint()
        for key, value in values.items():
            setattr(point, key, value)
        self.redraw()
        self.sync_inspector()

    def choose_joint_color(self) -> None:
        self.open_live_color_picker("joint")

    def choose_attention_color(self) -> None:
        self.open_live_color_picker("attention")

    def choose_color(self) -> None:
        self.open_live_color_picker("glyph")

    def open_live_color_picker(self, target: str) -> None:
        if self.color_picker and self.color_picker.winfo_exists():
            self.color_picker.destroy()
        if target == "glyph":
            target_ids = set(self.selected_element_ids)
            selected = self.selected()
            if not target_ids or not selected:
                return
            initial = selected.color
            title = "Cor dos glyphs"
        elif target == "joint":
            target_ids = {
                point.id
                for point in self.joints
                if point.id in self.selected_joint_ids and point.kind == "joint"
            }
            selected_joint = self.selected_joint()
            if not target_ids or not selected_joint or selected_joint.kind != "joint":
                return
            initial = selected_joint.color
            title = "Cor das articulacoes"
        elif target == "attention":
            target_ids = {
                point.id
                for point in self.joints
                if point.id in self.selected_joint_ids and point.kind == "attention"
            }
            selected_attention = self.selected_joint()
            if not target_ids or not selected_attention or selected_attention.kind != "attention":
                return
            initial = selected_attention.color
            title = "Cor dos pontos de atencao"
        elif target == "accent":
            target_ids = set()
            initial = self.accent
            title = "Cor do acento"
        elif target.startswith("skin:"):
            skin = self.current_skin()
            if skin is None:
                return
            target_ids = set()
            initial = getattr(skin, target.removeprefix("skin:"))
            title = f"Cor da pele: {skin.name}"
        else:
            target_ids = set()
            initial = self.background
            title = "Cor do fundo"

        red, green, blue = ImageColor.getrgb(initial)[:3]
        hue, saturation, value = colorsys.rgb_to_hsv(red / 255, green / 255, blue / 255)
        picker = tk.Toplevel(self.root)
        picker.title(title)
        picker.geometry("390x390")
        picker.resizable(False, False)
        picker.transient(self.root)
        self.color_picker = picker
        changed = {"value": False}
        hsv = {"h": hue, "s": saturation, "v": value}
        hex_value = tk.StringVar(value=initial)
        size = 256

        body = ttk.Frame(picker, padding=12)
        body.pack(fill=tk.BOTH, expand=True)
        sv_canvas = tk.Canvas(
            body,
            width=size,
            height=size,
            highlightthickness=1,
            highlightbackground="#555555",
            cursor="crosshair",
        )
        sv_canvas.grid(row=0, column=0, sticky=tk.NW)
        hue_canvas = tk.Canvas(
            body,
            width=30,
            height=size,
            highlightthickness=1,
            highlightbackground="#555555",
            cursor="sb_v_double_arrow",
        )
        hue_canvas.grid(row=0, column=1, sticky=tk.NW, padx=(10, 0))
        preview = tk.Label(body, bg=initial, width=7, relief=tk.SUNKEN)
        preview.grid(row=0, column=2, sticky="nsew", padx=(10, 0))

        hue_image = Image.new("RGB", (1, size))
        hue_image.putdata(
            [
                tuple(round(channel * 255) for channel in colorsys.hsv_to_rgb(y / (size - 1), 1, 1))
                for y in range(size)
            ]
        )
        hue_image = hue_image.resize((30, size), Image.Resampling.NEAREST)
        picker.hue_photo = ImageTk.PhotoImage(hue_image)
        hue_canvas.create_image(0, 0, image=picker.hue_photo, anchor=tk.NW)

        def render_sv_square() -> None:
            hue_rgb = tuple(
                round(channel * 255) for channel in colorsys.hsv_to_rgb(hsv["h"], 1, 1)
            )
            saturation_row = Image.new("RGB", (size, 1))
            saturation_row.putdata(
                [
                    tuple(round(255 + (channel - 255) * x / (size - 1)) for channel in hue_rgb)
                    for x in range(size)
                ]
            )
            saturation_image = saturation_row.resize((size, size), Image.Resampling.NEAREST)
            value_column = Image.new("L", (1, size))
            value_column.putdata([round(255 * (1 - y / (size - 1))) for y in range(size)])
            value_image = value_column.resize((size, size), Image.Resampling.NEAREST)
            value_rgb = Image.merge("RGB", (value_image, value_image, value_image))
            picker.sv_photo = ImageTk.PhotoImage(ImageChops.multiply(saturation_image, value_rgb))
            sv_canvas.delete("gradient")
            sv_canvas.create_image(0, 0, image=picker.sv_photo, anchor=tk.NW, tags=("gradient",))
            sv_canvas.tag_lower("gradient")

        def draw_markers() -> None:
            sv_canvas.delete("marker")
            marker_x = hsv["s"] * (size - 1)
            marker_y = (1 - hsv["v"]) * (size - 1)
            sv_canvas.create_oval(
                marker_x - 7,
                marker_y - 7,
                marker_x + 7,
                marker_y + 7,
                outline="#000000",
                width=3,
                tags=("marker",),
            )
            sv_canvas.create_oval(
                marker_x - 6,
                marker_y - 6,
                marker_x + 6,
                marker_y + 6,
                outline="#ffffff",
                width=2,
                tags=("marker",),
            )
            hue_canvas.delete("marker")
            marker_hue = hsv["h"] * (size - 1)
            hue_canvas.create_rectangle(
                -1,
                marker_hue - 3,
                31,
                marker_hue + 3,
                outline="#ffffff",
                width=2,
                tags=("marker",),
            )

        def update_color() -> None:
            rgb = tuple(
                round(channel * 255)
                for channel in colorsys.hsv_to_rgb(hsv["h"], hsv["s"], hsv["v"])
            )
            color = "#{:02x}{:02x}{:02x}".format(*rgb)
            if not changed["value"]:
                self.checkpoint()
                changed["value"] = True
            if target == "glyph":
                for element in self.elements:
                    if element.id in target_ids:
                        element.color = color
            elif target == "joint":
                for joint in self.joints:
                    if joint.id in target_ids and joint.kind == "joint":
                        joint.color = color
            elif target == "attention":
                for point in self.joints:
                    if point.id in target_ids and point.kind == "attention":
                        point.color = color
            elif target == "accent":
                self.accent = color
            elif target.startswith("skin:"):
                skin = self.current_skin()
                if skin:
                    setattr(skin, target.removeprefix("skin:"), color)
                    self.sync_skin_list()
            else:
                self.background = color
            hex_value.set(color)
            preview.configure(bg=color)
            self.redraw()
            self.sync_inspector()

        def choose_sv(event: tk.Event) -> None:
            hsv["s"] = max(0.0, min(1.0, event.x / (size - 1)))
            hsv["v"] = 1 - max(0.0, min(1.0, event.y / (size - 1)))
            draw_markers()
            update_color()

        def choose_hue(event: tk.Event) -> None:
            hsv["h"] = max(0.0, min(1.0, event.y / (size - 1)))
            render_sv_square()
            draw_markers()
            update_color()

        sv_canvas.bind("<Button-1>", choose_sv)
        sv_canvas.bind("<B1-Motion>", choose_sv)
        hue_canvas.bind("<Button-1>", choose_hue)
        hue_canvas.bind("<B1-Motion>", choose_hue)

        render_sv_square()
        draw_markers()

        footer = ttk.Frame(picker, padding=10)
        footer.pack(fill=tk.X)
        ttk.Label(footer, text="Hex").pack(side=tk.LEFT)
        entry = ttk.Entry(footer, textvariable=hex_value, width=10)
        entry.pack(side=tk.LEFT, padx=6)

        def apply_hex(_event: tk.Event | None = None) -> None:
            try:
                rgb = ImageColor.getrgb(hex_value.get())[:3]
            except ValueError:
                return
            hsv["h"], hsv["s"], hsv["v"] = colorsys.rgb_to_hsv(*(value / 255 for value in rgb))
            render_sv_square()
            draw_markers()
            update_color()

        entry.bind("<Return>", apply_hex)
        ttk.Button(footer, text="Fechar", command=picker.destroy).pack(side=tk.RIGHT)

        swatches = ttk.Frame(picker, padding=(10, 0, 10, 10))
        swatches.pack(fill=tk.X)

        def choose_swatch(color: str) -> None:
            rgb = ImageColor.getrgb(color)[:3]
            hsv["h"], hsv["s"], hsv["v"] = colorsys.rgb_to_hsv(*(value / 255 for value in rgb))
            render_sv_square()
            draw_markers()
            update_color()

        for color in (
            "#ffffff", "#b8bec9", "#181818", "#ff4057", "#ff8c42", "#ffd43b",
            "#57e389", "#3fd8ff", "#4d7cff", "#a970ff", "#ff65c3", "#8b5a2b",
        ):
            tk.Button(
                swatches,
                bg=color,
                activebackground=color,
                width=2,
                relief=tk.FLAT,
                command=lambda selected_color=color: choose_swatch(selected_color),
            ).pack(side=tk.LEFT, expand=True, fill=tk.X, padx=1)
        picker.protocol("WM_DELETE_WINDOW", picker.destroy)
        picker.focus_set()

    def open_glyph_table(self) -> None:
        if self.glyph_palette and self.glyph_palette.winfo_exists():
            self.glyph_palette.deiconify()
            self.glyph_palette.lift()
            self.glyph_palette.focus_set()
            return
        font_path = default_font_path()
        if not read_rom(font_path):
            messagebox.showerror(APP_NAME, "A fonte assets/fonts/ibm_vga_8x16.bin nao foi encontrada.")
            return
        modal = tk.Toplevel(self.root)
        modal.title("Tabela de glyphs CP437")
        modal.geometry("760x650")
        modal.minsize(560, 420)
        self.glyph_palette = modal
        modal.bind("<FocusOut>", lambda _event: modal.after(100, self.hide_inactive_glyph_table))

        header = ttk.Frame(modal, padding=8)
        header.pack(fill=tk.X)
        ttk.Label(
            header,
            text="Clique em um glyph para usa-lo na selecao.",
            font=("TkDefaultFont", 11, "bold"),
        ).pack(side=tk.LEFT)
        append = tk.BooleanVar(value=False)
        ttk.Checkbutton(header, text="Acrescentar ao texto", variable=append).pack(side=tk.RIGHT)

        holder = ttk.Frame(modal)
        holder.pack(fill=tk.BOTH, expand=True, padx=8, pady=(0, 8))
        table = tk.Canvas(holder, bg="#202020", highlightthickness=0)
        scroll = ttk.Scrollbar(holder, orient=tk.VERTICAL, command=table.yview)
        table.configure(yscrollcommand=scroll.set)
        table.pack(side=tk.LEFT, fill=tk.BOTH, expand=True)
        scroll.pack(side=tk.RIGHT, fill=tk.Y)
        grid = tk.Frame(table, bg="#202020")
        window = table.create_window((0, 0), window=grid, anchor=tk.NW)

        modal.glyph_photos = []
        for code in range(256):
            photo = ImageTk.PhotoImage(rom_code_image(code, font_path, scale=3))
            modal.glyph_photos.append(photo)
            button = tk.Button(
                grid,
                image=photo,
                text=f"{code:02X}",
                compound=tk.TOP,
                bg="#303030",
                fg="#dddddd",
                activebackground="#4a4a4a",
                activeforeground="#ffffff",
                relief=tk.FLAT,
                command=lambda value=code: self.choose_table_glyph(value, append.get(), modal),
            )
            button.grid(row=code // 16, column=code % 16, padx=2, pady=2, sticky="nsew")
        for column in range(16):
            grid.columnconfigure(column, weight=1)

        def update_scrollregion(_event: tk.Event | None = None) -> None:
            table.configure(scrollregion=table.bbox("all"))
            table.itemconfigure(window, width=max(table.winfo_width(), grid.winfo_reqwidth()))

        grid.bind("<Configure>", update_scrollregion)
        table.bind("<Configure>", update_scrollregion)
        modal.bind(
            "<MouseWheel>",
            lambda event: table.yview_scroll(-1 * int(event.delta / 120), "units"),
        )
        modal.glyph_table = table
        modal.protocol("WM_DELETE_WINDOW", self.close_glyph_table)
        modal.focus_set()

    def close_glyph_table(self) -> None:
        if self.glyph_palette and self.glyph_palette.winfo_exists():
            self.glyph_palette.destroy()
        self.glyph_palette = None

    def hide_inactive_glyph_table(self) -> None:
        modal = self.glyph_palette
        if not modal or not modal.winfo_exists() or modal.state() == "iconic":
            return
        focused = self.root.focus_get()
        widget = focused
        while widget is not None:
            if widget == modal:
                return
            widget = getattr(widget, "master", None)
        modal.iconify()

    def choose_table_glyph(self, code: int, append: bool, modal: tk.Toplevel) -> None:
        targets = [element for element in self.elements if element.id in self.selected_element_ids]
        if targets:
            self.checkpoint()
        else:
            self.add_glyph()
            targets = [self.selected()] if self.selected() else []
        character = code_to_char(code)
        for element in targets:
            element.glyph = f"{element.glyph}{character}" if append else character
            element.font_path = default_font_path()
        self.font_path = default_font_path()
        self.redraw()
        self.sync_inspector()
        self.status.set(f"Glyph CP437 0x{code:02X} aplicado")
        modal.title(f"Tabela de glyphs CP437 — ultimo: 0x{code:02X}")

    def choose_font(self) -> None:
        path = filedialog.askopenfilename(
            title="Escolher fonte",
            filetypes=(("Fontes", "*.ttf *.otf *.ttc"), ("Todos os arquivos", "*.*")),
        )
        if not path:
            return
        self.vars["font_path"].set(path)
        self.apply_properties()

    def choose_background(self) -> None:
        self.open_live_color_picker("background")

    def apply_canvas(self) -> None:
        try:
            width = max(64, int(self.width_var.get()))
            height = max(64, int(self.height_var.get()))
            grid_size = max(2, int(self.grid_var.get()))
        except (tk.TclError, ValueError):
            messagebox.showerror(APP_NAME, "Largura, altura ou grade invalida.")
            return
        if (width, height, grid_size) == (self.canvas_width, self.canvas_height, self.grid_size):
            return
        self.checkpoint()
        self.canvas_width = width
        self.canvas_height = height
        self.grid_size = grid_size
        self.redraw()

    def transform_cursor(self, mode: str) -> str:
        if mode == "handle:rotate":
            return "exchange"
        corner = mode.rsplit(":", 1)[-1]
        first_diagonal = corner in {"nw", "se"}
        element = self.selected()
        quarter_turns = round((element.rotation if element else 0) / 90) % 2
        if quarter_turns:
            first_diagonal = not first_diagonal
        return "size_nw_se" if first_diagonal else "size_ne_sw"

    def on_canvas_motion(self, event: tk.Event) -> None:
        if self.transform_mode:
            self.canvas.configure(cursor=self.transform_cursor(self.transform_mode))
            return
        x = self.canvas.canvasx(event.x)
        y = self.canvas.canvasy(event.y)
        mode = None
        for item in reversed(self.canvas.find_overlapping(x - 1, y - 1, x + 1, y + 1)):
            mode = next((tag for tag in self.canvas.gettags(item) if tag.startswith("handle:")), None)
            if mode:
                break
        self.canvas.configure(cursor=self.transform_cursor(mode) if mode else "")

    def on_canvas_press(self, event: tk.Event) -> None:
        self.canvas.focus_set()
        x = self.canvas.canvasx(event.x)
        y = self.canvas.canvasy(event.y)
        additive = bool(event.state & 0x0001)
        overlapping = self.canvas.find_overlapping(x - 1, y - 1, x + 1, y + 1)
        handle_tag = None
        for item in reversed(overlapping):
            handle_tag = next(
                (tag for tag in self.canvas.gettags(item) if tag.startswith("handle:")), None
            )
            if handle_tag:
                break
        transform_target = self.selected()
        if handle_tag and transform_target:
            self.transform_mode = handle_tag
            geometry = self.transform_geometry(transform_target)
            if handle_tag == "handle:rotate":
                self.transform_start = {
                    "pointer": (x, y),
                    "center": geometry["center"],
                    "rotation": transform_target.rotation,
                }
            else:
                corner = handle_tag.rsplit(":", 1)[-1]
                opposite = {"nw": "se", "ne": "sw", "se": "nw", "sw": "ne"}[corner]
                self.transform_start = {
                    "corner": corner,
                    "anchor": geometry["corners"][opposite],
                    "width": geometry["width"],
                    "height": geometry["height"],
                    "scale_x": transform_target.scale_x,
                    "scale_y": transform_target.scale_y,
                    "rotation": transform_target.rotation,
                    "flip_x": transform_target.flip_x,
                    "flip_y": transform_target.flip_y,
                }
            self.transform_checkpointed = False
            self.drag_offset = None
            self.marquee_start = None
            self.canvas.configure(cursor=self.transform_cursor(handle_tag))
            return
        element_id = None
        joint_id = None
        for item in reversed(overlapping):
            tags = self.canvas.gettags(item)
            joint_tag = next((value for value in tags if value.startswith("joint:")), None)
            element_tag = next((value for value in tags if value.startswith("element:")), None)
            if joint_tag:
                joint_id = joint_tag.split(":", 1)[1]
                break
            if element_tag:
                element_id = element_tag.split(":", 1)[1]
                break
        if joint_id:
            if additive:
                self.select_joint(joint_id, additive=True)
            elif joint_id in self.selected_joint_ids:
                self.selected_joint_id = joint_id
                self.selected_id = None
            else:
                self.select_joint(joint_id)
        elif element_id:
            if additive:
                self.select_element(element_id, additive=True)
            elif element_id in self.selected_element_ids:
                self.selected_id = element_id
                self.selected_joint_id = None
            else:
                self.select_element(element_id)
        else:
            if not additive:
                self.clear_selection()
            self.marquee_start = (x, y)
            self.marquee_additive = additive
            self.canvas.delete("marquee")
            self.canvas.create_rectangle(
                x, y, x, y, outline="#75e69a", dash=(4, 3), width=2, tags=("marquee",)
            )
        element = self.selected()
        joint = self.selected_joint()
        selected = element or joint
        self.drag_offset = (
            (
                (x - self.origin[0]) / self.zoom - selected.x,
                (y - self.origin[1]) / self.zoom - selected.y,
            )
            if selected and not additive
            else None
        )
        self.drag_checkpointed = False
        self.draw_selection()
        self.sync_inspector()
        if selected:
            self.update_selection_status()

    def on_canvas_drag(self, event: tk.Event) -> None:
        if self.transform_mode and self.transform_start:
            element = self.selected()
            if not element:
                return
            if not self.transform_checkpointed:
                self.checkpoint()
                self.transform_checkpointed = True
            x = self.canvas.canvasx(event.x)
            y = self.canvas.canvasy(event.y)
            if self.transform_mode == "handle:rotate":
                start_x, start_y = self.transform_start["pointer"]
                center_x, center_y = self.transform_start["center"]
                rotation = float(self.transform_start["rotation"])
                start_angle = math.atan2(start_y - center_y, start_x - center_x)
                angle = math.atan2(y - center_y, x - center_x)
                element.rotation = round(rotation + math.degrees(angle - start_angle), 2)
                self.vars["rotation"].set(element.rotation)
            else:
                corner = str(self.transform_start["corner"])
                horizontal_sign = -1 if "w" in corner else 1
                vertical_sign = -1 if "n" in corner else 1
                anchor_x, anchor_y = self.transform_start["anchor"]
                rotation = float(self.transform_start["rotation"])
                angle = math.radians(rotation)
                cosine, sine = math.cos(angle), math.sin(angle)
                delta_x, delta_y = x - anchor_x, y - anchor_y
                local_x = delta_x * cosine + delta_y * sine
                local_y = -delta_x * sine + delta_y * cosine
                width = max(1.0, abs(local_x))
                height = max(1.0, abs(local_y))
                if getattr(event, "state", 0) & 0x0001:
                    ratio = max(
                        width / float(self.transform_start["width"]),
                        height / float(self.transform_start["height"]),
                    )
                    width = float(self.transform_start["width"]) * ratio
                    height = float(self.transform_start["height"]) * ratio
                    local_x = math.copysign(width, local_x or horizontal_sign)
                    local_y = math.copysign(height, local_y or vertical_sign)
                    x = anchor_x + local_x * cosine - local_y * sine
                    y = anchor_y + local_x * sine + local_y * cosine
                old_x, old_y = element.x, element.y
                self.shift_element(
                    element,
                    round(((anchor_x + x) / 2 - self.origin[0]) / self.zoom, 4) - old_x,
                    round(((anchor_y + y) / 2 - self.origin[1]) / self.zoom, 4) - old_y,
                )
                element.scale_x = round(
                    max(
                        0.02,
                        float(self.transform_start["scale_x"])
                        * width
                        / float(self.transform_start["width"]),
                    ),
                    4,
                )
                element.scale_y = round(
                    max(
                        0.02,
                        float(self.transform_start["scale_y"])
                        * height
                        / float(self.transform_start["height"]),
                    ),
                    4,
                )
                element.flip_x = bool(self.transform_start["flip_x"]) ^ (local_x * horizontal_sign < 0)
                element.flip_y = bool(self.transform_start["flip_y"]) ^ (local_y * vertical_sign < 0)
                for joint in self.joints:
                    if joint.kind == "attention" and joint.attached_element_id == element.id:
                        joint.x += element.x - old_x
                        joint.y += element.y - old_y
                self.vars["x"].set(element.x)
                self.vars["y"].set(element.y)
                self.vars["scale_x"].set(element.scale_x)
                self.vars["scale_y"].set(element.scale_y)
                self.vars["flip_x"].set(element.flip_x)
                self.vars["flip_y"].set(element.flip_y)
            self.redraw()
            return
        if self.marquee_start is not None:
            x = self.canvas.canvasx(event.x)
            y = self.canvas.canvasy(event.y)
            start_x, start_y = self.marquee_start
            self.canvas.coords("marquee", start_x, start_y, x, y)
            return
        element = self.selected()
        joint = self.selected_joint()
        selected = element or joint
        if not selected or self.drag_offset is None:
            return
        if not self.drag_checkpointed:
            self.checkpoint()
            self.drag_checkpointed = True
        x = (self.canvas.canvasx(event.x) - self.origin[0]) / self.zoom - self.drag_offset[0]
        y = (self.canvas.canvasy(event.y) - self.origin[1]) / self.zoom - self.drag_offset[1]
        new_x, new_y = round(self.snap_value(x), 2), round(self.snap_value(y), 2)
        delta_x, delta_y = new_x - selected.x, new_y - selected.y
        if not delta_x and not delta_y:
            return
        for selected_element in self.elements:
            if selected_element.id in self.selected_element_ids:
                self.shift_element(selected_element, delta_x, delta_y)
        for moved_joint in self.joints:
            if moved_joint.id in self.dragged_joint_ids():
                moved_joint.x += delta_x
                moved_joint.y += delta_y
        if element:
            self.vars["x"].set(element.x)
            self.vars["y"].set(element.y)
        elif joint:
            variables = self.attention_vars if joint.kind == "attention" else self.joint_vars
            variables["x"].set(joint.x)
            variables["y"].set(joint.y)
        self.redraw()

    def on_canvas_release(self, event: tk.Event) -> None:
        if self.transform_mode:
            self.transform_mode = None
            self.transform_start = None
            self.transform_checkpointed = False
            self.on_canvas_motion(event)
            return
        if self.marquee_start is not None:
            end_x = self.canvas.canvasx(event.x)
            end_y = self.canvas.canvasy(event.y)
            start_x, start_y = self.marquee_start
            left, right = sorted((start_x, end_x))
            top, bottom = sorted((start_y, end_y))
            found_elements: list[str] = []
            found_joints: list[str] = []
            for item in self.canvas.find_overlapping(left, top, right, bottom):
                tags = self.canvas.gettags(item)
                for tag in tags:
                    if tag.startswith("element:"):
                        found_elements.append(tag.split(":", 1)[1])
                    elif tag.startswith("joint:"):
                        found_joints.append(tag.split(":", 1)[1])
            self.selected_element_ids.update(found_elements)
            self.selected_joint_ids.update(found_joints)
            self.selected_id = found_elements[-1] if found_elements else None
            self.selected_joint_id = found_joints[-1] if found_joints else None
            if self.selected_joint_id:
                self.selected_id = None
            self.canvas.delete("marquee")
            self.marquee_start = None
            self.draw_selection()
            self.sync_inspector()
            self.update_selection_status()
        self.drag_offset = None
        self.drag_checkpointed = False

    def undo(self) -> None:
        if not self.undo_stack:
            return
        self.stop_play()
        self.redo_stack.append(copy.deepcopy(self.current_scene()))
        scene = self.undo_stack.pop()
        self.load_scene(scene)
        self.mark_dirty()

    def redo(self) -> None:
        if not self.redo_stack:
            return
        self.stop_play()
        self.undo_stack.append(copy.deepcopy(self.current_scene()))
        scene = self.redo_stack.pop()
        self.load_scene(scene)
        self.mark_dirty()

    def on_notes_modified(self, _event: tk.Event) -> None:
        if self.loading_notes:
            return
        if self.notes.edit_modified():
            self.mark_dirty()
            self.notes.edit_modified(False)

    def confirm_discard(self) -> bool:
        if not self.dirty:
            return True
        answer = messagebox.askyesnocancel(APP_NAME, "Salvar as alteracoes antes de continuar?")
        if answer is None:
            return False
        if answer:
            return self.save_project()
        return True

    def new_project(self, force: bool = False) -> None:
        if not force and not self.confirm_discard():
            return
        self.stop_play()
        self.font_path = default_font_path()
        self.vars["font_path"].set(self.font_path)
        self.clip_index = None
        self.frame_index = None
        scene = {
            "canvas": {
                "width": 800,
                "height": 600,
                "background": "#181818",
                "grid_size": 16,
                "accent": DEFAULT_ACCENT,
            },
            "elements": [],
            "notes": "Descreva aqui o que esta composicao representa e como deve ser usada.",
        }
        self.project_path = None
        self.load_scene(scene, reset_history=True)
        self.dirty = False
        self.update_title()
        self.status.set("Novo projeto")

    def open_project(self) -> None:
        if not self.confirm_discard():
            return
        self.stop_play()
        self.clip_index = None
        self.frame_index = None
        path = filedialog.askopenfilename(
            title="Abrir projeto",
            filetypes=(("Projeto Glyph Forge", "*.glyph.json"), ("JSON", "*.json")),
        )
        if not path:
            return
        try:
            scene = json.loads(Path(path).read_text(encoding="utf-8"))
            self.load_scene(scene, reset_history=True)
        except (OSError, ValueError, TypeError) as exc:
            messagebox.showerror(APP_NAME, f"Nao foi possivel abrir o projeto:\n{exc}")
            return
        self.project_path = Path(path)
        self.dirty = self.labels_migrated
        self.update_title()
        self.status.set(
            f"Aberto: {path} — conjuntos aninhados reconhecidos"
            if self.labels_migrated
            else f"Aberto: {path}"
        )

    def save_project(self) -> bool:
        if self.project_path is None:
            return self.save_project_as()
        return self.write_project(self.project_path)

    def save_project_as(self) -> bool:
        initial_name = self.project_path.name if self.project_path else "sem-titulo.glyph.json"
        path = filedialog.asksaveasfilename(
            title="Salvar projeto como",
            initialfile=initial_name,
            defaultextension=".glyph.json",
            filetypes=(("Projeto Glyph Forge", "*.glyph.json"),),
        )
        if not path:
            return False
        return self.write_project(Path(path))

    def write_project(self, path: Path) -> bool:
        try:
            path.write_text(
                json.dumps(self.current_scene(), ensure_ascii=False, indent=2), encoding="utf-8"
            )
        except OSError as exc:
            messagebox.showerror(APP_NAME, f"Nao foi possivel salvar:\n{exc}")
            return False
        self.project_path = path
        self.labels_migrated = False
        self.dirty = False
        self.update_title()
        self.status.set(f"Salvo: {path}")
        return True

    def check_scene(self) -> None:
        found = problems(self.current_scene())
        if not found:
            self.status.set("Conferido: nada que o jogo nao consiga ler")
            messagebox.showinfo(APP_NAME, "Nada quebrado: nomes unicos e todas as pecas no lugar.")
            return
        self.status.set(f"{len(found)} problema(s) -- veja a lista")
        messagebox.showwarning(
            APP_NAME,
            f"{len(found)} problema(s):\n\n" + "\n".join(f"- {problem}" for problem in found[:20]),
        )

    def export_bundle(self) -> None:
        default_name = "meu-modelo"
        if self.project_path:
            default_name = self.project_path.name.removesuffix(".glyph.json")
        requested_name = simpledialog.askstring(
            "Exportar como",
            "Nome do pacote:",
            initialvalue=default_name,
            parent=self.root,
        )
        if requested_name is None:
            return
        package_name = safe_export_name(requested_name)
        if not package_name:
            messagebox.showerror(APP_NAME, "Digite um nome valido para o pacote.")
            return
        export_root = self.project_path.parent if self.project_path else Path(__file__).resolve().parent / "exports"
        target = export_root / package_name
        try:
            target_is_file = target.is_file()
            target_has_files = target.exists() and not target_is_file and any(target.iterdir())
        except OSError as exc:
            messagebox.showerror(APP_NAME, f"Nao foi possivel preparar a exportacao:\n{exc}")
            return
        if target_is_file:
            messagebox.showerror(APP_NAME, f"Ja existe um arquivo com esse nome:\n{target}")
            return
        if target_has_files:
            if not messagebox.askyesno(
                APP_NAME,
                f"O pacote '{package_name}' ja existe. Atualizar seus arquivos?",
            ):
                return
        scene = self.current_scene()
        found = problems(scene)
        if found and not messagebox.askyesno(
            APP_NAME,
            f"{len(found)} problema(s) que o jogo pode nao conseguir ler:\n\n"
            + "\n".join(f"- {problem}" for problem in found[:10])
            + "\n\nExportar mesmo assim?",
        ):
            return
        exported_scene = portable_scene(scene)
        try:
            target.mkdir(parents=True, exist_ok=True)
            (target / "scene.json").write_text(
                json.dumps(exported_scene, ensure_ascii=False, indent=2), encoding="utf-8"
            )
            staged = staged_scene(scene)
            render_scene(staged).convert("RGB").save(target / "preview.png")
            render_scene(staged, show_rig=True).convert("RGB").save(target / "rig_preview.png")
            (target / "flattened.txt").write_text(flatten_scene(staged), encoding="utf-8")
            clips = export_clips(scene, target)
            notes = scene.get("notes", "").strip()
            prompt = (
                "# Modelo Glyph Forge\n\n"
                f"{notes or 'Sem observacoes adicionais.'}\n\n"
                "Analise `scene.json` como fonte de verdade para posicoes, cores e transformacoes. "
                "A fonte padrao e a ROM IBM VGA 8x16 CP437 descrita em `default_font`. "
                "Nos elementos, `flip_x` e `flip_y` registram o espelhamento local. "
                "Em `rig.joints`, `part_a_element_id` e `part_b_element_id` sao as duas pecas ligadas "
                "por um ponto independente; `constraint_type` define pivo ou solda e `fixed` ancora ao mundo. "
                "`attention_points` contem destaques com nome e descricao opcional que nao sao ossos. "
                "`labels` nomeia e descreve conjuntos: `element_ids` sao membros diretos e "
                "`label_ids` contem outros rotulos, herdando seus glyphs recursivamente. "
                "`elements` e a pose de repouso: cada quadro de `animation.clips` guarda em "
                "`keys` apenas os campos que mudam, por id de peca. "
                "Uma peca com `span` e um segmento entre dois pontos do rig, e uma com `follow` "
                "e carregada por um ponto a distancia `offset` -- em ambos os casos a posicao e "
                "derivada, e o que o quadro guarda sao os pontos. "
                "`role` diz o que a peca e para o jogo, e `marks` diz o que acontece em cada "
                "quadro (contato, brilho). "
                "`skins` decide glifo e cor de cada peca conforme o `role` dela (`body`, `limb` "
                "ou vazio). Use `rig_preview.png` "
                "para visualizar o esqueleto, `preview.png` para a arte limpa, `animacao/` para "
                "os GIFs e PNGs de cada quadro e `flattened.txt` apenas "
                "como aproximacao ASCII.\n"
            )
            (target / "prompt.md").write_text(prompt, encoding="utf-8")
        except (OSError, ValueError) as exc:
            messagebox.showerror(APP_NAME, f"Nao foi possivel exportar:\n{exc}")
            return
        self.status.set(f"Exportado para: {target}")
        messagebox.showinfo(
            APP_NAME,
            "Exportacao concluida: scene.json, preview.png, rig_preview.png, flattened.txt, "
            "prompt.md"
            + (f" e {len(clips)} animacao(oes) em animacao/" if clips else ""),
        )

    def on_close(self) -> None:
        if self.confirm_discard():
            self.root.destroy()


def self_test() -> None:
    assert safe_export_name(' boneco: "azul". ') == 'boneco_ _azul_'
    assert safe_export_name("CON") == "_CON"
    assert len(read_rom(default_font_path())) == 4096
    assert rom_code_image(char_to_code("A"), default_font_path()).getbbox()
    assert code_to_char(0xDB) == "█"
    glyph = Glyph.create(80, 60, default_font_path())
    root_joint = Joint.create(80, 60, 1)
    root_joint.part_a_element_id = glyph.id
    child_joint = Joint.create(100, 80, 2)
    child_joint.parent_id = root_joint.id
    attention = Joint.create_attention(120, 40, 1)
    attention.name = "olhar_aqui"
    attention.description = "Detalhe importante para a LLM."
    scene = {
        "canvas": {"width": 160, "height": 120, "background": "#000000"},
        "elements": [asdict(glyph)],
        "rig": {"joints": [asdict(root_joint), asdict(child_joint)]},
        "attention_points": [asdict(attention)],
    }
    image = render_scene(scene)
    rig_image = render_scene(scene, show_rig=True)
    flattened = flatten_scene(scene)
    assert image.size == (160, 120)
    assert rig_image.tobytes() != image.tobytes()
    assert scene_joints(scene)[1].parent_id == root_joint.id
    assert scene_attention_points(scene)[0].description.startswith("Detalhe")
    assert "O" in flattened
    assert portable_scene(scene)["elements"][0]["font_path"] == "assets/fonts/ibm_vga_8x16.bin"
    rune = SemanticLabel("rune", "custom_rune_1", ["a", "b", "c", "d"])
    runes = SemanticLabel(
        "runes", "Magic Runes", ["a", "b", "c", "d", "e", "f", "g", "h"]
    )
    unrelated = SemanticLabel("book", "magic book", ["cover", "page"])
    labels = [unrelated, rune, runes]
    assert infer_nested_labels(labels)
    assert runes.label_ids == [rune.id]
    assert set(runes.element_ids) == {"e", "f", "g", "h"}
    labels_by_id = {label.id: label for label in labels}
    assert resolved_label_elements(runes.id, labels_by_id) == set("abcdefgh")
    assert label_reaches(runes.id, rune.id, labels_by_id)
    assert not label_reaches(rune.id, runes.id, labels_by_id)
    json.dumps(scene)

    # --- animacao e peles ---
    # A cena guarda o repouso; o quadro guarda a diferenca. Se o quadro deixasse
    # de ser diferenca, mexer no repouso pararia de alcancar os quadros e cada
    # um viraria uma copia independente da cena.
    body = Glyph.create(80, 60, default_font_path())
    body.glyph = "O"
    body.role = "body"
    arm = Glyph.create(90, 70, default_font_path())
    arm.role = "limb"
    animated = {
        "canvas": {"width": 160, "height": 120, "background": "#000000", "accent": "#ff8800"},
        "elements": [asdict(body), asdict(arm)],
        "animation": {
            "clips": [
                asdict(
                    Clip(
                        id="c1",
                        name="aceno",
                        fps=4,
                        frames=[
                            Frame(id="f1", name="baixo"),
                            Frame(
                                id="f2",
                                name="alto",
                                hold=2,
                                tone="hurt",
                                keys={arm.id: {"y": 40.0, "rotation": 30.0}},
                            ),
                        ],
                    )
                )
            ]
        },
        "skins": [
            asdict(
                Skin(
                    id="s1",
                    name="pesado",
                    swap=[["O", "0"]],
                    accent="O",
                    limb="║",
                    body="#cccccc",
                    hurt="#ff0000",
                    limbs="#00ff00",
                )
            )
        ],
        "active_skin": "s1",
    }
    rest_pose = staged_scene(animated)
    high = staged_scene(animated, 0, 1)
    assert rest_pose["elements"][1]["y"] == 70, "o repouso nao pode carregar o quadro"
    assert high["elements"][1]["y"] == 40, "o quadro nao chegou na peca"
    assert high["elements"][1]["rotation"] == 30
    # A troca de glifo roda depois da cor: a cabeca continua com a cor de quem
    # veste o boneco mesmo tendo virado outro caractere.
    assert rest_pose["elements"][0]["glyph"] == "0"
    assert rest_pose["elements"][0]["color"] == "#ff8800"
    assert rest_pose["elements"][1]["glyph"] == "║"
    assert rest_pose["elements"][1]["color"] == "#00ff00"
    # O papel de cor do quadro muda a pele inteira, e nao uma peca:
    assert staged_scene(animated, 0, 0)["elements"][0]["color"] == "#ff8800"
    plain = copy.deepcopy(animated)
    plain.pop("skins")
    assert staged_scene(plain)["elements"][0]["glyph"] == "O", "sem pele, nada e trocado"
    # Um segmento sai dos dois pontos, entao o quadro guarda as coordenadas das
    # articulacoes -- que sao os mesmos numeros do rig do jogo -- e nunca o
    # meio-caminho, o angulo e a escala, que ninguem reverte a mao.
    shoulder = Joint.create(100, 100, 1)
    elbow = Joint.create(100, 132, 2)
    segment = Glyph.create(0, 0, default_font_path())
    segment.span = [shoulder.id, elbow.id]
    solve_spans([segment], [shoulder, elbow])
    assert (segment.x, segment.y) == (100, 116), (segment.x, segment.y)
    assert segment.rotation == 0.0, segment.rotation
    assert segment.scale_y == 0.5, segment.scale_y  # 32px sobre um glyph de 64
    elbow.x, elbow.y = 132, 100
    solve_spans([segment], [shoulder, elbow])
    assert segment.rotation == -90.0, segment.rotation
    assert (segment.x, segment.y) == (116, 100)

    # Uma peca presa a um ponto guarda a distancia, nao o lugar: mover o ponto
    # leva a peca junto, e o quadro grava a mao -- que e o que o jogo tem.
    hand = Joint.create(200, 100, 3)
    hand.name = "mao"
    sword = Glyph.create(0, 0, default_font_path())
    sword.follow = hand.id
    sword.offset = [6.0, -4.0]
    solve_spans([sword], [hand])
    assert (sword.x, sword.y) == (206.0, 96.0), (sword.x, sword.y)
    hand.x, hand.y = 260.0, 140.0
    solve_spans([sword], [hand])
    assert (sword.x, sword.y) == (266.0, 136.0), (sword.x, sword.y)

    # A conferencia acha o que vira `panic!` no Rust, que acha peca por nome.
    assert problems(animated) == [], problems(animated)
    broken = copy.deepcopy(animated)
    broken["labels"] = [
        asdict(SemanticLabel("l1", "cabo", [body.id])),
        asdict(SemanticLabel("l2", "cabo", [arm.id])),
        asdict(SemanticLabel("l3", "vazio", [])),
        asdict(SemanticLabel("l4", "fantasma", ["nao_existe"])),
    ]
    broken["elements"][0]["follow"] = "ponto_que_sumiu"
    broken["animation"]["clips"][0]["frames"][0]["keys"] = {"peca_morta": {"x": 1.0}}
    report = problems(broken)
    assert any("aparece 2 vezes" in problem for problem in report), report
    assert any("nao contem glyph nenhum" in problem for problem in report), report
    assert any("presa a um ponto que nao existe" in problem for problem in report), report
    assert any("peca que nao existe" in problem for problem in report), report

    with tempfile.TemporaryDirectory() as folder:
        written = export_clips(animated, Path(folder))
        assert written == ["aceno.gif"]
        assert (Path(folder) / "animacao" / "aceno_02.png").is_file()
        with Image.open(Path(folder) / "animacao" / "aceno.gif") as gif:
            assert gif.n_frames == 2
    json.dumps(animated)
    print("self-test OK")


if __name__ == "__main__":
    if "--self-test" in sys.argv:
        self_test()
    else:
        application = tk.Tk()
        GlyphForge(application)
        application.mainloop()
