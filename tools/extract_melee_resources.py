"""Extract Melee gameplay values from user-provided DAT files.

This tool intentionally does not download or vendor raw game data. Put locally
extracted files in resources/melee/raw, run this script, and commit only the
small JSON outputs if we decide a generated snapshot is useful for the current
bootstrap phase.
"""

from __future__ import annotations

import argparse
import json
import math
import struct
from dataclasses import dataclass
from pathlib import Path
from typing import Callable


PROJECT_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_RAW_DIR = PROJECT_ROOT / "resources" / "melee" / "raw"
DEFAULT_OUT_DIR = PROJECT_ROOT / "resources" / "melee" / "extracted"


@dataclass(frozen=True)
class Field:
    rust_name: str
    source_name: str
    offset: int
    kind: str


COMMON_FIELDS = (
    Field("tap_x_threshold", "x8_someStickThreshold", 0x08, "stick"),
    Field("tap_y_threshold", "xC", 0x0C, "stick"),
    Field("trigger_deadzone", "x10", 0x10, "trigger"),
    Field("z_shield_analog", "x14", 0x14, "trigger"),
    Field("trigger_timer_threshold", "x18", 0x18, "trigger"),
    Field("aerial_vertical_angle_tan_milli", "x20_radians", 0x20, "radian_tangent_milli"),
    Field("walk_x", "x24", 0x24, "stick"),
    Field("walk_slow_x", "x28", 0x28, "stick"),
    Field("walk_middle_x", "x2C", 0x2C, "stick"),
    Field("walk_fast_x", "x30", 0x30, "stick"),
    Field("turn_x", "x34", 0x34, "stick"),
    Field("dash_x", "x3C", 0x3C, "stick"),
    Field("dash_tap_window", "x40", 0x40, "i32_ticks"),
    Field("dash_early_action_window", "x44", 0x44, "f32_ticks"),
    Field("dash_defensive_action_window", "x48", 0x48, "f32_ticks"),
    Field("dash_late_action_window", "x4C", 0x4C, "f32_ticks"),
    Field("run_x", "x58_someLStickXThreshold", 0x58, "stick"),
    Field("guard_on_catch_dash_window", "x68", 0x68, "f32_ticks"),
    Field("tap_jump_y", "tap_jump_threshold", 0x70, "stick"),
    Field("tap_jump_window", "x74", 0x74, "i32_ticks"),
    Field("air_jump_backward_x", "x78", 0x78, "stick"),
    Field("tap_jump_release_y", "tap_jump_release_threshold", 0x7C, "stick"),
    Field("fast_fall_y", "x88", 0x88, "stick"),
    Field("fast_fall_window", "x8C", 0x8C, "i32_ticks"),
    Field("crouch_y", "x90", 0x90, "stick"),
    Field("crouch_release_y", "x94", 0x94, "stick"),
    Field("tilt_x", "x98", 0x98, "stick"),
    Field("tilt_y", "attackhi3_stick_threshold_y", 0xAC, "stick"),
    Field("aerial_neutral_x", "xDC", 0xDC, "stick"),
    Field("aerial_neutral_y", "xE0", 0xE0, "stick"),
    Field("fallspecial_platform_landing_y", "x25C", 0x25C, "stick"),
    Field("escape_y", "x314", 0x314, "stick"),
    Field("escape_y_tap_window", "x318", 0x318, "i32_ticks"),
    Field("escape_x", "x31C", 0x31C, "stick"),
    Field("escape_x_tap_window", "x320", 0x320, "i32_ticks"),
    Field("escapeair_deadzone_x", "escapeair_deadzone.x", 0x32C, "stick"),
    Field("escapeair_deadzone_y", "escapeair_deadzone.y", 0x330, "stick"),
    Field("escapeair_iasa_timer_ticks", "x334", 0x334, "i32_ticks"),
    Field("escapeair_force", "escapeair_force", 0x338, "milli"),
    Field("escapeair_decay_milli", "escapeair_decay", 0x33C, "milli"),
    Field("escapeair_landing_lag_ticks", "x344", 0x344, "f32_ticks"),
    Field("run_turn_run_no_interrupt_frames", "x430", 0x430, "f32_ticks"),
    Field("platform_pass_y", "x464", 0x464, "stick"),
    Field("platform_pass_y_tap_window", "x468", 0x468, "f32_ticks"),
    Field("pass_initial_y_velocity", "x46C", 0x46C, "milli"),
    Field("platform_drop_delay_ticks", "x470", 0x470, "f32_ticks"),
)


