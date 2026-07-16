from tools.generate_falcon_ecb_rust import (
    build_coverage_payload,
    render_generated_rust,
    render_live_jobj_payload,
)


def test_live_jobj_payload_clears_transn_per_action_anim_flags():
    rust = "\n".join(
        render_live_jobj_payload(
            {
                "joints": [
                    {
                        "parent_index": None,
                        "flags_raw": "0x00000000",
                        "rotation_raw": {"x": 0.0, "y": 0.0, "z": 0.0},
                        "scale_raw": {"x": 1.0, "y": 1.0, "z": 1.0},
                        "position_raw": {"x": 0.0, "y": 0.0, "z": 0.0},
                    },
                    {
                        "parent_index": 0,
                        "flags_raw": "0x00000000",
                        "rotation_raw": {"x": 0.0, "y": 0.0, "z": 0.0},
                        "scale_raw": {"x": 1.0, "y": 1.0, "z": 1.0},
                        "position_raw": {"x": 0.0, "y": 0.0, "z": 0.0},
                    },
                ],
                "ecb_source": {
                    "joint_indices": [1, 1, 1, 1, 1, 1],
                    "side_midpoint_offset_raw": 0.0,
                },
                "model_scaling": 1.0,
                "transn_joint_index": 1,
                "ftanim_copy_joints": [],
                "actions": {
                    36: {
                        "clear_transn_after_anim": False,
                        "tracks": [],
                    },
                    307: {
                        "clear_transn_after_anim": True,
                        "tracks": [],
                    },
                },
            }
        )
    )

    assert "clear_transn_after_anim: bool" in rust
    assert (
        "const FALCON_FIGA_ACTION_36: FalconFigaAction = FalconFigaAction { "
        "clear_transn_after_anim: false, end_frame: f32::from_bits(0x00000000), "
        "rewind_frame: f32::from_bits(0x00000000), aobj_flags: 0x00000000, "
        "tracks: &FALCON_FIGA_TRACKS_ACTION_36 "
        "};"
    ) in rust
    assert (
        "const FALCON_FIGA_ACTION_307: FalconFigaAction = FalconFigaAction { "
        "clear_transn_after_anim: true, end_frame: f32::from_bits(0x00000000), "
        "rewind_frame: f32::from_bits(0x00000000), aobj_flags: 0x00000000, "
        "tracks: &FALCON_FIGA_TRACKS_ACTION_307 "
        "};"
    ) in rust
    assert "if action.clear_transn_after_anim {" in rust


def test_live_jobj_payload_emits_exact_aobj_descriptor_metadata():
    rust = "\n".join(
        render_live_jobj_payload(
            {
                "joints": [],
                "ecb_source": {
                    "joint_indices": [0, 0, 0, 0, 0, 0],
                    "side_midpoint_offset_raw": 0.0,
                },
                "model_scaling": 1.0,
                "transn_joint_index": 0,
                "ftanim_copy_joints": [],
                "actions": {
                    20: {
                        "clear_transn_after_anim": False,
                        "end_frame": 29.5,
                        "rewind_frame": 0.0,
                        "aobj_flags": 0x20000000,
                        "tracks": [],
                    }
                },
            }
        )
    )

    assert "end_frame: f32" in rust
    assert "rewind_frame: f32" in rust
    assert "aobj_flags: u32" in rust
    assert "end_frame: f32::from_bits(0x41ec0000)" in rust
    assert "rewind_frame: f32::from_bits(0x00000000)" in rust
    assert "aobj_flags: 0x20000000" in rust
    assert "pub(crate) fn falcon_aobj_descriptor_for_action_table_id" in rust
    assert "SourceAObjDescriptor" in rust



