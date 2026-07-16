import math
import struct
from pathlib import Path

import tools.extract_melee_resources as melee_resources
from tools.extract_melee_resources import (
    CharacterResourceSpec,
    DatExtractError,
    PROJECT_ROOT,
    character_resource_spec,
    compute_ecb_from_jobj_pose,
    extract_character_action_animation_table,
    extract_character_common_parts_from_dat,
    extract_character_costume_skeleton_from_dat,
    extract_character_hurtbox_inits_from_dat,
    extract_character_profile_from_dat,
    extract_character_special_attrs_from_dat,
    extract_fighter_parts_tables_from_plco,
    extract_resources,
    extract_action_script_cmd_var_events,
    extract_action_script_hitbox_bone_indices,
    extract_captain_costume_skeleton_from_plcanr,
    extract_captain_action_animation_table,
    extract_captain_action_ecb_samples,
    extract_captain_action_hurtbox_samples,
    extract_captain_ecb_source_from_plca,
    extract_captain_hurtbox_inits_from_plca,
    extract_captain_profile_from_plca,
    extract_captain_special_attrs_from_plca,
    extract_common_data_from_plco,
    extract_figatree_summary,
    extract_file_from_gamecube_iso,
    extract_raw_files_from_gamecube_iso,
    iso_files_for_characters,
    sample_fobj_value,
    sample_figatree_node_tracks,
    sample_figatree_skeleton_pose,
    sample_hurtboxes_from_pose,
    source_create_x1a70_from_pose,
)


def be32(value):
    return value.to_bytes(4, "big")


def put_f32(data, offset, value):
    data[offset : offset + 4] = struct.pack(">f", value)


def put_i32(data, offset, value):
    data[offset : offset + 4] = struct.pack(">i", value)


def put_u32(data, offset, value):
    data[offset : offset + 4] = struct.pack(">I", value)


def hitbox_word0(*, bone, use_common_bone_ids=False):
    return (11 << 26) | (bone << 11) | ((1 if use_common_bone_ids else 0) << 10)


def make_dat(root_name, data_block, root_data_offset, relocation_offsets=()):
    header = bytearray(0x20)
    relocation_table = bytearray()
    for relocation_offset in relocation_offsets:
        relocation_table += be32(relocation_offset)
    root_table = bytearray()
    root_table += be32(root_data_offset)
    root_table += be32(0)
    string_table = root_name.encode("ascii") + b"\0"
    header[0x00:0x04] = be32(
        0x20 + len(data_block) + len(relocation_table) + len(root_table) + len(string_table)
    )
    header[0x04:0x08] = be32(len(data_block))
    header[0x08:0x0C] = be32(len(relocation_offsets))
    header[0x0C:0x10] = be32(1)
    header[0x10:0x14] = be32(0)
    return bytes(header + data_block + relocation_table + root_table + string_table)


def make_gamecube_iso_with_root_file(file_name, payload):
    iso = bytearray(0x1000)
    fst_offset = 0x500
    file_offset = 0x900
    string_table = b"\0" + file_name.encode("ascii") + b"\0"
    fst_size = 24 + len(string_table)

    iso[0x424:0x428] = be32(fst_offset)
    iso[0x428:0x42C] = be32(fst_size)
    iso[fst_offset : fst_offset + 4] = be32(0x01000000)
    iso[fst_offset + 4 : fst_offset + 8] = be32(0)
    iso[fst_offset + 8 : fst_offset + 12] = be32(2)
    iso[fst_offset + 12 : fst_offset + 16] = be32(1)
    iso[fst_offset + 16 : fst_offset + 20] = be32(file_offset)
    iso[fst_offset + 20 : fst_offset + 24] = be32(len(payload))
    iso[fst_offset + 24 : fst_offset + 24 + len(string_table)] = string_table
    iso[file_offset : file_offset + len(payload)] = payload
    return bytes(iso)


def make_plco_with_parts_table(*, kind_index, joint_to_part, part_to_joint):
    data_block = bytearray(0x800)
    root_offset = 0x40
    parts_pointer_table_offset = 0x100
    table_offset = 0x200
    joint_to_part_offset = 0x240
    part_to_joint_offset = 0x280

    data_block[root_offset + 4 * 4 : root_offset + 5 * 4] = be32(parts_pointer_table_offset)
    data_block[
        parts_pointer_table_offset + kind_index * 4 : parts_pointer_table_offset + kind_index * 4 + 4
    ] = be32(table_offset)
    data_block[table_offset : table_offset + 4] = be32(joint_to_part_offset)
    data_block[table_offset + 4 : table_offset + 8] = be32(part_to_joint_offset)
    data_block[table_offset + 8 : table_offset + 12] = be32(len(joint_to_part))
    data_block[joint_to_part_offset : joint_to_part_offset + len(joint_to_part)] = bytes(
        joint_to_part
    )
    data_block[part_to_joint_offset : part_to_joint_offset + len(part_to_joint)] = bytes(
        part_to_joint
    )
    return make_dat("ftLoadCommonData", data_block, root_offset)


def make_figatree_chunk(root_name="PlyCaptain5K_Share_ACTION_Test_figatree"):
    data_block = bytearray(0x180)
    root_offset = 0x100
    nodes_offset = 0x80
    tracks_offset = 0xC0
    anim_data_offset = 0x20

    data_block[nodes_offset : nodes_offset + 4] = bytes([2, 0, 1, 0xFF])
    data_block[root_offset : root_offset + 0x14] = (
        be32(1)
        + be32(0x20000000)
        + struct.pack(">f", 50.0)
        + be32(nodes_offset)
        + be32(tracks_offset)
    )
    data_block[tracks_offset : tracks_offset + 0x24] = (
        b"\x00\x05\x00\x00\x05\x88\x88\x00"
        + be32(anim_data_offset)
        + b"\x00\x06\x00\x02\x06\x66\x88\x00"
        + be32(anim_data_offset + 8)
        + b"\x00\x07\x00\x04\x07\x66\x66\x00"
        + be32(anim_data_offset + 0x10)
    )
    return make_dat(root_name, data_block, root_offset)


def put_vec3(data, offset, value):
    put_f32(data, offset, value[0])
    put_f32(data, offset + 4, value[1])
    put_f32(data, offset + 8, value[2])


def put_joint(
    data,
    offset,
    *,
    flags,
    child=0,
    next=0,
    rotation=(0.0, 0.0, 0.0),
    scale=(1.0, 1.0, 1.0),
    position=(0.0, 0.0, 0.0),
):
    data[offset + 0x04 : offset + 0x08] = be32(flags)
    data[offset + 0x08 : offset + 0x0C] = be32(child)
    data[offset + 0x0C : offset + 0x10] = be32(next)
    put_vec3(data, offset + 0x14, rotation)
    put_vec3(data, offset + 0x20, scale)
    put_vec3(data, offset + 0x2C, position)


def make_sampled_figatree_chunk():
    data_block = bytearray(0x180)
    root_offset = 0x100
    nodes_offset = 0x80
    tracks_offset = 0xC0
    data_block[nodes_offset : nodes_offset + 3] = bytes([2, 1, 0xFF])
    data_block[root_offset : root_offset + 0x14] = (
        be32(1)
        + be32(0)
        + struct.pack(">f", 12.0)
        + be32(nodes_offset)
        + be32(tracks_offset)
    )
    linear_0_to_10 = bytes([0x12, 0x00, 0x00, 0x0A, 0x0A, 0x00])
    linear_0_to_20 = bytes([0x12, 0x00, 0x00, 0x0A, 0x14, 0x00])
    linear_5_to_15 = bytes([0x12, 0x05, 0x00, 0x0A, 0x0F, 0x00])
    data_block[0x20 : 0x20 + len(linear_0_to_10)] = linear_0_to_10
    data_block[0x30 : 0x30 + len(linear_0_to_20)] = linear_0_to_20
    data_block[0x40 : 0x40 + len(linear_5_to_15)] = linear_5_to_15
    data_block[tracks_offset : tracks_offset + 0x24] = (
        b"\x00\x06\x00\x00\x05\x40\x40\x00"
        + be32(0x20)
        + b"\x00\x06\x00\x00\x06\x40\x40\x00"
        + be32(0x30)
        + b"\x00\x06\x00\x00\x01\x40\x40\x00"
        + be32(0x40)
    )
    return make_dat("PlyCaptain5K_Share_ACTION_Sampled_figatree", data_block, root_offset)


def make_transn_translation_figatree_chunk():
    data_block = bytearray(0x180)
    root_offset = 0x100
    nodes_offset = 0x80
    tracks_offset = 0xC0
    data_block[nodes_offset : nodes_offset + 3] = bytes([0, 1, 0xFF])
    data_block[root_offset : root_offset + 0x14] = (
        be32(1)
        + be32(0)
        + struct.pack(">f", 12.0)
        + be32(nodes_offset)
        + be32(tracks_offset)
    )
    linear_0_to_20 = bytes([0x12, 0x00, 0x00, 0x0A, 0x14, 0x00])
    data_block[0x20 : 0x20 + len(linear_0_to_20)] = linear_0_to_20
    data_block[tracks_offset : tracks_offset + 0x0C] = (
        b"\x00\x06\x00\x00\x06\x40\x40\x00" + be32(0x20)
    )
    return make_dat("PlyCaptain5K_Share_ACTION_TransN_figatree", data_block, root_offset)


def test_extract_file_from_gamecube_iso_reads_root_fst_file():
    payload = b"captain action figatree bytes"
    iso = make_gamecube_iso_with_root_file("PlCaAJ.dat", payload)

    extracted = extract_file_from_gamecube_iso(iso, "PlCaAJ.dat")

    assert extracted == payload


def test_extract_raw_files_from_gamecube_iso_writes_required_dat_files(tmp_path):
    payload = b"captain action figatree bytes"
    iso_path = tmp_path / "melee.iso"
    raw_dir = tmp_path / "raw"
    iso_path.write_bytes(make_gamecube_iso_with_root_file("PlCaAJ.dat", payload))

    written = extract_raw_files_from_gamecube_iso(iso_path, raw_dir, ("PlCaAJ.dat",))

    assert written == [raw_dir / "PlCaAJ.dat"]
    assert (raw_dir / "PlCaAJ.dat").read_bytes() == payload


