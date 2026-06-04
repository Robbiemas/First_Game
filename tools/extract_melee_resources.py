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

try:
    from tools.falcon_ecb_mapping import sampled_action_ids
except ModuleNotFoundError:  # pragma: no cover - direct script execution path
    from falcon_ecb_mapping import sampled_action_ids  # type: ignore

PROJECT_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_RAW_DIR = PROJECT_ROOT / "resources" / "melee" / "raw"
DEFAULT_OUT_DIR = PROJECT_ROOT / "resources" / "melee" / "extracted"
DEFAULT_ISO_FILES = ("PlCo.dat", "PlCa.dat", "PlCaAJ.dat", "PlCaNr.dat")
ECB_SAMPLE_ACTION_STATE_IDS = sampled_action_ids()
CAPTAIN_ACTION_COUNT = 318
FIGHTER_WAIT_ANIM_DATA_SIZE = 0x18
FIGA_TRACK_SIZE = 0x0C
FIGA_TREE_SIZE = 0x14
HSD_JOINT_SIZE = 0x40
FT_HURTBOX_INIT_SIZE = 0x28
JOBJ_INSTANCE = 1 << 12
HSD_A_OP_CON = 1
HSD_A_OP_LIN = 2
HSD_A_OP_SPL0 = 3
HSD_A_OP_SPL = 4
HSD_A_OP_SLP = 5
HSD_A_OP_KEY = 6
HSD_A_FRAC_FLOAT = 0 << 5
HSD_A_FRAC_S16 = 1 << 5
HSD_A_FRAC_U16 = 2 << 5
HSD_A_FRAC_S8 = 3 << 5
HSD_A_FRAC_U8 = 4 << 5
FOBJ_LOAD_DATA0 = 1
FOBJ_LOAD_DATA = 2
FOBJ_LOAD_WAIT = 3
JOBJ_ANIM_CHANNELS = {
    1: ("rotation", "x"),
    2: ("rotation", "y"),
    3: ("rotation", "z"),
    5: ("translation", "x"),
    6: ("translation", "y"),
    7: ("translation", "z"),
    8: ("scale", "x"),
    9: ("scale", "y"),
    10: ("scale", "z"),
}


@dataclass(frozen=True)
class Field:
    rust_name: str
    source_name: str
    offset: int
    kind: str


COMMON_FIELDS = (
    Field("main_stick_deadzone_x", "x0", 0x00, "stick"),
    Field("main_stick_deadzone_y", "x4", 0x04, "stick"),
    Field("c_stick_deadzone_x", "x0", 0x00, "stick"),
    Field("c_stick_deadzone_y", "x4", 0x04, "stick"),
    Field("tap_x_threshold", "x8_someStickThreshold", 0x08, "stick"),
    Field("tap_y_threshold", "xC", 0x0C, "stick"),
    Field("trigger_deadzone", "x10", 0x10, "trigger"),
    Field("z_shield_analog", "x14", 0x14, "trigger"),
    Field("trigger_timer_threshold", "x18", 0x18, "trigger"),
    Field("aerial_vertical_angle_tan_milli", "x20_radians", 0x20, "radian_tangent_milli"),
    Field("walk_x", "x24", 0x24, "stick"),
    Field("walk_middle_velocity_ratio", "x28", 0x28, "source_f32"),
    Field("walk_fast_velocity_ratio", "x2C", 0x2C, "source_f32"),
    Field("walk_accel_taper", "x30", 0x30, "source_f32"),
    Field("turn_x", "x34", 0x34, "stick"),
    Field("turn_run_x", "x38_someLStickXThreshold", 0x38, "stick"),
    Field("dash_x", "x3C", 0x3C, "stick"),
    Field("dash_tap_window", "x40", 0x40, "i32_ticks"),
    Field("dash_early_action_window", "x44", 0x44, "f32_ticks"),
    Field("dash_defensive_action_window", "x48", 0x48, "f32_ticks"),
    Field("dash_late_action_window", "x4C", 0x4C, "f32_ticks"),
    Field("dash_velocity_decay", "x54", 0x54, "source_f32"),
    Field("run_x", "x58_someLStickXThreshold", 0x58, "stick"),
    Field("run_accel_taper", "x5C", 0x5C, "source_f32"),
    Field("run_ground_friction_multiplier", "x60_someFrictionMul", 0x60, "source_f32"),
    Field("guard_on_catch_dash_window", "x68", 0x68, "f32_ticks"),
    Field("high_speed_ground_friction_multiplier", "x6C", 0x6C, "source_f32"),
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
    Field("lcancel_window", "xE4", 0xE4, "i32_ticks"),
    Field("lcancel_divisor", "xE8", 0xE8, "source_f32"),
    Field("fallspecial_platform_landing_y", "x25C", 0x25C, "stick"),
    Field("guard_reflect_input_window", "x2A0", 0x2A0, "i32_ticks"),
    Field("escape_y", "x314", 0x314, "stick"),
    Field("escape_y_tap_window", "x318", 0x318, "i32_ticks"),
    Field("escape_x", "x31C", 0x31C, "stick"),
    Field("escape_x_tap_window", "x320", 0x320, "i32_ticks"),
    Field("escapeair_deadzone_x", "escapeair_deadzone.x", 0x32C, "stick"),
    Field("escapeair_deadzone_y", "escapeair_deadzone.y", 0x330, "stick"),
    Field("escapeair_iasa_timer_ticks", "x334", 0x334, "i32_ticks"),
    Field("escapeair_force", "escapeair_force", 0x338, "source_f32"),
    Field("escapeair_decay", "escapeair_decay", 0x33C, "source_f32"),
    Field("escapeair_landing_lag_ticks", "x344", 0x344, "f32_ticks"),
    Field("run_brake_animation_pause_velocity", "x42C", 0x42C, "source_f32"),
    Field("run_turn_run_no_interrupt_frames", "x430", 0x430, "f32_ticks"),
    Field("animation_velocity_scale", "x440", 0x440, "source_f32"),
    Field("fall_animation_drift_threshold", "x444", 0x444, "source_f32"),
    Field("fall_animation_blend", "x448", 0x448, "source_f32"),
    Field("platform_pass_y", "x464", 0x464, "stick"),
    Field("platform_pass_y_tap_window", "x468", 0x468, "f32_ticks"),
    Field("pass_initial_y_velocity", "x46C", 0x46C, "source_f32"),
    Field("platform_drop_delay_ticks", "x470", 0x470, "f32_ticks"),
    Field("entry_start_ticks", "x6BC", 0x6BC, "i32_ticks"),
    Field("entry_end_ticks", "x6C0", 0x6C0, "i32_ticks"),
    Field("entry_initial_scale_y", "x6C4", 0x6C4, "source_f32"),
    Field("entry_collision_landing_lag_ticks", "x6C8", 0x6C8, "i32_ticks"),
)

FIGHTER_CMD_LENGTHS = (
    5,
    5,
    1,
    1,
    1,
    1,
    1,
    3,
    1,
    1,
    1,
    1,
    1,
    1,
    1,
    1,
    1,
    1,
    1,
    1,
    1,
    1,
    1,
    1,
    3,
    1,
    1,
    1,
    7,
    4,
    1,
    1,
    1,
    1,
    1,
    1,
    1,
    1,
    1,
    1,
    1,
    1,
    1,
    1,
    3,
    3,
    2,
    1,
    4,
)
COMMON_CMD_LENGTHS = (1, 1, 1, 1, 1, 1, 1, 1, 1, 1)


PROFILE_FIELDS = (
    Field("walk_initial_velocity", "walk_initial_velocity", 0x00, "source_f32"),
    Field("walk_accel", "walk_accel", 0x04, "source_f32"),
    Field("walk_max_vel", "walk_max_vel", 0x08, "source_f32"),
    Field("slow_walk_max_velocity", "slow_walk_max_velocity", 0x0C, "source_f32"),
    Field("mid_walk_threshold", "mid_walk_threshold", 0x10, "source_f32"),
    Field("fast_walk_threshold", "fast_walk_threshold", 0x14, "source_f32"),
    Field("ground_friction", "gr_friction", 0x18, "source_f32"),
    Field("dash_initial_velocity", "dash_initial_velocity", 0x1C, "source_f32"),
    Field("dash_run_acceleration_a", "dash_run_acceleration_a", 0x20, "source_f32"),
    Field("dash_run_acceleration_b", "dash_run_acceleration_b", 0x24, "source_f32"),
    Field("dash_run_terminal_velocity", "dash_run_terminal_velocity", 0x28, "source_f32"),
    Field("run_animation_scaling", "run_animation_scaling", 0x2C, "source_f32"),
    Field("max_run_brake_frames", "max_run_brake_frames", 0x30, "f32_ticks"),
    Field("ground_max_horizontal_velocity", "ground_max_horizontal_velocity", 0x34, "source_f32"),
    Field("jump_startup_time", "jump_startup_time", 0x38, "f32_ticks"),
    Field("jump_h_initial_velocity", "jump_h_initial_velocity", 0x3C, "source_f32"),
    Field("jump_v_initial_velocity", "jump_v_initial_velocity", 0x40, "source_f32"),
    Field("ground_to_air_jump_momentum_multiplier", "ground_to_air_jump_momentum_multiplier", 0x44, "source_f32"),
    Field("jump_h_max_velocity", "jump_h_max_velocity", 0x48, "source_f32"),
    Field("hop_v_initial_velocity", "hop_v_initial_velocity", 0x4C, "source_f32"),
    Field("air_jump_v_multiplier", "air_jump_v_multiplier", 0x50, "source_f32"),
    Field("air_jump_h_multiplier", "air_jump_h_multiplier", 0x54, "source_f32"),
    Field("max_jumps", "max_jumps", 0x58, "i32_ticks"),
    Field("grav", "grav", 0x5C, "source_f32"),
    Field("terminal_vel", "terminal_vel", 0x60, "source_f32"),
    Field("air_drift_stick_mul", "air_drift_stick_mul", 0x64, "source_f32"),
    Field("aerial_drift_base", "aerial_drift_base", 0x68, "source_f32"),
    Field("air_drift_max", "air_drift_max", 0x6C, "source_f32"),
    Field("aerial_friction", "aerial_friction", 0x70, "source_f32"),
    Field("fast_fall_velocity", "fast_fall_velocity", 0x74, "source_f32"),
    Field("air_max_horizontal_velocity", "air_max_horizontal_velocity", 0x78, "source_f32"),
    Field("frames_to_change_direction_on_standing_turn", "frames_to_change_direction_on_standing_turn", 0x84, "f32_ticks"),
    Field("normal_landing_lag", "normal_landing_lag", 0xE4, "f32_ticks"),
    Field("landingairn_lag", "landingairn_lag", 0xE8, "f32_ticks"),
    Field("landingairf_lag", "landingairf_lag", 0xEC, "f32_ticks"),
    Field("landingairb_lag", "landingairb_lag", 0xF0, "f32_ticks"),
    Field("landingairhi_lag", "landingairhi_lag", 0xF4, "f32_ticks"),
    Field("landingairlw_lag", "landingairlw_lag", 0xF8, "f32_ticks"),
    Field("entry_platform_offset_y", "trophy_scale*1.497345", 0x110, "entry_platform_offset_milli"),
)