def test_live_jobj_payload_emits_costume_skeleton_selector():
    neutral = {
        "parent_index": None,
        "flags_raw": "0x00000000",
        "rotation_raw": {"x": 0.0, "y": 0.0, "z": 0.0},
        "scale_raw": {"x": 1.0, "y": 1.0, "z": 1.0},
        "position_raw": {"x": 0.0, "y": 0.0, "z": 0.0},
    }
    blue = {**neutral, "position_raw": {"x": 0.0, "y": 0.25, "z": 0.0}}
    rust = "\n".join(
        render_live_jobj_payload(
            {
                "joints": [neutral],
                "costume_joints": [[neutral], [blue]],
                "ecb_source": {
                    "joint_indices": [0, 0, 0, 0, 0, 0],
                    "side_midpoint_offset_raw": 0.0,
                },
                "model_scaling": 1.0,
                "transn_joint_index": 0,
                "ftanim_copy_joints": [],
                "actions": {},
            }
        )
    )

    assert "const FALCON_SOURCE_COSTUME_JOINTS: [[FalconSourceJoint; 1]; 2]" in rust
    assert "fn falcon_source_joints(costume_index: u8)" in rust
    assert "falcon_source_joints(costume_index).iter()" in rust


def test_live_jobj_payload_keeps_coll_data_minimums_in_fighter_scale_space():
    rust = "\n".join(
        render_live_jobj_payload(
            {
                "joints": [],
                "ecb_source": {
                    "joint_indices": [0, 0, 0, 0, 0, 0],
                    "side_midpoint_offset_raw": 0.0,
                    "min_height_raw": 10.0,
                    "min_width_raw": 10.0,
                },
                "model_scaling": 0.97,
                "transn_joint_index": 0,
                "ftanim_copy_joints": [],
                "actions": {},
            }
        )
    )

    assert "const FALCON_ECB_MIN_HEIGHT: f32 = f32::from_bits(0x41200000);" in rust
    assert "const FALCON_ECB_MIN_WIDTH: f32 = f32::from_bits(0x41200000);" in rust

