from __future__ import annotations

import argparse
import copy
import json
import re
import subprocess
import sys
from collections import Counter, deque
from dataclasses import dataclass
from pathlib import Path
from typing import Any


PROJECT_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_GRAPH_DIR = PROJECT_ROOT / "docs" / "state_graphs"
DEFAULT_LAYOUT_PATH = PROJECT_ROOT / "config" / "state_graph_layout.json"
DEFAULT_COMMON_DATA_RS = PROJECT_ROOT / "crates" / "mole_core" / "src" / "common_data.rs"
DEFAULT_STATE_RS = PROJECT_ROOT / "crates" / "mole_core" / "src" / "state.rs"
GRAPH_FILES = ("melee_reference_graph.json", "mole_current_graph.json")
DEFAULT_VALUE_SHEET_DIR = DEFAULT_GRAPH_DIR / "value_sheets"
DEFAULT_INPUT_TRACE_DIR = PROJECT_ROOT / "logs"
DEFAULT_SLIPPI_REPORT_DIR = PROJECT_ROOT / "debug" / "slippi"
DEFAULT_ECB_COVERAGE_JSON = DEFAULT_GRAPH_DIR / "parity_reports" / "falcon_ecb_coverage.json"
DEFAULT_MOVE_FRAME_DATA_DIR = PROJECT_ROOT / "resources" / "melee" / "frame_data"
SOURCE_MANIFEST_FILENAME = "source_manifest.json"
INPUT_TRACE_GLOB = "controller-input-trace-*.jsonl"
SLIPPI_REPORT_GLOB = "*.report.md"
VALUE_SHEET_FILES = ("global_common_values.json", "captain_falcon_values.json")
TOOL_TITLE = "Mole Game Dev Tool"
STATE_GRAPHS_TAB_LABEL = "State Graphs"
PARITY_LEDGER_TAB_LABEL = "Parity Ledger"
ECB_COVERAGE_TAB_LABEL = "ECB Coverage"
INPUT_TRACE_TAB_LABEL = "Input Trace"
SLIPPI_REPLAY_TAB_LABEL = "Slippi Replay"
MOVE_KEYFRAMES_TAB_LABEL = "Move Keyframes"
MOVE_FRAME_DATA_CHARACTERS = [
    {"id": "dolphin_mole", "label": "Dolphin Mole"},
    {"id": "test_character_2", "label": "Test Character 2"},
]
GROUNDED_LEDGER_NODE_IDS = (
    "Wait",
    "WalkSlow",
    "WalkMiddle",
    "WalkFast",
    "Turn",
    "Dash",
    "Run",
    "RunBrake",
    "TurnRun",
)
TEST_CHARACTER_VALUES_TAB_LABEL = "Test Character Values"
GLOBAL_VALUES_TAB_LABEL = "Global Values"
CHARACTER_RUST_FIELD_MAP = {
    "walk_initial_velocity": "walk_initial_velocity",
    "walk_accel": "walk_accel",
    "walk_max_vel": "walk_max_velocity",
    "slow_walk_max_velocity": "slow_walk_max_velocity",
    "mid_walk_threshold": "mid_walk_point",
    "fast_walk_threshold": "fast_walk_min",
    "ground_friction": "ground_friction",
    "run_animation_scaling": "run_animation_scaling",
    "max_run_brake_frames": "max_run_brake_frames",
    "ground_max_horizontal_velocity": "ground_max_horizontal_velocity",
    "dash_initial_velocity": "dash_initial_velocity",
    "frames_to_change_direction_on_standing_turn": "standing_turn_direction_change_frames",
    "jump_startup_time": "jumpsquat_frames",
    "jump_h_initial_velocity": "jump_horizontal_initial_velocity",
    "jump_v_initial_velocity": "jump_vertical_initial_velocity",
    "ground_to_air_jump_momentum_multiplier": "ground_to_air_jump_momentum_multiplier",
    "jump_h_max_velocity": "jump_horizontal_max_velocity",
    "hop_v_initial_velocity": "hop_vertical_initial_velocity",
    "air_jump_v_multiplier": "air_jump_vertical_multiplier",
    "air_jump_h_multiplier": "air_jump_horizontal_multiplier",
    "max_jumps": "max_jumps",
    "air_drift_stick_mul": "air_drift_stick_multiplier",
    "aerial_drift_base": "aerial_drift_base",
    "air_drift_max": "air_drift_max",
    "aerial_friction": "aerial_friction",
    "air_max_horizontal_velocity": "air_max_horizontal_velocity",
    "grav": "gravity",
    "terminal_vel": "terminal_velocity",
    "fast_fall_velocity": "fast_fall_velocity",
    "normal_landing_lag": "normal_landing_lag_ticks",
    "landingairn_lag": "landing_air_n_lag_ticks",
    "landingairf_lag": "landing_air_f_lag_ticks",
    "landingairb_lag": "landing_air_b_lag_ticks",
    "landingairhi_lag": "landing_air_hi_lag_ticks",
    "landingairlw_lag": "landing_air_lw_lag_ticks",
    "entry_platform_offset_y": "entry_platform_offset_y",
}
CHARACTER_DERIVED_NOTES: dict[str, str] = {}
MIN_ZOOM = 0.35
MAX_ZOOM = 2.75
ZOOM_STEP = 1.12
SCROLL_REGION_MARGIN = 2400
ONE_TO_ONE_LAYOUT_EQUIVALENTS = {
    "Wait",
    "WalkSlow",
    "WalkMiddle",
    "WalkFast",
    "Turn",
    "Dash",
    "Squat",
    "GuardOn",
    "KneeBend",
    "Run",
    "RunBrake",
    "TurnRun",
    "Guard",
    "GuardOff",
    "EscapeN",
    "EscapeF",
    "EscapeB",
    "JumpF",
    "JumpB",
    "JumpAerialF",
    "JumpAerialB",
    "EscapeAir",
    "Fall",
    "FallF",
    "FallB",
    "FallAerial",
    "FallAerialF",
    "FallAerialB",
    "FallSpecial",
    "FallSpecialF",
    "FallSpecialB",
    "LandingFallSpecial",
    "Landing",
    "LandingAirN",
    "LandingAirF",
    "LandingAirB",
    "LandingAirHi",
    "LandingAirLw",
}


@dataclass(frozen=True)
class StatusStyle:
    fill: str
    outline: str
    text: str
    label: str


STATUS_STYLES = {
    "reference": StatusStyle("#eef2f7", "#64748b", "#0f172a", "Reference"),
    "aligned": StatusStyle("#dcfce7", "#16a34a", "#052e16", "Aligned"),
    "partial": StatusStyle("#fef9c3", "#ca8a04", "#422006", "Partial"),
    "mismatch": StatusStyle("#fee2e2", "#dc2626", "#450a0a", "Mismatch"),
    "missing": StatusStyle("#fecaca", "#991b1b", "#450a0a", "Missing"),
    "intentional": StatusStyle("#dbeafe", "#2563eb", "#172554", "Intentional"),
}
ALLOWED_STATUSES = set(STATUS_STYLES)
LEDGER_LIST_FIELDS = {
    "source_refs",
    "rust_refs",
    "value_refs",
    "known_gaps",
    "physics",
}
REFERENCE_FIELDS = {"source_refs", "rust_refs"}


def load_graphs(graph_dir: Path = DEFAULT_GRAPH_DIR) -> list[dict[str, Any]]:
    graphs = []
    for filename in GRAPH_FILES:
        path = graph_dir / filename
        with path.open("r", encoding="utf-8") as handle:
            graph = json.load(handle)
        errors = validate_graph(graph)
        if errors:
            joined = "\n".join(f"  - {error}" for error in errors)
            raise ValueError(f"{path} is not a valid state graph:\n{joined}")
        graphs.append(graph)
    return graphs


def load_value_sheets(value_sheet_dir: Path = DEFAULT_VALUE_SHEET_DIR) -> list[dict[str, Any]]:
    sheets = []
    for filename in VALUE_SHEET_FILES:
        path = value_sheet_dir / filename
        with path.open("r", encoding="utf-8") as handle:
            sheets.append(json.load(handle))
    return sheets


def load_ecb_coverage(path: Path = DEFAULT_ECB_COVERAGE_JSON) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def list_move_frame_data_characters(
    frame_data_dir: Path = DEFAULT_MOVE_FRAME_DATA_DIR,
) -> list[dict[str, Any]]:
    characters = []
    for character in MOVE_FRAME_DATA_CHARACTERS:
        character_dir = frame_data_dir / character["id"]
        state_records = (
            list_move_frame_data_states(frame_data_dir, character["id"])
            if character_dir.is_dir()
            else []
        )
        characters.append(
            {
                "id": character["id"],
                "label": character["label"],
                "populated": bool(state_records),
            }
        )
    return characters


def _is_move_frame_data_artifact(data: dict[str, Any]) -> bool:
    if data.get("artifact_kind") == "source_character_frame_data_manifest":
        return False
    return isinstance(data.get("keyframes"), list)


def _load_move_frame_data_state_record(path: Path) -> dict[str, Any] | None:
    try:
        data = _read_json_file(path)
    except (OSError, ValueError, json.JSONDecodeError):
        return None
    if not _is_move_frame_data_artifact(data):
        return None
    return {
        "state": str(data.get("state") or path.stem),
        "label": str(data.get("label") or path.stem),
        "populated": True,
    }


def list_move_frame_data_states(
    frame_data_dir: Path,
    character_id: str,
) -> list[dict[str, Any]]:
    character_dir = frame_data_dir / character_id
    if not character_dir.is_dir():
        return []
    states: list[dict[str, Any]] = []
    for path in sorted(character_dir.glob("*.json")):
        if path.name == SOURCE_MANIFEST_FILENAME:
            continue
        state_record = _load_move_frame_data_state_record(path)
        if state_record is not None:
            states.append(state_record)
    expanded_states = {record["state"] for record in states}
    manifest = load_move_frame_data_manifest(frame_data_dir, character_id)
    if manifest:
        for action in manifest.get("actions", []):
            if not isinstance(action, dict):
                continue
            state = str(action.get("state") or "")
            if not state or state in expanded_states:
                continue
            label = str(action.get("source_action_key") or state)
            states.append(
                {
                    "state": state,
                    "label": label,
                    "populated": True,
                    "source": "source_manifest",
                }
            )
    return states


def load_move_frame_data_manifest(
    frame_data_dir: Path,
    character_id: str,
) -> dict[str, Any] | None:
    path = frame_data_dir / character_id / SOURCE_MANIFEST_FILENAME
    if not path.exists():
        return None
    try:
        data = _read_json_file(path)
    except (OSError, ValueError, json.JSONDecodeError):
        return None
    if data.get("artifact_kind") != "source_character_frame_data_manifest":
        return None
    return data


def load_move_frame_data(
    frame_data_dir: Path,
    character_id: str,
    state: str,
) -> dict[str, Any]:
    path = frame_data_dir / character_id / f"{state}.json"
    if path.exists():
        return _read_json_file(path)
    manifest = load_move_frame_data_manifest(frame_data_dir, character_id)
    if manifest:
        for action in manifest.get("actions", []):
            if isinstance(action, dict) and action.get("state") == state:
                return build_manifest_action_frame_data(manifest, action)
    return _read_json_file(path)


def build_manifest_action_frame_data(
    manifest: dict[str, Any],
    action: dict[str, Any],
) -> dict[str, Any]:
    projection = copy.deepcopy(manifest.get("projection", {}))
    projection.setdefault("default_view", "xy")
    projection.setdefault(
        "z_policy",
        manifest.get("z_policy", "preserve_source_z_flatten_after_runtime_projection"),
    )
    decoded_action_script = copy.deepcopy(action.get("decoded_action_script") or {})
    procedures = decoded_action_script.get("procedures")
    procedure_count = len(procedures) if isinstance(procedures, list) else 0
    state = str(action.get("state") or action.get("source_action_key") or "Unknown")
    label = str(action.get("source_action_key") or state)
    return {
        "schema_version": manifest.get("schema_version", 2),
        "artifact_kind": "source_manifest_action_view",
        "target_character": manifest.get("target_character"),
        "target_character_label": manifest.get("target_character_label"),
        "source_character": manifest.get("source_character"),
        "source_character_label": manifest.get("source_character_label"),
        "state": state,
        "label": label,
        "projection": projection,
        "summary": {
            "total_frames": action.get("total_frames", "unknown"),
            "active_hitbox_windows": [],
            "active_hurtbox_windows": [],
            "active_body_volume_windows": [],
            "decoded_procedure_count": procedure_count,
        },
        "keyframes": [],
        "decoded_action_script": decoded_action_script,
        "manifest_action": copy.deepcopy(action),
        "sources": copy.deepcopy(manifest.get("sources", [])),
        "gaps": [
            {
                "field": "frame_capsules",
                "reason": "compact source manifest action has not been expanded through the Melee JObj/FigaTree sampler yet",
            }
        ],
    }