PROFILE_FIELDS = (
    Field("walk_initial_velocity", "walk_initial_velocity", 0x00, "milli"),
    Field("walk_accel", "walk_accel", 0x04, "milli"),
    Field("walk_max_vel", "walk_max_vel", 0x08, "milli"),
    Field("slow_walk_max_velocity", "slow_walk_max_velocity", 0x0C, "milli"),
    Field("mid_walk_threshold", "mid_walk_threshold", 0x10, "milli"),
    Field("fast_walk_threshold", "fast_walk_threshold", 0x14, "milli"),
    Field("traction_per_tick", "gr_friction", 0x18, "milli"),
    Field("dash_initial_velocity", "dash_initial_velocity", 0x1C, "milli"),
    Field("dash_run_acceleration_a", "dash_run_acceleration_a", 0x20, "milli"),
    Field("dash_run_acceleration_b", "dash_run_acceleration_b", 0x24, "milli"),
    Field("dash_run_terminal_velocity", "dash_run_terminal_velocity", 0x28, "milli"),
    Field("run_animation_scaling", "run_animation_scaling", 0x2C, "milli"),
    Field("max_run_brake_frames", "max_run_brake_frames", 0x30, "f32_ticks"),
    Field("ground_max_horizontal_velocity", "ground_max_horizontal_velocity", 0x34, "milli"),
    Field("jump_startup_time", "jump_startup_time", 0x38, "f32_ticks"),
    Field("jump_h_initial_velocity", "jump_h_initial_velocity", 0x3C, "milli"),
    Field("jump_v_initial_velocity", "jump_v_initial_velocity", 0x40, "milli"),
    Field("ground_to_air_jump_momentum_multiplier", "ground_to_air_jump_momentum_multiplier", 0x44, "milli"),
    Field("jump_h_max_velocity", "jump_h_max_velocity", 0x48, "milli"),
    Field("hop_v_initial_velocity", "hop_v_initial_velocity", 0x4C, "milli"),
    Field("air_jump_v_multiplier", "air_jump_v_multiplier", 0x50, "milli"),
    Field("air_jump_h_multiplier", "air_jump_h_multiplier", 0x54, "milli"),
    Field("max_jumps", "max_jumps", 0x58, "i32_ticks"),
    Field("grav", "grav", 0x5C, "milli"),
    Field("terminal_vel", "terminal_vel", 0x60, "milli"),
    Field("air_drift_stick_mul", "air_drift_stick_mul", 0x64, "milli"),
    Field("aerial_drift_base", "aerial_drift_base", 0x68, "milli"),
    Field("air_drift_max", "air_drift_max", 0x6C, "milli"),
    Field("aerial_friction", "aerial_friction", 0x70, "milli"),
    Field("fast_fall_velocity", "fast_fall_velocity", 0x74, "milli"),
    Field("air_max_horizontal_velocity", "air_max_horizontal_velocity", 0x78, "milli"),
    Field("frames_to_change_direction_on_standing_turn", "frames_to_change_direction_on_standing_turn", 0x84, "f32_ticks"),
    Field("normal_landing_lag", "normal_landing_lag", 0xE4, "f32_ticks"),
)


class DatExtractError(ValueError):
    pass


def read_u32(data: bytes | bytearray, offset: int) -> int:
    return int.from_bytes(data[offset : offset + 4], "big")


def read_i32(data: bytes | bytearray, offset: int) -> int:
    return int.from_bytes(data[offset : offset + 4], "big", signed=True)


def read_f32(data: bytes | bytearray, offset: int) -> float:
    return struct.unpack(">f", data[offset : offset + 4])[0]


def rust_round(value: float) -> int:
    if not math.isfinite(value):
        raise DatExtractError(f"non-finite float {value!r}")
    if value >= 0:
        return math.floor(value + 0.5)
    return math.ceil(value - 0.5)


def dat_header_counts(dat: bytes) -> tuple[int, int, int, int]:
    if len(dat) < 0x20:
        raise DatExtractError("DAT file is shorter than the 0x20-byte header")
    data_block_size = read_u32(dat, 0x04)
    relocation_count = read_u32(dat, 0x08)
    root_count = read_u32(dat, 0x0C)
    external_count = read_u32(dat, 0x10)
    return data_block_size, relocation_count, root_count, external_count


def dat_string(dat: bytes, string_table_offset: int, relative_offset: int) -> str:
    start = string_table_offset + relative_offset
    end = dat.index(0, start)
    return dat[start:end].decode("ascii")


def dat_roots(dat: bytes) -> dict[str, int]:
    data_block_size, relocation_count, root_count, external_count = dat_header_counts(dat)
    root_table_offset = 0x20 + data_block_size + relocation_count * 4
    external_table_offset = root_table_offset + root_count * 8
    string_table_offset = external_table_offset + external_count * 8
    roots: dict[str, int] = {}
    for index in range(root_count):
        entry = root_table_offset + index * 8
        data_offset = read_u32(dat, entry)
        string_offset = read_u32(dat, entry + 4)
        roots[dat_string(dat, string_table_offset, string_offset)] = data_offset
    return roots


