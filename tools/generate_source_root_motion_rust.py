"""Merge source-backed TransN root-motion samples into mole_core."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
from pathlib import Path
from typing import Any

try:
    from tools.rust_literals import render_f32_bits
except ModuleNotFoundError:  # pragma: no cover - direct script execution path
    from rust_literals import render_f32_bits  # type: ignore

PROJECT_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = PROJECT_ROOT / "crates" / "mole_core" / "src" / "generated" / "source_root_motion.rs"
DEFAULT_MOLE_BIN = PROJECT_ROOT / "target" / "debug" / "mole.exe"
DEFAULT_MANIFEST = PROJECT_ROOT / "resources" / "melee" / "frame_data" / "dolphin_mole" / "source_manifest.json"

SOURCE_STATES = {
    "SpecialHi": ("SPECIAL_HI", 65),
    "SpecialAirHi": ("SPECIAL_AIR_HI", 65),
}

SOURCE_ACTIONS = {
    "CliffAttackQuick": "CLIFF_ATTACK_QUICK",
    "CliffAttackSlow": "CLIFF_ATTACK_SLOW",
    "CliffClimbQuick": "CLIFF_CLIMB_QUICK",
    "CliffClimbSlow": "CLIFF_CLIMB_SLOW",
    "CliffEscapeQuick": "CLIFF_ESCAPE_QUICK",
    "CliffEscapeSlow": "CLIFF_ESCAPE_SLOW",
    "CliffJumpQuick1": "CLIFF_JUMP_QUICK1",
    "CliffJumpQuick2": "CLIFF_JUMP_QUICK2",
    "CliffJumpSlow1": "CLIFF_JUMP_SLOW1",
    "CliffJumpSlow2": "CLIFF_JUMP_SLOW2",
    "PassiveStandF": "PASSIVE_STAND_F",
    "PassiveStandB": "PASSIVE_STAND_B",
}

SOURCE_ACTION_TABLES = {
    "EscapeF": "ESCAPE_F",
    "EscapeB": "ESCAPE_B",
    "SpecialAirHi": "SPECIAL_AIR_HI",
    "SpecialHi": "SPECIAL_HI",
    "TurnRun": "TURN_RUN",
    "CliffCatch": "CLIFF_CATCH",
    "CliffWait1": "CLIFF_WAIT",
    **SOURCE_ACTIONS,
}


def sample_frame(mole_bin: Path, state: str, frame: int) -> tuple[dict[str, float], dict[str, float]]:
    command = [
        str(mole_bin),
        "frame-data",
        "sample",
        "--character",
        "dolphin_mole",
        "--state",
        state,
        "--frame",
        str(frame),
        "--json",
    ]
    result = subprocess.run(
        command,
        cwd=PROJECT_ROOT,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    payload: dict[str, Any] = json.loads(result.stdout)
    root_motion = payload["sample"]["source_root_motion"]
    return root_motion["transn_offset"], root_motion["transn_position"]


def fmt_float(value: float) -> str:
    return render_f32_bits(value)


def vec_literal(vec: dict[str, float]) -> str:
    return f"Vec3::new({fmt_float(vec['x'])}, {fmt_float(vec['y'])}, {fmt_float(vec['z'])})"


def normalize_vec3_float_literals(module: str) -> str:
    pattern = re.compile(
        r"Vec3::new\(\s*"
        r"(?P<x>[-+]?(?:\d+(?:\.\d*)?|\.\d+)(?:e[-+]?\d+)?)\s*,\s*"
        r"(?P<y>[-+]?(?:\d+(?:\.\d*)?|\.\d+)(?:e[-+]?\d+)?)\s*,\s*"
        r"(?P<z>[-+]?(?:\d+(?:\.\d*)?|\.\d+)(?:e[-+]?\d+)?)\s*\)",
        re.IGNORECASE,
    )

    return pattern.sub(
        lambda match: "Vec3::new("
        + ", ".join(
            fmt_float(float(match.group(axis)))
            for axis in ("x", "y", "z")
        )
        + ")",
        module,
    )


def render_const(name: str, kind: str, frames: list[dict[str, float]]) -> str:
    lines = [f"const {name}_TRANSN_{kind}: [Vec3; {len(frames)}] = ["]
    lines.extend(f"    {vec_literal(frame)}," for frame in frames)
    lines.append("];")
    return "\n".join(lines)


def replace_or_insert_const(module: str, const_text: str, marker: str) -> str:
    const_name = const_text.split(":", 1)[0].removeprefix("const ")
    start = module.find(f"const {const_name}:")
    if start != -1:
        end = module.find("\n];", start)
        if end == -1:
            raise RuntimeError(f"could not locate end of {const_name}")
        end += len("\n];")
        return module[:start] + const_text + module[end:]
    insert_at = module.find(marker)
    if insert_at == -1:
        raise RuntimeError(f"could not locate insertion marker {marker!r}")
    return module[:insert_at] + const_text + "\n\n" + module[insert_at:]


def ensure_match_arm(module: str, function_name: str, state: str, const_prefix: str, kind: str) -> str:
    arm = f"        MotionState::{state} => {const_prefix}_TRANSN_{kind}.get(index).copied(),"
    if arm in module:
        return module
    function_start = module.find(f"pub(crate) fn {function_name}")
    if function_start == -1:
        raise RuntimeError(f"could not locate {function_name}")
    insert_after = module.find("MotionState::EscapeB", function_start)
    insert_line_end = module.find("\n", insert_after)
    return module[: insert_line_end + 1] + arm + "\n" + module[insert_line_end + 1 :]


def ensure_frame_count_arm(module: str, state: str, const_prefix: str) -> str:
    arm = f"        MotionState::{state} => Some({const_prefix}_TRANSN_OFFSET.len() as u8),"
    if arm in module:
        return module
    function_start = module.find("pub(crate) fn transn_frame_count")
    if function_start == -1:
        raise RuntimeError("could not locate transn_frame_count")
    insert_after = module.find("MotionState::EscapeB", function_start)
    insert_line_end = module.find("\n", insert_after)
    return module[: insert_line_end + 1] + arm + "\n" + module[insert_line_end + 1 :]


def find_function_end(module: str, function_start: int) -> int:
    brace_start = module.find("{", function_start)
    if brace_start == -1:
        raise RuntimeError("could not locate function body")
    depth = 0
    for index in range(brace_start, len(module)):
        char = module[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return index + 1
    raise RuntimeError("could not locate function end")


def replace_or_insert_function(module: str, function_name: str, function_text: str, marker: str) -> str:
    function_start = module.find(f"pub(crate) fn {function_name}")
    if function_start != -1:
        function_end = find_function_end(module, function_start)
        return module[:function_start] + function_text + module[function_end:]
    insert_at = module.find(marker)
    if insert_at == -1:
        raise RuntimeError(f"could not locate insertion marker {marker!r}")
    return module[:insert_at] + function_text + "\n\n" + module[insert_at:]


def action_key_match_function(function_name: str, kind: str) -> str:
    lines = [
        f"pub(crate) fn {function_name}(source_action_key: SourceActionKey, source_frame: u8) -> Option<Vec3> {{",
        "    let index = usize::from(source_frame.saturating_sub(1));",
        "    match source_action_key.as_str() {",
    ]
    for action_key, const_prefix in SOURCE_ACTION_TABLES.items():
        lines.append(
            f"        \"{action_key}\" => {const_prefix}_TRANSN_{kind}.get(index).copied(),"
        )
    lines.extend(["        _ => None,", "    }", "}"])
    return "\n".join(lines)


def action_key_frame_count_function() -> str:
    lines = [
        "pub(crate) fn transn_frame_count_for_action_key(source_action_key: SourceActionKey) -> Option<u8> {",
        "    match source_action_key.as_str() {",
    ]
    for action_key, const_prefix in SOURCE_ACTION_TABLES.items():
        lines.append(
            f"        \"{action_key}\" => Some({const_prefix}_TRANSN_OFFSET.len() as u8),"
        )
    lines.extend(["        _ => None,", "    }", "}"])
    return "\n".join(lines)


def ensure_action_key_import(module: str) -> str:
    return module.replace(
        "use super::MotionState;",
        "use super::{MotionState, SourceActionKey};",
    )


def total_frames_for_action(manifest: dict[str, Any], source_action_key: str) -> int:
    for action in manifest["actions"]:
        if action.get("source_action_key") == source_action_key:
            return int(action["total_frames"])
    raise RuntimeError(f"source action {source_action_key} not found in source manifest")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--mole-bin", type=Path, default=DEFAULT_MOLE_BIN)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    args = parser.parse_args()

    if not args.mole_bin.exists():
        raise SystemExit(f"mole binary not found: {args.mole_bin}")

    manifest: dict[str, Any] = json.loads(args.manifest.read_text(encoding="utf-8"))
    module = args.output.read_text(encoding="utf-8")
    for state, (const_prefix, frame_count) in SOURCE_STATES.items():
        offsets: list[dict[str, float]] = []
        positions: list[dict[str, float]] = []
        for frame in range(1, frame_count + 1):
            offset, position = sample_frame(args.mole_bin, state, frame)
            offsets.append(offset)
            positions.append(position)

        offset_const = render_const(const_prefix, "OFFSET", offsets)
        position_const = render_const(const_prefix, "POSITION", positions)
        module = replace_or_insert_const(module, offset_const, "const TURN_RUN_TRANSN_OFFSET")
        module = replace_or_insert_const(module, position_const, "const TURN_RUN_TRANSN_OFFSET")
        module = ensure_match_arm(module, "transn_offset", state, const_prefix, "OFFSET")
        module = ensure_match_arm(module, "transn_position", state, const_prefix, "POSITION")
        module = ensure_frame_count_arm(module, state, const_prefix)

    for source_action_key, const_prefix in SOURCE_ACTIONS.items():
        frame_count = total_frames_for_action(manifest, source_action_key)
        offsets = []
        positions = []
        for frame in range(1, frame_count + 1):
            offset, position = sample_frame(args.mole_bin, source_action_key, frame)
            offsets.append(offset)
            positions.append(position)

        offset_const = render_const(const_prefix, "OFFSET", offsets)
        position_const = render_const(const_prefix, "POSITION", positions)
        module = replace_or_insert_const(module, offset_const, "const TURN_RUN_TRANSN_OFFSET")
        module = replace_or_insert_const(module, position_const, "const TURN_RUN_TRANSN_OFFSET")

    module = ensure_action_key_import(module)
    module = replace_or_insert_function(
        module,
        "transn_offset_for_action_key",
        action_key_match_function("transn_offset_for_action_key", "OFFSET"),
        "pub(crate) fn transn_offset",
    )
    module = replace_or_insert_function(
        module,
        "transn_position_for_action_key",
        action_key_match_function("transn_position_for_action_key", "POSITION"),
        "pub(crate) fn transn_position",
    )
    module = replace_or_insert_function(
        module,
        "transn_frame_count_for_action_key",
        action_key_frame_count_function(),
        "pub(crate) fn transn_frame_count",
    )

    module = normalize_vec3_float_literals(module)
    if "@generated by tools/generate_source_root_motion_rust.py" not in module:
        module = module.replace(
            "// @generated from resources/melee/frame_data/dolphin_mole/source_manifest.json via frame-data sample; do not edit by hand.",
            "// @generated by tools/generate_source_root_motion_rust.py via Mole frame-data sample; do not edit by hand.",
        )
    args.output.write_text(module, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
