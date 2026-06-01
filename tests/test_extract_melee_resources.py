import struct
from pathlib import Path

from tools.extract_melee_resources import (
    DatExtractError,
    PROJECT_ROOT,
    compute_ecb_from_jobj_pose,
    extract_action_script_cmd_var_events,
    extract_captain_costume_skeleton_from_plcanr,
    extract_captain_action_animation_table,
    extract_captain_action_ecb_samples,
    extract_captain_ecb_source_from_plca,
    extract_captain_profile_from_plca,
    extract_common_data_from_plco,
    extract_figatree_summary,
    extract_file_from_gamecube_iso,
    extract_raw_files_from_gamecube_iso,
    sample_fobj_value,
    sample_figatree_node_tracks,
    sample_figatree_skeleton_pose,
)


def be32(value):
    return value.to_bytes(4, "big")


def put_f32(data, offset, value):
    data[offset : offset + 4] = struct.pack(">f", value)


def put_i32(data, offset, value):
    data[offset : offset + 4] = struct.pack(">i", value)


def make_dat(root_name, data_block, root_data_offset):
    header = bytearray(0x20)
    header[0x00:0x04] = be32(0x20 + len(data_block))
    header[0x04:0x08] = be32(len(data_block))
    header[0x08:0x0C] = be32(0)
    header[0x0C:0x10] = be32(1)
    header[0x10:0x14] = be32(0)
    root_table = bytearray()
    root_table += be32(root_data_offset)
    root_table += be32(0)
    string_table = root_name.encode("ascii") + b"\0"
    return bytes(header + data_block + root_table + string_table)


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
    put_f32(data_block, common_offset + 0x25C, -0.62)
    put_i32(data_block, common_offset + 0x2A0, 2)
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
    put_f32(data_block, common_offset + 0x42C, 1.25)
    put_f32(data_block, common_offset + 0x430, 2.0)
    put_f32(data_block, common_offset + 0x444, 0.18)
    put_f32(data_block, common_offset + 0x448, 0.42)
    put_f32(data_block, common_offset + 0x464, 0.63)
    put_f32(data_block, common_offset + 0x468, 5.0)
    put_f32(data_block, common_offset + 0x46C, -1.25)
    put_f32(data_block, common_offset + 0x470, 6.0)
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
    assert extracted["fields"]["escapeair_force"]["milli"] == 3100
    assert extracted["fields"]["escapeair_decay_milli"]["milli"] == 915
    assert extracted["fields"]["escapeair_landing_lag_ticks"]["ticks"] == 10
    assert extracted["fields"]["high_speed_ground_friction_multiplier_milli"]["milli"] == 2000
    assert extracted["fields"]["turn_run_x"]["stick_byte"] == -44
    assert extracted["fields"]["run_brake_animation_pause_velocity_milli"]["milli"] == 1250
    assert extracted["fields"]["turn_run_x"]["source_name"] == "x38_someLStickXThreshold"
    assert extracted["fields"]["fall_animation_drift_threshold_milli"]["milli"] == 180
    assert extracted["fields"]["fall_animation_blend_milli"]["milli"] == 420
    assert extracted["fields"]["guard_reflect_input_window"]["ticks"] == 2
    assert extracted["fields"]["entry_start_ticks"]["ticks"] == 30
    assert extracted["fields"]["entry_end_ticks"]["ticks"] == 31
    assert extracted["fields"]["entry_initial_scale_y_milli"]["milli"] == 10
    assert extracted["fields"]["entry_collision_landing_lag_ticks"]["ticks"] == 120


