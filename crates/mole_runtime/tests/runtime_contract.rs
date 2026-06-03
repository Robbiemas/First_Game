use std::path::Path;

use mole_core::{
    step_world, FighterProfile, Frame, GameCubeButtonState, GameCubePadStatus, MeleeCommonData,
    MotionState, PlayerInput, StageProfile, Vec2, WalkSpeedBucket, World,
    UCF_DASHBACK_AMENDMENT_BIT,
};
use mole_runtime::{
    compare_slippi_export_from_match_start_with_core, compare_slippi_export_with_core,
    legacy_animation_for_motion_state, map_gamecube_pad_to_player_input, map_physical_input,
    native_replay_path, parse_wup_report, project_asset_root, slippi_core_report_path,
    trace_slippi_export_from_match_start_with_core, write_slippi_core_trace_report,
    ControllerInputTraceLog, DebugOverlay, DolphinMoleVisualProfile, FixedStepClock, FrameDebugLog,
    InputReadout, InputSource, InputTraceWriter, LegacyAnimationKey, LegacySpriteCue,
    PhysicalInput, RenderColor, RenderFrame, RenderRect, RenderScene, RenderTransform,
    ReplayCapture, SlippiCoreComparisonConfig, SlippiCoreTraceConfig, UdpRuntimeConfig,
    UdpRuntimeStats, WupInputConfig, WupInputMapper, WupPort, LEGACY_DOLPHIN_MOLE_ANIMATIONS,
};
use mole_transport::{InputPacket, PacketAcceptResult};

fn source_shield_trigger() -> u8 {
    MeleeCommonData::provisional_mole().z_shield_analog
}

fn trigger_raw_from_origin(origin: u8, offset: u8) -> u8 {
    origin.saturating_add(offset)
}

#[test]
fn fixed_step_clock_emits_one_tick_for_one_sixtieth_second() {
    let mut clock = FixedStepClock::default();

    assert_eq!(clock.add_elapsed_nanos(1_000_000_000 / 60), 1);
}

#[test]
fn fixed_step_clock_keeps_remainder_for_next_frame() {
    let mut clock = FixedStepClock::default();

    assert_eq!(clock.add_elapsed_nanos(8_000_000), 0);
    assert_eq!(clock.add_elapsed_nanos(9_000_000), 1);
}

#[test]
fn fixed_step_clock_caps_ticks_to_avoid_spiral_of_death() {
    let mut clock = FixedStepClock::default();

    assert_eq!(clock.add_elapsed_nanos(1_000_000_000), 5);
}

#[test]
fn input_source_returns_frame_indexed_inputs() {
    let mut source = ScriptedInputSource {
        input: PlayerInput::neutral().with_attack(true),
    };

    let inputs = source.poll_inputs(Frame(7));

    assert!(inputs[0].attack());
    assert_eq!(inputs[1], PlayerInput::neutral());
}

#[test]
fn runtime_step_uses_input_source_output_for_authoritative_gameplay() {
    let mut source = ScriptedInputSource {
        input: PlayerInput::neutral().with_left_stick(64, 0),
    };
    let mut world = World::for_two_players();

    let inputs = mole_runtime::step_world_from_input_source(&mut world, &mut source, Frame(0));

    assert_eq!(inputs[0].stick_x(), 64);
    assert_eq!(world.players()[0].motion_state, MotionState::WalkSlow);
}

fn minimal_slippi_export(frame_player: &str) -> String {
    format!(
        r#"{{
  "schema_version": 1,
  "source": {{"replay_path": "fixture.slp", "parser": "@slippi/slippi-js/node"}},
  "settings": {{"stage_id": 31, "players": {{"0": {{"controller_fix": "UCF"}}}}}},
  "metadata": {{"start_at": "2026-05-31T00:00:00Z", "last_frame": 0}},
  "export": {{"first_frame": 0, "last_frame": 0, "frame_count": 1}},
  "frames": [
    {{"frame": 0, "players": {{"0": {frame_player}}}}}
  ]
}}"#
    )
}

#[test]
fn slippi_core_comparison_accepts_matching_wait_frame_from_default_world() {
    let export = minimal_slippi_export(
        r#"{
          "pre": {
            "rust_player_input": {
              "stick_x": 0, "stick_y": 0, "c_stick_x": 0, "c_stick_y": 0,
              "left_trigger": 0, "right_trigger": 0, "physical_button_bits": 0
            }
          },
          "post": {
            "action_state_id": 14,
            "position": [1.5, 2.25],
            "self_induced_speeds": {"ground_x": 0.0, "air_x": 0.5, "y": -1.25}
          }
        }"#,
    );

    let comparison = compare_slippi_export_with_core(
        &export,
        SlippiCoreComparisonConfig {
            compare_players: [true, false],
            max_frames: None,
        },
    )
    .expect("fixture should parse");

    assert_eq!(comparison.frames_compared, 1);
    assert!(comparison.first_state_mismatch.is_none());
    assert!(comparison.report_markdown().contains("State mismatches: 0"));
}

#[test]
fn slippi_action_state_182_reports_guard_reflect_not_guard_off() {
    let export = minimal_slippi_export(
        r#"{
          "pre": {
            "rust_player_input": {
              "stick_x": 0, "stick_y": 0, "c_stick_x": 0, "c_stick_y": 0,
              "left_trigger": 0, "right_trigger": 0, "physical_button_bits": 0
            }
          },
          "post": {
            "action_state_id": 182,
            "self_induced_speeds": {"ground_x": 0.0, "air_x": 0.0, "y": 0.0}
          }
        }"#,
    );

    let comparison = compare_slippi_export_with_core(
        &export,
        SlippiCoreComparisonConfig {
            compare_players: [true, false],
            max_frames: None,
        },
    )
    .expect("fixture should parse");
    let mismatch = comparison
        .first_state_mismatch
        .expect("default world should not already be in GuardReflect");

    assert_eq!(mismatch.expected_slippi_state_id, 182);
    assert_eq!(mismatch.expected_motion_state, MotionState::GuardReflect);
    assert!(comparison.report_markdown().contains("GuardReflect (182)"));
}

#[test]
fn slippi_action_state_181_is_supported_as_guard_set_off() {
    let export = minimal_slippi_export(
        r#"{
          "pre": {
            "rust_player_input": {
              "stick_x": 0, "stick_y": 0, "c_stick_x": 0, "c_stick_y": 0,
              "left_trigger": 0, "right_trigger": 0, "physical_button_bits": 0
            }
          },
          "post": {
            "action_state_id": 181,
            "self_induced_speeds": {"ground_x": 0.0, "air_x": 0.0, "y": 0.0}
          }
        }"#,
    );

    let comparison = compare_slippi_export_with_core(
        &export,
        SlippiCoreComparisonConfig {
            compare_players: [true, false],
            max_frames: None,
        },
    )
    .expect("fixture should parse");

    assert_eq!(comparison.unsupported_state_count, 0);
    let mismatch = comparison
        .first_state_mismatch
        .expect("default world should not already be in GuardSetOff");
    assert_eq!(mismatch.expected_slippi_state_id, 181);
    assert_eq!(mismatch.expected_motion_state, MotionState::GuardSetOff);
    assert!(comparison.report_markdown().contains("GuardSetOff (181)"));
}

#[test]
fn slippi_common_action_state_19_reports_turn_run() {
    let export = minimal_slippi_export(
        r#"{
          "pre": {
            "rust_player_input": {
              "stick_x": 0, "stick_y": 0, "c_stick_x": 0, "c_stick_y": 0,
              "left_trigger": 0, "right_trigger": 0, "physical_button_bits": 0
            }
          },
          "post": {
            "action_state_id": 19,
            "self_induced_speeds": {"ground_x": 0.0, "air_x": 0.0, "y": 0.0}
          }
        }"#,
    );

    let comparison = compare_slippi_export_with_core(
        &export,
        SlippiCoreComparisonConfig {
            compare_players: [true, false],
            max_frames: None,
        },
    )
    .expect("fixture should parse");

    assert!(comparison.report_markdown().contains("TurnRun (19)"));
}

#[test]
fn slippi_common_action_state_22_reports_run_direct_not_run_brake() {
    let export = minimal_slippi_export(
        r#"{
          "pre": {
            "rust_player_input": {
              "stick_x": 0, "stick_y": 0, "c_stick_x": 0, "c_stick_y": 0,
              "left_trigger": 0, "right_trigger": 0, "physical_button_bits": 0
            }
          },
          "post": {
            "action_state_id": 22,
            "self_induced_speeds": {"ground_x": 0.0, "air_x": 0.0, "y": 0.0}
          }
        }"#,
    );

    let comparison = compare_slippi_export_with_core(
        &export,
        SlippiCoreComparisonConfig {
            compare_players: [true, false],
            max_frames: None,
        },
    )
    .expect("fixture should parse");
    let report = comparison.report_markdown();

    assert!(report.contains("RunDirect (22)"));
    assert!(!report.contains("RunBrake (22)"));
}

#[test]
fn slippi_common_action_state_23_reports_run_brake_not_turn_run() {
    let export = minimal_slippi_export(
        r#"{
          "pre": {
            "rust_player_input": {
              "stick_x": 0, "stick_y": 0, "c_stick_x": 0, "c_stick_y": 0,
              "left_trigger": 0, "right_trigger": 0, "physical_button_bits": 0
            }
          },
          "post": {
            "action_state_id": 23,
            "self_induced_speeds": {"ground_x": 0.0, "air_x": 0.0, "y": 0.0}
          }
        }"#,
    );

    let comparison = compare_slippi_export_with_core(
        &export,
        SlippiCoreComparisonConfig {
            compare_players: [true, false],
            max_frames: None,
        },
    )
    .expect("fixture should parse");
    let report = comparison.report_markdown();

    assert!(report.contains("RunBrake (23)"));
    assert!(!report.contains("TurnRun (23)"));
}

#[test]
fn slippi_fallspecial_directional_states_map_to_exact_motion_states() {
    let export = minimal_slippi_export(
        r#"{
          "pre": {
            "rust_player_input": {
              "stick_x": 0, "stick_y": 0, "c_stick_x": 0, "c_stick_y": 0,
              "left_trigger": 0, "right_trigger": 0, "physical_button_bits": 0
            }
          },
          "post": {
            "action_state_id": 36,
            "self_induced_speeds": {"ground_x": 0.0, "air_x": 0.0, "y": 0.0}
          }
        }"#,
    );

    let comparison = compare_slippi_export_with_core(
        &export,
        SlippiCoreComparisonConfig {
            compare_players: [true, false],
            max_frames: None,
        },
    )
    .expect("fixture should parse");
    let mismatch = comparison
        .first_state_mismatch
        .expect("default world should not already be in FallSpecialF");

    assert_eq!(mismatch.expected_slippi_state_id, 36);
    assert_eq!(mismatch.expected_motion_state, MotionState::FallSpecialF);
    assert!(comparison.report_markdown().contains("FallSpecialF (36)"));
}

#[test]
fn slippi_core_comparison_reports_first_state_mismatch() {
    let export = minimal_slippi_export(
        r#"{
          "pre": {
            "rust_player_input": {
              "stick_x": 127, "stick_y": 0, "c_stick_x": 0, "c_stick_y": 0,
              "left_trigger": 0, "right_trigger": 0, "physical_button_bits": 0
            }
          },
          "post": {
            "action_state_id": 14,
            "position": [1.5, 2.25],
            "self_induced_speeds": {"ground_x": 0.0, "air_x": 0.5, "y": -1.25}
          }
        }"#,
    );

    let comparison = compare_slippi_export_with_core(
        &export,
        SlippiCoreComparisonConfig {
            compare_players: [true, false],
            max_frames: None,
        },
    )
    .expect("fixture should parse");
    let mismatch = comparison
        .first_state_mismatch
        .expect("hard stick should diverge from expected Wait");

    assert_eq!(mismatch.frame, Frame(0));
    assert_eq!(mismatch.player_index, 0);
    assert_eq!(mismatch.expected_motion_state, MotionState::Wait);
    assert_eq!(mismatch.actual_motion_state, MotionState::Dash);
    assert_eq!(mismatch.expected_position, Vec2 { x: 1_500, y: 2_250 });
    assert_eq!(mismatch.expected_air_velocity_x, 500);
    assert_eq!(mismatch.expected_velocity_y, -1_250);
    assert!(comparison
        .report_markdown()
        .contains("First State Mismatch"));
    assert!(comparison
        .report_markdown()
        .contains("Position: Melee (1500, 2250)"));
    assert!(comparison.report_markdown().contains("Position delta:"));
    assert!(comparison
        .report_markdown()
        .contains("Ground velocity X delta:"));
    assert!(comparison
        .report_markdown()
        .contains("Air velocity X: Melee 500"));
}

