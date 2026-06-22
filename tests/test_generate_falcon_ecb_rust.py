import struct

from tools.generate_falcon_ecb_rust import (
    build_coverage_payload,
    render_generated_rust,
    render_live_jobj_payload,
)


def f32_bits(value: float) -> str:
    bits = struct.unpack(">I", struct.pack(">f", value))[0]
    return f"f32::from_bits(0x{bits:08x})"


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
        "clear_transn_after_anim: false, tracks: &FALCON_FIGA_TRACKS_ACTION_36 "
        "};"
    ) in rust
    assert (
        "const FALCON_FIGA_ACTION_307: FalconFigaAction = FalconFigaAction { "
        "clear_transn_after_anim: true, tracks: &FALCON_FIGA_TRACKS_ACTION_307 "
        "};"
    ) in rust
    assert "if action.clear_transn_after_anim {" in rust


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
    assert "const FALCON_ECB_ACTION_12" in rust
    assert "const FALCON_SOURCE_ECB_ACTION_12" in rust
    assert "SourceFighterEcb" in rust
    assert (
        f"right: SourceVec2 {{ x: {f32_bits(2.125)}, y: {f32_bits(5.25)} }}"
        in rust
    )
    assert (
        "pub(crate) fn falcon_source_ecb_samples_for_motion_state" in rust
    )
    assert "mut p0: f32" in rust
    assert "else { *d0 = 0.0; p0 = p1; }" in rust
    assert "length: u16" in rust
    assert "if pos >= track.length as usize { state = 6; continue; }" in rust
    assert "const FALCON_MODEL_SCALE: f32 = f32::from_bits(" in rust
    assert "srts[0].scale = FALCON_TOPN_COLLISION_SCALE;" in rust
    assert "const FALCON_TRANSN_JOINT_INDEX: usize = 1;" in rust
    assert "srts[FALCON_TRANSN_JOINT_INDEX].translation = SourceVec3Gen::default();" in rust
    assert "MotionState::Dash => Some(&FALCON_SOURCE_ECB_ACTION_12)" in rust
    assert "MotionState::Dash => Some(&FALCON_ECB_ACTION_12)" in rust
    assert "MotionState::KneeBend => Some(&FALCON_ECB_ACTION_15)" in rust
    assert "MotionState::EscapeAir => Some(&FALCON_ECB_ACTION_44)" in rust
    assert "MotionState::FallAerial => Some(&FALCON_ECB_ACTION_23)" in rust
    assert "MotionState::FallSpecialF => Some(&FALCON_ECB_ACTION_27)" in rust
    assert "MotionState::FallSpecialB => Some(&FALCON_ECB_ACTION_28)" in rust
    assert "MotionState::LandingFallSpecial => Some(&FALCON_ECB_ACTION_36)" in rust
    assert "MotionState::GuardReflect => Some(&FALCON_ECB_ACTION_37)" in rust
    assert "MotionState::LandingAirN => Some(&FALCON_ECB_ACTION_73)" in rust
    assert "MotionState::LandingAirF => Some(&FALCON_ECB_ACTION_74)" in rust
    assert "MotionState::LandingAirB => Some(&FALCON_ECB_ACTION_75)" in rust
    assert "MotionState::LandingAirHi => Some(&FALCON_ECB_ACTION_76)" in rust
    assert "MotionState::LandingAirLw => Some(&FALCON_ECB_ACTION_77)" in rust
    assert "MotionState::Entry => Some(&FALCON_ECB_ACTION_238)" in rust
    assert "MotionState::EntryStart => Some(&FALCON_ECB_ACTION_238)" in rust
    assert "MotionState::EntryEnd => Some(&FALCON_ECB_ACTION_238)" in rust
    assert "MotionState::CliffCatch => Some(&FALCON_ECB_ACTION_216)" in rust
    assert "MotionState::CliffWait => Some(&FALCON_ECB_ACTION_217)" in rust
    assert "MotionState::SpecialSStart => Some(&FALCON_ECB_ACTION_303)" in rust
    assert "MotionState::SpecialS => Some(&FALCON_ECB_ACTION_304)" in rust
    assert "MotionState::SpecialAirSStart => Some(&FALCON_ECB_ACTION_305)" in rust
    assert "MotionState::SpecialAirS => Some(&FALCON_ECB_ACTION_306)" in rust

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
