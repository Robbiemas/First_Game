from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_COMMON = ROOT / "resources" / "melee" / "extracted" / "plco_common_data.json"
DEFAULT_FALCON = ROOT / "resources" / "melee" / "extracted" / "captain_falcon_profile.json"
DEFAULT_OUTPUT = ROOT / "docs" / "state_graphs" / "value_sheets"
SOURCE_FLOAT_COMPARISON_FIELDS = {
    "dash_run_acceleration_a",
    "dash_run_acceleration_b",
    "dash_run_terminal_velocity",
}

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
        "walk_middle_velocity_ratio",
        "walk_fast_velocity_ratio",
        "walk_accel_taper",
        "turn_x",
        "turn_run_x",
        "dash_x",
        "dash_tap_window",
        "dash_early_action_window",
        "dash_defensive_action_window",
        "dash_late_action_window",
        "dash_velocity_decay",
        "run_x",
        "run_accel_taper",
        "run_ground_friction_multiplier",
        "high_speed_ground_friction_multiplier",
        "run_turn_run_no_interrupt_frames",
        "run_brake_animation_pause_velocity",
        "animation_velocity_scale",
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
        "fall_animation_drift_threshold",
        "fall_animation_blend",
        "air_speed_clamp_friction",
        "special_air_drift_stick_threshold",
    ],
    "collision_damage": [
        "knockback_weight_multiplier",
        "knockback_decay",
        "knockback_cap",
        "knockback_damage_scale",
        "knockback_hit_count_scale",
        "knockback_weight_set_damage",
        "knockback_result_scale",
        "knockback_result_offset",
    ],
    "defense_and_platforms": [
        "crouch_y",
        "crouch_release_y",
        "tilt_x",
        "tilt_y",
        "throw_down_y",
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
        "shield_aim_smoothing",
        "catch_ground_friction_multiplier",
    ],
    "escape_air": [
        "escapeair_deadzone_x",
        "escapeair_deadzone_y",
        "escapeair_iasa_timer_ticks",
        "escapeair_force",
        "escapeair_decay",
        "escapeair_landing_lag_ticks",
    ],
    "entry_and_respawn": [
        "dead_wait_ticks",
        "dead_up_star_wait_ticks",
        "dead_up_star_rise_ticks",
        "dead_up_star_exit_ticks",
        "top_blast_fall_ko_chance",
        "dead_up_fall_wait_ticks",
        "dead_up_fall_anim_ticks",
        "dead_up_fall_hit_camera_ticks",
        "dead_up_fall_drift_ticks",
        "dead_up_fall_exit_ticks",
        "rebirth_ticks",
        "rebirth_wait_ticks",
        "rebirth_hurt_intangible_ticks",
        "entry_start_ticks",
        "entry_end_ticks",
        "entry_initial_scale_y",
        "entry_collision_landing_lag_ticks",
    ],
    "ledge_and_cliff": [
        "cliff_grab_block_stick_y",
        "cliff_option_stick_threshold",
        "cliff_quick_percent_threshold",
        "cliff_wait_high_percent_ticks",
        "cliff_wait_low_percent_ticks",
        "cliff_wait_hurt_intangible_ticks",
        "ledge_cooldown_ticks",
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
        "ground_friction",
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
        "weight",
        "model_scaling",
        "entry_platform_offset_y",
    ],
    "shield_and_defense": [
        "initial_shield_size",
        "shield_break_initial_velocity",
    ],
}