class DatExtractError(ValueError):
    pass


@dataclass(frozen=True)
class CharacterResourceSpec:
    """Decomp-anchored Melee fighter resource names for offline extraction."""

    id: str
    output_stem: str
    data_dat: str
    action_dat: str
    neutral_costume_dat: str
    ft_data_symbol: str
    neutral_joint_root: str
    action_count: int = CAPTAIN_ACTION_COUNT
    derived_sample_action_state_ids: tuple[int, ...] = ()


# Source anchors:
# - ftCaptain/ftCa_Init.c names PlCa.dat, ftDataCaptain, PlyCaptain5K_Share_joint.
# - ftMars/ftMs_Init.c names PlMs.dat, ftDataMars, PlyMars5K_Share_joint, PlMsAJ.dat.
CHARACTER_RESOURCE_SPECS: dict[str, CharacterResourceSpec] = {
    "captain": CharacterResourceSpec(
        id="captain",
        output_stem="captain_falcon",
        data_dat="PlCa.dat",
        action_dat="PlCaAJ.dat",
        neutral_costume_dat="PlCaNr.dat",
        ft_data_symbol="ftDataCaptain",
        neutral_joint_root="PlyCaptain5K_Share_joint",
        derived_sample_action_state_ids=ECB_SAMPLE_ACTION_STATE_IDS,
    ),
    "marth": CharacterResourceSpec(
        id="marth",
        output_stem="marth",
        data_dat="PlMs.dat",
        action_dat="PlMsAJ.dat",
        neutral_costume_dat="PlMsNr.dat",
        ft_data_symbol="ftDataMars",
        neutral_joint_root="PlyMars5K_Share_joint",
    ),
}
CHARACTER_ALIASES = {
    "captain_falcon": "captain",
    "falcon": "captain",
    "mars": "marth",
}


def character_resource_spec(character: str) -> CharacterResourceSpec:
    key = CHARACTER_ALIASES.get(character.lower(), character.lower())
    try:
        return CHARACTER_RESOURCE_SPECS[key]
    except KeyError as error:
        supported = ", ".join(sorted(CHARACTER_RESOURCE_SPECS))
        raise DatExtractError(f"unsupported source character {character!r}; supported: {supported}") from error


def read_u32(data: bytes | bytearray, offset: int) -> int:
    return int.from_bytes(data[offset : offset + 4], "big")


def read_i32(data: bytes | bytearray, offset: int) -> int:
    return int.from_bytes(data[offset : offset + 4], "big", signed=True)


def read_i16(data: bytes | bytearray, offset: int) -> int:
    return int.from_bytes(data[offset : offset + 2], "big", signed=True)


def read_u16(data: bytes | bytearray, offset: int) -> int:
    return int.from_bytes(data[offset : offset + 2], "big")


def read_i8(data: bytes | bytearray, offset: int) -> int:
    return int.from_bytes(data[offset : offset + 1], "big", signed=True)


def read_f32(data: bytes | bytearray, offset: int) -> float:
    return struct.unpack(">f", data[offset : offset + 4])[0]


def checked_u32(data: bytes | bytearray, offset: int, field: str) -> int:
    if offset < 0 or offset + 4 > len(data):
        raise DatExtractError(f"{field} at 0x{offset:x} is outside the source bytes")
    return read_u32(data, offset)


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


def fst_string(iso: bytes, string_table_offset: int, relative_offset: int) -> str:
    start = string_table_offset + relative_offset
    if start < string_table_offset or start >= len(iso):
        raise DatExtractError(f"FST string offset 0x{relative_offset:x} is outside the ISO")
    try:
        end = iso.index(0, start)
    except ValueError as error:
        raise DatExtractError(f"FST string at 0x{relative_offset:x} is unterminated") from error
    return iso[start:end].decode("ascii")


def extract_file_from_gamecube_iso(iso: bytes, file_name: str) -> bytes:
    fst_offset = checked_u32(iso, 0x424, "FST offset")
    fst_size = checked_u32(iso, 0x428, "FST size")
    if fst_offset == 0 or fst_size == 0:
        raise DatExtractError("GameCube ISO does not contain an FST")
    if fst_offset + fst_size > len(iso):
        raise DatExtractError("GameCube ISO FST range is outside the source bytes")
    entry_count = checked_u32(iso, fst_offset + 8, "FST root next-index")
    if entry_count < 1:
        raise DatExtractError("GameCube ISO FST has no root entry")
    string_table_offset = fst_offset + entry_count * 12
    if string_table_offset > fst_offset + fst_size:
        raise DatExtractError("GameCube ISO FST string table starts outside the FST")

    for index in range(1, entry_count):
        entry = fst_offset + index * 12
        word0 = checked_u32(iso, entry, "FST entry word0")
        is_dir = (word0 & 0x0100_0000) != 0
        name_offset = word0 & 0x00FF_FFFF
        name = fst_string(iso, string_table_offset, name_offset)
        if is_dir or name != file_name:
            continue
        file_offset = checked_u32(iso, entry + 4, f"{file_name} offset")
        file_size = checked_u32(iso, entry + 8, f"{file_name} size")
        if file_offset + file_size > len(iso):
            raise DatExtractError(f"{file_name} range is outside the ISO")
        return iso[file_offset : file_offset + file_size]

    raise DatExtractError(f"{file_name} was not found in the GameCube ISO FST")


def extract_raw_files_from_gamecube_iso(
    iso_path: Path,
    raw_dir: Path,
    file_names: tuple[str, ...] = DEFAULT_ISO_FILES,
) -> list[Path]:
    iso = iso_path.read_bytes()
    written: list[Path] = []
    raw_dir.mkdir(parents=True, exist_ok=True)
    for file_name in file_names:
        payload = extract_file_from_gamecube_iso(iso, file_name)
        path = raw_dir / file_name
        path.write_bytes(payload)
        written.append(path)
    return written


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
    elif field.kind == "source_f32":
        pass
    elif field.kind == "f32_ticks":
        result["ticks"] = rust_round(float(raw))
    elif field.kind == "i32_ticks":
        result["ticks"] = int(raw)
    elif field.kind == "radian_tangent_milli":
        result["milli"] = rust_round(math.tan(float(raw)) * 1000.0)
    elif field.kind == "entry_platform_offset_milli":
        result["milli"] = rust_round(float(raw) * 1.497345 * 1000.0)
    else:
        raise DatExtractError(f"unknown field kind {field.kind!r}")
    return result


def _command_frame_value(raw_value: int) -> int:
    if raw_value >= 0x10000 and raw_value & 0xFFFF == 0:
        return raw_value >> 16
    return raw_value


def _bitfield(word: int, offset_from_msb: int, width: int) -> int:
    shift = 32 - offset_from_msb - width
    return (word >> shift) & ((1 << width) - 1)


def extract_action_script_cmd_var_events(script_bytes: bytes, script_offset: int) -> list[dict[str, int]]:
    """Decode the minimal fighter command subset needed for locomotion cmd vars."""
    events: list[dict[str, int]] = []
    word_offset = 0
    current_frame = 0

    while script_offset + word_offset * 4 + 4 <= len(script_bytes):
        word = read_u32(script_bytes, script_offset + word_offset * 4)
        opcode = word >> 26
        value = word & 0x03FF_FFFF

        if opcode == 0:
            break
        if opcode == 1:
            current_frame += _command_frame_value(value)
            word_offset += 1
            continue
        if opcode == 2:
            current_frame = _command_frame_value(value)
            word_offset += 1
            continue
        if opcode == 8:
            word_offset += 1
            continue
        if opcode in (3, 4, 5, 6, 7, 9):
            word_offset += COMMON_CMD_LENGTHS[opcode]
            continue
        if opcode == 19:
            events.append(
                {
                    "frame": current_frame,
                    "cmd_var": (word >> 24) & 0x03,
                    "value": word & 0x00FF_FFFF,
                    "word_offset": word_offset,
                }
            )
            word_offset += 1
            continue

        if opcode < 10:
            raise DatExtractError(
                f"unsupported common action script command opcode {opcode} at word {word_offset}"
            )
        fighter_index = opcode - 10
        if fighter_index >= len(FIGHTER_CMD_LENGTHS):
            raise DatExtractError(
                f"unsupported fighter action script opcode {opcode} at word {word_offset}"
            )
        word_offset += FIGHTER_CMD_LENGTHS[fighter_index]

    return events


