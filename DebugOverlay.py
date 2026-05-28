import json
from datetime import datetime
from pathlib import Path

import pygame


PANEL_BG = (16, 18, 20, 215)
TEXT = (238, 238, 238)
MUTED = (170, 180, 190)


def build_debug_lines(label, player, input_source):
    main = _pair(player, "main_stick")
    c_stick = _pair(player, "c_stick")
    lines = [
        f"{label} source: {_source_name(input_source)}",
        f"main: {_fmt(main[0])}, {_fmt(main[1])}  c: {_fmt(c_stick[0])}, {_fmt(c_stick[1])}",
        f"triggers: L {_fmt(_get(player, 'l_trigger'))}  R {_fmt(_get(player, 'r_trigger'))}",
        (
            "buttons: "
            f"A={_bit(_get(player, 'akey'))} "
            f"B={_bit(_get(player, 'specialkey'))} "
            f"J={_bit(_get(player, 'jumpkey'))} "
            f"G={_bit(_get(player, 'grabkey'))} "
            f"Z={_bit(_get(player, 'blockkey'))} "
            f"M={_bit(_get(player, 'menukey'))}"
        ),
        (
            "shield: "
            f"L={_bit(_get(player, 'l_shieldkey'))}/D{_bit(_get(player, 'l_trigger_digital'))}"
            f"/P{_bit(_get(player, 'l_shield_pressed'))} "
            f"R={_bit(_get(player, 'r_shieldkey'))}/D{_bit(_get(player, 'r_trigger_digital'))}"
            f"/P{_bit(_get(player, 'r_shield_pressed'))}"
        ),
        (
            "dpad: "
            f"U={_bit(_get(player, 'upkey'))} "
            f"D={_bit(_get(player, 'downkey'))} "
            f"L={_bit(_get(player, 'leftkey'))} "
            f"R={_bit(_get(player, 'rightkey'))}"
        ),
        (
            f"state: {_get(player, 'state', 'unknown')}  "
            f"pos: {_fmt(_get(player, 'x'))}, {_fmt(_get(player, 'y'))}  "
            f"vel: {_fmt(_get(player, 'xVelocity'))}, {_fmt(_get(player, 'yVelocity'))}"
        ),
        (
            f"flags: grounded={_bit(_get(player, 'grounded'))} "
            f"actionable={_bit(_get(player, 'actionable'))} "
            f"facing={_facing(player)}"
        ),
        (
            f"counts: x={_int(_get(player, 'xCount'))} "
            f"dash={_int(_get(player, 'dashCount'))} "
            f"turn={_int(_get(player, 'turnCount'))}  "
            f"can: dash={_bit(_get(player, 'canDash'))} "
            f"jump={_bit(_get(player, 'canJump'))} "
            f"block={_bit(_get(player, 'canBlock'))}"
        ),
    ]
    lines.extend(_source_debug_lines(input_source))
    return lines


def draw_debug_overlay(win, players, input_sources, *, logger=None):
    font = pygame.font.SysFont("consolas", 16)
    header_font = pygame.font.SysFont("consolas", 17, bold=True)
    lines = ["Input Debug  F3 overlay  F4 log"]
    if logger is not None and logger.enabled:
        lines.append(f"log: {logger.path}")
    else:
        lines.append("log: off")

    for index, player in enumerate(players):
        source = input_sources[index] if index < len(input_sources) else None
        lines.append("")
        lines.extend(build_debug_lines(f"P{index + 1}", player, source))

    line_height = font.get_linesize()
    padding = 10
    max_width = max(font.size(line)[0] for line in lines)
    width = min(max_width + padding * 2, max(win.get_width() - 20, 1))
    height = min(line_height * len(lines) + padding * 2, max(win.get_height() - 20, 1))

    panel = pygame.Surface((width, height), pygame.SRCALPHA)
    panel.fill(PANEL_BG)

    y = padding
    for index, line in enumerate(lines):
        active_font = header_font if index == 0 else font
        color = MUTED if line.startswith("log:") else TEXT
        rendered = active_font.render(line, True, color)
        panel.blit(rendered, (padding, y))
        y += line_height
        if y > height - padding:
            break

    win.blit(panel, (10, 80))