#[test]
fn slippi_core_comparison_reports_first_position_drift_without_state_mismatch() {
    let export = minimal_slippi_export(
        r#"{
          "pre": {
            "action_state_id": 14,
            "position": [0.0, 0.0],
            "facing": 1.0,
            "rust_player_input": {
              "stick_x": 0, "stick_y": 0, "c_stick_x": 0, "c_stick_y": 0,
              "left_trigger": 0, "right_trigger": 0, "physical_button_bits": 0
            }
          },
          "post": {
            "action_state_id": 14,
            "position": [2.0, 0.0],
            "self_induced_speeds": {"ground_x": 0.0, "air_x": 0.0, "y": 0.0}
          }
        }"#,
    );

    let comparison = compare_slippi_export_with_core(
        &export,
        SlippiCoreComparisonConfig {
            compare_players: [true, false],
            max_frames: None,
        },
    )
    .expect("fixture should parse");
    let drift = comparison
        .first_position_drift
        .expect("matching states with divergent positions should be reported");

    assert!(comparison.first_state_mismatch.is_none());
    assert_eq!(drift.frame, Frame(0));
    assert_eq!(drift.source_frame, 0);
    assert_eq!(drift.player_index, 0);
    assert_eq!(drift.expected_motion_state, MotionState::Wait);
    assert_eq!(drift.actual_motion_state, MotionState::Wait);
    assert_eq!(drift.expected_position, Vec2 { x: 2_000, y: 0 });
    assert_eq!(drift.actual_position, Vec2 { x: 0, y: 0 });

    let report = comparison.report_markdown();
    assert!(report.contains("First Significant Position Drift"));
    assert!(report.contains("Position delta: Rust - Melee (-2000, 0)"));
}

#[test]
fn slippi_core_comparison_applies_exported_ucf_dashback_amendment() {
    let export = r#"{
  "schema_version": 1,
  "source": {"replay_path": "fixture.slp", "parser": "@slippi/slippi-js/node"},
  "settings": {"stage_id": 31, "players": {"0": {"controller_fix": "UCF"}}},
  "metadata": {"start_at": "2026-05-31T00:00:00Z", "last_frame": 1},
  "export": {"first_frame": 0, "last_frame": 1, "frame_count": 2},
  "frames": [
    {
      "frame": 0,
      "players": {
        "0": {
          "pre": {
            "rust_player_input": {
              "stick_x": -40, "stick_y": 0, "c_stick_x": 0, "c_stick_y": 0,
              "left_trigger": 0, "right_trigger": 0, "physical_button_bits": 0
            }
          },
          "post": {"action_state_id": 18, "self_induced_speeds": {"ground_x": 0.0, "air_x": 0.0, "y": 0.0}}
        }
      }
    },
    {
      "frame": 1,
      "players": {
        "0": {
          "pre": {
            "action_state_id": 18,
            "facing": 1.0,
            "rust_player_input": {
              "stick_x": -102, "stick_y": 0, "c_stick_x": 0, "c_stick_y": 0,
              "left_trigger": 0, "right_trigger": 0, "physical_button_bits": 0,
              "ucf_dashback_amendment": true
            }
          },
          "post": {"action_state_id": 20, "self_induced_speeds": {"ground_x": 0.0, "air_x": 0.0, "y": 0.0}}
        }
      }
    }
  ]
}"#;

    let comparison = compare_slippi_export_with_core(
        export,
        SlippiCoreComparisonConfig {
            compare_players: [true, false],
            max_frames: None,
        },
    )
    .expect("fixture should parse");

    assert_eq!(comparison.state_mismatch_count, 0);
    assert_eq!(comparison.ucf_players, [true, false]);
    assert_eq!(comparison.ucf_dashback_amendment_frames, [1, 0]);
    assert!(comparison
        .report_markdown()
        .contains("Replay controller fixes: P1 UCF, P2 vanilla/unknown"));
    assert!(comparison
        .report_markdown()
        .contains("UCF dashback amendment frames consumed: P1 1, P2 0"));
}

#[test]
fn slippi_core_comparison_accepts_negative_pregame_entry_frames() {
    let export = r#"{
      "schema_version": 1,
      "source": {"replay_path": "fixture.slp", "parser": "@slippi/slippi-js/node"},
      "settings": {"stage_id": 31, "players": {"0": {"controller_fix": "UCF"}}},
      "metadata": {"start_at": "2026-05-31T00:00:00Z", "last_frame": 0},
      "export": {
        "first_frame": -123,
        "last_frame": -123,
        "frame_count": 1,
        "included_negative_frames": true
      },
      "frames": [
        {"frame": -123, "players": {"0": {
          "pre": {
            "action_state_id": 322,
            "position": [-38.8, 35.2],
            "facing": 1.0,
            "rust_player_input": {
              "stick_x": 0, "stick_y": 0, "c_stick_x": 0, "c_stick_y": 0,
              "left_trigger": 0, "right_trigger": 0, "physical_button_bits": 0
            }
          },
          "post": {
            "action_state_id": 322,
            "action_state_counter": -1,
            "self_induced_speeds": {"ground_x": 0.0, "air_x": 0.0, "y": 0.0}
          }
        }}}
      ]
    }"#;

    let comparison = compare_slippi_export_with_core(
        export,
        SlippiCoreComparisonConfig {
            compare_players: [true, false],
            max_frames: None,
        },
    )
    .expect("negative Slippi pre-game frame should parse");

    assert_eq!(comparison.frames_compared, 1);
    assert_eq!(comparison.unsupported_state_count, 0);
}

#[test]
fn slippi_match_start_comparison_replays_entry_frames_without_pre_state_seeding() {
    let export = r#"{
      "schema_version": 1,
      "source": {"replay_path": "fixture.slp", "parser": "@slippi/slippi-js/node"},
      "settings": {"stage_id": 31, "players": {"0": {"controller_fix": "UCF"}}},
      "metadata": {"start_at": "2026-05-31T00:00:00Z", "last_frame": 0},
      "export": {
        "first_frame": -123,
        "last_frame": -118,
        "frame_count": 6,
        "included_negative_frames": true
      },
      "frames": [
        {"frame": -123, "players": {"0": {"pre": {"rust_player_input": {"stick_x": 0, "stick_y": 0, "c_stick_x": 0, "c_stick_y": 0, "left_trigger": 0, "right_trigger": 0, "physical_button_bits": 0}}, "post": {"action_state_id": 322, "self_induced_speeds": {"ground_x": 0.0, "air_x": 0.0, "y": 0.0}}}}},
        {"frame": -122, "players": {"0": {"pre": {"rust_player_input": {"stick_x": 0, "stick_y": 0, "c_stick_x": 0, "c_stick_y": 0, "left_trigger": 0, "right_trigger": 0, "physical_button_bits": 0}}, "post": {"action_state_id": 322, "self_induced_speeds": {"ground_x": 0.0, "air_x": 0.0, "y": 0.0}}}}},
        {"frame": -121, "players": {"0": {"pre": {"rust_player_input": {"stick_x": 0, "stick_y": 0, "c_stick_x": 0, "c_stick_y": 0, "left_trigger": 0, "right_trigger": 0, "physical_button_bits": 0}}, "post": {"action_state_id": 322, "self_induced_speeds": {"ground_x": 0.0, "air_x": 0.0, "y": 0.0}}}}},
        {"frame": -120, "players": {"0": {"pre": {"rust_player_input": {"stick_x": 0, "stick_y": 0, "c_stick_x": 0, "c_stick_y": 0, "left_trigger": 0, "right_trigger": 0, "physical_button_bits": 0}}, "post": {"action_state_id": 322, "self_induced_speeds": {"ground_x": 0.0, "air_x": 0.0, "y": 0.0}}}}},
        {"frame": -119, "players": {"0": {"pre": {"rust_player_input": {"stick_x": 0, "stick_y": 0, "c_stick_x": 0, "c_stick_y": 0, "left_trigger": 0, "right_trigger": 0, "physical_button_bits": 0}}, "post": {"action_state_id": 322, "self_induced_speeds": {"ground_x": 0.0, "air_x": 0.0, "y": 0.0}}}}},
        {"frame": -118, "players": {"0": {"pre": {"rust_player_input": {"stick_x": 0, "stick_y": 0, "c_stick_x": 0, "c_stick_y": 0, "left_trigger": 0, "right_trigger": 0, "physical_button_bits": 0}}, "post": {"action_state_id": 323, "self_induced_speeds": {"ground_x": 0.0, "air_x": 0.0, "y": 0.0}}}}}
      ]
    }"#;

    let comparison = compare_slippi_export_from_match_start_with_core(
        export,
        SlippiCoreComparisonConfig {
            compare_players: [true, false],
            max_frames: None,
        },
    )
    .expect("match-start export should parse and compare");

    assert_eq!(comparison.frames_compared, 6);
    assert_eq!(comparison.player_frames_compared[0], 6);
    assert_eq!(comparison.unsupported_state_count, 0);
    assert!(comparison.first_state_mismatch.is_none());
    assert!(comparison
        .report_markdown()
        .contains("sequential Slippi match-start"));
}

#[test]
fn slippi_match_start_report_distinguishes_core_frame_from_source_replay_frame() {
    let export = r#"{
      "schema_version": 1,
      "source": {"replay_path": "fixture.slp", "parser": "@slippi/slippi-js/node"},
      "settings": {"stage_id": 31, "players": {"0": {"controller_fix": "UCF"}}},
      "metadata": {"start_at": "2026-05-31T00:00:00Z", "last_frame": 0},
      "export": {
        "first_frame": -123,
        "last_frame": -123,
        "frame_count": 1,
        "included_negative_frames": true
      },
      "frames": [
        {"frame": -123, "players": {"0": {
          "pre": {
            "rust_player_input": {
              "stick_x": 0, "stick_y": 0, "c_stick_x": 0, "c_stick_y": 0,
              "left_trigger": 0, "right_trigger": 0, "physical_button_bits": 0
            }
          },
          "post": {
            "action_state_id": 15,
            "position": [0.0, 0.0],
            "self_induced_speeds": {"ground_x": 0.0, "air_x": 0.0, "y": 0.0}
          }
        }}}
      ]
    }"#;

    let comparison = compare_slippi_export_from_match_start_with_core(
        export,
        SlippiCoreComparisonConfig {
            compare_players: [true, false],
            max_frames: None,
        },
    )
    .expect("match-start export should parse");
    let report = comparison.report_markdown();

    assert!(report.contains("- Core frame: 0"));
    assert!(report.contains("- Slippi frame: -123"));
}

#[test]
fn slippi_core_report_path_uses_local_debug_slippi_directory() {
    let path = slippi_core_report_path(Path::new("replays/Game_Example.slp"));

    assert_eq!(
        path,
        Path::new("debug")
            .join("slippi")
            .join("Game_Example.core.report.md")
    );
}

#[test]
fn slippi_match_start_trace_reports_expected_and_actual_frame_window() {
    let export = r#"{
      "schema_version": 1,
      "source": {"replay_path": "fixture.slp", "parser": "@slippi/slippi-js/node"},
      "settings": {"stage_id": 31, "players": {"0": {"controller_fix": "UCF"}}},
      "metadata": {"start_at": "2026-05-31T00:00:00Z", "last_frame": -16},
      "export": {
        "first_frame": -17,
        "last_frame": -16,
        "frame_count": 2,
        "included_negative_frames": true
      },
      "frames": [
        {"frame": -17, "players": {"0": {
          "pre": {
            "rust_player_input": {
              "stick_x": -125, "stick_y": 0, "c_stick_x": 0, "c_stick_y": 0,
              "left_trigger": 0, "right_trigger": 0, "physical_button_bits": 2048,
              "ucf_dashback_amendment": false
            }
          },
          "post": {
            "action_state_id": 24,
            "position": [32.2, 27.2],
            "self_induced_speeds": {"ground_x": -2.14, "air_x": -2.14, "y": 0.0}
          }
        }}},
        {"frame": -16, "players": {"0": {
          "pre": {
            "rust_player_input": {
              "stick_x": -125, "stick_y": 0, "c_stick_x": 0, "c_stick_y": 0,
              "left_trigger": 0, "right_trigger": 0, "physical_button_bits": 2048,
              "ucf_dashback_amendment": false
            }
          },
          "post": {
            "action_state_id": 24,
            "position": [30.22, 27.2],
            "self_induced_speeds": {"ground_x": -1.98, "air_x": -1.98, "y": 0.0}
          }
        }}}
      ]
    }"#;

    let trace = trace_slippi_export_from_match_start_with_core(
        export,
        SlippiCoreTraceConfig {
            player_index: 0,
            source_frame_start: -17,
            source_frame_end: -17,
            max_frames: None,
        },
    )
    .expect("trace window should parse");

    assert_eq!(trace.rows.len(), 1);
    assert_eq!(trace.rows[0].source_frame, -17);
    assert_eq!(trace.rows[0].input_stick_x, -125);
    assert_eq!(trace.rows[0].input_button_bits, 2048);

    let report = trace.report_markdown();
    assert!(report.contains("# Slippi Core Trace Window"));
    assert!(report.contains("| 0 | -17 | 0 | -125 | 0 | 2048 |"));
    assert!(report.contains("KneeBend"));
}