def extract_action_script_hitbox_bone_indices(script_bytes: bytes, script_offset: int) -> set[int]:
    """Decode hitbox bone indices from ftAction_8007121C fighter commands."""
    bone_indices: set[int] = set()
    word_offset = 0

    while script_offset + word_offset * 4 + 4 <= len(script_bytes):
        word = read_u32(script_bytes, script_offset + word_offset * 4)
        opcode = word >> 26
        value = word & 0x03FF_FFFF

        if opcode == 0:
            break
        if opcode in (1, 2):
            _command_frame_value(value)
            word_offset += 1
            continue
        if opcode in (8, 19):
            word_offset += 1
            continue
        if opcode in (3, 4, 5, 6, 7, 9):
            word_offset += COMMON_CMD_LENGTHS[opcode]
            continue

        if opcode < 10:
            raise DatExtractError(
                f"unsupported common action script command opcode {opcode} at word {word_offset}"
            )
        fighter_index = opcode - 10
        if fighter_index >= len(FIGHTER_CMD_LENGTHS):
            raise DatExtractError(
                f"unsupported fighter action script opcode {opcode} at word {word_offset}"
            )
        length = FIGHTER_CMD_LENGTHS[fighter_index]
        if script_offset + (word_offset + length) * 4 > len(script_bytes):
            raise DatExtractError("fighter action script command extends past script bytes")
        if fighter_index == 1 and _bitfield(word, 21, 1) == 0:
            bone_indices.add(_bitfield(word, 13, 8))
        word_offset += length

    return bone_indices


def vec3_raw(dat: bytes, data_block_offset: int) -> dict[str, float]:
    start = 0x20 + data_block_offset
    return {
        "x": read_f32(dat, start),
        "y": read_f32(dat, start + 4),
        "z": read_f32(dat, start + 8),
    }


def vec3_milli(raw: dict[str, float]) -> dict[str, int]:
    return {axis: rust_round(value * 1000.0) for axis, value in raw.items()}


def _require_fobj_bytes(data: bytes, pos: int, count: int) -> None:
    if pos < 0 or pos + count > len(data):
        raise DatExtractError("FObj animation data ended mid-token")


def _parse_fobj_float(data: bytes, pos: int, frac: int) -> tuple[float, int]:
    if frac == HSD_A_FRAC_FLOAT:
        _require_fobj_bytes(data, pos, 4)
        return struct.unpack("<f", data[pos : pos + 4])[0], pos + 4

    denom = 1 << (frac & 0x1F)
    frac_kind = frac & 0xE0
    if frac_kind == HSD_A_FRAC_S8:
        _require_fobj_bytes(data, pos, 1)
        numer = int.from_bytes(data[pos : pos + 1], "little", signed=True)
        return numer / denom, pos + 1
    if frac_kind == HSD_A_FRAC_U8:
        _require_fobj_bytes(data, pos, 1)
        return data[pos] / denom, pos + 1
    if frac_kind == HSD_A_FRAC_S16:
        _require_fobj_bytes(data, pos, 2)
        numer = int.from_bytes(data[pos : pos + 2], "little", signed=True)
        return numer / denom, pos + 2
    if frac_kind == HSD_A_FRAC_U16:
        _require_fobj_bytes(data, pos, 2)
        numer = int.from_bytes(data[pos : pos + 2], "little")
        return numer / denom, pos + 2
    raise DatExtractError(f"unsupported FObj fraction byte 0x{frac:02x}")


def _parse_fobj_pack_info(data: bytes, pos: int) -> tuple[int, int]:
    _require_fobj_bytes(data, pos, 1)
    d = data[pos]
    pos += 1
    nb_pack = ((d >> 4) & 7) + 1
    shift = 3
    while d & 0x80:
        _require_fobj_bytes(data, pos, 1)
        d = data[pos]
        pos += 1
        nb_pack += (d & 0x7F) << shift
        shift += 7
    return nb_pack, pos


def _parse_fobj_wait(data: bytes, pos: int) -> tuple[int, int]:
    wait = 0
    shift = 0
    while True:
        _require_fobj_bytes(data, pos, 1)
        d = data[pos]
        pos += 1
        wait |= (d & 0x7F) << shift
        shift += 7
        if not d & 0x80:
            return wait, pos


def _spl_get_hermite(inv_fterm: float, time: float, p0: float, p1: float, d0: float, d1: float) -> float:
    time_sq = time * time
    inv_sq = inv_fterm * inv_fterm
    time_sq_inv = time_sq * inv_fterm
    inv_sq_time_cu = inv_sq * (time_sq * time)
    two_time_cu_inv_cu = 2.0 * inv_sq_time_cu * inv_fterm
    three_time_sq_inv_sq = 3.0 * time_sq * inv_sq
    return (
        d1 * (inv_sq_time_cu - time_sq_inv)
        + d0 * (time + ((inv_sq_time_cu - time_sq_inv) - time_sq_inv))
        + p0 * (1.0 + (two_time_cu_inv_cu - three_time_sq_inv_sq))
        + p1 * (-two_time_cu_inv_cu + three_time_sq_inv_sq)
    )


def sample_fobj_value(
    animation_data: bytes,
    *,
    length: int,
    startframe: int,
    frac_value: int,
    frac_slope: int,
    frame: float,
) -> float:
    """Sample one HSD FObj track using the decomp's first-play request flow."""

    data = animation_data[:length]
    pos = 0
    time = float(startframe) + frame
    flags = 0
    state = FOBJ_LOAD_DATA0
    op = 0
    op_intrp = 0
    nb_pack = 0
    fterm = 0
    p0 = 0.0
    p1 = 0.0
    d0 = 0.0
    d1 = 0.0
    carried_fterm = 0.0
    last_value: float | None = None

    def launch_key_data() -> None:
        nonlocal flags, op_intrp, p0
        if flags & 0x40:
            op_intrp = op
            flags &= ~0x40
            flags |= 0x80
            p0 = p1

    def update_anim() -> float | None:
        nonlocal flags, p0, d0
        if op_intrp == HSD_A_OP_KEY:
            if flags & 0x80:
                flags &= ~0x80
                return p0
            return None
        if op_intrp == HSD_A_OP_CON:
            return p1 if time >= fterm else p0
        if op_intrp == HSD_A_OP_LIN:
            if flags & 0x20:
                flags &= ~0x20
                if fterm != 0:
                    d0 = (p1 - p0) / fterm
                else:
                    d0 = 0.0
                    p0 = p1
            return d0 * time + p0
        if op_intrp in (HSD_A_OP_SPL0, HSD_A_OP_SPL, HSD_A_OP_SLP):
            if fterm != 0:
                return _spl_get_hermite(1.0 / fterm, time, p0, p1, d0, d1)
            return p1
        return None

    while True:
        if state in (FOBJ_LOAD_DATA0, FOBJ_LOAD_DATA):
            if pos >= len(data):
                state = 6
                continue
            load_state = state
            op_intrp = op
            if nb_pack == 0:
                op = data[pos] & 0xF
                nb_pack, pos = _parse_fobj_pack_info(data, pos)
            nb_pack -= 1

            if op == HSD_A_OP_CON:
                p0 = p1
                p1, pos = _parse_fobj_float(data, pos, frac_value)
                if op_intrp != HSD_A_OP_SLP:
                    d0 = d1
                    d1 = 0.0
            elif op == HSD_A_OP_LIN:
                p0 = p1
                p1, pos = _parse_fobj_float(data, pos, frac_value)
                if op_intrp != HSD_A_OP_SLP:
                    d0 = d1
                    d1 = 0.0
            elif op == HSD_A_OP_SPL0:
                p0 = p1
                d0 = d1
                p1, pos = _parse_fobj_float(data, pos, frac_value)
                d1 = 0.0
            elif op == HSD_A_OP_SPL:
                p0 = p1
                p1, pos = _parse_fobj_float(data, pos, frac_value)
                d0 = d1
                d1, pos = _parse_fobj_float(data, pos, frac_slope)
            elif op == HSD_A_OP_SLP:
                d0 = d1
                d1, pos = _parse_fobj_float(data, pos, frac_slope)
            elif op == HSD_A_OP_KEY:
                launch_key_data()
                p1, pos = _parse_fobj_float(data, pos, frac_value)
                flags |= 0x40
            else:
                raise DatExtractError(f"unsupported FObj op {op}")
            if op == HSD_A_OP_SLP:
                state = load_state
            else:
                state = FOBJ_LOAD_WAIT if load_state == FOBJ_LOAD_DATA0 else 4
            continue

        if state == FOBJ_LOAD_WAIT:
            if flags & 0x80:
                updated = update_anim()
                if updated is not None:
                    last_value = updated
            if pos >= len(data):
                state = 6
            else:
                fterm, pos = _parse_fobj_wait(data, pos)
                flags |= 0x20
                state = FOBJ_LOAD_DATA
            continue

        if state == 4:
            if fterm <= time:
                carried_fterm = float(fterm)
                time -= fterm
                state = FOBJ_LOAD_WAIT
                continue
            updated = update_anim()
            if updated is not None:
                return updated
            state = 5
            continue

        if state == 5:
            state = 4
            continue

        if state == 6:
            time += carried_fterm
            launch_key_data()
            updated = update_anim()
            if updated is not None:
                return updated
            if last_value is not None:
                return last_value
            raise DatExtractError("FObj track produced no sampled value")

        if state == 0:
            if last_value is None:
                raise DatExtractError("FObj track stopped before producing a sampled value")
            return last_value


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