def test_extract_common_data_from_plco_uses_ftload_common_attribute_pointer():
    data_block = bytearray(0x900)
    ftload_offset = 0x40
    common_offset = 0x100
    data_block[ftload_offset : ftload_offset + 4] = be32(common_offset)

    put_f32(data_block, common_offset + 0x08, 0.37)
    put_f32(data_block, common_offset + 0x0C, 0.38)
    put_f32(data_block, common_offset + 0x10, 0.12)
    put_f32(data_block, common_offset + 0x14, 0.31)
    put_f32(data_block, common_offset + 0x18, 0.55)
    put_f32(data_block, common_offset + 0x20, 0.7853982)
    put_f32(data_block, common_offset + 0x24, 0.21)
    put_f32(data_block, common_offset + 0x28, 0.31)
    put_f32(data_block, common_offset + 0x2C, 0.52)
    put_f32(data_block, common_offset + 0x30, 0.74)
    put_f32(data_block, common_offset + 0x34, 0.24)
    put_f32(data_block, common_offset + 0x38, -0.35)
    put_f32(data_block, common_offset + 0x3C, 0.82)
    put_i32(data_block, common_offset + 0x40, 5)
    put_f32(data_block, common_offset + 0x44, 6.0)
    put_f32(data_block, common_offset + 0x48, 7.0)
    put_f32(data_block, common_offset + 0x4C, 16.0)
    put_f32(data_block, common_offset + 0x58, 0.66)
    put_f32(data_block, common_offset + 0x64, 0.25)
    put_f32(data_block, common_offset + 0x68, 4.0)
    put_f32(data_block, common_offset + 0x6C, 2.0)
    put_f32(data_block, common_offset + 0x70, 0.81)
    put_i32(data_block, common_offset + 0x74, 4)
    put_f32(data_block, common_offset + 0x78, 0.22)
    put_f32(data_block, common_offset + 0x7C, 0.41)
    put_f32(data_block, common_offset + 0x88, 0.83)
    put_i32(data_block, common_offset + 0x8C, 2)
    put_f32(data_block, common_offset + 0x90, 0.35)
    put_f32(data_block, common_offset + 0x94, 0.28)
    put_f32(data_block, common_offset + 0x98, 0.25)
    put_f32(data_block, common_offset + 0xAC, 0.26)
    put_f32(data_block, common_offset + 0xDC, 0.43)
    put_f32(data_block, common_offset + 0xE0, 0.44)
    put_f32(data_block, common_offset + 0x164, 8.3)
    put_f32(data_block, common_offset + 0x1FC, 0.04)
    put_f32(data_block, common_offset + 0x200, 1.5)
    put_f32(data_block, common_offset + 0x204, 0.051)
    put_f32(data_block, common_offset + 0x1A4, 1.25)
    put_f32(data_block, common_offset + 0x10C, 109.36)
    put_f32(data_block, common_offset + 0x258, 0.1)
    put_f32(data_block, common_offset + 0x25C, -0.62)
    put_f32(data_block, common_offset + 0x2F8, 402.0)
    put_f32(data_block, common_offset + 0x2FC, 92.0)
    put_f32(data_block, common_offset + 0x300, 1.25)
    put_f32(data_block, common_offset + 0x304, 3.5)
    put_i32(data_block, common_offset + 0x2A0, 2)
    put_f32(data_block, common_offset + 0x308, 33.0 / 127.0)
    put_f32(data_block, common_offset + 0x314, 0.84)
    put_i32(data_block, common_offset + 0x318, 3)
    put_f32(data_block, common_offset + 0x31C, 0.85)
    put_i32(data_block, common_offset + 0x320, 4)
    put_f32(data_block, common_offset + 0x32C, 0.20)
    put_f32(data_block, common_offset + 0x330, 0.25)
    put_i32(data_block, common_offset + 0x334, 15)
    put_f32(data_block, common_offset + 0x338, 3.1)
    put_f32(data_block, common_offset + 0x33C, 0.915)
    put_f32(data_block, common_offset + 0x344, 10.0)
    put_i32(data_block, common_offset + 0x348, 9)
    put_f32(data_block, common_offset + 0x354, 30.0)
    put_f32(data_block, common_offset + 0x358, 8.0)
    put_f32(data_block, common_offset + 0x35C, 9.0)
    put_f32(data_block, common_offset + 0x360, 15.0)
    put_f32(data_block, common_offset + 0x364, 4.0)
    put_f32(data_block, common_offset + 0x368, 1.6)
    put_f32(data_block, common_offset + 0x370, 1.0)
    put_f32(data_block, common_offset + 0x374, 1.25)
    put_f32(data_block, common_offset + 0x378, 2.5)
    put_f32(data_block, common_offset + 0x37C, 0.01)
    put_f32(data_block, common_offset + 0x3A4, 1.0)
    put_f32(data_block, common_offset + 0x3A8, 6.0)
    put_f32(data_block, common_offset + 0x3AC, 16.0)
    put_f32(data_block, common_offset + 0x3B0, 10.0)
    put_f32(data_block, common_offset + 0x3B4, 2.0)
    put_f32(data_block, common_offset + 0x42C, 1.25)
    put_f32(data_block, common_offset + 0x430, 2.0)
    put_f32(data_block, common_offset + 0x444, 0.18)
    put_f32(data_block, common_offset + 0x448, 0.42)
    put_f32(data_block, common_offset + 0x44C, 0.625)
    put_f32(data_block, common_offset + 0x464, 0.63)
    put_f32(data_block, common_offset + 0x468, 5.0)
    put_f32(data_block, common_offset + 0x46C, -1.25)
    put_f32(data_block, common_offset + 0x470, 6.0)
    put_i32(data_block, common_offset + 0x500, 60)
    put_i32(data_block, common_offset + 0x6BC, 30)
    put_i32(data_block, common_offset + 0x6C0, 31)
    put_f32(data_block, common_offset + 0x6C4, 0.01)
    put_i32(data_block, common_offset + 0x6C8, 120)

    dat = make_dat("ftLoadCommonData", data_block, ftload_offset)
    extracted = extract_common_data_from_plco(
        dat, source_path=PROJECT_ROOT / "resources" / "melee" / "raw" / "PlCo.dat"
    )

    assert extracted["source"]["symbol"] == "ftLoadCommonData"
    assert extracted["source"]["file"] == "resources/melee/raw/PlCo.dat"
    assert extracted["source"]["common_attributes_offset"] == common_offset
    assert extracted["fields"]["escapeair_force"]["kind"] == "source_f32"
    assert extracted["fields"]["escapeair_force"]["raw"] == 3.0999999046325684
    assert extracted["fields"]["escapeair_decay"]["kind"] == "source_f32"
    assert extracted["fields"]["escapeair_decay"]["raw"] == 0.9150000214576721
    assert extracted["fields"]["escapeair_landing_lag_ticks"]["ticks"] == 10
    assert extracted["fields"]["throw_collision_lockout_ticks"]["source_name"] == "x348"
    assert extracted["fields"]["throw_collision_lockout_ticks"]["ticks"] == 9
    assert extracted["fields"]["throw_weight_animation_scale"]["source_name"] == "x37C"
    assert extracted["fields"]["throw_weight_animation_scale"]["raw"] == 0.009999999776482582
    assert extracted["fields"]["grab_mash_stick_threshold"]["source_name"] == "x308"
    assert extracted["fields"]["grab_mash_stick_threshold"]["stick_byte"] == 33
    assert extracted["fields"]["grab_timer_base"]["raw"] == 30.0
    assert extracted["fields"]["grab_timer_percent_scale"]["raw"] == 1.600000023841858
    assert extracted["fields"]["capture_jump_velocity_x"]["raw"] == 1.25
    assert extracted["fields"]["capture_jump_velocity_y"]["raw"] == 2.5
    assert extracted["fields"]["grab_timer_decrement"]["raw"] == 1.0
    assert extracted["fields"]["grab_mash_timer_decrement"]["raw"] == 6.0
    assert extracted["fields"]["capture_wait_jump_input_window"]["raw"] == 16.0
    assert extracted["fields"]["capture_wait_mash_anim_rate"]["raw"] == 2.0
    assert extracted["fields"]["catch_ground_friction_multiplier"]["kind"] == "source_f32"
    assert extracted["fields"]["catch_ground_friction_multiplier"]["source_name"] == "x64"
    assert extracted["fields"]["catch_ground_friction_multiplier"]["raw"] == 0.25
    assert extracted["fields"]["hitlag_electric_multiplier"]["kind"] == "source_f32"
    assert extracted["fields"]["hitlag_electric_multiplier"]["source_name"] == "x1A4"
    assert extracted["fields"]["hitlag_electric_multiplier"]["raw"] == 1.25
    assert extracted["fields"]["high_speed_ground_friction_multiplier"]["kind"] == "source_f32"
    assert extracted["fields"]["high_speed_ground_friction_multiplier"]["raw"] == 2.0
    assert extracted["fields"]["turn_run_x"]["stick_byte"] == -44
    assert extracted["fields"]["run_brake_animation_pause_velocity"]["kind"] == "source_f32"
    assert extracted["fields"]["run_brake_animation_pause_velocity"]["raw"] == 1.25
    assert extracted["fields"]["turn_run_x"]["source_name"] == "x38_someLStickXThreshold"
    assert extracted["fields"]["fall_animation_drift_threshold"]["kind"] == "source_f32"
    assert extracted["fields"]["fall_animation_drift_threshold"]["raw"] == 0.18000000715255737
    assert extracted["fields"]["air_speed_clamp_friction"]["source_name"] == "x1FC"
    assert extracted["fields"]["air_speed_clamp_friction"]["raw"] == 0.03999999910593033
    assert (
        extracted["fields"]["damage_ground_knockback_friction_multiplier"]["source_name"]
        == "x200"
    )
    assert (
        extracted["fields"]["damage_ground_knockback_friction_multiplier"]["raw"]
        == 1.5
    )
    assert (
        extracted["fields"]["damage_knockback_frame_decay"]["source_name"]
        == "x204_knockbackFrameDecay"
    )
    assert extracted["fields"]["damage_knockback_frame_decay"]["raw"] == 0.050999999046325684
    assert extracted["fields"]["damage_ground_knockback_init_clamp"]["source_name"] == "x164"
    assert extracted["fields"]["damage_ground_knockback_init_clamp"]["raw"] == 8.300000190734863
    assert extracted["fields"]["throw_knockback_weight"]["source_name"] == "x10C"
    assert extracted["fields"]["throw_knockback_weight"]["raw"] == 109.36000061035156
    assert extracted["fields"]["special_air_drift_stick_threshold"]["source_name"] == "x258"
    assert extracted["fields"]["special_air_drift_stick_threshold"]["raw"] == 0.10000000149011612
    assert extracted["fields"]["fall_animation_blend"]["kind"] == "source_f32"
    assert extracted["fields"]["fall_animation_blend"]["raw"] == 0.41999998688697815
    assert extracted["fields"]["shield_aim_smoothing"]["source_name"] == "x44C"
    assert extracted["fields"]["shield_aim_smoothing"]["kind"] == "source_f32"
    assert extracted["fields"]["shield_aim_smoothing"]["raw"] == 0.625
    assert extracted["fields"]["shield_break_furafura_percent_base"]["source_name"] == "x2F8"
    assert extracted["fields"]["shield_break_furafura_percent_base"]["raw"] == 402.0
    assert extracted["fields"]["shield_break_furafura_timer_base"]["source_name"] == "x2FC"
    assert extracted["fields"]["shield_break_furafura_timer_base"]["raw"] == 92.0
    assert extracted["fields"]["shield_break_furafura_timer_decrement"]["source_name"] == "x300"
    assert extracted["fields"]["shield_break_furafura_timer_decrement"]["raw"] == 1.25
    assert extracted["fields"]["shield_break_furafura_mash_decrement"]["source_name"] == "x304"
    assert extracted["fields"]["shield_break_furafura_mash_decrement"]["raw"] == 3.5
    assert extracted["fields"]["guard_reflect_input_window"]["ticks"] == 2
    assert extracted["fields"]["dead_wait_ticks"]["source_name"] == "x500"
    assert extracted["fields"]["dead_wait_ticks"]["ticks"] == 60
    assert extracted["fields"]["entry_start_ticks"]["ticks"] == 30
    assert extracted["fields"]["entry_end_ticks"]["ticks"] == 31
    assert extracted["fields"]["entry_initial_scale_y"]["kind"] == "source_f32"
    assert extracted["fields"]["entry_initial_scale_y"]["raw"] == 0.009999999776482582
    assert extracted["fields"]["entry_collision_landing_lag_ticks"]["ticks"] == 120