#[test]
fn slippi_core_trace_report_writer_uses_markdown() {
    let trace = trace_slippi_export_from_match_start_with_core(
        r#"{
          "source": {"replay_path": "fixture.slp"},
          "settings": {"players": {}},
          "frames": []
        }"#,
        SlippiCoreTraceConfig {
            player_index: 0,
            source_frame_start: 0,
            source_frame_end: 0,
            max_frames: None,
        },
    )
    .expect("empty trace should parse");
    let path =
        std::env::temp_dir().join(format!("mole-slippi-core-trace-{}.md", std::process::id()));

    write_slippi_core_trace_report(&path, &trace).expect("trace report should write");

    let text = std::fs::read_to_string(&path).expect("trace report should be readable");
    let _ = std::fs::remove_file(&path);
    assert!(text.contains("# Slippi Core Trace Window"));
}

#[test]
fn render_frame_copies_world_without_owning_simulation_state() {
    let world = World::for_two_players();
    let render_frame = RenderFrame::from_world(&world);

    assert_eq!(render_frame.frame, world.frame());
    assert_eq!(render_frame.checksum, world.checksum());
    assert_eq!(
        render_frame.player_positions[0],
        world.players()[0].position
    );
}

#[test]
fn render_frame_consumes_core_snapshot_boundary() {
    let mut world = World::for_two_players();
    let inputs = [
        PlayerInput::neutral().with_left_stick(64, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &inputs);
    step_world(&mut world, Frame(1), &inputs);

    let core_snapshot = world.snapshot();
    let mut render_frame = RenderFrame::from_snapshot(core_snapshot);

    assert_eq!(render_frame.frame, core_snapshot.frame);
    assert_eq!(render_frame.player_motion_states[0], MotionState::WalkSlow);
    assert_eq!(
        render_frame.player_state_frames[0],
        core_snapshot.players[0].state_frame
    );
    assert_eq!(
        render_frame.player_animation_frames[0],
        core_snapshot.players[0].animation_frame
    );
    assert_eq!(render_frame.player_animation_frames[0], 1);
    assert_eq!(
        render_frame.player_debug_input_facts[0].walk_speed_bucket,
        WalkSpeedBucket::Middle
    );

    render_frame.player_positions[0].x += 99;

    assert_ne!(
        render_frame.player_positions[0],
        world.players()[0].position
    );
}

#[test]
fn render_frame_uses_core_source_pose_frame_for_variable_rate_walk() {
    let profile = FighterProfile {
        walk_initial_velocity: 1.0,
        walk_accel: 0.0,
        walk_max_velocity: 2.0,
        slow_walk_max_velocity: 0.1,
        mid_walk_point: 0.2,
        fast_walk_min: 0.25,
        ground_friction: 0.0,
        ..FighterProfile::falcon_like()
    };
    let common = MeleeCommonData {
        walk_middle_velocity_ratio: 0.05,
        walk_fast_velocity_ratio: 0.1,
        ..MeleeCommonData::provisional_mole()
    };
    let mut world = World::for_two_players_on_stage_with_profiles_and_common_data(
        StageProfile::battlefield_test(),
        [profile; 2],
        common,
    );
    let walk_right = [
        PlayerInput::neutral().with_left_stick(80, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &walk_right);
    step_world(&mut world, Frame(1), &walk_right);
    step_world(&mut world, Frame(2), &walk_right);
    step_world(&mut world, Frame(3), &walk_right);

    let render_frame = RenderFrame::from_world(&world);

    assert_eq!(render_frame.player_state_frames[0], 3);
    assert!(
        render_frame.player_animation_frames[0] > render_frame.player_state_frames[0],
        "source capsule sampling should follow Melee's variable ftAnim_SetAnimRate pose frame"
    );
    assert_eq!(
        render_frame.player_source_pose_frames[0],
        render_frame.player_animation_frames[0].saturating_add(1),
        "runtime must consume the source pose frame selected by core instead of deriving a separate capsule frame"
    );
    assert_eq!(
        render_frame.player_source_pose_motion_states[0],
        render_frame.player_motion_states[0]
    );
    assert_eq!(render_frame.player_source_pose_model_facings[0], 1);
}

#[test]
fn render_scene_places_players_deterministically_from_render_frame() {
    let world = World::for_two_players();
    let render_frame = RenderFrame::from_world(&world);

    let scene = RenderScene::from_frame(&render_frame, 960, 540);

    assert_eq!(
        scene.background,
        RenderColor {
            r: 17,
            g: 19,
            b: 24,
            a: 255
        }
    );
    assert_eq!(
        scene.stage,
        RenderRect {
            x: 120,
            y: 405,
            width: 720,
            height: 8,
            color: RenderColor {
                r: 180,
                g: 187,
                b: 196,
                a: 255
            }
        }
    );
    assert_eq!(
        scene.players[0],
        RenderRect {
            x: 348,
            y: 286,
            width: 54,
            height: 119,
            color: RenderColor {
                r: 74,
                g: 138,
                b: 255,
                a: 255
            }
        }
    );
    assert_eq!(
        scene.players[1],
        RenderRect {
            x: 558,
            y: 286,
            width: 54,
            height: 119,
            color: RenderColor {
                r: 255,
                g: 198,
                b: 87,
                a: 255
            }
        }
    );
}

#[test]
fn default_play_world_starts_from_source_shaped_entry_spawns() {
    let world = mole_runtime::default_play_world();

    assert_eq!(world.players()[0].motion_state, MotionState::Entry);
    assert_eq!(world.players()[0].entry_timer, 5);
    assert_eq!(
        world.players()[0].position,
        Vec2 {
            x: -38_800,
            y: 35_200
        }
    );
    assert_eq!(world.players()[1].motion_state, MotionState::Entry);
    assert_eq!(world.players()[1].entry_timer, 10);
    assert_eq!(
        world.players()[1].position,
        Vec2 {
            x: 38_800,
            y: 35_200
        }
    );
}

#[test]
fn render_scene_exposes_match_intro_and_entry_platform_cues() {
    let mut world = mole_runtime::default_play_world();
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    for frame in 0..6 {
        step_world(&mut world, Frame(frame), &neutral);
    }
    assert_eq!(world.players()[0].motion_state, MotionState::EntryStart);
    let render_frame = RenderFrame::from_world(&world);

    let scene = RenderScene::from_frame(&render_frame, 960, 540);

    assert_eq!(scene.match_intro_label.as_deref(), Some("READY"));
    let platform = scene.entry_platforms[0].expect("EntryStart should expose P1 spawn platform");
    let base_y = scene
        .transform
        .world_to_screen(Vec2 {
            x: world.players()[0].position.x,
            y: world.players()[0].entry_base_y,
        })
        .y;
    assert!(platform.width >= scene.players[0].width * 2);
    assert!(platform.height >= 1);
    assert!(platform.y <= base_y);
    assert!(scene.entry_platforms[1].is_none());
}

#[test]
fn render_entry_platform_lifecycle_matches_source_accessory_states() {
    let mut world = mole_runtime::default_play_world();
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let mut next_frame = 0;

    let scene = RenderScene::from_frame(&RenderFrame::from_world(&world), 960, 540);
    assert_eq!(scene.match_intro_label.as_deref(), Some("READY"));
    assert!(scene.entry_platforms.iter().all(Option::is_none));

    while !world.players().iter().any(|player| {
        matches!(
            player.motion_state,
            MotionState::EntryStart | MotionState::EntryEnd
        )
    }) {
        step_world(&mut world, Frame(next_frame), &neutral);
        next_frame += 1;
        assert!(
            next_frame < 20,
            "EntryStart should appear after source stagger"
        );
    }

    let frame = RenderFrame::from_world(&world);
    let scene = RenderScene::from_frame(&frame, 960, 540);

    assert_eq!(scene.match_intro_label.as_deref(), Some("READY"));
    assert_eq!(scene.stage_surfaces.len(), 4);
    assert!(scene.entry_platforms.iter().any(Option::is_some));

    while world.players().iter().any(|player| {
        matches!(
            player.motion_state,
            MotionState::Entry | MotionState::EntryStart | MotionState::EntryEnd
        )
    }) {
        step_world(&mut world, Frame(next_frame), &neutral);
        next_frame += 1;
        assert!(
            next_frame < 120,
            "entry flow should resolve before gameplay"
        );
    }

    let frame = RenderFrame::from_world(&world);
    let scene = RenderScene::from_frame(&frame, 960, 540);

    assert!(scene.entry_platforms.iter().all(Option::is_none));
    assert!(scene.match_intro_label.is_none());
}

#[test]
fn render_transform_maps_core_units_to_screen_without_hidden_gameplay_scale() {
    let transform = RenderTransform::battlefield_camera(960, 540);
    let origin = transform.world_to_screen(Vec2 { x: 0, y: 0 });

    assert_eq!(origin.y, transform.ground_y);
    assert!(transform.pixels_per_core_unit_milli > 0);
}

#[test]
fn render_scene_contains_battlefield_surfaces_and_diamond_ecb() {
    let world = World::for_two_players();
    let frame = RenderFrame::from_world(&world);
    let scene = RenderScene::from_frame(&frame, 960, 540);

    assert_eq!(scene.stage_surfaces.len(), 4);
    assert_eq!(scene.player_ecbs[0].points.len(), 4);
    assert_eq!(
        scene.player_ecbs[0].points[0].x,
        scene.player_ecbs[0].points[2].x
    );
}

#[test]
fn runtime_asset_root_contains_background_and_sprite_files() {
    let asset_root = project_asset_root();

    assert!(asset_root.join("background.png").is_file());
    assert!(asset_root
        .join("DolphinMole")
        .join("standing")
        .join("Standing1.png")
        .is_file());
}

#[test]
fn render_scene_references_background_and_sprite_asset_paths() {
    let world = World::for_two_players();
    let frame = RenderFrame::from_world(&world);
    let scene = RenderScene::from_frame(&frame, 960, 540);

    assert_eq!(scene.background_image.relative_path, "background.png");
    assert_eq!(scene.background_image.rect.width, 960);
    assert_eq!(scene.background_image.rect.height, 540);
    assert_eq!(
        scene.player_sprites[0].relative_path(),
        "DolphinMole/standing/Standing1.png"
    );
}

#[test]
fn render_scene_draws_translucent_bubble_shield_for_guard_states() {
    let mut world = World::for_two_players();
    step_world(
        &mut world,
        Frame(0),
        &[
            PlayerInput::neutral().with_shield(true),
            PlayerInput::neutral(),
        ],
    );
    assert_eq!(world.players()[0].motion_state, MotionState::GuardOn);
    let frame = RenderFrame::from_world(&world);

    let scene = RenderScene::from_frame(&frame, 960, 540);
    let shield = scene.player_shields[0].expect("guarding player should expose a shield bubble");
    let player = scene.players[0];

    assert!(scene.player_shields[1].is_none());
    assert_eq!(shield.center.x, player.x + player.width as i32 / 2);
    assert_eq!(shield.center.y, player.y + player.height as i32 / 2);
    assert!(shield.radius > player.width / 2);
    assert!(shield.radius < player.height);
    assert_eq!(shield.color.a, 96);
}

#[test]
fn render_scene_exposes_attack_air_n_source_hitbox_and_hurtbox_pills() {
    let world = World::for_two_players();
    let mut frame = RenderFrame::from_world(&world);
    frame.player_motion_states[0] = MotionState::AttackAirN;
    frame.player_state_frames[0] = 6;
    frame.player_animation_frames[0] = 6;
    frame.player_facings[0] = 1;
    frame.player_source_pose_motion_states[0] = MotionState::AttackAirN;
    frame.player_source_pose_frames[0] = 7;
    frame.player_source_pose_model_facings[0] = 1;

    let scene = RenderScene::from_frame(&frame, 960, 540);

    assert_eq!(scene.player_hitbox_pills[0].len(), 3);
    assert_eq!(scene.player_hurtbox_pills[0].len(), 11);
    assert!(scene.player_hitbox_pills[1].is_empty());
    assert_eq!(scene.player_hurtbox_pills[1].len(), 11);
    assert_eq!(scene.player_hurtbox_pills[1][0].source_space, "melee_xyz");
    assert_eq!(
        scene.player_hitbox_pills[0][0].color,
        RenderColor::HITBOX_PILL
    );
    assert_eq!(
        scene.player_hurtbox_pills[0][0].color,
        RenderColor::HURTBOX_PILL
    );
    assert!(scene.player_hitbox_pills[0][0].color.a < 255);
    assert!(scene.player_hurtbox_pills[0][0].color.a < 255);
    assert!(scene.player_hitbox_pills[0][0].radius > 0);
    assert!(scene.player_hurtbox_pills[0][0].radius > 0);
    assert_eq!(scene.player_hitbox_pills[0][0].source_space, "melee_xyz");
    assert_eq!(
        scene.player_hitbox_pills[0][0].projected_view_kind,
        "derived_debug_view"
    );
    assert_eq!(
        scene.player_hitbox_pills[0][0].source_artifact_kind,
        "generated_frame_data_boxes"
    );
    assert_eq!(scene.player_hurtbox_pills[0][0].source_space, "melee_xyz");
    assert_eq!(
        scene.player_hurtbox_pills[0][0].projected_view_kind,
        "derived_debug_view"
    );
    assert_eq!(
        scene.player_hurtbox_pills[0][0].source_artifact_kind,
        "generated_frame_data_boxes"
    );
    assert_ne!(scene.player_hitbox_pills[0][0].source.a.z, 0.0);
    assert_ne!(scene.player_hurtbox_pills[0][0].source.a.z, 0.0);
    assert_ne!(
        scene.player_hurtbox_pills[0][0].a,
        scene.player_hurtbox_pills[0][0].b
    );
}

#[test]
fn render_scene_uses_attack_air_n_source_clear_frames_for_hitbox_pills() {
    let world = World::for_two_players();
    let mut frame = RenderFrame::from_world(&world);
    frame.player_motion_states[0] = MotionState::AttackAirN;
    frame.player_facings[0] = 1;
    frame.player_source_pose_motion_states[0] = MotionState::AttackAirN;
    frame.player_source_pose_model_facings[0] = 1;

    frame.player_state_frames[0] = 12;
    frame.player_animation_frames[0] = 12;
    frame.player_source_pose_frames[0] = 13;
    let cleared_first_window = RenderScene::from_frame(&frame, 960, 540);
    assert!(cleared_first_window.player_hitbox_pills[0].is_empty());
    assert_eq!(cleared_first_window.player_hurtbox_pills[0].len(), 11);

    frame.player_state_frames[0] = 19;
    frame.player_animation_frames[0] = 19;
    frame.player_source_pose_frames[0] = 20;
    let second_window = RenderScene::from_frame(&frame, 960, 540);
    assert_eq!(second_window.player_hitbox_pills[0].len(), 3);

    frame.player_state_frames[0] = 29;
    frame.player_animation_frames[0] = 29;
    frame.player_source_pose_frames[0] = 30;
    let cleared_second_window = RenderScene::from_frame(&frame, 960, 540);
    assert!(cleared_second_window.player_hitbox_pills[0].is_empty());
    assert_eq!(cleared_second_window.player_hurtbox_pills[0].len(), 11);
}

#[test]
fn render_scene_samples_source_capsules_from_animation_pose_frame() {
    let world = World::for_two_players();
    let mut frame = RenderFrame::from_world(&world);
    frame.player_motion_states[0] = MotionState::AttackAirN;
    frame.player_facings[0] = 1;
    frame.player_source_pose_motion_states[0] = MotionState::AttackAirN;
    frame.player_source_pose_model_facings[0] = 1;

    // Melee advances collision endpoints from the current JObj pose
    // (HSD_JObjReqAnimAll/HSD_JObjAnimAll -> ftColl_8007AD18), not from
    // gameplay state age alone. Frame 13 is clear, while pose frame 20 has
    // Captain Falcon Nair's second active window.
    frame.player_state_frames[0] = 12;
    frame.player_animation_frames[0] = 19;
    frame.player_source_pose_frames[0] = 20;

    let scene = RenderScene::from_frame(&frame, 960, 540);

    assert_eq!(scene.player_hitbox_pills[0].len(), 3);
    assert_eq!(scene.player_hurtbox_pills[0].len(), 11);
}

#[test]
fn runtime_airborne_a_press_reaches_attack_air_n_scene_pills() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.grounded = false;
    player.motion_state = MotionState::Fall;
    player.motion_frame = 0;
    player.position.y = 30_000;
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(0),
        &[
            PlayerInput::neutral().with_attack(true),
            PlayerInput::neutral(),
        ],
    );
    assert_eq!(world.players()[0].motion_state, MotionState::AttackAirN);

    for frame in 1..=6 {
        step_world(
            &mut world,
            Frame(frame),
            &[PlayerInput::neutral(), PlayerInput::neutral()],
        );
    }

    let frame = RenderFrame::from_world(&world);
    let scene = RenderScene::from_frame(&frame, 960, 540);

    assert_eq!(frame.player_motion_states[0], MotionState::AttackAirN);
    assert_eq!(frame.player_state_frames[0], 6);
    assert_eq!(scene.player_hitbox_pills[0].len(), 3);
    assert_eq!(scene.player_hurtbox_pills[0].len(), 11);
    assert_eq!(
        scene.player_hitbox_pills[0][0].color,
        RenderColor::HITBOX_PILL
    );
    assert_eq!(
        scene.player_hurtbox_pills[0][0].color,
        RenderColor::HURTBOX_PILL
    );
}