def extract_character_profile_from_dat(
    dat: bytes, source_path: Path, spec: CharacterResourceSpec
) -> dict[str, object]:
    symbol, ftdata_offset = find_root(
        dat, lambda name: name == spec.ft_data_symbol, spec.ft_data_symbol
    )
    header_offset = 0x20 + ftdata_offset
    attrs_offset = read_u32(dat, header_offset)
    data_block_size, _relocation_count, _root_count, _external_count = dat_header_counts(dat)
    attrs_end = read_u32(dat, header_offset + 4)
    if attrs_end <= attrs_offset or attrs_end > data_block_size:
        attrs_end = data_block_size
    required_len = attrs_offset + max(field.offset for field in PROFILE_FIELDS) + 4
    if required_len > attrs_end:
        raise DatExtractError(
            f"{spec.ft_data_symbol}.x0 does not cover required ftCo_DatAttrs fields"
        )
    attrs = dat[0x20 + attrs_offset : 0x20 + attrs_end]
    return {
        "source": {
            "file": source_path_for_json(source_path),
            "symbol": symbol,
            "ft_data_offset": ftdata_offset,
            "ftco_dat_attrs_offset": attrs_offset,
            "ftco_dat_attrs_len": attrs_end - attrs_offset,
            "format": "HSD DAT, big-endian floats/ints",
            "source_character": spec.id,
        },
        "fields": {field.rust_name: field_value(attrs, field) for field in PROFILE_FIELDS},
    }


def extract_captain_profile_from_plca(dat: bytes, source_path: Path) -> dict[str, object]:
    return extract_character_profile_from_dat(dat, source_path, character_resource_spec("captain"))


def extract_character_ecb_source_from_dat(
    dat: bytes, source_path: Path, spec: CharacterResourceSpec
) -> dict[str, object]:
    symbol, ftdata_offset = find_root(
        dat, lambda name: name == spec.ft_data_symbol, spec.ft_data_symbol
    )
    header_offset = 0x20 + ftdata_offset
    ecb_source_offset = read_u32(dat, header_offset + 0x44)
    data_block_size, _relocation_count, _root_count, _external_count = dat_header_counts(dat)
    required_len = ecb_source_offset + 0x1C
    if ecb_source_offset == 0 or required_len > data_block_size:
        raise DatExtractError(
            f"{spec.ft_data_symbol}.x44 does not cover ftData_x44_t ECB source data"
        )

    block_offset = 0x20 + ecb_source_offset
    joint_indices = [read_i16(dat, block_offset + index * 2) for index in range(6)]
    side_midpoint_offset = read_f32(dat, block_offset + 0x0C)
    ledge_snap_x = read_f32(dat, block_offset + 0x10)
    ledge_snap_y = read_f32(dat, block_offset + 0x14)
    ledge_snap_height = read_f32(dat, block_offset + 0x18)
    action_animation_path = source_path.with_name(spec.action_dat)

    return {
        "source": {
            "file": source_path_for_json(source_path),
            "symbol": symbol,
            "ft_data_offset": ftdata_offset,
            "ftdata_ecb_source_offset": ecb_source_offset,
            "format": "HSD DAT, big-endian ftData_x44_t",
            "source_character": spec.id,
        },
        "ecb_source": {
            "joint_indices": joint_indices,
            "side_midpoint_offset_raw": side_midpoint_offset,
            "side_midpoint_offset_milli": rust_round(side_midpoint_offset * 1000.0),
            "ledge_snap_x_raw": ledge_snap_x,
            "ledge_snap_x_milli": rust_round(ledge_snap_x * 1000.0),
            "ledge_snap_y_raw": ledge_snap_y,
            "ledge_snap_y_milli": rust_round(ledge_snap_y * 1000.0),
            "ledge_snap_height_raw": ledge_snap_height,
            "ledge_snap_height_milli": rust_round(ledge_snap_height * 1000.0),
        },
        "animation_dependency": {
            "file": source_path_for_json(action_animation_path),
            "status": (
                "available_for_per_frame_ecb"
                if action_animation_path.exists()
                else "missing_required_for_per_frame_ecb"
            ),
            "reason": (
                f"{spec.data_dat} points at action figatrees, but per-frame JObj ECB "
                f"requires the {spec.id} action animation resource."
            ),
        },
    }


def extract_captain_ecb_source_from_plca(dat: bytes, source_path: Path) -> dict[str, object]:
    return extract_character_ecb_source_from_dat(dat, source_path, character_resource_spec("captain"))


def extract_captain_hurtbox_inits_from_plca(
    dat: bytes, source_path: Path
) -> dict[str, object]:
    return extract_character_hurtbox_inits_from_dat(
        dat, source_path, character_resource_spec("captain")
    )


def extract_character_hurtbox_inits_from_dat(
    dat: bytes, source_path: Path, spec: CharacterResourceSpec
) -> dict[str, object]:
    symbol, ftdata_offset = find_root(
        dat, lambda name: name == spec.ft_data_symbol, spec.ft_data_symbol
    )
    header_offset = 0x20 + ftdata_offset
    hurtbox_table_offset = read_u32(dat, header_offset + 0x30)
    data_block_size, _relocation_count, _root_count, _external_count = dat_header_counts(dat)
    if hurtbox_table_offset == 0 or hurtbox_table_offset + 8 > data_block_size:
        raise DatExtractError(
            f"{spec.ft_data_symbol}.x30 does not cover ftData_x30 hurtbox init metadata"
        )

    table = 0x20 + hurtbox_table_offset
    count = read_i32(dat, table)
    inits_offset = read_u32(dat, table + 0x04)
    if count < 0 or count > 15:
        raise DatExtractError(
            f"{spec.id} hurt capsule count {count} is outside Fighter.hurt_capsules[15]"
        )
    if inits_offset == 0 or inits_offset + count * FT_HURTBOX_INIT_SIZE > data_block_size:
        raise DatExtractError(
            f"{spec.id} hurt capsule init records are outside {spec.data_dat} data block"
        )

    hurtboxes: list[dict[str, object]] = []
    for index in range(count):
        record_offset = inits_offset + index * FT_HURTBOX_INIT_SIZE
        record = 0x20 + record_offset
        a_offset_raw = vec3_raw(dat, record_offset + 0x0C)
        b_offset_raw = vec3_raw(dat, record_offset + 0x18)
        scale_raw = read_f32(dat, record + 0x24)
        hurtboxes.append(
            {
                "id": index,
                "bone_idx": read_u32(dat, record),
                "height": read_u32(dat, record + 0x04),
                "is_grabbable": read_u32(dat, record + 0x08) != 0,
                "a_offset_raw": a_offset_raw,
                "a_offset_milli": vec3_milli(a_offset_raw),
                "b_offset_raw": b_offset_raw,
                "b_offset_milli": vec3_milli(b_offset_raw),
                "scale_raw": scale_raw,
                "scale_milli": rust_round(scale_raw * 1000.0),
            }
        )

    return {
        "source": {
            "file": source_path_for_json(source_path),
            "symbol": symbol,
            "ft_data_offset": ftdata_offset,
            "ftdata_hurtbox_table_offset": hurtbox_table_offset,
            "hurtbox_inits_offset": inits_offset,
            "format": "HSD DAT, big-endian ftData.x30 hurt capsule init table",
            "source_character": spec.id,
        },
        "count": count,
        "hurtboxes": hurtboxes,
    }


def extract_captain_costume_skeleton_from_plcanr(
    dat: bytes, source_path: Path
) -> dict[str, object]:
    return extract_character_costume_skeleton_from_dat(
        dat, source_path, character_resource_spec("captain")
    )