def test_extract_common_data_from_plco_exports_stale_move_reductions_from_ftload_pointer():
    data_block = bytearray(0x940)
    ftload_offset = 0x40
    common_offset = 0x100
    stale_table_offset = 0x850
    data_block[ftload_offset : ftload_offset + 4] = be32(common_offset)
    data_block[ftload_offset + 0x0C : ftload_offset + 0x10] = be32(stale_table_offset)
    for index, value in enumerate((0.09, 0.08, 0.07, 0.06, 0.05, 0.04, 0.03, 0.02, 0.01)):
        put_f32(data_block, stale_table_offset + index * 4, value)

    dat = make_dat("ftLoadCommonData", data_block, ftload_offset)
    extracted = extract_common_data_from_plco(
        dat, source_path=PROJECT_ROOT / "resources" / "melee" / "raw" / "PlCo.dat"
    )

    stale_table = extracted["tables"]["stale_move_damage_reductions"]
    assert stale_table["source_name"] == "Fighter_804D6548"
    assert stale_table["ft_load_common_data_index"] == 3
    assert stale_table["offset"] == stale_table_offset
    assert stale_table["values"] == [
        0.09000000357627869,
        0.07999999821186066,
        0.07000000029802322,
        0.05999999865889549,
        0.05000000074505806,
        0.03999999910593033,
        0.029999999329447746,
        0.019999999552965164,
        0.009999999776482582,
    ]


def test_extract_fighter_parts_tables_from_plco_reads_source_part_to_joint_mapping():
    joint_to_part = [0, 1, 2, 255, 4, 5]
    part_to_joint = [0, 1, 2, 4, 5, 255]
    dat = make_plco_with_parts_table(
        kind_index=2,
        joint_to_part=joint_to_part,
        part_to_joint=part_to_joint,
    )

    extracted = extract_fighter_parts_tables_from_plco(
        dat, source_path=PROJECT_ROOT / "resources" / "melee" / "raw" / "PlCo.dat"
    )

    captain = extracted["tables"]["2"]
    assert extracted["source"]["symbol"] == "ftLoadCommonData"
    assert extracted["source"]["ft_load_common_data_index"] == 4
    assert extracted["source"]["parts_table_pointer_array_offset"] == 0x100
    assert captain["source_name"] == "FTKIND_CAPTAIN"
    assert captain["parts_num"] == 6
    assert captain["joint_to_part"] == joint_to_part
    assert captain["part_to_joint"] == part_to_joint


def test_source_create_x1a70_uses_ftparts_part_to_joint_world_positions():
    pose = {
        "joints": [
            {"index": 0, "world_position_raw": {"x": 0.0, "y": 0.0, "z": 0.0}},
            {"index": 1, "world_position_raw": {"x": 10.0, "y": 20.0, "z": 30.0}},
            {"index": 2, "world_position_raw": {"x": -99.0, "y": -99.0, "z": -99.0}},
            {"index": 3, "world_position_raw": {"x": 7.5, "y": 15.25, "z": 25.75}},
        ]
    }
    parts_table = {
        "part_to_joint": [0, 3, 1],
    }

    x1a70 = source_create_x1a70_from_pose(pose, parts_table)

    assert x1a70["source"] == "Fighter_UnkUpdateVecFromBones_8006876C"
    assert x1a70["transn_part"] == 1
    assert x1a70["xrotn_part"] == 2
    assert x1a70["transn_joint"] == 3
    assert x1a70["xrotn_joint"] == 1
    assert x1a70["raw"] == {"x": -2.5, "y": -4.75, "z": -4.25}
    assert x1a70["milli"] == {"x": -2500, "y": -4750, "z": -4250}


def test_extract_captain_profile_from_plca_uses_ftdata_attribute_range():
    data_block = bytearray(0x400)
    ftdata_offset = 0x60
    attrs_offset = 0x100
    attrs_end = 0x284
    data_block[ftdata_offset : ftdata_offset + 4] = be32(attrs_offset)
    data_block[ftdata_offset + 4 : ftdata_offset + 8] = be32(attrs_end)

    put_f32(data_block, attrs_offset + 0x18, 0.08)
    put_f32(data_block, attrs_offset + 0x1C, 2.0)
    put_f32(data_block, attrs_offset + 0x30, 8.0)
    put_f32(data_block, attrs_offset + 0x34, 2.35)
    put_f32(data_block, attrs_offset + 0x3C, 0.4)
    put_f32(data_block, attrs_offset + 0x40, 3.1)
    put_f32(data_block, attrs_offset + 0x5C, 0.13)
    put_f32(data_block, attrs_offset + 0x60, 2.9)
    put_f32(data_block, attrs_offset + 0x64, 0.04)
    put_f32(data_block, attrs_offset + 0x68, 0.02)
    put_f32(data_block, attrs_offset + 0x6C, 1.12)
    put_f32(data_block, attrs_offset + 0x70, 0.01)
    put_f32(data_block, attrs_offset + 0xE8, 15.0)
    put_f32(data_block, attrs_offset + 0xEC, 19.0)
    put_f32(data_block, attrs_offset + 0xF0, 18.0)
    put_f32(data_block, attrs_offset + 0xF4, 15.0)
    put_f32(data_block, attrs_offset + 0xF8, 24.0)
    put_f32(data_block, attrs_offset + 0x110, 1.1)
    data_block[attrs_offset + 0x180] = 0x07

    dat = make_dat("ftDataCaptain", data_block, ftdata_offset)
    extracted = extract_captain_profile_from_plca(dat, source_path=Path("PlCa.dat"))

    assert extracted["source"]["symbol"] == "ftDataCaptain"
    assert extracted["source"]["ftco_dat_attrs_offset"] == attrs_offset
    assert extracted["source"]["ftco_dat_attrs_len"] == attrs_end - attrs_offset
    assert extracted["fields"]["ground_friction"]["kind"] == "source_f32"
    assert extracted["fields"]["ground_friction"]["raw"] == 0.07999999821186066
    assert extracted["fields"]["dash_initial_velocity"]["kind"] == "source_f32"
    assert extracted["fields"]["dash_initial_velocity"]["raw"] == 2.0
    assert extracted["fields"]["max_run_brake_frames"]["ticks"] == 8
    assert extracted["fields"]["ground_max_horizontal_velocity"]["kind"] == "source_f32"
    assert extracted["fields"]["ground_max_horizontal_velocity"]["raw"] == 2.3499999046325684
    assert extracted["fields"]["air_drift_max"]["kind"] == "source_f32"
    assert extracted["fields"]["air_drift_max"]["raw"] == 1.1200000047683716
    assert extracted["fields"]["landingairn_lag"]["ticks"] == 15
    assert extracted["fields"]["landingairf_lag"]["ticks"] == 19
    assert extracted["fields"]["landingairb_lag"]["ticks"] == 18
    assert extracted["fields"]["landingairhi_lag"]["ticks"] == 15
    assert extracted["fields"]["landingairlw_lag"]["ticks"] == 24
    assert extracted["fields"]["entry_platform_offset_y"]["milli"] == 1647
    assert extracted["fields"]["weight_independent_throws_mask"] == {
        "source_name": "weight_independent_throws_mask",
        "offset": 0x180,
        "kind": "source_u8",
        "raw": 0x07,
    }


def test_extract_captain_special_attrs_from_plca_reads_ftdata_ext_attr():
    data_block = bytearray(0x340)
    ftdata_offset = 0x40
    common_attrs_offset = 0x100
    ext_attrs_offset = 0x180
    data_block[ftdata_offset : ftdata_offset + 4] = be32(common_attrs_offset)
    data_block[ftdata_offset + 4 : ftdata_offset + 8] = be32(ext_attrs_offset)

    put_f32(data_block, ext_attrs_offset + 0x00, -0.35)
    put_f32(data_block, ext_attrs_offset + 0x08, 0.72)
    put_f32(data_block, ext_attrs_offset + 0x10, 1.4)
    put_f32(data_block, ext_attrs_offset + 0x40, 0.85)
    put_f32(data_block, ext_attrs_offset + 0x44, 0.55)
    put_f32(data_block, ext_attrs_offset + 0x48, 0.8)
    put_f32(data_block, ext_attrs_offset + 0x4C, 30.0)
    put_f32(data_block, ext_attrs_offset + 0x58, 0.25)
    put_i32(data_block, ext_attrs_offset + 0x64, 18)
    put_u32(data_block, ext_attrs_offset + 0x6C, 0x1234ABCD)
    put_f32(data_block, ext_attrs_offset + 0x88, 0.09)

    dat = make_dat("ftDataCaptain", data_block, ftdata_offset)
    extracted = extract_captain_special_attrs_from_plca(dat, source_path=Path("PlCa.dat"))

    assert extracted["source"]["symbol"] == "ftDataCaptain"
    assert extracted["source"]["ftdata_ext_attr_offset"] == ext_attrs_offset
    assert extracted["source"]["source_character"] == "captain"
    assert extracted["fields"]["specialn_stick_range_y_neg"]["raw"] == -0.3499999940395355
    assert extracted["fields"]["specialn_angle_diff"]["raw"] == 0.7200000286102295
    assert extracted["fields"]["specialn_vel_mul"]["raw"] == 1.399999976158142
    assert extracted["fields"]["specialhi_air_friction_mul"]["raw"] == 0.8500000238418579
    assert extracted["fields"]["specialhi_horz_vel"]["raw"] == 0.550000011920929
    assert extracted["fields"]["specialhi_freefall_air_spd_mul"]["raw"] == 0.800000011920929
    assert extracted["fields"]["specialhi_landing_lag"]["raw"] == 30.0
    assert extracted["fields"]["specialhi_input_var"]["raw"] == 0.25
    assert extracted["fields"]["specialhi_air_var"]["raw"] == 18
    assert extracted["fields"]["speciallw_unk1"]["raw"] == 0x1234ABCD
    assert extracted["fields"]["speciallw_air_landing_traction"]["raw"] == 0.09000000357627869


def test_extract_character_common_parts_allows_relocated_offset_zero_ftdata_x8():
    data_block = bytearray(0x220)
    ftdata_offset = 0x40
    ftdata_x8_offset = 0x00
    thrown_hitbox_offset = 0x180
    data_block[ftdata_offset + 0x08 : ftdata_offset + 0x0C] = be32(ftdata_x8_offset)
    data_block[ftdata_offset + 0x34 : ftdata_offset + 0x38] = be32(thrown_hitbox_offset)
    data_block[ftdata_x8_offset : ftdata_x8_offset + 0x18] = (
        be32(1)
        + be32(0x120)
        + be32(0)
        + be32(0x140)
        + bytes([57, 61, 39, 10, 16, 0, 0, 0])
    )
    data_block[thrown_hitbox_offset : thrown_hitbox_offset + 0x04] = be32(14)
    put_f32(data_block, thrown_hitbox_offset + 0x04, 3.5)

    dat = make_dat(
        "ftDataCaptain",
        data_block,
        ftdata_offset,
        relocation_offsets=(ftdata_offset + 0x08, ftdata_offset + 0x34),
    )
    extracted = extract_character_common_parts_from_dat(
        dat, Path("resources/melee/raw/PlCa.dat"), character_resource_spec("captain")
    )

    assert extracted["source"]["ftdata_common_parts_offset"] == 0
    assert extracted["source"]["ftdata_common_parts_relocated"] is True
    assert extracted["item_hold_part"] == 57
    assert extracted["capture_anchor_part"] == 61
    assert extracted["item_spawn_part"] == 39
    assert extracted["visibility_part_a"] == 10
    assert extracted["visibility_part_b"] == 16
    assert extracted["thrown_hitbox_part"] == 14
    assert extracted["thrown_hitbox_scale"] == 3.5