#[test]
fn motion_visual_changes_keep_sprite_anchor_at_character_position() {
    let world = World::for_two_players();
    let base_frame = RenderFrame::from_world(&world);
    let standing_scene = RenderScene::from_frame(&base_frame, 960, 540);

    let mut dash_frame = base_frame;
    dash_frame.player_motion_states[0] = MotionState::Dash;
    let dash_scene = RenderScene::from_frame(&dash_frame, 960, 540);

    let mut air_dodge_frame = base_frame;
    air_dodge_frame.player_motion_states[0] = MotionState::EscapeAir;
    let air_dodge_scene = RenderScene::from_frame(&air_dodge_frame, 960, 540);

    assert_eq!(
        dash_scene.player_contact_points[0],
        standing_scene.player_contact_points[0]
    );
    assert_eq!(
        air_dodge_scene.player_contact_points[0],
        standing_scene.player_contact_points[0]
    );
    assert!(dash_scene.players[0].width > dash_scene.players[0].height);
    assert!(air_dodge_scene.players[0].width > air_dodge_scene.players[0].height);
    assert!(dash_scene.players[0].height < standing_scene.players[0].height);
    assert!(air_dodge_scene.players[0].height < standing_scene.players[0].height);
}

#[test]
fn turn_run_capsule_projection_uses_model_entry_facing_not_mid_action_gameplay_facing() {
    let world = World::for_two_players();
    let mut frame = RenderFrame::from_world(&world);
    frame.player_motion_states[0] = MotionState::TurnRun;
    frame.player_animation_frames[0] = 10;
    frame.player_facings[0] = 1;
    frame.player_turn_run_accel_mul[0] = 1;
    frame.player_source_pose_motion_states[0] = MotionState::TurnRun;
    frame.player_source_pose_frames[0] = 11;
    frame.player_source_pose_model_facings[0] = 1;
    let entry_facing_scene = RenderScene::from_frame(&frame, 960, 540);

    frame.player_facings[0] = -1;
    let flipped_gameplay_facing_scene = RenderScene::from_frame(&frame, 960, 540);

    frame.player_turn_run_accel_mul[0] = -1;
    frame.player_source_pose_model_facings[0] = -1;
    let flipped_model_facing_scene = RenderScene::from_frame(&frame, 960, 540);

    assert_eq!(
        flipped_gameplay_facing_scene.player_hurtbox_pills[0],
        entry_facing_scene.player_hurtbox_pills[0],
        "TurnRun_Anim flips fp->facing_dir before changing the model root rotation"
    );
    assert_ne!(
        flipped_model_facing_scene.player_hurtbox_pills[0],
        entry_facing_scene.player_hurtbox_pills[0]
    );
}

#[test]
fn root_motion_capsules_render_after_melee_transn_reset() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.motion_state = MotionState::EscapeF;
    player.motion_frame = 1;
    let source_root = mole_core::source_root_motion_position(MotionState::EscapeF, 2)
        .expect("EscapeF frame 2 should have sampled TransN root position");
    player.position.x = (source_root.z * 1_000.0).round() as i32;
    player.facing = 1;
    player.grounded = true;
    assert!(world.set_player_state_for_diagnostic(0, player));
    let frame = RenderFrame::from_world(&world);

    let scene = RenderScene::from_frame(&frame, 960, 540);
    let first_hurtbox = scene.player_hurtbox_pills[0][0];
    let transn = mole_core::source_root_motion_position(
        MotionState::EscapeF,
        frame.player_source_pose_frames[0],
    )
    .unwrap();
    let expected_world_a = Vec2 {
        x: frame.player_positions[0].x
            + ((first_hurtbox.source.a.z - f64::from(transn.z)) * 1_000.0).round() as i32,
        y: frame.player_positions[0].y + (first_hurtbox.source.a.y * 1_000.0).round() as i32,
    };

    assert_eq!(
        first_hurtbox.a,
        scene.transform.world_to_screen(expected_world_a)
    );
}

#[test]
fn turn_run_capsules_render_after_transn_reset_at_floor_edge() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.motion_state = MotionState::TurnRun;
    player.motion_frame = 9;
    player.motion_anim_frame_milli = 9_000;
    player.position.x = world.stage().main_floor.right_x;
    player.facing = 1;
    player.turn_run_accel_mul = 1;
    player.grounded = true;
    assert!(world.set_player_state_for_diagnostic(0, player));
    let frame = RenderFrame::from_world(&world);

    let scene = RenderScene::from_frame(&frame, 960, 540);
    let first_hurtbox = scene.player_hurtbox_pills[0][0];
    let transn = mole_core::source_root_motion_position(
        MotionState::TurnRun,
        frame.player_source_pose_frames[0],
    )
    .expect("TurnRun source pose should expose TransN root position");
    let expected_world_a = Vec2 {
        x: frame.player_positions[0].x
            + ((first_hurtbox.source.a.z - f64::from(transn.z)) * 1_000.0).round() as i32,
        y: frame.player_positions[0].y + (first_hurtbox.source.a.y * 1_000.0).round() as i32,
    };

    assert_eq!(
        first_hurtbox.a,
        scene.transform.world_to_screen(expected_world_a)
    );
}

#[test]
fn render_scene_draws_core_owned_active_ecb() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.motion_state = MotionState::EscapeAir;
    player.motion_frame = 2;
    player.position = Vec2 { x: 1_000, y: 2_000 };
    player.ecb_bottom_offset_y = 2_790;
    player.grounded = false;
    assert!(world.set_player_state_for_diagnostic(0, player));
    let frame = RenderFrame::from_world(&world);

    let scene = RenderScene::from_frame(&frame, 960, 540);

    assert_eq!(frame.player_ecbs[0].bottom, Vec2 { x: 1_000, y: 3_568 });
    assert_eq!(
        scene.player_ecbs[0].points[2],
        scene.transform.world_to_screen(frame.player_ecbs[0].bottom)
    );
    assert_ne!(
        scene.player_ecbs[0].points[2],
        scene.player_contact_points[0]
    );
}

#[test]
fn sdl_runtime_launcher_uses_native_play_mode() {
    let launcher = project_asset_root()
        .join("execs")
        .join("Run SDL3 Runtime.cmd");
    let text =
        std::fs::read_to_string(&launcher).expect("SDL3 runtime launcher should be readable");

    assert!(text.contains(".local\\SDL3"));
    assert!(text.contains("--features \"sdl wup\""));
    assert!(text.contains("-- --sdl --play --input-trace"));
    assert!(!text.contains("--no-ucf"));
    assert!(!text.contains("--frames 600"));
}

#[test]
fn sdl_runtime_vanilla_launcher_disables_ucf_for_controller_testing() {
    let launcher = project_asset_root()
        .join("execs")
        .join("Run SDL3 Runtime Vanilla No UCF.cmd");
    let text =
        std::fs::read_to_string(&launcher).expect("vanilla SDL3 launcher should be readable");

    assert!(text.contains(".local\\SDL3"));
    assert!(text.contains("--features \"sdl wup\""));
    assert!(text.contains("-- --sdl --play --input-trace --no-ucf"));
    assert!(text.contains("Open State Graphs.cmd"));
}

#[test]
fn dolphin_mole_sprite_scale_uses_standing_height_reference() {
    let visual = DolphinMoleVisualProfile::default();

    assert_eq!(visual.standing_source_height_px, 136);
    assert_eq!(
        visual.standing_target_height_units,
        FighterProfile::falcon_like().standing_height_units
    );
    assert_eq!(visual.scale_milli_for_source_height(136), 167);
    assert_eq!(visual.scaled_size_units(171, 49).height, 8_167);
}