def sample_manifest_action_frame(
    frame_data_dir: Path,
    character_id: str,
    state: str,
    frame_number: int,
) -> dict[str, Any] | None:
    manifest = load_move_frame_data_manifest(frame_data_dir, character_id)
    if not manifest:
        return None
    action = next(
        (
            action
            for action in manifest.get("actions", [])
            if isinstance(action, dict) and action.get("state") == state
        ),
        None,
    )
    if action is None:
        return None
    source_character = str(
        manifest.get("source_character")
        or action.get("source_character")
        or ""
    )
    command = [
        "cargo",
        "run",
        "-q",
        "-p",
        "mole_cli",
        "--",
        "--root",
        str(PROJECT_ROOT),
        "frame-data",
        "sample",
        "--character",
        character_id,
        "--source-character",
        source_character,
        "--state",
        state,
        "--frame",
        str(frame_number),
        "--json",
    ]
    result = subprocess.run(
        command,
        cwd=PROJECT_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        return None
    try:
        payload = json.loads(result.stdout)
    except json.JSONDecodeError:
        return None
    sample = payload.get("sample")
    if not isinstance(sample, dict):
        return None
    hitboxes = sample.get("hit_capsules", [])
    hurtboxes = sample.get("hurt_capsules", [])
    if not isinstance(hitboxes, list) or not isinstance(hurtboxes, list):
        return None
    return {
        "frame": int(sample.get("frame", frame_number)),
        "hitboxes": hitboxes,
        "hurtboxes": hurtboxes,
        "body_volumes": sample.get("body_volumes", []),
        "sample_source": "frame-data sample",
        "source_action_key": sample.get("source_action_key"),
        "source_action_name": sample.get("source_action_name"),
        "projected_view_kind": sample.get("projected_view_kind"),
    }


def build_move_keyframe_empty_state(character_label: str) -> str:
    return (
        f"No move frame data populated for {character_label}.\n\n"
        "Use `mole frame-data extract --character dolphin_mole --source-character captain --state AttackAirN` "
        "to inspect extracted source data, or switch back to Dolphin Mole."
    )


def project_move_point(point: dict[str, Any], projection: dict[str, Any]) -> dict[str, float]:
    x = float(point.get("x", 0.0))
    y = float(point.get("y", 0.0))
    z = float(point.get("z", 0.0))
    default_view = projection.get("default_view", "xy")
    if default_view == "xz":
        view_x, view_y = x, z
    elif default_view == "yz":
        view_x, view_y = y, z
    else:
        view_x, view_y = x, y
    return {"x": x, "y": y, "z": z, "view_x": view_x, "view_y": view_y}


def _format_xyz(point: dict[str, Any]) -> str:
    return "x={x}, y={y}, z={z}".format(
        x=point.get("x", "unknown"),
        y=point.get("y", "unknown"),
        z=point.get("z", "unknown"),
    )


def frame_has_active_hitbox(data: dict[str, Any], frame_number: int) -> bool:
    windows = data.get("summary", {}).get("active_hitbox_windows", [])
    for window in windows:
        try:
            start = int(window.get("start", 0))
            end = int(window.get("end", 0))
        except (TypeError, ValueError):
            continue
        if start <= frame_number <= end:
            return True
    return False


def nearest_move_keyframe(data: dict[str, Any], frame_number: int) -> dict[str, Any] | None:
    keyframes = sorted(
        data.get("keyframes", []),
        key=lambda frame: int(frame.get("frame", 0)),
    )
    selected = None
    for frame in keyframes:
        if int(frame.get("frame", 0)) <= frame_number:
            selected = frame
        else:
            break
    return selected or (keyframes[0] if keyframes else None)


def format_move_keyframe_details(data: dict[str, Any], frame: dict[str, Any]) -> str:
    lines = [
        f"{data.get('target_character_label', data.get('target_character', 'Unknown'))} - {data.get('label', data.get('state', 'Unknown'))}",
        f"Frame {frame.get('frame', '?')}",
        f"Projection: {data.get('projection', {}).get('default_view', 'xy')} ({data.get('projection', {}).get('z_policy', 'preserve_and_project')})",
        "",
        "Hitboxes:",
    ]
    hitboxes = frame.get("hitboxes", [])
    if hitboxes:
        for hitbox in hitboxes:
            center = hitbox.get("center", {})
            source_center = hitbox.get("source_center", {})
            source_center_text = ""
            if source_center:
                source_center_text = " source_center=(x={x}, y={y}, z={z})".format(
                    x=source_center.get("x", "unknown"),
                    y=source_center.get("y", "unknown"),
                    z=source_center.get("z", "unknown"),
                )
            lines.append(
                "hitbox[{id}] bone={bone} center=(x={x}, y={y}, z={z}){source_center}".format(
                    id=hitbox.get("id", "?"),
                    bone=hitbox.get("bone", "unknown"),
                    x=center.get("x", "unknown"),
                    y=center.get("y", "unknown"),
                    z=center.get("z", "unknown"),
                    source_center=source_center_text,
                )
            )
            value_fields = [
                "radius",
                "damage",
                "angle",
                "kbg",
                "bkb",
                "weight_set_kb",
                "element",
                "shield_damage",
                "hit_grounded",
                "hit_aerial",
                "hit_group",
                "use_common_bone_ids",
                "previous_center",
                "source_previous_center",
                "source_offset",
                "source_pose_joint",
                "source_space",
                "source_hit_capsule_state",
                "source_sweep",
                "source_handler",
                "source_word_offset",
                "confidence",
                "source",
            ]
            for field in value_fields:
                if isinstance(hitbox.get(field), dict):
                    lines.append(f"  {field}=({_format_xyz(hitbox[field])})")
                elif field in hitbox:
                    lines.append(f"  {field}={hitbox[field]}")
    else:
        lines.append("none")
    lines.extend(["", "Hurtboxes:"])
    hurtboxes = frame.get("hurtboxes", [])
    if hurtboxes:
        for hurtbox in hurtboxes:
            a = hurtbox.get("a", {})
            b = hurtbox.get("b", {})
            source_a = hurtbox.get("source_a", {})
            source_b = hurtbox.get("source_b", {})
            source_text = ""
            if source_a:
                source_text += f" source_a=({_format_xyz(source_a)})"
            if source_b:
                source_text += f" source_b=({_format_xyz(source_b)})"
            lines.append(
                "hurtbox[{id}] bone={bone} a=({a}) b=({b}) radius={radius}{source_text}".format(
                    id=hurtbox.get("id", "?"),
                    bone=hurtbox.get("bone", "unknown"),
                    a=_format_xyz(a),
                    b=_format_xyz(b),
                    radius=hurtbox.get("radius", "unknown"),
                    source_text=source_text,
                )
            )
            for field in [
                "radius",
                "scale",
                "height",
                "is_grabbable",
                "state",
                "state_raw",
                "state_source",
                "source_handler",
                "source_word_offset",
                "confidence",
                "source",
            ]:
                if field in hurtbox:
                    lines.append(f"  {field}={hurtbox[field]}")
            for field in [
                "a_pos",
                "b_pos",
                "source_a_pos",
                "source_b_pos",
                "a_offset",
                "b_offset",
            ]:
                if isinstance(hurtbox.get(field), dict):
                    lines.append(f"  {field}=({_format_xyz(hurtbox[field])})")
            for field in [
                "source_init_handler",
                "source_update_handler",
                "source_draw_handler",
                "source_render_endpoints",
                "source_render_radius",
                "source_color_table",
                "source_skip_update_pos_after_transform",
                "source_z_policy",
            ]:
                if field in hurtbox:
                    lines.append(f"  {field}={hurtbox[field]}")
    else:
        lines.append("none")
    lines.extend(["", "Body volumes:"])
    body_volumes = frame.get("body_volumes", [])
    if body_volumes:
        for body_volume in body_volumes:
            lines.append(
                "body_volume[{id}] kind={kind} top=({top}) right=({right}) bottom=({bottom}) left=({left})".format(
                    id=body_volume.get("id", "?"),
                    kind=body_volume.get("kind", "unknown"),
                    top=_format_xyz(body_volume.get("top", {})),
                    right=_format_xyz(body_volume.get("right", {})),
                    bottom=_format_xyz(body_volume.get("bottom", {})),
                    left=_format_xyz(body_volume.get("left", {})),
                )
            )
            for field in ["source_top", "source_right", "source_bottom", "source_left"]:
                if isinstance(body_volume.get(field), dict):
                    lines.append(f"  {field}=({_format_xyz(body_volume[field])})")
            for field in ["source_joint_indices", "confidence", "source"]:
                if field in body_volume:
                    lines.append(f"  {field}={body_volume[field]}")
    else:
        lines.append("none")
    lines.extend(["", "Sources:"])
    for source in data.get("sources", []):
        line = source.get("line")
        suffix = f":{line}" if line else ""
        lines.append(f"- {source.get('kind', 'source')}: {source.get('path', 'unknown')}{suffix}")
    return "\n".join(lines)


def format_move_frame_data_summary(data: dict[str, Any]) -> str:
    projection = data.get("projection", {})
    summary = data.get("summary", {})
    lines = [
        "Move Keyframes",
        "",
        f"Character: {data.get('target_character_label', data.get('target_character', 'Unknown'))}",
        f"Source Character: {data.get('source_character_label', data.get('source_character', 'Unknown'))}",
        f"State: {data.get('state', 'Unknown')} ({data.get('label', 'Unknown')})",
        f"Projection: {projection.get('default_view', 'xy')} / {projection.get('z_policy', 'preserve_and_project')}",
        f"Total frames: {summary.get('total_frames', 'unknown')}",
    ]
    if data.get("artifact_kind") == "source_manifest_action_view":
        action = data.get("manifest_action", {})
        lines.extend(
            [
                "Compact source manifest action",
                f"Action state id: {action.get('action_state_id', 'unknown')}",
                f"Subaction script offset: {action.get('subaction_script_offset', 'unknown')}",
                f"Decoded procedures: {summary.get('decoded_procedure_count', 0)}",
                "Frame capsules: sampled on demand via `frame-data sample`",
            ]
        )
    windows = summary.get("active_hitbox_windows", [])
    if windows:
        ranges = [f"{window.get('start', '?')}-{window.get('end', '?')}" for window in windows]
        lines.append(f"Hitbox active frames: {', '.join(ranges)}")
    hurtbox_windows = summary.get("active_hurtbox_windows", [])
    if hurtbox_windows:
        ranges = [
            f"{window.get('start', '?')}-{window.get('end', '?')}"
            for window in hurtbox_windows
        ]
        lines.append(f"Hurtbox active frames: {', '.join(ranges)}")
    body_volume_windows = summary.get("active_body_volume_windows", [])
    if body_volume_windows:
        ranges = [
            f"{window.get('start', '?')}-{window.get('end', '?')}"
            for window in body_volume_windows
        ]
        lines.append(f"Body volume active frames: {', '.join(ranges)}")
    lines.extend(["", "Keyframes:"])
    for frame in data.get("keyframes", []):
        lines.append(
            "Frame {frame}: {hitboxes} hitbox(es), {hurtboxes} hurtbox(es), {body_volumes} body volume(s)".format(
                frame=frame.get("frame", "?"),
                hitboxes=len(frame.get("hitboxes", [])),
                hurtboxes=len(frame.get("hurtboxes", [])),
                body_volumes=len(frame.get("body_volumes", [])),
            )
        )
    gaps = data.get("gaps", [])
    if gaps:
        lines.extend(["", "Gaps:"])
        for gap in gaps:
            lines.append(f"- {gap.get('field', 'unknown')}: {gap.get('reason', '')}")
    return "\n".join(lines)


def latest_input_trace_path(log_dir: Path = DEFAULT_INPUT_TRACE_DIR) -> Path | None:
    traces = sorted(
        log_dir.glob(INPUT_TRACE_GLOB),
        key=lambda path: (path.stat().st_mtime, path.name),
        reverse=True,
    )
    return traces[0] if traces else None


def latest_slippi_report_path(report_dir: Path = DEFAULT_SLIPPI_REPORT_DIR) -> Path | None:
    reports = sorted(
        report_dir.glob(SLIPPI_REPORT_GLOB),
        key=lambda path: (path.stat().st_mtime, path.name),
        reverse=True,
    )
    return reports[0] if reports else None


def load_recent_input_trace_rows(path: Path, limit: int = 120) -> list[dict[str, Any]]:
    rows: deque[dict[str, Any]] = deque(maxlen=max(1, limit))
    with path.open("r", encoding="utf-8") as handle:
        for line in handle:
            line = line.strip()
            if not line:
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError:
                continue
            if isinstance(row, dict):
                rows.append(row)
    return list(rows)


def format_recent_input_trace(path: Path | None = None, limit: int = 90) -> str:
    path = path or latest_input_trace_path()
    if path is None:
        return (
            "Input Trace\n\n"
            "No controller input trace found yet. Run the SDL3 runtime, move the controller, then refresh this tab."
        )

    rows = load_recent_input_trace_rows(path, limit=limit)
    lines = [
        "Input Trace",
        "",
        f"File: {path.name}",
        f"Frames shown: {len(rows)}",
        "",
    ]
    if not rows:
        lines.append("Trace file is present but does not contain readable JSONL rows yet.")
        return "\n".join(lines)

    lines.extend(_format_input_trace_row(row) for row in rows)
    return "\n".join(lines)


def format_latest_slippi_report(
    path: Path | None = None,
    report_dir: Path = DEFAULT_SLIPPI_REPORT_DIR,
) -> str:
    path = path or latest_slippi_report_path(report_dir)
    if path is None:
        return (
            "Slippi Replay\n\n"
            "No Slippi replay diagnostic report found yet. Generate one with:\n"
            "node tools\\slippi_replay_to_inputs.cjs --replay replays\\Game.slp --frames 1800"
        )

    return "\n".join(
        [
            "Slippi Replay",
            "",
            f"File: {path.name}",
            "",
            path.read_text(encoding="utf-8"),
        ]
    )


def _format_input_trace_row(row: dict[str, Any]) -> str:
    frame = row.get("frame", "?")
    wup_player = _first_player(row.get("wup", {}).get("players", []))
    after_player = _first_player(row.get("after", {}).get("players", []))
    melee = wup_player.get("melee", {}) if isinstance(wup_player, dict) else {}
    core_facts = after_player.get("core_facts", {}) if isinstance(after_player, dict) else {}
    raw = _axis_pair(wup_player.get("raw", {}) if isinstance(wup_player, dict) else {})
    native = _axis_pair(wup_player.get("native", {}) if isinstance(wup_player, dict) else {})
    ucf = _axis_pair(wup_player.get("ucf", {}) if isinstance(wup_player, dict) else {})
    dashback = wup_player.get("dashback_amendment") if isinstance(wup_player, dict) else None
    tap = melee.get("x_tap_timer", "?")
    dash = core_facts.get("dash_direction", melee.get("dash_direction", "?"))
    held_dash = core_facts.get("held_dash_x_direction", melee.get("held_dash_x_direction", "?"))
    state = after_player.get("motion_state", "?") if isinstance(after_player, dict) else "?"
    state_frame = after_player.get("state_frame", "?") if isinstance(after_player, dict) else "?"
    velocity_x = after_player.get("velocity_x", "?") if isinstance(after_player, dict) else "?"
    velocity_y = after_player.get("velocity_y", "?") if isinstance(after_player, dict) else "?"

    return (
        f"F{frame} raw={raw} native={native} ucf={ucf} "
        f"tap={tap} dash={dash} held_dash={held_dash} "
        f"state={state} sf={state_frame} vx={velocity_x} vy={velocity_y} "
        f"ucf_db={dashback}"
    )


def _first_player(players: Any) -> dict[str, Any]:
    if isinstance(players, list) and players and isinstance(players[0], dict):
        return players[0]
    return {}


def _axis_pair(pad: dict[str, Any]) -> str:
    return f"({pad.get('main_x', '?')},{pad.get('main_y', '?')})"


def load_rust_global_values(path: Path = DEFAULT_COMMON_DATA_RS) -> dict[str, Any]:
    text = path.read_text(encoding="utf-8")
    block = _extract_rust_initializer_block(text, "pub const PROVISIONAL: Self = Self {")
    return _parse_rust_initializer_values(block)


def load_rust_character_values(path: Path = DEFAULT_STATE_RS) -> dict[str, Any]:
    text = path.read_text(encoding="utf-8")
    block = _extract_rust_initializer_block(
        text,
        "pub const FALCON_LIKE: Self = Self {",
        after="impl FighterProfile",
    )
    return _parse_rust_initializer_values(block)


def apply_saved_layout(
    graphs: list[dict[str, Any]],
    layout_path: Path = DEFAULT_LAYOUT_PATH,
) -> None:
    if not layout_path.exists():
        return
    with layout_path.open("r", encoding="utf-8") as handle:
        payload = json.load(handle)
    saved_graphs = payload.get("graphs", {})
    for graph in graphs:
        saved_graph = saved_graphs.get(graph["id"], {})
        if isinstance(saved_graph, dict) and isinstance(saved_graph.get("zoom"), (int, float)):
            graph["zoom"] = clamp_zoom(saved_graph["zoom"])
        saved_nodes = saved_graph.get("nodes", saved_graph)
        for node in graph["nodes"]:
            saved_pos = saved_nodes.get(node["id"])
            if _is_position(saved_pos):
                node["pos"] = saved_pos


def clamp_zoom(value: float) -> float:
    return max(MIN_ZOOM, min(MAX_ZOOM, round(float(value), 4)))


def zoom_from_wheel_delta(current_zoom: float, wheel_delta: int) -> float:
    if wheel_delta > 0:
        return clamp_zoom(current_zoom * ZOOM_STEP)
    if wheel_delta < 0:
        return clamp_zoom(current_zoom / ZOOM_STEP)
    return clamp_zoom(current_zoom)


def equivalent_layout_targets(source_graph_id: str, source_node_id: str) -> list[tuple[str, str]]:
    if source_graph_id != "mole_current":
        return []
    if source_node_id not in ONE_TO_ONE_LAYOUT_EQUIVALENTS:
        return []
    return [("melee_reference", source_node_id)]


def equivalent_edge_targets(
    graphs: list[dict[str, Any]],
    source_graph_id: str,
    source_edge_index: int,
) -> list[tuple[str, int]]:
    graph_by_id = {graph["id"]: graph for graph in graphs}
    source_graph = graph_by_id.get(source_graph_id)
    reference = graph_by_id.get("melee_reference")
    if source_graph_id != "mole_current" or source_graph is None or reference is None:
        return []
    try:
        source_edge = source_graph["edges"][source_edge_index]
    except IndexError:
        return []
    if (
        source_edge["from"] not in ONE_TO_ONE_LAYOUT_EQUIVALENTS
        or source_edge["to"] not in ONE_TO_ONE_LAYOUT_EQUIVALENTS
    ):
        return []
    matches = []
    for index, edge in enumerate(reference["edges"]):
        if edge["from"] == source_edge["from"] and edge["to"] == source_edge["to"]:
            matches.append(("melee_reference", index))
    return matches


def pin_edge_label(graph: dict[str, Any], edge_index: int) -> None:
    graph.setdefault("_pinned_edge_labels", set()).add(edge_index)


def unpin_edge_label(graph: dict[str, Any], edge_index: int) -> None:
    graph.setdefault("_pinned_edge_labels", set()).discard(edge_index)


def toggle_edge_label_pin(graph: dict[str, Any], edge_index: int) -> bool:
    pinned = graph.setdefault("_pinned_edge_labels", set())
    if edge_index in pinned:
        pinned.remove(edge_index)
        return False
    pinned.add(edge_index)
    return True


def clear_pinned_edge_labels(graphs: list[dict[str, Any]]) -> None:
    for graph in graphs:
        graph.setdefault("_pinned_edge_labels", set()).clear()


def set_global_edge_labels(graphs: list[dict[str, Any]], show_all: bool) -> None:
    if not show_all:
        clear_pinned_edge_labels(graphs)


def is_edge_label_visible(
    graph: dict[str, Any],
    edge_index: int,
    *,
    show_all: bool,
) -> bool:
    return show_all or edge_index in graph.setdefault("_pinned_edge_labels", set())


def apply_linked_node_delta(
    graphs: list[dict[str, Any]],
    source_graph_id: str,
    source_node_id: str,
    delta_x: float,
    delta_y: float,
) -> int:
    graph_by_id = {graph["id"]: graph for graph in graphs}
    moved = 0
    for target_graph_id, target_node_id in equivalent_layout_targets(
        source_graph_id,
        source_node_id,
    ):
        target_graph = graph_by_id.get(target_graph_id)
        if target_graph is None:
            continue
        target_node = _find_node(target_graph, target_node_id)
        if target_node is None:
            continue
        target_node["pos"] = [
            round(target_node["pos"][0] + delta_x, 3),
            round(target_node["pos"][1] + delta_y, 3),
        ]
        moved += 1
    return moved


def save_layout(
    graphs: list[dict[str, Any]],
    layout_path: Path = DEFAULT_LAYOUT_PATH,
) -> None:
    payload = {
        "version": 1,
        "graphs": {
            graph["id"]: {
                "zoom": round(float(graph.get("zoom", 1.0)), 3),
                "nodes": {
                    node["id"]: [round(node["pos"][0], 3), round(node["pos"][1], 3)]
                    for node in graph["nodes"]
                },
            }
            for graph in graphs
        },
    }
    layout_path.parent.mkdir(parents=True, exist_ok=True)
    with layout_path.open("w", encoding="utf-8") as handle:
        json.dump(payload, handle, indent=2)
        handle.write("\n")


def validate_graph(graph: dict[str, Any]) -> list[str]:
    errors = []
    required = ("id", "title", "root", "nodes", "edges")
    for key in required:
        if key not in graph:
            errors.append(f"missing top-level key {key!r}")

    nodes = graph.get("nodes", [])
    edges = graph.get("edges", [])
    if not isinstance(nodes, list):
        errors.append("'nodes' must be a list")
        nodes = []
    if not isinstance(edges, list):
        errors.append("'edges' must be a list")
        edges = []

    node_ids = set()
    for index, node in enumerate(nodes):
        node_id = node.get("id")
        if not node_id:
            errors.append(f"node {index} missing id")
            continue
        if node_id in node_ids:
            errors.append(f"duplicate node id {node_id!r}")
        node_ids.add(node_id)
        status = node.get("status")
        if status not in ALLOWED_STATUSES:
            errors.append(f"node {node_id!r} has unknown status {status!r}")
        if not _is_position(node.get("pos")):
            errors.append(f"node {node_id!r} must have numeric [x, y] pos")
        errors.extend(validate_ledger_fields(node, item_label=f"node {node_id!r}"))

    root = graph.get("root")
    if root and root not in node_ids:
        errors.append(f"root {root!r} is not present in nodes")

    for index, edge in enumerate(edges):
        source = edge.get("from")
        target = edge.get("to")
        if source not in node_ids:
            errors.append(f"edge {index} source {source!r} is not a node")
        if target not in node_ids:
            errors.append(f"edge {index} target {target!r} is not a node")
        for key in ("input", "frames"):
            if not edge.get(key):
                errors.append(f"edge {index} missing {key!r}")
        status = edge.get("status")
        if status not in ALLOWED_STATUSES:
            errors.append(f"edge {index} has unknown status {status!r}")
        errors.extend(validate_ledger_fields(edge, item_label=f"edge {index}"))

    return errors


def summarize_comparison(graph: dict[str, Any]) -> dict[str, int]:
    counter = Counter()
    for collection_name in ("nodes", "edges"):
        for item in graph.get(collection_name, []):
            status = item.get("status")
            if status in ALLOWED_STATUSES and status != "reference":
                counter[status] += 1
    return {status: counter[status] for status in sorted(ALLOWED_STATUSES - {"reference"})}


def summarize_value_sheet(sheet: dict[str, Any]) -> dict[str, int]:
    categories = sheet.get("categories", [])
    field_count = sum(len(category.get("fields", [])) for category in categories)
    return {"categories": len(categories), "fields": field_count}


def build_value_comparison_rows(
    sheet: dict[str, Any],
    rust_values: dict[str, Any],
    *,
    character_value: bool = False,
) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    field_map = CHARACTER_RUST_FIELD_MAP if character_value else {}
    derived_notes = CHARACTER_DERIVED_NOTES if character_value else {}
    for category in sheet.get("categories", []):
        for field in category.get("fields", []):
            field_name = field["rust_name"]
            rust_field = field_map.get(field_name, field_name)
            rust_value = rust_values.get(rust_field)
            decomp_value = field.get("converted_value")
            note = derived_notes.get(field_name, "")
            if field_name in derived_notes:
                status = "derived"
            elif rust_field not in rust_values:
                status = "missing"
            elif rust_value == decomp_value:
                status = "match"
            else:
                status = "diff"
            rows.append(
                {
                    "category": category["id"],
                    "field": field_name,
                    "source_field": field["source_name"],
                    "offset": field["offset_hex"],
                    "decomp_value": decomp_value,
                    "rust_field": rust_field,
                    "rust_value": rust_value,
                    "status": status,
                    "provenance": field["provenance"],
                    "kind": field["kind"],
                    "raw": field.get("raw"),
                    "note": note,
                }
            )
    return rows


def build_parity_ledger_overview(
    graphs: list[dict[str, Any]],
    value_sheets: list[dict[str, Any]],
) -> str:
    graph_by_id = {graph["id"]: graph for graph in graphs}
    mole = graph_by_id.get("mole_current", {})
    nodes_by_id = {node["id"]: node for node in mole.get("nodes", [])}
    covered_nodes = [
        node_id
        for node_id in GROUNDED_LEDGER_NODE_IDS
        if _has_grounded_ledger_coverage(nodes_by_id.get(node_id, {}))
    ]
    dash_edges = [
        edge
        for edge in mole.get("edges", [])
        if edge.get("from") == "Dash" or edge.get("to") == "Dash"
    ]
    dash_physics_edges = [edge for edge in dash_edges if edge.get("physics")]
    dash_gaps = nodes_by_id.get("Dash", {}).get("known_gaps", [])

    lines = [
        "Parity Ledger",
        "",
        "Value sheets:",
    ]
    for sheet in value_sheets:
        summary = summarize_value_sheet(sheet)
        lines.append(f"- {sheet['id']}: {summary['categories']} categories, {summary['fields']} fields")
    lines.extend(
        [
            "",
            f"Grounded ledger coverage: {len(covered_nodes)}/{len(GROUNDED_LEDGER_NODE_IDS)} nodes",
            "- " + ", ".join(covered_nodes),
            f"Dash-related physics edges: {len(dash_physics_edges)}/{len(dash_edges)}",
        ]
    )
    if dash_gaps:
        lines.extend(["", "Dash known gaps:"])
        lines.extend(f"- {gap}" for gap in dash_gaps)
    return "\n".join(lines)


def format_ecb_coverage(coverage: dict[str, Any]) -> str:
    mapped = coverage.get("mapped_motion_states", [])
    missing = coverage.get("missing_sampled_mappings", [])
    unmapped = coverage.get("unmapped_derived_motion_states", [])
    action_groups: dict[Any, list[str]] = {}
    for row in mapped:
        if not isinstance(row, dict):
            continue
        action_id = row.get("action_state_id")
        motion_state = row.get("motion_state")
        if action_id is None or motion_state is None:
            continue
        action_groups.setdefault(action_id, []).append(str(motion_state))
    shared_aliases = [
        (action_id, motion_states)
        for action_id, motion_states in action_groups.items()
        if len(motion_states) > 1
    ]
    lines = [
        str(coverage.get("title", "ECB Coverage")),
        "",
        f"Samples: {coverage.get('source', {}).get('samples', '?')}",
        f"Generated Rust: {coverage.get('source', {}).get('generated_rust', '?')}",
        f"Mapped exact action-table states: {coverage.get('mapped_motion_state_count', len(mapped))}",
        f"Mapped exact action ids: {coverage.get('mapped_action_count', '?')}",
        f"Missing sampled mappings: {len(missing)}",
        "",
        "Unmapped derived/current Rust states:",
    ]
    if unmapped:
        lines.extend(f"- {state}" for state in unmapped)
    else:
        lines.append("- none")
    lines.extend(["", "Shared source-action aliases:"])
    if shared_aliases:
        lines.extend(
            f"- action {action_id}: {', '.join(motion_states)}"
            for action_id, motion_states in shared_aliases
        )
    else:
        lines.append("- none")
    lines.extend(
        [
            "",
            "Shared aliases must point at the exact Melee source action data they use at runtime.",
            "",
            "Mapped states:",
        ]
    )
    for row in mapped:
        lines.append(
            "- {motion_state} -> action {action_state_id} ({sample_frames} frames, {status})".format(
                **row
            )
        )
    return "\n".join(lines)


def _has_grounded_ledger_coverage(node: dict[str, Any]) -> bool:
    return bool(node.get("source_refs") and node.get("rust_refs") and node.get("value_refs"))


def _extract_rust_initializer_block(text: str, marker: str, *, after: str | None = None) -> str:
    search_start = text.index(after) if after else 0
    marker_index = text.index(marker, search_start)
    open_index = text.index("{", marker_index)
    depth = 0
    for index in range(open_index, len(text)):
        character = text[index]
        if character == "{":
            depth += 1
        elif character == "}":
            depth -= 1
            if depth == 0:
                return text[open_index + 1:index]
    raise ValueError(f"could not find closing brace for {marker!r}")


def _parse_rust_initializer_values(block: str) -> dict[str, Any]:
    values: dict[str, Any] = {}
    for line in block.splitlines():
        line = line.split("//", 1)[0].strip()
        if ":" not in line:
            continue
        key, value = line.split(":", 1)
        key = key.strip()
        values[key] = _parse_rust_scalar(value.strip().rstrip(","))
    return values


def _parse_rust_scalar(value: str) -> Any:
    if value.startswith("Some(") and value.endswith(")"):
        return _parse_rust_scalar(value[5:-1])
    if value == "None":
        return None
    if value.startswith('"') and value.endswith('"'):
        return value[1:-1]
    if re.fullmatch(r"-?\d[\d_]*", value):
        return int(value.replace("_", ""))
    if re.fullmatch(r"-?(?:\d[\d_]*)?\.\d[\d_]*(?:[eE][+-]?\d[\d_]*)?", value):
        return float(value.replace("_", ""))
    return value


def format_ledger_details(item: dict[str, Any]) -> str:
    sections: list[str] = []
    _append_ref_section(sections, "Source", item.get("source_refs", []))
    _append_ref_section(sections, "Rust", item.get("rust_refs", []))
    _append_string_section(sections, "Values", item.get("value_refs", []))
    _append_string_section(sections, "Physics", item.get("physics", []))
    _append_string_section(sections, "Known gaps", item.get("known_gaps", []))
    return "\n\n".join(sections)


def _append_ref_section(sections: list[str], label: str, refs: list[dict[str, Any]]) -> None:
    if not refs:
        return
    lines = [f"{label}:"]
    for ref in refs:
        target = ref.get("path") or ref.get("url") or ""
        function = ref.get("function")
        suffix = f" ({function})" if function else ""
        lines.append(f"- {ref['label']}: {target}{suffix}")
    sections.append("\n".join(lines))


def _append_string_section(sections: list[str], label: str, values: list[str]) -> None:
    if not values:
        return
    sections.append("\n".join([f"{label}:"] + [f"- {value}" for value in values]))


def _find_node(graph: dict[str, Any], node_id: str) -> dict[str, Any] | None:
    for node in graph["nodes"]:
        if node["id"] == node_id:
            return node
    return None


def launch_viewer(
    graphs: list[dict[str, Any]],
    layout_path: Path = DEFAULT_LAYOUT_PATH,
    value_sheets: list[dict[str, Any]] | None = None,
) -> None:
    import tkinter as tk
    from tkinter import ttk

    graphs = copy.deepcopy(graphs)
    value_sheets = copy.deepcopy(value_sheets) if value_sheets is not None else load_value_sheets()
    apply_saved_layout(graphs, layout_path)

    root = tk.Tk()
    root.title(TOOL_TITLE)
    root.geometry("1540x940")
    root.minsize(1100, 760)

    notebook = ttk.Notebook(root)
    notebook.pack(fill=tk.BOTH, expand=True)

    graphs_tab = tk.Frame(notebook, bg="#ffffff")
    ledger_tab = tk.Frame(notebook, bg="#ffffff")
    ecb_coverage_tab = tk.Frame(notebook, bg="#ffffff")
    input_trace_tab = tk.Frame(notebook, bg="#ffffff")
    slippi_replay_tab = tk.Frame(notebook, bg="#ffffff")
    move_keyframes_tab = tk.Frame(notebook, bg="#ffffff")
    notebook.add(graphs_tab, text=STATE_GRAPHS_TAB_LABEL)
    notebook.add(ledger_tab, text=PARITY_LEDGER_TAB_LABEL)
    notebook.add(ecb_coverage_tab, text=ECB_COVERAGE_TAB_LABEL)
    notebook.add(input_trace_tab, text=INPUT_TRACE_TAB_LABEL)
    notebook.add(slippi_replay_tab, text=SLIPPI_REPLAY_TAB_LABEL)
    notebook.add(move_keyframes_tab, text=MOVE_KEYFRAMES_TAB_LABEL)

    show_labels = tk.BooleanVar(value=False)
    curved_edges = tk.BooleanVar(value=True)
    status_text = tk.StringVar(value="Layout changes are not saved until you click Save Layout.")

    toolbar = tk.Frame(graphs_tab, padx=10, pady=7)
    toolbar.pack(fill=tk.X)
    draw_legend(toolbar)
    tk.Label(toolbar, textvariable=status_text, fg="#475569").pack(side=tk.LEFT, padx=(8, 18))
    tk.Button(
        toolbar,
        text="Save Layout",
        command=lambda: save_current_layout(graphs, layout_path, status_text),
    ).pack(side=tk.RIGHT, padx=(12, 0))
    tk.Checkbutton(
        toolbar,
        text="Curved edge lanes",
        variable=curved_edges,
        command=lambda: [pane.redraw() for pane in panes],
    ).pack(side=tk.RIGHT, padx=(12, 0))
    tk.Checkbutton(
        toolbar,
        text="Show edge labels",
        variable=show_labels,
        command=lambda: on_show_labels_changed(),
    ).pack(side=tk.RIGHT)

    split = tk.PanedWindow(graphs_tab, orient=tk.HORIZONTAL, sashrelief=tk.RAISED)
    split.pack(fill=tk.BOTH, expand=True)
    draw_parity_ledger_tab(ledger_tab, graphs, value_sheets)
    draw_ecb_coverage_tab(ecb_coverage_tab)
    draw_input_trace_tab(input_trace_tab)
    draw_slippi_replay_tab(slippi_replay_tab)
    draw_move_keyframes_tab(move_keyframes_tab)

    panes: list[StateGraphPane] = []

    def on_node_moved(source_pane: StateGraphPane, node_id: str, dx: float, dy: float) -> None:
        moved = apply_linked_node_delta(
            graphs,
            source_pane.graph["id"],
            node_id,
            dx,
            dy,
        )
        if moved:
            status_text.set(f"Moved {moved} equivalent reference node(s). Click Save Layout to keep it.")
            for pane in panes:
                if pane is not source_pane:
                    pane.redraw()
        elif source_pane.graph["id"] == "mole_current" and node_id:
            status_text.set(
                f"No one-to-one Melee reference node for {node_id}; only the Mole node moved. Click Save Layout to keep it."
            )
        else:
            status_text.set("Layout changed. Click Save Layout to keep it.")

    def on_edge_label_pinned(source_pane: StateGraphPane, edge_index: int) -> None:
        pinned = toggle_edge_label_pin(source_pane.graph, edge_index)
        changed_equivalents = 0
        graph_by_id = {graph["id"]: graph for graph in graphs}
        for target_graph_id, target_edge_index in equivalent_edge_targets(
            graphs,
            source_pane.graph["id"],
            edge_index,
        ):
            target_graph = graph_by_id.get(target_graph_id)
            if target_graph is None:
                continue
            if pinned:
                pin_edge_label(target_graph, target_edge_index)
            else:
                unpin_edge_label(target_graph, target_edge_index)
            changed_equivalents += 1
        action = "Pinned" if pinned else "Unpinned"
        if changed_equivalents:
            status_text.set(f"{action} edge label and {changed_equivalents} exact Melee equivalent(s).")
        else:
            status_text.set(f"{action} edge label. No exact Melee equivalent changed.")
        for pane in panes:
            pane.redraw()

    def on_show_labels_changed() -> None:
        show_all = bool(show_labels.get())
        set_global_edge_labels(graphs, show_all)
        if show_all:
            status_text.set("Showing all edge labels; pinned labels are unchanged.")
        else:
            status_text.set("Hid all edge labels and cleared pinned labels.")
        for pane in panes:
            pane.redraw()

    panes = [
        StateGraphPane(
            split,
            graphs[0],
            show_labels,
            curved_edges,
            on_node_moved,
            on_edge_label_pinned,
            "left",
        ),
        StateGraphPane(
            split,
            graphs[1],
            show_labels,
            curved_edges,
            on_node_moved,
            on_edge_label_pinned,
            "right",
        ),
    ]
    for pane in panes:
        split.add(pane.frame, stretch="always")
        pane.redraw()

    root.protocol("WM_DELETE_WINDOW", root.destroy)
    root.mainloop()


def draw_parity_ledger_tab(
    parent: Any,
    graphs: list[dict[str, Any]],
    value_sheets: list[dict[str, Any]],
) -> None:
    import tkinter as tk
    from tkinter import ttk

    frame = tk.Frame(parent, bg="#ffffff", padx=12, pady=12)
    frame.pack(fill=tk.BOTH, expand=True)
    header = tk.Label(
        frame,
        text=PARITY_LEDGER_TAB_LABEL,
        anchor="w",
        bg="#ffffff",
        fg="#0f172a",
        font=("Segoe UI", 13, "bold"),
    )
    header.pack(fill=tk.X, pady=(0, 8))
    summary = tk.Label(
        frame,
        text=build_parity_ledger_overview(graphs, value_sheets),
        justify=tk.LEFT,
        anchor="w",
        bg="#f8fafc",
        fg="#0f172a",
        relief=tk.FLAT,
        padx=12,
        pady=8,
    )
    summary.pack(fill=tk.X, pady=(0, 10))

    sheets_by_id = {sheet["id"]: sheet for sheet in value_sheets}
    ledger_tabs = ttk.Notebook(frame)
    ledger_tabs.pack(fill=tk.BOTH, expand=True)
    draw_value_comparison_tab(
        ledger_tabs,
        GLOBAL_VALUES_TAB_LABEL,
        build_value_comparison_rows(
            sheets_by_id["global_common_values"],
            load_rust_global_values(),
        ),
    )
    draw_value_comparison_tab(
        ledger_tabs,
        TEST_CHARACTER_VALUES_TAB_LABEL,
        build_value_comparison_rows(
            sheets_by_id["captain_falcon_values"],
            load_rust_character_values(),
            character_value=True,
        ),
    )


def draw_ecb_coverage_tab(parent: Any, coverage_path: Path = DEFAULT_ECB_COVERAGE_JSON) -> None:
    import tkinter as tk

    frame = tk.Frame(parent, bg="#ffffff", padx=12, pady=12)
    frame.pack(fill=tk.BOTH, expand=True)

    header_row = tk.Frame(frame, bg="#ffffff")
    header_row.pack(fill=tk.X, pady=(0, 8))
    tk.Label(
        header_row,
        text=ECB_COVERAGE_TAB_LABEL,
        anchor="w",
        bg="#ffffff",
        fg="#0f172a",
        font=("Segoe UI", 13, "bold"),
    ).pack(side=tk.LEFT)
    status = tk.StringVar(value="")

    coverage_text = tk.Text(
        frame,
        wrap=tk.NONE,
        bg="#f8fafc",
        fg="#0f172a",
        relief=tk.FLAT,
        font=("Consolas", 9),
        padx=10,
        pady=8,
    )
    y_scroll = tk.Scrollbar(frame, orient=tk.VERTICAL, command=coverage_text.yview)
    x_scroll = tk.Scrollbar(frame, orient=tk.HORIZONTAL, command=coverage_text.xview)
    coverage_text.configure(yscrollcommand=y_scroll.set, xscrollcommand=x_scroll.set)
    y_scroll.pack(side=tk.RIGHT, fill=tk.Y)
    x_scroll.pack(side=tk.BOTTOM, fill=tk.X)
    coverage_text.pack(side=tk.LEFT, fill=tk.BOTH, expand=True)

    def refresh() -> None:
        coverage = load_ecb_coverage(coverage_path)
        _set_text(coverage_text, format_ecb_coverage(coverage))
        status.set(f"Source: {coverage_path.name}")

    tk.Button(header_row, text="Refresh", command=refresh).pack(side=tk.RIGHT)
    tk.Label(header_row, textvariable=status, bg="#ffffff", fg="#475569").pack(
        side=tk.RIGHT,
        padx=(0, 12),
    )
    refresh()


def draw_value_comparison_tab(
    notebook: Any,
    tab_label: str,
    rows: list[dict[str, Any]],
) -> None:
    import tkinter as tk
    from tkinter import ttk

    frame = tk.Frame(notebook, bg="#ffffff", padx=8, pady=8)
    notebook.add(frame, text=tab_label)

    columns = (
        "category",
        "field",
        "source_field",
        "offset",
        "decomp_value",
        "rust_field",
        "rust_value",
        "status",
    )
    table_frame = tk.Frame(frame)
    table_frame.pack(fill=tk.BOTH, expand=True)
    table = ttk.Treeview(table_frame, columns=columns, show="headings", selectmode="browse")
    y_scroll = ttk.Scrollbar(table_frame, orient=tk.VERTICAL, command=table.yview)
    x_scroll = ttk.Scrollbar(table_frame, orient=tk.HORIZONTAL, command=table.xview)
    table.configure(yscrollcommand=y_scroll.set, xscrollcommand=x_scroll.set)
    y_scroll.pack(side=tk.RIGHT, fill=tk.Y)
    x_scroll.pack(side=tk.BOTTOM, fill=tk.X)
    table.pack(side=tk.LEFT, fill=tk.BOTH, expand=True)

    headings = {
        "category": "Category",
        "field": "Ledger Field",
        "source_field": "Decomp Field",
        "offset": "Offset",
        "decomp_value": "Decomp Value",
        "rust_field": "Rust Field",
        "rust_value": "Rust Value",
        "status": "Status",
    }
    widths = {
        "category": 160,
        "field": 230,
        "source_field": 190,
        "offset": 74,
        "decomp_value": 110,
        "rust_field": 260,
        "rust_value": 110,
        "status": 88,
    }
    for column in columns:
        table.heading(column, text=headings[column])
        table.column(column, width=widths[column], minwidth=70, stretch=column in {"field", "rust_field"})

    table.tag_configure("match", background="#ecfdf5")
    table.tag_configure("diff", background="#fff7ed")
    table.tag_configure("derived", background="#eff6ff")
    table.tag_configure("missing", background="#fef2f2")

    row_by_iid: dict[str, dict[str, Any]] = {}
    for index, row in enumerate(rows):
        iid = str(index)
        row_by_iid[iid] = row
        table.insert(
            "",
            "end",
            iid=iid,
            values=tuple(_display_value(row[column]) for column in columns),
            tags=(row["status"],),
        )

    details = tk.Text(
        frame,
        height=8,
        wrap=tk.WORD,
        bg="#f8fafc",
        fg="#0f172a",
        relief=tk.FLAT,
        font=("Segoe UI", 9),
        padx=10,
        pady=8,
    )
    details.pack(fill=tk.X, pady=(8, 0))

    def show_row_details(_event: Any | None = None) -> None:
        selected = table.selection()
        if not selected:
            return
        _set_text(details, format_value_comparison_details(row_by_iid[selected[0]]))

    table.bind("<<TreeviewSelect>>", show_row_details)
    if rows:
        table.selection_set("0")
        show_row_details()


def draw_input_trace_tab(parent: Any, log_dir: Path = DEFAULT_INPUT_TRACE_DIR) -> None:
    import tkinter as tk

    frame = tk.Frame(parent, bg="#ffffff", padx=12, pady=12)
    frame.pack(fill=tk.BOTH, expand=True)

    header_row = tk.Frame(frame, bg="#ffffff")
    header_row.pack(fill=tk.X, pady=(0, 8))
    tk.Label(
        header_row,
        text=INPUT_TRACE_TAB_LABEL,
        anchor="w",
        bg="#ffffff",
        fg="#0f172a",
        font=("Segoe UI", 13, "bold"),
    ).pack(side=tk.LEFT)
    status = tk.StringVar(value="")

    trace_text = tk.Text(
        frame,
        wrap=tk.NONE,
        bg="#f8fafc",
        fg="#0f172a",
        relief=tk.FLAT,
        font=("Consolas", 9),
        padx=10,
        pady=8,
    )
    y_scroll = tk.Scrollbar(frame, orient=tk.VERTICAL, command=trace_text.yview)
    x_scroll = tk.Scrollbar(frame, orient=tk.HORIZONTAL, command=trace_text.xview)
    trace_text.configure(yscrollcommand=y_scroll.set, xscrollcommand=x_scroll.set)
    y_scroll.pack(side=tk.RIGHT, fill=tk.Y)
    x_scroll.pack(side=tk.BOTTOM, fill=tk.X)
    trace_text.pack(side=tk.LEFT, fill=tk.BOTH, expand=True)

    def refresh() -> None:
        path = latest_input_trace_path(log_dir)
        text = format_recent_input_trace(path)
        _set_text(trace_text, text)
        status.set(f"Latest: {path.name}" if path else "Latest: none")

    tk.Button(header_row, text="Refresh", command=refresh).pack(side=tk.RIGHT)
    tk.Label(header_row, textvariable=status, bg="#ffffff", fg="#475569").pack(
        side=tk.RIGHT,
        padx=(0, 12),
    )
    refresh()


def draw_slippi_replay_tab(parent: Any, report_dir: Path = DEFAULT_SLIPPI_REPORT_DIR) -> None:
    import tkinter as tk

    frame = tk.Frame(parent, bg="#ffffff", padx=12, pady=12)
    frame.pack(fill=tk.BOTH, expand=True)

    header_row = tk.Frame(frame, bg="#ffffff")
    header_row.pack(fill=tk.X, pady=(0, 8))
    tk.Label(
        header_row,
        text=SLIPPI_REPLAY_TAB_LABEL,
        anchor="w",
        bg="#ffffff",
        fg="#0f172a",
        font=("Segoe UI", 13, "bold"),
    ).pack(side=tk.LEFT)
    status = tk.StringVar(value="")

    report_text = tk.Text(
        frame,
        wrap=tk.NONE,
        bg="#f8fafc",
        fg="#0f172a",
        relief=tk.FLAT,
        font=("Consolas", 9),
        padx=10,
        pady=8,
    )
    y_scroll = tk.Scrollbar(frame, orient=tk.VERTICAL, command=report_text.yview)
    x_scroll = tk.Scrollbar(frame, orient=tk.HORIZONTAL, command=report_text.xview)
    report_text.configure(yscrollcommand=y_scroll.set, xscrollcommand=x_scroll.set)
    y_scroll.pack(side=tk.RIGHT, fill=tk.Y)
    x_scroll.pack(side=tk.BOTTOM, fill=tk.X)
    report_text.pack(side=tk.LEFT, fill=tk.BOTH, expand=True)

    def refresh() -> None:
        path = latest_slippi_report_path(report_dir)
        text = format_latest_slippi_report(path=path, report_dir=report_dir)
        _set_text(report_text, text)
        status.set(f"Latest: {path.name}" if path else "Latest: none")

    tk.Button(header_row, text="Refresh", command=refresh).pack(side=tk.RIGHT)
    tk.Label(header_row, textvariable=status, bg="#ffffff", fg="#475569").pack(
        side=tk.RIGHT,
        padx=(0, 12),
    )
    refresh()


def draw_move_keyframes_tab(
    parent: Any,
    frame_data_dir: Path = DEFAULT_MOVE_FRAME_DATA_DIR,
) -> None:
    import tkinter as tk
    from tkinter import ttk

    frame = tk.Frame(parent, bg="#ffffff", padx=12, pady=12)
    frame.pack(fill=tk.BOTH, expand=True)

    header_row = tk.Frame(frame, bg="#ffffff")
    header_row.pack(fill=tk.X, pady=(0, 8))
    tk.Label(
        header_row,
        text=MOVE_KEYFRAMES_TAB_LABEL,
        anchor="w",
        bg="#ffffff",
        fg="#0f172a",
        font=("Segoe UI", 13, "bold"),
    ).pack(side=tk.LEFT)

    characters = list_move_frame_data_characters(frame_data_dir)
    character_by_label = {character["label"]: character for character in characters}
    selected_character = tk.StringVar(value=characters[0]["label"])
    selected_state = tk.StringVar(value="")
    selected_frame_number = tk.IntVar(value=1)

    controls = tk.Frame(frame, bg="#ffffff")
    controls.pack(fill=tk.X, pady=(0, 8))
    tk.Label(controls, text="Character", bg="#ffffff").pack(side=tk.LEFT)
    character_combo = ttk.Combobox(
        controls,
        textvariable=selected_character,
        values=[character["label"] for character in characters],
        state="readonly",
        width=24,
    )
    character_combo.pack(side=tk.LEFT, padx=(6, 16))
    tk.Label(controls, text="State", bg="#ffffff").pack(side=tk.LEFT)
    state_combo = ttk.Combobox(controls, textvariable=selected_state, state="readonly", width=24)
    state_combo.pack(side=tk.LEFT, padx=(6, 16))

    body = tk.PanedWindow(frame, orient=tk.HORIZONTAL, sashrelief=tk.RAISED)
    body.pack(fill=tk.BOTH, expand=True)
    canvas = tk.Canvas(body, bg="#111827", width=720, height=460, highlightthickness=0)
    details = tk.Text(
        body,
        wrap=tk.WORD,
        bg="#f8fafc",
        fg="#0f172a",
        relief=tk.FLAT,
        font=("Consolas", 9),
        padx=10,
        pady=8,
    )
    body.add(canvas, stretch="always")
    body.add(details, stretch="always")

    state_records: list[dict[str, Any]] = []
    loaded_data: dict[str, Any] | None = None

    def refresh_states() -> None:
        nonlocal state_records
        character = character_by_label[selected_character.get()]
        state_records = list_move_frame_data_states(frame_data_dir, character["id"])
        state_combo.configure(values=[state["label"] for state in state_records])
        selected_state.set(state_records[0]["label"] if state_records else "")
        redraw()

    def current_state_record() -> dict[str, Any] | None:
        for record in state_records:
            if record["label"] == selected_state.get():
                return record
        return None

    def redraw() -> None:
        nonlocal loaded_data
        canvas.delete("all")
        character = character_by_label[selected_character.get()]
        record = current_state_record()
        if record is None:
            loaded_data = None
            selected_frame_number.set(1)
            _set_text(details, build_move_keyframe_empty_state(character["label"]))
            canvas.create_text(
                360,
                210,
                text=build_move_keyframe_empty_state(character["label"]),
                fill="#cbd5e1",
                font=("Segoe UI", 12),
                justify=tk.CENTER,
            )
            return
        loaded_data = load_move_frame_data(frame_data_dir, character["id"], record["state"])
        total_frames = int(loaded_data.get("summary", {}).get("total_frames", 1))
        if selected_frame_number.get() < 1 or selected_frame_number.get() > total_frames:
            selected_frame_number.set(1)
        draw_selected_frame()

    def draw_selected_frame() -> None:
        if loaded_data is None:
            return
        frame_number = selected_frame_number.get()
        effective_frame = nearest_move_keyframe(loaded_data, frame_number)
        if effective_frame is None:
            sampled_frame = None
            if loaded_data.get("artifact_kind") == "source_manifest_action_view":
                sampled_frame = sample_manifest_action_frame(
                    frame_data_dir,
                    character_combo.get(),
                    state_combo.get(),
                    frame_number,
                )
            if sampled_frame is not None:
                sampled_data = copy.deepcopy(loaded_data)
                sampled_data["keyframes"] = [sampled_frame]
                sampled_data["summary"]["active_hitbox_windows"] = (
                    [{"start": frame_number, "end": frame_number, "source": "frame-data sample"}]
                    if sampled_frame.get("hitboxes")
                    else []
                )
                sampled_data["summary"]["active_hurtbox_windows"] = (
                    [{"start": frame_number, "end": frame_number, "source": "frame-data sample"}]
                    if sampled_frame.get("hurtboxes")
                    else []
                )
                text = "\n\n".join(
                    [
                        format_move_frame_data_summary(sampled_data),
                        f"Selected frame: {frame_number}",
                        "Live sampled frame from compact source manifest",
                        format_move_keyframe_details(sampled_data, sampled_frame),
                    ]
                )
                _set_text(details, text)
                draw_move_keyframe_canvas(
                    canvas,
                    sampled_data,
                    selected_frame_number=frame_number,
                    on_frame_selected=select_frame,
                )
                return
            _set_text(details, format_move_frame_data_summary(loaded_data))
            draw_move_keyframe_canvas(canvas, loaded_data, selected_frame_number=frame_number)
            return
        text = "\n\n".join(
            [
                format_move_frame_data_summary(loaded_data),
                f"Selected frame: {frame_number}",
                f"Hitbox active: {frame_has_active_hitbox(loaded_data, frame_number)}",
                format_move_keyframe_details(loaded_data, effective_frame),
            ]
        )
        _set_text(details, text)
        draw_move_keyframe_canvas(
            canvas,
            loaded_data,
            selected_frame_number=frame_number,
            on_frame_selected=select_frame,
        )

    def select_frame(frame_number: int) -> None:
        selected_frame_number.set(frame_number)
        draw_selected_frame()

    def on_state_changed(_event: Any | None = None) -> None:
        redraw()

    character_combo.bind("<<ComboboxSelected>>", lambda _event: refresh_states())
    state_combo.bind("<<ComboboxSelected>>", on_state_changed)
    refresh_states()


def draw_move_keyframe_canvas(
    canvas: Any,
    data: dict[str, Any],
    selected_frame_number: int = 1,
    on_frame_selected: Any | None = None,
) -> None:
    import tkinter as tk

    canvas.delete("all")
    keyframes = data.get("keyframes", [])
    if not keyframes:
        canvas.create_text(360, 220, text="No keyframes", fill="#cbd5e1")
        return
    frame = nearest_move_keyframe(data, selected_frame_number) or keyframes[0]
    projection = data.get("projection", {})
    scale = float(projection.get("view_scale", 10.0))
    origin_x, origin_y = 360.0, 260.0

    canvas.create_text(
        16,
        16,
        anchor=tk.NW,
        text=f"{data.get('target_character_label', 'Unknown')} - {data.get('label', 'Unknown')} - Frame {selected_frame_number}",
        fill="#e2e8f0",
        font=("Segoe UI", 12, "bold"),
    )
    active_label = "hitbox active" if frame_has_active_hitbox(data, selected_frame_number) else "no active hitbox"
    canvas.create_text(
        16,
        36,
        anchor=tk.NW,
        text=active_label,
        fill="#fca5a5" if frame_has_active_hitbox(data, selected_frame_number) else "#94a3b8",
        font=("Segoe UI", 10),
    )

    _draw_move_hitbox_travel(canvas, frame, projection, scale, origin_x, origin_y)
    _draw_move_body_volumes(canvas, frame, projection, scale, origin_x, origin_y, ghost=False)
    _draw_move_frame_volumes(canvas, frame, projection, scale, origin_x, origin_y, ghost=False)

    total_frames = int(data.get("summary", {}).get("total_frames", max(1, len(keyframes))))
    cell_width = max(8, min(18, 680 // max(1, total_frames)))
    timeline_x = 20
    timeline_y = 420
    keyed_frames = {int(item.get("frame", 0)) for item in keyframes}
    for frame_number in range(1, total_frames + 1):
        x0 = timeline_x + (frame_number - 1) * cell_width
        x1 = x0 + cell_width - 2
        fill = "#0f172a"
        outline = "#334155"
        if frame_has_active_hitbox(data, frame_number):
            fill = "#7f1d1d"
            outline = "#ef4444"
        if frame_number in keyed_frames:
            fill = "#1d4ed8"
            outline = "#60a5fa"
        if frame_has_active_hitbox(data, frame_number) and frame_number in keyed_frames:
            fill = "#7c2d12"
            outline = "#fb923c"
        if frame_number == selected_frame_number:
            fill = "#047857"
            outline = "#34d399"
        tag = f"move_frame_{frame_number}"
        canvas.create_rectangle(
            x0,
            timeline_y,
            x1,
            timeline_y + 18,
            fill=fill,
            outline=outline,
            tags=(tag, "move_timeline_frame"),
        )
        if on_frame_selected is not None:
            canvas.tag_bind(tag, "<Button-1>", lambda _event, n=frame_number: on_frame_selected(n))


def _draw_move_frame_volumes(
    canvas: Any,
    frame: dict[str, Any],
    projection: dict[str, Any],
    scale: float,
    origin_x: float,
    origin_y: float,
    ghost: bool,
) -> None:
    hit_outline = "#6ee7b7" if ghost else "#10b981"
    hurt_outline = "#93c5fd" if ghost else "#3b82f6"
    dash = (4, 3) if ghost else None
    for hurtbox in frame.get("hurtboxes", []):
        a = project_move_point(hurtbox.get("a_pos", hurtbox.get("a", {})), projection)
        b = project_move_point(hurtbox.get("b_pos", hurtbox.get("b", {})), projection)
        radius = float(hurtbox.get("radius", 1.0)) * scale
        ax = origin_x + a["view_x"] * scale
        ay = origin_y - a["view_y"] * scale
        bx = origin_x + b["view_x"] * scale
        by = origin_y - b["view_y"] * scale
        canvas.create_line(
            ax,
            ay,
            bx,
            by,
            fill=hurt_outline,
            width=2,
            dash=dash,
            tags=("move_hurtbox",),
        )
        for cx, cy in [(ax, ay), (bx, by)]:
            canvas.create_oval(
                cx - radius,
                cy - radius,
                cx + radius,
                cy + radius,
                outline=hurt_outline,
                width=2,
                dash=dash,
                tags=("move_hurtbox",),
            )
    for hitbox in frame.get("hitboxes", []):
        center = project_move_point(hitbox.get("center", {}), projection)
        radius = float(hitbox.get("radius", 1.0)) * scale
        cx = origin_x + center["view_x"] * scale
        cy = origin_y - center["view_y"] * scale
        canvas.create_oval(
            cx - radius,
            cy - radius,
            cx + radius,
            cy + radius,
            outline=hit_outline,
            width=2,
            dash=dash,
            tags=("move_hitbox",),
        )


def _draw_move_body_volumes(
    canvas: Any,
    frame: dict[str, Any],
    projection: dict[str, Any],
    scale: float,
    origin_x: float,
    origin_y: float,
    ghost: bool,
) -> None:
    outline = "#fcd34d" if ghost else "#f59e0b"
    dash = (5, 4) if ghost else None
    for body_volume in frame.get("body_volumes", []):
        points = [
            body_volume.get("top", {}),
            body_volume.get("right", {}),
            body_volume.get("bottom", {}),
            body_volume.get("left", {}),
            body_volume.get("top", {}),
        ]
        projected = [
            _move_canvas_point(point, projection, scale, origin_x, origin_y) for point in points
        ]
        for (x0, y0), (x1, y1) in zip(projected, projected[1:]):
            canvas.create_line(
                x0,
                y0,
                x1,
                y1,
                fill=outline,
                width=2,
                dash=dash,
                tags=("move_body_volume",),
            )


def _draw_move_hitbox_travel(
    canvas: Any,
    current: dict[str, Any],
    projection: dict[str, Any],
    scale: float,
    origin_x: float,
    origin_y: float,
) -> None:
    for hitbox in current.get("hitboxes", []):
        previous_center_raw = hitbox.get("previous_center")
        if not isinstance(previous_center_raw, dict):
            continue
        prior_center = project_move_point(previous_center_raw, projection)
        current_center = project_move_point(hitbox.get("center", {}), projection)
        x0 = origin_x + prior_center["view_x"] * scale
        y0 = origin_y - prior_center["view_y"] * scale
        x1 = origin_x + current_center["view_x"] * scale
        y1 = origin_y - current_center["view_y"] * scale
        if x0 == x1 and y0 == y1:
            continue
        canvas.create_line(
            x0,
            y0,
            x1,
            y1,
            fill="#fbbf24",
            width=1,
            dash=(2, 3),
            tags=("move_hitbox_travel",),
        )


def _move_canvas_point(
    point: dict[str, Any],
    projection: dict[str, Any],
    scale: float,
    origin_x: float,
    origin_y: float,
) -> tuple[float, float]:
    projected = project_move_point(point, projection)
    return (
        origin_x + projected["view_x"] * scale,
        origin_y - projected["view_y"] * scale,
    )


def format_value_comparison_details(row: dict[str, Any]) -> str:
    lines = [
        f"{row['field']} [{row['status']}]",
        "",
        f"Category: {row['category']}",
        f"Decomp: {row['source_field']} @ {row['offset']} -> {_display_value(row['decomp_value'])}",
        f"Rust: {row['rust_field']} -> {_display_value(row['rust_value'])}",
        f"Kind: {row['kind']}",
        f"Raw decomp value: {_display_value(row['raw'])}",
        f"Provenance: {row['provenance']}",
    ]
    if row.get("note"):
        lines.extend(["", row["note"]])
    return "\n".join(lines)


def _set_text(text: Any, value: str) -> None:
    text.configure(state="normal")
    text.delete("1.0", "end")
    text.insert("1.0", value)
    text.configure(state="disabled")


def _read_json_file(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        data = json.load(handle)
    if not isinstance(data, dict):
        raise ValueError(f"{path} did not contain a JSON object")
    return data


def _display_value(value: Any) -> str:
    return "" if value is None else str(value)


def save_current_layout(graphs: list[dict[str, Any]], layout_path: Path, status_text: Any) -> None:
    save_layout(graphs, layout_path)
    status_text.set(f"Saved layout to {layout_path}.")


class StateGraphPane:
    NODE_WIDTH = 190
    NODE_HEIGHT = 62
    X_GAP = 240
    Y_GAP = 130
    X_ORIGIN = 40
    Y_ORIGIN = 112

    def __init__(
        self,
        parent: Any,
        graph: dict[str, Any],
        show_labels: Any,
        curved_edges: Any,
        on_node_moved: Any,
        on_edge_label_pinned: Any,
        side: str,
    ) -> None:
        import tkinter as tk

        self.graph = graph
        self.show_labels = show_labels
        self.curved_edges = curved_edges
        self.on_node_moved = on_node_moved
        self.on_edge_label_pinned = on_edge_label_pinned
        self.side = side
        self.zoom = clamp_zoom(graph.get("zoom", 1.0))
        self.graph["zoom"] = self.zoom
        self.drag_node_id: str | None = None
        self.drag_offset = (0, 0)
        self.node_boxes: dict[str, tuple[float, float, float, float]] = {}
        self.edge_lane_offsets: list[float] = []
        self.scroll_region: tuple[float, float, float, float] | None = None
        self.x_min = min(0, *(node["pos"][0] for node in graph["nodes"]))
        self.y_min = min(0, *(node["pos"][1] for node in graph["nodes"]))

        self.frame = tk.Frame(parent, bg="#ffffff")
        title = tk.Label(
            self.frame,
            text=graph["title"],
            anchor="w",
            bg="#ffffff",
            fg="#0f172a",
            font=("Segoe UI", 13, "bold"),
            padx=10,
            pady=4,
        )
        title.pack(fill=tk.X)
        description = tk.Label(
            self.frame,
            text=graph.get("description", ""),
            anchor="w",
            justify=tk.LEFT,
            wraplength=680,
            bg="#ffffff",
            fg="#475569",
            font=("Segoe UI", 9),
            padx=10,
        )
        description.pack(fill=tk.X)

        canvas_frame = tk.Frame(self.frame)
        canvas_frame.pack(fill=tk.BOTH, expand=True)
        self.canvas = tk.Canvas(canvas_frame, bg="#ffffff", highlightthickness=0)
        x_scroll = tk.Scrollbar(canvas_frame, orient=tk.HORIZONTAL, command=self.canvas.xview)
        y_scroll = tk.Scrollbar(canvas_frame, orient=tk.VERTICAL, command=self.canvas.yview)
        self.canvas.configure(xscrollcommand=x_scroll.set, yscrollcommand=y_scroll.set)
        y_scroll.pack(side=tk.RIGHT, fill=tk.Y)
        x_scroll.pack(side=tk.BOTTOM, fill=tk.X)
        self.canvas.pack(side=tk.LEFT, fill=tk.BOTH, expand=True)

        self.details = tk.Text(
            self.frame,
            height=5,
            wrap=tk.WORD,
            bg="#f8fafc",
            fg="#0f172a",
            relief=tk.FLAT,
            font=("Segoe UI", 9),
        )
        self.details.pack(fill=tk.X)
        self.details.insert(
            "1.0",
            "Mouse wheel zooms around the cursor. Right-click drag pans the graph. Click a node or edge badge for details. Left-click drag nodes to customize the layout; click Save Layout to keep it.",
        )
        self.details.configure(state=tk.DISABLED)

        self.canvas.bind("<ButtonPress-1>", self._on_press)
        self.canvas.bind("<Double-Button-1>", self._on_double_press)
        self.canvas.bind("<B1-Motion>", self._on_drag)
        self.canvas.bind("<ButtonRelease-1>", self._on_release)
        self.canvas.bind("<ButtonPress-3>", self._on_pan_press)
        self.canvas.bind("<B3-Motion>", self._on_pan_drag)
        self.canvas.bind("<ButtonRelease-3>", self._on_pan_release)
        self.canvas.bind("<MouseWheel>", self._on_mousewheel_zoom)
        self.canvas.bind("<Button-4>", self._on_mousewheel_zoom)
        self.canvas.bind("<Button-5>", self._on_mousewheel_zoom)

    def redraw(self) -> None:
        view_left = self.canvas.canvasx(0)
        view_top = self.canvas.canvasy(0)
        self.canvas.delete("all")
        self.node_boxes = {node["id"]: self._node_box(node) for node in self.graph["nodes"]}
        self.edge_lane_offsets = compute_edge_lane_offsets(
            self.graph["edges"],
            spacing=20 * self.zoom,
        )
        for index, edge in enumerate(self.graph["edges"], start=1):
            self._draw_edge(index, edge)
        for node in self.graph["nodes"]:
            self._draw_node(node)
        self._update_scroll_region()
        self._scroll_canvas_top_left_to(view_left, view_top)

    def _node_box(self, node: dict[str, Any]) -> tuple[float, float, float, float]:
        x, y = node["pos"]
        left = self.X_ORIGIN + (x - self.x_min) * self._x_gap()
        top = self.Y_ORIGIN + (y - self.y_min) * self._y_gap()
        return (left, top, left + self._node_width(), top + self._node_height())

    def _draw_node(self, node: dict[str, Any]) -> None:
        left, top, right, bottom = self.node_boxes[node["id"]]
        style = STATUS_STYLES[node["status"]]
        tags = ("node", f"node:{node['id']}")
        self.canvas.create_rectangle(
            left,
            top,
            right,
            bottom,
            fill=style.fill,
            outline=style.outline,
            width=max(1, round(2 * self.zoom)),
            tags=tags,
        )
        self.canvas.create_text(
            left + 10 * self.zoom,
            top + 9 * self.zoom,
            text=node["label"],
            anchor="nw",
            width=(right - left) - 20 * self.zoom,
            fill=style.text,
            font=("Segoe UI", self._font_size(9), "bold"),
            tags=tags,
        )
        self.canvas.create_text(
            left + 10 * self.zoom,
            bottom - 20 * self.zoom,
            text=style.label,
            anchor="nw",
            fill=style.outline,
            font=("Segoe UI", self._font_size(8)),
            tags=tags,
        )

    def _draw_edge(self, index: int, edge: dict[str, Any]) -> None:
        source = _edge_anchor(self.node_boxes[edge["from"]], self.node_boxes[edge["to"]])
        target = _target_anchor(self.node_boxes[edge["from"]], self.node_boxes[edge["to"]])
        style = STATUS_STYLES[edge["status"]]
        tags = ("edge", f"edge:{index - 1}")
        lane_offset = self.edge_lane_offsets[index - 1] if self.edge_lane_offsets else 0
        use_curves = bool(self.curved_edges.get())
        points = route_edge(source, target, lane_offset=lane_offset, curved=use_curves)
        self.canvas.create_line(
            *points,
            fill=style.outline,
            width=max(1, round(2 * self.zoom)),
            arrow="last",
            smooth=use_curves,
            splinesteps=24,
            tags=tags,
        )
        badge_x, badge_y = edge_badge_position(points)
        if is_edge_label_visible(self.graph, index - 1, show_all=bool(self.show_labels.get())):
            self._draw_edge_label(edge, badge_x, badge_y, tags)
        else:
            badge_radius = max(7, 11 * self.zoom)
            self.canvas.create_oval(
                badge_x - badge_radius,
                badge_y - badge_radius,
                badge_x + badge_radius,
                badge_y + badge_radius,
                fill=style.fill,
                outline=style.outline,
                width=max(1, round(2 * self.zoom)),
                tags=tags,
            )
            self.canvas.create_text(
                badge_x,
                badge_y,
                text=str(index),
                fill=style.text,
                font=("Segoe UI", self._font_size(8), "bold"),
                tags=tags,
            )

    def _draw_edge_label(
        self,
        edge: dict[str, Any],
        x: float,
        y: float,
        tags: tuple[str, str],
    ) -> None:
        text_id = self.canvas.create_text(
            x,
            y,
            text=f"{edge['input']}\n{edge['frames']}",
            anchor="center",
            width=170 * self.zoom,
            fill="#111827",
            font=("Segoe UI", self._font_size(7)),
            tags=tags,
        )
        bbox = self.canvas.bbox(text_id)
        if bbox is None:
            return
        pad = 3
        rect = self.canvas.create_rectangle(
            bbox[0] - pad,
            bbox[1] - pad,
            bbox[2] + pad,
            bbox[3] + pad,
            fill="#ffffff",
            outline="#e2e8f0",
            tags=tags,
        )
        self.canvas.tag_lower(rect, text_id)

    def _on_press(self, event: Any) -> None:
        tags = self.canvas.gettags("current")
        node_id = _tag_value(tags, "node:")
        edge_index = _tag_value(tags, "edge:")
        canvas_x = self.canvas.canvasx(event.x)
        canvas_y = self.canvas.canvasy(event.y)
        if node_id:
            box = self.node_boxes[node_id]
            self.drag_node_id = node_id
            self.drag_offset = (canvas_x - box[0], canvas_y - box[1])
            self._show_node_details(node_id)
        elif edge_index is not None:
            self.drag_node_id = None
            self._show_edge_details(int(edge_index))
        else:
            self.drag_node_id = None

    def _on_drag(self, event: Any) -> None:
        if not self.drag_node_id:
            return
        node = self._node_by_id(self.drag_node_id)
        old_x, old_y = node["pos"]
        canvas_x = self.canvas.canvasx(event.x)
        canvas_y = self.canvas.canvasy(event.y)
        left = max(0, canvas_x - self.drag_offset[0])
        top = max(70, canvas_y - self.drag_offset[1])
        new_pos = [
            round(self.x_min + (left - self.X_ORIGIN) / self._x_gap(), 3),
            round(self.y_min + (top - self.Y_ORIGIN) / self._y_gap(), 3),
        ]
        node["pos"] = new_pos
        delta_x = round(new_pos[0] - old_x, 3)
        delta_y = round(new_pos[1] - old_y, 3)
        if delta_x or delta_y:
            self.on_node_moved(self, self.drag_node_id, delta_x, delta_y)
        self.redraw()

    def _on_double_press(self, event: Any) -> str:
        tags = self.canvas.gettags("current")
        edge_index = _tag_value(tags, "edge:")
        if edge_index is None:
            return "break"
        self.drag_node_id = None
        self.on_edge_label_pinned(self, int(edge_index))
        self._show_edge_details(int(edge_index))
        return "break"

    def _on_release(self, _event: Any) -> None:
        self.drag_node_id = None

    def _on_mousewheel_zoom(self, event: Any) -> str:
        delta = _event_wheel_delta(event)
        if delta == 0:
            return "break"
        before_canvas_x = self.canvas.canvasx(event.x)
        before_canvas_y = self.canvas.canvasy(event.y)
        graph_x, graph_y = self._canvas_to_graph_point(before_canvas_x, before_canvas_y)
        next_zoom = zoom_from_wheel_delta(self.zoom, delta)
        if next_zoom == self.zoom:
            return "break"

        self.zoom = next_zoom
        self.graph["zoom"] = next_zoom
        self.redraw()

        after_canvas_x, after_canvas_y = self._graph_to_canvas_point(graph_x, graph_y)
        self._scroll_canvas_to_keep_cursor_point(after_canvas_x, after_canvas_y, event.x, event.y)
        self._set_details(
            f"{self.graph['title']}\n\n"
            f"Zoom: {self.zoom:.2f}x. Mouse wheel zooms around the cursor; click Save Layout to keep it."
        )
        self.on_node_moved(self, "", 0, 0)
        return "break"

    def _on_pan_press(self, event: Any) -> str:
        if not is_right_drag_event(event):
            return "break"
        self.drag_node_id = None
        self.canvas.scan_mark(event.x, event.y)
        self.canvas.configure(cursor="fleur")
        return "break"

    def _on_pan_drag(self, event: Any) -> str:
        self.canvas.scan_dragto(event.x, event.y, gain=1)
        return "break"

    def _on_pan_release(self, _event: Any) -> str:
        self.canvas.configure(cursor="")
        return "break"

    def _show_node_details(self, node_id: str) -> None:
        node = self._node_by_id(node_id)
        style = STATUS_STYLES[node["status"]]
        ledger = format_ledger_details(node)
        body = f"{node['label']} [{style.label}]\n\n{node.get('notes', 'No notes yet.')}"
        if ledger:
            body = f"{body}\n\n{ledger}"
        self._set_details(body)

    def _show_edge_details(self, edge_index: int) -> None:
        edge = self.graph["edges"][edge_index]
        style = STATUS_STYLES[edge["status"]]
        notes = edge.get("notes", "No notes yet.")
        ledger = format_ledger_details(edge)
        body = (
            f"{edge['from']} -> {edge['to']} [{style.label}]\n\n"
            f"Input: {edge['input']}\n"
            f"Frames: {edge['frames']}\n\n"
            f"{notes}"
        )
        if ledger:
            body = f"{body}\n\n{ledger}"
        self._set_details(body)

    def _set_details(self, text: str) -> None:
        self.details.configure(state="normal")
        self.details.delete("1.0", "end")
        self.details.insert("1.0", text)
        self.details.configure(state="disabled")

    def _node_by_id(self, node_id: str) -> dict[str, Any]:
        for node in self.graph["nodes"]:
            if node["id"] == node_id:
                return node
        raise KeyError(node_id)

    def _node_width(self) -> float:
        return self.NODE_WIDTH * self.zoom

    def _node_height(self) -> float:
        return self.NODE_HEIGHT * self.zoom

    def _x_gap(self) -> float:
        return self.X_GAP * self.zoom

    def _y_gap(self) -> float:
        return self.Y_GAP * self.zoom

    def _font_size(self, base_size: int) -> int:
        return max(5, min(16, round(base_size * self.zoom)))

    def _canvas_to_graph_point(self, canvas_x: float, canvas_y: float) -> tuple[float, float]:
        return (
            self.x_min + (canvas_x - self.X_ORIGIN) / self._x_gap(),
            self.y_min + (canvas_y - self.Y_ORIGIN) / self._y_gap(),
        )

    def _graph_to_canvas_point(self, graph_x: float, graph_y: float) -> tuple[float, float]:
        return (
            self.X_ORIGIN + (graph_x - self.x_min) * self._x_gap(),
            self.Y_ORIGIN + (graph_y - self.y_min) * self._y_gap(),
        )

    def _scroll_canvas_to_keep_cursor_point(
        self,
        canvas_x: float,
        canvas_y: float,
        cursor_x: float,
        cursor_y: float,
    ) -> None:
        if self.scroll_region is None:
            return
        left, top, right, bottom = self.scroll_region
        width = max(1, right - left)
        height = max(1, bottom - top)
        target_x = (canvas_x - cursor_x - left) / width
        target_y = (canvas_y - cursor_y - top) / height
        self.canvas.xview_moveto(max(0.0, min(1.0, target_x)))
        self.canvas.yview_moveto(max(0.0, min(1.0, target_y)))

    def _update_scroll_region(self) -> None:
        bbox = self.canvas.bbox("all")
        if bbox is None:
            return
        self.scroll_region = expand_scroll_region(
            self.scroll_region,
            bbox,
            margin=SCROLL_REGION_MARGIN * self.zoom,
        )
        self.canvas.configure(scrollregion=self.scroll_region)

    def _scroll_canvas_top_left_to(self, canvas_x: float, canvas_y: float) -> None:
        if self.scroll_region is None:
            return
        self.canvas.xview_moveto(scroll_fraction_for_canvas_coordinate(self.scroll_region, canvas_x, "x"))
        self.canvas.yview_moveto(scroll_fraction_for_canvas_coordinate(self.scroll_region, canvas_y, "y"))


def edge_lane_offsets(count: int, spacing: float = 22) -> list[float]:
    if count <= 0:
        return []
    center = (count - 1) / 2
    return [round((index - center) * spacing, 3) for index in range(count)]


def compute_edge_lane_offsets(edges: list[dict[str, Any]], spacing: float = 22) -> list[float]:
    offsets = [0.0 for _edge in edges]
    _add_lane_group_offsets(offsets, edges, "from", spacing, weight=1.0)
    _add_lane_group_offsets(offsets, edges, "to", spacing, weight=0.55)
    return [round(offset, 3) for offset in offsets]


def route_edge(
    source: tuple[float, float],
    target: tuple[float, float],
    *,
    lane_offset: float = 0,
    curved: bool = False,
) -> list[float]:
    if not curved:
        if abs(source[0] - target[0]) < 8:
            return [source[0], source[1], target[0], target[1]]
        mid_y = (source[1] + target[1]) / 2
        return [source[0], source[1], source[0], mid_y, target[0], mid_y, target[0], target[1]]

    normal_x, normal_y = _normal(source, target)
    dx = target[0] - source[0]
    dy = target[1] - source[1]
    control_one = (
        source[0] + dx * 0.35 + normal_x * lane_offset,
        source[1] + dy * 0.35 + normal_y * lane_offset,
    )
    control_two = (
        source[0] + dx * 0.65 + normal_x * lane_offset,
        source[1] + dy * 0.65 + normal_y * lane_offset,
    )
    return [
        source[0],
        source[1],
        control_one[0],
        control_one[1],
        control_two[0],
        control_two[1],
        target[0],
        target[1],
    ]


def edge_badge_position(points: list[float]) -> tuple[float, float]:
    if len(points) >= 8:
        return (points[2] + points[4]) / 2, (points[3] + points[5]) / 2
    return (points[0] + points[-2]) / 2, (points[1] + points[-1]) / 2


def _add_lane_group_offsets(
    offsets: list[float],
    edges: list[dict[str, Any]],
    key: str,
    spacing: float,
    *,
    weight: float,
) -> None:
    groups: dict[str, list[int]] = {}
    for index, edge in enumerate(edges):
        groups.setdefault(str(edge[key]), []).append(index)
    for indices in groups.values():
        if len(indices) == 1:
            continue
        for index, lane_offset in zip(indices, edge_lane_offsets(len(indices), spacing)):
            offsets[index] += lane_offset * weight


def _normal(
    source: tuple[float, float],
    target: tuple[float, float],
) -> tuple[float, float]:
    dx = target[0] - source[0]
    dy = target[1] - source[1]
    length = (dx * dx + dy * dy) ** 0.5
    if length == 0:
        return 0, 1
    return -dy / length, dx / length


def expand_scroll_region(
    current: tuple[float, float, float, float] | None,
    content_bbox: tuple[float, float, float, float],
    *,
    margin: float,
) -> tuple[float, float, float, float]:
    expanded = (
        content_bbox[0] - margin,
        content_bbox[1] - margin,
        content_bbox[2] + margin,
        content_bbox[3] + margin,
    )
    if current is None:
        return expanded
    return (
        min(current[0], expanded[0]),
        min(current[1], expanded[1]),
        max(current[2], expanded[2]),
        max(current[3], expanded[3]),
    )


def scroll_fraction_for_canvas_coordinate(
    scroll_region: tuple[float, float, float, float],
    canvas_coordinate: float,
    axis: str,
) -> float:
    if axis == "x":
        start, end = scroll_region[0], scroll_region[2]
    elif axis == "y":
        start, end = scroll_region[1], scroll_region[3]
    else:
        raise ValueError(f"unknown axis {axis!r}")
    span = max(1.0, end - start)
    return max(0.0, min(1.0, (canvas_coordinate - start) / span))


def _legacy_route_edge(source: tuple[float, float], target: tuple[float, float]) -> list[float]:
    if abs(source[0] - target[0]) < 8:
        return [source[0], source[1], target[0], target[1]]
    mid_y = (source[1] + target[1]) / 2
    return [source[0], source[1], source[0], mid_y, target[0], mid_y, target[0], target[1]]


def draw_legend(parent: Any) -> None:
    import tkinter as tk

    for status in ("aligned", "partial", "mismatch", "missing", "intentional", "reference"):
        style = STATUS_STYLES[status]
        swatch = tk.Label(parent, width=2, bg=style.fill, relief=tk.SOLID, borderwidth=1)
        swatch.pack(side=tk.LEFT, padx=(0, 4))
        label = tk.Label(parent, text=style.label, fg="#0f172a")
        label.pack(side=tk.LEFT, padx=(0, 18))


def _edge_anchor(
    source_box: tuple[float, float, float, float],
    target_box: tuple[float, float, float, float],
) -> tuple[float, float]:
    s_left, s_top, s_right, s_bottom = source_box
    _t_left, t_top, _t_right, t_bottom = target_box
    s_center_x = (s_left + s_right) / 2
    s_center_y = (s_top + s_bottom) / 2
    t_center_y = (t_top + t_bottom) / 2
    if t_center_y >= s_center_y:
        return s_center_x, s_bottom
    return s_center_x, s_top


def _target_anchor(
    source_box: tuple[float, float, float, float],
    target_box: tuple[float, float, float, float],
) -> tuple[float, float]:
    _s_left, s_top, _s_right, s_bottom = source_box
    t_left, t_top, t_right, t_bottom = target_box
    s_center_y = (s_top + s_bottom) / 2
    t_center_x = (t_left + t_right) / 2
    t_center_y = (t_top + t_bottom) / 2
    if t_center_y >= s_center_y:
        return t_center_x, t_top
    return t_center_x, t_bottom


def _tag_value(tags: tuple[str, ...], prefix: str) -> str | None:
    for tag in tags:
        if tag.startswith(prefix):
            return tag.removeprefix(prefix)
    return None


def _event_wheel_delta(event: Any) -> int:
    if getattr(event, "num", None) == 4:
        return 120
    if getattr(event, "num", None) == 5:
        return -120
    return int(getattr(event, "delta", 0))


def is_right_drag_event(event: Any) -> bool:
    return getattr(event, "num", None) == 3


def _is_position(value: Any) -> bool:
    return (
        isinstance(value, list)
        and len(value) == 2
        and all(isinstance(item, (int, float)) for item in value)
    )


def validate_ledger_fields(item: dict[str, Any], *, item_label: str) -> list[str]:
    errors: list[str] = []
    for field in LEDGER_LIST_FIELDS:
        if field not in item:
            continue
        value = item[field]
        if not isinstance(value, list):
            errors.append(f"{item_label} {field} must be a list")
            continue
        for index, entry in enumerate(value):
            if field in REFERENCE_FIELDS:
                if not isinstance(entry, dict):
                    errors.append(f"{item_label} {field}[{index}] must be an object")
                    continue
                if not entry.get("label"):
                    errors.append(f"{item_label} {field}[{index}] missing label")
                if not (entry.get("path") or entry.get("url") or entry.get("function")):
                    errors.append(f"{item_label} {field}[{index}] missing path, url, or function")
            elif not isinstance(entry, str):
                errors.append(f"{item_label} {field}[{index}] must be a string")
    return errors


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Open the Mole/Melee state graph viewer.")
    parser.add_argument("--graph-dir", type=Path, default=DEFAULT_GRAPH_DIR)
    parser.add_argument("--layout", type=Path, default=DEFAULT_LAYOUT_PATH)
    parser.add_argument("--value-sheets", type=Path, default=DEFAULT_VALUE_SHEET_DIR)
    parser.add_argument("--check", action="store_true", help="Validate graph data without opening a window.")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    graphs = load_graphs(args.graph_dir)
    if args.check:
        apply_saved_layout(graphs, args.layout)
        for graph in graphs:
            summary = summarize_comparison(graph)
            print(f"{graph['id']}: {len(graph['nodes'])} nodes, {len(graph['edges'])} edges")
            if any(summary.values()):
                print("  " + ", ".join(f"{key}={value}" for key, value in summary.items()))
        for sheet in load_value_sheets(args.value_sheets):
            summary = summarize_value_sheet(sheet)
            print(f"{sheet['id']}: {summary['categories']} categories, {summary['fields']} fields")
        return 0
    launch_viewer(graphs, args.layout, load_value_sheets(args.value_sheets))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