def extract_character_costume_skeleton_from_dat(
    dat: bytes, source_path: Path, spec: CharacterResourceSpec
) -> dict[str, object]:
    symbol, root_offset = find_root(
        dat, lambda name: name == spec.neutral_joint_root, f"{spec.id} neutral costume joint"
    )
    data_block_size, _relocation_count, _root_count, _external_count = dat_header_counts(dat)
    if root_offset == 0 or root_offset + HSD_JOINT_SIZE > data_block_size:
        raise DatExtractError(f"{spec.neutral_joint_root} root is outside the data block")

    joints: list[dict[str, object]] = []
    visited: set[int] = set()

    def visit(offset: int, parent_index: int | None, depth: int) -> None:
        if offset == 0:
            return
        if offset in visited:
            raise DatExtractError(f"HSD_Joint tree repeats offset 0x{offset:x}")
        if offset + HSD_JOINT_SIZE > data_block_size:
            raise DatExtractError(f"HSD_Joint at 0x{offset:x} is outside the data block")
        visited.add(offset)

        joint = 0x20 + offset
        flags = read_u32(dat, joint + 0x04)
        child_offset = read_u32(dat, joint + 0x08)
        next_offset = read_u32(dat, joint + 0x0C)
        rotation_raw = vec3_raw(dat, offset + 0x14)
        scale_raw = vec3_raw(dat, offset + 0x20)
        position_raw = vec3_raw(dat, offset + 0x2C)
        index = len(joints)
        joints.append(
            {
                "index": index,
                "offset": offset,
                "parent_index": parent_index,
                "depth": depth,
                "flags_raw": f"0x{flags:08x}",
                "child_offset": child_offset,
                "next_offset": next_offset,
                "rotation_raw": rotation_raw,
                "rotation_milli": vec3_milli(rotation_raw),
                "scale_raw": scale_raw,
                "scale_milli": vec3_milli(scale_raw),
                "position_raw": position_raw,
                "position_milli": vec3_milli(position_raw),
            }
        )

        if child_offset != 0 and (flags & JOBJ_INSTANCE) == 0:
            visit(child_offset, index, depth + 1)
        if next_offset != 0:
            visit(next_offset, parent_index, depth)

    visit(root_offset, None, 0)

    return {
        "source": {
            "file": source_path_for_json(source_path),
            "root": symbol,
            "root_offset": root_offset,
            "joint_count": len(joints),
            "format": f"HSD_Joint preorder tree from {spec.id} neutral costume DAT",
            "source_character": spec.id,
        },
        "joints": joints,
    }


def data_block_string(dat: bytes, data_block_offset: int) -> str:
    start = 0x20 + data_block_offset
    if start < 0x20 or start >= len(dat):
        raise DatExtractError(f"data-block string offset 0x{data_block_offset:x} is outside DAT")
    try:
        end = dat.index(0, start)
    except ValueError as error:
        raise DatExtractError(
            f"data-block string at 0x{data_block_offset:x} is unterminated"
        ) from error
    return dat[start:end].decode("ascii")


def figatree_root_for_chunk(plcaaj: bytes, offset: int, size: int) -> str | None:
    if offset == 0 and size == 0:
        return None
    if offset < 0 or size < 0 or offset + size > len(plcaaj):
        raise DatExtractError(
            f"PlCaAJ.dat figatree chunk 0x{offset:x}..0x{offset + size:x} is outside the file"
        )
    chunk = plcaaj[offset : offset + size]
    roots = dat_roots(chunk)
    if len(roots) != 1:
        raise DatExtractError(
            f"PlCaAJ.dat figatree chunk at 0x{offset:x} has {len(roots)} roots"
        )
    return next(iter(roots.keys()))


def extract_figatree_summary(figatree_chunk: bytes) -> dict[str, object]:
    data_block_size, _relocation_count, _root_count, _external_count = dat_header_counts(
        figatree_chunk
    )
    roots = dat_roots(figatree_chunk)
    if len(roots) != 1:
        raise DatExtractError(f"figatree chunk has {len(roots)} roots")
    root_name, root_offset = next(iter(roots.items()))
    if root_offset == 0 or root_offset + FIGA_TREE_SIZE > data_block_size:
        raise DatExtractError("FigaTree root is outside the data block")

    root = 0x20 + root_offset
    tree_type = read_i32(figatree_chunk, root)
    flags = read_u32(figatree_chunk, root + 0x04)
    frames = read_f32(figatree_chunk, root + 0x08)
    nodes_offset = read_u32(figatree_chunk, root + 0x0C)
    tracks_offset = read_u32(figatree_chunk, root + 0x10)
    if nodes_offset >= data_block_size:
        raise DatExtractError("FigaTree nodes pointer is outside the data block")
    if tracks_offset >= data_block_size:
        raise DatExtractError("FigaTree tracks pointer is outside the data block")

    track_counts_by_node: list[int] = []
    node_pos = 0x20 + nodes_offset
    while True:
        if node_pos >= 0x20 + data_block_size:
            raise DatExtractError("FigaTree nodes list is unterminated")
        value = read_i8(figatree_chunk, node_pos)
        node_pos += 1
        if value == -1:
            break
        if value < 0:
            raise DatExtractError(f"FigaTree node track count {value} is invalid")
        track_counts_by_node.append(value)

    track_count = sum(track_counts_by_node)
    track_table_end = tracks_offset + track_count * FIGA_TRACK_SIZE
    if track_table_end > data_block_size:
        raise DatExtractError("FigaTree track table is outside the data block")

    tracks: list[dict[str, object]] = []
    for index in range(track_count):
        track = 0x20 + tracks_offset + index * FIGA_TRACK_SIZE
        data_offset = read_u32(figatree_chunk, track + 0x08)
        length = read_u16(figatree_chunk, track)
        if data_offset + length > data_block_size:
            raise DatExtractError(
                f"FigaTrack {index} data range 0x{data_offset:x}..0x{data_offset + length:x} "
                "is outside the data block"
            )
        tracks.append(
            {
                "index": index,
                "length": length,
                "startframe": read_u16(figatree_chunk, track + 0x02),
                "obj_type": figatree_chunk[track + 0x04],
                "frac_value_raw": f"0x{figatree_chunk[track + 0x05]:02x}",
                "frac_slope_raw": f"0x{figatree_chunk[track + 0x06]:02x}",
                "data_offset": data_offset,
            }
        )

    return {
        "root": root_name,
        "type": tree_type,
        "flags_raw": f"0x{flags:08x}",
        "frames_raw": frames,
        "frames_ticks": rust_round(frames),
        "nodes_offset": nodes_offset,
        "tracks_offset": tracks_offset,
        "track_counts_by_node": track_counts_by_node,
        "animated_node_count": len(track_counts_by_node),
        "track_count": track_count,
        "tracks": tracks,
    }


def sample_figatree_node_tracks(
    figatree_chunk: bytes, *, node_index: int, frame: float
) -> dict[str, object]:
    """Sample SRT animation tracks for one FigaTree node at a frame."""

    summary = extract_figatree_summary(figatree_chunk)
    track_counts = summary["track_counts_by_node"]
    if not isinstance(track_counts, list):
        raise DatExtractError("FigaTree track count metadata is malformed")
    if node_index < 0 or node_index >= len(track_counts):
        raise DatExtractError(f"FigaTree node index {node_index} is outside the nodes list")

    tracks = summary["tracks"]
    if not isinstance(tracks, list):
        raise DatExtractError("FigaTree track metadata is malformed")

    track_start = sum(int(count) for count in track_counts[:node_index])
    track_end = track_start + int(track_counts[node_index])
    sampled: dict[str, object] = {
        "node_index": node_index,
        "frame": frame,
        "rotation": {},
        "translation": {},
        "scale": {},
        "tracks": [],
    }

    sampled_tracks: list[dict[str, object]] = []
    for track in tracks[track_start:track_end]:
        if not isinstance(track, dict):
            raise DatExtractError("FigaTree track metadata is malformed")
        frac_value = int(str(track["frac_value_raw"]), 16)
        frac_slope = int(str(track["frac_slope_raw"]), 16)
        data_offset = int(track["data_offset"])
        length = int(track["length"])
        value = sample_fobj_value(
            figatree_chunk[0x20 + data_offset : 0x20 + data_offset + length],
            length=length,
            startframe=int(track["startframe"]),
            frac_value=frac_value,
            frac_slope=frac_slope,
            frame=frame,
        )
        obj_type = int(track["obj_type"])
        channel = JOBJ_ANIM_CHANNELS.get(obj_type)
        sampled_track = {
            "index": int(track["index"]),
            "obj_type": obj_type,
            "value": value,
        }
        if channel is None:
            sampled_track["channel"] = "unsupported"
        else:
            transform, axis = channel
            sampled_track["channel"] = f"{transform}.{axis}"
            sampled_transform = sampled[transform]
            if not isinstance(sampled_transform, dict):
                raise DatExtractError("FigaTree sampled transform bucket is malformed")
            sampled_transform[axis] = value
        sampled_tracks.append(sampled_track)

    sampled["tracks"] = sampled_tracks
    return sampled


def _vec3_from_mapping(mapping: dict[str, object], defaults: tuple[float, float, float]) -> dict[str, float]:
    return {
        "x": float(mapping.get("x", defaults[0])),
        "y": float(mapping.get("y", defaults[1])),
        "z": float(mapping.get("z", defaults[2])),
    }


def _matrix_srt(
    scale: dict[str, float],
    rotation: dict[str, float],
    translation: dict[str, float],
    parent_scale: dict[str, float] | None,
) -> list[list[float]]:
    sin_x = math.sin(rotation["x"])
    cos_x = math.cos(rotation["x"])
    sin_y = math.sin(rotation["y"])
    cos_y = math.cos(rotation["y"])
    sin_z = math.sin(rotation["z"])
    cos_z = math.cos(rotation["z"])

    scale_x2 = scale_x1 = scale_x = scale["x"]
    scale_y2 = scale_y1 = scale_y = scale["y"]
    scale_z2 = scale_z1 = scale_z = scale["z"]

    if parent_scale is not None:
        scale_y2 *= parent_scale["y"] / parent_scale["x"]
        scale_z2 *= parent_scale["z"] / parent_scale["x"]
        scale_x1 *= parent_scale["x"] / parent_scale["y"]
        scale_z1 *= parent_scale["z"] / parent_scale["y"]
        scale_x *= parent_scale["x"] / parent_scale["z"]
        scale_y *= parent_scale["y"] / parent_scale["z"]

    return [
        [
            cos_z * (scale_x2 * cos_y),
            scale_y2 * ((cos_z * (sin_x * sin_y)) - (cos_x * sin_z)),
            scale_z2 * ((cos_z * (cos_x * sin_y)) + (sin_x * sin_z)),
            translation["x"],
        ],
        [
            sin_z * (scale_x1 * cos_y),
            scale_y1 * ((sin_z * (sin_x * sin_y)) + (cos_x * cos_z)),
            scale_z1 * ((sin_z * (cos_x * sin_y)) - (sin_x * cos_z)),
            translation["y"],
        ],
        [
            -scale_x * sin_y,
            cos_y * (scale_y * sin_x),
            cos_y * (scale_z * cos_x),
            translation["z"],
        ],
    ]