#[test]
fn render_scene_exposes_legacy_sprite_cues_without_replacing_rect_fallback() {
    let mut world = World::for_two_players();
    step_world(
        &mut world,
        Frame(0),
        &[
            PlayerInput::neutral().with_left_stick(64, 0),
            PlayerInput::neutral(),
        ],
    );
    let render_frame = RenderFrame::from_world(&world);

    let scene = RenderScene::from_frame(&render_frame, 960, 540);

    assert_eq!(
        scene.player_sprites[0].animation,
        LegacyAnimationKey::Walking
    );
    assert_eq!(scene.player_sprites[0].directory, "DolphinMole/walking");
    assert!(!scene.player_sprites[0].flip_x);
    assert_eq!(
        scene.player_sprites[1].animation,
        LegacyAnimationKey::Standing
    );
    assert!(scene.player_sprites[1].flip_x);
    assert_eq!(scene.players[0].width, 54);
    assert_eq!(scene.players[0].height, 119);
}

#[test]
fn legacy_dolphin_mole_asset_manifest_points_to_existing_files() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

    for animation in LEGACY_DOLPHIN_MOLE_ANIMATIONS {
        assert!(
            repo_root.join(animation.directory).is_dir(),
            "missing animation directory {}",
            animation.directory
        );

        for frame in animation.frames {
            let path = repo_root.join(animation.directory).join(frame);
            assert!(path.is_file(), "missing animation frame {}", path.display());
        }
    }
}

#[test]
fn render_asset_mapping_uses_legacy_animation_identity_without_mechanics() {
    assert_eq!(
        legacy_animation_for_motion_state(MotionState::Wait),
        LegacyAnimationKey::Standing
    );
    assert_eq!(
        legacy_animation_for_motion_state(MotionState::WalkSlow),
        LegacyAnimationKey::Walking
    );
    assert_eq!(
        legacy_animation_for_motion_state(MotionState::WalkMiddle),
        LegacyAnimationKey::Walking
    );
    assert_eq!(
        legacy_animation_for_motion_state(MotionState::WalkFast),
        LegacyAnimationKey::Walking
    );
    assert_eq!(
        legacy_animation_for_motion_state(MotionState::Dash),
        LegacyAnimationKey::Dashing
    );
    assert_eq!(
        legacy_animation_for_motion_state(MotionState::Guard),
        LegacyAnimationKey::Blocking
    );
    assert_eq!(
        legacy_animation_for_motion_state(MotionState::EscapeAir),
        LegacyAnimationKey::AirDodge
    );
    assert_eq!(
        legacy_animation_for_motion_state(MotionState::Pass),
        LegacyAnimationKey::Air
    );
}

#[test]
fn legacy_sprite_cue_uses_state_frame_and_facing_deterministically() {
    let cue = LegacySpriteCue::for_player(MotionState::Wait, 3, -1);

    assert_eq!(cue.animation, LegacyAnimationKey::Standing);
    assert_eq!(cue.directory, "DolphinMole/standing");
    assert_eq!(cue.frame, "Standing2.png");
    assert!(cue.flip_x);
}

#[test]
fn debug_overlay_reports_core_frame_and_checksum() {
    let world = World::for_two_players();
    let render_frame = RenderFrame::from_world(&world);

    let overlay = DebugOverlay::from_frame(&render_frame);

    assert_eq!(
        overlay.lines,
        vec![
            format!("FRAME {}", render_frame.frame.0),
            format!("CHECKSUM {}", render_frame.checksum),
        ]
    );
}

#[test]
fn debug_overlay_reports_each_player_motion_state_for_play_window() {
    let mut world = World::for_two_players();
    let inputs = [
        PlayerInput::neutral().with_left_stick(64, 0),
        PlayerInput::neutral().with_left_stick(-127, 0),
    ];
    step_world(&mut world, Frame(0), &inputs);
    let render_frame = RenderFrame::from_world(&world);

    let overlay = DebugOverlay::from_frame(&render_frame);

    assert_eq!(
        overlay.player_state_lines,
        ["P1 WALKSLOW F0".to_string(), "P2 DASH F0".to_string(),]
    );
}

#[test]
fn frame_debug_log_reports_input_state_physics_ecb_and_render_transform() {
    let mut world = World::for_two_players();
    let inputs = [
        PlayerInput::neutral().with_left_stick(64, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &inputs);
    let frame = RenderFrame::from_world(&world);
    let scene = RenderScene::from_frame(&frame, 960, 540);
    let line = FrameDebugLog::from_frame_and_scene(&frame, &scene, inputs).to_json_line();

    assert!(line.contains("\"frame\":1"));
    assert!(line.contains("\"p1_bits\":"));
    assert!(line.contains("\"motion_state\":\"WalkSlow\""));
    assert!(line.contains("\"velocity_x\":"));
    assert!(line.contains("\"ecb\":["));
    assert!(line.contains("\"render_transform\":"));
}

#[test]
fn frame_debug_log_reports_match_intro_and_entry_platform_cues() {
    let mut world = mole_runtime::default_play_world();
    let inputs = [PlayerInput::neutral(), PlayerInput::neutral()];
    for frame in 0..6 {
        step_world(&mut world, Frame(frame), &inputs);
    }
    let frame = RenderFrame::from_world(&world);
    let scene = RenderScene::from_frame(&frame, 960, 540);

    let line = FrameDebugLog::from_frame_and_scene(&frame, &scene, inputs).to_json_line();

    assert!(line.contains("\"match_intro_label\":\"READY\""));
    assert!(line.contains("\"entry_platforms\":[{\"x\":"));
    assert!(line.contains(",null]"));
}

#[test]
fn debug_overlay_reports_udp_packet_stats_when_available() {
    let world = World::for_two_players();
    let render_frame = RenderFrame::from_world(&world);
    let mut stats = UdpRuntimeStats::default();
    let packet = InputPacket::new(Frame(7), 1, PlayerInput::neutral(), 0x1234);
    stats.record_sent();
    stats.record_accept(PacketAcceptResult::Accepted, packet);
    stats.record_accept(PacketAcceptResult::Duplicate, packet);
    stats.record_missing_remote_frame();

    let overlay = DebugOverlay::from_frame_with_udp_stats(&render_frame, &stats);

    assert!(overlay
        .lines
        .contains(&"UDP TX 1 RX 1 DUP 1 MISS 1".to_string()));
    assert!(overlay
        .lines
        .contains(&"REMOTE FRAME 7 CHECKSUM 4660".to_string()));
    assert!(overlay.lines.contains(&"UDP RTT 7F".to_string()));
}

#[test]
fn replay_capture_serializes_runtime_frames_that_replay_to_the_same_checksum() {
    let initial = World::for_two_players();
    let mut world = initial.clone();
    let inputs = [
        PlayerInput::neutral().with_left_stick(64, 0),
        PlayerInput::neutral().with_attack(true),
    ];
    let mut capture = ReplayCapture::new(initial.clone());

    step_world(&mut world, Frame(0), &inputs);
    capture.record_frame(Frame(0), inputs, world.checksum());

    let text = capture.to_text();
    let parsed = ReplayCapture::from_text(initial, &text).expect("capture text should parse");
    let replayed = parsed.replay().expect("capture should replay");

    assert!(text.starts_with("mole_replay_v1\n"));
    assert!(text.contains(&format!("p1_bits={}", inputs[0].bits())));
    assert!(text.contains(&format!("checksum={}", world.checksum())));
    assert_eq!(replayed.checksum(), world.checksum());
}

#[test]
fn native_replay_path_uses_project_local_debug_replay_directory() {
    let path = native_replay_path(120);

    assert!(path.starts_with("debug/replays"));
    assert_eq!(
        path.file_name().and_then(|name| name.to_str()),
        Some("native-replay-120-frames.mrep")
    );
}

#[test]
fn udp_runtime_config_parses_direct_local_peer_addresses() {
    let config = UdpRuntimeConfig::from_args(&[
        "--udp".to_string(),
        "--local-addr".to_string(),
        "127.0.0.1:41001".to_string(),
        "--peer-addr".to_string(),
        "127.0.0.1:41002".to_string(),
        "--player-index".to_string(),
        "1".to_string(),
    ])
    .expect("direct UDP args should parse");

    assert_eq!(config.local_addr.to_string(), "127.0.0.1:41001");
    assert_eq!(config.peer_addr.to_string(), "127.0.0.1:41002");
    assert_eq!(config.player_index, 1);
}

#[test]
fn udp_runtime_config_rejects_missing_peer_address() {
    let error = UdpRuntimeConfig::from_args(&[
        "--udp".to_string(),
        "--local-addr".to_string(),
        "127.0.0.1:41001".to_string(),
    ])
    .expect_err("peer addr is required");

    assert!(error.contains("--peer-addr"));
}

#[test]
fn udp_runtime_stats_track_sent_received_duplicate_and_missing_packets() {
    let mut stats = UdpRuntimeStats::default();
    let packet = InputPacket::new(
        Frame(3),
        1,
        PlayerInput::neutral().with_left_stick(-64, 0),
        333,
    );

    stats.record_sent();
    stats.record_accept(PacketAcceptResult::Accepted, packet);
    stats.record_accept(PacketAcceptResult::Duplicate, packet);
    stats.record_missing_remote_frame();

    assert_eq!(stats.sent_packets, 1);
    assert_eq!(stats.received_packets, 1);
    assert_eq!(stats.duplicate_packets, 1);
    assert_eq!(stats.missing_remote_frames, 1);
    assert_eq!(stats.last_remote_frame, Some(Frame(3)));
    assert_eq!(stats.last_remote_checksum, Some(333));
}

#[test]
fn udp_runtime_stats_estimate_rtt_from_acknowledged_timing_probe() {
    let mut stats = UdpRuntimeStats::default();
    let packet =
        InputPacket::new(Frame(10), 1, PlayerInput::neutral(), 777).with_timing_probe(21, 6);

    stats.record_accept_at(Frame(14), PacketAcceptResult::Accepted, packet);

    assert_eq!(stats.last_remote_sequence, Some(21));
    assert_eq!(stats.last_acked_sequence, Some(6));
    assert_eq!(stats.last_rtt_frames, Some(8));
}

#[test]
fn physical_input_maps_gamecube_style_buttons_to_player_input() {
    let physical = PhysicalInput {
        left_x: 32_767,
        left_y: -32_768,
        c_x: -32_768,
        c_y: 32_767,
        left_trigger: 42,
        right_trigger: 201,
        attack: true,
        special: true,
        jump_primary: true,
        shield: true,
        grab: true,
        left_trigger_pressed: true,
        start: true,
        dpad_down: true,
        dpad_right: true,
        ..PhysicalInput::default()
    };

    let input = map_physical_input(physical);

    assert_eq!(input.stick_x(), 127);
    assert_eq!(input.stick_y(), -127);
    assert_eq!(input.c_stick_x(), -127);
    assert_eq!(input.c_stick_y(), 127);
    assert_eq!(input.left_trigger_analog(), 42);
    assert_eq!(input.right_trigger_analog(), 201);
    assert!(input.left_trigger_digital());
    assert!(!input.right_trigger_digital());
    assert!(input.attack());
    assert!(input.special());
    assert!(input.jump());
    assert!(input.jump_primary());
    assert!(!input.jump_secondary());
    assert!(input.shield());
    assert!(input.explicit_shield());
    assert!(input.grab());
    assert!(input.start());
    assert!(input.dpad_down());
    assert!(input.dpad_right());
}

#[test]
fn physical_input_keeps_trigger_shield_separate_from_generic_shield_bit() {
    let input = map_physical_input(PhysicalInput {
        left_trigger: 80,
        right_trigger_pressed: true,
        ..PhysicalInput::default()
    });

    assert!(input.shield());
    assert!(!input.explicit_shield());
    assert_eq!(input.left_trigger_analog(), 80);
    assert!(input.right_trigger_digital());
}

#[test]
fn physical_input_preserves_separate_jump_buttons() {
    let input = map_physical_input(PhysicalInput {
        jump_secondary: true,
        ..PhysicalInput::default()
    });

    assert!(!input.jump_primary());
    assert!(input.jump_secondary());
    assert!(input.jump());
}

#[test]
fn physical_input_applies_deadzone_to_small_stick_values() {
    let physical = PhysicalInput {
        left_x: 3_000,
        left_y: -3_000,
        ..PhysicalInput::default()
    };

    let input = map_physical_input(physical);

    assert_eq!(input.stick_x(), 0);
    assert_eq!(input.stick_y(), 0);
}

#[test]
fn gamecube_pad_maps_directly_to_compact_player_input_without_float_normalization() {
    let pad = GameCubePadStatus {
        stick_x: 255,
        stick_y: 0,
        c_stick_x: 0,
        c_stick_y: 255,
        left_trigger: 42,
        right_trigger: 201,
        buttons: GameCubeButtonState::from_bits(
            (1 << 0) | (1 << 1) | (1 << 2) | (1 << 3) | (1 << 4) | (1 << 7) | (1 << 9) | (1 << 11),
        ),
    };

    let input = map_gamecube_pad_to_player_input(pad);

    assert_eq!(input.stick_x(), 127);
    assert_eq!(input.stick_y(), -128);
    assert_eq!(input.c_stick_x(), -128);
    assert_eq!(input.c_stick_y(), 127);
    assert_eq!(input.left_trigger_analog(), 42);
    assert_eq!(input.right_trigger_analog(), 201);
    assert!(input.left_trigger_digital());
    assert!(!input.right_trigger_digital());
    assert!(input.attack());
    assert!(input.special());
    assert!(input.jump());
    assert!(input.jump_primary());
    assert!(input.jump_secondary());
    assert!(input.shield());
    assert!(input.grab());
    assert!(input.dpad_up());
    assert!(input.dpad_left());
}

#[test]
fn gamecube_pad_preserves_y_only_jump_button() {
    let pad = GameCubePadStatus {
        buttons: GameCubeButtonState::empty().with_y(true),
        ..GameCubePadStatus::neutral()
    };

    let input = map_gamecube_pad_to_player_input(pad);

    assert!(!input.jump_primary());
    assert!(input.jump_secondary());
    assert!(input.jump());
}

#[test]
fn wup_report_preserves_raw_gamecube_pad_status() {
    let mut report = [0u8; 37];
    report[0] = 0x21;
    report[1] = 0x10;
    let buttons = (1 << 0) | (1 << 3) | (1 << 8) | (1 << 10) | (1 << 11);
    report[2] = buttons as u8;
    report[3] = (buttons >> 8) as u8;
    report[4] = 255;
    report[5] = 0;
    report[6] = 64;
    report[7] = 192;
    report[8] = 42;
    report[9] = 201;

    let ports = parse_wup_report(report);
    let pad = ports[0].pad;

    assert!(ports[0].connected);
    assert_eq!(pad.stick_x, 255);
    assert_eq!(pad.stick_y, 0);
    assert_eq!(pad.c_stick_x, 64);
    assert_eq!(pad.c_stick_y, 192);
    assert_eq!(pad.left_trigger, 42);
    assert_eq!(pad.right_trigger, 201);
    assert_eq!(pad.buttons.bits(), buttons);
    assert!(pad.buttons.a());
    assert!(pad.buttons.y());
    assert!(pad.buttons.start());
    assert!(pad.buttons.r());
    assert!(pad.buttons.l());
    assert_eq!(pad.main_stick_i16(), (32_512, -32_768));
    assert_eq!(pad.c_stick_i16(), (-16_384, 16_384));
}

#[test]
fn wup_report_maps_port_zero_to_player_input_from_raw_gamecube_status() {
    let mut report = [0u8; 37];
    report[0] = 0x21;
    report[1] = 0x10;
    report[2] = 0b0000_0111;
    report[3] = 0b0000_1111;
    report[4] = 255;
    report[5] = 0;
    report[8] = 90;
    report[9] = 0;

    let ports = parse_wup_report(report);
    let input = map_gamecube_pad_to_player_input(ports[0].pad);

    assert!(ports[0].connected);
    assert_eq!(input.stick_x(), 127);
    assert_eq!(input.stick_y(), -128);
    assert!(input.attack());
    assert!(input.special());
    assert!(input.jump());
    assert!(input.shield());
    assert!(input.grab());
    assert!(input.start());
}

#[test]
fn wup_report_maps_c_stick_dpad_and_split_triggers() {
    let mut report = [0u8; 37];
    report[0] = 0x21;
    report[1] = 0x10;
    let buttons = (1 << 4) | (1 << 5) | (1 << 6) | (1 << 7) | (1 << 10) | (1 << 11);
    report[2] = buttons as u8;
    report[3] = (buttons >> 8) as u8;
    report[6] = 255;
    report[7] = 0;
    report[8] = 42;
    report[9] = 201;

    let ports = parse_wup_report(report);
    let pad = ports[0].pad;

    assert!(pad.buttons.dpad_left());
    assert!(pad.buttons.dpad_right());
    assert!(pad.buttons.dpad_down());
    assert!(pad.buttons.dpad_up());
    assert!(pad.buttons.l());
    assert!(pad.buttons.r());
    assert_eq!(pad.left_trigger, 42);
    assert_eq!(pad.right_trigger, 201);
    assert_eq!(pad.c_stick_i16(), (32_512, -32_768));
    let input = map_gamecube_pad_to_player_input(pad);
    assert!(input.shield());
    assert_eq!(input.left_trigger_analog(), 42);
    assert_eq!(input.right_trigger_analog(), 201);
    assert!(input.left_trigger_digital());
    assert!(input.right_trigger_digital());
    assert_eq!(input.c_stick_x(), 127);
    assert_eq!(input.c_stick_y(), -128);
    assert!(input.dpad_left());
    assert!(input.dpad_right());
    assert!(input.dpad_down());
    assert!(input.dpad_up());
}

#[test]
fn wup_report_marks_disconnected_ports_neutral() {
    let mut report = [0u8; 37];
    report[0] = 0x21;

    let ports = parse_wup_report(report);

    assert!(!ports[0].connected);
    assert_eq!(
        map_gamecube_pad_to_player_input(ports[0].pad),
        PlayerInput::neutral()
    );
}

#[test]
fn wup_ports_assign_first_connected_controller_to_player_one() {
    let ports = [
        WupPort::default(),
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                buttons: GameCubeButtonState::empty().with_a(true),
                ..GameCubePadStatus::neutral()
            },
        },
        WupPort::default(),
        WupPort::default(),
    ];

    let inputs = mole_runtime::map_wup_ports_to_player_inputs(ports);

    assert!(inputs[0].attack());
    assert_eq!(inputs[1], PlayerInput::neutral());
}