def test_extract_character_common_parts_resolves_source_parts_through_ftparts_table():
    data_block = bytearray(0x220)
    ftdata_offset = 0x40
    ftdata_x8_offset = 0x00
    thrown_hitbox_offset = 0x180
    data_block[ftdata_offset + 0x08 : ftdata_offset + 0x0C] = be32(ftdata_x8_offset)
    data_block[ftdata_offset + 0x34 : ftdata_offset + 0x38] = be32(thrown_hitbox_offset)
    data_block[ftdata_x8_offset : ftdata_x8_offset + 0x18] = (
        be32(1)
        + be32(0x120)
        + be32(0)
        + be32(0x140)
        + bytes([57, 61, 39, 10, 16, 0, 0, 0])
    )
    data_block[thrown_hitbox_offset : thrown_hitbox_offset + 0x04] = be32(14)
    put_f32(data_block, thrown_hitbox_offset + 0x04, 3.5)
    part_to_joint = list(range(56))
    part_to_joint[14] = 27
    part_to_joint[52] = 61

    dat = make_dat(
        "ftDataCaptain",
        data_block,
        ftdata_offset,
        relocation_offsets=(ftdata_offset + 0x08, ftdata_offset + 0x34),
    )
    extracted = extract_character_common_parts_from_dat(
        dat,
        Path("resources/melee/raw/PlCa.dat"),
        character_resource_spec("captain"),
        parts_table={"part_to_joint": part_to_joint},
    )

    assert extracted["source_part_indices"] == {
        "transn_part": 1,
        "xrotn_part": 2,
        "hipn_part": 4,
        "transn2_part": 52,
    }
    assert extracted["part_to_joint"] == part_to_joint
    assert extracted["transn_part"] == 1
    assert extracted["xrotn_part"] == 2
    assert extracted["hipn_part"] == 4
    assert extracted["transn2_part"] == 52
    assert extracted["transn_joint"] == 1
    assert extracted["xrotn_joint"] == 2
    assert extracted["hipn_joint"] == 4
    assert extracted["transn2_joint"] == 61
    assert extracted["thrown_hitbox_part"] == 14
    assert extracted["thrown_hitbox_joint"] == 27
    assert extracted["thrown_hitbox_scale"] == 3.5


def test_extract_character_common_parts_reads_ftdata_x34_thrown_hitbox_source():
    data_block = bytearray(0x220)
    ftdata_offset = 0x40
    ftdata_x8_offset = 0x00
    thrown_hitbox_offset = 0x180
    data_block[ftdata_offset + 0x08 : ftdata_offset + 0x0C] = be32(ftdata_x8_offset)
    data_block[ftdata_offset + 0x34 : ftdata_offset + 0x38] = be32(thrown_hitbox_offset)
    data_block[ftdata_x8_offset : ftdata_x8_offset + 0x18] = (
        be32(1)
        + be32(0x120)
        + be32(0)
        + be32(0x140)
        + bytes([57, 61, 39, 10, 16, 0, 0, 0])
    )
    data_block[thrown_hitbox_offset : thrown_hitbox_offset + 0x04] = be32(14)
    put_f32(data_block, thrown_hitbox_offset + 0x04, 4.25)
    part_to_joint = list(range(64))
    part_to_joint[14] = 27

    dat = make_dat(
        "ftDataCaptain",
        data_block,
        ftdata_offset,
        relocation_offsets=(ftdata_offset + 0x08, ftdata_offset + 0x34),
    )
    extracted = extract_character_common_parts_from_dat(
        dat,
        Path("resources/melee/raw/PlCa.dat"),
        character_resource_spec("captain"),
        parts_table={"part_to_joint": part_to_joint},
    )

    assert extracted["source"]["ftdata_thrown_hitbox_field_offset"] == ftdata_offset + 0x34
    assert extracted["source"]["ftdata_thrown_hitbox_offset"] == thrown_hitbox_offset
    assert extracted["source"]["ftdata_thrown_hitbox_relocated"] is True
    assert extracted["thrown_hitbox_part"] == 14
    assert extracted["thrown_hitbox_joint"] == 27
    assert extracted["thrown_hitbox_scale"] == 4.25


def test_extract_captain_ecb_source_from_plca_records_joint_indices_and_animation_dependency():
    data_block = bytearray(0x240)
    ftdata_offset = 0x40
    ecb_source_offset = 0x180
    data_block[ftdata_offset + 0x44 : ftdata_offset + 0x48] = be32(ecb_source_offset)
    data_block[ecb_source_offset : ecb_source_offset + 0x0C] = struct.pack(
        ">hhhhhh", 39, 47, 25, 14, 8, 4
    )
    put_f32(data_block, ecb_source_offset + 0x0C, 0.25)
    put_f32(data_block, ecb_source_offset + 0x10, 9.0)
    put_f32(data_block, ecb_source_offset + 0x14, 17.0)
    put_f32(data_block, ecb_source_offset + 0x18, 11.0)

    dat = make_dat("ftDataCaptain", data_block, ftdata_offset)
    extracted = extract_captain_ecb_source_from_plca(
        dat, source_path=Path("isolated/raw/PlCa.dat")
    )

    assert extracted["source"]["symbol"] == "ftDataCaptain"
    assert extracted["source"]["ftdata_ecb_source_offset"] == ecb_source_offset
    assert extracted["ecb_source"]["joint_indices"] == [39, 47, 25, 14, 8, 4]
    assert extracted["ecb_source"]["side_midpoint_offset_milli"] == 250
    assert extracted["ecb_source"]["ledge_snap_x_milli"] == 9000
    assert extracted["animation_dependency"]["file"] == "isolated/raw/PlCaAJ.dat"
    assert extracted["animation_dependency"]["status"] == "missing_required_for_per_frame_ecb"


def test_extract_captain_hurtbox_inits_from_plca_reads_ftdata_x30():
    data_block = bytearray(0x340)
    ftdata_offset = 0x40
    hurtbox_table_offset = 0x180
    hurtbox_inits_offset = 0x200
    data_block[ftdata_offset + 0x30 : ftdata_offset + 0x34] = be32(hurtbox_table_offset)
    put_i32(data_block, hurtbox_table_offset, 2)
    data_block[hurtbox_table_offset + 0x04 : hurtbox_table_offset + 0x08] = be32(
        hurtbox_inits_offset
    )

    data_block[hurtbox_inits_offset : hurtbox_inits_offset + 0x0C] = (
        be32(14) + be32(2) + be32(1)
    )
    put_vec3(data_block, hurtbox_inits_offset + 0x0C, (1.0, 2.0, -3.0))
    put_vec3(data_block, hurtbox_inits_offset + 0x18, (4.0, 5.0, 6.0))
    put_f32(data_block, hurtbox_inits_offset + 0x24, 3.5)

    second = hurtbox_inits_offset + 0x28
    data_block[second : second + 0x0C] = be32(7) + be32(1) + be32(0)
    put_vec3(data_block, second + 0x0C, (-1.0, 0.0, 0.25))
    put_vec3(data_block, second + 0x18, (1.0, 0.0, -0.25))
    put_f32(data_block, second + 0x24, 2.25)

    dat = make_dat("ftDataCaptain", data_block, ftdata_offset)
    extracted = extract_captain_hurtbox_inits_from_plca(
        dat, source_path=Path("isolated/raw/PlCa.dat")
    )

    assert extracted["source"]["symbol"] == "ftDataCaptain"
    assert (
        extracted["source"]["format"]
        == "HSD DAT, big-endian ftData.x30 hurt capsule init table"
    )
    assert extracted["source"]["ftdata_hurtbox_table_offset"] == hurtbox_table_offset
    assert extracted["source"]["hurtbox_inits_offset"] == hurtbox_inits_offset
    assert extracted["count"] == 2
    assert extracted["hurtboxes"][0] == {
        "id": 0,
        "bone_idx": 14,
        "height": 2,
        "is_grabbable": True,
        "a_offset_raw": {"x": 1.0, "y": 2.0, "z": -3.0},
        "a_offset_milli": {"x": 1000, "y": 2000, "z": -3000},
        "b_offset_raw": {"x": 4.0, "y": 5.0, "z": 6.0},
        "b_offset_milli": {"x": 4000, "y": 5000, "z": 6000},
        "scale_raw": 3.5,
        "scale_milli": 3500,
    }
    assert extracted["hurtboxes"][1]["bone_idx"] == 7
    assert extracted["hurtboxes"][1]["is_grabbable"] is False


def test_extract_figatree_summary_reads_nodes_tracks_and_frame_count():
    chunk = make_figatree_chunk()

    extracted = extract_figatree_summary(chunk)

    assert extracted["root"] == "PlyCaptain5K_Share_ACTION_Test_figatree"
    assert extracted["type"] == 1
    assert extracted["flags_raw"] == "0x20000000"
    assert extracted["frames_raw"] == 50.0
    assert extracted["frames_ticks"] == 50
    assert extracted["nodes_offset"] == 0x80
    assert extracted["track_counts_by_node"] == [2, 0, 1]
    assert extracted["animated_node_count"] == 3
    assert extracted["track_count"] == 3
    assert extracted["tracks"][0] == {
        "index": 0,
        "length": 5,
        "startframe": 0,
        "obj_type": 5,
        "frac_value_raw": "0x88",
        "frac_slope_raw": "0x88",
        "data_offset": 0x20,
    }
    assert extracted["tracks"][2]["obj_type"] == 7
    assert extracted["tracks"][2]["data_offset"] == 0x30


def test_sample_fobj_value_matches_hsd_linear_keyframe_flow():
    # One LIN pack with two U16 integer values: value 0, wait 10, value 10.
    animation_data = bytes([0x12, 0x00, 0x00, 0x0A, 0x0A, 0x00])

    assert sample_fobj_value(
        animation_data,
        length=len(animation_data),
        startframe=0,
        frac_value=0x40,
        frac_slope=0x40,
        frame=0.0,
    ) == 0.0
    assert sample_fobj_value(
        animation_data,
        length=len(animation_data),
        startframe=0,
        frac_value=0x40,
        frac_slope=0x40,
        frame=5.0,
    ) == 5.0
    assert sample_fobj_value(
        animation_data,
        length=len(animation_data),
        startframe=0,
        frac_value=0x40,
        frac_slope=0x40,
        frame=10.0,
    ) == 10.0


def test_sample_fobj_value_keeps_loading_data_after_slope_opcode():
    # HSD_A_OP_SLP updates the incoming slope and does not consume a wait token.
    animation_data = bytes([0x05, 0x00, 0x00, 0x12, 0x00, 0x00, 0x0A, 0x0A, 0x00])

    assert sample_fobj_value(
        animation_data,
        length=len(animation_data),
        startframe=0,
        frac_value=0x40,
        frac_slope=0x40,
        frame=5.0,
    ) == 5.0