def _matrix_concat(parent: list[list[float]], child: list[list[float]]) -> list[list[float]]:
    result: list[list[float]] = []
    for row in range(3):
        out_row = []
        for col in range(4):
            value = (
                parent[row][0] * child[0][col]
                + parent[row][1] * child[1][col]
                + parent[row][2] * child[2][col]
            )
            if col == 3:
                value += parent[row][3]
            out_row.append(value)
        result.append(out_row)
    return result


def _matrix_translation(matrix: list[list[float]]) -> dict[str, float]:
    return {"x": matrix[0][3], "y": matrix[1][3], "z": matrix[2][3]}


def _matrix_transform_point(
    matrix: list[list[float]], point: dict[str, object]
) -> dict[str, float]:
    x = float(point["x"])
    y = float(point["y"])
    z = float(point["z"])
    return {
        "x": matrix[0][0] * x + matrix[0][1] * y + matrix[0][2] * z + matrix[0][3],
        "y": matrix[1][0] * x + matrix[1][1] * y + matrix[1][2] * z + matrix[1][3],
        "z": matrix[2][0] * x + matrix[2][1] * y + matrix[2][2] * z + matrix[2][3],
    }


MELEE_RIGHT_FACING_RENDER_TRANSFORM = "ftPartSetRotY(TopN, M_PI_2 * fp->facing_dir)"
MELEE_RIGHT_FACING_FLATTEN_POLICY = "right_facing_melee_xy"
MELEE_HURTBOX_SAMPLE_METADATA = {
    "source_init_handler": "ftColl_8007B3A0/ftColl_8007B4E0 ftData.x30",
    "source_update_handler": "lbColl_800083C4/lbColl_8000A244/lbColl_8000A584",
    "source_draw_handler": "ftDrawCommon_800805C8 -> lbColl_8000A244/lbColl_8000A584 -> lbColl_DrawHitResult",
    "source_render_endpoints": "HurtCapsule.a_pos -> HurtCapsule.b_pos",
    "source_render_radius": "HurtCapsule.scale",
    "source_render_transform": MELEE_RIGHT_FACING_RENDER_TRANSFORM,
    "flatten_after_render": MELEE_RIGHT_FACING_FLATTEN_POLICY,
    "source_color_table": "lbColl_803B9928[hurt->state]",
    "source_skip_update_pos_after_transform": True,
    "source_z_policy": "preserve JObj-transformed z; debug render may force fighter->cur_pos.z when ftCommon_8007F804 returns non-null",
    "source": "ftData.x30 + PlCaAJ FigaTree + PlCaNr JObj skeleton",
    "confidence": "source_extracted",
}


def _flatten_z(point: dict[str, float]) -> dict[str, float]:
    return {"x": point["x"], "y": point["y"], "z": 0.0}


def _flatten_right_facing_melee_render(point: dict[str, float]) -> dict[str, float]:
    return {"x": point["z"], "y": point["y"], "z": 0.0}


def _vec2_milli(raw: dict[str, float]) -> dict[str, int]:
    return {axis: rust_round(value * 1000.0) for axis, value in raw.items()}


def sample_figatree_skeleton_pose(
    figatree_chunk: bytes, skeleton: dict[str, object], *, frame: float
) -> dict[str, object]:
    """Apply a FigaTree to an extracted HSD_Joint skeleton and return world positions."""

    joints = skeleton.get("joints")
    if not isinstance(joints, list):
        raise DatExtractError("skeleton does not contain a joints list")

    summary = extract_figatree_summary(figatree_chunk)
    track_counts = summary["track_counts_by_node"]
    if not isinstance(track_counts, list):
        raise DatExtractError("FigaTree track count metadata is malformed")

    matrices: list[list[list[float]]] = []
    global_scales: list[dict[str, float] | None] = []
    sampled_joints: list[dict[str, object]] = []
    for index, joint in enumerate(joints):
        if not isinstance(joint, dict):
            raise DatExtractError("skeleton joint metadata is malformed")
        rotation_raw = joint.get("rotation_raw")
        scale_raw = joint.get("scale_raw")
        position_raw = joint.get("position_raw")
        if not isinstance(rotation_raw, dict) or not isinstance(scale_raw, dict) or not isinstance(position_raw, dict):
            raise DatExtractError("skeleton joint SRT metadata is malformed")

        rotation = _vec3_from_mapping(rotation_raw, (0.0, 0.0, 0.0))
        scale = _vec3_from_mapping(scale_raw, (1.0, 1.0, 1.0))
        translation = _vec3_from_mapping(position_raw, (0.0, 0.0, 0.0))
        tracks: list[dict[str, object]] = []
        if index < len(track_counts):
            sampled = sample_figatree_node_tracks(figatree_chunk, node_index=index, frame=frame)
            for transform_name, target in (
                ("rotation", rotation),
                ("translation", translation),
                ("scale", scale),
            ):
                transform = sampled[transform_name]
                if not isinstance(transform, dict):
                    raise DatExtractError("sampled FigaTree transform metadata is malformed")
                for axis, value in transform.items():
                    target[str(axis)] = float(value)
            raw_tracks = sampled["tracks"]
            if not isinstance(raw_tracks, list):
                raise DatExtractError("sampled FigaTree track metadata is malformed")
            tracks = raw_tracks

        parent_index = joint.get("parent_index")
        parent_scale = None
        parent_matrix = None
        if parent_index is not None:
            parent_int = int(parent_index)
            parent_scale = global_scales[parent_int]
            parent_matrix = matrices[parent_int]

        flags = int(str(joint["flags_raw"]), 16)
        if flags & 8:
            global_scale = dict(parent_scale) if parent_scale is not None else None
        elif parent_scale is None:
            global_scale = dict(scale)
        else:
            global_scale = {
                "x": scale["x"] * parent_scale["x"],
                "y": scale["y"] * parent_scale["y"],
                "z": scale["z"] * parent_scale["z"],
            }

        local_matrix = _matrix_srt(scale, rotation, translation, parent_scale)
        matrix = _matrix_concat(parent_matrix, local_matrix) if parent_matrix is not None else local_matrix
        matrices.append(matrix)
        global_scales.append(global_scale)
        world_position = _matrix_translation(matrix)
        sampled_joints.append(
            {
                "index": index,
                "parent_index": parent_index,
                "rotation": rotation,
                "translation": translation,
                "scale": scale,
                "world_matrix": matrix,
                "world_position_raw": world_position,
                "world_position_milli": vec3_milli(world_position),
                "tracks": tracks,
            }
        )

    return {
        "frame": frame,
        "source": "HSD_JObj FigaTree sampled pose",
        "joints": sampled_joints,
    }


def compute_ecb_from_jobj_pose(
    pose: dict[str, object],
    ecb_source: dict[str, object],
    *,
    flags: int,
    cur_pos: dict[str, float] | None = None,
) -> dict[str, object]:
    """Reduce sampled JObj source points with Melee's mpColl_LoadECB_JObj rules."""

    joints = pose.get("joints")
    indices = ecb_source.get("joint_indices")
    if not isinstance(joints, list) or not isinstance(indices, list) or len(indices) != 6:
        raise DatExtractError("ECB JObj pose requires six source joint indices")

    position = cur_pos if cur_pos is not None else {"x": 0.0, "y": 0.0}
    points: list[dict[str, float]] = []
    source_points: list[dict[str, float]] = []
    for index in indices:
        joint = joints[int(index)]
        if not isinstance(joint, dict):
            raise DatExtractError("pose joint metadata is malformed")
        raw_position = joint.get("world_position_raw")
        if not isinstance(raw_position, dict):
            raise DatExtractError("pose joint does not contain a world position")
        source_point = {
            "x": float(raw_position["x"]) - float(position.get("x", 0.0)),
            "y": float(raw_position["y"]) - float(position.get("y", 0.0)),
            "z": float(raw_position["z"]),
        }
        rendered = _flatten_right_facing_melee_render(source_point)
        source_points.append(source_point)
        points.append({"x": rendered["x"], "y": rendered["y"]})

    left_x = right_x = points[0]["x"]
    bottom_y = top_y = points[0]["y"]
    for point in points[1:]:
        left_x = min(left_x, point["x"])
        right_x = max(right_x, point["x"])
        bottom_y = min(bottom_y, point["y"])
        top_y = max(top_y, point["y"])

    if not (flags & 0b100):
        left_x -= 2.0
        right_x += 2.0
        bottom_y -= 2.0
        top_y += 2.0

    min_width = 10.0
    width = abs(right_x - left_x)
    if width < min_width:
        right_x = 0.5 * width
        left_x = -right_x

    min_height = 10.0
    height = abs(top_y - bottom_y)
    if height < min_height:
        half_height = 0.5 * height
        mid_y = 0.5 * (top_y + bottom_y)
        top_y = mid_y + half_height
        bottom_y = mid_y - half_height

    if flags & 0b1000:
        left_x = -1.0
        right_x = 1.0
    else:
        right_x = 2.0 if right_x < 2.0 else right_x
        left_x = -2.0 if left_x > -2.0 else left_x

    if flags & 1:
        bottom_y = 0.0
        if flags & 0b10000:
            top_y = 2.0
    else:
        if bottom_y < 0.0:
            bottom_y = 0.0
        if flags & 0b10000:
            mid_y = 0.5 * (bottom_y + top_y)
            bottom_y = mid_y - 1.0
            top_y = mid_y + 1.0
            if bottom_y < 0.0:
                bottom_y = 0.0
                top_y = 2.0

    midpoint_y = float(ecb_source.get("side_midpoint_offset_raw", 0.0)) + 0.5 * (
        bottom_y + top_y
    )
    result = {
        "top": {"x": 0.0, "y": top_y},
        "bottom": {"x": 0.0, "y": bottom_y},
        "right": {"x": right_x, "y": midpoint_y},
        "left": {"x": left_x, "y": midpoint_y},
        "source_points": source_points,
        "flags": flags,
        "source_render_transform": MELEE_RIGHT_FACING_RENDER_TRANSFORM,
        "flatten_after_render": MELEE_RIGHT_FACING_FLATTEN_POLICY,
    }
    result["top_milli"] = _vec2_milli(result["top"])
    result["bottom_milli"] = _vec2_milli(result["bottom"])
    result["right_milli"] = _vec2_milli(result["right"])
    result["left_milli"] = _vec2_milli(result["left"])
    return result