#[test]
fn wup_input_mapper_uses_first_connected_raw_sample_as_console_origin() {
    let mut mapper = WupInputMapper::default();
    let neutral_ports = [
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                stick_x: 131,
                stick_y: 125,
                c_stick_x: 127,
                c_stick_y: 130,
                left_trigger: 7,
                right_trigger: 4,
                buttons: GameCubeButtonState::empty(),
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ];

    let neutral = mapper.map_ports(neutral_ports);

    assert_eq!(neutral[0].stick_x(), 0);
    assert_eq!(neutral[0].stick_y(), 0);
    assert!(!neutral[0].shield());

    let moved = mapper.map_ports([
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                stick_x: 255,
                stick_y: 0,
                left_trigger: trigger_raw_from_origin(7, source_shield_trigger()),
                buttons: GameCubeButtonState::empty(),
                ..GameCubePadStatus::neutral()
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ]);

    assert_eq!(moved[0].stick_x(), 89);
    assert_eq!(moved[0].stick_y(), -89);
    assert_eq!(moved[0].left_trigger_analog(), source_shield_trigger());
    assert_eq!(moved[0].right_trigger_analog(), 0);
    assert!(moved[0].shield());
}

#[test]
fn wup_input_mapper_treats_small_post_origin_trigger_drift_as_neutral() {
    let mut mapper = WupInputMapper::default();
    mapper.map_ports_to_melee_snapshots([
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                stick_x: 131,
                stick_y: 125,
                c_stick_x: 127,
                c_stick_y: 130,
                left_trigger: 7,
                right_trigger: 4,
                buttons: GameCubeButtonState::empty(),
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ]);

    let drift = mapper.map_ports_to_melee_snapshots([
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                stick_x: 131,
                stick_y: 125,
                c_stick_x: 127,
                c_stick_y: 130,
                left_trigger: 7,
                right_trigger: 20,
                buttons: GameCubeButtonState::empty(),
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ]);
    let p1 = drift[0].expect("connected port should produce player one snapshot");
    let facts = p1.facts(Default::default());

    assert_eq!(p1.left_trigger, 0);
    assert_eq!(p1.right_trigger, 0);
    assert!(!p1.shield_held);
    assert!(!p1.shield_pressed);
    assert!(!facts.source_held.lr());
    assert!(!facts.source_pressed.lr());
}

#[test]
fn wup_input_mapper_preserves_native_gate_distance_after_origin() {
    let mut mapper = WupInputMapper::default();
    mapper.map_ports([
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                stick_x: 131,
                stick_y: 125,
                c_stick_x: 127,
                c_stick_y: 130,
                buttons: GameCubeButtonState::empty(),
                ..GameCubePadStatus::neutral()
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ]);

    let moved = mapper.map_ports([
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                stick_x: 232,
                stick_y: 20,
                c_stick_x: 20,
                c_stick_y: 232,
                buttons: GameCubeButtonState::empty(),
                ..GameCubePadStatus::neutral()
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ]);

    assert_eq!(moved[0].stick_x(), 87);
    assert_eq!(moved[0].stick_y(), -90);
    assert_eq!(moved[0].c_stick_x(), -90);
    assert_eq!(moved[0].c_stick_y(), 87);
}

#[test]
fn wup_input_mapper_preserves_c_stick_and_dpad_in_player_input() {
    let mut mapper = WupInputMapper::default();
    mapper.map_ports([
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                c_stick_x: 127,
                c_stick_y: 130,
                buttons: GameCubeButtonState::empty(),
                ..GameCubePadStatus::neutral()
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ]);

    let moved = mapper.map_ports([
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                c_stick_x: 20,
                c_stick_y: 232,
                buttons: GameCubeButtonState::from_bits((1 << 4) | (1 << 7)),
                ..GameCubePadStatus::neutral()
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ]);

    assert_eq!(moved[0].c_stick_x(), -90);
    assert_eq!(moved[0].c_stick_y(), 87);
    assert!(moved[0].dpad_left());
    assert!(moved[0].dpad_up());
}

#[test]
fn wup_input_mapper_preserves_split_triggers_in_player_input() {
    let mut mapper = WupInputMapper::default();
    mapper.map_ports([
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                left_trigger: 7,
                right_trigger: 4,
                buttons: GameCubeButtonState::empty(),
                ..GameCubePadStatus::neutral()
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ]);

    let moved = mapper.map_ports([
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                left_trigger: trigger_raw_from_origin(7, source_shield_trigger()),
                right_trigger: 205,
                buttons: GameCubeButtonState::empty().with_r(true),
                ..GameCubePadStatus::neutral()
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ]);

    assert_eq!(moved[0].left_trigger_analog(), source_shield_trigger());
    assert_eq!(moved[0].right_trigger_analog(), 201);
    assert!(!moved[0].left_trigger_digital());
    assert!(moved[0].right_trigger_digital());
    assert!(moved[0].shield());
}

#[test]
fn wup_input_mapper_preserves_held_buttons_without_injecting_tap_jump() {
    let mut mapper = WupInputMapper::default();
    mapper.map_ports([
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                stick_x: 131,
                stick_y: 125,
                buttons: GameCubeButtonState::empty(),
                ..GameCubePadStatus::neutral()
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ]);

    let tap_jump_attack = [
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                stick_x: 131,
                stick_y: 220,
                buttons: GameCubeButtonState::empty().with_a(true),
                ..GameCubePadStatus::neutral()
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ];

    let first = mapper.map_ports(tap_jump_attack);

    assert!(first[0].attack());
    assert!(!first[0].jump());
    assert_eq!(first[0].stick_y(), 127);

    let mut held = first;
    for _ in 0..3 {
        held = mapper.map_ports(tap_jump_attack);
    }

    assert!(held[0].attack());
    assert!(!held[0].jump());
    assert_eq!(held[0].stick_y(), 127);
}

#[test]
fn wup_input_mapper_preserves_held_jump_button_across_frames() {
    let mut mapper = WupInputMapper::default();
    mapper.map_ports([
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                stick_x: 131,
                stick_y: 125,
                buttons: GameCubeButtonState::empty(),
                ..GameCubePadStatus::neutral()
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ]);

    let held_x = [
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                stick_x: 131,
                stick_y: 125,
                buttons: GameCubeButtonState::empty().with_x(true),
                ..GameCubePadStatus::neutral()
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ];

    let first = mapper.map_ports(held_x);
    let held = mapper.map_ports(held_x);

    assert!(first[0].jump());
    assert!(held[0].jump());
}

#[test]
fn wup_input_mapper_preserves_y_only_jump_button() {
    let mut mapper = WupInputMapper::default();
    mapper.map_ports([
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                buttons: GameCubeButtonState::empty(),
                ..GameCubePadStatus::neutral()
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ]);

    let mapped = mapper.map_ports([
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                buttons: GameCubeButtonState::empty().with_y(true),
                ..GameCubePadStatus::neutral()
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ]);

    assert!(!mapped[0].jump_primary());
    assert!(mapped[0].jump_secondary());
    assert!(mapped[0].jump());
}