def find_root(dat: bytes, predicate: Callable[[str], bool], description: str) -> tuple[str, int]:
    for name, offset in dat_roots(dat).items():
        if predicate(name):
            return name, offset
    raise DatExtractError(f"could not find {description} root node")


def field_value(block: bytes, field: Field) -> dict[str, object]:
    raw = read_f32(block, field.offset) if field.kind != "i32_ticks" else read_i32(block, field.offset)
    result: dict[str, object] = {
        "source_name": field.source_name,
        "offset": field.offset,
        "kind": field.kind,
        "raw": raw,
    }
    if field.kind == "stick":
        result["stick_byte"] = rust_round(float(raw) * 127.0)
    elif field.kind == "trigger":
        result["trigger_byte"] = rust_round(float(raw) * 255.0)
    elif field.kind == "milli":
        result["milli"] = rust_round(float(raw) * 1000.0)
    elif field.kind == "f32_ticks":
        result["ticks"] = rust_round(float(raw))
    elif field.kind == "i32_ticks":
        result["ticks"] = int(raw)
    elif field.kind == "radian_tangent_milli":
        result["milli"] = rust_round(math.tan(float(raw)) * 1000.0)
    else:
        raise DatExtractError(f"unknown field kind {field.kind!r}")
    return result


def source_path_for_json(source_path: Path) -> str:
    try:
        return source_path.resolve().relative_to(PROJECT_ROOT).as_posix()
    except ValueError:
        return source_path.as_posix()


def extract_common_data_from_plco(dat: bytes, source_path: Path) -> dict[str, object]:
    symbol, ftload_offset = find_root(dat, lambda name: name == "ftLoadCommonData", "ftLoadCommonData")
    common_offset = read_u32(dat, 0x20 + ftload_offset)
    data_block_size, _relocation_count, _root_count, _external_count = dat_header_counts(dat)
    required_len = common_offset + max(field.offset for field in COMMON_FIELDS) + 4
    if required_len > data_block_size:
        raise DatExtractError("CommonAttributes pointer does not cover required common-data fields")
    block = dat[0x20 + common_offset : 0x20 + data_block_size]
    return {
        "source": {
            "file": source_path_for_json(source_path),
            "symbol": symbol,
            "ft_load_common_data_offset": ftload_offset,
            "common_attributes_offset": common_offset,
            "format": "HSD DAT, big-endian floats/ints",
        },
        "fields": {field.rust_name: field_value(block, field) for field in COMMON_FIELDS},
    }


def extract_captain_profile_from_plca(dat: bytes, source_path: Path) -> dict[str, object]:
    symbol, ftdata_offset = find_root(dat, lambda name: name == "ftDataCaptain", "ftDataCaptain")
    header_offset = 0x20 + ftdata_offset
    attrs_offset = read_u32(dat, header_offset)
    data_block_size, _relocation_count, _root_count, _external_count = dat_header_counts(dat)
    attrs_end = read_u32(dat, header_offset + 4)
    if attrs_end <= attrs_offset or attrs_end > data_block_size:
        attrs_end = data_block_size
    required_len = attrs_offset + max(field.offset for field in PROFILE_FIELDS) + 4
    if required_len > attrs_end:
        raise DatExtractError("ftDataCaptain.x0 does not cover required ftCo_DatAttrs fields")
    attrs = dat[0x20 + attrs_offset : 0x20 + attrs_end]
    return {
        "source": {
            "file": source_path_for_json(source_path),
            "symbol": symbol,
            "ft_data_offset": ftdata_offset,
            "ftco_dat_attrs_offset": attrs_offset,
            "ftco_dat_attrs_len": attrs_end - attrs_offset,
            "format": "HSD DAT, big-endian floats/ints",
        },
        "fields": {field.rust_name: field_value(attrs, field) for field in PROFILE_FIELDS},
    }


def write_json(path: Path, payload: dict[str, object]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def extract_resources(raw_dir: Path, out_dir: Path) -> list[Path]:
    written: list[Path] = []
    plco = raw_dir / "PlCo.dat"
    if plco.exists():
        out_path = out_dir / "plco_common_data.json"
        write_json(out_path, extract_common_data_from_plco(plco.read_bytes(), plco))
        written.append(out_path)
    plca = raw_dir / "PlCa.dat"
    if plca.exists():
        out_path = out_dir / "captain_falcon_profile.json"
        write_json(out_path, extract_captain_profile_from_plca(plca.read_bytes(), plca))
        written.append(out_path)
    return written


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--raw-dir", type=Path, default=DEFAULT_RAW_DIR)
    parser.add_argument("--out-dir", type=Path, default=DEFAULT_OUT_DIR)
    args = parser.parse_args()

    written = extract_resources(args.raw_dir, args.out_dir)
    if not written:
        print(f"No supported DAT files found in {args.raw_dir}")
        print("Expected user-provided files: PlCo.dat and/or PlCa.dat")
        return 2
    for path in written:
        print(path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