def sample_hurtboxes_from_pose(
    pose: dict[str, object],
    hurtbox_inits: dict[str, object],
) -> list[dict[str, object]]:
    joints = pose.get("joints")
    hurtboxes = hurtbox_inits.get("hurtboxes")
    if not isinstance(joints, list) or not isinstance(hurtboxes, list):
        raise DatExtractError(
            "hurtbox sampling requires pose joints and static hurtbox init records"
        )

    sampled: list[dict[str, object]] = []
    for init in hurtboxes:
        if not isinstance(init, dict):
            raise DatExtractError("hurtbox init record is malformed")
        bone_idx = int(init["bone_idx"])
        joint = joints[bone_idx]
        if not isinstance(joint, dict):
            raise DatExtractError(f"pose joint {bone_idx} metadata is malformed")
        matrix = joint.get("world_matrix")
        if (
            not isinstance(matrix, list)
            or len(matrix) != 3
            or any(not isinstance(row, list) or len(row) != 4 for row in matrix)
        ):
            raise DatExtractError(f"pose joint {bone_idx} does not contain a world matrix")
        a_offset = init.get("a_offset_raw")
        b_offset = init.get("b_offset_raw")
        if not isinstance(a_offset, dict) or not isinstance(b_offset, dict):
            raise DatExtractError("hurtbox init record does not contain endpoint offsets")

        source_a = _matrix_transform_point(matrix, a_offset)
        source_b = _matrix_transform_point(matrix, b_offset)
        flat_a = _flatten_right_facing_melee_render(source_a)
        flat_b = _flatten_right_facing_melee_render(source_b)
        sampled.append(
            {
                "id": init["id"],
                "kind": "capsule",
                "bone": bone_idx,
                "height": init["height"],
                "is_grabbable": init["is_grabbable"],
                "a": flat_a,
                "b": flat_b,
                "source_a": source_a,
                "source_b": source_b,
                "a_offset": a_offset,
                "b_offset": b_offset,
                "radius": init["scale_raw"],
                "scale": init["scale_raw"],
                "state": "HurtCapsule_Enabled",
            }
        )
    return sampled


def compact_pose_for_collision(
    pose: dict[str, object], required_joint_indices: set[int] | None = None
) -> dict[str, object]:
    joints = pose.get("joints")
    if not isinstance(joints, list):
        raise DatExtractError("collision pose sampling requires pose joints")
    compact_joints: list[dict[str, object]] = []
    for joint in joints:
        if not isinstance(joint, dict):
            raise DatExtractError("pose joint metadata is malformed")
        index = int(joint["index"])
        if required_joint_indices is not None and index not in required_joint_indices:
            continue
        compact_joints.append(
            {
                "index": index,
                "world_matrix": joint["world_matrix"],
            }
        )
    return {
        "source": pose.get("source", "HSD_JObj FigaTree sampled pose"),
        "joints": compact_joints,
    }


def extract_captain_action_animation_table(
    plca: bytes,
    plca_source_path: Path,
    plcaaj: bytes,
    plcaaj_source_path: Path,
    action_count: int = CAPTAIN_ACTION_COUNT,
) -> dict[str, object]:
    return extract_character_action_animation_table(
        plca,
        plca_source_path,
        plcaaj,
        plcaaj_source_path,
        character_resource_spec("captain"),
        action_count=action_count,
    )


def extract_character_action_animation_table(
    fighter_dat: bytes,
    fighter_source_path: Path,
    action_dat: bytes,
    action_source_path: Path,
    spec: CharacterResourceSpec,
    action_count: int | None = None,
) -> dict[str, object]:
    action_count = spec.action_count if action_count is None else action_count
    symbol, ftdata_offset = find_root(
        fighter_dat, lambda name: name == spec.ft_data_symbol, spec.ft_data_symbol
    )
    header_offset = 0x20 + ftdata_offset
    action_table_offset = read_u32(fighter_dat, header_offset + 0x0C)
    data_block_size, _relocation_count, _root_count, _external_count = dat_header_counts(
        fighter_dat
    )
    table_end = action_table_offset + action_count * FIGHTER_WAIT_ANIM_DATA_SIZE
    if action_table_offset == 0 or table_end > data_block_size:
        raise DatExtractError(
            f"{spec.ft_data_symbol}.xC does not cover {spec.id} action animation records"
        )

    actions: list[dict[str, object]] = []
    for action_state_id in range(action_count):
        entry = 0x20 + action_table_offset + action_state_id * FIGHTER_WAIT_ANIM_DATA_SIZE
        name_offset = read_u32(fighter_dat, entry)
        figatree_archive_offset = read_u32(fighter_dat, entry + 0x04)
        figatree_archive_size = read_u32(fighter_dat, entry + 0x08)
        subaction_script_offset = read_u32(fighter_dat, entry + 0x0C)
        flags = read_u32(fighter_dat, entry + 0x10)
        runtime_archive_pointer = read_u32(fighter_dat, entry + 0x14)
        name = data_block_string(fighter_dat, name_offset) if name_offset != 0 else ""
        figatree_root = (
            figatree_root_for_chunk(action_dat, figatree_archive_offset, figatree_archive_size)
            if figatree_archive_size != 0
            else None
        )
        figatree = (
            extract_figatree_summary(
                action_dat[
                    figatree_archive_offset : figatree_archive_offset
                    + figatree_archive_size
                ]
            )
            if figatree_root is not None
            else None
        )
        actions.append(
            {
                "action_state_id": action_state_id,
                "name": name,
                "figatree_archive_offset": figatree_archive_offset,
                "figatree_archive_size": figatree_archive_size,
                "figatree_root": figatree_root,
                "figatree": figatree,
                "subaction_script_offset": subaction_script_offset,
                "cmd_var_events": (
                    extract_action_script_cmd_var_events(
                        fighter_dat, 0x20 + subaction_script_offset
                    )
                    if subaction_script_offset != 0
                    else []
                ),
                "flags_raw": f"0x{flags:08x}",
                "runtime_archive_pointer_raw": runtime_archive_pointer,
                "status": (
                    "available_for_jobj_sampling"
                    if figatree_root is not None
                    else "no_figatree_for_action_state"
                ),
            }
        )

    return {
        "source": {
            "fighter_file": source_path_for_json(fighter_source_path),
            "action_file": source_path_for_json(action_source_path),
            "plca_file": source_path_for_json(fighter_source_path),
            "plcaaj_file": source_path_for_json(action_source_path),
            "symbol": symbol,
            "ft_data_offset": ftdata_offset,
            "action_table_offset": action_table_offset,
            "action_count": action_count,
            "format": (
                f"{spec.data_dat} Fighter_WaitAnimData records pointing into "
                f"{spec.action_dat} HSD figatree chunks"
            ),
            "source_character": spec.id,
        },
        "actions": actions,
    }