#[test]
fn wup_input_mapper_feeds_console_origin_pads_through_melee_processor() {
    let mut mapper = WupInputMapper::default();
    let neutral_ports = [
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                stick_x: 131,
                stick_y: 125,
                c_stick_x: 127,
                c_stick_y: 130,
                left_trigger: 7,
                right_trigger: 4,
                buttons: GameCubeButtonState::empty(),
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ];

    let neutral = mapper.map_ports_to_melee_snapshots(neutral_ports);
    let neutral_p1 = neutral[0].expect("connected port should produce player one snapshot");

    assert_eq!(neutral_p1.lstick, (0, 0));
    assert_eq!(neutral_p1.cstick, (0, 0));
    assert_eq!(neutral_p1.left_trigger, 0);
    assert_eq!(neutral_p1.right_trigger, 0);
    assert!(!neutral_p1.shield_held);
    assert_eq!(neutral_p1.x_tap_timer, 0xfe);

    let moved_ports = [
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                stick_x: 255,
                stick_y: 0,
                c_stick_x: 255,
                c_stick_y: 0,
                left_trigger: trigger_raw_from_origin(7, source_shield_trigger()),
                right_trigger: 4,
                buttons: GameCubeButtonState::empty().with_a(true),
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ];

    let moved = mapper.map_ports_to_melee_snapshots(moved_ports);
    let moved_p1 = moved[0].expect("connected port should produce player one snapshot");

    assert_eq!(moved_p1.prev_lstick, (0, 0));
    assert_eq!(moved_p1.lstick, (89, -89));
    assert_eq!(moved_p1.cstick, (89, -89));
    assert_eq!(moved_p1.left_trigger, source_shield_trigger());
    assert_eq!(moved_p1.right_trigger, 0);
    assert!(moved_p1.pressed.a());
    assert!(moved_p1.shield_pressed);
    assert_eq!(moved_p1.x_tap_timer, 0);
    assert_eq!(moved_p1.trigger_timer, 0);

    let held = mapper.map_ports_to_melee_snapshots(moved_ports);
    let held_p1 = held[0].expect("connected port should produce player one snapshot");

    assert_eq!(held_p1.prev_lstick, (89, -89));
    assert_eq!(held_p1.lstick, (89, -84));
    assert!(held_p1.held.a());
    assert!(!held_p1.pressed.a());
    assert!(!held_p1.shield_pressed);
    assert_eq!(held_p1.x_tap_timer, 1);
    assert_eq!(held_p1.trigger_timer, 1);
}

#[test]
fn wup_input_mapper_collapses_subframe_capture_window_before_melee_processing() {
    let mut mapper = WupInputMapper::new(WupInputConfig { ucf_enabled: false });
    mapper.map_ports_to_input_trace(connected_wup_pad(GameCubePadStatus::neutral()));

    let early_button = connected_wup_pad(GameCubePadStatus {
        stick_x: 144,
        buttons: GameCubeButtonState::empty().with_a(true),
        ..GameCubePadStatus::neutral()
    });
    let latest_analog = connected_wup_pad(GameCubePadStatus {
        stick_x: 232,
        buttons: GameCubeButtonState::empty(),
        ..GameCubePadStatus::neutral()
    });

    let trace = mapper.map_capture_window_to_input_trace(&[early_button, latest_analog]);
    let p1 = trace.players[0].expect("connected capture window should produce player one");

    assert_eq!(trace.capture_report_count, 2);
    assert_eq!(p1.raw.stick_x, 232);
    assert!(p1.raw.buttons.a());
    assert!(p1.snapshot.held.a());
    assert!(p1.input.attack());
    assert!(p1.input.stick_x() > 0);
}

#[test]
fn wup_input_mapper_capture_window_keeps_latest_analog_sample_not_stale_queue_head() {
    let mut mapper = WupInputMapper::new(WupInputConfig { ucf_enabled: false });
    mapper.map_ports_to_input_trace(connected_wup_pad(GameCubePadStatus::neutral()));

    let stale_left = connected_wup_pad(GameCubePadStatus {
        stick_x: 0,
        ..GameCubePadStatus::neutral()
    });
    let fresh_right = connected_wup_pad(GameCubePadStatus {
        stick_x: 255,
        ..GameCubePadStatus::neutral()
    });

    let trace = mapper.map_capture_window_to_input_trace(&[stale_left, fresh_right]);
    let p1 = trace.players[0].expect("connected capture window should produce player one");

    assert_eq!(trace.capture_report_count, 2);
    assert_eq!(p1.raw.stick_x, 255);
    assert!(p1.input.stick_x() > 0);
}

#[test]
fn wup_input_mapper_capture_window_uses_first_connected_sample_as_origin_before_collapsing() {
    let mut mapper = WupInputMapper::new(WupInputConfig { ucf_enabled: false });

    let first_origin = connected_wup_pad(GameCubePadStatus {
        stick_x: 131,
        stick_y: 125,
        buttons: GameCubeButtonState::empty(),
        ..GameCubePadStatus::neutral()
    });
    let latest_gameplay = connected_wup_pad(GameCubePadStatus {
        stick_x: 232,
        stick_y: 125,
        buttons: GameCubeButtonState::empty(),
        ..GameCubePadStatus::neutral()
    });

    let trace = mapper.map_capture_window_to_input_trace(&[first_origin, latest_gameplay]);
    let p1 = trace.players[0].expect("connected capture window should produce player one");

    assert_eq!(trace.capture_report_count, 2);
    assert_eq!(p1.origin.stick_x, 131);
    assert_eq!(p1.raw.stick_x, 232);
    assert!(p1.input.stick_x() > 0);
}

#[test]
fn wup_input_mapper_applies_ucf_cardinals_after_console_origin() {
    let mut mapper = WupInputMapper::default();
    mapper.map_ports_to_melee_snapshots([
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                stick_x: 128,
                stick_y: 128,
                c_stick_x: 128,
                c_stick_y: 128,
                buttons: GameCubeButtonState::empty(),
                ..GameCubePadStatus::neutral()
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ]);

    let cardinal = mapper.map_ports([
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                stick_x: 209,
                stick_y: 133,
                c_stick_x: 133,
                c_stick_y: 209,
                buttons: GameCubeButtonState::empty(),
                ..GameCubePadStatus::neutral()
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ]);

    assert_eq!(cardinal[0].stick_x(), 127);
    assert_eq!(cardinal[0].stick_y(), 0);
    assert_eq!(cardinal[0].c_stick_x(), 0);
    assert_eq!(cardinal[0].c_stick_y(), 127);
}

#[test]
fn wup_input_mapper_can_disable_ucf_preprocessing() {
    let mut mapper = WupInputMapper::new(WupInputConfig { ucf_enabled: false });
    mapper.map_ports_to_melee_snapshots([
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                stick_x: 128,
                stick_y: 128,
                buttons: GameCubeButtonState::empty(),
                ..GameCubePadStatus::neutral()
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ]);

    let snapshots = mapper.map_ports_to_melee_snapshots([
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                stick_x: 209,
                stick_y: 133,
                buttons: GameCubeButtonState::empty(),
                ..GameCubePadStatus::neutral()
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ]);
    let p1 = snapshots[0].expect("connected port should produce player one snapshot");

    assert_eq!(p1.lstick, (125, 0));
    assert_eq!(p1.facts(Default::default()).dash_direction, 1);
}

#[test]
fn wup_input_mapper_applies_native_hsd_stick_clamp_even_when_ucf_disabled() {
    let mut mapper = WupInputMapper::new(WupInputConfig { ucf_enabled: false });
    mapper.map_ports_to_melee_snapshots([
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                stick_x: 128,
                stick_y: 128,
                c_stick_x: 128,
                c_stick_y: 128,
                buttons: GameCubeButtonState::empty(),
                ..GameCubePadStatus::neutral()
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ]);

    let snapshots = mapper.map_ports_to_melee_snapshots([
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                stick_x: 255,
                stick_y: 255,
                c_stick_x: 0,
                c_stick_y: 0,
                buttons: GameCubeButtonState::empty(),
                ..GameCubePadStatus::neutral()
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ]);
    let p1 = snapshots[0].expect("connected port should produce player one snapshot");

    assert_eq!(p1.lstick, (89, 89));
    assert_eq!(p1.cstick, (-89, -89));
}

#[test]
fn wup_input_mapper_leaves_non_ucf_cardinal_cross_axis_for_vanilla_deadzone_cleanup() {
    let mut mapper = WupInputMapper::default();
    mapper.map_ports_to_melee_snapshots([
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                stick_x: 128,
                stick_y: 128,
                buttons: GameCubeButtonState::empty(),
                ..GameCubePadStatus::neutral()
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ]);

    let snapshots = mapper.map_ports_to_melee_snapshots([
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                stick_x: 231,
                stick_y: 150,
                buttons: GameCubeButtonState::empty(),
                ..GameCubePadStatus::neutral()
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ]);
    let p1 = snapshots[0].expect("connected port should produce player one snapshot");
    let facts = p1.facts(Default::default());

    assert_eq!(p1.lstick, (124, 0));
    assert_eq!(facts.horizontal_smash_direction, 1);
    assert_eq!(facts.dash_direction, 1);
}

#[test]
fn wup_input_mapper_outputs_vanilla_dash_snapshot_after_ucf_preprocessing() {
    let mut mapper = WupInputMapper::default();
    mapper.map_ports_to_melee_snapshots([
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                stick_x: 128,
                stick_y: 128,
                buttons: GameCubeButtonState::empty(),
                ..GameCubePadStatus::neutral()
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ]);
    mapper.map_ports_to_melee_snapshots([
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                stick_x: 161,
                stick_y: 128,
                buttons: GameCubeButtonState::empty(),
                ..GameCubePadStatus::neutral()
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ]);

    let dash = mapper.map_ports_to_melee_snapshots([
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                stick_x: 232,
                stick_y: 128,
                buttons: GameCubeButtonState::empty(),
                ..GameCubePadStatus::neutral()
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ]);
    let p1 = dash[0].expect("connected port should produce player one snapshot");
    let facts = p1.facts(Default::default());

    assert_eq!(p1.lstick, (127, 0));
    assert_eq!(facts.dash_direction, 1);
}

#[test]
fn wup_input_mapper_carries_ucf_dashback_amendment_into_player_input() {
    let mut mapper = WupInputMapper::default();
    mapper.map_ports([
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                stick_x: 128,
                stick_y: 128,
                buttons: GameCubeButtonState::empty(),
                ..GameCubePadStatus::neutral()
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ]);
    mapper.map_ports([
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                stick_x: 88,
                stick_y: 128,
                buttons: GameCubeButtonState::empty(),
                ..GameCubePadStatus::neutral()
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ]);

    let amended = mapper.map_ports([
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                stick_x: 0,
                stick_y: 128,
                buttons: GameCubeButtonState::empty(),
                ..GameCubePadStatus::neutral()
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ]);

    assert_eq!(amended[0].stick_x(), -127);
    assert_ne!(amended[0].bits() & UCF_DASHBACK_AMENDMENT_BIT, 0);
}

#[test]
fn wup_input_mapper_trace_exposes_raw_origin_native_ucf_and_core_input() {
    let mut mapper = WupInputMapper::default();
    let neutral_ports = [
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                stick_x: 131,
                stick_y: 125,
                c_stick_x: 128,
                c_stick_y: 128,
                buttons: GameCubeButtonState::empty(),
                ..GameCubePadStatus::neutral()
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ];
    mapper.map_ports_to_input_trace(neutral_ports);

    let moved_ports = [
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                stick_x: 255,
                stick_y: 125,
                c_stick_x: 128,
                c_stick_y: 128,
                buttons: GameCubeButtonState::empty(),
                ..GameCubePadStatus::neutral()
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ];

    let trace = mapper.map_ports_to_input_trace(moved_ports);
    let p1 = trace.players[0].expect("connected controller should have a trace row");

    assert!(trace.ucf_enabled);
    assert_eq!(trace.adapter_ports, [true, false, false, false]);
    assert_eq!(p1.source_port, 0);
    assert_eq!(p1.raw.stick_x, 255);
    assert_eq!(p1.origin.stick_x, 131);
    assert_eq!(p1.origin_adjusted.stick_x, 252);
    assert_eq!(p1.native.stick_x, 255);
    assert_eq!(p1.ucf.stick_x, 255);
    assert_eq!(p1.snapshot.lstick, (127, 0));
    assert_eq!(p1.input.stick_x(), 127);
    assert_eq!(trace.inputs[0].stick_x(), 127);
    assert!(p1.dashback_amendment);
    assert!(trace.inputs[0].ucf_dashback_amendment());
}