def test_generated_falcon_ecb_rust_maps_motion_states_to_action_samples():
    samples = {
        "actions": [
            {
                "action_state_id": 12,
                "name": "PlyCaptain5K_Share_ACTION_Dash_figatree",
                "frames": [
                    {
                        "frame": 0,
                        "top_milli": {"x": 0, "y": 10},
                        "right_milli": {"x": 2, "y": 5},
                        "bottom_milli": {"x": 0, "y": 0},
                        "left_milli": {"x": -2, "y": 5},
                    }
                ],
            },
            {
                "action_state_id": 15,
                "name": "PlyCaptain5K_Share_ACTION_Landing_figatree",
                "frames": [
                    {
                        "frame": 0,
                        "top_milli": {"x": 0, "y": 11},
                        "right_milli": {"x": 2, "y": 5},
                        "bottom_milli": {"x": 0, "y": 0},
                        "left_milli": {"x": -2, "y": 5},
                    }
                ],
            },
            {
                "action_state_id": 44,
                "name": "PlyCaptain5K_Share_ACTION_EscapeAir_figatree",
                "frames": [
                    {
                        "frame": 0,
                        "top_milli": {"x": 0, "y": 12},
                        "right_milli": {"x": 3, "y": 6},
                        "bottom_milli": {"x": 0, "y": 1},
                        "left_milli": {"x": -3, "y": 6},
                    }
                ],
            },
            {
                "action_state_id": 23,
                "name": "PlyCaptain5K_Share_ACTION_FallAerial_figatree",
                "frames": [
                    {
                        "frame": 0,
                        "top_milli": {"x": 0, "y": 14},
                        "right_milli": {"x": 4, "y": 7},
                        "bottom_milli": {"x": 0, "y": 2},
                        "left_milli": {"x": -4, "y": 7},
                    }
                ],
            },
            {
                "action_state_id": 27,
                "name": "PlyCaptain5K_Share_ACTION_FallSpecialF_figatree",
                "frames": [
                    {
                        "frame": 0,
                        "top_milli": {"x": 0, "y": 16},
                        "right_milli": {"x": 5, "y": 8},
                        "bottom_milli": {"x": 0, "y": 3},
                        "left_milli": {"x": -5, "y": 8},
                    }
                ],
            },
            {
                "action_state_id": 28,
                "name": "PlyCaptain5K_Share_ACTION_FallSpecialB_figatree",
                "frames": [
                    {
                        "frame": 0,
                        "top_milli": {"x": 0, "y": 18},
                        "right_milli": {"x": 6, "y": 9},
                        "bottom_milli": {"x": 0, "y": 4},
                        "left_milli": {"x": -6, "y": 9},
                    }
                ],
            },
            {
                "action_state_id": 36,
                "name": "PlyCaptain5K_Share_ACTION_Landing_figatree",
                "frames": [
                    {
                        "frame": 0,
                        "top_milli": {"x": 0, "y": 20},
                        "right_milli": {"x": 7, "y": 10},
                        "bottom_milli": {"x": 0, "y": 0},
                        "left_milli": {"x": -7, "y": 10},
                    }
                ],
            },
            {
                "action_state_id": 37,
                "name": "PlyCaptain5K_Share_ACTION_GuardOn_figatree",
                "frames": [
                    {
                        "frame": 0,
                        "top_milli": {"x": 0, "y": 21},
                        "right_milli": {"x": 7, "y": 10},
                        "bottom_milli": {"x": 0, "y": 0},
                        "left_milli": {"x": -7, "y": 10},
                    }
                ],
            },
            {
                "action_state_id": 73,
                "name": "PlyCaptain5K_Share_ACTION_LandingAirN_figatree",
                "frames": [
                    {
                        "frame": 0,
                        "top_milli": {"x": 0, "y": 22},
                        "right_milli": {"x": 8, "y": 11},
                        "bottom_milli": {"x": 0, "y": 1},
                        "left_milli": {"x": -8, "y": 11},
                    }
                ],
            },
            {
                "action_state_id": 74,
                "name": "PlyCaptain5K_Share_ACTION_LandingAirF_figatree",
                "frames": [
                    {
                        "frame": 0,
                        "top_milli": {"x": 0, "y": 24},
                        "right_milli": {"x": 9, "y": 12},
                        "bottom_milli": {"x": 0, "y": 2},
                        "left_milli": {"x": -9, "y": 12},
                    }
                ],
            },
            {
                "action_state_id": 75,
                "name": "PlyCaptain5K_Share_ACTION_LandingAirB_figatree",
                "frames": [
                    {
                        "frame": 0,
                        "top_milli": {"x": 0, "y": 26},
                        "right_milli": {"x": 10, "y": 13},
                        "bottom_milli": {"x": 0, "y": 3},
                        "left_milli": {"x": -10, "y": 13},
                    }
                ],
            },
            {
                "action_state_id": 76,
                "name": "PlyCaptain5K_Share_ACTION_LandingAirHi_figatree",
                "frames": [
                    {
                        "frame": 0,
                        "top_milli": {"x": 0, "y": 28},
                        "right_milli": {"x": 11, "y": 14},
                        "bottom_milli": {"x": 0, "y": 4},
                        "left_milli": {"x": -11, "y": 14},
                    }
                ],
            },
            {
                "action_state_id": 77,
                "name": "PlyCaptain5K_Share_ACTION_LandingAirLw_figatree",
                "frames": [
                    {
                        "frame": 0,
                        "top_milli": {"x": 0, "y": 30},
                        "right_milli": {"x": 12, "y": 15},
                        "bottom_milli": {"x": 0, "y": 5},
                        "left_milli": {"x": -12, "y": 15},
                    }
                ],
            },
            {
                "action_state_id": 238,
                "name": "PlyCaptain5K_Share_ACTION_Entry_figatree",
                "frames": [
                    {
                        "frame": 0,
                        "top_milli": {"x": 0, "y": 32},
                        "right_milli": {"x": 13, "y": 16},
                        "bottom_milli": {"x": 0, "y": 6},
                        "left_milli": {"x": -13, "y": 16},
                    }
                ],
            },
            {
                "action_state_id": 216,
                "name": "PlyCaptain5K_Share_ACTION_CliffCatch_figatree",
                "frames": [
                    {
                        "frame": 0,
                        "top_milli": {"x": 0, "y": 42},
                        "right_milli": {"x": 18, "y": 21},
                        "bottom_milli": {"x": 0, "y": 11},
                        "left_milli": {"x": -18, "y": 21},
                    }
                ],
            },
            {
                "action_state_id": 217,
                "name": "PlyCaptain5K_Share_ACTION_CliffWait1_figatree",
                "frames": [
                    {
                        "frame": 0,
                        "top_milli": {"x": 0, "y": 44},
                        "right_milli": {"x": 19, "y": 22},
                        "bottom_milli": {"x": 0, "y": 12},
                        "left_milli": {"x": -19, "y": 22},
                    }
                ],
            },
            {
                "action_state_id": 303,
                "name": "PlyCaptain5K_Share_ACTION_SpecialSStart_figatree",
                "frames": [
                    {
                        "frame": 0,
                        "top_milli": {"x": 0, "y": 34},
                        "right_milli": {"x": 14, "y": 17},
                        "bottom_milli": {"x": 0, "y": 7},
                        "left_milli": {"x": -14, "y": 17},
                    }
                ],
            },
            {
                "action_state_id": 304,
                "name": "PlyCaptain5K_Share_ACTION_SpecialS_figatree",
                "frames": [
                    {
                        "frame": 0,
                        "top_milli": {"x": 0, "y": 36},
                        "right_milli": {"x": 15, "y": 18},
                        "bottom_milli": {"x": 0, "y": 8},
                        "left_milli": {"x": -15, "y": 18},
                    }
                ],
            },
            {
                "action_state_id": 305,
                "name": "PlyCaptain5K_Share_ACTION_SpecialAirSStart_figatree",
                "frames": [
                    {
                        "frame": 0,
                        "top_milli": {"x": 0, "y": 38},
                        "right_milli": {"x": 16, "y": 19},
                        "bottom_milli": {"x": 0, "y": 9},
                        "left_milli": {"x": -16, "y": 19},
                    }
                ],
            },
            {
                "action_state_id": 306,
                "name": "PlyCaptain5K_Share_ACTION_SpecialAirS_figatree",
                "frames": [
                    {
                        "frame": 0,
                        "top_milli": {"x": 0, "y": 40},
                        "right_milli": {"x": 17, "y": 20},
                        "bottom_milli": {"x": 0, "y": 10},
                        "left_milli": {"x": -17, "y": 20},
                    }
                ],
            },
        ]
    }

    for action in samples["actions"]:
        for frame in action["frames"]:
            for point in ("top", "right", "bottom", "left"):
                milli = frame[f"{point}_milli"]
                frame[f"{point}_raw"] = {
                    "x": milli["x"] / 1000.0,
                    "y": milli["y"] / 1000.0,
                }
    samples["actions"][0]["frames"][0]["right_raw"] = {
        "x": 2.125,
        "y": 5.25,
    }

    rust = render_generated_rust(samples)

    assert "pub(crate) const FALCON_ECB_MAPPED_ACTION_COUNT: usize = 20;" in rust
    assert "const FALCON_ECB_ACTION_12" not in rust
    assert "const FALCON_SOURCE_ECB_ACTION_12" not in rust
    assert "fn falcon_ecb_sample_count_for_motion_state" in rust
    assert "MotionState::Guard => Some(370)" not in rust
    assert "SourceFighterEcb" in rust
    assert (
        "pub(crate) fn falcon_has_source_ecb_jobj_for_motion_state" in rust
    )
    assert "mut p0: f32" in rust
    assert "else { *d0 = 0.0; p0 = p1; }" in rust
    assert "length: u16" in rust
    assert "if pos >= track.length as usize { state = 6; continue; }" in rust
    assert "const FALCON_MODEL_SCALE: f32 = f32::from_bits(" in rust
    assert "srts[0].scale = FALCON_TOPN_COLLISION_SCALE;" in rust
    assert "const FALCON_TRANSN_JOINT_INDEX: usize = 1;" in rust
    assert "srts[FALCON_TRANSN_JOINT_INDEX].translation = SourceVec3Gen::default();" in rust
    assert "MotionState::Dash => Some(1)" in rust
    assert "MotionState::KneeBend => Some(1)" in rust
    assert "MotionState::EscapeAir => Some(1)" in rust
    assert "MotionState::FallAerial => Some(1)" in rust
    assert "MotionState::FallSpecialF => Some(1)" in rust
    assert "MotionState::FallSpecialB => Some(1)" in rust
    assert "MotionState::LandingFallSpecial => Some(1)" in rust
    assert "MotionState::GuardReflect => Some(1)" in rust
    assert "MotionState::LandingAirN => Some(1)" in rust
    assert "MotionState::LandingAirF => Some(1)" in rust
    assert "MotionState::LandingAirB => Some(1)" in rust
    assert "MotionState::LandingAirHi => Some(1)" in rust
    assert "MotionState::LandingAirLw => Some(1)" in rust
    assert "MotionState::Entry => Some(1)" in rust
    assert "MotionState::EntryStart => Some(1)" in rust
    assert "MotionState::EntryEnd => Some(1)" in rust
    assert "MotionState::CliffCatch => Some(1)" in rust
    assert "MotionState::CliffWait => Some(1)" in rust
    assert "MotionState::SpecialSStart => Some(1)" in rust
    assert "MotionState::SpecialS => Some(1)" in rust
    assert "MotionState::SpecialAirSStart => Some(1)" in rust
    assert "MotionState::SpecialAirS => Some(1)" in rust

    coverage = build_coverage_payload(samples)
    assert "KneeBend" not in coverage["unmapped_derived_motion_states"]
    assert "GuardReflect" not in coverage["unmapped_derived_motion_states"]
    assert "Entry" not in coverage["unmapped_derived_motion_states"]
    assert "EntryStart" not in coverage["unmapped_derived_motion_states"]
    assert "EntryEnd" not in coverage["unmapped_derived_motion_states"]
    assert coverage["unmapped_derived_motion_states"] == []
    assert coverage["special_action_bindings"][0] == {
        "motion_state": "SpecialN",
        "runtime_action_state_id": 347,
        "source_action_table_id": 301,
        "source_action_key": "SpecialN",
        "total_frames": 100,
    }
    assert coverage["special_action_bindings"][-1] == {
        "motion_state": None,
        "runtime_action_state_id": 363,
        "source_action_table_id": 317,
        "source_action_key": "SpecialHiThrow",
        "total_frames": 60,
    }
    assert sorted(coverage.keys()) == [
        "id",
        "mapped_action_count",
        "mapped_motion_state_count",
        "mapped_motion_states",
        "missing_sampled_mappings",
        "source",
        "special_action_bindings",
        "title",
        "unmapped_derived_motion_states",
    ]