def test_extract_captain_profile_from_plca_uses_ftdata_attribute_range():
    data_block = bytearray(0x400)
    ftdata_offset = 0x60
    attrs_offset = 0x100
    attrs_end = 0x280
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

    dat = make_dat("ftDataCaptain", data_block, ftdata_offset)
    extracted = extract_captain_profile_from_plca(dat, source_path=Path("PlCa.dat"))

    assert extracted["source"]["symbol"] == "ftDataCaptain"
    assert extracted["source"]["ftco_dat_attrs_offset"] == attrs_offset
    assert extracted["source"]["ftco_dat_attrs_len"] == attrs_end - attrs_offset
    assert extracted["fields"]["traction_per_tick"]["milli"] == 80
    assert extracted["fields"]["dash_initial_velocity"]["milli"] == 2000
    assert extracted["fields"]["max_run_brake_frames"]["ticks"] == 8
    assert extracted["fields"]["ground_max_horizontal_velocity"]["milli"] == 2350
    assert extracted["fields"]["air_drift_max"]["milli"] == 1120
    assert extracted["fields"]["landingairn_lag"]["ticks"] == 15
    assert extracted["fields"]["landingairf_lag"]["ticks"] == 19
    assert extracted["fields"]["landingairb_lag"]["ticks"] == 18
    assert extracted["fields"]["landingairhi_lag"]["ticks"] == 15
    assert extracted["fields"]["landingairlw_lag"]["ticks"] == 24
    assert extracted["fields"]["entry_platform_offset_y"]["milli"] == 1647


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


def test_compute_ecb_from_jobj_pose_matches_mpcoll_jobj_reduction():
    pose = {
        "joints": [
            {"world_position_raw": {"x": 0.0, "y": 0.0, "z": 0.0}},
            {"world_position_raw": {"x": 3.0, "y": 10.0, "z": 0.0}},
            {"world_position_raw": {"x": -5.0, "y": 4.0, "z": 0.0}},
            {"world_position_raw": {"x": 2.0, "y": 6.0, "z": 0.0}},
            {"world_position_raw": {"x": -1.0, "y": 8.0, "z": 0.0}},
            {"world_position_raw": {"x": 1.0, "y": 2.0, "z": 0.0}},
        ]
    }
    ecb_source = {
        "joint_indices": [0, 1, 2, 3, 4, 5],
        "side_midpoint_offset_raw": 0.25,
    }

    ecb = compute_ecb_from_jobj_pose(pose, ecb_source, flags=6)

    assert ecb["top"] == {"x": 0.0, "y": 10.0}
    assert ecb["bottom"] == {"x": 0.0, "y": 0.0}
    assert ecb["right"] == {"x": 4.0, "y": 5.25}
    assert ecb["left"] == {"x": -4.0, "y": 5.25}
    assert ecb["bottom_milli"] == {"x": 0, "y": 0}


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

    assert samples["actions"][0]["action_state_id"] == 44
    assert samples["actions"][0]["frames"][5]["bottom_milli"] == {"x": 0, "y": 10000}
    assert samples["actions"][0]["frames"][5]["top_milli"] == {"x": 0, "y": 12000}
    assert samples["actions"][0]["frames"][5]["source_joint_indices"] == [0, 1, 0, 1, 0, 1]


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
    assert extracted["actions"][0]["action_state_id"] == 0
    assert extracted["actions"][0]["name"] == "Wait1_action"
    assert extracted["actions"][0]["figatree_root"] == "PlyCaptain5K_Share_ACTION_Wait1_figatree"
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
    script += be32(0x0800000F)  # async timer frame 15
    script += be32(0x4C000000)  # set cmd_vars[0] = 0
    script += be32(0)

    events = extract_action_script_cmd_var_events(bytes(script), 0)

    assert events == [
        {"frame": 9, "cmd_var": 1, "value": 1, "word_offset": 6},
        {"frame": 15, "cmd_var": 0, "value": 0, "word_offset": 8},
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


def test_extract_captain_profile_rejects_non_captain_roots():
    data_block = bytearray(0x400)
    dat = make_dat("ftDataFox", data_block, 0x60)

    try:
        extract_captain_profile_from_plca(dat, source_path=Path("PlCa.dat"))
    except DatExtractError as error:
        assert "ftDataCaptain" in str(error)
    else:
        raise AssertionError("expected non-Captain DAT root to be rejected")