def extract_captain_action_ecb_samples(
    plcaaj: bytes,
    action_table: dict[str, object],
    skeleton: dict[str, object],
    ecb_source_snapshot: dict[str, object],
    *,
    action_state_ids: tuple[int, ...] = ECB_SAMPLE_ACTION_STATE_IDS,
    flags: int = 6,
) -> dict[str, object]:
    ecb_source = ecb_source_snapshot.get("ecb_source", ecb_source_snapshot)
    if not isinstance(ecb_source, dict):
        raise DatExtractError("Captain ECB source snapshot is malformed")
    actions = action_table.get("actions")
    if not isinstance(actions, list):
        raise DatExtractError("Captain action animation table is malformed")

    sampled_actions: list[dict[str, object]] = []
    for action_state_id in action_state_ids:
        action = next(
            (
                item
                for item in actions
                if isinstance(item, dict) and int(item["action_state_id"]) == action_state_id
            ),
            None,
        )
        if action is None:
            raise DatExtractError(f"Captain action state {action_state_id} is missing")
        figatree = action.get("figatree")
        if not isinstance(figatree, dict):
            raise DatExtractError(f"Captain action state {action_state_id} has no FigaTree")
        offset = int(action["figatree_archive_offset"])
        size = int(action["figatree_archive_size"])
        if offset < 0 or size <= 0 or offset + size > len(plcaaj):
            raise DatExtractError(
                f"Captain action state {action_state_id} FigaTree archive range is outside PlCaAJ.dat"
            )
        chunk = plcaaj[offset : offset + size]
        frame_count = int(figatree["frames_ticks"])
        frames: list[dict[str, object]] = []
        for frame in range(frame_count):
            pose = sample_figatree_skeleton_pose(chunk, skeleton, frame=float(frame))
            ecb = compute_ecb_from_jobj_pose(pose, ecb_source, flags=flags)
            frames.append(
                {
                    "frame": frame,
                    "source_joint_indices": list(ecb_source["joint_indices"]),
                    "top_milli": ecb["top_milli"],
                    "bottom_milli": ecb["bottom_milli"],
                    "right_milli": ecb["right_milli"],
                    "left_milli": ecb["left_milli"],
                    "top_raw": ecb["top"],
                    "bottom_raw": ecb["bottom"],
                    "right_raw": ecb["right"],
                    "left_raw": ecb["left"],
                    "source_points": ecb["source_points"],
                    "source_render_transform": ecb["source_render_transform"],
                    "flatten_after_render": ecb["flatten_after_render"],
                }
            )
        sampled_actions.append(
            {
                "action_state_id": action_state_id,
                "name": action.get("name", ""),
                "figatree_root": action.get("figatree_root"),
                "flags": flags,
                "frames": frames,
            }
        )

    return {
        "source": {
            "format": "Captain Falcon per-frame ECB samples from PlCaAJ FigaTree, PlCaNr HSD_Joint skeleton, and ftDataCaptain.x44 JObj source joints",
            "mpcoll_flags": flags,
        },
        "actions": sampled_actions,
    }


def extract_captain_action_hurtbox_samples(
    plcaaj: bytes,
    action_table: dict[str, object],
    skeleton: dict[str, object],
    hurtbox_inits: dict[str, object],
    *,
    action_state_ids: tuple[int, ...] = (68,),
    action_script_bytes: bytes | None = None,
) -> dict[str, object]:
    actions = action_table.get("actions")
    if not isinstance(actions, list):
        raise DatExtractError("Captain action animation table is malformed")

    sampled_actions: list[dict[str, object]] = []
    for action_state_id in action_state_ids:
        action = next(
            (
                item
                for item in actions
                if isinstance(item, dict) and int(item["action_state_id"]) == action_state_id
            ),
            None,
        )
        if action is None:
            raise DatExtractError(f"Captain action state {action_state_id} is missing")
        figatree = action.get("figatree")
        if not isinstance(figatree, dict):
            raise DatExtractError(f"Captain action state {action_state_id} has no FigaTree")
        offset = int(action["figatree_archive_offset"])
        size = int(action["figatree_archive_size"])
        if offset < 0 or size <= 0 or offset + size > len(plcaaj):
            raise DatExtractError(
                f"Captain action state {action_state_id} FigaTree archive range is outside PlCaAJ.dat"
            )
        chunk = plcaaj[offset : offset + size]
        frame_count = int(figatree["frames_ticks"])
        hurtboxes = hurtbox_inits.get("hurtboxes")
        if not isinstance(hurtboxes, list):
            raise DatExtractError("Captain hurtbox init records are malformed")
        frames: list[dict[str, object]] = []
        for frame in range(frame_count):
            pose = sample_figatree_skeleton_pose(chunk, skeleton, frame=float(frame))
            frames.append(
                {
                    "frame": frame + 1,
                    "pose": compact_pose_for_collision(pose),
                    "hurtboxes": sample_hurtboxes_from_pose(pose, hurtbox_inits),
                }
            )
        sampled_actions.append(
            {
                "action_state_id": action_state_id,
                "name": action.get("name", ""),
                "figatree_root": action.get("figatree_root"),
                "frames": frames,
            }
        )

    return {
        "source": {
            "format": "Captain Falcon hurt capsule samples from ftData.x30, PlCaAJ FigaTree, and PlCaNr HSD_Joint skeleton",
        },
        "sample_metadata": MELEE_HURTBOX_SAMPLE_METADATA,
        "actions": sampled_actions,
    }


def write_json(path: Path, payload: dict[str, object]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def iso_files_for_characters(characters: tuple[str, ...]) -> tuple[str, ...]:
    files = ["PlCo.dat"]
    for character in characters:
        spec = character_resource_spec(character)
        files.extend([spec.data_dat, spec.action_dat, spec.neutral_costume_dat])
    return tuple(dict.fromkeys(files))


def extract_resources(
    raw_dir: Path,
    out_dir: Path,
    characters: tuple[str, ...] = ("captain",),
) -> list[Path]:
    written: list[Path] = []
    plco = raw_dir / "PlCo.dat"
    if plco.exists():
        out_path = out_dir / "plco_common_data.json"
        write_json(out_path, extract_common_data_from_plco(plco.read_bytes(), plco))
        written.append(out_path)

    for character in characters:
        spec = character_resource_spec(character)
        fighter_dat = raw_dir / spec.data_dat
        action_dat = raw_dir / spec.action_dat
        costume_dat = raw_dir / spec.neutral_costume_dat
        hurtbox_inits: dict[str, object] | None = None

        if fighter_dat.exists():
            fighter_bytes = fighter_dat.read_bytes()
            out_path = out_dir / f"{spec.output_stem}_profile.json"
            write_json(out_path, extract_character_profile_from_dat(fighter_bytes, fighter_dat, spec))
            written.append(out_path)

            out_path = out_dir / f"{spec.output_stem}_ecb_source.json"
            write_json(
                out_path, extract_character_ecb_source_from_dat(fighter_bytes, fighter_dat, spec)
            )
            written.append(out_path)

            hurtbox_inits = extract_character_hurtbox_inits_from_dat(
                fighter_bytes, fighter_dat, spec
            )
            out_path = out_dir / f"{spec.output_stem}_hurtbox_inits.json"
            write_json(out_path, hurtbox_inits)
            written.append(out_path)

            if action_dat.exists():
                out_path = out_dir / f"{spec.output_stem}_action_animation_table.json"
                write_json(
                    out_path,
                    extract_character_action_animation_table(
                        fighter_bytes,
                        fighter_dat,
                        action_dat.read_bytes(),
                        action_dat,
                        spec,
                    ),
                )
                written.append(out_path)

        if costume_dat.exists():
            out_path = out_dir / f"{spec.output_stem}_costume_skeleton.json"
            write_json(
                out_path,
                extract_character_costume_skeleton_from_dat(
                    costume_dat.read_bytes(), costume_dat, spec
                ),
            )
            written.append(out_path)

        if (
            fighter_dat.exists()
            and action_dat.exists()
            and costume_dat.exists()
            and spec.derived_sample_action_state_ids
        ):
            fighter_bytes = fighter_dat.read_bytes()
            action_bytes = action_dat.read_bytes()
            action_table = extract_character_action_animation_table(
                fighter_bytes, fighter_dat, action_bytes, action_dat, spec
            )
            skeleton = extract_character_costume_skeleton_from_dat(
                costume_dat.read_bytes(), costume_dat, spec
            )
            ecb_source = extract_character_ecb_source_from_dat(fighter_bytes, fighter_dat, spec)
            if hurtbox_inits is None:
                hurtbox_inits = extract_character_hurtbox_inits_from_dat(
                    fighter_bytes, fighter_dat, spec
                )
            out_path = out_dir / f"{spec.output_stem}_action_ecb_samples.json"
            write_json(
                out_path,
                extract_captain_action_ecb_samples(action_bytes, action_table, skeleton, ecb_source),
            )
            written.append(out_path)
    return written


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--raw-dir", type=Path, default=DEFAULT_RAW_DIR)
    parser.add_argument("--out-dir", type=Path, default=DEFAULT_OUT_DIR)
    parser.add_argument(
        "--character",
        action="append",
        default=None,
        help=(
            "Source character to extract, e.g. captain or marth. May be repeated. "
            "Defaults to captain for the current bootstrap resources."
        ),
    )
    parser.add_argument(
        "--iso",
        type=Path,
        default=None,
        help=(
            "Optional user-provided Melee ISO/GCM path. When supplied, the tool "
            "extracts PlCo.dat, PlCa.dat, PlCaAJ.dat, and PlCaNr.dat into --raw-dir before "
            "generating JSON snapshots."
        ),
    )
    args = parser.parse_args()
    characters = tuple(args.character or ("captain",))

    if args.iso is not None:
        for path in extract_raw_files_from_gamecube_iso(
            args.iso, args.raw_dir, iso_files_for_characters(characters)
        ):
            print(f"extracted {path}")

    written = extract_resources(args.raw_dir, args.out_dir, characters)
    if not written:
        print(f"No supported DAT files found in {args.raw_dir}")
        expected = ", ".join(iso_files_for_characters(characters))
        print(f"Expected user-provided files: {expected}")
        return 2
    for path in written:
        print(path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