def test_sample_fobj_value_allows_value_read_to_cross_declared_length_like_hsd():
    # HSD checks FObjDesc.length before starting a token. parseFloat can still
    # consume the bytes required by that token after the boundary.
    animation_data = bytes([0x12, 0x00, 0x00, 0x0A, 0x0A, 0x00])

    assert sample_fobj_value(
        animation_data,
        length=len(animation_data) - 1,
        startframe=0,
        frac_value=0x40,
        frac_slope=0x40,
        frame=10.0,
    ) == 10.0


def test_sample_figatree_node_tracks_maps_tracks_to_jobj_channels():
    chunk = make_sampled_figatree_chunk()

    node_zero = sample_figatree_node_tracks(chunk, node_index=0, frame=5.0)
    node_one = sample_figatree_node_tracks(chunk, node_index=1, frame=5.0)

    assert node_zero["translation"] == {"x": 5.0, "y": 10.0}
    assert node_zero["rotation"] == {}
    assert node_zero["tracks"][0]["channel"] == "translation.x"
    assert node_zero["tracks"][1]["channel"] == "translation.y"
    assert node_one["rotation"] == {"x": 10.0}
    assert node_one["tracks"][0]["channel"] == "rotation.x"


def test_sample_figatree_skeleton_pose_concatenates_hsd_jobj_transforms():
    chunk = make_sampled_figatree_chunk()
    data_block = bytearray(0x100)
    root_offset = 0x20
    child_offset = 0x60
    put_joint(data_block, root_offset, flags=0, child=child_offset, position=(1.0, 0.0, 0.0))
    put_joint(data_block, child_offset, flags=0, position=(0.0, 2.0, 0.0))
    dat = make_dat("PlyCaptain5K_Share_joint", data_block, root_offset)
    skeleton = extract_captain_costume_skeleton_from_plcanr(
        dat, source_path=Path("resources/melee/raw/PlCaNr.dat")
    )

    pose = sample_figatree_skeleton_pose(chunk, skeleton, frame=5.0)

    assert pose["joints"][0]["translation"] == {"x": 5.0, "y": 10.0, "z": 0.0}
    assert pose["joints"][0]["world_position_milli"] == {"x": 5000, "y": 10000, "z": 0}
    assert pose["joints"][1]["world_position_milli"] == {"x": 5000, "y": 12000, "z": 0}


def test_sample_figatree_skeleton_pose_applies_topn_rot_y_before_world_matrices():
    chunk = make_sampled_figatree_chunk()
    data_block = bytearray(0x100)
    root_offset = 0x20
    child_offset = 0x60
    put_joint(data_block, root_offset, flags=0, child=child_offset, position=(0.0, 0.0, 0.0))
    put_joint(data_block, child_offset, flags=0, position=(0.0, 0.0, 3.0))
    dat = make_dat("PlyCaptain5K_Share_joint", data_block, root_offset)
    skeleton = extract_captain_costume_skeleton_from_plcanr(
        dat, source_path=Path("resources/melee/raw/PlCaNr.dat")
    )

    pose = sample_figatree_skeleton_pose(chunk, skeleton, frame=5.0, topn_rot_y=math.pi / 2)

    assert pose["joints"][1]["world_position_milli"] == {"x": 8000, "y": 10000, "z": 0}


def test_sample_figatree_skeleton_pose_applies_topn_scale_before_world_matrices():
    chunk = make_sampled_figatree_chunk()
    data_block = bytearray(0x100)
    root_offset = 0x20
    child_offset = 0x60
    put_joint(data_block, root_offset, flags=0, child=child_offset, position=(0.0, 0.0, 0.0))
    put_joint(data_block, child_offset, flags=0, position=(0.0, 0.0, 3.0))
    dat = make_dat("PlyCaptain5K_Share_joint", data_block, root_offset)
    skeleton = extract_captain_costume_skeleton_from_plcanr(
        dat, source_path=Path("resources/melee/raw/PlCaNr.dat")
    )

    pose = sample_figatree_skeleton_pose(
        chunk,
        skeleton,
        frame=5.0,
        topn_rot_y=math.pi / 2,
        topn_scale=2.0,
    )

    assert pose["joints"][1]["world_position_milli"] == {"x": 11000, "y": 10000, "z": 0}


def test_sample_figatree_skeleton_pose_can_clear_transn_after_fobj_sampling():
    chunk = make_transn_translation_figatree_chunk()
    data_block = bytearray(0x140)
    root_offset = 0x20
    transn_offset = 0x60
    child_offset = 0xA0
    put_joint(data_block, root_offset, flags=0, child=transn_offset)
    put_joint(data_block, transn_offset, flags=0, child=child_offset)
    put_joint(data_block, child_offset, flags=0, position=(0.0, 3.0, 0.0))
    dat = make_dat("PlyCaptain5K_Share_joint", data_block, root_offset)
    skeleton = extract_captain_costume_skeleton_from_plcanr(
        dat, source_path=Path("resources/melee/raw/PlCaNr.dat")
    )

    raw_pose = sample_figatree_skeleton_pose(chunk, skeleton, frame=5.0)
    collision_pose = sample_figatree_skeleton_pose(
        chunk,
        skeleton,
        frame=5.0,
        clear_after_anim_joint_indices=(1,),
    )

    assert raw_pose["joints"][1]["translation"] == {"x": 0.0, "y": 10.0, "z": 0.0}
    assert raw_pose["joints"][2]["world_position_milli"] == {"x": 0, "y": 13000, "z": 0}
    assert collision_pose["joints"][1]["translation"] == {"x": 0.0, "y": 0.0, "z": 0.0}
    assert collision_pose["joints"][2]["world_position_milli"] == {"x": 0, "y": 3000, "z": 0}


def test_compute_ecb_from_jobj_pose_matches_mpcoll_jobj_reduction():
    pose = {
        "joints": [
            {"world_position_raw": {"x": 0.0, "y": 0.0, "z": 0.0}},
            {"world_position_raw": {"x": 30.0, "y": 10.0, "z": 3.0}},
            {"world_position_raw": {"x": -50.0, "y": 4.0, "z": -5.0}},
            {"world_position_raw": {"x": 20.0, "y": 6.0, "z": 2.0}},
            {"world_position_raw": {"x": -10.0, "y": 8.0, "z": -1.0}},
            {"world_position_raw": {"x": 10.0, "y": 2.0, "z": 1.0}},
        ]
    }
    ecb_source = {
        "joint_indices": [0, 1, 2, 3, 4, 5],
        "side_midpoint_offset_raw": 0.25,
    }

    ecb = compute_ecb_from_jobj_pose(pose, ecb_source, flags=6)

    assert ecb["top"] == {"x": 0.0, "y": 10.0}
    assert ecb["bottom"] == {"x": 0.0, "y": 0.0}
    assert ecb["right"] == {"x": 30.0, "y": 5.25}
    assert ecb["left"] == {"x": -50.0, "y": 5.25}
    assert ecb["source_points"][1] == {"x": 30.0, "y": 10.0, "z": 3.0}
    assert "render_points" not in ecb
    assert ecb["source_render_transform"] == "none; mpColl_LoadECB_JObj uses lb_8000B1CC world positions"
    assert ecb["flatten_after_render"] == "none_collision_world_xy"
    assert ecb["bottom_milli"] == {"x": 0, "y": 0}


def test_compute_ecb_from_jobj_pose_uses_source_min_extents():
    pose = {
        "joints": [
            {"world_position_raw": {"x": 5.0, "y": 0.0, "z": 0.0}},
            {"world_position_raw": {"x": 10.0, "y": 1.0, "z": 0.0}},
            {"world_position_raw": {"x": 7.0, "y": 0.5, "z": 0.0}},
            {"world_position_raw": {"x": 8.0, "y": 0.5, "z": 0.0}},
            {"world_position_raw": {"x": 6.0, "y": 0.25, "z": 0.0}},
            {"world_position_raw": {"x": 9.0, "y": 0.25, "z": 0.0}},
        ]
    }
    ecb_source = {
        "joint_indices": [0, 1, 2, 3, 4, 5],
        "side_midpoint_offset_raw": 0.0,
        "min_height_raw": 4.0,
        "min_width_raw": 4.0,
    }

    ecb = compute_ecb_from_jobj_pose(pose, ecb_source, flags=6)

    assert ecb["bottom"] == {"x": 0.0, "y": 0.0}
    assert ecb["right"]["x"] == 10.0
    assert ecb["left"]["x"] == -2.0


def test_extract_character_ecb_source_records_runtime_min_extents():
    ftdata_offset = 0x20
    ecb_source_offset = 0x80
    data_block = bytearray(0xC0)
    put_u32(data_block, ftdata_offset + 0x44, ecb_source_offset)
    for index, joint_index in enumerate((39, 47, 25, 14, 8, 4)):
        data_block[ecb_source_offset + index * 2 : ecb_source_offset + index * 2 + 2] = (
            joint_index.to_bytes(2, "big", signed=True)
        )
    put_f32(data_block, ecb_source_offset + 0x0C, 0.25)
    put_f32(data_block, ecb_source_offset + 0x10, 9.0)
    put_f32(data_block, ecb_source_offset + 0x14, 17.0)
    put_f32(data_block, ecb_source_offset + 0x18, 11.0)
    dat = make_dat("ftDataTest", data_block, ftdata_offset)
    spec = CharacterResourceSpec(
        id="test",
        output_stem="test_character",
        data_dat="Test.dat",
        action_dat="TestAJ.dat",
        neutral_costume_dat="TestNr.dat",
        ft_data_symbol="ftDataTest",
        neutral_joint_root="Test_joint",
    )

    extracted = melee_resources.extract_character_ecb_source_from_dat(
        dat, Path("resources/melee/raw/Test.dat"), spec
    )

    assert extracted["ecb_source"]["min_height_raw"] == 10.0
    assert extracted["ecb_source"]["min_width_raw"] == 10.0
    assert extracted["ecb_source"]["min_height_source"] == "ft_80081B38 sets ecb_source.x128 = 10.0F * fp->x34_scale.y"
    assert extracted["ecb_source"]["min_width_source"] == "ft_80081B38 sets ecb_source.x12C = 10.0F * fp->x34_scale.y"


def test_extract_captain_action_ecb_samples_uses_figatree_skeleton_and_source_joints():
    chunk = make_sampled_figatree_chunk()
    data_block = bytearray(0x100)
    root_offset = 0x20
    child_offset = 0x60
    put_joint(data_block, root_offset, flags=0, child=child_offset, position=(1.0, 0.0, 0.0))
    put_joint(data_block, child_offset, flags=0, position=(0.0, 2.0, 0.0))
    dat = make_dat("PlyCaptain5K_Share_joint", data_block, root_offset)
    skeleton = extract_captain_costume_skeleton_from_plcanr(
        dat, source_path=Path("resources/melee/raw/PlCaNr.dat")
    )
    action_table = {
        "actions": [
            {
                "action_state_id": 44,
                "name": "PlyCaptain5K_Share_ACTION_Sampled_figatree",
                "figatree_archive_offset": 0,
                "figatree_archive_size": len(chunk),
                "figatree": {"frames_ticks": 12},
            }
        ]
    }
    ecb_source = {"ecb_source": {"joint_indices": [0, 1, 0, 1, 0, 1]}}

    samples = extract_captain_action_ecb_samples(
        chunk, action_table, skeleton, ecb_source, action_state_ids=(44,)
    )

    assert samples["actions"][0]["identity_kind"] == "fighter_wait_anim_data_index"
    assert samples["actions"][0]["action_table_index"] == 44
    assert samples["actions"][0]["action_state_id"] == 44
    assert samples["actions"][0]["frames"][5]["bottom_milli"] == {"x": 0, "y": 10000}
    assert samples["actions"][0]["frames"][5]["top_milli"] == {"x": 0, "y": 12000}
    assert samples["actions"][0]["frames"][5]["source_joint_indices"] == [0, 1, 0, 1, 0, 1]
    assert "source_points" in samples["actions"][0]["frames"][5]
    assert "render_points" not in samples["actions"][0]["frames"][5]