class DebugInputLogger:
    def __init__(self, enabled=False, log_dir=None):
        self.enabled = bool(enabled)
        self.log_dir = Path(log_dir or Path(__file__).resolve().parent / "logs")
        self.path = None
        self._handle = None
        if self.enabled:
            self._open()

    def toggle(self):
        self.enabled = not self.enabled
        if self.enabled:
            self._open()
        else:
            self.close()

    def write_frame(self, frame, players, input_sources):
        if not self.enabled:
            return
        if self._handle is None:
            self._open()
        payload = {
            "frame": frame,
            "players": [
                build_debug_snapshot(f"P{index + 1}", player, input_sources[index] if index < len(input_sources) else None)
                for index, player in enumerate(players)
            ],
        }
        self._handle.write(json.dumps(payload, separators=(",", ":")) + "\n")

    def close(self):
        if self._handle is not None:
            self._handle.close()
            self._handle = None

    def _open(self):
        if self._handle is not None:
            return
        self.log_dir.mkdir(parents=True, exist_ok=True)
        stamp = datetime.now().strftime("%Y%m%d-%H%M%S")
        self.path = self.log_dir / f"input-debug-{stamp}.jsonl"
        self._handle = self.path.open("a", encoding="utf-8")


def build_debug_snapshot(label, player, input_source):
    return {
        "label": label,
        "source": _source_name(input_source),
        "main_stick": _pair(player, "main_stick"),
        "c_stick": _pair(player, "c_stick"),
        "l_trigger": _get(player, "l_trigger"),
        "r_trigger": _get(player, "r_trigger"),
        "buttons": {
            "a": bool(_get(player, "akey")),
            "b": bool(_get(player, "specialkey")),
            "jump": bool(_get(player, "jumpkey")),
            "grab": bool(_get(player, "grabkey")),
            "shield": bool(_get(player, "blockkey")),
            "shield_l": bool(_get(player, "l_shieldkey")),
            "shield_r": bool(_get(player, "r_shieldkey")),
            "shield_l_pressed": bool(_get(player, "l_shield_pressed")),
            "shield_r_pressed": bool(_get(player, "r_shield_pressed")),
            "trigger_l_digital": bool(_get(player, "l_trigger_digital")),
            "trigger_r_digital": bool(_get(player, "r_trigger_digital")),
            "menu": bool(_get(player, "menukey")),
        },
        "dpad": {
            "up": bool(_get(player, "upkey")),
            "down": bool(_get(player, "downkey")),
            "left": bool(_get(player, "leftkey")),
            "right": bool(_get(player, "rightkey")),
        },
        "state": _get(player, "state", "unknown"),
        "position": [_get(player, "x"), _get(player, "y")],
        "velocity": [_get(player, "xVelocity"), _get(player, "yVelocity")],
        "grounded": bool(_get(player, "grounded")),
        "actionable": bool(_get(player, "actionable")),
        "facing": _facing(player),
        "counts": {
            "x": _int(_get(player, "xCount")),
            "dash": _int(_get(player, "dashCount")),
            "turn": _int(_get(player, "turnCount")),
        },
        "can": {
            "dash": bool(_get(player, "canDash")),
            "jump": bool(_get(player, "canJump")),
            "block": bool(_get(player, "canBlock")),
        },
    }


def _source_name(input_source):
    if input_source is None:
        return "None"
    if hasattr(input_source, "get_name"):
        try:
            return input_source.get_name()
        except Exception:
            return input_source.__class__.__name__
    return input_source.__class__.__name__


def _source_debug_lines(input_source):
    if input_source is None or not hasattr(input_source, "debug_lines"):
        return []
    try:
        lines = input_source.debug_lines()
    except Exception:
        return []
    return [str(line) for line in lines]


def _pair(player, attr):
    value = _get(player, attr, [0, 0])
    if not isinstance(value, (list, tuple)) or len(value) < 2:
        return [0, 0]
    return [value[0], value[1]]


def _get(player, attr, default=0):
    return getattr(player, attr, default)


def _fmt(value):
    return f"{float(value):+.3f}"


def _bit(value):
    return "1" if bool(value) else "0"


def _int(value):
    return int(value)


def _facing(player):
    return "R" if bool(_get(player, "isRight", True)) else "L"