#[test]
fn controller_input_trace_log_joins_wup_stages_with_core_state_and_facts() {
    let mut mapper = WupInputMapper::default();
    let neutral_ports = [
        WupPort {
            connected: true,
            pad: GameCubePadStatus::neutral(),
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ];
    mapper.map_ports_to_input_trace(neutral_ports);
    let dash_ports = [
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                stick_x: 232,
                buttons: GameCubeButtonState::empty(),
                ..GameCubePadStatus::neutral()
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ];
    let trace = mapper.map_ports_to_input_trace(dash_ports);
    let mut world = World::for_two_players();
    let before = RenderFrame::from_world(&world);

    step_world(&mut world, Frame(0), &trace.inputs);
    let after = RenderFrame::from_world(&world);
    let line =
        ControllerInputTraceLog::from_wup_trace(Frame(0), &trace, &before, &after).to_json_line();

    assert!(line.contains("\"frame\":0"));
    assert!(line.contains("\"ucf_enabled\":true"));
    assert!(line.contains("\"capture_report_count\":1"));
    assert!(line.contains("\"raw\":{\"stick_x\":232"));
    assert!(line.contains("\"origin\":{\"stick_x\":128"));
    assert!(line.contains("\"native\":{\"stick_x\":255"));
    assert!(line.contains("\"ucf\":{\"stick_x\":255"));
    assert!(line.contains("\"input\":{\"bits\":"));
    assert!(line.contains("\"stick_x\":127"));
    assert!(line.contains("\"x_tap_timer\":0"));
    assert!(line.contains("\"dash_direction\":1"));
    assert!(line.contains("\"before\":{\"frame\":0"));
    assert!(line.contains("\"after\":{\"frame\":1"));
    assert!(line.contains("\"motion_state\":\"Dash\""));
    assert!(line.contains("\"velocity_x\":"));
    assert!(line.contains("\"ground_velocity_x\":"));
    assert!(line.contains("\"ground_accel_x\":"));
    assert!(line.contains("\"dash_x0\":"));
    assert!(line.contains("\"walk_anim_velocity_x\":"));
    assert!(line.contains("\"walk_accel_mul_milli\":"));
    assert!(line.contains("\"turn_facing_after\":"));
    assert!(line.contains("\"turn_has_turned\":"));
    assert!(line.contains("\"turn_just_turned\":"));
    assert!(line.contains("\"turn_frames_to_turn\":"));
    assert!(line.contains("\"turn_dash_after_direction\":"));
    assert!(line.contains("\"turn_latched_buttons\":"));
    assert!(line.contains("\"run_no_interrupt_frames\":"));
    assert!(line.contains("\"motion_cmd_var0\":"));
    assert!(line.contains("\"motion_cmd_var1\":"));
    assert!(line.contains("\"run_brake_frames_remaining\":"));
    assert!(line.contains("\"turn_run_x14\":"));
    assert!(line.contains("\"motion_anim_rate_milli\":"));
    assert!(line.contains("\"core_facts\":{"));
    assert!(line.contains("\"checksum\":"));
}

#[test]
fn input_trace_writer_creates_jsonl_file_in_requested_directory() {
    let dir = std::env::temp_dir().join(format!(
        "mole-runtime-input-trace-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos()
    ));
    let mut writer =
        InputTraceWriter::create_in_dir(&dir).expect("input trace writer should open a JSONL file");
    let mut mapper = WupInputMapper::default();
    let trace = mapper.map_ports_to_input_trace([
        WupPort {
            connected: true,
            pad: GameCubePadStatus::neutral(),
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ]);
    let world = World::for_two_players();
    let frame = RenderFrame::from_world(&world);
    let log = ControllerInputTraceLog::from_wup_trace(Frame(0), &trace, &frame, &frame);

    writer
        .write_line(&log)
        .expect("input trace writer should write a JSON line");

    let text = std::fs::read_to_string(writer.path()).expect("trace file should be readable");
    assert!(writer.path().starts_with(&dir));
    assert!(writer
        .path()
        .file_name()
        .and_then(|name| name.to_str())
        .expect("trace filename should be utf-8")
        .starts_with("controller-input-trace-"));
    assert!(text.ends_with('\n'));
    assert!(text.contains("\"input_backend\":\"wup\""));
}

#[test]
fn wup_input_mapper_recaptures_origin_after_gamecube_recenter_combo() {
    let mut mapper = WupInputMapper::default();
    mapper.map_ports([
        WupPort {
            connected: true,
            pad: GameCubePadStatus::neutral(),
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ]);

    let recenter_pad = GameCubePadStatus {
        stick_x: 140,
        stick_y: 120,
        c_stick_x: 132,
        c_stick_y: 124,
        left_trigger: 9,
        right_trigger: 11,
        buttons: GameCubeButtonState::from_bits((1 << 2) | (1 << 3) | (1 << 8)),
    };

    let mut mapped = [PlayerInput::neutral(), PlayerInput::neutral()];
    for _ in 0..mole_runtime::wup_input::GAMECUBE_RECENTER_FRAMES {
        mapped = mapper.map_ports([
            WupPort {
                connected: true,
                pad: recenter_pad,
            },
            WupPort::default(),
            WupPort::default(),
            WupPort::default(),
        ]);
    }

    assert_eq!(mapped[0].stick_x(), 0);
    assert_eq!(mapped[0].stick_y(), 0);
    assert!(!mapped[0].shield());
}

#[test]
fn input_readout_preserves_adapter_port_and_raw_physical_state() {
    let ports = [
        WupPort::default(),
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                stick_x: 255,
                stick_y: 0,
                c_stick_x: 0,
                c_stick_y: 255,
                left_trigger: 100,
                right_trigger: 200,
                buttons: GameCubeButtonState::from_bits(
                    (1 << 0) | (1 << 1) | (1 << 2) | (1 << 5) | (1 << 11),
                ),
            },
        },
        WupPort::default(),
        WupPort::default(),
    ];

    let readout = InputReadout::from_wup_ports(ports);

    assert_eq!(readout.adapter_ports, [false, true, false, false]);
    assert!(readout.players[0].connected);
    assert_eq!(readout.players[0].source_port, Some(1));
    assert_eq!(readout.players[0].input.stick_x(), 127);
    assert_eq!(readout.players[0].input.stick_y(), -128);
    assert!(readout.players[0].buttons.attack);
    assert!(readout.players[0].buttons.special);
    assert!(readout.players[0].buttons.jump);
    assert!(readout.players[0].buttons.shield);
    assert!(readout.players[0].buttons.left_trigger);
    assert!(!readout.players[0].buttons.right_trigger);
    assert!(readout.players[0].buttons.dpad_right);
    assert_eq!(readout.players[0].raw_stick_x, 255);
    assert_eq!(readout.players[0].raw_stick_y, 0);
    assert_eq!(readout.players[0].raw_c_stick_x, 0);
    assert_eq!(readout.players[0].raw_c_stick_y, 255);
    assert_eq!(readout.players[0].stick_x, 32_512);
    assert_eq!(readout.players[0].stick_y, -32_768);
    assert_eq!(readout.players[0].c_stick_x, -32_768);
    assert_eq!(readout.players[0].c_stick_y, 32_512);
    assert_eq!(readout.players[0].left_trigger, 100);
    assert_eq!(readout.players[0].right_trigger, 200);
    assert!(!readout.players[1].connected);
}

#[test]
fn input_readout_treats_light_analog_trigger_as_shield() {
    let ports = [
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                left_trigger: 42,
                buttons: GameCubeButtonState::empty(),
                ..GameCubePadStatus::neutral()
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ];

    let readout = InputReadout::from_wup_ports(ports);

    assert!(readout.players[0].buttons.shield);
    assert!(!readout.players[0].buttons.left_trigger);
    assert!(!readout.players[0].buttons.right_trigger);
}

#[test]
fn input_readout_stream_json_keeps_native_axes_and_buttons() {
    let ports = [
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                stick_x: 255,
                stick_y: 0,
                c_stick_x: 128,
                c_stick_y: 255,
                left_trigger: 77,
                right_trigger: 255,
                buttons: GameCubeButtonState::from_bits((1 << 0) | (1 << 7) | (1 << 9) | (1 << 10)),
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ];

    let line = InputReadout::from_wup_ports(ports).to_json_line();

    assert!(line.contains("\"raw_main_x\":255"));
    assert!(line.contains("\"raw_main_y\":0"));
    assert!(line.contains("\"raw_c_y\":255"));
    assert!(line.contains("\"main_x\":32512"));
    assert!(line.contains("\"main_y\":-32768"));
    assert!(line.contains("\"c_y\":32512"));
    assert!(line.contains("\"right_trigger\":255"));
    assert!(line.contains("\"a\":true"));
    assert!(line.contains("\"z\":true"));
    assert!(line.contains("\"r\":true"));
    assert!(line.contains("\"dpad_up\":true"));
}

#[test]
fn input_readout_stream_json_can_include_melee_snapshot_facts() {
    let mut mapper = WupInputMapper::default();
    let neutral_ports = [
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                stick_x: 131,
                stick_y: 125,
                c_stick_x: 127,
                c_stick_y: 130,
                left_trigger: 7,
                right_trigger: 4,
                buttons: GameCubeButtonState::empty(),
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ];
    mapper.map_ports_to_melee_snapshots(neutral_ports);

    let moved_ports = [
        WupPort {
            connected: true,
            pad: GameCubePadStatus {
                stick_x: 255,
                stick_y: 125,
                c_stick_x: 255,
                c_stick_y: 0,
                left_trigger: trigger_raw_from_origin(7, source_shield_trigger()),
                right_trigger: 4,
                buttons: GameCubeButtonState::empty().with_x(true),
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ];
    let melee = mapper.map_ports_to_melee_snapshots(moved_ports);
    let line = InputReadout::from_wup_ports_with_melee_snapshots(moved_ports, melee).to_json_line();

    assert!(line.contains("\"melee\":{"));
    assert!(line.contains("\"lstick_x\":127"));
    assert!(line.contains("\"lstick_y\":0"));
    assert!(line.contains("\"prev_lstick_x\":0"));
    assert!(line.contains("\"x_tap_timer\":0"));
    assert!(line.contains("\"trigger_timer\":0"));
    assert!(line.contains("\"trigger_timer\":0,\"left_trigger\":89,\"right_trigger\":0,"));
    assert!(line.contains("\"tilt_direction_x\":1"));
    assert!(line.contains("\"horizontal_smash_direction\":1"));
    assert!(line.contains("\"dash_direction\":1"));
    assert!(line.contains("\"tap_jump\":false"));
    assert!(line.contains("\"button_jump_pressed\":true"));
    assert!(line.contains("\"button_jump_held\":true"));
    assert!(line.contains("\"cstick_jump\":false"));
    assert!(line.contains("\"jump_input\":\"xy\""));
    assert!(line.contains("\"lstick_jump_released\":true"));
    assert!(line.contains("\"cstick_jump_released\":true"));
    assert!(line.contains("\"source_held_bits\":2147484672"));
    assert!(line.contains("\"source_pressed_bits\":2147484672"));
    assert!(line.contains("\"source_released_bits\":0"));
    assert!(line.contains("\"source_lr_held\":true"));
    assert!(line.contains("\"source_lr_pressed\":true"));
    assert!(line.contains("\"source_lr_released\":false"));
    assert!(line.contains("\"source_a_held\":false"));
    assert!(line.contains("\"source_a_pressed\":false"));
    assert!(line.contains("\"source_z_held\":false"));
    assert!(line.contains("\"source_z_pressed\":false"));
    assert!(line.contains("\"fast_fall\":false"));
    assert!(line.contains("\"attack_pressed\":false"));
    assert!(line.contains("\"neutral_attack_pressed\":false"));
    assert!(line.contains("\"smash_attack_direction_x\":0"));
    assert!(line.contains("\"shield_pressed\":true"));
    assert!(line.contains("\"analog_shield\":89"));
    assert!(line.contains("\"analog_shield_pressed\":true"));
    assert!(line.contains("\"digital_shield_pressed\":false"));
    assert!(line.contains("\"air_dodge_pressed\":false"));
    assert!(line.contains("\"left_trigger_analog_held\":true"));
    assert!(line.contains("\"right_trigger_analog_held\":false"));
    assert!(line.contains("\"left_trigger_analog_pressed\":true"));
    assert!(line.contains("\"right_trigger_analog_pressed\":false"));
    assert!(line.contains("\"cstick_x\":89"));
    assert!(line.contains("\"cstick_y\":-89"));
    assert!(line.contains("\"cstick_smash_direction_x\":1"));
    assert!(line.contains("\"cstick_smash_direction_y\":0"));
}

fn connected_wup_pad(pad: GameCubePadStatus) -> [WupPort; 4] {
    [
        WupPort {
            connected: true,
            pad,
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ]
}

struct ScriptedInputSource {
    input: PlayerInput,
}

impl InputSource for ScriptedInputSource {
    fn poll_inputs(&mut self, _frame: Frame) -> [PlayerInput; 2] {
        [self.input, PlayerInput::neutral()]
    }
}
