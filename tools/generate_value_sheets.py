from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_COMMON = ROOT / "resources" / "melee" / "extracted" / "plco_common_data.json"
DEFAULT_FALCON = ROOT / "resources" / "melee" / "extracted" / "captain_falcon_profile.json"
DEFAULT_OUTPUT = ROOT / "docs" / "state_graphs" / "value_sheets"

GLOBAL_CATEGORIES = {
    "input": [
        "main_stick_deadzone_x",
        "main_stick_deadzone_y",
        "c_stick_deadzone_x",
        "c_stick_deadzone_y",
        "tap_x_threshold",
        "tap_y_threshold",
        "trigger_deadzone",
        "trigger_timer_threshold",
        "z_shield_analog",
    ],
    "grounded_locomotion": [
        "walk_x",
        "walk_middle_velocity_ratio_milli",
        "walk_fast_velocity_ratio_milli",
        "walk_accel_taper_milli",
        "turn_x",
        "turn_run_x",
        "dash_x",
        "dash_tap_window",
        "dash_early_action_window",
        "dash_defensive_action_window",
        "dash_late_action_window",
        "dash_velocity_decay_milli",
        "run_x",
        "run_accel_taper_milli",
        "run_ground_friction_multiplier_milli",
        "high_speed_ground_friction_multiplier_milli",
        "run_turn_run_no_interrupt_frames",
        "animation_velocity_scale_milli",
    ],
    "jump_and_air": [
        "tap_jump_y",
        "tap_jump_window",
        "air_jump_backward_x",
        "tap_jump_release_y",
        "fast_fall_y",
        "fast_fall_window",
        "aerial_neutral_x",
        "aerial_neutral_y",
        "aerial_vertical_angle_tan_milli",
        "fall_animation_drift_threshold_milli",
        "fall_animation_blend_milli",
    ],
    "defense_and_platforms": [
        "crouch_y",
        "crouch_release_y",
        "tilt_x",
        "tilt_y",
        "escape_x",
        "escape_x_tap_window",
        "escape_y",
        "escape_y_tap_window",
        "fallspecial_platform_landing_y",
        "platform_pass_y",
        "platform_pass_y_tap_window",
        "pass_initial_y_velocity",
        "platform_drop_delay_ticks",
        "guard_on_catch_dash_window",
        "guard_reflect_input_window",
    ],
    "escape_air": [
        "escapeair_deadzone_x",
        "escapeair_deadzone_y",
        "escapeair_iasa_timer_ticks",
        "escapeair_force",
        "escapeair_decay_milli",
        "escapeair_landing_lag_ticks",
    ],
    "entry_and_respawn": [
        "entry_start_ticks",
        "entry_end_ticks",
        "entry_initial_scale_y_milli",
        "entry_collision_landing_lag_ticks",
    ],
}

CHARACTER_CATEGORIES = {
    "walk_and_run": [
        "walk_initial_velocity",
        "walk_accel",
        "walk_max_vel",
        "slow_walk_max_velocity",
        "mid_walk_threshold",
        "fast_walk_threshold",
        "traction_per_tick",
        "dash_run_terminal_velocity",
        "run_animation_scaling",
        "max_run_brake_frames",
        "ground_max_horizontal_velocity",
    ],
    "dash_and_turn": [
        "dash_initial_velocity",
        "dash_run_acceleration_a",
        "dash_run_acceleration_b",
        "frames_to_change_direction_on_standing_turn",
    ],
    "jump_and_air": [
        "jump_startup_time",
        "jump_h_initial_velocity",
        "jump_v_initial_velocity",
        "ground_to_air_jump_momentum_multiplier",
        "jump_h_max_velocity",
        "hop_v_initial_velocity",
        "air_jump_v_multiplier",
        "air_jump_h_multiplier",
        "max_jumps",
        "air_drift_stick_mul",
        "aerial_drift_base",
        "air_drift_max",
        "aerial_friction",
        "air_max_horizontal_velocity",
    ],
    "gravity_and_landing": [
        "grav",
        "terminal_vel",
        "fast_fall_velocity",
        "normal_landing_lag",
        "landingairn_lag",
        "landingairf_lag",
        "landingairb_lag",
        "landingairhi_lag",
        "landingairlw_lag",
    ],
    "entry_and_collision": [
        "entry_platform_offset_y",
    ],
}


def build_global_sheet(path: Path = DEFAULT_COMMON) -> dict[str, Any]:
    payload = _read_json(path)
    return _build_sheet(
        sheet_id="global_common_values",
        title="Global Common Values",
        scope="global",
        source=payload["source"],
        fields=payload["fields"],
        categories=GLOBAL_CATEGORIES,
        extra={},
    )


def build_character_sheet(
    path: Path = DEFAULT_FALCON,
    *,
    character_id: str = "captain_falcon",
) -> dict[str, Any]:
    payload = _read_json(path)
    return _build_sheet(
        sheet_id=f"{character_id}_values",
        title="Captain Falcon Character Values",
        scope="character",
        source=payload["source"],
        fields=payload["fields"],
        categories=CHARACTER_CATEGORIES,
        extra={"character_id": character_id},
    )


def generate_value_sheets(
    common_path: Path = DEFAULT_COMMON,
    falcon_path: Path = DEFAULT_FALCON,
    output_dir: Path = DEFAULT_OUTPUT,
) -> list[Path]:
    output_dir.mkdir(parents=True, exist_ok=True)
    sheets = [
        (output_dir / "global_common_values.json", build_global_sheet(common_path)),
        (output_dir / "captain_falcon_values.json", build_character_sheet(falcon_path)),
    ]
    for path, sheet in sheets:
        path.write_text(
            json.dumps(sheet, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
    return [path for path, _sheet in sheets]


def _build_sheet(
    *,
    sheet_id: str,
    title: str,
    scope: str,
    source: dict[str, Any],
    fields: dict[str, Any],
    categories: dict[str, list[str]],
    extra: dict[str, Any],
) -> dict[str, Any]:
    return {
        "id": sheet_id,
        "title": title,
        "scope": scope,
        "engine_boundary": "rust_core_authority",
        "source": source,
        **extra,
        "categories": [
            {
                "id": category_id,
                "label": category_id.replace("_", " ").title(),
                "fields": [
                    _field_row(rust_name, fields[rust_name])
                    for rust_name in rust_names
                ],
            }
            for category_id, rust_names in categories.items()
        ],
    }


def _field_row(rust_name: str, field: dict[str, Any]) -> dict[str, Any]:
    return {
        "rust_name": rust_name,
        "source_name": field["source_name"],
        "offset": field["offset"],
        "offset_hex": f"0x{field['offset']:x}",
        "kind": field["kind"],
        "raw": field.get("raw"),
        "converted_value": _converted_value(field),
        "provenance": "extracted_melee_dat",
    }


def _converted_value(field: dict[str, Any]) -> int | float | None:
    for key in ("stick_byte", "trigger_byte", "ticks", "milli"):
        if key in field:
            return field[key]
    return field.get("raw")


def _read_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Generate parity value sheets.")
    parser.add_argument("--common", type=Path, default=DEFAULT_COMMON)
    parser.add_argument("--falcon", type=Path, default=DEFAULT_FALCON)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    for path in generate_value_sheets(args.common, args.falcon, args.output):
        print(path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