GLOBAL_COMBAT_CATEGORIES = {
    "knockback": [
        "knockback_weight_multiplier",
        "knockback_decay",
        "knockback_cap",
        "knockback_damage_scale",
        "knockback_hit_count_scale",
        "knockback_weight_set_damage",
        "knockback_result_scale",
        "knockback_result_offset",
        "damage_knockback_velocity_scale",
        "damage_ground_knockback_friction_multiplier",
        "damage_ground_knockback_init_clamp",
        "damage_knockback_frame_decay",
        "throw_knockback_weight",
        "throw_weight_animation_scale",
    ],
    "damage_motion_thresholds": [
        "damage_landing_basic_knockback_threshold",
        "damage_landing_down_bound_knockback_threshold",
        "damage_motion_tier_1_threshold",
        "damage_motion_tier_2_threshold",
        "damage_motion_tier_3_threshold",
        "down_wait_timer",
    ],
    "damage_angles": [
        "damage_sakurai_air_angle_radians",
        "damage_sakurai_ground_angle_degrees",
        "damage_sakurai_ground_max_knockback",
        "damage_sakurai_ground_min_knockback",
        "damage_fly_top_angle_min_radians",
        "damage_fly_top_angle_max_radians",
        "damage_fly_top_random_chance",
        "damage_fly_top_random_percent_threshold",
    ],
    "hitlag_and_lcancel": [
        "hitlag_base_frames",
        "hitlag_crouch_multiplier",
        "hitlag_damage_scale",
        "hitlag_electric_multiplier",
        "hitlag_max_frames",
        "lcancel_divisor",
        "lcancel_window",
        "sdi_min_stick_mag",
        "sdi_stick_window",
        "sdi_pos_scale",
        "asdi_pos_scale",
        "di_angle_degrees",
        "trigger_di_knockback_multiplier",
    ],
    "passive_and_recovery": [
        "down_stand_stick_y",
        "passive_input_age_threshold",
        "passive_stand_stick_x",
        "passive_window_max",
        "landing_wait_y_velocity_threshold",
    ],
    "shield_and_defense": [
        "shield_start_health",
        "shield_release_lockout_frames",
        "shield_hold_drain",
        "shield_regen",
        "shield_break_reset_health",
        "shield_hit_drain_damage_scale",
        "shield_hit_drain_base",
        "shield_hit_lightshield_min",
        "shield_hit_lightshield_max",
        "shield_hold_lightshield_min",
        "shield_hold_lightshield_max",
        "shield_break_furafura_percent_base",
        "shield_break_furafura_timer_base",
        "shield_break_furafura_timer_decrement",
        "shield_break_furafura_mash_decrement",
        "guard_reflect_timer",
        "guard_reflect_damage_skip_timer",
        "shield_setoff_duration_base",
        "shield_setoff_duration_damage_scale",
        "shield_setoff_lightshield_min",
        "shield_setoff_lightshield_max",
        "shield_setoff_pushback_scale",
        "shield_setoff_pushback_cap",
        "shield_setoff_nonreflect_pushback_multiplier",
    ],
    "damage_response": [
        "damage_duration_scale",
        "throw_collision_lockout_ticks",
    ],
    "grab_and_capture": [
        "grab_mash_stick_threshold",
        "grab_timer_base",
        "grab_timer_handicap_scale",
        "grab_timer_handicap_offset",
        "grab_timer_rank_scale",
        "grab_timer_rank_offset",
        "grab_timer_percent_scale",
        "catch_cut_ground_velocity",
        "capture_jump_velocity_x",
        "capture_jump_velocity_y",
        "grab_timer_decrement",
        "grab_mash_timer_decrement",
        "capture_wait_jump_input_window",
        "capture_wait_mash_anim_timer",
        "capture_wait_mash_anim_rate",
        "throw_down_y",
    ],
}

FALCON_COMBAT_CATEGORIES = {
    "combat_attributes": [
        "weight",
        "weight_independent_throws_mask",
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


def build_global_combat_sheet(path: Path = DEFAULT_COMMON) -> dict[str, Any]:
    payload = _read_json(path)
    return _build_sheet(
        sheet_id="global_combat_values",
        title="Global Combat Values",
        scope="global_combat",
        source=payload["source"],
        fields=payload["fields"],
        categories=GLOBAL_COMBAT_CATEGORIES,
        extra={},
        owner_scope="global",
    )


def build_falcon_combat_sheet(
    path: Path = DEFAULT_FALCON,
    *,
    character_id: str = "captain_falcon",
) -> dict[str, Any]:
    payload = _read_json(path)
    return _build_sheet(
        sheet_id=f"{character_id}_combat_values",
        title="Captain Falcon Combat Values",
        scope="character_combat",
        source=payload["source"],
        fields=payload["fields"],
        categories=FALCON_COMBAT_CATEGORIES,
        extra={"character_id": character_id},
        owner_scope="character",
        owner_id=character_id,
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
        (output_dir / "global_combat_values.json", build_global_combat_sheet(common_path)),
        (
            output_dir / "captain_falcon_combat_values.json",
            build_falcon_combat_sheet(falcon_path),
        ),
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
    owner_scope: str | None = None,
    owner_id: str | None = None,
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
                    _field_row(rust_name, fields[rust_name], owner_scope, owner_id)
                    for rust_name in rust_names
                ],
            }
            for category_id, rust_names in categories.items()
        ],
    }


def _field_row(
    rust_name: str,
    field: dict[str, Any],
    owner_scope: str | None = None,
    owner_id: str | None = None,
) -> dict[str, Any]:
    row = {
        "rust_name": rust_name,
        "source_name": field["source_name"],
        "offset": field["offset"],
        "offset_hex": f"0x{field['offset']:x}",
        "kind": field["kind"],
        "raw": field.get("raw"),
        "converted_value": _converted_value(rust_name, field),
        "comparison_value_kind": _comparison_value_kind(rust_name, field),
        "provenance": "extracted_melee_dat",
    }
    if owner_scope is not None:
        row["owner_scope"] = owner_scope
    if owner_id is not None:
        row["owner_id"] = owner_id
    return row


def _converted_value(rust_name: str, field: dict[str, Any]) -> int | float | None:
    if rust_name in SOURCE_FLOAT_COMPARISON_FIELDS or field.get("kind") == "source_f32":
        return field.get("raw")
    for key in ("stick_byte", "trigger_byte", "ticks", "milli"):
        if key in field:
            return field[key]
    return field.get("raw")


def _comparison_value_kind(rust_name: str, field: dict[str, Any]) -> str:
    if rust_name in SOURCE_FLOAT_COMPARISON_FIELDS or field.get("kind") == "source_f32":
        return "source_f32"
    for key in ("stick_byte", "trigger_byte", "ticks", "milli"):
        if key in field:
            return key
    return "raw"


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