def test_generated_falcon_ecb_rust_maps_common_cliff_option_samples():
    samples = {
        "actions": [
            {"action_state_id": 219, "frames": [{"frame": 0}] * 59},
            {"action_state_id": 220, "frames": [{"frame": 0}] * 33},
            {"action_state_id": 221, "frames": [{"frame": 0}] * 69},
            {"action_state_id": 222, "frames": [{"frame": 0}] * 55},
            {"action_state_id": 223, "frames": [{"frame": 0}] * 79},
            {"action_state_id": 224, "frames": [{"frame": 0}] * 49},
            {"action_state_id": 225, "frames": [{"frame": 0}] * 19},
            {"action_state_id": 226, "frames": [{"frame": 0}] * 36},
            {"action_state_id": 227, "frames": [{"frame": 0}] * 12},
            {"action_state_id": 228, "frames": [{"frame": 0}] * 33},
        ]
    }

    rust = render_generated_rust(samples)

    assert "MotionState::CliffClimbSlow => Some(59)" in rust
    assert "MotionState::CliffClimbQuick => Some(33)" in rust
    assert "MotionState::CliffAttackSlow => Some(69)" in rust
    assert "MotionState::CliffAttackQuick => Some(55)" in rust
    assert "MotionState::CliffEscapeSlow => Some(79)" in rust
    assert "MotionState::CliffEscapeQuick => Some(49)" in rust
    assert "MotionState::CliffJumpSlow1 => Some(19)" in rust
    assert "MotionState::CliffJumpSlow2 => Some(36)" in rust
    assert "MotionState::CliffJumpQuick1 => Some(12)" in rust
    assert "MotionState::CliffJumpQuick2 => Some(33)" in rust