def test_extract_captain_action_ecb_samples_clears_transn_before_collision_ecb():
    chunk = make_transn_translation_figatree_chunk()
    data_block = bytearray(0x140)
    root_offset = 0x20
    transn_offset = 0x60
    child_offset = 0xA0
    put_joint(data_block, root_offset, flags=0, child=transn_offset)
    put_joint(data_block, transn_offset, flags=0, child=child_offset)
    put_joint(data_block, child_offset, flags=0, position=(0.0, 3.0, 0.0))
    dat = make_dat("PlyCaptain5K_Share_joint", data_block, root_offset)
    skeleton = extract_captain_costume_skeleton_from_plcanr(
        dat, source_path=Path("resources/melee/raw/PlCaNr.dat")
    )
    action_table = {
        "actions": [
            {
                "action_state_id": 307,
                "name": "PlyCaptain5K_Share_ACTION_SpecialHi_figatree",
                "flags_raw": "0x82000002",
                "figatree_archive_offset": 0,
                "figatree_archive_size": len(chunk),
                "figatree": {"frames_ticks": 12},
            }
        ]
    }
    ecb_source = {
        "ecb_source": {
            "joint_indices": [2, 2, 2, 2, 2, 2],
            "side_midpoint_offset_raw": 0.0,
        }
    }

    samples = extract_captain_action_ecb_samples(
        chunk, action_table, skeleton, ecb_source, action_state_ids=(307,)
    )

    frame = samples["actions"][0]["frames"][5]
    assert frame["bottom_milli"] == {"x": 0, "y": 3000}
    assert frame["source_points"][0] == {"x": 0.0, "y": 3.0, "z": 0.0}
    assert samples["actions"][0]["anim_curr_flags_raw"] == "0x82000002"
    assert samples["actions"][0]["collision_pose_transn_cleared"] is True


def test_extract_captain_action_ecb_samples_preserves_transn_without_anim_flag_b0():
    chunk = make_transn_translation_figatree_chunk()
    data_block = bytearray(0x140)
    root_offset = 0x20
    transn_offset = 0x60
    child_offset = 0xA0
    put_joint(data_block, root_offset, flags=0, child=transn_offset)
    put_joint(data_block, transn_offset, flags=0, child=child_offset)
    put_joint(data_block, child_offset, flags=0, position=(0.0, 3.0, 0.0))
    dat = make_dat("PlyCaptain5K_Share_joint", data_block, root_offset)
    skeleton = extract_captain_costume_skeleton_from_plcanr(
        dat, source_path=Path("resources/melee/raw/PlCaNr.dat")
    )
    action_table = {
        "actions": [
            {
                "action_state_id": 36,
                "name": "PlyCaptain5K_Share_ACTION_Landing_figatree",
                "flags_raw": "0x00000002",
                "figatree_archive_offset": 0,
                "figatree_archive_size": len(chunk),
                "figatree": {"frames_ticks": 12},
            }
        ]
    }
    ecb_source = {
        "ecb_source": {
            "joint_indices": [2, 2, 2, 2, 2, 2],
            "side_midpoint_offset_raw": 0.0,
        }
    }

    samples = extract_captain_action_ecb_samples(
        chunk, action_table, skeleton, ecb_source, action_state_ids=(36,)
    )

    frame = samples["actions"][0]["frames"][5]
    assert frame["bottom_milli"] == {"x": 0, "y": 13000}
    assert frame["source_points"][0] == {"x": 0.0, "y": 13.0, "z": 0.0}
    assert samples["actions"][0]["anim_curr_flags_raw"] == "0x00000002"
    assert samples["actions"][0]["collision_pose_transn_cleared"] is False


def test_sample_hurtboxes_from_pose_renders_right_facing_before_flattening_view_z():
    pose = {
        "joints": [
            {
                "world_matrix": [
                    [0.0, -1.0, 0.0, 10.0],
                    [1.0, 0.0, 0.0, 20.0],
                    [0.0, 0.0, 1.0, 30.0],
                ],
                "world_position_raw": {"x": 10.0, "y": 20.0, "z": 30.0},
            }
        ]
    }
    hurtbox_inits = {
        "hurtboxes": [
            {
                "id": 0,
                "bone_idx": 0,
                "height": 2,
                "is_grabbable": True,
                "a_offset_raw": {"x": 1.0, "y": 0.0, "z": 2.0},
                "b_offset_raw": {"x": 0.0, "y": 3.0, "z": -1.0},
                "scale_raw": 2.5,
            }
        ]
    }

    hurtboxes = sample_hurtboxes_from_pose(pose, hurtbox_inits)

    assert hurtboxes[0]["source_a"] == {"x": 10.0, "y": 21.0, "z": 32.0}
    assert hurtboxes[0]["source_b"] == {"x": 7.0, "y": 20.0, "z": 29.0}
    assert hurtboxes[0]["a"] == {"x": 32.0, "y": 21.0, "z": 0.0}
    assert hurtboxes[0]["b"] == {"x": 29.0, "y": 20.0, "z": 0.0}
    assert hurtboxes[0]["radius"] == 2.5
    assert hurtboxes[0]["scale"] == 2.5
    assert hurtboxes[0]["state"] == "HurtCapsule_Enabled"
    assert "source_a_pos" not in hurtboxes[0]
    assert "source_b_pos" not in hurtboxes[0]
    assert "a_pos" not in hurtboxes[0]
    assert "b_pos" not in hurtboxes[0]
    assert "source_render_endpoints" not in hurtboxes[0]


def test_extract_captain_action_hurtbox_samples_uses_figatree_skeleton_and_static_inits():
    chunk = make_sampled_figatree_chunk()
    data_block = bytearray(0x100)
    root_offset = 0x20
    child_offset = 0x60
    put_joint(data_block, root_offset, flags=0, child=child_offset, position=(1.0, 0.0, 0.0))
    put_joint(data_block, child_offset, flags=0, position=(0.0, 2.0, 0.0))
    dat = make_dat("PlyCaptain5K_Share_joint", data_block, root_offset)
    skeleton = extract_captain_costume_skeleton_from_plcanr(
        dat, source_path=Path("resources/melee/raw/PlCaNr.dat")
    )
    action_table = {
        "actions": [
            {
                "action_state_id": 44,
                "name": "PlyCaptain5K_Share_ACTION_Sampled_figatree",
                "figatree_archive_offset": 0,
                "figatree_archive_size": len(chunk),
                "figatree": {"frames_ticks": 12},
            }
        ]
    }
    hurtbox_inits = {
        "hurtboxes": [
            {
                "id": 0,
                "bone_idx": 0,
                "height": 1,
                "is_grabbable": True,
                "a_offset_raw": {"x": 0.0, "y": 0.0, "z": 1.0},
                "b_offset_raw": {"x": 0.0, "y": 1.0, "z": -1.0},
                "scale_raw": 2.25,
            }
        ]
    }

    samples = extract_captain_action_hurtbox_samples(
        chunk, action_table, skeleton, hurtbox_inits, action_state_ids=(44,)
    )

    action = samples["actions"][0]
    metadata = samples["sample_metadata"]
    assert action["action_state_id"] == 44
    assert metadata["source"] == "ftData.x30 + PlCaAJ FigaTree + PlCaNr JObj skeleton"
    assert metadata["source_render_endpoints"] == "HurtCapsule.a_pos -> HurtCapsule.b_pos"
    assert metadata["source_render_radius"] == "HurtCapsule.scale"
    assert metadata["source_render_transform"] == "ftPartSetRotY(TopN, M_PI_2 * fp->facing_dir)"
    assert metadata["flatten_after_render"] == "right_facing_melee_xy"
    assert metadata["source_init_handler"] == "ftColl_8007B3A0/ftColl_8007B4E0 ftData.x30"
    assert metadata["source_update_handler"] == "lbColl_800083C4/lbColl_8000A244/lbColl_8000A584"
    assert (
        metadata["source_draw_handler"]
        == "ftDrawCommon_800805C8 -> lbColl_8000A244/lbColl_8000A584 -> lbColl_DrawHitResult"
    )
    assert metadata["source_color_table"] == "lbColl_803B9928[hurt->state]"
    assert metadata["source_skip_update_pos_after_transform"] is True
    assert (
        metadata["source_z_policy"]
        == "preserve JObj-transformed z; debug render may force fighter->cur_pos.z when ftCommon_8007F804 returns non-null"
    )
    assert action["frames"][0]["frame"] == 1
    assert action["frames"][5]["frame"] == 6
    assert action["frames"][5]["pose"]["joints"][0]["world_matrix"] == [
        [1.0, 0.0, 0.0, 5.0],
        [0.0, 1.0, 0.0, 10.0],
        [0.0, 0.0, 1.0, 0.0],
    ]
    assert set(action["frames"][5]["pose"]["joints"][0]) == {"index", "world_matrix"}
    hurtbox = action["frames"][5]["hurtboxes"][0]
    assert "source" not in hurtbox
    assert hurtbox["source_a"]["z"] == 1.0
    assert hurtbox["a"]["z"] == 0.0
    assert hurtbox["source_b"]["z"] == -1.0
    assert hurtbox["b"]["z"] == 0.0


def test_extract_captain_action_hurtbox_samples_keeps_action_hitbox_pose_bones():
    chunk = make_sampled_figatree_chunk()
    data_block = bytearray(0x140)
    root_offset = 0x20
    child_offset = 0x60
    sibling_offset = 0xA0
    put_joint(data_block, root_offset, flags=0, child=child_offset, position=(1.0, 0.0, 0.0))
    put_joint(
        data_block,
        child_offset,
        flags=0,
        next=sibling_offset,
        position=(0.0, 2.0, 0.0),
    )
    put_joint(data_block, sibling_offset, flags=0, position=(0.0, 0.0, 3.0))
    dat = make_dat("PlyCaptain5K_Share_joint", data_block, root_offset)
    skeleton = extract_captain_costume_skeleton_from_plcanr(
        dat, source_path=Path("resources/melee/raw/PlCaNr.dat")
    )
    action_table = {
        "actions": [
            {
                "action_state_id": 44,
                "name": "PlyCaptain5K_Share_ACTION_Sampled_figatree",
                "figatree_archive_offset": 0,
                "figatree_archive_size": len(chunk),
                "figatree": {"frames_ticks": 12},
                "subaction_script_offset": 0,
            }
        ]
    }
    hurtbox_inits = {
        "hurtboxes": [
            {
                "id": 0,
                "bone_idx": 0,
                "height": 1,
                "is_grabbable": True,
                "a_offset_raw": {"x": 0.0, "y": 0.0, "z": 0.0},
                "b_offset_raw": {"x": 0.0, "y": 1.0, "z": 0.0},
                "scale_raw": 2.25,
            }
        ]
    }
    script_words = [hitbox_word0(bone=1), 0, 0, 0, 0, 0]
    script_bytes = b"\0" * 0x20 + b"".join(be32(word) for word in script_words)

    samples = extract_captain_action_hurtbox_samples(
        chunk,
        action_table,
        skeleton,
        hurtbox_inits,
        action_state_ids=(44,),
        action_script_bytes=script_bytes,
    )

    joint_indices = {
        joint["index"] for joint in samples["actions"][0]["frames"][0]["pose"]["joints"]
    }
    assert extract_action_script_hitbox_bone_indices(script_bytes, 0x20) == {1}
    assert joint_indices == {0, 1, 2}


def test_extract_captain_costume_skeleton_from_plcanr_walks_joint_tree_preorder():
    data_block = bytearray(0x180)
    root_offset = 0x40
    child_offset = 0x80
    sibling_offset = 0xC0
    put_joint(
        data_block,
        root_offset,
        flags=0x300D008E,
        child=child_offset,
        position=(0.0, 0.0, 0.0),
    )
    put_joint(
        data_block,
        child_offset,
        flags=0x9,
        next=sibling_offset,
        position=(1.25, 2.5, -0.5),
    )
    put_joint(
        data_block,
        sibling_offset,
        flags=0x8,
        position=(-3.0, 4.0, 0.0),
    )
    dat = make_dat("PlyCaptain5K_Share_joint", data_block, root_offset)

    extracted = extract_captain_costume_skeleton_from_plcanr(
        dat, source_path=Path("resources/melee/raw/PlCaNr.dat")
    )

    assert extracted["source"]["root"] == "PlyCaptain5K_Share_joint"
    assert extracted["source"]["joint_count"] == 3
    assert extracted["joints"][0]["index"] == 0
    assert extracted["joints"][0]["child_offset"] == child_offset
    assert extracted["joints"][1]["index"] == 1
    assert extracted["joints"][1]["parent_index"] == 0
    assert extracted["joints"][1]["next_offset"] == sibling_offset
    assert extracted["joints"][1]["position_milli"] == {"x": 1250, "y": 2500, "z": -500}
    assert extracted["joints"][2]["index"] == 2
    assert extracted["joints"][2]["parent_index"] == 0
    assert extracted["joints"][2]["depth"] == 1


def test_extract_captain_action_animation_table_links_plca_records_to_plcaaj_chunks():
    first_chunk = make_figatree_chunk("PlyCaptain5K_Share_ACTION_Wait1_figatree")
    second_chunk = make_figatree_chunk("PlyCaptain5K_Share_ACTION_Dash_figatree")
    plcaaj = bytearray(0x500)
    plcaaj[0 : len(first_chunk)] = first_chunk
    plcaaj[0x200 : 0x200 + len(second_chunk)] = second_chunk

    data_block = bytearray(0x400)
    ftdata_offset = 0x40
    action_table_offset = 0x180
    wait_name_offset = 0x300
    dash_name_offset = 0x340
    data_block[ftdata_offset + 0x0C : ftdata_offset + 0x10] = be32(action_table_offset)
    data_block[wait_name_offset : wait_name_offset + 13] = b"Wait1_action\0"
    data_block[dash_name_offset : dash_name_offset + 12] = b"Dash_action\0"

    data_block[action_table_offset : action_table_offset + 0x18] = (
        be32(wait_name_offset)
        + be32(0)
        + be32(len(first_chunk))
        + be32(0x220)
        + be32(0x2)
        + be32(0)
    )
    data_block[action_table_offset + 0x18 : action_table_offset + 0x30] = (
        be32(dash_name_offset)
        + be32(0x200)
        + be32(len(second_chunk))
        + be32(0x240)
        + be32(0x80000002)
        + be32(0)
    )

    plca = make_dat("ftDataCaptain", data_block, ftdata_offset)

    extracted = extract_captain_action_animation_table(
        plca,
        Path("resources/melee/raw/PlCa.dat"),
        bytes(plcaaj),
        Path("resources/melee/raw/PlCaAJ.dat"),
        action_count=2,
    )

    assert extracted["source"]["action_table_offset"] == action_table_offset
    assert extracted["source"]["action_count"] == 2
    assert extracted["actions"][0]["identity_kind"] == "fighter_wait_anim_data_index"
    assert extracted["actions"][0]["action_table_index"] == 0
    assert extracted["actions"][0]["action_state_id"] == 0
    assert extracted["actions"][0]["name"] == "Wait1_action"
    assert extracted["actions"][0]["figatree_root"] == "PlyCaptain5K_Share_ACTION_Wait1_figatree"
    assert extracted["actions"][1]["identity_kind"] == "fighter_wait_anim_data_index"
    assert extracted["actions"][1]["action_table_index"] == 1
    assert extracted["actions"][1]["action_state_id"] == 1
    assert extracted["actions"][1]["figatree_archive_offset"] == 0x200
    assert extracted["actions"][1]["figatree_root"] == "PlyCaptain5K_Share_ACTION_Dash_figatree"
    assert extracted["actions"][1]["figatree"]["frames_ticks"] == 50
    assert extracted["actions"][1]["figatree"]["track_counts_by_node"] == [2, 0, 1]
    assert extracted["actions"][1]["flags_raw"] == "0x80000002"
    assert extracted["actions"][1]["status"] == "available_for_jobj_sampling"


def test_extract_action_script_cmd_var_events_decodes_minimal_fighter_scripts():
    script = bytearray()
    script += be32(0x28000000)  # fighter opcode 10, length 5
    script += be32(0x04010000) + be32(0) + be32(0) + be32(0)
    script += be32(0x08000009)  # async timer frame 9
    script += be32(0x4D000001)  # set cmd_vars[1] = 1
    script += be32(0x1C000000)  # Command_07 common Goto command word
    script += be32(0x0800000F)  # async timer frame 15
    script += be32(0x4C000000)  # set cmd_vars[0] = 0
    script += be32(0)

    events = extract_action_script_cmd_var_events(bytes(script), 0)

    assert events == [
        {"frame": 9, "cmd_var": 1, "value": 1, "word_offset": 6},
        {"frame": 15, "cmd_var": 0, "value": 0, "word_offset": 9},
    ]


def test_extract_captain_dash_script_cmd_var0_run_gate_from_real_resources():
    plca = (PROJECT_ROOT / "resources/melee/raw/PlCa.dat").read_bytes()
    plcaaj = (PROJECT_ROOT / "resources/melee/raw/PlCaAJ.dat").read_bytes()

    extracted = extract_captain_action_animation_table(
        plca,
        PROJECT_ROOT / "resources/melee/raw/PlCa.dat",
        plcaaj,
        PROJECT_ROOT / "resources/melee/raw/PlCaAJ.dat",
    )
    dash = extracted["actions"][12]

    assert dash["figatree_root"] == "PlyCaptain5K_Share_ACTION_Dash_figatree"
    assert dash["figatree"]["frames_ticks"] == 29
    assert dash["cmd_var_events"] == [
        {"frame": 0, "cmd_var": 0, "value": 0, "word_offset": 5},
        {"frame": 16, "cmd_var": 0, "value": 1, "word_offset": 13},
    ]


def test_extract_captain_escape_air_script_skip_decay_gate_from_real_resources():
    plca = (PROJECT_ROOT / "resources/melee/raw/PlCa.dat").read_bytes()
    plcaaj = (PROJECT_ROOT / "resources/melee/raw/PlCaAJ.dat").read_bytes()

    extracted = extract_captain_action_animation_table(
        plca,
        PROJECT_ROOT / "resources/melee/raw/PlCa.dat",
        plcaaj,
        PROJECT_ROOT / "resources/melee/raw/PlCaAJ.dat",
    )
    escape_air = extracted["actions"][44]

    assert escape_air["identity_kind"] == "fighter_wait_anim_data_index"
    assert escape_air["action_table_index"] == 44
    assert escape_air["action_state_id"] == 44
    assert escape_air["figatree_root"] == "PlyCaptain5K_Share_ACTION_EscapeAir_figatree"
    assert escape_air["figatree"]["frames_ticks"] == 50
    assert escape_air["cmd_var_events"] == [
        {"frame": 30, "cmd_var": 0, "value": 1, "word_offset": 7},
    ]


def test_extract_captain_special_hi_scripts_preserve_cmd_var_air_transition_events():
    plca = (PROJECT_ROOT / "resources/melee/raw/PlCa.dat").read_bytes()
    plcaaj = (PROJECT_ROOT / "resources/melee/raw/PlCaAJ.dat").read_bytes()

    extracted = extract_captain_action_animation_table(
        plca,
        PROJECT_ROOT / "resources/melee/raw/PlCa.dat",
        plcaaj,
        PROJECT_ROOT / "resources/melee/raw/PlCaAJ.dat",
    )

    special_hi = extracted["actions"][307]
    special_air_hi = extracted["actions"][308]

    assert special_hi["figatree_root"] == "PlyCaptain5K_Share_ACTION_SpecialHi_figatree"
    assert special_hi["figatree"]["frames_ticks"] == 65
    assert special_hi["cmd_var_events"] == [
        {"frame": 13, "cmd_var": 0, "value": 1, "word_offset": 7},
    ]
    assert special_air_hi["figatree_root"] == (
        "PlyCaptain5K_Share_ACTION_SpecialAirHi_figatree"
    )
    assert special_air_hi["figatree"]["frames_ticks"] == 65
    assert special_air_hi["cmd_var_events"] == [
        {"frame": 13, "cmd_var": 0, "value": 1, "word_offset": 7},
    ]


def test_extract_captain_profile_rejects_non_captain_roots():
    data_block = bytearray(0x400)
    dat = make_dat("ftDataFox", data_block, 0x60)

    try:
        extract_captain_profile_from_plca(dat, source_path=Path("PlCa.dat"))
    except DatExtractError as error:
        assert "ftDataCaptain" in str(error)
    else:
        raise AssertionError("expected non-Captain DAT root to be rejected")


def test_character_resource_spec_uses_decomp_marth_names():
    spec = character_resource_spec("marth")

    assert spec == CharacterResourceSpec(
        id="marth",
        output_stem="marth",
        data_dat="PlMs.dat",
        action_dat="PlMsAJ.dat",
        neutral_costume_dat="PlMsNr.dat",
        ft_data_symbol="ftDataMars",
        neutral_joint_root="PlyMars5K_Share_joint",
        fighter_kind_index=18,
    )
    assert character_resource_spec("mars") == spec
    assert iso_files_for_characters(("marth",)) == (
        "PlCo.dat",
        "PlMs.dat",
        "PlMsAJ.dat",
        "PlMsNr.dat",
    )


def test_extract_character_profile_accepts_marth_ftdata_root():
    data_block = bytearray(0x400)
    ftdata_offset = 0x60
    attrs_offset = 0x100
    attrs_end = 0x284
    data_block[ftdata_offset : ftdata_offset + 4] = be32(attrs_offset)
    data_block[ftdata_offset + 4 : ftdata_offset + 8] = be32(attrs_end)
    put_f32(data_block, attrs_offset + 0x18, 0.09)
    put_f32(data_block, attrs_offset + 0x1C, 1.7)
    put_f32(data_block, attrs_offset + 0x30, 7.0)
    put_f32(data_block, attrs_offset + 0x34, 2.1)
    put_f32(data_block, attrs_offset + 0x3C, 0.42)
    put_f32(data_block, attrs_offset + 0x40, 2.8)
    put_f32(data_block, attrs_offset + 0x5C, 0.08)
    put_f32(data_block, attrs_offset + 0x60, 2.2)
    put_f32(data_block, attrs_offset + 0x64, 0.03)
    put_f32(data_block, attrs_offset + 0x68, 0.02)
    put_f32(data_block, attrs_offset + 0x6C, 1.05)
    put_f32(data_block, attrs_offset + 0x70, 0.01)
    put_f32(data_block, attrs_offset + 0xE8, 15.0)
    put_f32(data_block, attrs_offset + 0xEC, 16.0)
    put_f32(data_block, attrs_offset + 0xF0, 17.0)
    put_f32(data_block, attrs_offset + 0xF4, 18.0)
    put_f32(data_block, attrs_offset + 0xF8, 19.0)
    put_f32(data_block, attrs_offset + 0x110, 1.0)

    dat = make_dat("ftDataMars", data_block, ftdata_offset)
    extracted = extract_character_profile_from_dat(
        dat, Path("resources/melee/raw/PlMs.dat"), character_resource_spec("marth")
    )

    assert extracted["source"]["source_character"] == "marth"
    assert extracted["source"]["symbol"] == "ftDataMars"
    assert extracted["source"]["file"] == "resources/melee/raw/PlMs.dat"
    assert extracted["fields"]["landingairlw_lag"]["ticks"] == 19


def test_extract_resources_writes_marth_character_scoped_outputs(tmp_path):
    raw_dir = tmp_path / "raw"
    out_dir = tmp_path / "extracted"
    raw_dir.mkdir()

    fighter_block = bytearray(0x500)
    ftdata_offset = 0x40
    attrs_offset = 0x100
    attrs_end = 0x284
    ecb_source_offset = 0x2C0
    hurtbox_table_offset = 0x300
    hurtbox_inits_offset = 0x340
    action_table_offset = 0x3C0
    thrown_hitbox_offset = 0x470
    action_name_offset = 0x480
    common_parts_offset = 0x4A0
    fighter_block[ftdata_offset : ftdata_offset + 4] = be32(attrs_offset)
    fighter_block[ftdata_offset + 4 : ftdata_offset + 8] = be32(attrs_end)
    fighter_block[ftdata_offset + 0x08 : ftdata_offset + 0x0C] = be32(common_parts_offset)
    fighter_block[ftdata_offset + 0x0C : ftdata_offset + 0x10] = be32(action_table_offset)
    fighter_block[ftdata_offset + 0x30 : ftdata_offset + 0x34] = be32(hurtbox_table_offset)
    fighter_block[ftdata_offset + 0x34 : ftdata_offset + 0x38] = be32(thrown_hitbox_offset)
    fighter_block[ftdata_offset + 0x44 : ftdata_offset + 0x48] = be32(ecb_source_offset)
    for field_offset in (0x18, 0x1C, 0x30, 0x34, 0x3C, 0x40, 0x5C, 0x60, 0x64, 0x68, 0x6C, 0x70):
        put_f32(fighter_block, attrs_offset + field_offset, 1.0)
    for field_offset in (0xE8, 0xEC, 0xF0, 0xF4, 0xF8, 0x110):
        put_f32(fighter_block, attrs_offset + field_offset, 1.0)
    fighter_block[ecb_source_offset : ecb_source_offset + 0x0C] = struct.pack(
        ">hhhhhh", 0, 0, 0, 0, 0, 0
    )
    put_f32(fighter_block, ecb_source_offset + 0x0C, 0.0)
    put_f32(fighter_block, ecb_source_offset + 0x10, 0.0)
    put_f32(fighter_block, ecb_source_offset + 0x14, 0.0)
    put_f32(fighter_block, ecb_source_offset + 0x18, 0.0)
    put_i32(fighter_block, hurtbox_table_offset, 1)
    fighter_block[hurtbox_table_offset + 0x04 : hurtbox_table_offset + 0x08] = be32(
        hurtbox_inits_offset
    )
    fighter_block[hurtbox_inits_offset : hurtbox_inits_offset + 0x0C] = (
        be32(0) + be32(0) + be32(1)
    )
    put_vec3(fighter_block, hurtbox_inits_offset + 0x0C, (0.0, 0.0, 0.0))
    put_vec3(fighter_block, hurtbox_inits_offset + 0x18, (1.0, 0.0, 0.0))
    put_f32(fighter_block, hurtbox_inits_offset + 0x24, 1.0)
    fighter_block[action_name_offset : action_name_offset + 17] = b"AttackLw3_action\0"
    fighter_block[action_table_offset : action_table_offset + 0x18] = (
        be32(action_name_offset) + be32(0) + be32(0) + be32(0) + be32(0) + be32(0)
    )
    fighter_block[thrown_hitbox_offset : thrown_hitbox_offset + 0x04] = be32(1)
    put_f32(fighter_block, thrown_hitbox_offset + 0x04, 1.0)
    fighter_block[common_parts_offset : common_parts_offset + 0x18] = (
        be32(1)
        + be32(0x120)
        + be32(0)
        + be32(0x140)
        + bytes([57, 61, 39, 10, 16, 0, 0, 0])
    )
    (raw_dir / "PlMs.dat").write_bytes(make_dat("ftDataMars", fighter_block, ftdata_offset))

    skeleton_block = bytearray(0x100)
    put_joint(
        skeleton_block,
        0x40,
        flags=0,
        rotation=(0.0, 0.0, 0.0),
        scale=(1.0, 1.0, 1.0),
        position=(0.0, 0.0, 0.0),
    )
    (raw_dir / "PlMsNr.dat").write_bytes(
        make_dat("PlyMars5K_Share_joint", skeleton_block, 0x40)
    )

    written = extract_resources(raw_dir, out_dir, ("marth",))

    written_names = {path.name for path in written}
    assert "marth_profile.json" in written_names
    assert "marth_hurtbox_inits.json" in written_names
    assert "marth_costume_skeleton.json" in written_names
    profile = (out_dir / "marth_profile.json").read_text(encoding="utf-8")
    assert '"source_character": "marth"' in profile
    skeleton = (out_dir / "marth_costume_skeleton.json").read_text(encoding="utf-8")
    assert "PlyMars5K_Share_joint" in skeleton


def test_extract_resources_writes_declared_character_special_attrs(tmp_path, monkeypatch):
    raw_dir = tmp_path / "raw"
    out_dir = tmp_path / "extracted"
    raw_dir.mkdir()

    spec = CharacterResourceSpec(
        id="test",
        output_stem="test_character",
        data_dat="Test.dat",
        action_dat="TestAJ.dat",
        neutral_costume_dat="TestNr.dat",
        ft_data_symbol="ftDataTest",
        neutral_joint_root="Test_joint",
        special_attr_fields=(melee_resources.Field("special_x", "special_x", 0, "source_f32"),),
    )
    (raw_dir / spec.data_dat).write_bytes(b"fighter")

    monkeypatch.setattr(melee_resources, "character_resource_spec", lambda _character: spec)
    monkeypatch.setattr(
        melee_resources, "extract_character_profile_from_dat", lambda *_args: {}
    )
    monkeypatch.setattr(
        melee_resources,
        "extract_character_special_attrs_from_dat",
        lambda *_args: {"fields": {"special_x": {"raw": 1.0}}},
    )
    monkeypatch.setattr(
        melee_resources, "extract_character_ecb_source_from_dat", lambda *_args: {}
    )
    monkeypatch.setattr(
        melee_resources,
        "extract_character_hurtbox_inits_from_dat",
        lambda *_args: {"hurtboxes": []},
    )
    monkeypatch.setattr(
        melee_resources,
        "extract_character_common_parts_from_dat",
        lambda *_args, **_kwargs: {"transn_part": 1, "xrotn_part": 2, "hipn_part": 4, "transn2_part": 52},
    )

    written = extract_resources(raw_dir, out_dir, ("test",))

    written_names = {path.name for path in written}
    assert "test_character_special_attrs.json" in written_names
    assert '"special_x"' in (out_dir / "test_character_special_attrs.json").read_text(
        encoding="utf-8"
    )


def test_extract_resources_writes_plco_fighter_parts_tables(tmp_path, monkeypatch):
    raw_dir = tmp_path / "raw"
    out_dir = tmp_path / "extracted"
    raw_dir.mkdir()
    (raw_dir / "PlCo.dat").write_bytes(b"plco")

    monkeypatch.setattr(melee_resources, "extract_common_data_from_plco", lambda *_args: {})
    monkeypatch.setattr(
        melee_resources,
        "extract_fighter_parts_tables_from_plco",
        lambda *_args: {"tables": {"2": {"source_name": "FTKIND_CAPTAIN"}}},
    )

    written = extract_resources(raw_dir, out_dir, ())

    written_names = {path.name for path in written}
    assert "fighter_parts_tables.json" in written_names
    assert '"FTKIND_CAPTAIN"' in (out_dir / "fighter_parts_tables.json").read_text(
        encoding="utf-8"
    )


def test_extract_resources_does_not_write_hurtbox_sample_cache_by_default(
    tmp_path, monkeypatch
):
    raw_dir = tmp_path / "raw"
    out_dir = tmp_path / "extracted"
    raw_dir.mkdir()

    spec = CharacterResourceSpec(
        id="test",
        output_stem="test_character",
        data_dat="Test.dat",
        action_dat="TestAJ.dat",
        neutral_costume_dat="TestNr.dat",
        ft_data_symbol="ftDataTest",
        neutral_joint_root="Test_joint",
        derived_sample_action_state_ids=(1,),
    )
    (raw_dir / spec.data_dat).write_bytes(b"fighter")
    (raw_dir / spec.action_dat).write_bytes(b"action")
    (raw_dir / spec.neutral_costume_dat).write_bytes(b"costume")

    monkeypatch.setattr(melee_resources, "character_resource_spec", lambda _character: spec)
    monkeypatch.setattr(
        melee_resources, "extract_character_profile_from_dat", lambda *_args: {}
    )
    monkeypatch.setattr(
        melee_resources, "extract_character_ecb_source_from_dat", lambda *_args: {}
    )
    monkeypatch.setattr(
        melee_resources,
        "extract_character_hurtbox_inits_from_dat",
        lambda *_args: {"hurtboxes": []},
    )
    monkeypatch.setattr(
        melee_resources,
        "extract_character_common_parts_from_dat",
        lambda *_args, **_kwargs: {"transn_part": 1, "xrotn_part": 2, "hipn_part": 4, "transn2_part": 52},
    )
    monkeypatch.setattr(
        melee_resources,
        "extract_character_action_animation_table",
        lambda *_args, **_kwargs: {"actions": []},
    )
    monkeypatch.setattr(
        melee_resources,
        "extract_character_costume_skeleton_from_dat",
        lambda *_args: {"joints": []},
    )
    monkeypatch.setattr(
        melee_resources,
        "extract_captain_action_ecb_samples",
        lambda *_args, **_kwargs: {"actions": []},
    )
    monkeypatch.setattr(
        melee_resources,
        "extract_captain_action_hurtbox_samples",
        lambda *_args, **_kwargs: {"actions": []},
    )

    written = extract_resources(raw_dir, out_dir, ("test",))

    written_names = {path.name for path in written}
    assert "test_character_action_ecb_samples.json" in written_names
    assert "test_character_action_hurtbox_samples.json" not in written_names
    assert not (out_dir / "test_character_action_hurtbox_samples.json").exists()
