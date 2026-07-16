use std::{fs, path::Path, time::Instant};

use mole_core::collision::{
    source_damage_result_for_victim, source_grab_confirms, Capsule3, SourceCollisionCapsule,
    SourceDamageResultInput, SourceDamageStage, SourceHitboxAttributes, Vec3,
    SOURCE_HIT_ELEMENT_CATCH, SOURCE_HURT_HEIGHT_HIGH, SOURCE_HURT_HEIGHT_LOW,
    SOURCE_SHIELD_HURTBOX_ID,
};
use mole_core::{
    source_units_to_milli, step_world, EcbDiamond, FighterEntryPlatformProfile, FighterProfile,
    Frame, GameCubeButtonState, GameCubePadStatus, MatchPhase, MeleeActionStateId, MeleeCommonData,
    MotionState, PlayerInput, SourceActionKey, SourcePosePoint, SourceVec2, SourceVec3,
    StageProfile, Vec2, WalkSpeedBucket, World, PLAYER_STATE_NONE,
    SOURCE_COLLISION_STATE_HURT_INTANGIBLE, TICK_NANOS, UCF_DASHBACK_AMENDMENT_BIT,
};
use mole_runtime::{
    apply_source_collisions_for_world, compare_slippi_export_from_match_start_with_core,
    compare_slippi_export_with_core, first_slippi_divergence_from_match_start_with_core,
    frame_pacing_coarse_sleep_nanos, frame_pacing_sleep_nanos, legacy_animation_for_motion_state,
    map_gamecube_pad_to_player_input, map_physical_input, native_replay_path,
    packaged_asset_root_for_exe, parse_wup_report, preload_runtime_source_frame_data,
    project_asset_root, render_source_hitbox_selection, render_source_hurtbox_selection,
    runtime_source_frame_data_is_preloaded, scan_slippi_export_from_match_start_with_core,
    slippi_core_report_path, slippi_match_start_world_from_export,
    slippi_visual_replay_divergence_for_frame, slippi_visual_replay_inputs_from_match_start,
    source_collision_frame_from_frame, source_collision_hits_from_frame,
    source_collision_step_from_frame, source_damage_results_from_frame,
    source_damage_results_from_stages_for_frame, source_damage_stages_from_frame,
    source_hit_confirms_from_frame, step_world_with_source_collision_step,
    step_world_with_source_collisions, trace_slippi_export_from_match_start_with_core,
    visual_platform_ledge_probe_world, write_slippi_core_trace_report, ControllerInputTraceLog,
    DebugOverlay, DolphinMoleVisualProfile, FixedStepClock, FrameDebugLog, InputReadout,
    InputSource, InputTraceWriter, LegacyAnimationKey, LegacySpriteCue, NetplayLogEvent,
    NetplayLogRole, PhysicalInput, RenderCameraState, RenderCapsule, RenderColor, RenderFrame,
    RenderRect, RenderScene, RenderTransform, ReplayCapture, SlippiCoreComparisonConfig,
    SlippiCoreDivergenceKind, SlippiCoreDivergenceScanConfig, SlippiCoreTraceConfig,
    SlippiVisualReplayDivergenceGate, UdpRuntimeConfig, UdpRuntimeStats, WupInputConfig,
    WupInputMapper, WupPort, LEGACY_DOLPHIN_MOLE_ANIMATIONS,
};
use mole_transport::{InputPacket, PacketAcceptResult};

fn read_slippi_match_start_fixture_export() -> Option<String> {
    let slippi_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate should be inside workspace")
        .join("debug")
        .join("slippi");
    for filename in [
        "Game_20260530T214929.inputs.json",
        "Game_20260530T214929.full.inputs.json",
    ] {
        if let Ok(text) = fs::read_to_string(slippi_dir.join(filename)) {
            return Some(text);
        }
    }
    None
}

#[test]
fn bounded_netplay_log_writes_compact_jsonl_and_caps_once() {
    let mut bytes = Vec::new();
    let mut logger = mole_runtime::BoundedNetplayLogger::new(&mut bytes, 190);

    assert!(logger
        .write_event(
            NetplayLogEvent::new(NetplayLogRole::HeadlessPeer, "session_start")
                .with_room_code("ABCD12")
                .with_peer_id("P2PEER")
                .with_frame(Frame(0))
        )
        .unwrap());
    assert!(!logger
        .write_event(
            NetplayLogEvent::new(NetplayLogRole::HeadlessPeer, "frame_summary")
                .with_message("this deliberately long summary should exceed the remaining cap")
        )
        .unwrap());
    assert!(!logger
        .write_event(NetplayLogEvent::new(
            NetplayLogRole::HeadlessPeer,
            "another_summary"
        ))
        .unwrap());

    drop(logger);
    let text = String::from_utf8(bytes).unwrap();
    assert_eq!(text.matches("\"event\":\"log_cap_reached\"").count(), 1);
    assert!(text.contains("\"role\":\"headless_peer\""));
    assert!(text.contains("\"room_code\":\"ABCD12\""));
    assert!(text.lines().count() <= 2);
}

#[test]
fn netplay_log_event_serializes_structured_rollback_diagnostics() {
    let mut stats = UdpRuntimeStats::default();
    let packet =
        InputPacket::new(Frame(42), 1, PlayerInput::neutral(), 0xFEED).with_timing_probe(42, 40);
    stats.record_accept_at(Frame(44), PacketAcceptResult::Accepted, packet);
    stats.record_missing_remote_frame();
    stats.record_rollback_correction();

    let event = NetplayLogEvent::new(NetplayLogRole::VisibleHost, "frame_summary")
        .with_frame(Frame(44))
        .with_netplay_stats(&stats)
        .with_world_checksum(0xCAFE)
        .with_packet_bundle_len(8)
        .with_pacing(1_010_000, true, false);

    let encoded = serde_json::to_value(event).expect("event should serialize");

    assert_eq!(encoded["sent_packets"], serde_json::Value::Null);
    assert_eq!(encoded["received_packets"], 1);
    assert_eq!(encoded["missing_remote_frames"], 1);
    assert_eq!(encoded["rollback_corrections"], 1);
    assert_eq!(encoded["last_remote_frame"], 42);
    assert_eq!(encoded["last_remote_checksum"], 0xFEED);
    assert_eq!(encoded["last_rtt_frames"], 4);
    assert_eq!(encoded["world_checksum"], 0xCAFE);
    assert_eq!(encoded["packet_bundle_len"], 8);
    assert_eq!(encoded["speed_ppm"], 1_010_000);
    assert_eq!(encoded["advance_online_frame"], true);
    assert_eq!(encoded["skip_online_frame"], false);
}

#[test]
fn runtime_crate_uses_compact_source_export_not_generated_capsule_table() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo_toml = fs::read_to_string(manifest_dir.join("Cargo.toml")).unwrap();
    let lib_rs = fs::read_to_string(manifest_dir.join("src/lib.rs")).unwrap();

    assert!(
        cargo_toml.contains("mole_frame_data"),
        "runtime should use the Rust JObj/FigaTree evaluator over compact exported source data"
    );
    assert!(
        lib_rs.contains("source_frame_data"),
        "runtime should import the compact source export module"
    );
    assert!(
        lib_rs.contains("preload_runtime_source_frame_data"),
        "runtime should preload embedded source frame data before gameplay"
    );
    assert!(
        lib_rs.contains("SOURCE_FRAME_CAPSULES_BYTES"),
        "runtime should load the CLI-baked source action/frame capsule sidecar"
    );
    assert!(
        lib_rs.contains("RuntimeSourceExportEvaluator::from_export"),
        "runtime must construct typed evaluators over compact source JObj/FigaTree data for live pose sampling"
    );
    assert!(
        lib_rs.contains("sample_live_pose"),
        "runtime capture/down-bound pose metadata must sample live source JObj world positions at f32 animation time"
    );
    assert!(
        !lib_rs.contains("sample_action_keyframes_from_export"),
        "runtime must not build JSON sampled frame caches from compact source data"
    );
    assert!(
        !lib_rs.contains("serde_json::Value"),
        "runtime collision sampling must use typed compact evaluator output, not generated JSON views"
    );
    assert!(
        lib_rs.contains("frames: Vec<RuntimeSourceFrameCapsules>"),
        "runtime preload should bake compact source capsules into an in-memory action/frame cache"
    );
    assert!(
        lib_rs.contains("live_pose_evaluator: RuntimeActionFrameEvaluator"),
        "runtime source action cache should retain a compact evaluator for live JObj pose metadata"
    );
    assert!(
        !lib_rs.contains("frame_data_boxes"),
        "runtime must not consume generated per-frame capsule primitive tables"
    );
    assert!(
        !lib_rs.contains(".research") && !lib_rs.contains("resources/melee/raw"),
        "player runtime must not depend on decomp or raw Melee resource paths"
    );
}

#[test]
fn runtime_preloads_embedded_source_frame_data_before_gameplay() {
    let loaded_actions =
        preload_runtime_source_frame_data().expect("embedded runtime source data should preload");

    assert!(loaded_actions > 0);
    assert!(runtime_source_frame_data_is_preloaded());
}

#[test]
fn runtime_source_capsules_are_baked_during_preload_not_sampled_during_gameplay() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_rs = fs::read_to_string(manifest_dir.join("src/lib.rs")).unwrap();
    let generated_rs = fs::read_to_string(manifest_dir.join("src/generated/source_frame_data.rs"))
        .expect("generated runtime source frame data module should exist");
    let lookup_start = lib_rs
        .find("fn runtime_source_frame_capsules_ref")
        .expect("runtime source borrowed capsule lookup should exist");
    let lookup_end = lib_rs[lookup_start..]
        .find("fn runtime_source_actions")
        .map(|offset| lookup_start + offset)
        .expect("runtime source action cache accessor should follow lookup");
    let lookup_body = &lib_rs[lookup_start..lookup_end];
    let load_start = lib_rs
        .find("fn load_all_runtime_source_actions")
        .expect("runtime source action preload should exist");
    let load_end = lib_rs[load_start..]
        .find("impl RuntimeSourceAction")
        .map(|offset| load_start + offset)
        .expect("runtime source frame helpers should follow preload");
    let load_body = &lib_rs[load_start..load_end];

    assert!(
        !lookup_body.contains(".sample_frame_capsules("),
        "gameplay/render capsule lookup must use pre-baked compact frame data, not sample source FigaTree data per frame"
    );
    assert!(
        !load_body.contains(".sample_frame_capsules("),
        "player runtime preload must deserialize CLI-baked source capsules, not sample source FigaTree data"
    );
    assert!(
        load_body.contains("RuntimeSourceExportEvaluator")
            && load_body.contains("action_frame_evaluator"),
        "player runtime preload should construct typed compact evaluators for live JObj pose metadata"
    );
    assert!(
        !lib_rs.contains("fn runtime_source_frame_capsules("),
        "runtime capsule lookup should borrow baked frame data instead of cloning capsule vectors per frame"
    );
    assert!(
        generated_rs.contains("SOURCE_FRAME_CAPSULES_BYTES")
            && generated_rs.contains("source_frame_capsules.bin"),
        "CLI runtime export should include a baked source capsule sidecar"
    );
    assert!(
        generated_rs.contains("SOURCE_MANIFEST_JSON")
            && generated_rs.contains("source_manifest.json")
            && generated_rs.contains("RuntimeFigatreeChunk")
            && generated_rs.contains("SOURCE_FIGATREE_BUNDLE_BYTES")
            && generated_rs.contains("source_figatree_bundle.bin")
            && !generated_rs.contains(".figatree.bin"),
        "CLI runtime export should include compact source manifest and one bundled FigaTree sidecar for live pose sampling"
    );
}

#[test]
fn runtime_source_generated_artifacts_are_compact_bundle_only() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let sidecar_dir = manifest_dir.join("src/generated/source_frame_data");
    let mut sidecars = fs::read_dir(&sidecar_dir)
        .expect("runtime source frame data sidecar directory should exist")
        .map(|entry| {
            entry
                .expect("runtime source frame data sidecar should be readable")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();
    sidecars.sort();

    assert_eq!(
        sidecars,
        vec![
            "source_figatree_bundle.bin".to_string(),
            "source_frame_capsules.bin".to_string(),
            "source_manifest.json".to_string(),
        ],
        "runtime source sidecars should stay compact: one FigaTree bundle, one capsule blob, and one source manifest"
    );
    for sidecar in sidecars {
        let path = sidecar_dir.join(&sidecar);
        assert!(
            fs::metadata(&path)
                .expect("runtime source sidecar should have metadata")
                .len()
                > 0,
            "runtime source sidecar {sidecar} should not be empty"
        );
        assert!(
            !sidecar.ends_with(".figatree.bin"),
            "loose per-action FigaTree sidecars should not reappear"
        );
    }
}

#[test]
fn runtime_export_includes_source_only_grab_throw_capture_bindings() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let generated_rs = fs::read_to_string(manifest_dir.join("src/generated/source_frame_data.rs"))
        .expect("generated runtime source frame data module should exist");

    for (runtime_id, source_table_index, source_key) in [
        (213, 242, "Catch"),
        (216, 244, "CatchWait"),
        (219, 247, "ThrowF"),
        (220, 248, "ThrowB"),
        (221, 249, "ThrowHi"),
        (222, 250, "ThrowLw"),
        (239, 262, "TCaptainThrowF"),
        (240, 263, "TCaptainThrowB"),
        (241, 264, "TCaptainThrowHi"),
        (242, 265, "TCaptainThrowLw"),
        (275, 276, "TCaptainSpecialHi"),
        (223, 251, "CapturePulledHi"),
        (224, 252, "CaptureWaitHi"),
        (226, 254, "CapturePulledLw"),
        (227, 255, "CaptureWaitLw"),
    ] {
        let binding = format!(
            "RuntimeActionBinding {{ melee_motion_state_id: None, source_action_table_index: SourceActionTableIndex::new({source_table_index}), source_action_key: \"{source_key}\", motion_state: None }}"
        );
        assert!(
            generated_rs.contains(&binding),
            "runtime source export must bake source-only common binding {runtime_id}/{source_table_index}/{source_key}"
        );
    }
}

#[test]
fn runtime_action_bindings_do_not_reintroduce_ambiguous_raw_action_state_id_field() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let generated_rs = fs::read_to_string(manifest_dir.join("src/generated/source_frame_data.rs"))
        .expect("generated runtime source frame data module should exist");

    assert!(
        !generated_rs.contains("RuntimeActionBinding { action_state_id:"),
        "generated runtime bindings must use explicit melee_motion_state_id and source_action_table_index fields"
    );
    assert!(
        !generated_rs.contains("pub(crate) action_state_id:"),
        "RuntimeActionBinding must not expose an unqualified action_state_id field"
    );
}

#[test]
fn runtime_capture_pose_sampling_uses_source_anim_frame_not_state_age() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");

    fn pulled_position_for_grabber_anim_frame(anim_frame_milli: i32) -> SourceVec2 {
        let mut world = World::for_two_players();
        let mut grabber = world.players()[0];
        grabber.motion_state_alias = None;
        grabber.motion_state = MotionState::Catch;
        grabber.melee_action_state_id = Some(MeleeActionStateId::new(213));
        grabber.source_action_key = Some(SourceActionKey::new("Catch"));
        grabber.source_action_total_frames = 30;
        grabber.source_victim_index = Some(1);
        grabber.source_x1a5c_index = Some(1);
        grabber.source_position = SourceVec2 { x: 10.0, y: 0.0 };
        grabber.position = grabber.source_position.to_milli();
        grabber.motion_frame = 6;
        grabber.motion_anim_frame_milli = anim_frame_milli;
        grabber.motion_anim_rate_milli = 0;
        assert!(world.set_player_state_for_diagnostic(0, grabber));

        let mut victim = world.players()[1];
        victim.motion_state_alias = None;
        victim.motion_state = MotionState::GuardOn;
        victim.melee_action_state_id = Some(MeleeActionStateId::new(223));
        victim.source_action_key = Some(SourceActionKey::new("CapturePulledHi"));
        victim.source_action_total_frames = 20;
        victim.source_victim_index = Some(0);
        victim.source_x1a5c_index = Some(0);
        victim.source_position = SourceVec2 { x: 20.0, y: 0.0 };
        victim.position = victim.source_position.to_milli();
        victim.motion_frame = 0;
        victim.motion_anim_frame_milli = 0;
        victim.motion_anim_rate_milli = 0;
        assert!(world.set_player_state_for_diagnostic(1, victim));

        step_world_with_source_collisions(
            &mut world,
            Frame(0),
            &[PlayerInput::neutral(), PlayerInput::neutral()],
        );

        world.players()[1].source_position
    }

    let state_age_pose = pulled_position_for_grabber_anim_frame(3_000);
    let source_anim_pose = pulled_position_for_grabber_anim_frame(8_000);

    assert_ne!(
        state_age_pose.x.to_bits(),
        source_anim_pose.x.to_bits(),
        "ftCo capture physics samples the grabber's live JObj world position via lb_8000B1CC; baked runtime capture metadata must be keyed by source animation frame, not motion state age"
    );
}

#[test]
fn runtime_capture_pose_sampling_preserves_fractional_source_anim_time() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");

    fn pulled_position_for_grabber_anim_frame(anim_frame_milli: i32) -> SourceVec2 {
        let mut world = World::for_two_players();
        let mut grabber = world.players()[0];
        grabber.motion_state_alias = None;
        grabber.motion_state = MotionState::Catch;
        grabber.melee_action_state_id = Some(MeleeActionStateId::new(213));
        grabber.source_action_key = Some(SourceActionKey::new("Catch"));
        grabber.source_action_total_frames = 30;
        grabber.source_victim_index = Some(1);
        grabber.source_x1a5c_index = Some(1);
        grabber.source_position = SourceVec2 { x: 10.0, y: 0.0 };
        grabber.position = grabber.source_position.to_milli();
        grabber.motion_frame = 6;
        grabber.motion_anim_frame_milli = anim_frame_milli;
        grabber.motion_anim_rate_milli = 0;
        assert!(world.set_player_state_for_diagnostic(0, grabber));

        let mut victim = world.players()[1];
        victim.motion_state_alias = None;
        victim.motion_state = MotionState::GuardOn;
        victim.melee_action_state_id = Some(MeleeActionStateId::new(226));
        victim.source_action_key = Some(SourceActionKey::new("CapturePulledLw"));
        victim.source_action_total_frames = 20;
        victim.source_victim_index = Some(0);
        victim.source_x1a5c_index = Some(0);
        victim.source_position = SourceVec2 { x: 20.0, y: 0.0 };
        victim.position = victim.source_position.to_milli();
        victim.motion_frame = 0;
        victim.motion_anim_frame_milli = 0;
        victim.motion_anim_rate_milli = 0;
        assert!(world.set_player_state_for_diagnostic(1, victim));

        step_world_with_source_collisions(
            &mut world,
            Frame(0),
            &[PlayerInput::neutral(), PlayerInput::neutral()],
        );

        world.players()[1].source_position
    }

    let whole_frame_pose = pulled_position_for_grabber_anim_frame(6_000);
    let fractional_pose = pulled_position_for_grabber_anim_frame(6_260);

    assert_ne!(
        whole_frame_pose.x.to_bits(),
        fractional_pose.x.to_bits(),
        "decomp capture placement samples live JObj world positions at fp->cur_anim_frame; runtime must not floor source animation time through an integer frame sidecar"
    );
}

#[test]
fn runtime_executable_steps_gameplay_with_source_collision_damage() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let main_rs = fs::read_to_string(manifest_dir.join("src/main.rs")).unwrap();

    assert!(
        main_rs.contains("preload_runtime_source_frame_data()?"),
        "play loops must preload source frame data before ticking"
    );
    assert!(
        main_rs.contains("step_world_with_source_collisions"),
        "play loops must apply source collision and damage as part of world stepping"
    );
    assert!(
        !main_rs.contains("step_world(&mut world"),
        "runtime executable must not bypass collision/damage with bare core stepping"
    );
}

#[test]
fn source_collision_match_start_holds_spawn_fall_until_final_source_ecb_crosses_platform() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    for frame in 0..=75 {
        step_world_with_source_collisions(&mut world, Frame(frame), &neutral);
    }

    assert_eq!(
        world.players()[0].motion_state,
        MotionState::Fall,
        "Falcon Fall's source figatree is non-looping; after frame 7 the runtime ECB must hold the final source pose, keeping the Battlefield side-platform sweep above the floor for this tick"
    );
    assert_eq!(
        world.players()[0].position.y,
        mole_core::melee_units_f32(25.114_899)
    );
    assert!(!world.players()[0].grounded);
    assert_eq!(world.players()[0].source_floor_for_diagnostic().1, None);
    assert_eq!(
        world.players()[0].velocity.y,
        mole_core::melee_units_f32(-1.56),
        "ftCo_Fall_Phys applies gravity before map collision; the held final Fall ECB must not commit the landing until the next source frame"
    );

    step_world_with_source_collisions(&mut world, Frame(76), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::Landing);
    assert_eq!(
        world.players()[0].position.y,
        mole_core::melee_units_f32(27.200_1)
    );
    assert!(world.players()[0].grounded);
    assert_eq!(world.players()[0].source_floor_for_diagnostic().1, Some(2));
    assert_eq!(
        world.players()[0].velocity.y,
        mole_core::melee_units_f32(-1.69),
        "ftCo_Landing_Enter_Basic changes state in map phase; Landing physics clears self_vel.y on the following physics tick, not during the collision callback"
    );
}

#[test]
fn slippi_match_start_frame19_p2_turn_dash_after_stays_grounded_on_battlefield_platform() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(143))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();

    for frame in frames {
        step_world_with_source_collisions(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 19 {
            break;
        }
    }

    assert_eq!(
        world.players()[1].motion_state,
        MotionState::Dash,
        "Slippi source frame 19 keeps P2 grounded through ftCo_Turn_Coll/ft_80083F88 so turn dash-after can enter Dash instead of falling off the Battlefield right platform"
    );
    assert_eq!(
        world.players()[1].position.y,
        mole_core::melee_units_f32(54.400)
    );
}

#[test]
fn slippi_match_start_platform_grounded_y_uses_stage_floor_height_for_position_compare() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };

    let comparison = compare_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreComparisonConfig {
            compare_players: [true, true],
            max_frames: Some(180),
        },
    )
    .expect("match-start replay fixture should compare");

    assert_ne!(
        comparison
            .first_position_drift
            .map(|drift| (drift.player_index, drift.source_frame)),
        Some((1, 49)),
        "Slippi reports grounded platform post-frame y relative to the contacted floor; the comparator should lift it to Battlefield's right-platform height before reporting drift"
    );
}

#[test]
fn slippi_match_start_frame20_p2_entry_end_handoff_preserves_source_platform_y() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };

    let comparison = compare_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreComparisonConfig {
            compare_players: [true, true],
            max_frames: Some(180),
        },
    )
    .expect("match-start replay fixture should compare");

    assert_ne!(
        comparison
            .first_position_drift_by_player[1]
            .map(|drift| drift.source_frame),
        Some(20),
        "EntryEnd must route through source Entry collision and ftCommon_8007D92C instead of forcing a local Fall/ECB shortcut that drops P2 below Slippi's platform Y"
    );
}

#[test]
fn slippi_match_start_frame20_p2_dash_collision_enters_fall_at_top_platform_edge() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };

    let comparison = compare_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreComparisonConfig {
            compare_players: [true, true],
            max_frames: Some(180),
        },
    )
    .expect("match-start replay fixture should compare");

    assert_ne!(
        comparison
            .first_state_mismatch
            .map(|mismatch| (mismatch.player_index, mismatch.source_frame)),
        Some((1, 20)),
        "ftCo_Dash_Coll must route through ft_800844EC/ft_80082708 and enter Fall when mpColl no longer supports P2 on Battlefield top platform"
    );
}

#[test]
fn slippi_match_start_frame48_landing_turn_dash_retains_main_floor_support() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };

    let comparison = compare_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreComparisonConfig {
            compare_players: [true, true],
            max_frames: Some(190),
        },
    )
    .expect("match-start replay fixture should compare");

    assert_ne!(
        comparison
            .first_state_mismatch
            .map(|mismatch| (mismatch.player_index, mismatch.source_frame)),
        Some((1, 48)),
        "ftCo_Landing_Coll must run ft_80084280 before Landing IASA routes through Turn into Dash, preserving fresh main-floor CollData for ft_800844EC"
    );
}

#[test]
fn slippi_match_start_frame_minus59_p1_entry_end_air_handoff_runs_first_fall_physics_tick() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };

    let comparison = compare_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreComparisonConfig {
            compare_players: [true, true],
            max_frames: Some(120),
        },
    )
    .expect("match-start replay fixture should compare");

    assert_ne!(
        comparison
            .first_position_drift
            .map(|drift| (drift.player_index, drift.source_frame)),
        Some((0, -59)),
        "EntryEnd_Anim's ftCommon_8007D92C air branch must enter Fall early enough for the first ftCo_Fall_Phys gravity tick seen by Slippi"
    );
}

#[test]
fn slippi_match_start_frame66_p2_dash_keeps_right_platform_edge_support() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };

    let comparison = compare_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreComparisonConfig {
            compare_players: [true, true],
            max_frames: Some(220),
        },
    )
    .expect("match-start replay fixture should compare");

    assert_ne!(
        comparison
            .first_state_mismatch
            .map(|mismatch| (mismatch.player_index, mismatch.source_frame)),
        Some((1, 66)),
        "Dash collision should keep P2 grounded for the first right-platform left-edge overstep once earlier replay boundaries are in parity"
    );
}

#[test]
fn slippi_match_start_frame68_p2_dash_uses_source_floor_fallback() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };

    let comparison = compare_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreComparisonConfig {
            compare_players: [true, true],
            max_frames: Some(220),
        },
    )
    .expect("match-start replay fixture should compare");

    assert_ne!(
        comparison
            .first_state_mismatch
            .map(|mismatch| (mismatch.player_index, mismatch.source_frame)),
        Some((1, 68)),
        "mpColl_8004ACE4 should keep P2 in Dash through the source floor fallback after the right-platform edge overlap no longer reaches the edge"
    );
}

#[test]
fn slippi_match_start_frame71_p2_turn_uses_source_ground_support() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };

    let comparison = compare_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreComparisonConfig {
            compare_players: [true, true],
            max_frames: Some(220),
        },
    )
    .expect("match-start replay fixture should compare");

    assert_ne!(
        comparison
            .first_position_drift
            .map(|drift| (drift.player_index, drift.source_frame)),
        Some((1, 71)),
        "ftCo_Turn_Coll should route through source ground support instead of clamping the retained platform floor to the coarse surface endpoint"
    );
}

#[test]
fn slippi_match_start_frame76_p2_dash_jump_enters_kneebend_on_retained_source_floor() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };

    let comparison = compare_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreComparisonConfig {
            compare_players: [true, true],
            max_frames: Some(220),
        },
    )
    .expect("match-start replay fixture should compare");

    assert_ne!(
        comparison
            .first_state_mismatch
            .map(|mismatch| (mismatch.player_index, mismatch.source_frame)),
        Some((1, 76)),
        "ftCo_KneeBend_Coll should use ft_80083F88/ft_80082708 source ground support so a dash jump can enter jumpsquat before the retained floor falls away"
    );
}

#[test]
fn slippi_match_start_frame55_p2_escape_air_lands_on_solid_floor() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };

    let comparison = compare_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreComparisonConfig {
            compare_players: [false, true],
            max_frames: Some(80),
        },
    )
    .expect("match-start replay fixture should compare");

    assert!(
        comparison.first_state_mismatch.is_none(),
        "solid Battlefield floor contact must still enter LandingFallSpecial at source frame 55; first mismatch was {:?}",
        comparison.first_state_mismatch
    );
}

#[test]
fn slippi_match_start_frame92_p2_escape_air_source_collision_enters_landing_fall_special() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };

    let comparison = compare_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreComparisonConfig {
            compare_players: [true, true],
            max_frames: Some(240),
        },
    )
    .expect("match-start replay fixture should compare");

    assert_ne!(
        comparison
            .first_state_mismatch
            .map(|mismatch| (mismatch.player_index, mismatch.source_frame)),
        Some((1, 92)),
        "ftCo_EscapeAir_Coll should route ft_80082C74's source floor callback into LandingFallSpecial on the airdodge contact frame"
    );
}

#[test]
fn slippi_match_start_frame281_p2_escape_air_source_collision_enters_landing_fall_special() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };

    let comparison = compare_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreComparisonConfig {
            compare_players: [true, true],
            max_frames: Some(430),
        },
    )
    .expect("match-start replay fixture should compare");

    assert_ne!(
        comparison
            .first_state_mismatch
            .map(|mismatch| (mismatch.player_index, mismatch.source_frame)),
        Some((1, 281)),
        "ftCo_EscapeAir_Coll must route ft_80082C74's floor callback into ftCo_80099D70/ftCo_LandingFallSpecial_Enter at the second P2 airdodge landing"
    );
}

#[test]
fn slippi_match_start_frame142_p2_double_jump_ecb_lock_avoids_false_platform_commit() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let scan = scan_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreDivergenceScanConfig {
            compare_players: [false, true],
            max_frames: Some(300),
            ..SlippiCoreDivergenceScanConfig::default()
        },
    )
    .expect("match-start replay fixture should scan");
    assert!(
        scan.scenarios
            .iter()
            .all(|scenario| !(scenario.player_index == 1 && scenario.source_frame == 142)),
        "ftCo_JumpAerial_Enter_Basic must reset ftCommon_8007D5D4's ECB lock so the later EscapeAir pose change does not manufacture a soft-platform crossing"
    );

    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(300))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();

    for frame in frames {
        step_world_with_source_collisions(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 142 {
            break;
        }
    }

    let player = world.players()[1];
    assert_eq!(player.motion_state, MotionState::EscapeAir);
    assert!(!player.grounded);
    assert_eq!(
        player.position,
        Vec2 {
            x: 32_781,
            y: 23_112
        }
    );
}

#[test]
fn slippi_match_start_frame_negative48_p1_fall_platform_commit_is_mixed_phase_witness() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let scan = scan_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreDivergenceScanConfig {
            compare_players: [true, false],
            max_frames: Some(120),
            ..SlippiCoreDivergenceScanConfig::default()
        },
    )
    .expect("match-start replay fixture should scan");
    let frame_negative48 = scan
        .scenarios
        .iter()
        .find(|scenario| scenario.player_index == 0 && scenario.source_frame == -48)
        .expect("frame -48 P1 should be represented in the divergence scan");
    assert_eq!(
        frame_negative48.kind,
        SlippiCoreDivergenceKind::MixedPhaseWitness
    );
}

#[test]
fn slippi_match_start_frame2625_p2_wait_platform_commit_stays_aligned_after_ecb_lock_fix() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let scan = scan_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreDivergenceScanConfig {
            compare_players: [false, true],
            max_frames: Some(2800),
            ..SlippiCoreDivergenceScanConfig::default()
        },
    )
    .expect("match-start replay fixture should scan");
    let frame2625 = scan
        .scenarios
        .iter()
        .find(|scenario| scenario.player_index == 1 && scenario.source_frame == 2625);
    assert!(
        frame2625.is_none(),
        "source frame 2625 should remain aligned after ecb_lock is consumed in the Fighter_procMap phase, not tracked as a live divergence"
    );
}

#[test]
fn slippi_match_start_frame2686_p2_damage_lw3_stays_aligned_after_damage_anim_rate_fix() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let scan = scan_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreDivergenceScanConfig {
            compare_players: [false, true],
            max_frames: Some(2_820),
            ..SlippiCoreDivergenceScanConfig::default()
        },
    )
    .expect("match-start replay fixture should scan");
    let frame2686 = scan
        .scenarios
        .iter()
        .find(|scenario| scenario.player_index == 1 && scenario.source_frame == 2686);
    assert!(
        frame2686.is_none(),
        "DamageLw3 entry must seed frame_speed_mul=1.0F and mpColl_LoadECB_JObj must sample the post-ftAnim pose, keeping source 2686 aligned instead of snapping to Landing one row early"
    );
}

#[test]
fn slippi_match_start_comparison_carries_first_non_witness_divergence() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let comparison = compare_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreComparisonConfig {
            compare_players: [false, true],
            max_frames: Some(3_200),
        },
    )
    .expect("match-start replay fixture should compare");
    let first = comparison
        .first_divergence
        .expect("comparison should carry the first classified divergence");

    assert_eq!(first.kind, SlippiCoreDivergenceKind::StateMismatch);
    assert_eq!(first.player_index, 1);
    assert_eq!(first.source_frame, 2586);
    assert_eq!(first.core_frame, Frame(2709));
}

#[test]
fn visual_replay_gate_skips_proven_telemetry_and_pauses_on_first_state_mismatch() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(3_200))
        .expect("visual replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let mut gate = SlippiVisualReplayDivergenceGate::new(0);
    let mut stop = None;

    for frame in frames.iter() {
        step_world_with_source_collisions(&mut world, frame.core_frame, &frame.inputs);
        let divergence = slippi_visual_replay_divergence_for_frame(
            frame,
            world.snapshot(),
            SlippiCoreDivergenceScanConfig::default(),
        );
        if let Some(divergence) = gate.observe(divergence) {
            stop = Some(divergence);
            break;
        }
        if frame.source_frame >= 2584 {
            break;
        }
    }

    let stop = stop.expect("visual replay should stop at the first engine-state disagreement");
    assert_eq!(
        stop.source_frame, 2583,
        "same-state rows with exact source velocity components are telemetry witnesses, but the SpecialSStart/SpecialS state mismatch must stop immediately"
    );
    assert_eq!(
        stop.kind,
        SlippiCoreDivergenceKind::StateMismatch,
        "source frame 2583 is the first actual state disagreement after proven telemetry witnesses"
    );
}

#[test]
fn slippi_match_start_frame_negative47_p1_landing_velocity_drift_cascades_from_floor_commit_witness(
) {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let scan = scan_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreDivergenceScanConfig {
            compare_players: [true, false],
            max_frames: Some(120),
            ..SlippiCoreDivergenceScanConfig::default()
        },
    )
    .expect("match-start replay fixture should scan");
    let frame_negative47 = scan
        .scenarios
        .iter()
        .find(|scenario| scenario.player_index == 0 && scenario.source_frame == -47)
        .expect("frame -47 P1 should be represented in the divergence scan");
    assert_eq!(
        frame_negative47.kind,
        SlippiCoreDivergenceKind::VelocityDrift
    );
    assert_eq!(
        frame_negative47.cascades_from_source_frame,
        Some(-48),
        "ftCommon_ApplyGroundMovement clears self_vel.y on the first flat-floor Landing physics tick; Slippi's stale y-speed row should cascade from the proven frame -48 floor-commit witness"
    );
}

#[test]
fn slippi_first_divergence_reports_first_non_witness_divergence() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let first = first_slippi_divergence_from_match_start_with_core(
        &export,
        SlippiCoreDivergenceScanConfig {
            compare_players: [true, true],
            max_frames: Some(3_200),
            ..SlippiCoreDivergenceScanConfig::default()
        },
    )
    .expect("match-start replay fixture should scan first divergence");

    let first =
        first.expect("frame-by-frame parity should report the first non-witness divergence");
    assert_eq!(first.kind, SlippiCoreDivergenceKind::StateMismatch);
    assert_eq!(first.player_index, 0);
    assert_eq!(first.source_frame, 2583);
}

#[test]
fn slippi_match_start_frame390_p1_dash_position_drift_cascades_from_p2_unresolved_root() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let scan = scan_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreDivergenceScanConfig {
            compare_players: [true, true],
            max_frames: Some(650),
            max_scenarios: Some(30),
            ..SlippiCoreDivergenceScanConfig::default()
        },
    )
    .expect("match-start replay fixture should scan");
    let frame390 = scan
        .scenarios
        .iter()
        .find(|scenario| scenario.player_index == 0 && scenario.source_frame == 390)
        .expect("frame 390 P1 should be represented in the divergence scan");

    assert_eq!(frame390.kind, SlippiCoreDivergenceKind::PositionDrift);
    assert_eq!(frame390.first_frame.actual_motion_state, MotionState::Dash);
    assert_eq!(
        frame390.first_frame.expected_motion_state,
        Some(MotionState::Dash)
    );
    assert_eq!(
        frame390.first_frame.actual_velocity_x - frame390.first_frame.expected_ground_velocity_x,
        0,
        "P1 frame 390 agrees on Dash self velocity; the position drift begins when P2's unresolved frame-142 worldline prevents decomp player-nudge overlap"
    );
    assert_eq!(
        frame390.cascades_from_source_frame,
        Some(142),
        "cross-player player-nudge drift must be classified as cascading from the unresolved P2 mixed-phase root"
    );
}

#[test]
fn runtime_escape_air_frame212_p2_lands_when_decomp_floor_sweep_crosses_platform() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let mut player = world.players()[1];
    player.motion_state = MotionState::JumpAerialF;
    player.set_motion_state_alias(MotionState::JumpAerialF);
    player.melee_action_state_id = Some(MeleeActionStateId::new(27));
    player.source_action_key = Some(SourceActionKey::new("JumpAerialF"));
    player.source_action_total_frames = 50;
    player.motion_frame = 11;
    player.set_source_motion_anim_frame(11.0);
    player.position = Vec2 {
        x: source_units_to_milli(32.561_07),
        y: source_units_to_milli(28.650_091),
    };
    player.velocity = Vec2 {
        x: source_units_to_milli(0.969_500_06),
        y: source_units_to_milli(1.229_999_3),
    };
    player.grounded = false;
    assert!(world.set_player_state_for_diagnostic(1, player));

    step_world_with_source_collisions(
        &mut world,
        Frame(208),
        &[
            PlayerInput::neutral(),
            PlayerInput::neutral().with_left_stick(-114, -56),
        ],
    );
    assert_eq!(world.players()[1].motion_state, MotionState::JumpAerialF);

    let shield_airdodge = PlayerInput::neutral()
        .with_left_stick(-113, -56)
        .with_left_trigger_analog(255)
        .with_left_trigger_digital(true);
    step_world_with_source_collisions(
        &mut world,
        Frame(209),
        &[PlayerInput::neutral(), shield_airdodge],
    );
    assert_eq!(world.players()[1].motion_state, MotionState::EscapeAir);

    step_world_with_source_collisions(
        &mut world,
        Frame(210),
        &[PlayerInput::neutral(), shield_airdodge],
    );
    assert_eq!(world.players()[1].motion_state, MotionState::EscapeAir);

    step_world_with_source_collisions(
        &mut world,
        Frame(211),
        &[PlayerInput::neutral(), shield_airdodge],
    );
    assert_eq!(world.players()[1].motion_state, MotionState::EscapeAir);

    step_world_with_source_collisions(
        &mut world,
        Frame(212),
        &[PlayerInput::neutral(), shield_airdodge],
    );

    let player = world.players()[1];
    assert_eq!(
        player.motion_state,
        MotionState::LandingFallSpecial,
        "ftCo_EscapeAir_Coll routes ft_80082C74/mpColl_800471F8 floor contact into LandingFallSpecial as soon as the ECB bottom crosses the platform"
    );
    assert!(player.grounded);
    assert_eq!(player.position.y, source_units_to_milli(27.200_1));

    step_world_with_source_collisions(
        &mut world,
        Frame(213),
        &[PlayerInput::neutral(), shield_airdodge],
    );

    let player = world.players()[1];
    assert_eq!(
        player.motion_state,
        MotionState::LandingFallSpecial,
        "LandingFallSpecial should continue after the decomp contact frame"
    );
    assert!(player.grounded);
    assert_eq!(player.position.y, source_units_to_milli(27.200_1));
}

#[test]
fn slippi_match_start_frame1383_p2_landing_fall_special_uses_source_landing_collision() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };

    let comparison = compare_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreComparisonConfig {
            compare_players: [true, true],
            max_frames: Some(1510),
        },
    )
    .expect("match-start replay fixture should compare");

    assert_ne!(
        comparison
            .first_state_mismatch
            .map(|mismatch| (mismatch.player_index, mismatch.source_frame)),
        Some((1, 1383)),
        "LandingFallSpecial must run its decomp collision callback before falling instead of using a coarse pre-collision floor support shortcut"
    );
}

#[test]
fn slippi_match_start_frame1378_p1_landing_jump_iasa_runs_kneebend_ground_physics() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };

    let scan = scan_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreDivergenceScanConfig {
            compare_players: [true, true],
            max_frames: Some(1520),
            lookahead_frames: 5,
            max_scenarios: Some(1),
            position_tolerance_milli: 100,
            velocity_tolerance_milli: 1,
        },
    )
    .expect("match-start replay fixture should scan");

    assert!(
        scan.scenarios.is_empty(),
        "Landing IASA jump must enter ftCo_KneeBend, then run ftCo_KneeBend_Phys/ft_80084F3C before the update position add; scenarios={:?}",
        scan.scenarios
    );
}

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
fn frame_pacing_subtracts_work_from_sixty_hz_sleep_budget() {
    assert_eq!(frame_pacing_sleep_nanos(0), TICK_NANOS);
    assert_eq!(frame_pacing_sleep_nanos(5_000_000), TICK_NANOS - 5_000_000);
    assert_eq!(frame_pacing_sleep_nanos(TICK_NANOS), 0);
    assert_eq!(frame_pacing_sleep_nanos(TICK_NANOS + 1), 0);
    assert_eq!(frame_pacing_coarse_sleep_nanos(8_000_000), 0);
    assert_eq!(frame_pacing_coarse_sleep_nanos(6_000_000), 0);
    assert_eq!(frame_pacing_coarse_sleep_nanos(5_000_000), 0);
    assert_eq!(frame_pacing_coarse_sleep_nanos(2_000_000), 0);
    assert_eq!(frame_pacing_coarse_sleep_nanos(1_000_000), 0);
}

#[test]
fn runtime_frame_loops_do_not_sleep_a_full_tick_after_work() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let main_rs = fs::read_to_string(manifest_dir.join("src/main.rs")).unwrap();

    assert!(
        main_rs.contains("wait_until_frame_deadline"),
        "runtime loops should pace to a rolling 60 Hz frame deadline after frame work"
    );
    assert!(
        main_rs.contains("request_high_resolution_frame_timer"),
        "runtime loops should request high-resolution host sleep timing before 60 Hz pacing"
    );
    assert!(
        !main_rs.contains("Duration::from_nanos(TICK_NANOS)"),
        "sleeping a full tick after update/render makes frame time work + 16.67ms"
    );
}

#[test]
fn sdl_timing_reports_uncapped_work_budget_before_sixty_hz_cap() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let main_rs = fs::read_to_string(manifest_dir.join("src/main.rs")).unwrap();

    assert!(
        main_rs.contains("--no-frame-cap"),
        "SDL smoke should expose an uncapped perf mode while keeping the normal path capped"
    );
    assert!(
        main_rs.contains("avg_work_ms") && main_rs.contains("uncapped_work_fps"),
        "SDL timing output should report real work cost separately from 60 Hz sleep"
    );
    assert!(
        main_rs.contains("UNCAPPED_WORK_BUDGET_TARGET_FPS: f64 = 240.0"),
        "uncapped perf target should encode the four-player 60 Hz sanity budget"
    );
    assert!(
        main_rs.contains("if frame_cap_enabled {")
            && main_rs.contains("wait_until_frame_deadline(next_frame_deadline);"),
        "normal SDL runtime should remain capped to a rolling frame deadline while perf smoke can skip the cap"
    );
}

#[test]
fn sdl_runtime_disables_renderer_vsync_for_fixed_step_pacing() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let main_rs = fs::read_to_string(manifest_dir.join("src/main.rs")).unwrap();

    assert!(
        main_rs.contains("disable_sdl_renderer_vsync"),
        "SDL present must not own frame pacing when the engine already paces fixed 60 Hz frames"
    );
    assert!(
        main_rs.contains("SDL_SetRenderVSync") && main_rs.contains("SDL_RENDERER_VSYNC_DISABLED"),
        "SDL renderer vsync must be explicitly disabled so present does not add a second wait"
    );
}

#[test]
fn minimal_runtime_frame_pipeline_stays_inside_sixty_hz_budget() {
    const FRAMES: u32 = 600;
    let mut world = World::for_two_players();
    let inputs = [PlayerInput::neutral(), PlayerInput::neutral()];
    let mut scene_fingerprint = 0usize;
    let mut source_confirm_count = 0usize;

    let started = Instant::now();
    for frame_index in 0..FRAMES {
        step_world(&mut world, Frame(frame_index), &inputs);
        let frame = RenderFrame::from_world(&world);
        source_confirm_count += source_hit_confirms_from_frame(&frame).len();
        let scene = RenderScene::from_frame(&frame, 960, 540);
        scene_fingerprint = scene_fingerprint
            .wrapping_add(scene.players[0].x as usize)
            .wrapping_add(scene.players[1].x as usize)
            .wrapping_add(scene.player_hurtbox_pills[0].len())
            .wrapping_add(scene.player_hurtbox_pills[1].len());
    }
    let elapsed_nanos = started.elapsed().as_nanos();
    let budget_nanos = TICK_NANOS as u128 * FRAMES as u128;
    let average_nanos = elapsed_nanos / FRAMES as u128;

    assert_eq!(world.frame(), Frame(FRAMES));
    assert_ne!(scene_fingerprint, 0);
    assert_eq!(source_confirm_count, 0);
    assert!(
        elapsed_nanos <= budget_nanos,
        "minimal runtime pipeline took {elapsed_nanos}ns for {FRAMES} frames ({average_nanos}ns/frame), exceeding 60 Hz budget {budget_nanos}ns"
    );
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
fn slippi_core_comparison_prefers_game_facing_stick_over_raw_replay_noise() {
    let export = r#"{
  "schema_version": 1,
  "source": {"replay_path": "fixture.slp", "parser": "@slippi/slippi-js/node"},
  "settings": {"stage_id": 31, "players": {"0": {"controller_fix": "UCF"}}},
  "metadata": {"start_at": "2026-05-31T00:00:00Z", "last_frame": 384},
  "export": {"first_frame": 383, "last_frame": 384, "frame_count": 2},
  "frames": [
    {
      "frame": 383,
      "players": {
        "0": {
          "pre": {
            "action_state_id": 42,
            "position": [2.111639, 0.0001],
            "facing": 1,
            "main_stick": [0, 0],
            "c_stick": [0, 0],
            "rust_player_input": {
              "stick_x": 0, "stick_y": 0, "c_stick_x": 0, "c_stick_y": 0,
              "left_trigger": 0, "right_trigger": 0, "physical_button_bits": 0
            }
          },
          "post": {
            "action_state_id": 42,
            "action_state_counter": 9,
            "position": [2.111639, 0.0001],
            "self_induced_speeds": {"ground_x": -0.02557, "air_x": -0.02557, "y": 0}
          }
        }
      }
    },
    {
      "frame": 384,
      "players": {
        "0": {
          "pre": {
            "action_state_id": 42,
            "position": [2.111639, 0.0001],
            "facing": 1,
            "main_stick": [0, 0],
            "c_stick": [0, 0],
            "raw_joystick_x": -20,
            "raw_joystick_y": 3,
            "rust_player_input": {
              "stick_x": -32, "stick_y": 5, "c_stick_x": 0, "c_stick_y": 0,
              "left_trigger": 0, "right_trigger": 0, "physical_button_bits": 0
            }
          },
          "post": {
            "action_state_id": 42,
            "action_state_counter": 10,
            "position": [2.111639, 0.0001],
            "self_induced_speeds": {"ground_x": 0, "air_x": 0, "y": 0}
          }
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
    assert_eq!(
        mismatch.expected_motion_state,
        Some(MotionState::GuardReflect)
    );
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
    assert_eq!(
        mismatch.expected_motion_state,
        Some(MotionState::GuardSetOff)
    );
    assert!(comparison.report_markdown().contains("GuardSetOff (181)"));
}

#[test]
fn slippi_action_state_252_is_supported_as_cliff_catch() {
    let export = minimal_slippi_export(
        r#"{
          "pre": {
            "rust_player_input": {
              "stick_x": 0, "stick_y": 0, "c_stick_x": 0, "c_stick_y": 0,
              "left_trigger": 0, "right_trigger": 0, "physical_button_bits": 0
            }
          },
          "post": {
            "action_state_id": 252,
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
        .expect("default world should not already be in CliffCatch");
    assert_eq!(mismatch.expected_slippi_state_id, 252);
    assert_eq!(
        mismatch.expected_motion_state,
        Some(MotionState::CliffCatch)
    );
    assert!(comparison.report_markdown().contains("CliffCatch (252)"));
}

#[test]
fn slippi_source_only_jab_followup_is_supported_by_canonical_action_id() {
    let export = minimal_slippi_export(
        r#"{
          "pre": {
            "rust_player_input": {
              "stick_x": 0, "stick_y": 0, "c_stick_x": 0, "c_stick_y": 0,
              "left_trigger": 0, "right_trigger": 0, "physical_button_bits": 0
            }
          },
          "post": {
            "action_state_id": 45,
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
        .expect("default world should not already be in canonical Attack12");
    assert_eq!(mismatch.expected_slippi_state_id, 45);
    assert_eq!(
        mismatch.expected_action_state_id,
        MeleeActionStateId::new(45)
    );
    assert_eq!(mismatch.expected_motion_state, None);
    assert!(comparison.report_markdown().contains("Attack12 (45)"));
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
    assert_eq!(
        mismatch.expected_motion_state,
        Some(MotionState::FallSpecialF)
    );
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
    assert_eq!(mismatch.expected_motion_state, Some(MotionState::Wait));
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
fn slippi_core_divergence_scan_groups_realigned_scenarios_with_rollback_replay() {
    let export = r#"{
      "schema_version": 1,
      "source": {"replay_path": "fixture.slp", "parser": "@slippi/slippi-js/node"},
      "settings": {"stage_id": 31, "players": {"0": {"controller_fix": "UCF"}}},
      "metadata": {"start_at": "2026-05-31T00:00:00Z", "last_frame": 1},
      "export": {"first_frame": 0, "last_frame": 1, "frame_count": 2},
      "frames": [
        {"frame": 0, "players": {"0": {
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
            "action_state_id": 14,
            "position": [-38.8, 35.2],
            "self_induced_speeds": {"ground_x": 0.0, "air_x": 0.0, "y": 0.0}
          }
        }}},
        {"frame": 1, "players": {"0": {
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
            "position": [-38.8, 35.2],
            "self_induced_speeds": {"ground_x": 0.0, "air_x": 0.0, "y": 0.0}
          }
        }}}
      ]
    }"#;

    let scan = scan_slippi_export_from_match_start_with_core(
        export,
        SlippiCoreDivergenceScanConfig {
            compare_players: [true, false],
            max_frames: None,
            lookahead_frames: 3,
            max_scenarios: None,
            position_tolerance_milli: 100,
            velocity_tolerance_milli: 1,
        },
    )
    .expect("fixture should parse");

    assert_eq!(scan.frames_scanned, 2);
    assert_eq!(scan.scenarios.len(), 1);
    let scenario = &scan.scenarios[0];
    assert_eq!(scenario.kind, SlippiCoreDivergenceKind::StateMismatch);
    assert_eq!(scenario.player_index, 0);
    assert_eq!(scenario.source_frame, 0);
    assert_eq!(scenario.duration_frames, 1);
    assert_eq!(scenario.realign_source_frame, Some(1));
    assert!(scenario.realigned_within_lookahead);
    assert!(scenario.rollback_replay_deterministic);
    assert_eq!(
        scenario.first_frame.expected_motion_state,
        Some(MotionState::Wait)
    );
    assert_eq!(scenario.first_frame.actual_motion_state, MotionState::Entry);
}

#[test]
fn slippi_scan_treats_source_damage_actions_as_known_source_only_states() {
    let export = minimal_slippi_export(
        r#"{
          "pre": {
            "action_state_id": 79,
            "position": [0.0, 0.0],
            "facing": 1.0,
            "rust_player_input": {
              "stick_x": 0, "stick_y": 0, "c_stick_x": 0, "c_stick_y": 0,
              "left_trigger": 0, "right_trigger": 0, "physical_button_bits": 0
            }
          },
          "post": {
            "action_state_id": 79,
            "position": [0.0, 0.0],
            "self_induced_speeds": {"ground_x": 0.0, "air_x": 0.0, "y": 0.0}
          }
        }"#,
    );

    let scan = scan_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreDivergenceScanConfig {
            compare_players: [true, false],
            max_frames: Some(1),
            lookahead_frames: 0,
            max_scenarios: None,
            position_tolerance_milli: 100,
            velocity_tolerance_milli: 1,
        },
    )
    .expect("fixture should parse");

    assert_eq!(scan.scenarios.len(), 1);
    assert_eq!(
        scan.scenarios[0].kind,
        SlippiCoreDivergenceKind::StateMismatch
    );
    assert_eq!(
        scan.scenarios[0].first_frame.expected_action_state_id,
        MeleeActionStateId::new(79)
    );
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
    assert_eq!(drift.expected_motion_state, Some(MotionState::Wait));
    assert_eq!(drift.actual_motion_state, MotionState::Wait);
    assert_eq!(drift.expected_position, Vec2 { x: 2_000, y: 0 });
    assert_eq!(drift.actual_position, Vec2 { x: 0, y: 0 });

    let report = comparison.report_markdown();
    assert!(report.contains("First Significant Position Drift"));
    assert!(report.contains("Position delta: Rust - Melee (-2000, 0)"));
}

#[test]
fn slippi_core_comparison_uses_air_velocity_lane_for_airborne_horizontal_diff() {
    let export = r#"{
      "schema_version": 1,
      "source": {"replay_path": "fixture.slp", "parser": "@slippi/slippi-js/node"},
      "settings": {"stage_id": 31, "players": {"0": {"controller_fix": "UCF"}}},
      "metadata": {"start_at": "2026-05-31T00:00:00Z", "last_frame": 1},
      "export": {"first_frame": 0, "last_frame": 1, "frame_count": 2},
      "frames": [
        {"frame": 0, "players": {"0": {
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
            "action_state_counter": 5.0,
            "position": [0.0, 0.0],
            "self_induced_speeds": {"ground_x": 0.0, "air_x": 1.106, "y": 2.43}
          }
        }}},
        {"frame": 1, "players": {"0": {
          "pre": {
            "action_state_id": 25,
            "position": [0.0, 0.0],
            "facing": 1.0,
            "rust_player_input": {
              "stick_x": 0, "stick_y": 0, "c_stick_x": 0, "c_stick_y": 0,
              "left_trigger": 0, "right_trigger": 0, "physical_button_bits": 0
            }
          },
          "post": {
            "action_state_id": 25,
            "action_state_counter": 6.0,
            "position": [1.096, 2.32],
            "self_induced_speeds": {"ground_x": 0.0, "air_x": 1.096, "y": 2.32}
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
    .expect("fixture should parse");

    assert_eq!(comparison.first_state_mismatch, None);
    assert_eq!(comparison.first_position_drift, None);
    assert_eq!(
        comparison.max_abs_ground_velocity_diff[0], 2_430,
        "the composed velocity metric now includes vertical drift as well as the airborne horizontal air_x lane"
    );
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
    assert_eq!(
        trace.rows[0].expected_ground_velocity_x_source.to_bits(),
        (-2.14_f32).to_bits()
    );
    assert_eq!(
        trace.rows[0].expected_air_velocity_x_source.to_bits(),
        (-2.14_f32).to_bits()
    );
    assert_eq!(
        trace.rows[0].expected_velocity_y_source.to_bits(),
        0.0_f32.to_bits()
    );

    let report = trace.report_markdown();
    assert!(report.contains("# Slippi Core Trace Window"));
    assert!(report.contains("| 0 | -17 | 0 | -125 | 0 | 2048 |"));
    assert!(report.contains("KneeBend"));
}

#[test]
fn slippi_match_start_trace_applies_source_hit_collision_for_first_neutral_air_hit() {
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };

    let trace = trace_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreTraceConfig {
            player_index: 1,
            source_frame_start: 2252,
            source_frame_end: 2253,
            max_frames: Some(2_400),
        },
    )
    .expect("trace window should parse");

    assert_eq!(trace.rows.len(), 2);
    let pre_hit = &trace.rows[0];
    assert_eq!(pre_hit.source_frame, 2252);
    assert_eq!(pre_hit.expected_slippi_state_id, 20);
    assert_eq!(
        pre_hit.expected_action_state_id,
        MeleeActionStateId::new(20)
    );
    assert_eq!(
        pre_hit.actual_action_state_id,
        Some(MeleeActionStateId::new(20))
    );

    let hit = &trace.rows[1];
    assert_eq!(hit.source_frame, 2253);
    assert_eq!(hit.expected_slippi_state_id, 79);
    assert_eq!(hit.expected_action_state_id, MeleeActionStateId::new(79));
    assert_eq!(
        hit.actual_action_state_id,
        Some(MeleeActionStateId::new(79))
    );
}

fn test_capsule_gap_milli(a: &Capsule3, b: &Capsule3) -> f32 {
    fn dot(a: mole_core::collision::Vec3, b: mole_core::collision::Vec3) -> f32 {
        a.x * b.x + a.y * b.y + a.z * b.z
    }
    fn sub(
        a: mole_core::collision::Vec3,
        b: mole_core::collision::Vec3,
    ) -> mole_core::collision::Vec3 {
        mole_core::collision::Vec3::new(a.x - b.x, a.y - b.y, a.z - b.z)
    }
    fn add(
        a: mole_core::collision::Vec3,
        b: mole_core::collision::Vec3,
    ) -> mole_core::collision::Vec3 {
        mole_core::collision::Vec3::new(a.x + b.x, a.y + b.y, a.z + b.z)
    }
    fn mul(v: mole_core::collision::Vec3, scalar: f32) -> mole_core::collision::Vec3 {
        mole_core::collision::Vec3::new(v.x * scalar, v.y * scalar, v.z * scalar)
    }

    let d1 = sub(a.b, a.a);
    let d2 = sub(b.b, b.a);
    let r = sub(a.a, b.a);
    let aa = dot(d1, d1);
    let e = dot(d2, d2);
    let f = dot(d2, r);
    let (mut s, mut t);
    if aa <= f32::EPSILON && e <= f32::EPSILON {
        return dot(r, r).sqrt() - a.radius - b.radius;
    }
    if aa <= f32::EPSILON {
        s = 0.0;
        t = (f / e).clamp(0.0, 1.0);
    } else {
        let c = dot(d1, r);
        if e <= f32::EPSILON {
            t = 0.0;
            s = (-c / aa).clamp(0.0, 1.0);
        } else {
            let b_dot = dot(d1, d2);
            let denom = aa * e - b_dot * b_dot;
            s = if denom.abs() > f32::EPSILON {
                ((b_dot * f - c * e) / denom).clamp(0.0, 1.0)
            } else {
                0.0
            };
            t = (b_dot * s + f) / e;
            if t < 0.0 {
                t = 0.0;
                s = (-c / aa).clamp(0.0, 1.0);
            } else if t > 1.0 {
                t = 1.0;
                s = ((b_dot - c) / aa).clamp(0.0, 1.0);
            }
        }
    }

    let p = add(a.a, mul(d1, s));
    let q = add(b.a, mul(d2, t));
    dot(sub(p, q), sub(p, q)).sqrt() - a.radius - b.radius
}

fn test_source_capsule_summary(capsules: &[SourceCollisionCapsule]) -> String {
    capsules
        .iter()
        .take(4)
        .map(|capsule| {
            format!(
                "#{} f={:?} life={:?} a=({:.1},{:.1},{:.1}) b=({:.1},{:.1},{:.1}) r={:.1}",
                capsule.capsule_id,
                capsule.source_frame,
                capsule.hitbox_lifecycle_id.map(|id| id.get()),
                capsule.capsule.a.x,
                capsule.capsule.a.y,
                capsule.capsule.a.z,
                capsule.capsule.b.x,
                capsule.capsule.b.y,
                capsule.capsule.b.z,
                capsule.capsule.radius,
            )
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

#[test]
fn slippi_match_start_frame2299_ground_grab_enters_capture_pull() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(2_423))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    for frame in frames {
        step_world_with_source_collisions(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 2299 {
            break;
        }
    }

    let render_frame = RenderFrame::from_world(&world);
    let collision_frame = source_collision_frame_from_frame(&render_frame);
    let p1_hits = collision_frame
        .hits
        .iter()
        .filter(|capsule| capsule.owner_index == 0)
        .copied()
        .collect::<Vec<_>>();
    let p2_hurts = collision_frame
        .hurts
        .iter()
        .filter(|capsule| capsule.owner_index == 1)
        .copied()
        .collect::<Vec<_>>();
    let grab_confirms = source_grab_confirms(&collision_frame);
    let nearest_gap = p1_hits
        .iter()
        .flat_map(|hit| {
            p2_hurts.iter().map(move |hurt| {
                (
                    hit.capsule_id,
                    hurt.capsule_id,
                    test_capsule_gap_milli(&hit.capsule, &hurt.capsule),
                )
            })
        })
        .min_by(|a, b| a.2.total_cmp(&b.2));
    let p1_hit_summary = test_source_capsule_summary(&p1_hits);
    let p2_hurt_summary = test_source_capsule_summary(&p2_hurts);
    let players = world.players();

    assert_eq!(
        players[0].melee_action_state_id,
        Some(MeleeActionStateId::new(213)),
        "ftColl_80078A2C -> ftCo_Catch_Coll should route P1 Catch into CatchPull at Slippi source frame 2299. p1_action={:?} p2_action={:?} p1_pos={:?} p2_pos={:?} p1_hits={} p2_hurts={} nearest_gap={:?} grab_confirms={:?}",
        players[0].melee_action_state_id,
        players[1].melee_action_state_id,
        players[0].source_position,
        players[1].source_position,
        p1_hit_summary,
        p2_hurt_summary,
        nearest_gap,
        grab_confirms,
    );
    assert_eq!(
        players[1].melee_action_state_id,
        Some(MeleeActionStateId::new(226)),
        "grounded victim should enter CapturePulledLw from the same source grab confirm"
    );
}

#[test]
fn runtime_left_facing_catch_wait_holds_victim_on_faced_capturedamage_joint() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let mut world = World::for_two_players();

    let mut grabber = world.players()[0];
    grabber.motion_state_alias = None;
    grabber.motion_state = MotionState::Catch;
    grabber.melee_action_state_id = Some(MeleeActionStateId::new(213));
    grabber.source_action_key = Some(SourceActionKey::new("Catch"));
    grabber.source_action_total_frames = 30;
    grabber.source_victim_index = Some(1);
    grabber.source_x1a5c_index = Some(1);
    grabber.source_x221b_b5 = true;
    grabber.facing = -1;
    grabber.source_position = SourceVec2 { x: 10.0, y: 0.0 };
    grabber.position = grabber.source_position.to_milli();
    grabber.motion_frame = 6;
    grabber.set_source_motion_anim_frame(7.0);
    grabber.motion_throw_flags = 1 << 3;
    assert!(world.set_player_state_for_diagnostic(0, grabber));

    let mut victim = world.players()[1];
    victim.motion_state_alias = None;
    victim.motion_state = MotionState::GuardOn;
    victim.melee_action_state_id = Some(MeleeActionStateId::new(226));
    victim.source_action_key = Some(SourceActionKey::new("CapturePulledLw"));
    victim.source_action_total_frames = 20;
    victim.source_victim_index = Some(0);
    victim.source_x1a5c_index = Some(0);
    victim.grounded = true;
    victim.source_position = SourceVec2 { x: 20.0, y: 0.0 };
    victim.position = victim.source_position.to_milli();
    assert!(world.set_player_state_for_diagnostic(1, victim));

    step_world_with_source_collisions(
        &mut world,
        Frame(1),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    let players = world.players();
    assert_eq!(
        players[0].melee_action_state_id,
        Some(MeleeActionStateId::new(216)),
        "ftCo_CatchPull_Anim should run fn_800DA1D8 into CatchWait"
    );
    assert_eq!(
        players[1].melee_action_state_id,
        Some(MeleeActionStateId::new(227)),
        "fn_800DA1D8 should call fn_800DB6C8 into CaptureWaitLw"
    );
    assert!(
        players[1].source_position.x < players[0].source_position.x,
        "lb_8000B1CC(capturedamage.x18) should place a left-facing grab hold on the grabber's faced side; grabber_x={:.6} victim_x={:.6}",
        players[0].source_position.x,
        players[1].source_position.x
    );

    let frame = RenderFrame::from_world(&world);
    let scene = RenderScene::from_frame(&frame, 960, 540);
    let victim_hurtbox = scene.player_hurtbox_pills[1]
        .first()
        .expect("CaptureWaitLw victim should render source hurtboxes");
    assert!(
        victim_hurtbox.a.x < scene.player_contact_points[0].x
            || victim_hurtbox.b.x < scene.player_contact_points[0].x,
        "rendered source hurtboxes for the held victim should appear on the same faced side as the decomp capturedamage joint; grabber_screen_x={} hurt_a_x={} hurt_b_x={}",
        scene.player_contact_points[0].x,
        victim_hurtbox.a.x,
        victim_hurtbox.b.x
    );
}

#[test]
fn slippi_match_start_frame2301_capture_wait_lw_preserves_source_position() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(2_425))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    for frame in frames {
        step_world_with_source_collisions(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 2301 {
            break;
        }
    }

    let players = world.players();
    let victim = players[1];
    assert_eq!(
        victim.melee_action_state_id,
        Some(MeleeActionStateId::new(227)),
        "P2 should enter CaptureWaitLw on the same source frame as Slippi"
    );
    assert!(
        (victim.source_position.x - 45.725_582).abs() <= 0.000_01,
        "P2 CaptureWaitLw source X should match Slippi at source frame 2301; actual={:.6}",
        victim.source_position.x
    );
    assert_eq!(victim.position.x, 45_726);
}

#[test]
fn slippi_match_start_frame2302_thrown_hi_uses_decomp_accessory_position() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(2_426))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    for frame in frames {
        step_world_with_source_collisions(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 2302 {
            break;
        }
    }

    let victim = world.players()[1];
    assert_eq!(
        victim.melee_action_state_id,
        Some(MeleeActionStateId::new(241)),
        "P2 should enter TCaptainThrowHi on the same source frame as Slippi"
    );
    assert!(
        (victim.source_position.x - 46.353_378).abs() <= 0.000_01,
        "ftCo_800DE508 should place the thrown victim from constrained XRotN plus scaled x1A70.z; actual x={:.6}",
        victim.source_position.x
    );
    assert!(
        (victim.source_position.y - -1.975_364).abs() <= 0.000_01,
        "ftCo_800DE508 should place the thrown victim from constrained XRotN plus scaled x1A70.y; actual y={:.6}",
        victim.source_position.y
    );
}

#[test]
fn slippi_match_start_frame2313_thrown_hi_keeps_decomp_accessory_position() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(2_437))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    for frame in frames {
        step_world_with_source_collisions(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 2313 {
            break;
        }
    }

    let victim = world.players()[1];
    assert_eq!(
        victim.melee_action_state_id,
        Some(MeleeActionStateId::new(241)),
        "P2 should still be in source TCaptainThrowHi before ftCo_800DD724 releases the throw"
    );
    assert!(
        (victim.source_position.x - 46.977_928).abs() <= 0.000_01,
        "ftCo_800DE508 should keep using live source JObj XRotN plus x1A70 at source frame 2313; actual x={:.6}",
        victim.source_position.x
    );
    assert!(
        (victim.source_position.y - -2.755_544).abs() <= 0.000_01,
        "ftCo_800DE508 should keep using live source JObj XRotN plus x1A70 at source frame 2313; actual y={:.6}",
        victim.source_position.y
    );
}

#[test]
fn slippi_match_start_frame2318_throw_hi_waits_for_decomp_hitlag_gate_before_release() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(2_442))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    for frame in frames {
        step_world_with_source_collisions(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 2318 {
            break;
        }
    }

    let thrower = world.players()[0];
    let victim = world.players()[1];
    assert_eq!(
        thrower.melee_action_state_id,
        Some(MeleeActionStateId::new(221)),
        "P1 should still be in ThrowHi while the source hitlag gate prevents the Throw Anim release callback"
    );
    assert_eq!(
        victim.melee_action_state_id,
        Some(MeleeActionStateId::new(241)),
        "P2 should still be in TCaptainThrowHi until ftCo_800DD724 is allowed to consume throw_flags_b3"
    );
    assert_eq!(
        victim.velocity,
        Vec2 { x: 0, y: 0 },
        "held victim should not receive throw release velocity before the decomp callback gate opens"
    );
    assert!(
        (victim.source_position.x - 46.977_390).abs() <= 0.000_01,
        "held victim should remain on the decomp accessory-position path at source frame 2318; actual x={:.6}",
        victim.source_position.x
    );
    assert!(
        (victim.source_position.y - -1.966_638).abs() <= 0.000_01,
        "held victim should remain on the decomp accessory-position path at source frame 2318; actual y={:.6}",
        victim.source_position.y
    );
}

#[test]
fn runtime_frame_zero_throw_hitbox_event_populates_xdf4_on_first_action_tick() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let mut world = World::for_two_players();
    let mut thrower = world.players()[0];
    thrower.motion_state_alias = None;
    thrower.motion_state = MotionState::GuardOn;
    thrower.melee_action_state_id = Some(MeleeActionStateId::new(221));
    thrower.source_action_key = Some(SourceActionKey::new("ThrowHi"));
    thrower.source_action_total_frames = 44;
    thrower.motion_frame = 0;
    thrower.set_source_motion_anim_frame(1.0);
    assert!(world.set_player_state_for_diagnostic(0, thrower));

    step_world_with_source_collisions(&mut world, Frame(1), &[PlayerInput::neutral(); 2]);

    let throw_hitbox = world.players()[0].source_throw_hitboxes[0]
        .expect("ThrowHi frame-0 ftAction_80071E04 should populate fp->xDF4[0]");
    assert_eq!(throw_hitbox.hitbox.hitbox_idx, 0);
    assert_eq!(throw_hitbox.hitbox.damage, 3);
    assert_eq!(throw_hitbox.damage.to_bits(), 3.0_f32.to_bits());
    assert_eq!(throw_hitbox.unk_count, 3);
    assert_eq!(throw_hitbox.hitbox.angle, 85);
    assert_eq!(throw_hitbox.hitbox.hit_x24, 105);
    assert_eq!(throw_hitbox.hitbox.hit_x28, 0);
    assert_eq!(throw_hitbox.hitbox.hit_x2c, 70);
    assert_eq!(throw_hitbox.hitbox.element, 0);
}

#[test]
fn slippi_match_start_frame2319_throw_hi_release_runs_decomp_damage_entry_update() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(2_443))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    for frame in frames {
        step_world_with_source_collisions(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 2319 {
            break;
        }
    }

    let thrower = world.players()[0];
    let throw_hitbox = thrower.source_throw_hitboxes[0].expect(
        "ThrowHi should carry persistent fp->xDF4[0] before ftCo_800DD724 consumes throw_flags_b3",
    );
    assert_eq!(
        thrower.motion_throw_flags & (1 << 3),
        0,
        "ftCo_800DD724 should consume and clear throw_flags_b3 on the release frame"
    );
    assert_eq!(
        thrower.source_victim_index, None,
        "ftCo_800DD724 release should clear the thrower's victim link through ftCo_800DDDE4; motion_frame={} source_anim={:.3} flags={:#04x}",
        thrower.motion_frame,
        thrower.source_motion_anim_frame,
        thrower.motion_throw_flags,
    );

    let victim = world.players()[1];
    assert_eq!(
        victim.melee_action_state_id,
        Some(MeleeActionStateId::new(90)),
        "P2 should enter DamageFlyTop when ftCo_800DD724 consumes throw_flags_b3"
    );
    assert_eq!(
        victim.facing, -1,
        "ftCo_800DE7C0 should preserve the decomp damage facing direction on release"
    );
    assert!(
        !victim.grounded,
        "up-throw release should put the victim airborne"
    );
    assert![
        (victim.source_position.x - 47.384_254).abs() <= 0.000_01,
        "released victim should receive the same-frame source damage position update at frame 2319; actual x={:.6}; xDF4 damage={:.6} raw_damage={} unk_count={} percent={:.6} temp={:.6} applied={}",
        victim.source_position.x,
        throw_hitbox.damage,
        throw_hitbox.hitbox.damage,
        throw_hitbox.unk_count,
        victim.damage_percent,
        victim.damage_percent_temp,
        victim.damage_applied
    ];
    assert![
        (victim.source_position.y - -0.045_247).abs() <= 0.000_01,
        "released victim should receive the same-frame source damage position update at frame 2319; actual y={:.6}",
        victim.source_position.y
    ];
    assert![
        (victim.source_knockback_velocity_x - 0.407_118).abs() <= 0.000_01,
        "released victim should expose Slippi/decomp attack X velocity after damage update; actual={:.6}; xDF4 damage={:.6} raw_damage={} unk_count={} percent={:.6} temp={:.6} applied={}",
        victim.source_knockback_velocity_x,
        throw_hitbox.damage,
        throw_hitbox.hitbox.damage,
        throw_hitbox.unk_count,
        victim.damage_percent,
        victim.damage_percent_temp,
        victim.damage_applied
    ];
    assert![
        (victim.source_knockback_velocity_y - 2.784_849).abs() <= 0.000_01,
        "released victim should expose Slippi/decomp attack Y velocity after damage update; actual={:.6}; xDF4 damage={:.6} raw_damage={} unk_count={} percent={:.6} temp={:.6} applied={}",
        victim.source_knockback_velocity_y,
        throw_hitbox.damage,
        throw_hitbox.hitbox.damage,
        throw_hitbox.unk_count,
        victim.damage_percent,
        victim.damage_percent_temp,
        victim.damage_applied
    ];
    assert![
        (victim.source_self_velocity_y - -0.130_000).abs() <= 0.000_01,
        "ftCo_Damage_Phys should apply Falcon gravity to self velocity on the same exported release frame; actual={:.6}",
        victim.source_self_velocity_y
    ];
}

#[test]
fn slippi_match_start_frame2454_falcon_fair_enters_damage_n3() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(2_578))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let mut raw_p2_fair_confirm_frames = Vec::new();
    let mut source_step_frames = Vec::new();
    for frame in frames {
        let step =
            step_world_with_source_collision_step(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame >= 2450 {
            source_step_frames.push((
                frame.source_frame,
                step.geometry_confirms
                    .iter()
                    .map(|confirm| {
                        (
                            confirm.attacker_index,
                            confirm.victim_index,
                            confirm.hitbox_id,
                            confirm.hurtbox_id,
                            confirm.source_action_key,
                            confirm.source_frame,
                        )
                    })
                    .collect::<Vec<_>>(),
                step.raw_confirms
                    .iter()
                    .map(|confirm| {
                        (
                            confirm.attacker_index,
                            confirm.victim_index,
                            confirm.hitbox_id,
                            confirm.hurtbox_id,
                            confirm.source_action_key,
                            confirm.source_frame,
                        )
                    })
                    .collect::<Vec<_>>(),
                step.logged_confirms
                    .iter()
                    .map(|confirm| {
                        (
                            confirm.attacker_index,
                            confirm.victim_index,
                            confirm.hitbox_id,
                            confirm.hurtbox_id,
                            confirm.source_action_key,
                            confirm.source_frame,
                        )
                    })
                    .collect::<Vec<_>>(),
                step.confirms
                    .iter()
                    .map(|confirm| {
                        (
                            confirm.attacker_index,
                            confirm.victim_index,
                            confirm.hitbox_id,
                            confirm.hurtbox_id,
                            confirm.source_action_key,
                            confirm.source_frame,
                        )
                    })
                    .collect::<Vec<_>>(),
                step.stages
                    .iter()
                    .map(|stage| {
                        (
                            stage.attacker_index,
                            stage.victim_index,
                            stage.hitbox_id,
                            stage.hurtbox_id,
                            stage.source_action_key,
                            stage.source_frame,
                            stage.damage,
                        )
                    })
                    .collect::<Vec<_>>(),
                step.results
                    .iter()
                    .map(|result| {
                        (
                            result.stage.attacker_index,
                            result.stage.victim_index,
                            result.stage.hitbox_id,
                            result.stage.hurtbox_id,
                            result.stage.source_action_key,
                            result.stage.source_frame,
                            result.knockback,
                            result.angle,
                        )
                    })
                    .collect::<Vec<_>>(),
                step.applied_stage_count,
            ));
        }
        let raw_confirms = source_hit_confirms_from_frame(&RenderFrame::from_world(&world));
        if raw_confirms.iter().any(|confirm| {
            confirm.attacker_index == 1
                && confirm.victim_index == 0
                && confirm.source_action_key == Some(SourceActionKey::new("AttackAirF"))
        }) {
            let players = world.players();
            raw_p2_fair_confirm_frames.push((
                frame.source_frame,
                players[0].melee_action_state_id,
                players[0].source_hurt_collision_state,
                players[0].source_hurt_collision_lockout_timer,
                players[0].source_collision_state,
            ));
        }
        if frame.source_frame == 2454 {
            break;
        }
    }

    let render_frame = RenderFrame::from_world(&world);
    let collision_frame = source_collision_frame_from_frame(&render_frame);
    let confirms = source_hit_confirms_from_frame(&render_frame);
    let p2_hits = collision_frame
        .hits
        .iter()
        .filter(|capsule| capsule.owner_index == 1)
        .copied()
        .collect::<Vec<_>>();
    let p1_hurts = collision_frame
        .hurts
        .iter()
        .filter(|capsule| capsule.owner_index == 0)
        .copied()
        .collect::<Vec<_>>();
    let p2_hit_summary = test_source_capsule_summary(&p2_hits);
    let p1_hurt_summary = test_source_capsule_summary(&p1_hurts);
    let nearest_gap = p2_hits
        .iter()
        .flat_map(|hit| {
            p1_hurts.iter().map(move |hurt| {
                (
                    hit.capsule_id,
                    hurt.capsule_id,
                    test_capsule_gap_milli(&hit.capsule, &hurt.capsule),
                )
            })
        })
        .min_by(|a, b| a.2.total_cmp(&b.2));
    let players = world.players();

    assert_eq!(
        players[0].melee_action_state_id,
        Some(MeleeActionStateId::new(80)),
        "Slippi source frame 2454 has P1 enter ftCo_MS_DamageN3 from P2 Falcon AttackAirF late hit. p1_action={:?} p2_action={:?} p1_pos={:?} p2_pos={:?} p1_hurt_state={} p1_hurt_lockout={} p1_collision_state={} p2_hits={} p1_hurts={} nearest_gap={:?} confirms={:?} raw_p2_fair_confirm_frames={:?} source_step_frames={:?}",
        players[0].melee_action_state_id,
        players[1].melee_action_state_id,
        players[0].source_position,
        players[1].source_position,
        players[0].source_hurt_collision_state,
        players[0].source_hurt_collision_lockout_timer,
        players[0].source_collision_state,
        p2_hit_summary,
        p1_hurt_summary,
        nearest_gap,
        confirms,
        raw_p2_fair_confirm_frames,
        source_step_frames,
    );
}

#[test]
fn slippi_match_start_frame1759_p1_jumpf_lower_ecb_edge_hits_battlefield_left_wall() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let trace = trace_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreTraceConfig {
            player_index: 0,
            source_frame_start: 1759,
            source_frame_end: 1759,
            max_frames: Some(1_800),
        },
    )
    .expect("replay fixture should trace");
    let row = trace.rows.first().expect("source frame 1759 should trace");

    assert_eq!(
        row.actual_action_state_id,
        Some(MeleeActionStateId::new(25))
    );
    assert_eq!(row.actual_velocity_x, -399);
    assert_eq!(row.actual_velocity_y, -2_260);
    assert_eq!(
        source_units_to_milli(row.actual_source_position.x),
        -71_853,
        "mpColl_80045B74_LeftWall's airborne bottom-to-right ECB edge has no floor-connected-wall exclusion, so Battlefield line 17 must correct P1 at source frame 1759. row={row:?}"
    );
    assert_eq!(source_units_to_milli(row.actual_source_position.y), -5_940);
}

#[test]
fn slippi_match_start_frame1771_p2_fall_lower_ecb_edge_hits_battlefield_right_ledge_wall() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let trace = trace_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreTraceConfig {
            player_index: 1,
            source_frame_start: 1771,
            source_frame_end: 1771,
            max_frames: Some(1_800),
        },
    )
    .expect("replay fixture should trace");
    let row = trace.rows.first().expect("source frame 1771 should trace");

    assert_eq!(
        row.actual_action_state_id,
        Some(MeleeActionStateId::new(29))
    );
    assert_eq!(row.actual_velocity_x, 601);
    assert_eq!(row.actual_velocity_y, -3_500);
    assert_eq!(
        source_units_to_milli(row.actual_source_position.x),
        70_526,
        "mpColl_80044E10_RightWall's airborne lower ECB checks must preserve the valid Battlefield right-ledge correction while P2 falls toward CliffCatch. row={row:?}"
    );
    assert_eq!(source_units_to_milli(row.actual_source_position.y), -3_630);
}

#[test]
fn slippi_match_start_frame2833_p2_cliff_catch_rejects_p1_dair_hit() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let trace = trace_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreTraceConfig {
            player_index: 1,
            source_frame_start: 2833,
            source_frame_end: 2833,
            max_frames: Some(2_840),
        },
    )
    .expect("replay fixture should trace");
    let row = trace.rows.first().expect("source frame 2833 should trace");

    assert_eq!(
        row.actual_action_state_id,
        Some(MeleeActionStateId::new(252)),
        "ftCliffCommon_80081370 must preserve CliffCatch against P1's active dair at source frame 2833. row={row:?}"
    );
    assert_eq!(row.actual_source_knockback_velocity_x, 0.0);
    assert_eq!(row.actual_source_knockback_velocity_y, 0.0);
    assert_eq!(row.actual_velocity_x, 0);
    assert_eq!(row.actual_velocity_y, 0);
}

#[test]
fn slippi_match_start_frame3025_p2_attackairhi_right_wall_scrape_matches_source_position() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let trace = trace_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreTraceConfig {
            player_index: 1,
            source_frame_start: 3025,
            source_frame_end: 3025,
            max_frames: Some(3_030),
        },
    )
    .expect("replay fixture should trace");
    let row = trace.rows.first().expect("source frame 3025 should trace");

    assert_eq!(
        row.actual_action_state_id,
        Some(MeleeActionStateId::new(68))
    );
    assert_eq!(row.actual_velocity_x, -533);
    assert_eq!(row.actual_velocity_y, 1880);
    assert_eq!(
        source_units_to_milli(row.actual_source_position.x),
        72_291,
        "Slippi source frame 3025 keeps P2 in AttackAirHi with matching velocity, so the right-wall scrape must correct the root position before the later source-frame 3126 Bair geometry. row={row:?}"
    );
    assert_eq!(source_units_to_milli(row.actual_source_position.y), -7_336);
}

#[test]
fn slippi_match_start_frame3026_p2_attackairhi_entry_pose_does_not_false_right_wall_correct() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let trace = trace_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreTraceConfig {
            player_index: 1,
            source_frame_start: 3026,
            source_frame_end: 3026,
            max_frames: Some(3_030),
        },
    )
    .expect("replay fixture should trace");
    let row = trace.rows.first().expect("source frame 3026 should trace");

    assert_eq!(
        row.actual_action_state_id,
        Some(MeleeActionStateId::new(68))
    );
    assert_eq!(row.actual_velocity_x, -579);
    assert_eq!(row.actual_velocity_y, 1750);
    assert_eq!(
        source_units_to_milli(row.actual_source_position.x),
        71_713,
        "source frame 3026 must not let Battlefield line 16 push P2 away from the underside; this regression exercises the live AttackAirHi ECB pose rather than the ground-only mpColl_80048AB0 exclusion path. row={row:?}"
    );
    assert_eq!(source_units_to_milli(row.actual_source_position.y), -5_586);
}

#[test]
fn slippi_match_start_frame3126_p1_bair_does_not_damage_p2_escape_air() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(3_300))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let mut frame3126_step = None;
    for frame in frames {
        let step =
            step_world_with_source_collision_step(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 3126 {
            frame3126_step = Some(step);
            break;
        }
    }

    let step = frame3126_step.expect("source frame 3126 should be present in the replay fixture");
    let p1_bair_p2_confirms = step
        .confirms
        .iter()
        .filter(|confirm| {
            confirm.attacker_index == 0
                && confirm.victim_index == 1
                && confirm.source_action_key == Some(SourceActionKey::new("AttackAirB"))
        })
        .map(|confirm| {
            (
                confirm.hitbox_id,
                confirm.hurtbox_id,
                confirm.source_frame,
                confirm.damaged_hurt_height,
                confirm.hitbox.damage,
                confirm.hitbox.angle,
            )
        })
        .collect::<Vec<_>>();
    let p1_bair_p2_stages = step
        .stages
        .iter()
        .filter(|stage| {
            stage.attacker_index == 0
                && stage.victim_index == 1
                && stage.source_action_key == Some(SourceActionKey::new("AttackAirB"))
        })
        .map(|stage| {
            (
                stage.hitbox_id,
                stage.hurtbox_id,
                stage.source_frame,
                stage.damage,
                stage.env_damage,
                stage.hitbox.damage,
                stage.hitbox.angle,
            )
        })
        .collect::<Vec<_>>();
    let p1_bair_p2_results = step
        .results
        .iter()
        .filter(|result| {
            result.stage.attacker_index == 0
                && result.stage.victim_index == 1
                && result.stage.source_action_key == Some(SourceActionKey::new("AttackAirB"))
        })
        .map(|result| {
            (
                result.stage.hitbox_id,
                result.stage.hurtbox_id,
                result.stage.source_frame,
                result.knockback,
                result.angle,
                result.element,
            )
        })
        .collect::<Vec<_>>();

    assert!(
        p1_bair_p2_confirms.is_empty()
            && p1_bair_p2_stages.is_empty()
            && p1_bair_p2_results.is_empty(),
        "Slippi source frame 3126 keeps P2 in EscapeAir with no attack velocity, so the Rust source collision pass must not apply a P1 AttackAirB damage confirm here. confirms={p1_bair_p2_confirms:?} stages={p1_bair_p2_stages:?} results={p1_bair_p2_results:?} all_confirms={:?} all_stages={:?} all_results={:?} p1={:?} p2={:?}",
        step.confirms,
        step.stages,
        step.results,
        world.players()[0],
        world.players()[1],
    );
    assert_eq!(
        world.players()[1].melee_action_state_id,
        Some(MeleeActionStateId::new(236)),
        "with no decomp-valid damage confirm, P2 should remain in ftCo_MS_EscapeAir for the exported 3126 row"
    );
}

#[test]
fn slippi_match_start_frame3183_p1_nair_sets_off_p2_guard_not_body_damage() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(3_310))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let mut frame3183_step = None;
    let mut p2_after_3182 = None;
    for frame in frames {
        let step =
            step_world_with_source_collision_step(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 3182 {
            p2_after_3182 = Some(world.players()[1]);
        }
        if frame.source_frame == 3183 {
            frame3183_step = Some(step);
            break;
        }
    }

    let step = frame3183_step.expect("source frame 3183 should be present in the replay fixture");
    let p1_nair_p2_shield_confirms = step
        .logged_confirms
        .iter()
        .filter(|confirm| {
            confirm.attacker_index == 0
                && confirm.victim_index == 1
                && confirm.source_action_key == Some(SourceActionKey::new("AttackAirN"))
                && confirm.hurtbox_id == SOURCE_SHIELD_HURTBOX_ID
        })
        .collect::<Vec<_>>();
    let p1_nair_p2_body_results = step
        .results
        .iter()
        .filter(|result| {
            result.stage.attacker_index == 0
                && result.stage.victim_index == 1
                && result.stage.source_action_key == Some(SourceActionKey::new("AttackAirN"))
        })
        .collect::<Vec<_>>();

    assert!(
        !p1_nair_p2_shield_confirms.is_empty() && p1_nair_p2_body_results.is_empty(),
        "Slippi source frame 3183 has P1 AttackAirN hit P2's held shield: ftColl_80076CBC should drive ftCo_80092E50/GuardDamage instead of ordinary body damage. shield_confirms={p1_nair_p2_shield_confirms:?} body_results={p1_nair_p2_body_results:?} p2_after_3182={p2_after_3182:?} logged={:?} results={:?} p1={:?} p2={:?}",
        step.logged_confirms,
        step.results,
        world.players()[0],
        world.players()[1],
    );
    assert_eq!(
        world.players()[1].melee_action_state_id,
        Some(MeleeActionStateId::new(181)),
        "P2 should enter ftCo_MS_GuardDamage/GuardSetOff on the shield-hit frame"
    );
    assert_eq!(world.players()[1].motion_state, MotionState::GuardSetOff);
    assert_eq!(
        source_units_to_milli(world.players()[1].ground_velocity_x),
        -510
    );
    assert_eq!(world.players()[1].velocity.y, 0);
}

#[test]
fn slippi_match_start_frame3190_guard_setoff_survives_hitlag_before_jump() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(3_320))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let mut p2_at_3190 = None;
    let mut p2_at_3191 = None;
    for frame in frames {
        step_world_with_source_collision_step(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 3190 {
            p2_at_3190 = Some(world.players()[1]);
        }
        if frame.source_frame == 3191 {
            p2_at_3191 = Some(world.players()[1]);
            break;
        }
    }

    let p2_at_3190 = p2_at_3190.expect("source frame 3190 should be present");
    let p2_at_3191 = p2_at_3191.expect("source frame 3191 should be present");

    assert_eq!(
        p2_at_3190.melee_action_state_id,
        Some(MeleeActionStateId::new(181)),
        "ftCo_80092F2C drives GuardSetOff by animation duration; hitlag must not consume the GuardDamage lifetime early"
    );
    assert_eq!(p2_at_3190.motion_state, MotionState::GuardSetOff);
    assert_eq!(
        p2_at_3191.melee_action_state_id,
        Some(MeleeActionStateId::new(24)),
        "once GuardSetOff's decomp lifetime expires, the held jump should begin KneeBend on source frame 3191"
    );
}

#[test]
fn slippi_match_start_frame2454_electric_fair_uses_decomp_hitlag_multiplier() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(2_591))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let mut hit_frame_hitlag = None;
    let mut frozen_2466 = None;
    let mut moving_2467 = None;
    for frame in frames {
        step_world_with_source_collisions(&mut world, frame.core_frame, &frame.inputs);
        let player = world.players()[0];
        if frame.source_frame == 2454 {
            hit_frame_hitlag = Some(player.hitlag_frames);
            assert_eq!(
                player.melee_action_state_id,
                Some(MeleeActionStateId::new(80))
            );
        }
        if frame.source_frame == 2466 {
            frozen_2466 = Some((
                player.position,
                player.source_knockback_velocity_x,
                player.source_knockback_velocity_y,
                player.hitlag_frames,
            ));
        }
        if frame.source_frame == 2467 {
            moving_2467 = Some((
                player.position,
                player.source_knockback_velocity_x,
                player.source_knockback_velocity_y,
                player.hitlag_frames,
            ));
            break;
        }
    }

    let hit_frame_hitlag = hit_frame_hitlag.expect("source 2454 should be simulated");
    assert_eq!(
        hit_frame_hitlag, 13,
        "ftColl_8007A06C sets x1960_vibrateMult from p_ftCommonData->x1A4 for element-2 hits, and Fighter_ProcessHit_8006D1EC passes that multiplier to ftCommon_CalcHitlag"
    );
    let frozen_2466 = frozen_2466.expect("source 2466 should be simulated");
    assert_eq!(frozen_2466.0, Vec2 { x: 2325, y: 0 });
    assert_eq!(frozen_2466.3, 1);
    assert!(
        (frozen_2466.1 - -1.697_094).abs() <= 0.000_01
            && (frozen_2466.2 - 1.060_462).abs() <= 0.000_01,
        "knockback velocity should be stored but not consumed while x195c hitlag remains"
    );
    let moving_2467 = moving_2467.expect("source 2467 should be simulated");
    assert_eq!(moving_2467.3, 0);
    assert_eq!(
        moving_2467.0,
        Vec2 { x: -1874, y: -1465 },
        "first post-hitlag DamageN3 physics step should line up with Slippi source 2467"
    );
}

#[test]
fn slippi_match_start_frame2486_walk_analog_trigger_enters_guard_on_from_held_lr() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(2_610))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let mut frame2486 = None;
    for frame in frames {
        step_world_with_source_collisions(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 2486 {
            frame2486 = Some(world.players()[0]);
            break;
        }
    }

    let player = frame2486.expect("source 2486 should be simulated");
    assert_eq!(
        player.motion_state,
        MotionState::GuardOn,
        "ftCo_Walk_IASA reaches ftCo_80091A4C, whose GuardOn path only requires held_inputs & HSD_PAD_LR plus shield health"
    );
    assert_eq!(player.position, Vec2 { x: -24_379, y: 0 });
    assert_eq!(
        player.velocity.x, 440,
        "GuardOn_Phys/ft_80084F3C should apply source ground traction on the entry tick"
    );
}

#[test]
fn slippi_match_start_frame2583_raptor_boost_routes_shield_detect_callback() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(2_760))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let mut step2583 = None;
    for frame in frames {
        let step =
            step_world_with_source_collision_step(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 2583 {
            step2583 = Some(step);
            break;
        }
    }

    let step = step2583.expect("source 2583 should be simulated");
    let render_frame = RenderFrame::from_world(&world);
    let collision_frame = source_collision_frame_from_frame(&render_frame);
    let p1_hits = collision_frame
        .hits
        .iter()
        .filter(|capsule| capsule.owner_index == 0)
        .copied()
        .collect::<Vec<_>>();
    let p2_hurts = collision_frame
        .hurts
        .iter()
        .filter(|capsule| capsule.owner_index == 1)
        .copied()
        .collect::<Vec<_>>();
    let players = world.players();
    assert!(
        step.logged_confirms.iter().any(|confirm| {
            confirm.attacker_index == 0
                && confirm.victim_index == 1
                && confirm.action_state_id == Some(MeleeActionStateId::new(349))
                && confirm.source_action_key == Some(SourceActionKey::new("SpecialSStart"))
        }),
        "source 2583 should expose a detected fighter before ftCa_SpecialS_OnDetect; decomp ftCa_SpecialS_OnDetect routes on detected fighter/item object, not shield-specific contact; geometry={:?} raw={:?} logged={:?} p1_hits={:?} p2_hurts={:?} p1={:?} p2={:?}",
        step.geometry_confirms,
        step.raw_confirms,
        step.logged_confirms,
        p1_hits,
        p2_hurts,
        players[0],
        players[1]
    );
    assert_eq!(
        players[0].melee_action_state_id,
        Some(MeleeActionStateId::new(350)),
        "ftCa_SpecialS_OnDetect should route SpecialSStart to SpecialS on the same source row"
    );
    assert_eq!(
        players[1].melee_action_state_id,
        Some(MeleeActionStateId::new(180)),
        "the source 2583 shield contact should feed Raptor Boost detect without also routing the guarded defender through body damage; geometry={:?} raw={:?} logged={:?} confirms={:?} stages={:?} results={:?} p1_hits={:?} p2_hurts={:?} p1={:?} p2={:?}",
        step.geometry_confirms,
        step.raw_confirms,
        step.logged_confirms,
        step.confirms,
        step.stages,
        step.results,
        p1_hits,
        p2_hurts,
        players[0],
        players[1]
    );
    assert!(
        step.stages.is_empty() && step.applied_stage_count == 0,
        "ftCa_SpecialS_OnDetect is a hurtbox-detect callback path, not ftColl_8007BE3C damage finalization; stages={:?}",
        step.stages
    );
    assert!(
        (players[0].ground_velocity_x - 0.6076804).abs() <= 0.00001,
        "onDetectGround should multiply gr_vel by specials_gr_vel_x"
    );
    assert_eq!(
        players[0].velocity.x, 608,
        "grounded ftCa_SpecialS_OnDetect leaves self_vel.x as Slippi's air velocity field, but the public/composed velocity must follow scaled gr_vel"
    );
}

#[test]
fn slippi_match_start_frame2586_raptor_boost_followthrough_clears_guard_shield_object() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(2_760))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let mut previous_steps = Vec::new();
    let mut step2586 = None;
    for frame in frames {
        let step =
            step_world_with_source_collision_step(&mut world, frame.core_frame, &frame.inputs);
        if (2580..=2585).contains(&frame.source_frame) {
            previous_steps.push((frame.source_frame, step.clone()));
        }
        if frame.source_frame == 2586 {
            step2586 = Some(step);
            break;
        }
    }

    let step = step2586.expect("source 2586 should be simulated");
    let render_frame = RenderFrame::from_world(&world);
    let collision_frame = source_collision_frame_from_frame(&render_frame);
    let p1_hits = collision_frame
        .hits
        .iter()
        .filter(|capsule| capsule.owner_index == 0)
        .copied()
        .collect::<Vec<_>>();
    let p2_hurts = collision_frame
        .hurts
        .iter()
        .filter(|capsule| capsule.owner_index == 1)
        .copied()
        .collect::<Vec<_>>();
    let mut next_specials_frame = render_frame;
    next_specials_frame.player_source_motion_anim_frames[0] = 4.0;
    next_specials_frame.player_source_pose_frames[0] = 4;
    let next_specials_confirms = source_hit_confirms_from_frame(&next_specials_frame);
    let mut previous_victim_root_frame = render_frame;
    previous_victim_root_frame.player_source_positions[1] =
        render_frame.player_source_previous_positions[1];
    let previous_victim_root_confirms = source_hit_confirms_from_frame(&previous_victim_root_frame);
    let players = world.players();
    assert_eq!(
        players[1].melee_action_state_id,
        Some(MeleeActionStateId::new(90)),
        "source 2586 should route the defender through ftCo_Damage into DamageFlyTop before any stale GuardOff shield object can consume another hit; geometry={:?} raw={:?} logged={:?} confirms={:?} stages={:?} results={:?} next_specials_confirms={:?} previous_victim_root_confirms={:?} p1_hits={:?} p2_hurts={:?} p1={:?} p2={:?}",
        step.geometry_confirms,
        step.raw_confirms,
        step.logged_confirms,
        step.confirms,
        step.stages,
        step.results,
        next_specials_confirms,
        previous_victim_root_confirms,
        p1_hits,
        p2_hurts,
        players[0],
        players[1]
    );
    assert_eq!(step.stages.len(), 1);
    assert_eq!(
        step.stages[0].damage.to_bits(),
        7.0_f32.to_bits(),
        "Raptor Boost body hit should enter ftColl_8007ABD0 with full unstaled damage before knockback; stages={:?} previous_steps={:?}",
        step.stages,
        previous_steps
    );
    assert!(
        !players[1].source_shield_collision_active
            && !players[1].source_shield_hit_active
            && !players[1].source_shield_hit_update_pos,
        "Fighter_ChangeMotionState clears x221A_b7/x221B_b0; p2={:?}",
        players[1]
    );
    assert!(
        !p2_hurts
            .iter()
            .any(|hurt| hurt.capsule_id == SOURCE_SHIELD_HURTBOX_ID),
        "GuardOff/damage should not expose the stale source shield hurt capsule; geometry={:?} raw={:?} logged={:?} confirms={:?} stages={:?} results={:?} p1_hits={:?} p2_hurts={:?} p1={:?} p2={:?}",
        step.geometry_confirms,
        step.raw_confirms,
        step.logged_confirms,
        step.confirms,
        step.stages,
        step.results,
        p1_hits,
        p2_hurts,
        players[0],
        players[1]
    );
}

#[test]
fn slippi_match_start_frame2586_p2_damage_velocity_uses_composed_slippi_lane() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };

    let trace = trace_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreTraceConfig {
            player_index: 1,
            source_frame_start: 2586,
            source_frame_end: 2610,
            max_frames: Some(2_760),
        },
    )
    .expect("match-start replay fixture should trace P2 2586-2610");
    assert_eq!(trace.rows.len(), 25);

    let frame2586 = trace
        .rows
        .iter()
        .find(|row| row.source_frame == 2586)
        .expect("source frame 2586 should be traced");
    assert_eq!(
        frame2586.actual_action_state_id,
        Some(MeleeActionStateId::new(90))
    );
    assert_eq!(frame2586.actual_position, frame2586.expected_position);
    assert_eq!(
        frame2586.actual_velocity_y, frame2586.expected_composed_velocity_y,
        "DamageFlyTop comparison should include knockback on the first damage row"
    );
    assert_eq!(
        frame2586.expected_velocity_y, 0,
        "Slippi's raw self-y lane stays zero while composed damage velocity carries knockback"
    );
    assert_eq!(frame2586.actual_velocity_y, 3_232);

    for row in &trace.rows {
        assert_eq!(
            row.actual_position, row.expected_position,
            "source frame {} should not introduce position drift",
            row.source_frame
        );
        assert_eq!(
            row.actual_velocity_x, row.expected_composed_velocity_x,
            "source frame {} should keep composed X velocity aligned",
            row.source_frame
        );
        assert_eq!(
            row.actual_velocity_y, row.expected_composed_velocity_y,
            "source frame {} should keep composed Y velocity aligned",
            row.source_frame
        );
    }
}

#[test]
fn slippi_match_start_frame3268_p1_damageair2_uses_source_di_velocity() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(3_400))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let mut frame3268_step = None;
    for frame in frames {
        let step =
            step_world_with_source_collision_step(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 3268 {
            frame3268_step = Some(step);
            break;
        }
    }

    let step = frame3268_step.expect("source frame 3268 should be present in the replay fixture");
    let p2_nair_p1_results = step
        .results
        .iter()
        .filter(|result| {
            result.stage.attacker_index == 1
                && result.stage.victim_index == 0
                && result.stage.source_action_key == Some(SourceActionKey::new("AttackAirN"))
        })
        .map(|result| (result.stage.hitbox_id, result.angle))
        .collect::<Vec<_>>();
    assert_eq!(
        p2_nair_p1_results,
        vec![(1, 78)],
        "decomp same-group victim logging should select the 78-degree NAir capsule before damage velocity is composed; geometry={:?} raw={:?} logged={:?} confirms={:?} stages={:?} results={:?}",
        step.geometry_confirms,
        step.raw_confirms,
        step.logged_confirms,
        step.confirms,
        step.stages,
        step.results
    );

    let trace = trace_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreTraceConfig {
            player_index: 0,
            source_frame_start: 3268,
            source_frame_end: 3272,
            max_frames: Some(3_400),
        },
    )
    .expect("match-start replay fixture should trace P1 3268-3272");

    let frame3268 = trace
        .rows
        .iter()
        .find(|row| row.source_frame == 3268)
        .expect("source frame 3268 should be traced");
    assert_eq!(
        frame3268.actual_action_state_id,
        Some(MeleeActionStateId::new(85))
    );
    assert_eq!(frame3268.actual_position, frame3268.expected_position);
    assert_eq!(
        frame3268.actual_velocity_x, frame3268.expected_composed_velocity_x,
        "DamageAir2 should expose Slippi's DI-adjusted attack velocity on the hit frame"
    );
    assert_eq!(
        frame3268.actual_velocity_y, frame3268.expected_composed_velocity_y,
        "DamageAir2 should expose Slippi's DI-adjusted attack velocity on the hit frame"
    );

    let frame3272 = trace
        .rows
        .iter()
        .find(|row| row.source_frame == 3272)
        .expect("source frame 3272 should be traced");
    assert_eq!(
        frame3272.actual_position, frame3272.expected_position,
        "hitlag should release on the same source frame as Slippi"
    );
    assert_eq!(
        frame3272.actual_velocity_y, frame3272.expected_composed_velocity_y,
        "damage gravity should resume on the same source frame as Slippi"
    );
}

#[test]
fn slippi_match_start_frame3426_p2_raptor_detect_precedes_nair_damage() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(3_560))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let mut step3426 = None;
    let mut p2_frame3426 = None;
    let mut p2_frame3428 = None;
    let mut p2_frame3433 = None;
    for frame in frames {
        let step =
            step_world_with_source_collision_step(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 3426 {
            step3426 = Some(step);
            p2_frame3426 = Some(world.players()[1]);
        }
        if frame.source_frame == 3428 {
            p2_frame3428 = Some(world.players()[1]);
        }
        if frame.source_frame == 3433 {
            p2_frame3433 = Some(world.players()[1]);
            break;
        }
    }

    let step = step3426.expect("source frame 3426 should be simulated");
    let p2_frame3426 = p2_frame3426.expect("P2 frame 3426 should be captured");
    assert_eq!(
        p2_frame3426.melee_action_state_id,
        Some(MeleeActionStateId::new(350)),
        "Raptor Boost detect should enter SpecialS before P1's NAir damages P2 on source frame 3428; geometry={:?} raw={:?} logged={:?} confirms={:?} stages={:?} results={:?} p2={:?}",
        step.geometry_confirms,
        step.raw_confirms,
        step.logged_confirms,
        step.confirms,
        step.stages,
        step.results,
        p2_frame3426
    );
    assert_eq!(p2_frame3426.velocity.x, 839);
    assert_eq!(p2_frame3426.velocity.y, 0);

    let p2_frame3428 = p2_frame3428.expect("P2 frame 3428 should be captured");
    assert_eq!(
        p2_frame3428.melee_action_state_id,
        Some(MeleeActionStateId::new(88)),
        "the stale frame-3426 overlap must not enter the victim log and suppress the real NAir hit on frame 3428"
    );
    assert_eq!(
        p2_frame3428.velocity,
        Vec2 {
            x: -1_998,
            y: 1_930
        }
    );
    assert_eq!(p2_frame3428.hitlag_frames, 5);

    let p2_frame3433 = p2_frame3433.expect("P2 frame 3433 should be captured");
    assert_eq!(
        p2_frame3433.melee_action_state_id,
        Some(MeleeActionStateId::new(199)),
        "the grounded damage launch must retain its ECB bottom through hitlag so the first active floor collision can enter Passive"
    );
    assert_eq!(p2_frame3433.position.y, 0);
    assert_eq!(p2_frame3433.velocity, Vec2 { x: -2_236, y: -130 });
}
#[test]
fn slippi_match_start_frame2300_p2_capture_pulled_lw_stays_grounded() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(2_430))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let mut p2_frame2300 = None;
    let mut p2_frame2301 = None;
    for frame in frames {
        step_world_with_source_collision_step(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 2300 {
            p2_frame2300 = Some(world.players()[1]);
        }
        if frame.source_frame == 2301 {
            p2_frame2301 = Some(world.players()[1]);
            break;
        }
    }

    let p2_frame2300 = p2_frame2300.expect("P2 frame 2300 should be captured");
    assert_eq!(
        p2_frame2300.melee_action_state_id,
        Some(MeleeActionStateId::new(226)),
        "a grounded center-stage pull must remain CapturePulledLw"
    );
    assert!(p2_frame2300.grounded);
    assert_eq!(p2_frame2300.position, Vec2 { x: 46_050, y: 0 });

    let p2_frame2301 = p2_frame2301.expect("P2 frame 2301 should be captured");
    assert_eq!(
        p2_frame2301.melee_action_state_id,
        Some(MeleeActionStateId::new(227)),
        "CatchPull should advance the grounded victim to CaptureWaitLw"
    );
    assert!(p2_frame2301.grounded);
}
#[test]
fn slippi_match_start_frame3467_p2_capture_pulled_lw_transitions_airborne_hi() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(3_620))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let mut p2_frame3467 = None;
    let mut p2_frame3468 = None;
    for frame in frames {
        step_world_with_source_collision_step(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 3467 {
            p2_frame3467 = Some(world.players()[1]);
        }
        if frame.source_frame == 3468 {
            p2_frame3468 = Some(world.players()[1]);
            break;
        }
    }

    let p2_frame3467 = p2_frame3467.expect("P2 frame 3467 should be captured");
    assert_eq!(
        p2_frame3467.melee_action_state_id,
        Some(MeleeActionStateId::new(223)),
        "CapturePulledLw_Coll must enter CapturePulledHi when the capture-joint pull loses floor support"
    );
    assert!(!p2_frame3467.grounded);
    assert_eq!(
        p2_frame3467.position,
        Vec2 {
            x: -74_683,
            y: -493
        }
    );

    let p2_frame3468 = p2_frame3468.expect("P2 frame 3468 should be captured");
    assert_eq!(
        p2_frame3468.melee_action_state_id,
        Some(MeleeActionStateId::new(224)),
        "CatchPull should advance the airborne victim from CapturePulledHi to CaptureWaitHi"
    );
    assert!(!p2_frame3468.grounded);
}

#[test]
fn slippi_match_start_frame3473_down_throw_uses_weight_scaled_pose_rate() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(3_620))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let mut players_frame3473 = None;
    for frame in frames {
        step_world_with_source_collision_step(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 3473 {
            players_frame3473 = Some(*world.players());
            break;
        }
    }

    let [thrower, victim] = players_frame3473.expect("source frame 3473 should be captured");
    assert_eq!(
        thrower.melee_action_state_id,
        Some(MeleeActionStateId::new(222))
    );
    assert_eq!(
        victim.melee_action_state_id,
        Some(MeleeActionStateId::new(242))
    );
    assert_eq!(
        thrower.motion_anim_rate_milli, 962,
        "ftCo_800DD4B0 scales Falcon down-throw animation by victim weight because bit 3 is clear in weight_independent_throws_mask"
    );
    assert_eq!(victim.motion_anim_rate_milli, 962);
    assert_eq!(
        victim.position,
        Vec2 {
            x: -70_317,
            y: -1_754
        },
        "ftCo_800DE508 should constrain the thrown victim to the weight-scaled ThrowLw TransN2 pose"
    );
}

#[test]
fn slippi_match_start_frame3493_down_throw_release_uses_damage_fly_top() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(3_630))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let mut p2_frame3493 = None;
    for frame in frames {
        step_world_with_source_collision_step(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 3493 {
            p2_frame3493 = Some(world.players()[1]);
            break;
        }
    }

    let victim = p2_frame3493.expect("P2 frame 3493 should be captured");
    assert_eq!(
        victim.melee_action_state_id,
        Some(MeleeActionStateId::new(90)),
        "ftCo_800DE7C0 forces motion 90 when the released victim came from ThrowLw; damage_angle={} knockback={} velocity=({}, {})",
        victim.damage_angle,
        victim.damage_knockback,
        victim.source_knockback_velocity_x,
        victim.source_knockback_velocity_y,
    );
    assert_eq!(victim.velocity, Vec2 { x: -406, y: 2_628 });
}

#[test]
fn slippi_match_start_frame3514_throw_end_runs_wait_iasa_same_tick() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(3_650))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let mut p1_frame3514 = None;
    for frame in frames {
        step_world_with_source_collision_step(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 3514 {
            p1_frame3514 = Some(world.players()[0]);
            break;
        }
    }

    let thrower = p1_frame3514.expect("P1 frame 3514 should be captured");
    assert_eq!(
        thrower.melee_action_state_id,
        Some(MeleeActionStateId::new(20)),
        "ThrowLw_Anim enters Wait through ftCommon_8007D92C, then the new Wait IASA callback must consume the same tick's fresh left input"
    );
    assert_eq!(thrower.motion_state, MotionState::Dash);
    assert_eq!(thrower.ground_velocity_x, -2.0);
    assert_eq!(thrower.position, Vec2 { x: -66_491, y: 0 });
}

#[test]
fn slippi_match_start_frame3530_damage_hitstun_end_runs_air_jump_before_rehit() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(3_670))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let mut p2_frame3530 = None;
    for frame in frames {
        step_world_with_source_collision_step(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 3530 {
            p2_frame3530 = Some(world.players()[1]);
            break;
        }
    }

    let victim = p2_frame3530.expect("P2 frame 3530 should be captured");
    assert_eq!(
        victim.melee_action_state_id,
        Some(MeleeActionStateId::new(88))
    );
    assert_eq!(
        victim.position,
        Vec2 {
            x: -84_733,
            y: -15_018
        },
        "DamageFlyTop_Anim clears hitstun before DamageFly_IASA consumes Y into JumpAerialF; jump physics must run before the fair collision starts new hitlag"
    );
    assert_eq!(victim.jumps_remaining, 0);
    assert_eq!(
        victim.velocity,
        Vec2 {
            x: -3_897,
            y: 2_435
        }
    );
}

#[test]
fn slippi_match_start_frame3542_damage_hitlag_uses_current_input_for_sdi() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(3_680))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let mut p2_frame3542 = None;
    for frame in frames {
        step_world_with_source_collision_step(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 3542 {
            p2_frame3542 = Some(world.players()[1]);
            break;
        }
    }

    let victim = p2_frame3542.expect("P2 frame 3542 should be captured");
    assert_eq!(
        victim.position,
        Vec2 {
            x: -79_033,
            y: -13_218
        },
        "Damage_OnEveryHitlag must consume the current 0.95/0.30 stick tap, producing the decomp's +5.7/+1.8 SDI displacement"
    );
    assert_eq!(
        victim.velocity,
        Vec2 {
            x: -3_897,
            y: 2_435
        }
    );
}

#[test]
fn slippi_match_start_frame3973_jumpsquat_iasa_nair_runs_attack_air_gravity() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(4_110))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let mut p1_frame3973 = None;
    for frame in frames {
        step_world_with_source_collision_step(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 3973 {
            p1_frame3973 = Some(world.players()[0]);
            break;
        }
    }

    let player = p1_frame3973.expect("P1 frame 3973 should be captured");
    assert_eq!(player.motion_state, MotionState::AttackAirN);
    assert_eq!(
        player.melee_action_state_id,
        Some(MeleeActionStateId::new(65))
    );
    assert_eq!(
        player.position,
        Vec2 {
            x: 17_110,
            y: 1_770
        }
    );
    assert_eq!(player.velocity, Vec2 { x: 1_210, y: 1_770 });
}

#[test]
fn slippi_match_start_frame4109_turn_cancel_preserves_pre_turn_facing_for_back_jump() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(4_250))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let mut p1_frame4109 = None;
    for frame in frames {
        step_world_with_source_collision_step(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 4109 {
            p1_frame4109 = Some(world.players()[0]);
            break;
        }
    }

    let player = p1_frame4109.expect("P1 frame 4109 should be captured");
    assert_eq!(player.facing, 1);
    assert_eq!(player.motion_state, MotionState::JumpAerialB);
    assert_eq!(
        player.melee_action_state_id,
        Some(MeleeActionStateId::new(28))
    );
    assert_eq!(player.velocity, Vec2 { x: -854, y: 2_660 });
}

#[test]
fn slippi_match_start_frame4118_double_jump_ecb_lock_prevents_false_air_dodge_landing() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(4_250))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let mut p1_frame4118 = None;
    for frame in frames {
        step_world_with_source_collision_step(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 4118 {
            p1_frame4118 = Some(world.players()[0]);
            break;
        }
    }

    let player = p1_frame4118.expect("P1 frame 4118 should be captured");
    assert_eq!(player.motion_state, MotionState::EscapeAir);
    assert!(!player.grounded);
    assert_eq!(
        player.position,
        Vec2 {
            x: 50_144,
            y: 23_266
        },
        "ftCo_JumpAerial_Enter_Basic resets the ftCommon_8007D5D4 ECB lock; when it expires on this tick, the bottom moves upward from the locked value and mpCheckFloor must not invent a platform crossing"
    );
    assert_eq!(
        player.velocity,
        Vec2 {
            x: -2_467,
            y: -1_304
        }
    );
}

#[test]
fn slippi_match_start_frame4119_p2_nair_hits_p1_escape_air() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(4_250))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let mut frame4119_step = None;
    for frame in frames {
        let step =
            step_world_with_source_collision_step(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 4119 {
            frame4119_step = Some(step);
            break;
        }
    }

    let step = frame4119_step.expect("source frame 4119 should be present");
    let player = world.players()[0];
    assert_eq!(
        player.melee_action_state_id,
        Some(MeleeActionStateId::new(86)),
        "P2 AttackAirN must hit P1 EscapeAir on the same frame as Slippi; geometry={:?} raw={:?} logged={:?} confirms={:?} stages={:?} results={:?}",
        step.geometry_confirms,
        step.raw_confirms,
        step.logged_confirms,
        step.confirms,
        step.stages,
        step.results,
    );
    assert_eq!(
        player.position,
        Vec2 {
            x: 47_924,
            y: 22_093
        }
    );
    assert_eq!(
        player.velocity,
        Vec2 { x: 1_256, y: 1_256 },
        "ftCo_Damage_CheckAirMotion must apply x190 from the x680/x684 digital-LR timers; results={:?} stages={:?} trigger_timer={}",
        step.results,
        step.stages,
        world.input_timers()[0].trigger,
    );
}

#[test]
fn slippi_match_start_frame4182_p1_upair_sets_off_p2_guard() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(4_310))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let mut frame4182_step = None;
    for frame in frames {
        let step =
            step_world_with_source_collision_step(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 4182 {
            frame4182_step = Some(step);
            break;
        }
    }

    let step = frame4182_step.expect("source frame 4182 should be present");
    let shield_confirms = step
        .logged_confirms
        .iter()
        .filter(|confirm| {
            confirm.attacker_index == 0
                && confirm.victim_index == 1
                && confirm.source_action_key == Some(SourceActionKey::new("AttackAirHi"))
                && confirm.hurtbox_id == SOURCE_SHIELD_HURTBOX_ID
        })
        .collect::<Vec<_>>();
    assert!(
        !shield_confirms.is_empty(),
        "source frame 4182 must route P1 AttackAirHi through P2's newly installed shield; geometry={:?} raw={:?} logged={:?}",
        step.geometry_confirms,
        step.raw_confirms,
        step.logged_confirms,
    );
    assert_eq!(
        world.players()[1].melee_action_state_id,
        Some(MeleeActionStateId::new(181))
    );
    assert_eq!(world.players()[1].motion_state, MotionState::GuardSetOff);
    assert_eq!(
        source_units_to_milli(world.players()[1].ground_velocity_x),
        888
    );
}

#[test]
fn slippi_match_start_frame4192_upair_update_does_not_rehit_p2_guard() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(4_330))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let mut frame4192_step = None;
    for frame in frames {
        let step =
            step_world_with_source_collision_step(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 4192 {
            frame4192_step = Some(step);
            break;
        }
    }

    let step = frame4192_step.expect("source frame 4192 should be present");
    assert!(
        step.logged_confirms.iter().all(|confirm| {
            !(confirm.attacker_index == 0
                && confirm.victim_index == 1
                && confirm.hurtbox_id == SOURCE_SHIELD_HURTBOX_ID)
        }),
        "the in-place up-air hitbox update must retain the decomp victim log; logged={:?}",
        step.logged_confirms,
    );
    let guard = world.players()[1];
    assert_eq!(guard.motion_state, MotionState::GuardSetOff);
    assert_eq!(guard.motion_frame, 4);
    assert_eq!(source_units_to_milli(guard.ground_velocity_x), 488);
}

#[test]
fn slippi_match_start_frame4378_guard_setoff_exits_hitlag_at_source_position() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(4_510))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    for frame in frames {
        step_world_with_source_collision_step(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 4378 {
            let guard = world.players()[0];
            assert_eq!(guard.motion_state, MotionState::GuardSetOff);
            assert_eq!(guard.position, Vec2 { x: -16_509, y: 0 });
            assert_eq!(source_units_to_milli(guard.ground_velocity_x), -592);
            return;
        }
    }
    panic!("source frame 4378 should be present");
}

#[test]
fn slippi_match_start_frame4403_grounded_downward_hit_reflects_from_floor() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(4_535))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    for frame in frames {
        step_world_with_source_collision_step(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 4403 {
            let victim = world.players()[1];
            assert_eq!(
                victim.melee_action_state_id,
                Some(MeleeActionStateId::new(88))
            );
            assert_eq!(
                source_units_to_milli(victim.source_knockback_velocity_y),
                2_815
            );
            assert_eq!(victim.position, Vec2 { x: -12_895, y: 0 });
            return;
        }
    }
    panic!("source frame 4403 should be present");
}

#[test]
fn slippi_match_start_frame4426_lcancel_timer_survives_hitlag() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(6_000))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    for frame in frames {
        step_world_with_source_collision_step(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 4426 {
            let player = world.players()[0];
            assert_eq!(
                player.motion_state,
                MotionState::KneeBend,
                "source_lr_timer={} trigger_timer={} landing_lag={}",
                player.source_lr_digital_press_timer,
                world.input_timers()[0].trigger,
                player.landing_lag_ticks,
            );
            assert_eq!(
                player.melee_action_state_id,
                Some(MeleeActionStateId::new(24))
            );
            assert_eq!(player.position, Vec2 { x: 1_098, y: 0 });
            return;
        }
    }
    panic!("source frame 4426 should be present");
}

#[test]
fn slippi_match_start_frame4488_down_wait_enters_forward_roll() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(6_000))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    for frame in frames {
        step_world_with_source_collision_step(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 4488 {
            let player = world.players()[1];
            assert_eq!(
                player.melee_action_state_id,
                Some(MeleeActionStateId::new(196)),
                "input={:?} stick=({}, {}) cstick=({}, {}) facing={} action_key={:?}",
                frame.inputs[1],
                frame.inputs[1].stick_x(),
                frame.inputs[1].stick_y(),
                frame.inputs[1].c_stick_x(),
                frame.inputs[1].c_stick_y(),
                player.facing,
                player.source_action_key,
            );
            assert_eq!(
                player.position,
                Vec2 {
                    x: -49_660,
                    y: 27_200
                }
            );
            return;
        }
    }
    panic!("source frame 4488 should be present");
}

#[test]
fn slippi_match_start_frame4511_down_roll_clamps_to_floor_edge() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(6_000))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    for frame in frames {
        step_world_with_source_collision_step(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 4511 {
            let player = world.players()[1];
            assert_eq!(
                player.melee_action_state_id,
                Some(MeleeActionStateId::new(196))
            );
            assert_eq!(
                player.position,
                Vec2 {
                    x: -20_000,
                    y: 27_200
                }
            );
            assert_eq!(source_units_to_milli(player.ground_velocity_x), 1_029);
            return;
        }
    }
    panic!("source frame 4511 should be present");
}

#[test]
fn slippi_match_start_frame4518_upair_hits_down_roll() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(6_000))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    for frame in frames {
        let step =
            step_world_with_source_collision_step(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 4518 {
            let victim = world.players()[1];
            assert_eq!(
                victim.melee_action_state_id,
                Some(MeleeActionStateId::new(80)),
                "geometry={:?} raw={:?} logged={:?} confirms={:?} collision_state={} hurt_state={}",
                step.geometry_confirms,
                step.raw_confirms,
                step.logged_confirms,
                step.confirms,
                victim.source_collision_state,
                victim.source_hurt_collision_state,
            );
            return;
        }
    }
    panic!("source frame 4518 should be present");
}

#[test]
fn slippi_match_start_frame4557_released_guard_startup_rejects_z_grab() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(6_000))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    for frame in frames {
        step_world_with_source_collision_step(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 4557 {
            assert_eq!(
                world.players()[1].melee_action_state_id,
                Some(MeleeActionStateId::new(178)),
                "ftCo_Catch_CheckInput requires held LR together with the synthetic A press from Z"
            );
            return;
        }
    }
    panic!("source frame 4557 should be present");
}

#[test]
fn slippi_match_start_frame4641_raptor_boost_detects_waiting_fighter() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(6_000))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let mut recent = Vec::new();
    for frame in frames {
        let step =
            step_world_with_source_collision_step(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame >= 4638 {
            recent.push((
                frame.source_frame,
                step.geometry_confirms.len(),
                step.raw_confirms.len(),
                step.logged_confirms.len(),
                world.players()[0].melee_action_state_id,
            ));
        }
        if frame.source_frame == 4641 {
            let collision_frame =
                source_collision_frame_from_frame(&RenderFrame::from_world(&world));
            assert_eq!(
                world.players()[0].melee_action_state_id,
                Some(MeleeActionStateId::new(350)),
                "recent={:?} geometry={:?} raw={:?} logged={:?} hits={:?} hurts={:?}",
                recent,
                step.geometry_confirms,
                step.raw_confirms,
                step.logged_confirms,
                collision_frame.hits,
                collision_frame.hurts,
            );
            return;
        }
    }
    panic!("source frame 4641 should be present");
}

#[test]
fn slippi_match_start_frame4693_damage_fly_landing_enters_passive() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(6_000))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    for frame in frames {
        step_world_with_source_collision_step(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 4693 {
            let player = world.players()[1];
            assert_eq!(
                player.melee_action_state_id,
                Some(MeleeActionStateId::new(199)),
                "current_lr_timer={} previous_lr_timer={} grounded={} action={:?}",
                player.source_lr_digital_press_timer,
                player.source_previous_lr_digital_press_timer,
                player.grounded,
                player.source_action_key,
            );
            return;
        }
    }
    panic!("source frame 4693 should be present");
}

#[test]
fn slippi_match_start_frame4709_landing_to_squat_wait_keeps_ground_physics() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(6_000))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    for frame in frames {
        step_world_with_source_collision_step(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 4709 {
            let player = world.players()[0];
            assert_eq!(
                player.melee_action_state_id,
                Some(MeleeActionStateId::new(40))
            );
            assert_eq!(player.position, Vec2 { x: -36_299, y: 0 });
            assert_eq!(source_units_to_milli(player.ground_velocity_x), -566);
            return;
        }
    }
    panic!("source frame 4709 should be present");
}

#[test]
fn slippi_match_start_frame4720_wait_has_no_stale_guard_collision_object() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(6_000))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    for frame in frames {
        step_world_with_source_collision_step(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 4720 {
            let player = world.players()[1];
            assert_eq!(
                player.melee_action_state_id,
                Some(MeleeActionStateId::new(14))
            );
            assert!(
                !player.source_shield_collision_active && !player.source_shield_hit_active,
                "Wait must not retain ftColl's Guard hit object: {player:?}"
            );
            return;
        }
    }
    panic!("source frame 4720 should be present");
}

#[test]
fn slippi_match_start_frame4721_new_guard_object_collides_on_next_fighter_pass() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(6_000))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let mut state_4721 = None;
    let mut state_4722 = None;
    let mut hitlag_4722 = None;
    for frame in frames {
        step_world_with_source_collision_step(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 4721 {
            state_4721 = world.players()[1].melee_action_state_id;
        }
        if frame.source_frame == 4722 {
            state_4722 = world.players()[1].melee_action_state_id;
            hitlag_4722 = Some((
                world.players()[0].hitlag_frames,
                world.players()[1].hitlag_frames,
            ));
            break;
        }
    }
    assert_eq!(state_4721, Some(MeleeActionStateId::new(178)));
    assert_eq!(state_4722, Some(MeleeActionStateId::new(181)));
    assert_eq!(hitlag_4722, Some((6, 6)));
}

#[test]
fn slippi_match_start_frame4788_bair_uses_source_knockback_magnitude() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(6_000))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    for frame in frames {
        let step =
            step_world_with_source_collision_step(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 4788 {
            let victim = world.players()[1];
            assert_eq!(
                victim.melee_action_state_id,
                Some(MeleeActionStateId::new(88))
            );
            assert_eq!(
                source_units_to_milli(victim.source_knockback_velocity_x),
                -3001,
                "results={:?} confirms={:?} attacker={:?} victim={:?}",
                step.results,
                step.logged_confirms,
                world.players()[0],
                victim
            );
            assert_eq!(
                source_units_to_milli(victim.source_knockback_velocity_y),
                2898
            );
            return;
        }
    }
    panic!("source frame 4788 should be present");
}

#[test]
fn slippi_match_start_frame4904_p2_jump_aerial_f_uses_right_facing_live_ecb() {
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let trace = trace_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreTraceConfig {
            player_index: 1,
            source_frame_start: 4904,
            source_frame_end: 4904,
            max_frames: Some(5_100),
        },
    )
    .expect("match-start replay fixture should trace P2 source frame 4904");
    let row = trace.rows.first().expect("source frame 4904 should exist");

    assert_eq!(row.actual_position, row.expected_position, "Fighter creation sets TopN rot_y to +PI/2 for facing_dir +1; the right-facing live JumpAerialF ECB must therefore retain the +PI/2 baked orientation during Battlefield wall correction: {row:?}");
}

#[test]
fn slippi_match_start_frame4926_p2_attack_air_hi_expanding_ecb_matches_left_wall() {
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let trace = trace_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreTraceConfig {
            player_index: 1,
            source_frame_start: 4926,
            source_frame_end: 4926,
            max_frames: Some(5_100),
        },
    )
    .expect("match-start replay fixture should trace P2 source frame 4926");
    let row = trace.rows.first().expect("source frame 4926 should exist");

    assert_eq!(
        row.actual_position, row.expected_position,
        "AttackAirHi frame 6 expands the live ECB while P2 hugs Battlefield line 17; the translated JObj pose and left-wall correction must remain exact: {row:?}"
    );
}

#[test]
fn slippi_match_start_frame4958_p1_damage_air2_animation_enters_fall() {
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let trace = trace_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreTraceConfig {
            player_index: 0,
            source_frame_start: 4958,
            source_frame_end: 4958,
            max_frames: Some(5_100),
        },
    )
    .expect("match-start replay fixture should trace P1 source frame 4958");
    let row = trace.rows.first().expect("source frame 4958 should exist");

    assert_eq!(row.actual_action_state_id, Some(row.expected_action_state_id), "ftCo_8008DC0C calls ftAnim_8006EBA4 immediately after entering DamageAir2, so its frame timeline must reach animation completion and ftCo_Fall_Enter on the same tick as the source: {row:?}");
    assert_eq!(row.actual_motion_state, MotionState::Fall);
    assert_eq!(row.actual_velocity_x, row.expected_composed_velocity_x);
    assert_eq!(row.actual_velocity_y, row.expected_composed_velocity_y);
}

#[test]
fn slippi_match_start_through_frame4966_p2_grab_does_not_capture_p1() {
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let trace = trace_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreTraceConfig {
            player_index: 0,
            source_frame_start: 4965,
            source_frame_end: 4966,
            max_frames: Some(5_100),
        },
    )
    .expect("match-start replay fixture should trace P1 through source frame 4966");
    let states = trace
        .rows
        .iter()
        .map(|row| {
            (
                row.source_frame,
                row.expected_action_state_id,
                row.actual_action_state_id,
                row.actual_motion_state,
            )
        })
        .collect::<Vec<_>>();
    assert!(
        trace.rows.iter().all(|row| {
            row.actual_action_state_id == Some(row.expected_action_state_id)
                && row.actual_motion_state == MotionState::Landing
        }),
        "ftColl_80078A2C passes each fighter's cur_pos.z to lbColl_80007ECC; preserving their Z separation must keep P1 in Landing: {states:?}"
    );
}

#[test]
fn slippi_match_start_frame4989_p2_catch_completion_allows_guard_on() {
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let trace = trace_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreTraceConfig {
            player_index: 1,
            source_frame_start: 4989,
            source_frame_end: 4989,
            max_frames: Some(5_150),
        },
    )
    .expect("match-start replay fixture should trace P2 source frame 4989");
    let row = trace.rows.first().expect("source frame 4989 should exist");
    assert_eq!(
        row.actual_action_state_id,
        Some(row.expected_action_state_id)
    );
    assert_eq!(row.actual_motion_state, MotionState::GuardOn);
}

#[test]
fn slippi_match_start_frame4722_shield_setoff_uses_collision_phase_inputs() {
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let trace = trace_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreTraceConfig {
            player_index: 1,
            source_frame_start: 4722,
            source_frame_end: 4722,
            max_frames: Some(4_850),
        },
    )
    .expect("match-start replay fixture should trace P2 source frame 4722");
    let row = trace.rows.first().expect("source frame 4722 should exist");
    assert_eq!(
        row.actual_action_state_id,
        Some(row.expected_action_state_id)
    );
    assert_eq!(row.actual_motion_state, MotionState::GuardSetOff);
    assert_eq!(row.actual_ground_velocity_x, -2.0);
    assert_eq!(
        row.actual_ground_velocity_x,
        row.expected_ground_velocity_x_source
    );
}

#[test]
fn slippi_match_start_frame5016_grounded_attacker_receives_shield_recoil() {
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let trace = trace_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreTraceConfig {
            player_index: 0,
            source_frame_start: 5016,
            source_frame_end: 5016,
            max_frames: Some(5_150),
        },
    )
    .expect("match-start replay fixture should trace P1 source frame 5016");
    let row = trace.rows.first().expect("source frame 5016 should exist");
    assert_eq!(
        row.actual_action_state_id,
        Some(row.expected_action_state_id)
    );
    assert_eq!(row.actual_position, row.expected_position);
}

#[test]
fn slippi_match_start_frame5037_special_s_completion_runs_wait_input_and_walk_physics() {
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let trace = trace_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreTraceConfig {
            player_index: 0,
            source_frame_start: 5037,
            source_frame_end: 5037,
            max_frames: Some(5_175),
        },
    )
    .expect("match-start replay fixture should trace P1 source frame 5037");
    let row = trace.rows.first().expect("source frame 5037 should exist");

    assert_eq!(
        row.actual_action_state_id,
        Some(row.expected_action_state_id)
    );
    assert_eq!(row.actual_motion_state, MotionState::WalkSlow);
    assert_eq!(row.actual_velocity_x, row.expected_ground_velocity_x);
    assert_eq!(row.actual_position, row.expected_position);
}

#[test]
fn slippi_match_start_frame5040_early_dash_digital_shield_enters_escape_forward() {
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let trace = trace_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreTraceConfig {
            player_index: 0,
            source_frame_start: 5040,
            source_frame_end: 5040,
            max_frames: Some(5_175),
        },
    )
    .expect("match-start replay fixture should trace P1 source frame 5040");
    let row = trace.rows.first().expect("source frame 5040 should exist");

    assert_eq!(
        row.actual_action_state_id,
        Some(row.expected_action_state_id)
    );
    assert_eq!(row.actual_motion_state, MotionState::EscapeF);
    assert_eq!(row.actual_position, row.expected_position);
}

#[test]
fn slippi_match_start_frame5081_attack_dash_uses_transn_ground_motion() {
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let trace = trace_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreTraceConfig {
            player_index: 0,
            source_frame_start: 5081,
            source_frame_end: 5081,
            max_frames: Some(5_250),
        },
    )
    .expect("match-start replay fixture should trace P1 source frame 5081");
    let row = trace.rows.first().expect("source frame 5081 should exist");

    assert_eq!(
        row.actual_action_state_id,
        Some(row.expected_action_state_id)
    );
    assert_eq!(row.actual_motion_state, MotionState::AttackDash);
    assert_eq!(row.actual_velocity_x, row.expected_ground_velocity_x);
    assert_eq!(row.actual_position, row.expected_position);
}

#[test]
fn slippi_match_start_frame5118_squat_collision_enters_fall_off_left_edge() {
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let trace = trace_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreTraceConfig {
            player_index: 0,
            source_frame_start: 5118,
            source_frame_end: 5118,
            max_frames: Some(5_313),
        },
    )
    .expect("match-start replay fixture should trace P1 source frame 5118");
    let row = trace.rows.first().expect("source frame 5118 should exist");

    assert_eq!(
        row.actual_action_state_id,
        Some(row.expected_action_state_id)
    );
    assert_eq!(row.actual_motion_state, MotionState::Fall);
    assert_eq!(row.actual_velocity_x, row.expected_air_velocity_x);
    assert_eq!(row.actual_position, row.expected_position);
}

#[test]
fn slippi_match_start_frame326_escape_air_landing_preserves_horizontal_travel() {
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let trace = trace_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreTraceConfig {
            player_index: 1,
            source_frame_start: 326,
            source_frame_end: 326,
            max_frames: Some(500),
        },
    )
    .expect("match-start replay fixture should trace P2 source frame 326");
    let row = trace.rows.first().expect("source frame 326 should exist");

    assert_eq!(
        row.actual_action_state_id,
        Some(row.expected_action_state_id)
    );
    assert_eq!(row.actual_motion_state, MotionState::LandingFallSpecial);
    assert_eq!(row.actual_position, row.expected_position);
}

#[test]
fn visual_slippi_frame4926_uses_export_configured_costume_world() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(5_050))
        .expect("visual replay fixture should parse");
    let mut world = slippi_match_start_world_from_export(&export)
        .expect("visual replay should construct its gameplay world from match settings");

    let mut target = None;
    for frame in frames {
        step_world_with_source_collisions(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 4926 {
            target = Some(frame);
            break;
        }
    }
    let frame = target.expect("source frame 4926 should exist");
    let divergence = slippi_visual_replay_divergence_for_frame(
        &frame,
        world.snapshot(),
        SlippiCoreDivergenceScanConfig::default(),
    );

    assert!(
        divergence.is_none(),
        "SDL visual playback must use the same export-configured gameplay world as match-start comparison: {divergence:?}"
    );
}

#[test]
fn slippi_match_start_frame5124_cliff_jump_quick_uses_air_collision_correction() {
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let trace = trace_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreTraceConfig {
            player_index: 1,
            source_frame_start: 5124,
            source_frame_end: 5124,
            max_frames: Some(5_313),
        },
    )
    .expect("match-start replay fixture should trace P2 source frame 5124");
    let row = trace.rows.first().expect("source frame 5124 should exist");

    assert_eq!(
        row.actual_action_state_id,
        Some(row.expected_action_state_id)
    );
    assert_eq!(row.actual_motion_state, MotionState::CliffJumpQuick1);
    assert_eq!(row.actual_position, row.expected_position);
}

#[test]
fn slippi_match_start_frame5131_cliff_jump_quick_2_sets_entry_velocity() {
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let trace = trace_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreTraceConfig {
            player_index: 1,
            source_frame_start: 5131,
            source_frame_end: 5131,
            max_frames: Some(5_313),
        },
    )
    .expect("match-start replay fixture should trace P2 source frame 5131");
    let row = trace.rows.first().expect("source frame 5131 should exist");

    assert_eq!(
        row.actual_action_state_id,
        Some(row.expected_action_state_id)
    );
    assert_eq!(row.actual_motion_state, MotionState::CliffJumpQuick2);
    assert_eq!(row.actual_velocity_x, row.expected_air_velocity_x);
    assert_eq!(row.actual_velocity_y, row.expected_velocity_y);
    assert_eq!(row.actual_position, row.expected_position);
}

#[test]
fn slippi_match_start_frame5126_p1_fall_preserves_left_wall_position() {
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let trace = trace_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreTraceConfig {
            player_index: 0,
            source_frame_start: 5125,
            source_frame_end: 5126,
            max_frames: Some(5_313),
        },
    )
    .expect("match-start replay fixture should trace P1 through source frame 5126");
    let row = trace
        .rows
        .iter()
        .find(|row| row.source_frame == 5126)
        .expect("source frame 5126 should exist");

    assert_eq!(row.actual_motion_state, MotionState::Fall);
    assert_eq!(row.actual_velocity_x, row.expected_air_velocity_x);
    assert_eq!(row.actual_velocity_y, row.expected_velocity_y);
    assert_eq!(row.actual_position, row.expected_position);
}

#[test]
fn slippi_match_start_frame5162_p2_cliff_jump_quick2_rejects_soft_platform() {
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let trace = trace_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreTraceConfig {
            player_index: 1,
            source_frame_start: 5161,
            source_frame_end: 5162,
            max_frames: Some(5_313),
        },
    )
    .expect("match-start replay fixture should trace P2 through source frame 5162");
    let row = trace
        .rows
        .iter()
        .find(|row| row.source_frame == 5162)
        .expect("source frame 5162 should exist");

    assert_eq!(row.actual_motion_state, MotionState::CliffJumpQuick2);
    assert_eq!(row.actual_velocity_x, row.expected_air_velocity_x);
    assert_eq!(row.actual_velocity_y, row.expected_velocity_y);
    assert_eq!(row.actual_position, row.expected_position);
}

#[test]
fn slippi_match_start_frame5163_p2_cliff_jump_quick2_ends_with_aobj() {
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let trace = trace_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreTraceConfig {
            player_index: 1,
            source_frame_start: 5163,
            source_frame_end: 5163,
            max_frames: Some(5_313),
        },
    )
    .expect("match-start replay fixture should trace P2 source frame 5163");
    let row = trace.rows.first().expect("source frame 5163 should exist");

    assert_eq!(
        row.actual_action_state_id,
        Some(row.expected_action_state_id)
    );
    assert_eq!(row.actual_motion_state, MotionState::Fall);
    assert_eq!(row.actual_velocity_x, row.expected_air_velocity_x);
    assert_eq!(row.actual_velocity_y, row.expected_velocity_y);
    assert_eq!(row.actual_position, row.expected_position);
}

#[test]
fn slippi_match_start_frame5189_final_stock_enters_dead_down() {
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let trace = trace_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreTraceConfig {
            player_index: 0,
            source_frame_start: 5189,
            source_frame_end: 5189,
            max_frames: Some(5_313),
        },
    )
    .expect("match-start replay fixture should trace P1 source frame 5189");
    let row = trace.rows.first().expect("source frame 5189 should exist");

    assert_eq!(
        row.actual_action_state_id,
        Some(row.expected_action_state_id)
    );
    assert_eq!(row.actual_motion_state, MotionState::DeadDown);
    assert_eq!(row.actual_velocity_x, row.expected_air_velocity_x);
    assert_eq!(row.actual_velocity_y, row.expected_velocity_y);
    assert_eq!(row.actual_position, row.expected_position);
}

#[test]
fn slippi_match_start_frame3289_p2_fast_falls_after_attacker_hitlag() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };

    let trace = trace_slippi_export_from_match_start_with_core(
        &export,
        SlippiCoreTraceConfig {
            player_index: 1,
            source_frame_start: 3284,
            source_frame_end: 3289,
            max_frames: Some(3_420),
        },
    )
    .expect("match-start replay fixture should trace P2 3284-3289");

    let frame3289 = trace
        .rows
        .iter()
        .find(|row| row.source_frame == 3289)
        .expect("source frame 3289 should be traced");
    assert_eq!(frame3289.actual_motion_state, MotionState::AttackAirN);
    assert_eq!(frame3289.actual_position, frame3289.expected_position);
    assert_eq!(
        frame3289.actual_velocity_y, frame3289.expected_composed_velocity_y,
        "the downward tap during attacker hitlag should remain fresh enough to fast-fall on the first active frame"
    );
    assert_eq!(frame3289.actual_velocity_y, -3_500);
}

#[test]
fn slippi_match_start_frame2312_throw_hi_hit_capsules_spawn_but_held_victim_is_not_generically_damaged(
) {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(2_436))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let mut frame2312_step = None;
    for frame in frames {
        let step =
            step_world_with_source_collision_step(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 2312 {
            frame2312_step = Some(step);
            break;
        }
    }

    let render_frame = RenderFrame::from_world(&world);
    let collision_frame = source_collision_frame_from_frame(&render_frame);
    let confirms = source_hit_confirms_from_frame(&render_frame);
    let p1_hits = collision_frame
        .hits
        .iter()
        .filter(|capsule| capsule.owner_index == 0)
        .copied()
        .collect::<Vec<_>>();
    let p2_hurts = collision_frame
        .hurts
        .iter()
        .filter(|capsule| capsule.owner_index == 1)
        .copied()
        .collect::<Vec<_>>();
    let players = world.players();

    assert_eq!(
        players[0].melee_action_state_id,
        Some(MeleeActionStateId::new(221)),
        "P1 should still be in ThrowHi before ftCo_800DD724 consumes throw_flags_b3"
    );
    assert_eq!(
        players[1].melee_action_state_id,
        Some(MeleeActionStateId::new(241)),
        "P2 should still be in TCaptainThrowHi before ftCo_800DD724 releases the held victim"
    );
    assert!(
        !p1_hits.is_empty()
            && p1_hits.iter().all(|hit| {
                hit.hitbox_flags.rebound() && !hit.hitbox_flags.skip_if_thrown_hitbox_owner_absent()
            })
            && confirms
                .iter()
                .any(|confirm| confirm.attacker_index == 0 && confirm.victim_index == 1),
        "ftAction_8007121C decodes ThrowHi create_hitbox_3.rebound separately from spawn_hitbox_skip.xF_b4; the spawned hitbox should still confirm against fp->victim_gobj so Fighter_ProcessHit_8006D1EC can create linked hitlag, while the generic pre-b3 damage path remains suppressed. confirms={confirms:?} p1_hits={} p2_hurts={}",
        test_source_capsule_summary(&p1_hits),
        test_source_capsule_summary(&p2_hurts),
    );
    assert!(
        players[0].hitlag_frames > 0,
        "ftColl_80076ED8 writes thrower dmg.x1914 before the active-throw-victim damage gate"
    );
    assert!(
        players[0].source_x2219_b5 && players[1].source_x2219_b5,
        "Fighter_UnkRecursiveFunc_8006D044 propagates x2219_b5 through fp->x1A5C for held throw links"
    );
    let step = frame2312_step.expect("frame 2312 should be reached");
    assert_eq!(
        step.stages.len(),
        1,
        "ThrowHi frame-11 held-victim confirm should stage damage without entering generic damage; geometry={:?} raw={:?} logged={:?} confirms={:?} p1={:?} p2={:?}",
        step.geometry_confirms,
        step.raw_confirms,
        step.logged_confirms,
        step.confirms,
        players[0],
        players[1]
    );
    assert!(
        step.results.is_empty(),
        "held-victim throw hit before throw_flags_b3 should not calculate normal knockback"
    );
    assert!(
        (players[1].damage_percent - 15.369_999).abs() <= 0.000_01,
        "held-victim throw hit should commit the visible +4 percent before release; actual={:.6} temp={:.6} applied={}",
        players[1].damage_percent,
        players[1].damage_percent_temp,
        players[1].damage_applied
    );
    assert_eq!(players[1].damage_applied, 0);
}

#[test]
fn slippi_match_start_frame2662_p1_upair_hits_p2_body_not_guard_setoff() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let Some(export) = read_slippi_match_start_fixture_export() else {
        return;
    };
    let frames = slippi_visual_replay_inputs_from_match_start(&export, Some(2_860))
        .expect("replay fixture should parse");
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let mut frame2662_step = None;
    let mut p2_after_2661 = None;
    for frame in frames {
        let step =
            step_world_with_source_collision_step(&mut world, frame.core_frame, &frame.inputs);
        if frame.source_frame == 2661 {
            p2_after_2661 = Some(world.players()[1]);
        }
        if frame.source_frame == 2662 {
            frame2662_step = Some(step);
            break;
        }
    }

    let step = frame2662_step.expect("source frame 2662 should be present in the replay fixture");
    let p1_upair_p2_shield_confirms = step
        .logged_confirms
        .iter()
        .filter(|confirm| {
            confirm.attacker_index == 0
                && confirm.victim_index == 1
                && confirm.source_action_key == Some(SourceActionKey::new("AttackAirHi"))
                && confirm.hurtbox_id == SOURCE_SHIELD_HURTBOX_ID
        })
        .collect::<Vec<_>>();
    let p1_upair_p2_body_results = step
        .results
        .iter()
        .filter(|result| {
            result.stage.attacker_index == 0
                && result.stage.victim_index == 1
                && result.stage.source_action_key == Some(SourceActionKey::new("AttackAirHi"))
        })
        .collect::<Vec<_>>();

    assert!(
        !p1_upair_p2_body_results.is_empty(),
        "Slippi source frame 2662 has P1 AttackAirHi put P2 into DamageLw3, so shield contact must not suppress the body damage result. shield_confirms={p1_upair_p2_shield_confirms:?} body_results={p1_upair_p2_body_results:?} p2_after_2661={p2_after_2661:?} geometry={:?} raw={:?} logged={:?} confirms={:?} stages={:?} results={:?} p1={:?} p2={:?}",
        step.geometry_confirms,
        step.raw_confirms,
        step.logged_confirms,
        step.confirms,
        step.stages,
        step.results,
        world.players()[0],
        world.players()[1],
    );
    assert_eq!(
        world.players()[1].melee_action_state_id,
        Some(MeleeActionStateId::new(83))
    );
    assert!((world.players()[1].source_knockback_velocity_x - 1.495_804).abs() <= 0.000_01);
    assert!((world.players()[1].source_knockback_velocity_y - 1.444_481).abs() <= 0.000_01);
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
fn slippi_core_trace_max_frames_limits_rows_after_source_window_filtering() {
    let export = r#"{
      "source": {"replay_path": "fixture.slp"},
      "settings": {"players": {}},
      "frames": [
        {"frame": -18, "players": {"0": {
          "pre": {
            "rust_player_input": {
              "stick_x": 0, "stick_y": 0, "c_stick_x": 0, "c_stick_y": 0,
              "left_trigger": 0, "right_trigger": 0, "physical_button_bits": 0,
              "ucf_dashback_amendment": false
            }
          },
          "post": {
            "action_state_id": 14,
            "position": [0.0, 0.0],
            "self_induced_speeds": {"ground_x": 0.0, "air_x": 0.0, "y": 0.0}
          }
        }}},
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
            source_frame_end: -16,
            max_frames: Some(1),
        },
    )
    .expect("trace window should parse");

    assert_eq!(trace.rows.len(), 1);
    assert_eq!(trace.rows[0].source_frame, -17);
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
fn render_frame_and_debug_log_expose_rollback_owned_source_damage_fields() {
    let mut world = World::for_two_players();
    let hitbox = SourceHitboxAttributes {
        bone: 14,
        hit_group: 0,
        damage: 5,
        angle: 78,
        knockback_growth: 100,
        weight_set_knockback: 40,
        base_knockback: 0,
        element: 0,
        shield_damage: 0,
        hit_grounded: true,
        hit_aerial: true,
    };
    let stages = [SourceDamageStage {
        attacker_index: 0,
        victim_index: 1,
        hitbox_id: 1,
        hurtbox_id: 10,
        action_state_id: Some(MeleeActionStateId::new(65)),
        source_action_key: Some(SourceActionKey::new("AttackAirN")),
        source_frame: Some(7),
        damaged_hurt_height: 1,
        damage: 5.0,
        env_damage: 5,
        unk_count: 5,
        hitbox,
    }];
    assert_eq!(world.apply_source_damage_stages(&stages), 1);

    let render_frame = RenderFrame::from_world(&world);
    let scene = RenderScene::from_frame(&render_frame, 960, 540);
    let line = FrameDebugLog::from_frame_and_scene(
        &render_frame,
        &scene,
        [PlayerInput::neutral(), PlayerInput::neutral()],
    )
    .to_json_line();
    let parsed: serde_json::Value = serde_json::from_str(&line).expect("debug log must be json");

    assert_eq!(render_frame.player_damage_percents[1], 0.0);
    assert_eq!(render_frame.player_damage_percent_temps[1], 5.0);
    assert_eq!(render_frame.player_damage_applied[1], 5);
    assert_eq!(render_frame.player_profile_weights[1], 104.0);
    assert_eq!(parsed["players"][1]["damage_percent"], 0.0);
    assert_eq!(parsed["players"][1]["damage_percent_temp"], 5.0);
    assert_eq!(parsed["players"][1]["damage_applied"], 5);
    assert_eq!(parsed["players"][1]["profile_weight"], 104.0);
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
        render_frame.player_source_pose_frames[0], render_frame.player_animation_frames[0],
        "runtime must consume decomp fp->cur_anim_frame directly for source hitbox/script sampling"
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
            r: 247,
            g: 250,
            b: 252,
            a: 255
        }
    );
    assert_eq!(
        scene.stage,
        RenderRect {
            x: 299,
            y: 388,
            width: 362,
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
            x: 414,
            y: 328,
            width: 27,
            height: 60,
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
            x: 520,
            y: 328,
            width: 27,
            height: 60,
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
    assert_eq!(
        platform.width,
        scene
            .transform
            .core_length_to_screen(source_units_to_milli(
                FighterEntryPlatformProfile::COMMON_TROPHY_PLATFORM
                    .accessory
                    .mesh_bounds
                    .width_x(),
            ))
            .max(1)
    );
    assert!(platform.height >= 1);
    assert!(platform.y <= base_y);
    assert!(scene.entry_platforms[1].is_none());
}

#[test]
fn render_frame_exposes_core_match_phase_for_intro_ui() {
    let world = mole_runtime::default_play_world();
    let frame = RenderFrame::from_world(&world);
    let scene = RenderScene::from_frame(&frame, 960, 540);

    assert_eq!(frame.match_phase, MatchPhase::Ready);
    assert!(frame.match_phase_timer > 0);
    assert_eq!(scene.match_intro_label.as_deref(), Some("READY"));
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
fn render_entry_platform_uses_source_accessory_profile_not_player_rect_width() {
    let mut world = mole_runtime::default_play_world();
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    for frame in 0..6 {
        step_world(&mut world, Frame(frame), &neutral);
    }
    let mut render_frame = RenderFrame::from_world(&world);
    assert_eq!(
        render_frame.player_entry_platforms[0],
        FighterEntryPlatformProfile::COMMON_TROPHY_PLATFORM
    );

    let scene = RenderScene::from_frame(&render_frame, 960, 540);
    let platform = scene.entry_platforms[0].expect("EntryStart should expose source platform cue");
    let expected_width = scene
        .transform
        .core_length_to_screen(source_units_to_milli(
            FighterEntryPlatformProfile::COMMON_TROPHY_PLATFORM
                .accessory
                .mesh_bounds
                .width_x(),
        ))
        .max(1);
    assert_eq!(platform.width, expected_width);

    render_frame.player_positions[0].x += 20_000;
    let shifted = RenderScene::from_frame(&render_frame, 960, 540).entry_platforms[0]
        .expect("EntryStart should still expose source platform cue");
    assert_eq!(shifted.width, expected_width);
}

#[test]
fn render_entry_platform_exposes_source_bounds_wireframe() {
    let mut world = mole_runtime::default_play_world();
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    for frame in 0..6 {
        step_world(&mut world, Frame(frame), &neutral);
    }
    let frame = RenderFrame::from_world(&world);
    let scene = RenderScene::from_frame(&frame, 960, 540);

    let rect = scene.entry_platforms[0].expect("EntryStart should expose source platform cue");
    let wireframe = &scene.entry_platform_wireframes[0];

    assert_eq!(wireframe.len(), 4);
    let min_x = wireframe
        .iter()
        .flat_map(|line| [line.start.x, line.end.x])
        .min()
        .unwrap();
    let max_x = wireframe
        .iter()
        .flat_map(|line| [line.start.x, line.end.x])
        .max()
        .unwrap();
    assert_eq!(max_x - min_x, rect.width as i32);
    assert!(scene.entry_platform_wireframes[1].is_empty());
}

#[test]
fn render_respawn_wait_exposes_source_platform_cue() {
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let stage = world.stage();
    let mut player = world.players()[0];
    player.position.x = stage.blast_zones.right_x + 1_000;
    player.source_position = mole_core::SourceVec2::from_milli(player.position);
    assert!(world.set_player_state_for_diagnostic(0, player));

    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    step_world(&mut world, Frame(0), &neutral);
    step_world(&mut world, Frame(1), &neutral);
    for frame in 2..=300 {
        step_world(&mut world, Frame(frame), &neutral);
        if world.players()[0].motion_state == MotionState::RebirthWait {
            break;
        }
    }
    assert_eq!(world.players()[0].motion_state, MotionState::RebirthWait);

    let scene = RenderScene::from_frame(&RenderFrame::from_world(&world), 960, 540);

    assert!(scene.entry_platforms[0].is_some());
    assert_eq!(scene.entry_platform_wireframes[0].len(), 4);
}

#[test]
fn visual_platform_ledge_probe_shows_platform_reject_and_stage_ledge_grab() {
    let world = visual_platform_ledge_probe_world();
    let stage = world.stage();
    let platform = stage.soft_platforms[0];
    let left_ledge = stage.ledges[0];
    let platform_probe = world.players()[0];
    let stage_ledge_probe = world.players()[1];

    assert_eq!(platform_probe.motion_state, MotionState::Fall);
    assert_eq!(platform_probe.source_cliff_ledge_id, None);
    assert_eq!(platform_probe.position.x, platform.left_x);
    assert!(platform_probe.position.y < platform.y - 5_000);

    assert_eq!(stage_ledge_probe.motion_state, MotionState::CliffCatch);
    assert_eq!(
        stage_ledge_probe.melee_action_state_id,
        Some(MeleeActionStateId::new(252))
    );
    assert_eq!(
        stage_ledge_probe.source_cliff_ledge_id,
        Some(left_ledge.index)
    );
}

#[test]
fn render_transform_maps_core_units_to_screen_without_hidden_gameplay_scale() {
    let transform = RenderTransform::battlefield_camera(960, 540);
    let origin = transform.world_to_screen(Vec2 { x: 0, y: 0 });

    assert_eq!(origin.y, transform.ground_y);
    assert!(transform.pixels_per_core_unit_milli > 0);
}

#[test]
fn battlefield_render_transform_fits_extracted_decomp_camera_bounds() {
    let transform = RenderTransform::battlefield_camera(960, 540);
    let left_top = transform.world_to_screen(Vec2 {
        x: source_units_to_milli(-160.0),
        y: source_units_to_milli(136.0),
    });
    let right_bottom = transform.world_to_screen(Vec2 {
        x: source_units_to_milli(160.0),
        y: source_units_to_milli(-47.2),
    });

    assert!(left_top.x >= 0);
    assert!(left_top.y >= 0);
    assert!(right_bottom.x <= 960);
    assert!(right_bottom.y <= 540);
    assert!(right_bottom.x - left_top.x > 800);
    assert!(right_bottom.y - left_top.y > 450);
}

#[test]
fn dynamic_melee_camera_interest_tracks_fighter_camera_boxes() {
    let world = World::for_two_players();
    let mut frame = RenderFrame::from_world(&world);
    let mut camera = RenderCameraState::battlefield();
    let initial_scene = RenderScene::from_frame_with_camera(&frame, 960, 540, &mut camera);
    let initial_interest_x = camera.transform.interest_x;
    let initial_center_x = initial_scene.transform.center_x;

    frame.player_positions = [Vec2 { x: 80_000, y: 0 }, Vec2 { x: 120_000, y: 0 }];
    frame.player_ecbs = [
        EcbDiamond::from_bottom_center_and_size(frame.player_positions[0], 12_000, 28_000),
        EcbDiamond::from_bottom_center_and_size(frame.player_positions[1], 12_000, 28_000),
    ];

    let mut moved_scene = initial_scene;
    for _ in 0..30 {
        moved_scene = RenderScene::from_frame_with_camera(&frame, 960, 540, &mut camera);
    }

    assert!(camera.transform.interest_x > initial_interest_x + 10.0);
    assert!(moved_scene.transform.center_x < initial_center_x);
}

#[test]
fn dynamic_melee_camera_depth_tracks_fighter_spread() {
    let world = World::for_two_players();
    let mut frame = RenderFrame::from_world(&world);
    let mut camera = RenderCameraState::battlefield();
    let _ = RenderScene::from_frame_with_camera(&frame, 960, 540, &mut camera);
    let initial_target_z = camera.transform.target_position_z;

    frame.player_positions = [Vec2 { x: -150_000, y: 0 }, Vec2 { x: 150_000, y: 0 }];
    frame.player_ecbs = [
        EcbDiamond::from_bottom_center_and_size(frame.player_positions[0], 12_000, 28_000),
        EcbDiamond::from_bottom_center_and_size(frame.player_positions[1], 12_000, 28_000),
    ];

    let _ = RenderScene::from_frame_with_camera(&frame, 960, 540, &mut camera);

    assert!(camera.transform.target_position_z > initial_target_z);
}

#[test]
fn dynamic_melee_camera_subjects_use_fighter_profile_camera_boxes() {
    let base_world = World::for_two_players();
    let mut base_frame = RenderFrame::from_world(&base_world);
    base_frame.player_positions = [Vec2 { x: 0, y: 0 }, Vec2 { x: 20_000, y: 0 }];
    base_frame.player_ecbs = [
        EcbDiamond::from_bottom_center_and_size(base_frame.player_positions[0], 4_000, 8_000),
        EcbDiamond::from_bottom_center_and_size(base_frame.player_positions[1], 4_000, 8_000),
    ];
    let mut base_camera = RenderCameraState::battlefield();
    let _ = RenderScene::from_frame_with_camera(&base_frame, 960, 540, &mut base_camera);

    let wide_box = mole_core::FighterCameraBox {
        x0: SourceVec3 {
            x: 10.0,
            y: 80.0,
            z: -80.0,
        },
        xc: SourceVec3 {
            x: 80.0,
            y: -80.0,
            z: 13.699999809265137,
        },
    };
    let wide_profile = FighterProfile::FALCON_LIKE.from_ftdata_x3c_camera_box(wide_box);
    let wide_world = World::for_two_players_with_profiles([wide_profile; 2]);
    let mut wide_frame = RenderFrame::from_world(&wide_world);
    wide_frame.player_positions = base_frame.player_positions;
    wide_frame.player_ecbs = base_frame.player_ecbs;
    let mut wide_camera = RenderCameraState::battlefield();
    let _ = RenderScene::from_frame_with_camera(&wide_frame, 960, 540, &mut wide_camera);

    assert!(
        wide_camera.transform.target_position_z > base_camera.transform.target_position_z,
        "Camera_8002958C should derive subject spread from ftData.x3C camera boxes, not active ECB extents"
    );
}

#[test]
fn dynamic_melee_camera_uses_decomp_gameplay_fov_not_fixed_stage_fov() {
    let world = World::for_two_players();
    let frame = RenderFrame::from_world(&world);
    let mut camera = RenderCameraState::battlefield();

    let _ = RenderScene::from_frame_with_camera(&frame, 960, 540, &mut camera);

    assert_eq!(
        camera.transform.target_fov_degrees.to_bits(),
        38.0_f32.to_bits()
    );
    assert!(
        camera.transform.fov_degrees > 30.0,
        "Camera_8002B3D4 lerps the 30 degree startup FOV toward cm_803BCCA0.x40=38 during gameplay"
    );
}

#[test]
fn render_scene_contains_battlefield_surfaces_and_decomp_ordered_ecb() {
    let world = World::for_two_players();
    let frame = RenderFrame::from_world(&world);
    let scene = RenderScene::from_frame(&frame, 960, 540);

    assert_eq!(scene.stage_surfaces.len(), 4);
    assert_eq!(scene.stage_collision_lines.len(), 23);
    assert_eq!(scene.player_ecbs[0].points.len(), 4);
    assert_eq!(
        scene.player_ecbs[0].points[0].x,
        scene.player_ecbs[0].points[2].x
    );
}

#[test]
fn render_scene_can_use_dev_flat_stage_without_battlefield_surfaces() {
    let stage = StageProfile::dev_flat_test();
    let world = World::for_two_players_on_stage(stage);
    let frame = RenderFrame::from_world(&world);
    let scene = RenderScene::from_frame(&frame, 640, 360);

    assert_eq!(scene.background, RenderColor::DEV_BACKGROUND);
    assert_eq!(scene.stage_surfaces.len(), 1);
    assert_eq!(scene.stage_collision_lines.len(), 0);
    assert_eq!(scene.stage.width, 480);
    assert!(scene.players[0].height < 320);
    assert!(scene.players[0].y >= 0);
    assert!(scene.players[0].y + scene.players[0].height as i32 <= 360);
    assert_eq!(scene.player_ecbs[0].points.len(), 4);
}

#[test]
fn runtime_asset_root_contains_sprite_files() {
    let asset_root = project_asset_root();

    assert!(asset_root
        .join("DolphinMole")
        .join("standing")
        .join("Standing1.png")
        .is_file());
}

#[test]
fn packaged_asset_root_detects_extracted_playtest_assets_next_to_runtime_exe() {
    let root = std::env::temp_dir().join(format!(
        "mole-runtime-packaged-asset-root-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos()
    ));
    let package = root.join("package");
    fs::create_dir_all(package.join("DolphinMole").join("standing")).unwrap();

    assert_eq!(
        packaged_asset_root_for_exe(&package.join("mole_runtime.exe")),
        Some(package)
    );
}

#[test]
fn render_scene_references_background_and_sprite_asset_paths() {
    let world = World::for_two_players();
    let frame = RenderFrame::from_world(&world);
    let scene = RenderScene::from_frame(&frame, 960, 540);

    assert!(scene.background_image.is_none());
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
    let source_shield_position = frame.player_source_shield_hit_positions[0];
    let expected_center = scene.transform.world_to_screen(Vec2 {
        x: mole_core::source_units_to_milli(source_shield_position.x),
        y: mole_core::source_units_to_milli(source_shield_position.y),
    });

    assert!(scene.player_shields[1].is_none());
    assert_eq!(shield.center, expected_center);
    assert!(shield.radius > 0);
    assert_eq!(shield.color.a, 96);
}

#[test]
fn render_scene_draws_source_shield_from_decomp_center_and_scale() {
    let common = MeleeCommonData {
        shield_start_health: 60.0,
        shield_size_health_scale: 0.15,
        shield_size_light_min: 1.0,
        shield_size_light_max: 0.5,
        ..MeleeCommonData::PROVISIONAL
    };
    let mut world = World::for_two_players_with_common_data(common);
    let mut player = world.players()[0];
    player.profile = FighterProfile {
        reference_character: "captain_falcon",
        initial_shield_size: 20.0,
        model_scaling: 0.5,
        ..FighterProfile::FALCON_LIKE
    };
    player.grounded = true;
    player.shield_health = 30.0;
    player.lightshield_amount = 0.5;
    player.source_shield_collision_active = true;
    player.source_shield_hit_active = true;
    player.source_shield_hit_position = SourcePosePoint {
        x: 12.0,
        y: 34.0,
        z: 0.0,
    };
    assert!(world.set_player_state_for_diagnostic(0, player));

    let frame = RenderFrame::from_world(&world);
    let scene = RenderScene::from_frame(&frame, 960, 540);
    let shield = scene.player_shields[0].expect("active source shield should render a bubble");
    let expected_center = scene.transform.world_to_screen(Vec2 {
        x: mole_core::source_units_to_milli(12.0),
        y: mole_core::source_units_to_milli(34.0),
    });
    let health_frac = 30.0_f32 / 60.0_f32;
    let light_scale = 0.5_f32 * (common.shield_size_light_max - common.shield_size_light_min)
        + common.shield_size_light_min;
    let expected_size = ((1.0 - common.shield_size_health_scale) * (health_frac * light_scale)
        + common.shield_size_health_scale)
        * 20.0
        * 0.5;
    let expected_radius = scene
        .transform
        .core_length_to_screen(mole_core::source_units_to_milli(expected_size));

    assert_eq!(
        shield.center, expected_center,
        "ftdrawcommon gates shield drawing through shield_hit, so the visible bubble should follow the source shield joint instead of the sprite rectangle"
    );
    assert_eq!(
        shield.radius, expected_radius,
        "ftCo inlineB0 scales non-Yoshi shields by health/lightshield, then the shield JObj matrix contributes fighter model scale before rendering"
    );
}

#[test]
fn render_scene_exposes_attack_air_n_source_hitbox_and_hurtbox_pills() {
    let world = World::for_two_players();
    let mut frame = RenderFrame::from_world(&world);
    frame.player_motion_states[0] = MotionState::AttackAirN;
    frame.player_state_frames[0] = 6;
    frame.player_animation_frames[0] = 7;
    frame.player_facings[0] = 1;
    frame.player_source_pose_motion_states[0] = MotionState::AttackAirN;
    frame.player_source_pose_action_state_ids[0] = Some(MeleeActionStateId::new(65));
    frame.player_source_action_keys[0] = Some(SourceActionKey::new("AttackAirN"));
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
        "runtime_source_frame_data"
    );
    assert_eq!(scene.player_hurtbox_pills[0][0].source_space, "melee_xyz");
    assert_eq!(
        scene.player_hurtbox_pills[0][0].projected_view_kind,
        "derived_debug_view"
    );
    assert_eq!(
        scene.player_hurtbox_pills[0][0].source_artifact_kind,
        "runtime_source_frame_data"
    );
    assert_ne!(scene.player_hitbox_pills[0][0].source.a.z, 0.0);
    assert_ne!(scene.player_hurtbox_pills[0][0].source.a.z, 0.0);
    assert_ne!(
        scene.player_hurtbox_pills[0][0].a,
        scene.player_hurtbox_pills[0][0].b
    );
}

#[test]
fn render_scene_keeps_cliff_states_visible_with_source_wireframes() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");

    for motion_state in [MotionState::CliffCatch, MotionState::CliffWait] {
        let mut world = World::for_two_players_on_stage(StageProfile::battlefield());
        let ledge = world.stage().ledges[0];
        let mut player = world.players()[0];
        player.set_motion_state_alias(motion_state);
        player.grounded = false;
        player.source_cliff_ledge_id = Some(ledge.index);
        player.position = Vec2 {
            x: ledge.x_milli + player.profile.ledge_snap_x_milli,
            y: ledge.y_milli + player.profile.ledge_snap_y_milli,
        };
        player.source_position = mole_core::SourceVec2::from_milli(player.position);
        assert!(world.set_player_state_for_diagnostic(0, player));

        let frame = RenderFrame::from_world(&world);
        let scene = RenderScene::from_frame(&frame, 960, 540);

        assert_eq!(scene.player_ecbs[0].color, RenderColor::ECB);
        assert!(
            !scene.player_hurtbox_pills[0].is_empty(),
            "{motion_state:?} should render baked source hurtbox wireframes"
        );
    }
}

#[test]
fn render_scene_keeps_shieldbreakfly_body_visible_after_shield_object_clears() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let common = MeleeCommonData {
        shield_break_reset_health: 30.0,
        shield_hold_drain: 2.0,
        shield_hold_lightshield_min: 1.0,
        shield_hold_lightshield_max: 1.0,
        ..MeleeCommonData::PROVISIONAL
    };
    let mut world = World::for_two_players_with_common_data(common);
    let mut player = world.players()[0];
    player.grounded = true;
    player.set_motion_state_alias(MotionState::Guard);
    player.motion_frame = 4;
    player.set_source_motion_anim_frame(10.0);
    player.shield_health = 1.0;
    player.lightshield_amount = 0.0;
    player.source_shield_collision_active = true;
    player.source_shield_hit_active = true;
    player.source_shield_hit_update_pos = true;
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(0),
        &[
            PlayerInput::neutral().with_left_trigger_digital(true),
            PlayerInput::neutral(),
        ],
    );

    let frame = RenderFrame::from_world(&world);
    assert_eq!(frame.player_motion_states[0], MotionState::ShieldBreakFly);
    assert_eq!(
        frame.player_source_pose_action_keys[0],
        Some(SourceActionKey::new("Guard")),
        "ShieldBreakFly should retain the previous model pose for source body capsules without reinstalling the shield object"
    );

    let scene = RenderScene::from_frame(&frame, 960, 540);
    assert!(scene.player_shields[0].is_none());
    assert!(
        !scene.player_hurtbox_pills[0].is_empty(),
        "ftCo_80098B20 clears the shield object, but Fighter_ChangeMotionState no-animation handling must not erase the fighter body model/hurt capsules"
    );

    let collision = source_collision_frame_from_frame(&frame);
    assert!(
        collision
            .hurts
            .iter()
            .any(|hurt| hurt.owner_index == 0 && hurt.capsule_id != SOURCE_SHIELD_HURTBOX_ID),
        "gameplay source collision should keep body hurt capsules visible during ShieldBreakFly"
    );
}

#[test]
fn render_scene_samples_source_capsules_by_canonical_action_identity() {
    let world = World::for_two_players();
    let mut frame = RenderFrame::from_world(&world);
    frame.player_motion_states[0] = MotionState::Wait;
    frame.player_source_pose_motion_states[0] = MotionState::Wait;
    frame.player_source_pose_action_state_ids[0] = Some(MeleeActionStateId::new(65));
    frame.player_source_action_keys[0] = Some(SourceActionKey::new("AttackAirN"));
    frame.player_state_frames[0] = 6;
    frame.player_animation_frames[0] = 7;
    frame.player_facings[0] = 1;
    frame.player_source_pose_frames[0] = 7;
    frame.player_source_pose_model_facings[0] = 1;

    let scene = RenderScene::from_frame(&frame, 960, 540);

    assert_eq!(scene.player_hitbox_pills[0].len(), 3);
    assert_eq!(scene.player_hurtbox_pills[0].len(), 11);
}

#[test]
fn runtime_source_collision_uses_canonical_action_identity_not_motion_alias() {
    let world = World::for_two_players();
    let mut frame = RenderFrame::from_world(&world);
    frame.player_positions[1] = frame.player_positions[0];
    frame.player_source_positions[1] = frame.player_source_positions[0];
    frame.player_source_previous_positions[1] = frame.player_source_positions[1];
    frame.player_motion_states[0] = MotionState::Wait;
    frame.player_source_pose_motion_states[0] = MotionState::Wait;
    frame.player_source_pose_action_state_ids[0] = Some(MeleeActionStateId::new(65));
    frame.player_source_action_keys[0] = Some(SourceActionKey::new("AttackAirN"));
    frame.player_animation_frames[0] = 7;
    frame.player_source_pose_frames[0] = 7;
    frame.player_source_pose_model_facings[0] = 1;

    let collisions = source_collision_hits_from_frame(&frame);

    assert!(
        collisions
            .iter()
            .any(|collision| collision.hit.owner_index == 0 && collision.hurt.owner_index == 1),
        "P1 AttackAirN source hitboxes should hit P2 hurtboxes even when P1's visible MotionState alias is Wait"
    );
    assert!(
        collisions
            .iter()
            .all(|collision| collision.hit.owner_index != collision.hurt.owner_index),
        "source collision must not report self hits"
    );
}

#[test]
fn runtime_source_collision_keeps_catch_hitboxes_on_ftcoll_80078a2c_frame() {
    let world = World::for_two_players();
    let mut catch_frame = RenderFrame::from_world(&world);
    catch_frame.player_source_pose_action_state_ids[0] = Some(MeleeActionStateId::new(212));
    catch_frame.player_source_action_keys[0] = Some(SourceActionKey::new("Catch"));
    catch_frame.player_source_pose_frames[0] = 7;
    catch_frame.player_source_motion_anim_frames[0] = 7.0;

    let catch_collision = source_collision_frame_from_frame(&catch_frame);
    let catch_hits = catch_collision
        .hits
        .iter()
        .filter(|hit| {
            hit.owner_index == 0
                && hit
                    .hitbox
                    .is_some_and(|hitbox| hitbox.element == SOURCE_HIT_ELEMENT_CATCH)
        })
        .collect::<Vec<_>>();

    assert!(
        !catch_hits.is_empty(),
        "Catch source frame 7 should expose ftColl_80078A2C catch hitboxes"
    );
    assert!(
        catch_hits.iter().all(|hit| hit.source_frame == Some(7)),
        "ftColl_80078A2C processes active HitElement_Catch capsules from the script/current frame, not the normal damage live-pose frame"
    );

    let mut damage_frame = RenderFrame::from_world(&world);
    damage_frame.player_source_pose_action_state_ids[0] = Some(MeleeActionStateId::new(65));
    damage_frame.player_source_action_keys[0] = Some(SourceActionKey::new("AttackAirN"));
    damage_frame.player_source_pose_frames[0] = 7;
    damage_frame.player_source_motion_anim_frames[0] = 7.0;

    let damage_collision = source_collision_frame_from_frame(&damage_frame);
    assert!(
        damage_collision.hits.iter().any(|hit| {
            hit.owner_index == 0
                && hit
                    .hitbox
                    .is_some_and(|hitbox| hitbox.element != SOURCE_HIT_ELEMENT_CATCH)
                && hit.source_frame == Some(8)
        }),
        "ftColl_80078C70 normal damage hitboxes should still use the post-animation live pose frame"
    );
}

#[test]
fn runtime_source_collision_does_not_spawn_damage_hitboxes_from_live_pose_frame() {
    let world = World::for_two_players();
    let mut frame = RenderFrame::from_world(&world);
    frame.player_source_pose_action_state_ids[0] = Some(MeleeActionStateId::new(65));
    frame.player_source_action_keys[0] = Some(SourceActionKey::new("AttackAirN"));
    frame.player_source_pose_frames[0] = 6;
    frame.player_source_motion_anim_frames[0] = 6.0;

    let collision_frame = source_collision_frame_from_frame(&frame);

    assert!(
        collision_frame
            .hits
            .iter()
            .filter(|hit| hit.owner_index == 0)
            .all(|hit| hit
                .hitbox
                .is_none_or(|hitbox| hitbox.element == SOURCE_HIT_ELEMENT_CATCH)),
        "ftAction_8007121C owns hitbox activation; ftColl_8007AD18 may sample live JObj endpoints, but AttackAirN script frame 6 has no normal damage hitboxes"
    );
}

#[test]
fn runtime_source_hit_confirms_carry_decomp_hitbox_attributes() {
    let world = World::for_two_players();
    let mut frame = RenderFrame::from_world(&world);
    frame.player_positions[1] = frame.player_positions[0];
    frame.player_source_positions[1] = frame.player_source_positions[0];
    frame.player_source_previous_positions[1] = frame.player_source_positions[1];
    frame.player_motion_states[0] = MotionState::Wait;
    frame.player_source_pose_motion_states[0] = MotionState::Wait;
    frame.player_source_pose_action_state_ids[0] = Some(MeleeActionStateId::new(65));
    frame.player_source_action_keys[0] = Some(SourceActionKey::new("AttackAirN"));
    frame.player_animation_frames[0] = 7;
    frame.player_source_pose_frames[0] = 7;
    frame.player_source_pose_model_facings[0] = 1;
    frame.player_grounded[1] = false;

    let confirms = source_hit_confirms_from_frame(&frame);

    let hitbox_one = confirms
        .iter()
        .find(|confirm| confirm.attacker_index == 0 && confirm.hitbox_id == 1)
        .expect("AttackAirN source frame 7 should confirm with hitbox id 1");
    assert_eq!(hitbox_one.victim_index, 1);
    assert_eq!(
        hitbox_one.action_state_id,
        Some(MeleeActionStateId::new(65))
    );
    assert_eq!(
        hitbox_one.source_action_key,
        Some(SourceActionKey::new("AttackAirN"))
    );
    assert_eq!(hitbox_one.source_frame, Some(7));
    assert_eq!(hitbox_one.hitbox.damage, 5);
    assert_eq!(hitbox_one.hitbox.angle, 78);
    assert_eq!(hitbox_one.hitbox.knockback_growth, 100);
    assert_eq!(hitbox_one.hitbox.weight_set_knockback, 40);
    assert_eq!(hitbox_one.hitbox.base_knockback, 0);
    assert_eq!(hitbox_one.hitbox.element, 0);
    assert_eq!(hitbox_one.hitbox.shield_damage, 0);
    assert!(hitbox_one.hitbox.hit_grounded);
    assert!(hitbox_one.hitbox.hit_aerial);
}

#[test]
fn runtime_source_damage_stages_carry_decomp_damage_stage_fields() {
    let world = World::for_two_players();
    let mut frame = RenderFrame::from_world(&world);
    assert_eq!(frame.common_data, world.common_data());
    assert_eq!(frame.player_profile_weights, [104.0, 104.0]);

    frame.player_positions[1] = frame.player_positions[0];
    frame.player_source_positions[1] = frame.player_source_positions[0];
    frame.player_source_previous_positions[1] = frame.player_source_positions[1];
    frame.player_motion_states[0] = MotionState::Wait;
    frame.player_source_pose_motion_states[0] = MotionState::Wait;
    frame.player_source_pose_action_state_ids[0] = Some(MeleeActionStateId::new(65));
    frame.player_source_action_keys[0] = Some(SourceActionKey::new("AttackAirN"));
    frame.player_animation_frames[0] = 7;
    frame.player_source_pose_frames[0] = 7;
    frame.player_source_pose_model_facings[0] = 1;
    frame.player_grounded[1] = false;

    let stages = source_damage_stages_from_frame(&frame);

    let hitbox_one = stages
        .iter()
        .find(|stage| stage.attacker_index == 0 && stage.hitbox_id == 1)
        .expect("AttackAirN source frame 7 should stage damage for hitbox id 1");
    assert_eq!(hitbox_one.victim_index, 1);
    assert_eq!(hitbox_one.damage, 5.0);
    assert_eq!(hitbox_one.env_damage, 5);
    assert_eq!(
        hitbox_one.action_state_id,
        Some(MeleeActionStateId::new(65))
    );
    assert_eq!(
        hitbox_one.source_action_key,
        Some(SourceActionKey::new("AttackAirN"))
    );
    assert_eq!(hitbox_one.source_frame, Some(7));
}

#[test]
fn runtime_source_damage_results_use_frame_weight_for_knockback_selection() {
    let world = World::for_two_players();
    let mut frame = RenderFrame::from_world(&world);
    frame.player_positions[1] = frame.player_positions[0];
    frame.player_source_positions[1] = frame.player_source_positions[0];
    frame.player_source_previous_positions[1] = frame.player_source_positions[1];
    frame.player_motion_states[0] = MotionState::Wait;
    frame.player_source_pose_motion_states[0] = MotionState::Wait;
    frame.player_source_pose_action_state_ids[0] = Some(MeleeActionStateId::new(65));
    frame.player_source_action_keys[0] = Some(SourceActionKey::new("AttackAirN"));
    frame.player_animation_frames[0] = 7;
    frame.player_source_pose_frames[0] = 7;
    frame.player_source_pose_model_facings[0] = 1;
    frame.player_grounded[1] = false;

    let mut light_frame = frame;
    light_frame.player_profile_weights[1] = 50.0;
    let mut heavy_frame = frame;
    heavy_frame.player_profile_weights[1] = 200.0;

    let light_results = source_damage_results_from_frame(&light_frame);
    let heavy_results = source_damage_results_from_frame(&heavy_frame);

    let light_result = light_results
        .iter()
        .find(|result| result.stage.victim_index == 1)
        .expect("overlapping AttackAirN should emit a selected damage result for player two");
    let heavy_result = heavy_results
        .iter()
        .find(|result| result.stage.victim_index == 1)
        .expect("overlapping AttackAirN should emit a selected damage result for player two");

    assert_eq!(light_result.stage.attacker_index, 0);
    assert_eq!(
        light_result.stage.unk_count,
        light_result.stage.hitbox.damage as u16
    );
    assert_eq!(light_result.angle, light_result.stage.hitbox.angle);
    assert_eq!(light_result.element, light_result.stage.hitbox.element);
    assert!(
        light_result.knockback > heavy_result.knockback,
        "Melee knockback decay should produce less knockback for the heavier target"
    );
}

#[test]
fn runtime_source_damage_results_from_stages_include_current_frame_percent_temp_like_ftcoll() {
    let world = World::for_two_players();
    let frame = RenderFrame::from_world(&world);
    let hitbox = SourceHitboxAttributes {
        bone: 14,
        hit_group: 0,
        damage: 10,
        angle: 78,
        knockback_growth: 100,
        weight_set_knockback: 0,
        base_knockback: 0,
        element: 0,
        shield_damage: 0,
        hit_grounded: true,
        hit_aerial: true,
    };
    let stages = [SourceDamageStage {
        attacker_index: 0,
        victim_index: 1,
        hitbox_id: 1,
        hurtbox_id: 10,
        action_state_id: Some(MeleeActionStateId::new(65)),
        source_action_key: Some(SourceActionKey::new("AttackAirN")),
        source_frame: Some(7),
        damaged_hurt_height: 1,
        damage: 10.0,
        env_damage: 10,
        unk_count: 10,
        hitbox,
    }];

    let result = source_damage_results_from_stages_for_frame(&frame, &stages)
        .into_iter()
        .find(|result| result.stage.victim_index == 1)
        .expect("staged normal-path hit should produce a selected damage result");
    let with_current_stage = source_damage_result_for_victim(
        frame.common_data,
        &stages,
        SourceDamageResultInput {
            victim_index: 1,
            victim_percent: frame.player_damage_percents[1],
            victim_percent_temp: 10.0,
            victim_weight: frame.player_profile_weights[1],
            stage: 1.0,
            attack: 1.0,
            defense: 1.0,
        },
    )
    .expect("same stage should produce an accumulated comparison result");

    assert_eq!(
        result.knockback.to_bits(),
        with_current_stage.knockback.to_bits()
    );
}

#[test]
fn runtime_source_collision_step_applies_render_frame_collisions_to_core_world() {
    let mut world = World::for_two_players();
    let mut frame = RenderFrame::from_world(&world);
    frame.player_positions[1] = frame.player_positions[0];
    frame.player_source_positions[1] = frame.player_source_positions[0];
    frame.player_source_previous_positions[1] = frame.player_source_positions[1];
    frame.player_motion_states[0] = MotionState::Wait;
    frame.player_source_pose_motion_states[0] = MotionState::Wait;
    frame.player_source_pose_action_state_ids[0] = Some(MeleeActionStateId::new(65));
    frame.player_source_action_keys[0] = Some(SourceActionKey::new("AttackAirN"));
    frame.player_animation_frames[0] = 7;
    frame.player_source_pose_frames[0] = 7;
    frame.player_source_pose_model_facings[0] = 1;
    frame.player_grounded[1] = false;

    let step = source_collision_step_from_frame(&mut world, &frame);

    assert!(!step.confirms.is_empty());
    assert!(!step.stages.is_empty());
    assert!(!step.results.is_empty());
    assert_eq!(step.applied_stage_count, step.stages.len());
    assert_eq!(world.players()[1].damage_percent, 0.0);
    assert!(world.players()[1].damage_percent_temp > 0.0);
    assert!(world.players()[1].damage_applied > 0);
}

#[test]
fn runtime_replay_nair_frame2253_source_collision_hits_dashing_victim() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");

    let mut world = World::for_two_players_on_stage(StageProfile::battlefield());
    let mut attacker = world.players()[0];
    attacker.position = Vec2 {
        x: source_units_to_milli(8.228595733642578),
        y: source_units_to_milli(11.560099601745605),
    };
    attacker.source_position = SourceVec2 {
        x: 8.228595733642578,
        y: 11.560099601745605,
    };
    attacker.velocity = Vec2 { x: 1055, y: 990 };
    attacker.source_self_velocity_x = 1.0554375648498535;
    attacker.source_self_velocity_y = 0.9900000095367432;
    attacker.grounded = false;
    attacker.facing = 1;
    attacker.set_motion_state_alias(MotionState::AttackAirN);
    attacker.motion_frame = 6;
    attacker.set_source_motion_anim_frame(7.0);
    assert!(world.set_player_state_for_diagnostic(0, attacker));

    let mut victim = world.players()[1];
    victim.position = Vec2 {
        x: source_units_to_milli(12.735595703125),
        y: source_units_to_milli(0.00009999999747378752),
    };
    victim.source_position = SourceVec2 {
        x: 12.735595703125,
        y: 0.00009999999747378752,
    };
    victim.velocity = Vec2 { x: 2271, y: 0 };
    victim.ground_velocity_x = 2.271250009536743;
    victim.source_self_velocity_x = 2.271250009536743;
    victim.source_self_velocity_y = 0.0;
    victim.grounded = true;
    victim.facing = 1;
    victim.set_motion_state_alias(MotionState::Dash);
    victim.motion_frame = 2;
    victim.set_source_motion_anim_frame(2.0);
    assert!(world.set_player_state_for_diagnostic(1, victim));

    let mut frame = RenderFrame::from_world(&world);
    frame.player_source_previous_positions[1] = SourceVec2 {
        x: 10.464345932006836,
        y: 0.00009999999747378752,
    };
    let collision_frame = source_collision_frame_from_frame(&frame);
    let source_hits = source_collision_hits_from_frame(&frame);
    let confirms = source_hit_confirms_from_frame(&frame);
    let damage_results = source_damage_results_from_frame(&frame);
    let mut previous_victim_position_frame = frame;
    previous_victim_position_frame.player_positions[1] = Vec2 {
        x: source_units_to_milli(10.464345932006836),
        y: source_units_to_milli(0.00009999999747378752),
    };
    previous_victim_position_frame.player_source_positions[1] = SourceVec2 {
        x: 10.464345932006836,
        y: 0.00009999999747378752,
    };
    previous_victim_position_frame.player_source_pose_frames[1] = 2;
    let previous_victim_position_confirms =
        source_hit_confirms_from_frame(&previous_victim_position_frame).len();
    let mut current_victim_pose2_frame = frame;
    current_victim_pose2_frame.player_source_pose_frames[1] = 2;
    let current_victim_pose2_confirms =
        source_hit_confirms_from_frame(&current_victim_pose2_frame).len();
    let mut current_victim_pose1_frame = frame;
    current_victim_pose1_frame.player_source_pose_frames[1] = 1;
    let current_victim_pose1_confirms =
        source_hit_confirms_from_frame(&current_victim_pose1_frame).len();
    let mut previous_victim_root_pose2_frame = frame;
    previous_victim_root_pose2_frame.player_source_previous_positions[1] = SourceVec2 {
        x: 10.464345932006836,
        y: 0.00009999999747378752,
    };
    previous_victim_root_pose2_frame.player_source_pose_frames[1] = 2;
    let previous_victim_root_pose2_confirms =
        source_hit_confirms_from_frame(&previous_victim_root_pose2_frame).len();
    let mut previous_victim_root_pose1_frame = previous_victim_root_pose2_frame;
    previous_victim_root_pose1_frame.player_source_pose_frames[1] = 1;
    let previous_victim_root_pose1_confirms =
        source_hit_confirms_from_frame(&previous_victim_root_pose1_frame).len();
    let mut previous_attacker_pose_frame = frame;
    previous_attacker_pose_frame.player_positions[0] = Vec2 {
        x: source_units_to_milli(7.173157691955566),
        y: source_units_to_milli(10.570099830627441),
    };
    previous_attacker_pose_frame.player_source_positions[0] = SourceVec2 {
        x: 7.173157691955566,
        y: 10.570099830627441,
    };
    previous_attacker_pose_frame.player_source_pose_frames[0] = 6;
    let previous_attacker_pose_confirms =
        source_hit_confirms_from_frame(&previous_attacker_pose_frame).len();
    let mut previous_both_frame = previous_victim_position_frame;
    previous_both_frame.player_positions[0] = Vec2 {
        x: source_units_to_milli(7.173157691955566),
        y: source_units_to_milli(10.570099830627441),
    };
    previous_both_frame.player_source_positions[0] = SourceVec2 {
        x: 7.173157691955566,
        y: 10.570099830627441,
    };
    previous_both_frame.player_source_pose_frames[0] = 6;
    let previous_both_confirms = source_hit_confirms_from_frame(&previous_both_frame).len();
    let mut next_attacker_pose_frame = frame;
    next_attacker_pose_frame.player_source_pose_frames[0] = 8;
    let next_attacker_pose_confirms =
        source_hit_confirms_from_frame(&next_attacker_pose_frame).len();
    let confirm_summary = |frame: &RenderFrame| {
        source_hit_confirms_from_frame(frame)
            .into_iter()
            .map(|confirm| {
                (
                    confirm.hitbox_id,
                    confirm.hurtbox_id,
                    confirm.hitbox.damage,
                    confirm.hitbox.angle,
                )
            })
            .collect::<Vec<_>>()
    };
    let current_variant_confirms = confirm_summary(&frame);
    let previous_victim_position_variant_confirms =
        confirm_summary(&previous_victim_position_frame);
    let current_victim_pose2_variant_confirms = confirm_summary(&current_victim_pose2_frame);
    let current_victim_pose1_variant_confirms = confirm_summary(&current_victim_pose1_frame);
    let previous_victim_root_pose2_variant_confirms =
        confirm_summary(&previous_victim_root_pose2_frame);
    let previous_victim_root_pose1_variant_confirms =
        confirm_summary(&previous_victim_root_pose1_frame);
    let previous_attacker_pose_variant_confirms = confirm_summary(&previous_attacker_pose_frame);
    let previous_both_variant_confirms = confirm_summary(&previous_both_frame);
    let next_attacker_pose_variant_confirms = confirm_summary(&next_attacker_pose_frame);
    let p1_hits: Vec<_> = collision_frame
        .hits
        .iter()
        .filter(|capsule| capsule.owner_index == 0)
        .map(|capsule| {
            (
                capsule.capsule_id,
                capsule.source_frame,
                capsule.capsule.a,
                capsule.capsule.b,
                capsule.capsule.radius,
            )
        })
        .collect();
    let p2_hurts: Vec<_> = collision_frame
        .hurts
        .iter()
        .filter(|capsule| capsule.owner_index == 1)
        .map(|capsule| {
            (
                capsule.capsule_id,
                capsule.source_frame,
                capsule.capsule.a,
                capsule.capsule.b,
                capsule.capsule.radius,
            )
        })
        .collect();
    let p2_hurts_use_live_dash_pose_boundary = !p2_hurts.is_empty()
        && p2_hurts
            .iter()
            .all(|(_, source_frame, _, _, _)| *source_frame == Some(3));
    let nearest_center_gap = collision_frame
        .hits
        .iter()
        .filter(|capsule| capsule.owner_index == 0)
        .flat_map(|hit| {
            collision_frame
                .hurts
                .iter()
                .filter(|hurt| hurt.owner_index == 1)
                .map(move |hurt| {
                    let hit_center = mole_core::collision::Vec3::new(
                        (hit.capsule.a.x + hit.capsule.b.x) * 0.5,
                        (hit.capsule.a.y + hit.capsule.b.y) * 0.5,
                        (hit.capsule.a.z + hit.capsule.b.z) * 0.5,
                    );
                    let hurt_center = mole_core::collision::Vec3::new(
                        (hurt.capsule.a.x + hurt.capsule.b.x) * 0.5,
                        (hurt.capsule.a.y + hurt.capsule.b.y) * 0.5,
                        (hurt.capsule.a.z + hurt.capsule.b.z) * 0.5,
                    );
                    let dx = hit_center.x - hurt_center.x;
                    let dy = hit_center.y - hurt_center.y;
                    let dz = hit_center.z - hurt_center.z;
                    (dx * dx + dy * dy + dz * dz).sqrt() - hit.capsule.radius - hurt.capsule.radius
                })
        })
        .min_by(|a, b| a.total_cmp(b));
    let confirm_debug: Vec<_> = confirms
        .iter()
        .map(|confirm| {
            (
                confirm.hitbox_id,
                confirm.hurtbox_id,
                confirm.hitbox.damage,
                confirm.hitbox.angle,
                confirm.hitbox.knockback_growth,
                confirm.hitbox.weight_set_knockback,
                confirm.hitbox.base_knockback,
                confirm.damaged_hurt_height,
            )
        })
        .collect();
    let selected_result = damage_results
        .iter()
        .find(|result| result.stage.attacker_index == 0 && result.stage.victim_index == 1)
        .expect("frame 2253 should select a source damage result for P2");

    assert_eq!(
        (selected_result.stage.hitbox_id, selected_result.angle),
        (1, 78),
        "Slippi frame 2253 exports a 78-degree AttackAirN launch vector; decomp ftColl_8007A06C selects the highest-kb DmgLogEntry and copies its hit->kb_angle into dmg.x1848_kb_angle. confirms={confirm_debug:?} damage_results={damage_results:?} previous_victim_position_confirms={previous_victim_position_confirms} current_victim_pose2_confirms={current_victim_pose2_confirms} current_victim_pose1_confirms={current_victim_pose1_confirms} previous_victim_root_pose2_confirms={previous_victim_root_pose2_confirms} previous_victim_root_pose1_confirms={previous_victim_root_pose1_confirms} previous_attacker_pose_confirms={previous_attacker_pose_confirms} previous_both_confirms={previous_both_confirms} next_attacker_pose_confirms={next_attacker_pose_confirms} current_variant_confirms={current_variant_confirms:?} previous_victim_position_variant_confirms={previous_victim_position_variant_confirms:?} current_victim_pose2_variant_confirms={current_victim_pose2_variant_confirms:?} current_victim_pose1_variant_confirms={current_victim_pose1_variant_confirms:?} previous_victim_root_pose2_variant_confirms={previous_victim_root_pose2_variant_confirms:?} previous_victim_root_pose1_variant_confirms={previous_victim_root_pose1_variant_confirms:?} previous_attacker_pose_variant_confirms={previous_attacker_pose_variant_confirms:?} previous_both_variant_confirms={previous_both_variant_confirms:?} next_attacker_pose_variant_confirms={next_attacker_pose_variant_confirms:?} p1_hits={p1_hits:?} p2_hurts={p2_hurts:?}"
    );

    assert!(
        p2_hurts_use_live_dash_pose_boundary
            && confirms
            .iter()
            .any(|confirm| confirm.attacker_index == 0 && confirm.victim_index == 1),
        "Slippi source frame 2253 has P2 entering damage action 79 from P1 AttackAirN; source collision should sample P2 hurt capsules from the decomp live Dash pose boundary and confirm before hitlag freezes frame 2254. hits={} hurts={} source_hits={} confirms={} p2_hurts_use_live_dash_pose_boundary={p2_hurts_use_live_dash_pose_boundary} previous_victim_position_confirms={previous_victim_position_confirms} current_victim_pose2_confirms={current_victim_pose2_confirms} current_victim_pose1_confirms={current_victim_pose1_confirms} previous_victim_root_pose2_confirms={previous_victim_root_pose2_confirms} previous_victim_root_pose1_confirms={previous_victim_root_pose1_confirms} previous_attacker_pose_confirms={previous_attacker_pose_confirms} previous_both_confirms={previous_both_confirms} next_attacker_pose_confirms={next_attacker_pose_confirms} nearest_center_gap={nearest_center_gap:?} p1_hits={p1_hits:?} p2_hurts={p2_hurts:?}",
        collision_frame.hits.len(),
        collision_frame.hurts.len(),
        source_hits.len(),
        confirms.len()
    );
}

#[test]
fn runtime_replay_nair_frame2272_does_not_reuse_stale_damage_hurt_root_for_active_hitbox() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");

    let world = World::for_two_players_on_stage(StageProfile::battlefield());
    let mut frame = RenderFrame::from_world(&world);

    frame.player_positions[0] = Vec2 {
        x: source_units_to_milli(17.27659797668457),
        y: source_units_to_milli(7.3100996017456055),
    };
    frame.player_source_positions[0] = SourceVec2 {
        x: 17.27659797668457,
        y: 7.3100996017456055,
    };
    frame.player_source_previous_positions[0] = SourceVec2 {
        x: 17.12466049194336,
        y: 10.810099601745605,
    };
    frame.player_motion_states[0] = MotionState::AttackAirN;
    frame.player_source_pose_motion_states[0] = MotionState::AttackAirN;
    frame.player_source_pose_action_state_ids[0] = Some(MeleeActionStateId::new(65));
    frame.player_source_action_keys[0] = Some(SourceActionKey::new("AttackAirN"));
    frame.player_animation_frames[0] = 23;
    frame.player_source_pose_frames[0] = 23;
    frame.player_source_pose_model_facings[0] = 1;
    frame.player_grounded[0] = false;

    frame.player_positions[1] = Vec2 {
        x: source_units_to_milli(23.182790756225586),
        y: source_units_to_milli(-4.0619354248046875),
    };
    frame.player_source_positions[1] = SourceVec2 {
        x: 23.182790756225586,
        y: -4.0619354248046875,
    };
    frame.player_source_previous_positions[1] = SourceVec2 {
        x: 10.464345932006836,
        y: 0.00009999999747378752,
    };
    frame.player_source_pose_action_state_ids[1] = Some(MeleeActionStateId::new(79));
    frame.player_source_action_keys[1] = Some(SourceActionKey::new("DamageN2"));
    frame.player_source_pose_motion_states[1] = MotionState::Wait;
    frame.player_source_pose_frames[1] = 16;
    frame.player_source_pose_model_facings[1] = -1;
    frame.player_grounded[1] = false;

    let confirms = source_hit_confirms_from_frame(&frame);

    assert!(
        confirms
            .iter()
            .all(|confirm| !(confirm.attacker_index == 0 && confirm.victim_index == 1)),
        "Slippi source frame 2272 keeps P2 in damage action 79; P1 AttackAirN's already-active second hit must not reuse stale first-hit CollData hurt roots and connect a frame early. confirms={confirms:?}"
    );
}

#[test]
fn runtime_apply_source_collisions_for_world_makes_active_hitboxes_affect_game_state() {
    let mut world = World::for_two_players();
    let mut attacker = world.players()[0];
    attacker.position = Vec2 { x: 0, y: 0 };
    attacker.grounded = false;
    attacker.set_motion_state_alias(MotionState::AttackAirN);
    attacker.motion_frame = 6;
    attacker.set_source_motion_anim_frame(7.0);
    assert!(world.set_player_state_for_diagnostic(0, attacker));
    let mut victim = world.players()[1];
    victim.position = Vec2 { x: 0, y: 0 };
    victim.grounded = false;
    assert!(world.set_player_state_for_diagnostic(1, victim));

    let step = apply_source_collisions_for_world(&mut world);

    assert!(!step.confirms.is_empty());
    assert!(!step.stages.is_empty());
    assert!(!step.results.is_empty());
    assert!(world.players()[1].damage_percent_temp > 0.0);
    assert!(world.players()[1].damage_applied > 0);
}

#[test]
fn runtime_source_collision_routes_falcon_dive_hit_through_capture_captain_callback() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let mut world = World::for_two_players();
    let mut attacker = world.players()[0];
    attacker.position = Vec2 { x: 0, y: 0 };
    attacker.source_position = SourceVec2 { x: 0.0, y: 0.0 };
    attacker.grounded = false;
    attacker.facing = 1;
    attacker.set_motion_state_alias(MotionState::SpecialAirHi);
    attacker.motion_frame = 13;
    attacker.set_source_motion_anim_frame(13.0);
    assert!(world.set_player_state_for_diagnostic(0, attacker));
    let mut victim = world.players()[1];
    victim.position = Vec2 { x: 0, y: 0 };
    victim.source_position = SourceVec2 { x: 0.0, y: 0.0 };
    victim.grounded = false;
    victim.facing = -1;
    victim.set_motion_state_alias(MotionState::Fall);
    victim.motion_frame = 13;
    victim.set_source_motion_anim_frame(13.0);
    assert!(world.set_player_state_for_diagnostic(1, victim));
    let mut probe_frame = RenderFrame::from_world(&world);
    let collision_frame = source_collision_frame_from_frame(&probe_frame);
    let mut overlap_position = None;
    'candidate: for hit in collision_frame
        .hits
        .iter()
        .filter(|hit| hit.owner_index == 0 && hit.hitbox.is_some())
    {
        for hurt in collision_frame
            .hurts
            .iter()
            .filter(|hurt| hurt.owner_index == 1)
        {
            for hit_point in [hit.capsule.a, hit.capsule.b] {
                for hurt_point in [hurt.capsule.a, hurt.capsule.b] {
                    let source_position = SourceVec2 {
                        x: (hit_point.x - hurt_point.x) / 1_000.0,
                        y: (hit_point.y - hurt_point.y) / 1_000.0,
                    };
                    probe_frame.player_positions[1] = source_position.to_milli();
                    probe_frame.player_source_positions[1] = source_position;
                    probe_frame.player_source_previous_positions[1] = source_position;
                    if source_grab_confirms(&source_collision_frame_from_frame(&probe_frame))
                        .iter()
                        .any(|confirm| confirm.grabber_index == 0 && confirm.victim_index == 1)
                    {
                        overlap_position = Some(source_position);
                        break 'candidate;
                    }
                }
            }
        }
    }
    let overlap_position = overlap_position.unwrap_or_else(|| {
        panic!(
            "test fixture should find a Falcon Dive source hit overlap; hits={}, hurts={}, first_hit={:?}, first_hurt={:?}",
            collision_frame.hits.len(),
            collision_frame.hurts.len(),
            collision_frame.hits.first(),
            collision_frame.hurts.first()
        )
    });
    let mut victim = world.players()[1];
    victim.position = overlap_position.to_milli();
    victim.source_position = overlap_position;
    victim.grounded = false;
    victim.facing = -1;
    victim.set_motion_state_alias(MotionState::Fall);
    victim.motion_frame = 13;
    victim.set_source_motion_anim_frame(13.0);
    assert!(world.set_player_state_for_diagnostic(1, victim));

    let step = apply_source_collisions_for_world(&mut world);
    let attacker = world.players()[0];
    let victim = world.players()[1];

    assert!(
        step.grab_confirms
            .iter()
            .any(|confirm| confirm.grabber_index == 0 && confirm.victim_index == 1),
        "Falcon Dive active source catch hitbox should make contact before the special callback is routed"
    );
    assert_eq!(
        attacker.melee_action_state_id,
        Some(MeleeActionStateId::new(355)),
        "ftCa_SpecialLw_800E5128 changes the attacker to ftCa_MS_SpecialHiCatch"
    );
    assert_eq!(
        attacker.source_action_key,
        Some(SourceActionKey::new("SpecialHiCatch"))
    );
    assert_eq!(
        victim.melee_action_state_id,
        Some(MeleeActionStateId::new(275)),
        "ftCo_8009CA0C changes the victim to ftCo_MS_CaptureCaptain"
    );
    assert_eq!(
        victim.source_action_key,
        Some(SourceActionKey::new("TCaptainSpecialHi"))
    );
    assert_eq!(attacker.source_victim_index, Some(1));
    assert_eq!(victim.source_victim_index, Some(0));
    assert!(
        victim.source_x2226_b2,
        "ftCo_800DB368 constrains airborne CaptureCaptain victims to Captain's TransN2"
    );
    assert!(
        step.stages.is_empty() && victim.damage_percent_temp == 0.0,
        "Falcon Dive's contact hitbox is routed through the special capture callback, not generic damage"
    );
}

#[test]
fn runtime_source_collision_routes_falcon_raptor_boost_detect_callback() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let mut world = World::for_two_players();
    let mut attacker = world.players()[0];
    attacker.position = Vec2 { x: 0, y: 0 };
    attacker.source_position = SourceVec2 { x: 0.0, y: 0.0 };
    attacker.grounded = true;
    attacker.facing = 1;
    attacker.set_motion_state_alias(MotionState::SpecialSStart);
    attacker.motion_frame = 20;
    attacker.set_source_motion_anim_frame(21.0);
    attacker.motion_cmd_var0 = 1;
    attacker.ground_velocity_x = 3.3760004;
    attacker.source_self_velocity_x = 3.3760004;
    assert!(world.set_player_state_for_diagnostic(0, attacker));
    let mut victim = world.players()[1];
    victim.position = Vec2 { x: 0, y: 0 };
    victim.source_position = SourceVec2 { x: 0.0, y: 0.0 };
    victim.grounded = true;
    victim.facing = -1;
    victim.set_motion_state_alias(MotionState::Guard);
    victim.motion_frame = 5;
    victim.set_source_motion_anim_frame(5.0);
    assert!(world.set_player_state_for_diagnostic(1, victim));

    let mut probe_frame = RenderFrame::from_world(&world);
    let collision_frame = source_collision_frame_from_frame(&probe_frame);
    let mut overlap_position = None;
    'candidate: for hit in collision_frame
        .hits
        .iter()
        .filter(|hit| hit.owner_index == 0 && hit.hitbox.is_some())
    {
        for hurt in collision_frame
            .hurts
            .iter()
            .filter(|hurt| hurt.owner_index == 1)
        {
            for hit_point in [hit.capsule.a, hit.capsule.b] {
                for hurt_point in [hurt.capsule.a, hurt.capsule.b] {
                    let source_position = SourceVec2 {
                        x: (hit_point.x - hurt_point.x) / 1_000.0,
                        y: (hit_point.y - hurt_point.y) / 1_000.0,
                    };
                    probe_frame.player_positions[1] = source_position.to_milli();
                    probe_frame.player_source_positions[1] = source_position;
                    probe_frame.player_source_previous_positions[1] = source_position;
                    if source_hit_confirms_from_frame(&probe_frame)
                        .iter()
                        .any(|confirm| confirm.attacker_index == 0 && confirm.victim_index == 1)
                    {
                        overlap_position = Some(source_position);
                        break 'candidate;
                    }
                }
            }
        }
    }
    let overlap_position =
        overlap_position.expect("test fixture should find a Raptor Boost source hit overlap");
    let mut victim = world.players()[1];
    victim.position = overlap_position.to_milli();
    victim.source_position = overlap_position;
    victim.grounded = true;
    victim.facing = -1;
    victim.set_motion_state_alias(MotionState::Guard);
    victim.motion_frame = 5;
    victim.set_source_motion_anim_frame(5.0);
    assert!(world.set_player_state_for_diagnostic(1, victim));

    let step = apply_source_collisions_for_world(&mut world);
    let attacker = world.players()[0];

    assert!(
        step.geometry_confirms
            .iter()
            .any(|confirm| confirm.attacker_index == 0 && confirm.victim_index == 1),
        "Raptor Boost source hitbox should detect the shielded fighter before ftCa_SpecialS_OnDetect is routed"
    );
    assert_eq!(
        attacker.melee_action_state_id,
        Some(MeleeActionStateId::new(350)),
        "ftCa_SpecialS_OnDetect changes grounded SpecialSStart to ftCa_MS_SpecialS"
    );
    assert_eq!(
        attacker.source_action_key,
        Some(SourceActionKey::new("SpecialS"))
    );
    assert_eq!(attacker.motion_state, MotionState::SpecialS);
    assert!(
        (attacker.ground_velocity_x - 0.60768014).abs() < 0.00001,
        "onDetectGround multiplies fp->gr_vel by specials_gr_vel_x; got {}",
        attacker.ground_velocity_x
    );
    assert_eq!(attacker.source_self_velocity_y, 0.0);
}

#[test]
fn runtime_source_collision_detects_raptor_boost_against_guard_shield_at_replay_2706() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let mut world = World::for_two_players();
    let mut attacker = world.players()[0];
    attacker.position = Vec2 { x: 23_228, y: 0 };
    attacker.source_position = SourceVec2 {
        x: 23.228433,
        y: 0.0001,
    };
    attacker.grounded = true;
    attacker.facing = 1;
    attacker.set_motion_state_alias(MotionState::SpecialSStart);
    attacker.motion_frame = 20;
    attacker.set_source_motion_anim_frame(21.0);
    attacker.motion_cmd_var0 = 1;
    attacker.ground_velocity_x = 3.376002;
    attacker.source_self_velocity_x = 3.376002;
    assert!(world.set_player_state_for_diagnostic(0, attacker));
    let mut victim = world.players()[1];
    victim.position = Vec2 { x: 41_930, y: 0 };
    victim.source_position = SourceVec2 {
        x: 41.93023,
        y: 0.0001,
    };
    victim.grounded = true;
    victim.facing = -1;
    victim.set_motion_state_alias(MotionState::GuardOff);
    victim.motion_frame = 10;
    victim.set_source_motion_anim_frame(10.0);
    victim.shield_health = 59.004993;
    victim.lightshield_amount = 0.0;
    assert!(world.set_player_state_for_diagnostic(1, victim));

    let frame = RenderFrame::from_world(&world);
    let collision_frame = source_collision_frame_from_frame(&frame);
    let confirms = source_hit_confirms_from_frame(&frame);

    assert!(
        confirms
            .iter()
            .any(|confirm| confirm.attacker_index == 0 && confirm.victim_index == 1),
        "source shield collision should detect the 2706 Raptor Boost/GuardOff contact; hits={} hurts={} p1_hits={} p2_hurts={:?} confirms={confirms:?}",
        collision_frame.hits.len(),
        collision_frame.hurts.len(),
        collision_frame.hits.iter().filter(|hit| hit.owner_index == 0).count(),
        collision_frame
            .hurts
            .iter()
            .filter(|hurt| hurt.owner_index == 1)
            .map(|hurt| (hurt.capsule_id, hurt.capsule))
            .collect::<Vec<_>>()
    );
}

#[test]
fn runtime_updates_aimed_guard_shield_position_from_guard_aim_frame_not_motion_playback_frame() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");

    fn shield_world_with_aim_angle(angle_degrees: f32) -> World {
        let mut world = World::for_two_players();
        let mut player = world.players()[0];
        player.position = Vec2 { x: 0, y: 0 };
        player.source_position = SourceVec2 { x: 0.0, y: 0.0 };
        player.source_position_z = 0.0;
        player.grounded = true;
        player.facing = 1;
        player.set_motion_state_alias(MotionState::Guard);
        player.motion_frame = 5;
        player.set_source_motion_anim_frame(5.0);
        player.source_shield_hit_active = true;
        player.source_shield_hit_update_pos = true;
        player.source_shield_aim_angle_degrees = angle_degrees;
        player.source_shield_aim_magnitude = 1.0;
        assert!(world.set_player_state_for_diagnostic(0, player));

        let mut other = world.players()[1];
        other.position = Vec2 { x: 200_000, y: 0 };
        other.source_position = SourceVec2 { x: 200.0, y: 0.0 };
        assert!(world.set_player_state_for_diagnostic(1, other));
        world
    }

    let shield_held = [
        PlayerInput::neutral().with_left_trigger_digital(true),
        PlayerInput::neutral(),
    ];
    let mut low_aim = shield_world_with_aim_angle(55.0);
    let mut high_aim = shield_world_with_aim_angle(200.0);

    step_world_with_source_collision_step(&mut low_aim, Frame(0), &shield_held);
    step_world_with_source_collision_step(&mut high_aim, Frame(0), &shield_held);

    assert_ne!(
        low_aim.players()[0].source_shield_hit_position,
        high_aim.players()[0].source_shield_hit_position,
        "ftCo_80091E78 poses the shield object from Guard action 38 at mv.co.guard.x8; different shield aim frames must move the hit object center"
    );
}

#[test]
fn runtime_guard_shield_position_blends_aimed_pose_by_shield_magnitude_like_ftco_80091e78() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");

    fn shield_position_with_aim_magnitude(aim_magnitude: f32) -> SourcePosePoint {
        let mut world = World::for_two_players_with_common_data(MeleeCommonData {
            shield_aim_smoothing: 0.0,
            ..MeleeCommonData::provisional_mole()
        });
        let mut player = world.players()[0];
        player.position = Vec2 { x: 0, y: 0 };
        player.source_position = SourceVec2 { x: 0.0, y: 0.0 };
        player.source_position_z = 0.0;
        player.grounded = true;
        player.facing = 1;
        player.set_motion_state_alias(MotionState::Guard);
        player.motion_frame = 5;
        player.set_source_motion_anim_frame(5.0);
        player.source_shield_hit_active = true;
        player.source_shield_hit_update_pos = true;
        player.source_shield_aim_angle_degrees = 200.0;
        player.source_shield_aim_magnitude = aim_magnitude;
        assert!(world.set_player_state_for_diagnostic(0, player));

        let shield_held = [
            PlayerInput::neutral().with_left_trigger_digital(true),
            PlayerInput::neutral(),
        ];
        step_world_with_source_collision_step(&mut world, Frame(0), &shield_held);
        world.players()[0].source_shield_hit_position
    }

    let base = shield_position_with_aim_magnitude(0.0);
    let quarter = shield_position_with_aim_magnitude(0.25);
    let full = shield_position_with_aim_magnitude(1.0);
    let base_to_quarter = ((base.x - quarter.x).abs()
        + (base.y - quarter.y).abs()
        + (base.z - quarter.z).abs()) as f64;
    let base_to_full =
        ((base.x - full.x).abs() + (base.y - full.y).abs() + (base.z - full.z).abs()) as f64;

    assert!(
        base_to_quarter > 0.0 && base_to_full > base_to_quarter,
        "ftCo_80091E78 blends the aimed Guard pose by mv.co.guard.x4 before updating the shield object center"
    );
    assert!(
        base_to_quarter < base_to_full * 0.75,
        "partial shield aim should not move the shield object as far as full shield aim; base_to_quarter={base_to_quarter}, base_to_full={base_to_full}"
    );
}

#[test]
fn runtime_guard_neutral_stick_does_not_play_guard_direction_table_like_ftco_80091e78() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");

    fn shield_position_for_guard_playback_frame(playback_frame: f32) -> SourcePosePoint {
        let mut world = World::for_two_players_with_common_data(MeleeCommonData {
            shield_aim_smoothing: 0.0,
            ..MeleeCommonData::provisional_mole()
        });
        let mut player = world.players()[0];
        player.position = Vec2 { x: 0, y: 0 };
        player.source_position = SourceVec2 { x: 0.0, y: 0.0 };
        player.source_position_z = 0.0;
        player.grounded = true;
        player.facing = 1;
        player.set_motion_state_alias(MotionState::Guard);
        player.motion_frame = playback_frame as u8;
        player.set_source_motion_anim_frame(playback_frame);
        player.source_shield_collision_active = true;
        player.source_shield_hit_active = true;
        player.source_shield_hit_update_pos = true;
        player.source_shield_aim_angle_degrees = 10.0;
        player.source_shield_aim_magnitude = 0.0;
        assert!(world.set_player_state_for_diagnostic(0, player));

        let shield_held = [
            PlayerInput::neutral().with_left_trigger_digital(true),
            PlayerInput::neutral(),
        ];
        step_world_with_source_collision_step(&mut world, Frame(0), &shield_held);
        world.players()[0].source_shield_hit_position
    }

    let early = shield_position_for_guard_playback_frame(1.0);
    let late = shield_position_for_guard_playback_frame(19.0);

    assert_eq!(
        early, late,
        "ftCo_80091E78 only samples Guard action 38 as a direction table when mv.co.guard.x4 is nonzero; neutral stick must not cycle shield position through Guard playback frames"
    );
}

#[test]
fn render_guard_neutral_stick_wireframe_does_not_play_guard_direction_table_like_ftco_80091e78() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");

    fn scene_for_guard_playback_frame(playback_frame: f32) -> RenderScene {
        let mut world = World::for_two_players();
        let mut player = world.players()[0];
        player.position = Vec2 { x: 0, y: 0 };
        player.source_position = SourceVec2 { x: 0.0, y: 0.0 };
        player.source_position_z = 0.0;
        player.grounded = true;
        player.facing = 1;
        player.set_motion_state_alias(MotionState::Guard);
        player.motion_frame = playback_frame as u8;
        player.set_source_motion_anim_frame(playback_frame);
        player.source_shield_collision_active = true;
        player.source_shield_hit_active = true;
        player.source_shield_aim_angle_degrees = 10.0;
        player.source_shield_aim_magnitude = 0.0;
        assert!(world.set_player_state_for_diagnostic(0, player));

        let mut other = world.players()[1];
        other.position = Vec2 { x: 200_000, y: 0 };
        other.source_position = SourceVec2 { x: 200.0, y: 0.0 };
        assert!(world.set_player_state_for_diagnostic(1, other));

        RenderScene::from_frame(&RenderFrame::from_world(&world), 960, 540)
    }

    let early = scene_for_guard_playback_frame(1.0);
    let late = scene_for_guard_playback_frame(19.0);

    assert_eq!(
        early.player_hurtbox_pills[0], late.player_hurtbox_pills[0],
        "ftCo_80091E78 keeps neutral shield pose on the non-directional branch; the wireframe/hurt capsules must not cycle through Guard action 38 while mv.co.guard.x4 is zero"
    );
}

#[test]
fn render_guard_hurtbox_wireframe_uses_aimed_guard_pose_like_ftco_80091e78() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");

    fn frame_with_guard_aim(angle_degrees: f32) -> RenderFrame {
        let mut world = World::for_two_players();
        let mut player = world.players()[0];
        player.position = Vec2 { x: 0, y: 0 };
        player.source_position = SourceVec2 { x: 0.0, y: 0.0 };
        player.source_position_z = 0.0;
        player.grounded = true;
        player.facing = 1;
        player.set_motion_state_alias(MotionState::Guard);
        player.motion_frame = 5;
        player.set_source_motion_anim_frame(5.0);
        player.source_shield_collision_active = true;
        player.source_shield_hit_active = true;
        player.source_shield_aim_angle_degrees = angle_degrees;
        player.source_shield_aim_magnitude = 1.0;
        assert!(world.set_player_state_for_diagnostic(0, player));

        let mut other = world.players()[1];
        other.position = Vec2 { x: 200_000, y: 0 };
        other.source_position = SourceVec2 { x: 200.0, y: 0.0 };
        assert!(world.set_player_state_for_diagnostic(1, other));
        RenderFrame::from_world(&world)
    }

    let low_aim_scene = RenderScene::from_frame(&frame_with_guard_aim(55.0), 960, 540);
    let high_aim_scene = RenderScene::from_frame(&frame_with_guard_aim(200.0), 960, 540);

    assert!(
        !low_aim_scene.player_hurtbox_pills[0].is_empty()
            && !high_aim_scene.player_hurtbox_pills[0].is_empty(),
        "Guard should expose source hurt capsules for shield-aim pose verification"
    );
    assert_ne!(
        low_aim_scene.player_hurtbox_pills[0],
        high_aim_scene.player_hurtbox_pills[0],
        "ftCo_80091E78 samples Guard action 38 at mv.co.guard.x8 before animating TransN; aimed shield wireframe/hurt capsules must follow that posed skeleton, not the ordinary Guard playback frame"
    );
}

#[test]
fn render_guard_hurtbox_wireframe_blends_aimed_pose_by_shield_magnitude_like_ftco_80091e78() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");

    fn scene_with_guard_aim(angle_degrees: f32, aim_magnitude: f32) -> RenderScene {
        let mut world = World::for_two_players();
        let mut player = world.players()[0];
        player.position = Vec2 { x: 0, y: 0 };
        player.source_position = SourceVec2 { x: 0.0, y: 0.0 };
        player.source_position_z = 0.0;
        player.grounded = true;
        player.facing = 1;
        player.set_motion_state_alias(MotionState::Guard);
        player.motion_frame = 5;
        player.set_source_motion_anim_frame(5.0);
        player.source_shield_collision_active = true;
        player.source_shield_hit_active = true;
        player.source_shield_aim_angle_degrees = angle_degrees;
        player.source_shield_aim_magnitude = aim_magnitude;
        assert!(world.set_player_state_for_diagnostic(0, player));

        let mut other = world.players()[1];
        other.position = Vec2 { x: 200_000, y: 0 };
        other.source_position = SourceVec2 { x: 200.0, y: 0.0 };
        assert!(world.set_player_state_for_diagnostic(1, other));

        RenderScene::from_frame(&RenderFrame::from_world(&world), 960, 540)
    }

    fn source_capsule_delta(left: &[RenderCapsule], right: &[RenderCapsule]) -> f64 {
        left.iter()
            .zip(right.iter())
            .map(|(left, right)| {
                (left.source.a.x - right.source.a.x).abs()
                    + (left.source.a.y - right.source.a.y).abs()
                    + (left.source.a.z - right.source.a.z).abs()
                    + (left.source.b.x - right.source.b.x).abs()
                    + (left.source.b.y - right.source.b.y).abs()
                    + (left.source.b.z - right.source.b.z).abs()
            })
            .sum()
    }

    let base_scene = scene_with_guard_aim(200.0, 0.0);
    let quarter_scene = scene_with_guard_aim(200.0, 0.25);
    let full_scene = scene_with_guard_aim(200.0, 1.0);
    let base_to_quarter = source_capsule_delta(
        &base_scene.player_hurtbox_pills[0],
        &quarter_scene.player_hurtbox_pills[0],
    );
    let base_to_full = source_capsule_delta(
        &base_scene.player_hurtbox_pills[0],
        &full_scene.player_hurtbox_pills[0],
    );

    assert!(
        base_to_quarter > 0.0 && base_to_full > base_to_quarter,
        "ftCo_80091E78 uses mv.co.guard.x4 as the Guard-pose blend weight; a partial shield aim magnitude must not snap to the full aimed skeleton"
    );
    assert!(
        base_to_quarter < base_to_full * 0.75,
        "partial shield aim should stay closer to the ordinary Guard pose than to the fully aimed Guard pose; base_to_quarter={base_to_quarter}, base_to_full={base_to_full}"
    );
}

#[test]
fn source_collision_guard_hurt_capsules_use_aimed_guard_pose_like_ftco_80091e78() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");

    fn body_hurts_with_guard_aim(angle_degrees: f32) -> Vec<SourceCollisionCapsule> {
        let mut world = World::for_two_players();
        let mut player = world.players()[0];
        player.position = Vec2 { x: 0, y: 0 };
        player.source_position = SourceVec2 { x: 0.0, y: 0.0 };
        player.source_position_z = 0.0;
        player.grounded = true;
        player.facing = 1;
        player.set_motion_state_alias(MotionState::Guard);
        player.motion_frame = 5;
        player.set_source_motion_anim_frame(5.0);
        player.source_shield_collision_active = true;
        player.source_shield_hit_active = true;
        player.source_shield_aim_angle_degrees = angle_degrees;
        player.source_shield_aim_magnitude = 1.0;
        assert!(world.set_player_state_for_diagnostic(0, player));

        let mut other = world.players()[1];
        other.position = Vec2 { x: 200_000, y: 0 };
        other.source_position = SourceVec2 { x: 200.0, y: 0.0 };
        assert!(world.set_player_state_for_diagnostic(1, other));

        source_collision_frame_from_frame(&RenderFrame::from_world(&world))
            .hurts
            .into_iter()
            .filter(|hurt| hurt.owner_index == 0 && hurt.capsule_id != SOURCE_SHIELD_HURTBOX_ID)
            .collect()
    }

    let low_aim_hurts = body_hurts_with_guard_aim(55.0);
    let high_aim_hurts = body_hurts_with_guard_aim(200.0);

    assert!(
        !low_aim_hurts.is_empty() && !high_aim_hurts.is_empty(),
        "Guard should expose source body hurt capsules for shield-aim collision verification"
    );
    assert_ne!(
        low_aim_hurts, high_aim_hurts,
        "ftCo_80091E78 samples Guard action 38 at mv.co.guard.x8 before hit processing; gameplay hurt capsules must follow the same aimed skeleton as the shield object"
    );
}

#[test]
fn runtime_source_shield_collision_uses_x221b_b0_gate_not_stale_hit_object_position() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.grounded = true;
    player.set_motion_state_alias(MotionState::GuardOff);
    player.source_shield_collision_active = false;
    player.source_shield_hit_active = true;
    player.source_shield_hit_update_pos = false;
    player.source_shield_hit_position = SourcePosePoint {
        x: 0.0,
        y: 8.0,
        z: 0.0,
    };
    assert!(world.set_player_state_for_diagnostic(0, player));

    let frame = RenderFrame::from_world(&world);
    let collision_frame = source_collision_frame_from_frame(&frame);

    assert!(
        !collision_frame
            .hurts
            .iter()
            .any(|hurt| hurt.owner_index == 0 && hurt.capsule_id == SOURCE_SHIELD_HURTBOX_ID),
        "ftcoll.c gates shield collision on fp->x221B_b0; a stale shield_hit position must not export a live shield hurtbox"
    );
}

#[test]
fn runtime_source_shield_collision_scales_radius_by_fighter_model_scale() {
    let common = MeleeCommonData {
        shield_start_health: 60.0,
        shield_size_health_scale: 0.15,
        shield_size_light_min: 1.0,
        shield_size_light_max: 0.6,
        ..MeleeCommonData::PROVISIONAL
    };
    let mut world = World::for_two_players_with_common_data(common);
    let mut player = world.players()[0];
    player.profile = FighterProfile {
        reference_character: "captain_falcon",
        initial_shield_size: 20.0,
        model_scaling: 0.5,
        ..FighterProfile::FALCON_LIKE
    };
    player.grounded = true;
    player.shield_health = common.shield_start_health;
    player.lightshield_amount = 0.0;
    player.source_shield_collision_active = true;
    player.source_shield_hit_active = true;
    player.source_shield_hit_position = Default::default();
    assert!(world.set_player_state_for_diagnostic(0, player));

    let frame = RenderFrame::from_world(&world);
    let collision_frame = source_collision_frame_from_frame(&frame);
    let shield_radius = collision_frame
        .hurts
        .iter()
        .find(|hurt| hurt.owner_index == 0 && hurt.capsule_id == SOURCE_SHIELD_HURTBOX_ID)
        .expect("active shield should expose a source shield hurt capsule")
        .capsule
        .radius;

    assert_eq!(
        shield_radius, 10_000.0,
        "lbColl_80007BCC tests the shield desc through the shield JObj matrix, so the collision radius includes the fighter model scale as well as inlineB0's local shield scale"
    );
}

#[test]
fn runtime_source_shield_size_uses_yoshi_initial_size_branch_like_ftco_inlineb0() {
    let common = MeleeCommonData {
        shield_start_health: 60.0,
        shield_size_health_scale: 0.15,
        shield_size_light_min: 1.0,
        shield_size_light_max: 0.6,
        ..MeleeCommonData::PROVISIONAL
    };

    fn shield_radius_for_profile(common: MeleeCommonData, profile: FighterProfile) -> f32 {
        let mut world = World::for_two_players_with_common_data(common);
        let mut player = world.players()[0];
        player.profile = profile;
        player.grounded = true;
        player.shield_health = 12.0;
        player.lightshield_amount = 0.75;
        player.source_shield_collision_active = true;
        player.source_shield_hit_active = true;
        player.source_shield_hit_position = Default::default();
        assert!(world.set_player_state_for_diagnostic(0, player));
        let frame = mole_runtime::RenderFrame::from_world(&world);
        let collision_frame = source_collision_frame_from_frame(&frame);
        collision_frame
            .hurts
            .iter()
            .find(|hurt| hurt.owner_index == 0 && hurt.capsule_id == SOURCE_SHIELD_HURTBOX_ID)
            .expect("active shield should expose a source shield hurt capsule")
            .capsule
            .radius
    }

    let normal_radius = shield_radius_for_profile(
        common,
        FighterProfile {
            reference_character: "captain_falcon",
            initial_shield_size: 20.0,
            model_scaling: 0.5,
            ..FighterProfile::FALCON_LIKE
        },
    );
    let yoshi_radius = shield_radius_for_profile(
        common,
        FighterProfile {
            reference_character: "yoshi",
            initial_shield_size: 20.0,
            model_scaling: 0.5,
            ..FighterProfile::FALCON_LIKE
        },
    );

    assert!(normal_radius < yoshi_radius);
    assert_eq!(
        yoshi_radius, 10_000.0,
        "ftCo inlineB0 hard-returns initial_shield_size for FTKIND_YOSHI before the shield JObj matrix contributes fighter model scale"
    );
}

#[test]
fn runtime_source_collision_does_not_reapply_same_active_hitbox_to_same_victim() {
    let mut world = World::for_two_players();
    let mut attacker = world.players()[0];
    attacker.position = Vec2 { x: 0, y: 0 };
    attacker.grounded = false;
    attacker.set_motion_state_alias(MotionState::AttackAirN);
    attacker.motion_frame = 6;
    attacker.set_source_motion_anim_frame(7.0);
    assert!(world.set_player_state_for_diagnostic(0, attacker));
    let mut victim = world.players()[1];
    victim.position = Vec2 { x: 0, y: 0 };
    victim.grounded = false;
    assert!(world.set_player_state_for_diagnostic(1, victim));

    let first = apply_source_collisions_for_world(&mut world);
    let after_first_damage = world.players()[1].damage_percent_temp;
    let second = apply_source_collisions_for_world(&mut world);

    assert_eq!(first.applied_stage_count, 1);
    assert_eq!(after_first_damage, 5.0);
    assert!(second.confirms.is_empty());
    assert!(second.stages.is_empty());
    assert_eq!(second.applied_stage_count, 0);
    assert_eq!(world.players()[1].damage_percent_temp, after_first_damage);
}

#[test]
fn runtime_source_collision_allows_rehit_after_source_hitbox_clear_and_respawn() {
    let mut world = World::for_two_players();
    let mut attacker = world.players()[0];
    attacker.position = Vec2 { x: 0, y: 0 };
    attacker.grounded = false;
    attacker.set_motion_state_alias(MotionState::AttackAirN);
    attacker.motion_frame = 6;
    attacker.set_source_motion_anim_frame(7.0);
    assert!(world.set_player_state_for_diagnostic(0, attacker));
    let mut victim = world.players()[1];
    victim.position = Vec2 { x: 0, y: 0 };
    victim.grounded = false;
    assert!(world.set_player_state_for_diagnostic(1, victim));

    let first_window = apply_source_collisions_for_world(&mut world);
    let mut attacker = world.players()[0];
    attacker.motion_frame = 12;
    attacker.set_source_motion_anim_frame(13.0);
    assert!(world.set_player_state_for_diagnostic(0, attacker));
    let clear_frame = apply_source_collisions_for_world(&mut world);
    let mut attacker = world.players()[0];
    attacker.motion_frame = 19;
    attacker.set_source_motion_anim_frame(20.0);
    assert!(world.set_player_state_for_diagnostic(0, attacker));
    let second_window = apply_source_collisions_for_world(&mut world);

    assert_eq!(first_window.applied_stage_count, 1);
    assert_eq!(clear_frame.applied_stage_count, 0);
    assert_eq!(second_window.applied_stage_count, 1);
    assert_eq!(first_window.stages[0].damage, 5.0);
    assert!(
        (second_window.stages[0].damage - 6.37).abs() < 0.000_01,
        "second NAir hit should be stale-scaled after the first source confirm"
    );
    assert!(
        (world.players()[1].damage_percent_temp - 11.37).abs() < 0.000_01,
        "damage temp should accumulate first hit plus stale-scaled second hit"
    );
}

#[test]
fn runtime_step_with_source_collisions_commits_staged_damage_like_fighter_process_hit() {
    let mut world = World::for_two_players();
    let mut attacker = world.players()[0];
    attacker.position = Vec2 { x: 0, y: 0 };
    attacker.grounded = false;
    attacker.set_motion_state_alias(MotionState::AttackAirN);
    attacker.motion_frame = 6;
    attacker.set_source_motion_anim_frame(7.0);
    assert!(world.set_player_state_for_diagnostic(0, attacker));
    let mut victim = world.players()[1];
    victim.position = Vec2 { x: 0, y: 0 };
    victim.grounded = false;
    assert!(world.set_player_state_for_diagnostic(1, victim));

    step_world_with_source_collisions(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    assert_eq!(world.players()[1].damage_percent, 5.0);
    assert_eq!(world.players()[1].damage_percent_temp, 0.0);
    assert_eq!(world.players()[1].damage_applied, 0);
}

#[test]
fn runtime_source_damage_high_knockback_uses_baked_hip_pose_to_enter_down_bound() {
    let mut world = World::for_two_players();
    let floor = world.stage().main_floor;
    let mut victim = world.players()[1];
    victim.grounded = false;
    victim.motion_state_alias = None;
    victim.motion_state = MotionState::Fall;
    victim.melee_action_state_id = Some(MeleeActionStateId::new(84));
    victim.source_action_key = None;
    victim.motion_frame = 0;
    victim.motion_anim_frame_milli = 0;
    victim.damage_hitstun_frames = 10;
    victim.source_action_total_frames = 20;
    victim.position = Vec2 {
        x: floor.left_x + 10_000,
        y: 0,
    };
    victim.velocity = Vec2 {
        x: mole_core::source_units_to_milli(20.0),
        y: mole_core::source_units_to_milli(-20.0),
    };
    assert!(world.set_player_state_for_diagnostic(1, victim));
    let source_bottom_offset_y =
        world.snapshot().players[1].active_ecb.bottom.y - world.players()[1].position.y;
    let mut victim = world.players()[1];
    victim.position.y = floor.y - source_bottom_offset_y + 1_000;
    assert!(world.set_player_state_for_diagnostic(1, victim));

    step_world_with_source_collisions(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    let bounded = world.players()[1];
    assert_eq!(
        bounded.melee_action_state_id,
        Some(MeleeActionStateId::new(183))
    );
    assert_eq!(
        bounded.source_action_key,
        Some(SourceActionKey::new("DownBoundU"))
    );
    assert_eq!(bounded.motion_state_alias, None);
    assert!(bounded.source_down_bound_pose.is_some());
}

#[test]
fn runtime_source_damage_fly_roll_floor_contact_uses_baked_pose_to_enter_down_bound() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let mut world = World::for_two_players();
    let floor = world.stage().main_floor;
    let mut victim = world.players()[1];
    victim.grounded = false;
    victim.motion_state_alias = None;
    victim.motion_state = MotionState::Fall;
    victim.melee_action_state_id = Some(MeleeActionStateId::new(91));
    victim.source_action_key = None;
    victim.motion_frame = 0;
    victim.motion_anim_frame_milli = 0;
    victim.damage_hitstun_frames = 10;
    victim.source_action_total_frames = 30;
    victim.position = Vec2 {
        x: floor.left_x + 10_000,
        y: 0,
    };
    victim.velocity = Vec2 {
        x: mole_core::source_units_to_milli(1.0),
        y: mole_core::source_units_to_milli(-1.0),
    };
    assert!(world.set_player_state_for_diagnostic(1, victim));
    let source_bottom_offset_y =
        world.snapshot().players[1].active_ecb.bottom.y - world.players()[1].position.y;
    let mut victim = world.players()[1];
    victim.position.y = floor.y - source_bottom_offset_y + 1_000;
    assert!(world.set_player_state_for_diagnostic(1, victim));

    step_world_with_source_collisions(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    let bounded = world.players()[1];
    assert_eq!(
        bounded.melee_action_state_id,
        Some(MeleeActionStateId::new(91))
    );
    assert_eq!(bounded.source_action_key, None);
    assert_eq!(bounded.motion_state_alias, None);
    assert!(bounded.source_down_bound_pose.is_some());
}

#[test]
fn runtime_source_damage_fly_recent_lr_uses_baked_passive_binding() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let mut world = World::for_two_players();
    let floor = world.stage().main_floor;
    let mut victim = world.players()[1];
    victim.grounded = false;
    victim.motion_state_alias = None;
    victim.motion_state = MotionState::Fall;
    victim.melee_action_state_id = Some(MeleeActionStateId::new(91));
    victim.source_action_key = None;
    victim.motion_frame = 0;
    victim.motion_anim_frame_milli = 0;
    victim.damage_hitstun_frames = 10;
    victim.source_action_total_frames = 30;
    victim.position = Vec2 {
        x: floor.left_x + 10_000,
        y: 0,
    };
    victim.velocity = Vec2 {
        x: mole_core::source_units_to_milli(1.0),
        y: mole_core::source_units_to_milli(-1.0),
    };
    assert!(world.set_player_state_for_diagnostic(1, victim));
    let source_bottom_offset_y =
        world.snapshot().players[1].active_ecb.bottom.y - world.players()[1].position.y;
    let mut victim = world.players()[1];
    victim.position.y = floor.y - source_bottom_offset_y + 1_000;
    assert!(world.set_player_state_for_diagnostic(1, victim));

    step_world_with_source_collisions(
        &mut world,
        Frame(0),
        &[
            PlayerInput::neutral(),
            PlayerInput::neutral().with_left_trigger_digital(true),
        ],
    );

    let passive = world.players()[1];
    assert_eq!(
        passive.melee_action_state_id,
        Some(MeleeActionStateId::new(91))
    );
    assert_eq!(passive.source_action_key, None);
    assert_eq!(passive.motion_state_alias, None);
    assert_eq!(passive.source_action_total_frames, 30);
}

#[test]
fn runtime_real_air_attack_input_reaches_source_collision_and_commits_visible_damage() {
    let mut world = World::for_two_players();
    let mut attacker = world.players()[0];
    attacker.position = Vec2 { x: 0, y: 10_000 };
    attacker.grounded = false;
    attacker.set_motion_state_alias(MotionState::Fall);
    assert!(world.set_player_state_for_diagnostic(0, attacker));
    let mut victim = world.players()[1];
    victim.position = Vec2 { x: 0, y: 10_000 };
    victim.grounded = false;
    victim.set_motion_state_alias(MotionState::Fall);
    assert!(world.set_player_state_for_diagnostic(1, victim));

    let attack = [
        PlayerInput::neutral().with_attack(true),
        PlayerInput::neutral(),
    ];
    step_world_with_source_collisions(&mut world, Frame(0), &attack);
    assert_eq!(world.players()[0].motion_state, MotionState::AttackAirN);

    for frame in 1..8 {
        step_world_with_source_collisions(
            &mut world,
            Frame(frame),
            &[PlayerInput::neutral(), PlayerInput::neutral()],
        );
    }

    let render_frame = RenderFrame::from_world(&world);
    assert_eq!(render_frame.player_damage_percents[1], 5.0);
    assert_eq!(render_frame.player_damage_percent_temps[1], 0.0);
    assert!(render_frame.player_damage_knockbacks[1] > 0.0);
    assert!(render_frame.player_hitlag_frames[1] > 0);
    assert_ne!(render_frame.player_velocities[1], Vec2 { x: 0, y: 0 });
    assert!(
        render_frame.player_action_state_ids[1]
            .is_some_and(|action| (75..=91).contains(&action.get())),
        "victim should enter a canonical Melee damage action state"
    );
    assert_eq!(
        render_frame.player_source_pose_action_state_ids[1],
        render_frame.player_action_state_ids[1],
        "canonical damage state must drive the source render pose instead of falling back to the legacy motion alias"
    );
    let scene = RenderScene::from_frame(&render_frame, 960, 540);
    assert!(
        !scene.player_hurtbox_pills[1].is_empty(),
        "canonical damage state must resolve to baked source hurtbox capsules"
    );
    let collision_frame = source_collision_frame_from_frame(&render_frame);
    let damage_hurt = collision_frame
        .hurts
        .iter()
        .find(|capsule| capsule.owner_index == 1)
        .expect("victim damage pose should emit source hurt capsules");
    assert_eq!(
        damage_hurt.action_state_id,
        render_frame.player_action_state_ids[1]
    );
    assert!(
        damage_hurt
            .source_action_key
            .is_some_and(|key| key.as_str().starts_with("Damage")),
        "victim damage pose should resolve through a Damage* source action key"
    );
}

#[test]
fn runtime_source_hitlag_freezes_damage_action_pose_and_position_until_timer_expires() {
    let mut world = World::for_two_players();
    let mut attacker = world.players()[0];
    attacker.position = Vec2 { x: 0, y: 10_000 };
    attacker.grounded = false;
    attacker.set_motion_state_alias(MotionState::Fall);
    assert!(world.set_player_state_for_diagnostic(0, attacker));
    let mut victim = world.players()[1];
    victim.position = Vec2 { x: 0, y: 10_000 };
    victim.grounded = false;
    victim.set_motion_state_alias(MotionState::Fall);
    assert!(world.set_player_state_for_diagnostic(1, victim));

    let attack = [
        PlayerInput::neutral().with_attack(true),
        PlayerInput::neutral(),
    ];
    step_world_with_source_collisions(&mut world, Frame(0), &attack);
    for frame in 1..8 {
        step_world_with_source_collisions(
            &mut world,
            Frame(frame),
            &[PlayerInput::neutral(), PlayerInput::neutral()],
        );
    }

    let hit_frame = RenderFrame::from_world(&world);
    let attacker_hitlag_frames = hit_frame.player_hitlag_frames[0];
    let hitlag_frames = hit_frame.player_hitlag_frames[1];
    assert!(
        attacker_hitlag_frames > 1,
        "decomp ftColl sets attacker dmg.x1914, Fighter_ProcessHit converts it to x195c hitlag"
    );
    assert!(
        !world.players()[0].source_allow_sdi,
        "Rust source_allow_sdi is currently the translated victim damage SDI path, not generic Fighter allow_sdi"
    );
    assert!(hitlag_frames > 1);
    assert_eq!(hit_frame.player_motion_state_aliases[1], None);
    assert!(
        hit_frame.player_action_state_ids[1]
            .is_some_and(|action| (75..=91).contains(&action.get())),
        "victim should be in a canonical source-only damage state"
    );

    step_world_with_source_collisions(
        &mut world,
        Frame(8),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    let frozen_frame = RenderFrame::from_world(&world);
    assert_eq!(
        frozen_frame.player_hitlag_frames[0],
        attacker_hitlag_frames - 1
    );
    assert_eq!(
        frozen_frame.player_source_pose_frames[0], hit_frame.player_source_pose_frames[0],
        "attacker x195c hitlag freezes the live action pose as Fighter_8006A1BC guards update"
    );
    assert_eq!(
        frozen_frame.player_state_frames[0], hit_frame.player_state_frames[0],
        "attacker x195c hitlag skips action-state tick"
    );
    assert_eq!(
        frozen_frame.player_positions[0], hit_frame.player_positions[0],
        "attacker x195c hitlag skips fighter physics/position advancement"
    );
    let held_stick = [
        PlayerInput::neutral().with_left_stick(-121, -40),
        PlayerInput::neutral(),
    ];
    let mut next_frame = 9;
    while world.players()[0].hitlag_frames > 1 {
        step_world_with_source_collisions(&mut world, Frame(next_frame), &held_stick);
        next_frame += 1;
    }
    let attacker_before_exit = RenderFrame::from_world(&world);
    assert_eq!(attacker_before_exit.player_hitlag_frames[0], 1);
    step_world_with_source_collisions(&mut world, Frame(next_frame), &held_stick);

    let attacker_after_hitlag = RenderFrame::from_world(&world);
    assert_eq!(attacker_after_hitlag.player_hitlag_frames[0], 0);
    assert!(
        attacker_after_hitlag.player_state_frames[0] > attacker_before_exit.player_state_frames[0],
        "Fighter_8006A1BC clears hitlag before Fighter_procUpdate, so the exit frame runs the action tick"
    );
    let attacker_exit_delta = Vec2 {
        x: attacker_after_hitlag.player_positions[0].x - attacker_before_exit.player_positions[0].x,
        y: attacker_after_hitlag.player_positions[0].y - attacker_before_exit.player_positions[0].y,
    };
    assert!(
        (attacker_exit_delta.x - attacker_after_hitlag.player_velocities[0].x).abs() <= 2
            && (attacker_exit_delta.y - attacker_after_hitlag.player_velocities[0].y).abs() <= 2,
        "attacker deal-damage hitlag must not run ftCo_Damage ASDI/DI callbacks on exit"
    );
    assert_eq!(frozen_frame.player_hitlag_frames[1], hitlag_frames - 1);
    assert_eq!(
        frozen_frame.player_action_state_ids[1],
        hit_frame.player_action_state_ids[1]
    );
    assert_eq!(
        frozen_frame.player_source_pose_action_state_ids[1],
        hit_frame.player_source_pose_action_state_ids[1]
    );
    assert_eq!(
        frozen_frame.player_source_pose_frames[1], hit_frame.player_source_pose_frames[1],
        "Melee hitlag freezes cur_anim_frame/source pose advancement"
    );
    assert_eq!(
        frozen_frame.player_state_frames[1], hit_frame.player_state_frames[1],
        "Melee hitlag skips the fighter action-state tick"
    );
    assert_eq!(
        frozen_frame.player_positions[1], hit_frame.player_positions[1],
        "Melee hitlag skips fighter physics/position advancement"
    );
    assert_eq!(
        frozen_frame.player_velocities[1], hit_frame.player_velocities[1],
        "hitlag stores knockback velocity without consuming it through physics"
    );
}

#[test]
fn runtime_source_damage_lockout_keeps_damage_action_authoritative_after_hitlag() {
    let mut world = World::for_two_players();
    let mut attacker = world.players()[0];
    attacker.position = Vec2 { x: 0, y: 10_000 };
    attacker.grounded = false;
    attacker.set_motion_state_alias(MotionState::Fall);
    assert!(world.set_player_state_for_diagnostic(0, attacker));
    let mut victim = world.players()[1];
    victim.position = Vec2 { x: 0, y: 10_000 };
    victim.grounded = false;
    victim.set_motion_state_alias(MotionState::Fall);
    assert!(world.set_player_state_for_diagnostic(1, victim));

    step_world_with_source_collisions(
        &mut world,
        Frame(0),
        &[
            PlayerInput::neutral().with_attack(true),
            PlayerInput::neutral(),
        ],
    );
    for frame in 1..8 {
        step_world_with_source_collisions(
            &mut world,
            Frame(frame),
            &[PlayerInput::neutral(), PlayerInput::neutral()],
        );
    }

    let hit_frame = RenderFrame::from_world(&world);
    let damage_action = hit_frame.player_action_state_ids[1]
        .expect("victim should enter a canonical damage action");
    assert!((75..=91).contains(&damage_action.get()));
    assert!(hit_frame.player_hitlag_frames[1] > 0);
    assert!(world.players()[1].damage_hitstun_frames > hit_frame.player_hitlag_frames[1] as u16);

    let mut frame = 8;
    while world.players()[1].hitlag_frames > 0 {
        step_world_with_source_collisions(
            &mut world,
            Frame(frame),
            &[PlayerInput::neutral(), PlayerInput::neutral()],
        );
        frame += 1;
    }

    step_world_with_source_collisions(
        &mut world,
        Frame(frame),
        &[
            PlayerInput::neutral(),
            PlayerInput::neutral().with_attack(true),
        ],
    );

    let damage_frame = RenderFrame::from_world(&world);
    assert_eq!(damage_frame.player_hitlag_frames[1], 0);
    assert_eq!(damage_frame.player_motion_state_aliases[1], None);
    assert_eq!(damage_frame.player_action_state_ids[1], Some(damage_action));
    assert_eq!(
        damage_frame.player_source_pose_action_state_ids[1],
        Some(damage_action)
    );
    assert!(
        damage_frame.player_source_action_keys[1]
            .is_none_or(|key| key.as_str().starts_with("Damage")),
        "source-only damage action must not be replaced by an aerial attack binding"
    );
    assert!(world.players()[1].damage_hitstun_frames > 0);
}

#[test]
fn runtime_source_damage_carries_baked_action_total_frames_into_core() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let mut world = World::for_two_players();
    let mut attacker = world.players()[0];
    attacker.position = Vec2 { x: 0, y: 10_000 };
    attacker.grounded = false;
    attacker.set_motion_state_alias(MotionState::Fall);
    assert!(world.set_player_state_for_diagnostic(0, attacker));
    let mut victim = world.players()[1];
    victim.position = Vec2 { x: 0, y: 10_000 };
    victim.grounded = false;
    victim.set_motion_state_alias(MotionState::Fall);
    assert!(world.set_player_state_for_diagnostic(1, victim));

    step_world_with_source_collisions(
        &mut world,
        Frame(0),
        &[
            PlayerInput::neutral().with_attack(true),
            PlayerInput::neutral(),
        ],
    );
    for frame in 1..8 {
        step_world_with_source_collisions(
            &mut world,
            Frame(frame),
            &[PlayerInput::neutral(), PlayerInput::neutral()],
        );
    }

    let damage_player = world.players()[1];
    assert_eq!(damage_player.motion_state_alias, None);
    assert!(damage_player
        .melee_action_state_id
        .is_some_and(|action| (75..=91).contains(&action.get())));
    assert!(
        damage_player.source_action_total_frames > 0,
        "runtime source damage must carry baked action length into rollback-owned core state"
    );
}

#[test]
fn render_scene_uses_attack_air_n_source_clear_frames_for_hitbox_pills() {
    let world = World::for_two_players();
    let mut frame = RenderFrame::from_world(&world);
    frame.player_motion_states[0] = MotionState::AttackAirN;
    frame.player_facings[0] = 1;
    frame.player_source_pose_motion_states[0] = MotionState::AttackAirN;
    frame.player_source_pose_action_state_ids[0] = Some(MeleeActionStateId::new(65));
    frame.player_source_action_keys[0] = Some(SourceActionKey::new("AttackAirN"));
    frame.player_source_pose_model_facings[0] = 1;

    frame.player_state_frames[0] = 13;
    frame.player_animation_frames[0] = 13;
    frame.player_source_pose_frames[0] = 13;
    let cleared_first_window = RenderScene::from_frame(&frame, 960, 540);
    assert!(cleared_first_window.player_hitbox_pills[0].is_empty());
    assert_eq!(cleared_first_window.player_hurtbox_pills[0].len(), 11);

    frame.player_state_frames[0] = 20;
    frame.player_animation_frames[0] = 20;
    frame.player_source_pose_frames[0] = 21;
    let second_window = RenderScene::from_frame(&frame, 960, 540);
    assert_eq!(second_window.player_hitbox_pills[0].len(), 3);

    frame.player_state_frames[0] = 30;
    frame.player_animation_frames[0] = 30;
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
    frame.player_source_pose_action_state_ids[0] = Some(MeleeActionStateId::new(65));
    frame.player_source_action_keys[0] = Some(SourceActionKey::new("AttackAirN"));
    frame.player_source_pose_model_facings[0] = 1;

    // Melee action hitbox scripts are evaluated against cur_anim_frame
    // (ftAnim_8006E9B4 -> ftAction_80073240), not against gameplay state age.
    // Frame 13 is clear, while animation frame 20 has Captain Falcon Nair's
    // second active window.
    frame.player_state_frames[0] = 12;
    frame.player_animation_frames[0] = 20;
    frame.player_source_pose_frames[0] = 21;

    let scene = RenderScene::from_frame(&frame, 960, 540);

    assert_eq!(scene.player_hitbox_pills[0].len(), 3);
    assert_eq!(scene.player_hurtbox_pills[0].len(), 11);
}

#[test]
fn source_collision_frame_samples_hitbox_script_capsules_from_source_pose_frame() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let world = World::for_two_players();
    let mut frame = RenderFrame::from_world(&world);
    frame.player_motion_states[0] = MotionState::AttackAirN;
    frame.player_source_pose_motion_states[0] = MotionState::AttackAirN;
    frame.player_source_pose_action_state_ids[0] = Some(MeleeActionStateId::new(65));
    frame.player_action_state_ids[0] = Some(MeleeActionStateId::new(65));
    frame.player_source_action_keys[0] = Some(SourceActionKey::new("AttackAirN"));
    frame.player_source_pose_model_facings[0] = 1;

    frame.player_state_frames[0] = 0;
    frame.player_animation_frames[0] = 6;
    frame.player_source_pose_frames[0] = 6;
    assert!(
        source_collision_frame_from_frame(&frame).hits.is_empty(),
        "Captain Falcon AttackAirN source frame 6 has no active hitboxes"
    );

    frame.player_animation_frames[0] = 6;
    frame.player_source_pose_frames[0] = 7;
    let collision_frame = source_collision_frame_from_frame(&frame);

    assert!(
        collision_frame
            .hits
            .iter()
            .any(|capsule| capsule.owner_index == 0
                && capsule.source_action_key == Some(SourceActionKey::new("AttackAirN"))
                && capsule.source_frame == Some(7)),
        "ftAnim_8006EBA4 updates live JObjs, then ftAction_80073240 runs the action script for the source pose frame"
    );
}

#[test]
fn source_collision_frame_samples_catch_hitboxes_from_live_source_anim_frame() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let world = World::for_two_players();

    let first_active_frame = (1..=65)
        .find(|source_frame| {
            let mut frame = RenderFrame::from_world(&world);
            frame.player_motion_states[0] = MotionState::SpecialAirHi;
            frame.player_source_pose_motion_states[0] = MotionState::SpecialAirHi;
            frame.player_source_pose_action_state_ids[0] = Some(MeleeActionStateId::new(354));
            frame.player_action_state_ids[0] = Some(MeleeActionStateId::new(354));
            frame.player_source_action_keys[0] = Some(SourceActionKey::new("SpecialAirHi"));
            frame.player_source_pose_action_keys[0] = Some(SourceActionKey::new("SpecialAirHi"));
            frame.player_source_pose_model_facings[0] = 1;
            frame.player_source_pose_frames[0] = *source_frame;
            frame.player_source_motion_anim_frames[0] = f32::from(source_frame.saturating_sub(1));
            source_collision_frame_from_frame(&frame)
                .hits
                .iter()
                .any(|capsule| {
                    capsule.owner_index == 0
                        && capsule
                            .hitbox
                            .is_some_and(|hitbox| hitbox.element == SOURCE_HIT_ELEMENT_CATCH)
                })
        })
        .expect("SpecialAirHi should expose a source catch hitbox");
    assert!(
        first_active_frame > 1,
        "test needs a stale inactive pose frame before Falcon Dive's active catch frame"
    );

    let mut frame = RenderFrame::from_world(&world);
    frame.player_motion_states[0] = MotionState::SpecialAirHi;
    frame.player_source_pose_motion_states[0] = MotionState::SpecialAirHi;
    frame.player_source_pose_action_state_ids[0] = Some(MeleeActionStateId::new(354));
    frame.player_action_state_ids[0] = Some(MeleeActionStateId::new(354));
    frame.player_source_action_keys[0] = Some(SourceActionKey::new("SpecialAirHi"));
    frame.player_source_pose_action_keys[0] = Some(SourceActionKey::new("SpecialAirHi"));
    frame.player_source_pose_model_facings[0] = 1;
    frame.player_source_pose_frames[0] = first_active_frame - 1;
    frame.player_source_motion_anim_frames[0] = f32::from(first_active_frame - 1);

    let collision_frame = source_collision_frame_from_frame(&frame);

    assert!(
        collision_frame.hits.iter().any(|capsule| {
            capsule.owner_index == 0
                && capsule.source_action_key == Some(SourceActionKey::new("SpecialAirHi"))
                && capsule.source_frame == Some(first_active_frame)
                && capsule
                    .hitbox
                    .is_some_and(|hitbox| hitbox.element == SOURCE_HIT_ELEMENT_CATCH)
        }),
        "ftAnim_8006EBA4 advances Falcon Dive's live pose before collision; catch-element command grab hitboxes must use the live source animation frame"
    );
}

#[test]
fn runtime_source_down_bound_actions_resolve_from_canonical_action_ids() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");

    for (action_state_id, source_action_key) in [
        (
            MeleeActionStateId::new(183),
            SourceActionKey::new("DownBoundU"),
        ),
        (
            MeleeActionStateId::new(184),
            SourceActionKey::new("DownWaitU"),
        ),
        (
            MeleeActionStateId::new(186),
            SourceActionKey::new("DownStandU"),
        ),
        (
            MeleeActionStateId::new(187),
            SourceActionKey::new("DownAttackU"),
        ),
        (
            MeleeActionStateId::new(191),
            SourceActionKey::new("DownBoundD"),
        ),
        (
            MeleeActionStateId::new(192),
            SourceActionKey::new("DownWaitD"),
        ),
        (
            MeleeActionStateId::new(194),
            SourceActionKey::new("DownStandD"),
        ),
        (
            MeleeActionStateId::new(195),
            SourceActionKey::new("DownAttackD"),
        ),
    ] {
        let world = World::for_two_players();
        let mut frame = RenderFrame::from_world(&world);
        frame.player_source_pose_action_state_ids[0] = Some(action_state_id);
        frame.player_action_state_ids[0] = Some(action_state_id);
        frame.player_source_pose_frames[0] = 1;
        frame.player_source_action_keys[0] = None;
        frame.player_source_pose_action_keys[0] = None;

        let scene = RenderScene::from_frame(&frame, 960, 540);
        assert!(
            !scene.player_hurtbox_pills[0].is_empty(),
            "canonical DownBound action {action_state_id:?} must resolve to baked source hurt capsules"
        );

        let collision_frame = source_collision_frame_from_frame(&frame);
        let hurt = collision_frame
            .hurts
            .iter()
            .find(|capsule| capsule.owner_index == 0)
            .expect("DownBound source pose should emit hurt capsules");
        assert_eq!(hurt.action_state_id, Some(action_state_id));
        assert_eq!(hurt.source_action_key, Some(source_action_key));
    }
}

#[test]
fn runtime_source_collision_frame_skips_hurt_capsules_for_source_intangible_players() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let world = World::for_two_players();
    let mut frame = RenderFrame::from_world(&world);
    frame.player_source_collision_states[1] = SOURCE_COLLISION_STATE_HURT_INTANGIBLE;

    let collision_frame = source_collision_frame_from_frame(&frame);

    assert!(
        collision_frame
            .hurts
            .iter()
            .any(|capsule| capsule.owner_index == 0),
        "normal player should still emit source hurt capsules"
    );
    assert!(
        !collision_frame
            .hurts
            .iter()
            .any(|capsule| capsule.owner_index == 1),
        "source x198C hurt-intangible player should not emit hurt capsules"
    );
}

#[test]
fn runtime_source_collision_frame_preserves_baked_hurt_height() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let world = World::for_two_players();
    let frame = RenderFrame::from_world(&world);

    let collision_frame = source_collision_frame_from_frame(&frame);
    let high = collision_frame
        .hurts
        .iter()
        .find(|capsule| capsule.owner_index == 0 && capsule.capsule_id == 2)
        .expect("Falcon head hurt capsule should be present");
    let low = collision_frame
        .hurts
        .iter()
        .find(|capsule| capsule.owner_index == 0 && capsule.capsule_id == 7)
        .expect("Falcon leg hurt capsule should be present");

    assert_eq!(high.hurt_height, SOURCE_HURT_HEIGHT_HIGH);
    assert_eq!(low.hurt_height, SOURCE_HURT_HEIGHT_LOW);
}

#[test]
fn runtime_source_passive_actions_resolve_from_canonical_action_ids() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");

    for (action_state_id, source_action_key) in [
        (
            MeleeActionStateId::new(199),
            SourceActionKey::new("Passive"),
        ),
        (
            MeleeActionStateId::new(200),
            SourceActionKey::new("PassiveStandF"),
        ),
        (
            MeleeActionStateId::new(201),
            SourceActionKey::new("PassiveStandB"),
        ),
    ] {
        let world = World::for_two_players();
        let mut frame = RenderFrame::from_world(&world);
        frame.player_source_pose_action_state_ids[0] = Some(action_state_id);
        frame.player_action_state_ids[0] = Some(action_state_id);
        frame.player_source_pose_frames[0] = 1;
        frame.player_source_action_keys[0] = None;
        frame.player_source_pose_action_keys[0] = None;

        let scene = RenderScene::from_frame(&frame, 960, 540);
        assert!(
            !scene.player_hurtbox_pills[0].is_empty(),
            "canonical Passive action {action_state_id:?} must resolve to baked source hurt capsules"
        );

        let collision_frame = source_collision_frame_from_frame(&frame);
        let hurt = collision_frame
            .hurts
            .iter()
            .find(|capsule| capsule.owner_index == 0)
            .expect("Passive source pose should emit hurt capsules");
        assert_eq!(hurt.action_state_id, Some(action_state_id));
        assert_eq!(hurt.source_action_key, Some(source_action_key));
    }
}

#[test]
fn runtime_source_entry_aliases_resolve_from_canonical_action_ids() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");

    for (motion_state, action_state_id) in [
        (MotionState::Entry, MeleeActionStateId::new(322)),
        (MotionState::EntryStart, MeleeActionStateId::new(323)),
        (MotionState::EntryEnd, MeleeActionStateId::new(324)),
    ] {
        let world = World::for_two_players();
        let mut frame = RenderFrame::from_world(&world);
        frame.player_motion_states[0] = motion_state;
        frame.player_source_pose_motion_states[0] = motion_state;
        frame.player_source_pose_action_state_ids[0] = Some(action_state_id);
        frame.player_action_state_ids[0] = Some(action_state_id);
        frame.player_source_pose_frames[0] = 1;
        frame.player_source_action_keys[0] = None;
        frame.player_source_pose_action_keys[0] = None;

        let scene = RenderScene::from_frame(&frame, 960, 540);
        assert!(
            !scene.player_hurtbox_pills[0].is_empty(),
            "{motion_state:?} action {action_state_id:?} must resolve to baked Entry source hurt capsules"
        );

        let collision_frame = source_collision_frame_from_frame(&frame);
        let hurt = collision_frame
            .hurts
            .iter()
            .find(|capsule| capsule.owner_index == 0)
            .expect("Entry source pose should emit hurt capsules");
        assert_eq!(hurt.action_state_id, Some(action_state_id));
        assert_eq!(hurt.source_action_key, Some(SourceActionKey::new("Entry")));
    }
}

#[test]
fn runtime_source_jab_followup_actions_resolve_from_canonical_action_ids() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");

    for (action_state_id, source_action_key) in [
        (
            MeleeActionStateId::new(45),
            SourceActionKey::new("Attack12"),
        ),
        (
            MeleeActionStateId::new(46),
            SourceActionKey::new("Attack13"),
        ),
        (
            MeleeActionStateId::new(47),
            SourceActionKey::new("Attack100Start"),
        ),
        (
            MeleeActionStateId::new(48),
            SourceActionKey::new("Attack100Loop"),
        ),
        (
            MeleeActionStateId::new(49),
            SourceActionKey::new("Attack100End"),
        ),
    ] {
        let world = World::for_two_players();
        let mut frame = RenderFrame::from_world(&world);
        frame.player_source_pose_action_state_ids[0] = Some(action_state_id);
        frame.player_action_state_ids[0] = Some(action_state_id);
        frame.player_source_pose_frames[0] = 1;
        frame.player_source_action_keys[0] = None;
        frame.player_source_pose_action_keys[0] = None;

        let scene = RenderScene::from_frame(&frame, 960, 540);
        assert!(
            !scene.player_hurtbox_pills[0].is_empty(),
            "canonical jab follow-up action {action_state_id:?} must resolve to baked source hurt capsules"
        );

        let collision_frame = source_collision_frame_from_frame(&frame);
        let hurt = collision_frame
            .hurts
            .iter()
            .find(|capsule| capsule.owner_index == 0)
            .expect("jab follow-up source pose should emit hurt capsules");
        assert_eq!(hurt.action_state_id, Some(action_state_id));
        assert_eq!(hurt.source_action_key, Some(source_action_key));
    }
}

#[test]
fn runtime_source_down_bound_animation_end_uses_baked_down_wait_binding() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let mut world = World::for_two_players();
    let floor = world.stage().main_floor;
    let mut player = world.players()[1];
    player.grounded = true;
    player.motion_state_alias = None;
    player.motion_state = MotionState::Fall;
    player.melee_action_state_id = Some(MeleeActionStateId::new(183));
    player.source_action_key = Some(SourceActionKey::new("DownBoundU"));
    player.source_action_total_frames = 26;
    player.motion_frame = 25;
    player.motion_anim_frame_milli = 25_000;
    player.position = Vec2 {
        x: floor.left_x + 10_000,
        y: floor.y,
    };
    assert!(world.set_player_state_for_diagnostic(1, player));

    step_world_with_source_collisions(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    let waiting = world.players()[1];
    assert_eq!(
        waiting.melee_action_state_id,
        Some(MeleeActionStateId::new(184))
    );
    assert_eq!(
        waiting.source_action_key,
        Some(SourceActionKey::new("DownWaitU"))
    );
    assert_eq!(waiting.source_action_total_frames, 70);
    assert_eq!(
        waiting.source_down_wait_timer.to_bits(),
        world.common_data().down_wait_timer.to_bits()
    );
}

#[test]
fn runtime_source_down_wait_timer_expiry_uses_baked_down_stand_binding() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let mut world = World::for_two_players();
    let floor = world.stage().main_floor;
    let mut player = world.players()[1];
    player.grounded = true;
    player.motion_state_alias = None;
    player.motion_state = MotionState::Fall;
    player.melee_action_state_id = Some(MeleeActionStateId::new(192));
    player.source_action_key = Some(SourceActionKey::new("DownWaitD"));
    player.source_action_total_frames = 70;
    player.source_down_wait_timer = 1.0;
    player.motion_frame = 12;
    player.motion_anim_frame_milli = 12_000;
    player.position = Vec2 {
        x: floor.left_x + 10_000,
        y: floor.y,
    };
    assert!(world.set_player_state_for_diagnostic(1, player));

    step_world_with_source_collisions(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    let standing = world.players()[1];
    assert_eq!(
        standing.melee_action_state_id,
        Some(MeleeActionStateId::new(194))
    );
    assert_eq!(
        standing.source_action_key,
        Some(SourceActionKey::new("DownStandD"))
    );
    assert_eq!(standing.motion_state_alias, None);
    assert_eq!(standing.motion_frame, 0);
    assert_eq!(standing.source_action_total_frames, 30);
    assert_eq!(standing.source_down_wait_timer.to_bits(), 0.0_f32.to_bits());
}

#[test]
fn runtime_source_down_wait_fresh_attack_uses_baked_down_attack_binding() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let mut world = World::for_two_players();
    let floor = world.stage().main_floor;
    let mut player = world.players()[1];
    player.grounded = true;
    player.motion_state_alias = None;
    player.motion_state = MotionState::Fall;
    player.melee_action_state_id = Some(MeleeActionStateId::new(192));
    player.source_action_key = Some(SourceActionKey::new("DownWaitD"));
    player.source_action_total_frames = 70;
    player.source_down_wait_timer = 10.0;
    player.motion_frame = 12;
    player.motion_anim_frame_milli = 12_000;
    player.position = Vec2 {
        x: floor.left_x + 10_000,
        y: floor.y,
    };
    assert!(world.set_player_state_for_diagnostic(1, player));

    step_world_with_source_collisions(
        &mut world,
        Frame(0),
        &[
            PlayerInput::neutral(),
            PlayerInput::neutral().with_attack(true),
        ],
    );

    let attacking = world.players()[1];
    assert_eq!(
        attacking.melee_action_state_id,
        Some(MeleeActionStateId::new(195))
    );
    assert_eq!(
        attacking.source_action_key,
        Some(SourceActionKey::new("DownAttackD"))
    );
    assert_eq!(attacking.motion_state_alias, None);
    assert_eq!(attacking.motion_frame, 0);
    assert_eq!(attacking.source_action_total_frames, 50);
    assert_eq!(
        attacking.source_down_wait_timer.to_bits(),
        9.0_f32.to_bits()
    );
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
    assert_eq!(
        frame.player_action_state_ids[0],
        Some(MeleeActionStateId::new(65))
    );
    assert_eq!(
        frame.player_source_action_keys[0],
        Some(SourceActionKey::new("AttackAirN"))
    );
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
fn render_source_selection_reports_escape_air_action_44_for_authoritative_capsules() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let world = World::for_two_players();
    let mut frame = RenderFrame::from_world(&world);
    frame.player_motion_states[0] = MotionState::EscapeAir;
    frame.player_source_pose_motion_states[0] = MotionState::EscapeAir;
    frame.player_source_pose_action_state_ids[0] = Some(MeleeActionStateId::new(236));
    frame.player_source_pose_action_keys[0] = Some(SourceActionKey::new("EscapeAir"));
    frame.player_source_action_keys[0] = Some(SourceActionKey::new("EscapeAir"));
    frame.player_source_pose_frames[0] = 11;
    frame.player_source_motion_anim_frames[0] = 10.0;

    let hitbox = render_source_hitbox_selection(&frame, 0);
    let hurtbox = render_source_hurtbox_selection(&frame, 0);

    assert_eq!(
        hitbox.source_action_key,
        Some(SourceActionKey::new("EscapeAir"))
    );
    assert_eq!(
        hurtbox.source_action_key,
        Some(SourceActionKey::new("EscapeAir"))
    );
    assert_eq!(hitbox.source_frame, 11);
    assert_eq!(hurtbox.source_frame, 11);
}

#[test]
fn render_source_selection_reports_landing_and_landing_air_aliases() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let world = World::for_two_players();
    let mut frame = RenderFrame::from_world(&world);

    for (motion_state, action_state_id, source_key) in [
        (MotionState::Landing, 42, "Landing"),
        (MotionState::LandingFallSpecial, 43, "Landing"),
        (MotionState::LandingAirN, 70, "LandingAirN"),
        (MotionState::LandingAirF, 71, "LandingAirF"),
        (MotionState::LandingAirB, 72, "LandingAirB"),
        (MotionState::LandingAirHi, 73, "LandingAirHi"),
        (MotionState::LandingAirLw, 74, "LandingAirLw"),
    ] {
        frame.player_motion_states[0] = motion_state;
        frame.player_source_pose_motion_states[0] = motion_state;
        frame.player_source_pose_action_state_ids[0] =
            Some(MeleeActionStateId::new(action_state_id));
        frame.player_source_pose_action_keys[0] = Some(SourceActionKey::new(source_key));
        frame.player_source_action_keys[0] = Some(SourceActionKey::new(source_key));
        frame.player_source_pose_frames[0] = 6;
        frame.player_source_motion_anim_frames[0] = 5.0;

        let selection = render_source_hurtbox_selection(&frame, 0);
        assert_eq!(
            selection.source_action_key,
            Some(SourceActionKey::new(source_key)),
            "{motion_state:?} must render authoritative source capsules from {source_key}"
        );
        assert_eq!(selection.source_frame, 6);
    }
}

#[test]
fn render_source_selection_reports_source_only_jab_grab_throw_capture_actions() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let world = World::for_two_players();
    let mut frame = RenderFrame::from_world(&world);

    for (action_state_id, source_key) in [
        (45, "Attack12"),
        (46, "Attack13"),
        (47, "Attack100Start"),
        (48, "Attack100Loop"),
        (49, "Attack100End"),
        (216, "CatchWait"),
        (217, "CatchAttack"),
        (218, "CatchCut"),
        (219, "ThrowF"),
        (220, "ThrowB"),
        (221, "ThrowHi"),
        (222, "ThrowLw"),
        (223, "CapturePulledHi"),
        (224, "CaptureWaitHi"),
        (225, "CaptureDamageHi"),
        (226, "CapturePulledLw"),
        (227, "CaptureWaitLw"),
        (228, "CaptureDamageLw"),
        (229, "CaptureCut"),
        (230, "CaptureJump"),
        (239, "TCaptainThrowF"),
        (240, "TCaptainThrowB"),
        (241, "TCaptainThrowHi"),
        (242, "TCaptainThrowLw"),
        (275, "TCaptainSpecialHi"),
    ] {
        frame.player_source_pose_action_state_ids[0] =
            Some(MeleeActionStateId::new(action_state_id));
        frame.player_source_pose_action_keys[0] = Some(SourceActionKey::new(source_key));
        frame.player_source_action_keys[0] = Some(SourceActionKey::new(source_key));
        frame.player_source_pose_frames[0] = 4;
        frame.player_source_motion_anim_frames[0] = 3.0;

        let hitbox = render_source_hitbox_selection(&frame, 0);
        let hurtbox = render_source_hurtbox_selection(&frame, 0);

        assert_eq!(
            hitbox.source_action_key,
            Some(SourceActionKey::new(source_key)),
            "source hitboxes must stay keyed to source-only action {action_state_id}/{source_key}"
        );
        assert_eq!(
            hurtbox.source_action_key,
            Some(SourceActionKey::new(source_key)),
            "source hurtboxes must stay keyed to source-only action {action_state_id}/{source_key}"
        );
        assert_eq!(hitbox.source_frame, 4);
        assert_eq!(hurtbox.source_frame, 4);
    }
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
    frame.player_source_pose_action_state_ids[0] = Some(MeleeActionStateId::new(19));
    frame.player_source_pose_action_keys[0] = Some(SourceActionKey::new("TurnRun"));
    frame.player_source_action_keys[0] = Some(SourceActionKey::new("TurnRun"));
    frame.player_source_pose_frames[0] = 11;
    frame.player_source_motion_anim_frames[0] = 10.0;
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
    let mut frame = RenderFrame::from_world(&world);
    frame.player_source_pose_action_state_ids[0] = Some(MeleeActionStateId::new(233));
    frame.player_source_pose_action_keys[0] = Some(SourceActionKey::new("EscapeF"));
    frame.player_source_action_keys[0] = Some(SourceActionKey::new("EscapeF"));
    frame.player_source_pose_frames[0] = 2;
    frame.player_source_motion_anim_frames[0] = 1.0;

    let scene = RenderScene::from_frame(&frame, 960, 540);
    let first_hurtbox = scene.player_hurtbox_pills[0][0];
    let transn = mole_core::source_root_motion_position(
        MotionState::EscapeF,
        frame.player_source_pose_frames[0],
    )
    .unwrap();
    let expected_world_a = Vec2 {
        x: frame.player_source_positions[0].to_milli().x
            + ((first_hurtbox.source.a.x - f64::from(transn.z)) * 1_000.0).round() as i32,
        y: frame.player_source_positions[0].to_milli().y
            + ((first_hurtbox.source.a.y - f64::from(transn.y)) * 1_000.0).round() as i32,
    };

    assert_eq!(
        first_hurtbox.a,
        scene.transform.world_to_screen(expected_world_a)
    );
}

#[test]
fn special_air_hi_capsules_render_after_melee_transn_reset() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.motion_state = MotionState::SpecialAirHi;
    player.motion_frame = 5;
    player.motion_anim_frame_milli = 5_000;
    player.position = Vec2 {
        x: -54_157,
        y: 36_109,
    };
    player.source_position = SourceVec2::from_milli(player.position);
    player.facing = -1;
    player.grounded = false;
    assert!(world.set_player_state_for_diagnostic(0, player));
    let frame = RenderFrame::from_world(&world);

    let scene = RenderScene::from_frame(&frame, 960, 540);
    let first_hurtbox = scene.player_hurtbox_pills[0][0];
    let transn = mole_core::source_root_motion_position(
        MotionState::SpecialAirHi,
        frame.player_source_pose_frames[0],
    )
    .expect("SpecialAirHi source pose should expose TransN root position");
    let facing_sign = if frame.player_source_pose_model_facings[0] < 0 {
        -1.0
    } else {
        1.0
    };
    let expected_world_a = Vec2 {
        x: frame.player_positions[0].x
            + ((first_hurtbox.source.a.x - f64::from(transn.z)) * facing_sign * 1_000.0).round()
                as i32,
        y: frame.player_positions[0].y
            + ((first_hurtbox.source.a.y - f64::from(transn.y)) * 1_000.0).round() as i32,
    };

    assert_eq!(
        first_hurtbox.a,
        scene.transform.world_to_screen(expected_world_a),
        "Falcon Up-B render capsules must use the same decomp TransN reset as the core fighter root, not an additional live-root offset"
    );
}

#[test]
fn special_air_hi_hit_collision_uses_melee_transn_reset() {
    preload_runtime_source_frame_data().expect("runtime source frame data should preload");
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.motion_state = MotionState::SpecialAirHi;
    player.motion_frame = 13;
    player.motion_anim_frame_milli = 13_000;
    player.source_position = SourceVec2 { x: 12.0, y: 24.0 };
    player.position = player.source_position.to_milli();
    player.facing = 1;
    player.grounded = false;
    assert!(world.set_player_state_for_diagnostic(0, player));
    let frame = RenderFrame::from_world(&world);

    let rendered = RenderScene::from_frame(&frame, 960, 540).player_hitbox_pills[0][0];
    let collision = source_collision_frame_from_frame(&frame)
        .hits
        .into_iter()
        .find(|hit| hit.owner_index == 0)
        .expect("Falcon Up-B should expose its active source catch capsule");
    let transn = mole_core::source_root_motion_position(
        MotionState::SpecialAirHi,
        frame.player_source_pose_frames[0],
    )
    .expect("SpecialAirHi source pose should expose TransN root position");
    let expected_b_x = frame.player_source_positions[0].x * 1_000.0
        + (rendered.source.b.x as f32 - transn.z) * 1_000.0;
    let expected_b_y = frame.player_source_positions[0].y * 1_000.0
        + (rendered.source.b.y as f32 - transn.y) * 1_000.0;

    assert!(
        (collision.capsule.b.x - expected_b_x).abs() <= 0.5
            && (collision.capsule.b.y - expected_b_y).abs() <= 0.5,
        "ftAnim_8006E054 clears TransN before hit collision reads the live JObj pose; collision={:?}, expected_b=({expected_b_x}, {expected_b_y})",
        collision.capsule
    );
}

fn falcon_special_air_hi_render_frame(
    source_frame: u8,
    live_anim_frame: f32,
    public_position: Vec2,
    source_position: SourceVec2,
) -> RenderFrame {
    let world = World::for_two_players();
    let mut frame = RenderFrame::from_world(&world);
    frame.player_motion_states[0] = MotionState::SpecialAirHi;
    frame.player_state_frames[0] = source_frame.saturating_sub(1).into();
    frame.player_animation_frames[0] = source_frame;
    frame.player_grounded[0] = false;
    frame.player_facings[0] = 1;
    frame.player_positions[0] = public_position;
    frame.player_source_positions[0] = source_position;
    frame.player_source_previous_positions[0] = source_position;
    frame.player_source_pose_motion_states[0] = MotionState::SpecialAirHi;
    frame.player_source_pose_action_state_ids[0] = Some(MeleeActionStateId::new(354));
    frame.player_source_pose_action_keys[0] = Some(SourceActionKey::new("SpecialAirHi"));
    frame.player_source_action_keys[0] = Some(SourceActionKey::new("SpecialAirHi"));
    frame.player_source_pose_frames[0] = source_frame;
    frame.player_source_motion_anim_frames[0] = live_anim_frame;
    frame.player_source_pose_model_facings[0] = 1;
    frame
}

#[test]
fn special_air_hi_hurt_pills_render_from_source_root_not_stale_public_root() {
    let source_frame = 6;
    let source_position = SourceVec2 { x: 32.0, y: 18.0 };
    let frame = falcon_special_air_hi_render_frame(
        source_frame,
        f32::from(source_frame.saturating_sub(1)),
        Vec2 { x: 0, y: 0 },
        source_position,
    );

    let scene = RenderScene::from_frame(&frame, 960, 540);
    let first_hurtbox = scene.player_hurtbox_pills[0]
        .first()
        .copied()
        .expect("SpecialAirHi should render source hurt capsules");
    let transn = mole_core::source_root_motion_position(MotionState::SpecialAirHi, source_frame)
        .expect("SpecialAirHi should expose TransN root position");
    let expected_world_a = Vec2 {
        x: source_position.to_milli().x
            + ((first_hurtbox.source.a.x - f64::from(transn.z)) * 1_000.0).round() as i32,
        y: source_position.to_milli().y
            + ((first_hurtbox.source.a.y - f64::from(transn.y)) * 1_000.0).round() as i32,
    };

    assert_eq!(
        first_hurtbox.a,
        scene.transform.world_to_screen(expected_world_a),
        "decomp source capsules are recomputed from the fighter/JObj world pose each frame; Falcon Up-B hurt pills must follow source_position even when the debug public position is stale"
    );
}

#[test]
fn special_air_hi_hit_pills_sample_live_source_anim_frame() {
    let first_active_frame = (1..=65)
        .find(|source_frame| {
            let frame = falcon_special_air_hi_render_frame(
                *source_frame,
                0.0,
                Vec2 { x: 0, y: 0 },
                SourceVec2 { x: 0.0, y: 0.0 },
            );
            !RenderScene::from_frame(&frame, 960, 540).player_hitbox_pills[0].is_empty()
        })
        .expect("SpecialAirHi should have at least one active hitbox frame");
    assert!(
        first_active_frame > 1,
        "test needs an inactive previous frame to prove live-frame sampling"
    );

    let stale_pose_frame = first_active_frame - 1;
    let frame = falcon_special_air_hi_render_frame(
        stale_pose_frame,
        f32::from(first_active_frame - 1),
        Vec2 { x: 0, y: 0 },
        SourceVec2 { x: 0.0, y: 0.0 },
    );
    let scene = RenderScene::from_frame(&frame, 960, 540);

    assert!(
        !scene.player_hitbox_pills[0].is_empty(),
        "ftColl updates active hit capsules from the live fighter pose; render must use the same live source animation frame instead of the stale integer pose frame"
    );
}

#[test]
fn special_air_hi_command_grab_hit_pills_use_catch_element_color() {
    let first_active_frame = (1..=65)
        .find(|source_frame| {
            let frame = falcon_special_air_hi_render_frame(
                *source_frame,
                0.0,
                Vec2 { x: 0, y: 0 },
                SourceVec2 { x: 0.0, y: 0.0 },
            );
            RenderScene::from_frame(&frame, 960, 540).player_hitbox_pills[0]
                .iter()
                .any(|pill| pill.color == RenderColor::COMMAND_GRAB_PILL)
        })
        .expect("SpecialAirHi catch-element hitboxes should render as command-grab pills");

    let frame = falcon_special_air_hi_render_frame(
        first_active_frame,
        f32::from(first_active_frame.saturating_sub(1)),
        Vec2 { x: 0, y: 0 },
        SourceVec2 { x: 0.0, y: 0.0 },
    );
    let scene = RenderScene::from_frame(&frame, 960, 540);

    assert!(
        scene.player_hitbox_pills[0]
            .iter()
            .all(|pill| pill.color == RenderColor::COMMAND_GRAB_PILL),
        "Falcon Dive's action script marks its hitboxes with SOURCE_HIT_ELEMENT_CATCH; the debug overlay should distinguish command grabs from ordinary damaging hitboxes without changing collision data"
    );
}

#[test]
fn special_air_hi_hit_pills_render_current_source_pose_not_previous_sweep_root() {
    let first_active_frame = (1..=65)
        .find(|source_frame| {
            let frame = falcon_special_air_hi_render_frame(
                *source_frame,
                0.0,
                Vec2 { x: 0, y: 0 },
                SourceVec2 { x: 0.0, y: 0.0 },
            );
            !RenderScene::from_frame(&frame, 960, 540).player_hitbox_pills[0].is_empty()
        })
        .expect("SpecialAirHi should have at least one active hitbox frame");
    assert!(
        first_active_frame > 1,
        "test needs a previous source frame so stale sweep-root usage can move render pills"
    );
    let render_frame = first_active_frame + 1;
    let current_source_position = SourceVec2 { x: 25.0, y: 40.0 };
    let mut current_pose_frame = falcon_special_air_hi_render_frame(
        render_frame,
        f32::from(render_frame - 1),
        Vec2 { x: 0, y: 0 },
        current_source_position,
    );
    current_pose_frame.player_source_previous_positions[0] = current_source_position;
    let mut stale_previous_frame = current_pose_frame;
    stale_previous_frame.player_source_previous_positions[0] = SourceVec2 {
        x: -200.0,
        y: -150.0,
    };

    let current_pose_scene = RenderScene::from_frame(&current_pose_frame, 960, 540);
    let stale_previous_scene = RenderScene::from_frame(&stale_previous_frame, 960, 540);

    assert_eq!(
        current_pose_scene.player_hitbox_pills[0][0],
        stale_previous_scene.player_hitbox_pills[0][0],
        "debug hitbox pills should render from the current decomp/JObj pose; swept previous-root state belongs to collision, not the visual overlay"
    );
    assert_eq!(
        current_pose_scene.player_hitbox_pills[0][0].source.a,
        current_pose_scene.player_hitbox_pills[0][0].source.b,
        "Melee action hitboxes are current-pose spheres for debug rendering; the previous->current capsule sweep belongs to collision resolution"
    );
}

#[test]
fn looping_locomotion_source_capsules_wrap_baked_pose_frames() {
    let world = World::for_two_players();
    let mut frame = RenderFrame::from_world(&world);
    frame.player_motion_states[0] = MotionState::WalkSlow;
    frame.player_source_pose_motion_states[0] = MotionState::WalkSlow;
    frame.player_source_pose_action_state_ids[0] = Some(MeleeActionStateId::new(15));
    frame.player_action_state_ids[0] = Some(MeleeActionStateId::new(15));
    frame.player_source_pose_action_keys[0] = Some(SourceActionKey::new("WalkSlow"));
    frame.player_source_action_keys[0] = Some(SourceActionKey::new("WalkSlow"));
    frame.player_source_pose_model_facings[0] = 1;
    frame.player_source_positions[0] = SourceVec2 { x: 0.0, y: 0.0 };
    frame.player_source_previous_positions[0] = frame.player_source_positions[0];

    frame.player_source_motion_anim_frames[0] = 0.0;
    frame.player_source_pose_frames[0] = 1;
    let first_cycle_scene = RenderScene::from_frame(&frame, 960, 540);

    frame.player_source_motion_anim_frames[0] = 54.0;
    frame.player_source_pose_frames[0] = 54;
    let wrapped_scene = RenderScene::from_frame(&frame, 960, 540);

    assert_eq!(
        first_cycle_scene.player_hurtbox_pills[0],
        wrapped_scene.player_hurtbox_pills[0],
        "ftWalkCommon leaves animation playback running; source debug capsules must wrap WalkSlow's baked FigaTree frames instead of clamping to the final pose after one cycle"
    );
}

#[test]
fn attack100_loop_hit_pills_render_from_decoded_subroutine_script() {
    let world = World::for_two_players();
    let mut frame = RenderFrame::from_world(&world);
    frame.player_motion_states[0] = MotionState::Attack1;
    frame.player_source_pose_action_state_ids[0] = Some(MeleeActionStateId::new(48));
    frame.player_source_pose_action_keys[0] = Some(SourceActionKey::new("Attack100Loop"));
    frame.player_source_action_keys[0] = Some(SourceActionKey::new("Attack100Loop"));
    frame.player_source_pose_frames[0] = 4;
    frame.player_source_motion_anim_frames[0] = 3.0;
    frame.player_source_positions[0] = SourceVec2 { x: 0.0, y: 0.0 };
    frame.player_source_previous_positions[0] = frame.player_source_positions[0];

    let scene = RenderScene::from_frame(&frame, 960, 540);

    assert_eq!(
        scene.player_hitbox_pills[0].len(),
        2,
        "Falcon rapid jab loop hitboxes live behind lbcommand Command_05 subroutine calls and must be present in baked runtime source data"
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
    let source_frame = frame.player_source_motion_anim_frames[0].floor().max(0.0) as u8 + 1;
    let transn = mole_core::source_root_motion_position(MotionState::TurnRun, source_frame)
        .expect("TurnRun source pose should expose TransN root position");
    let expected_world_a = Vec2 {
        x: frame.player_source_positions[0].to_milli().x
            + ((first_hurtbox.source.a.x - f64::from(transn.z)) * 1_000.0).round() as i32,
        y: frame.player_source_positions[0].to_milli().y
            + ((first_hurtbox.source.a.y - f64::from(transn.y)) * 1_000.0).round() as i32,
    };

    assert_eq!(
        first_hurtbox.a,
        scene.transform.world_to_screen(expected_world_a)
    );
}

#[test]
fn turn_run_capsules_reset_projected_transn_z_not_raw_transn_x() {
    let world = World::for_two_players();
    let mut frame = RenderFrame::from_world(&world);
    let source_frame = 10;
    let source_position = SourceVec2 { x: 68.4, y: 0.0 };
    frame.player_motion_states[0] = MotionState::TurnRun;
    frame.player_positions[0] = source_position.to_milli();
    frame.player_source_positions[0] = source_position;
    frame.player_source_previous_positions[0] = source_position;
    frame.player_facings[0] = 1;
    frame.player_turn_run_accel_mul[0] = 1;
    frame.player_source_pose_motion_states[0] = MotionState::TurnRun;
    frame.player_source_pose_action_state_ids[0] = Some(MeleeActionStateId::new(19));
    frame.player_source_pose_action_keys[0] = Some(SourceActionKey::new("TurnRun"));
    frame.player_source_action_keys[0] = Some(SourceActionKey::new("TurnRun"));
    frame.player_source_pose_frames[0] = source_frame;
    frame.player_source_motion_anim_frames[0] = f32::from(source_frame - 1);
    frame.player_source_pose_model_facings[0] = 1;

    let scene = RenderScene::from_frame(&frame, 960, 540);
    let first_hurtbox = scene.player_hurtbox_pills[0][0];
    let transn = mole_core::source_root_motion_position(MotionState::TurnRun, source_frame)
        .expect("TurnRun source pose should expose TransN root position");
    assert_eq!(transn.x, 0.0);
    assert!(
        transn.z > 1.0,
        "test needs TurnRun's projected horizontal TransN root, got {transn:?}"
    );
    let expected_world_a = Vec2 {
        x: source_position.to_milli().x
            + ((first_hurtbox.source.a.x - f64::from(transn.z)) * 1_000.0).round() as i32,
        y: source_position.to_milli().y
            + ((first_hurtbox.source.a.y - f64::from(transn.y)) * 1_000.0).round() as i32,
    };

    assert_eq!(
        first_hurtbox.a,
        scene.transform.world_to_screen(expected_world_a),
        "TurnRun source capsules are sampled after ftPartSetRotY projects TransN.z into view-space X; rendering must reset against that projected root instead of raw TransN.x"
    );
}

#[test]
fn cliffcatch_source_capsules_render_after_full_transn_reset() {
    let world = visual_platform_ledge_probe_world();
    let frame = RenderFrame::from_world(&world);
    let scene = RenderScene::from_frame(&frame, 960, 540);
    let first_hurtbox = scene.player_hurtbox_pills[1]
        .first()
        .copied()
        .expect("CliffCatch should emit source hurt capsules");
    let transn = mole_core::source_root_motion_position(
        MotionState::CliffCatch,
        frame.player_source_pose_frames[1],
    )
    .expect("CliffCatch source pose should expose TransN root position");
    let facing_sign = if frame.player_source_pose_model_facings[1] < 0 {
        -1.0
    } else {
        1.0
    };
    let expected_world_a = Vec2 {
        x: frame.player_positions[1].x
            + ((first_hurtbox.source.a.x - f64::from(transn.z)) * facing_sign * 1_000.0).round()
                as i32,
        y: frame.player_positions[1].y
            + ((first_hurtbox.source.a.y - f64::from(transn.y)) * 1_000.0).round() as i32,
    };

    assert_eq!(
        first_hurtbox.a,
        scene.transform.world_to_screen(expected_world_a)
    );
}

#[test]
fn cliffcatch_source_collision_capsules_use_full_transn_reset() {
    let world = visual_platform_ledge_probe_world();
    let frame = RenderFrame::from_world(&world);
    let scene = RenderScene::from_frame(&frame, 960, 540);
    let first_rendered_hurtbox = scene.player_hurtbox_pills[1]
        .first()
        .copied()
        .expect("CliffCatch should emit source hurt capsules");
    let transn = mole_core::source_root_motion_position(
        MotionState::CliffCatch,
        frame.player_source_pose_frames[1],
    )
    .expect("CliffCatch source pose should expose TransN root position");
    let first_collision_hurtbox = source_collision_frame_from_frame(&frame)
        .hurts
        .into_iter()
        .find(|hurtbox| hurtbox.owner_index == 1)
        .expect("CliffCatch should emit source collision hurt capsules");
    let expected_y = frame.player_positions[1].y as f32
        + (first_rendered_hurtbox.source.a.y as f32 - transn.y) * 1_000.0;

    assert!(
        (first_collision_hurtbox.capsule.a.y - expected_y).abs() <= 0.5,
        "source collision capsule y should reset the same live TransN root as the render overlay"
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

    assert_eq!(frame.player_ecbs[0].bottom, Vec2 { x: 1_000, y: 3_998 });
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
fn sdl_runtime_launcher_uses_local_sdl_play_mode() {
    let launcher = project_asset_root()
        .join("execs")
        .join("Run SDL3 Runtime.cmd");
    let text =
        std::fs::read_to_string(&launcher).expect("SDL3 runtime launcher should be readable");

    assert!(text.contains(".local\\SDL3"));
    assert!(text.contains("--features \"sdl wup\""));
    assert!(text.contains("-- --sdl --play --input-trace"));
    assert!(!text.contains("--friend-connect"));
    assert!(!text.contains("--netplay-delay"));
    assert!(!text.contains("--no-ucf"));
    assert!(!text.contains("--frames 600"));
}

#[test]
fn gameplay_launch_paths_do_not_require_wup_adapter_at_startup() {
    let source =
        std::fs::read_to_string(project_asset_root().join("crates/mole_runtime/src/main.rs"))
            .expect("runtime main source should be readable");

    assert!(source.contains("fn open_optional_wup_input_source("));
    assert!(source.contains("fn retry_optional_wup_input_source("));
    assert!(source.contains("fn poll_optional_wup_inputs("));
    assert!(source.contains("fn poll_traced_wup_inputs("));
    assert!(source.contains("input_source: &mut Option<WupInputSource>,"));
    assert!(
        !source.contains(
            "let mut gameplay_input_source =\n        WupInputSource::open_with_config(mole_runtime::WupInputConfig { ucf_enabled })?;"
        ),
        "local SDL gameplay startup must not fail just because WUP is absent"
    );
    assert!(
        !source.contains(
            "let mut local_input_source =\n        WupInputSource::open_with_config(mole_runtime::WupInputConfig { ucf_enabled })?;"
        ),
        "UDP SDL gameplay startup must not fail just because WUP is absent"
    );
}

#[test]
fn friend_connect_main_debug_overlay_is_opt_in_for_playtest_performance() {
    let source =
        std::fs::read_to_string(project_asset_root().join("crates/mole_runtime/src/main.rs"))
            .expect("runtime main source should be readable");

    assert!(source.contains("debug_overlay: bool,"));
    assert!(source.contains("let overlay = debug_overlay.then(|| match &network"));
    let friend_connect_body = source
        .split("fn run_friend_connect_sdl(")
        .nth(1)
        .and_then(|body| body.split("fn handle_friend_connect_event(").next())
        .expect("Friend Connect SDL body should be present");
    assert!(!friend_connect_body.contains("Some(&overlay)"));
    assert!(friend_connect_body.contains("overlay.as_ref()"));
}

#[test]
fn visual_slippi_replay_runtime_uses_zero_delay_diagnostic_stop_gate() {
    let source =
        std::fs::read_to_string(project_asset_root().join("crates/mole_runtime/src/main.rs"))
            .expect("runtime main source should be readable");
    let sdl_runtime_body = source
        .split("fn run_sdl_smoke(")
        .nth(1)
        .and_then(|body| body.split("fn write_visual_slippi_divergence_log(").next())
        .expect("SDL runtime body should be present");

    assert!(
        sdl_runtime_body.contains("SlippiVisualReplayDivergenceGate::new(0)"),
        "visual replay should not add a realignment delay; mixed-phase/cascade suppression belongs inside the diagnostic stop gate"
    );
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
    assert!(!text.contains("--friend-connect"));
    assert!(!text.contains("--netplay-delay"));
    assert!(text.contains("Open Dev Tool.cmd"));
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
    assert_eq!(scene.players[0].width, 27);
    assert_eq!(scene.players[0].height, 60);
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
fn render_scene_uses_source_animation_frame_for_legacy_sprite_cue() {
    let world = World::for_two_players();
    let mut frame = RenderFrame::from_world(&world);
    frame.player_motion_states[0] = MotionState::Wait;
    frame.player_state_frames[0] = 0;
    frame.player_animation_frames[0] = 1;

    let scene = RenderScene::from_frame(&frame, 960, 540);

    assert_eq!(
        scene.player_sprites[0].animation,
        LegacyAnimationKey::Standing
    );
    assert_eq!(scene.player_sprites[0].frame, "Standing2.png");
}

#[test]
fn render_scene_source_hurtboxes_prefer_active_common_pose_over_stale_common_action_id() {
    let world = World::for_two_players();
    let wait_frame = RenderFrame::from_world(&world);
    let wait_scene = RenderScene::from_frame(&wait_frame, 960, 540);
    let wait_hurtboxes = wait_scene.player_hurtbox_pills[0].clone();
    assert!(!wait_hurtboxes.is_empty());

    let mut stale_frame = wait_frame.clone();
    stale_frame.player_motion_states[0] = MotionState::Wait;
    stale_frame.player_source_pose_motion_states[0] = MotionState::Wait;
    stale_frame.player_source_pose_action_state_ids[0] = Some(MeleeActionStateId::new(29));
    stale_frame.player_source_pose_action_keys[0] = Some(SourceActionKey::new("Wait1"));
    stale_frame.player_source_action_keys[0] = Some(SourceActionKey::new("Fall"));
    let stale_scene = RenderScene::from_frame(&stale_frame, 960, 540);

    assert_eq!(
        stale_scene.player_hurtbox_pills[0], wait_hurtboxes,
        "common-state source hurtboxes should follow the active JObj pose binding; stale Fall action identity must not make standing characters render Fall capsules"
    );
}

#[test]
fn render_scene_source_hurtboxes_project_decomp_x_axis_for_standing_pose() {
    let world = World::for_two_players();
    let frame = RenderFrame::from_world(&world);
    let scene = RenderScene::from_frame(&frame, 960, 540);
    let first_hurtbox = scene.player_hurtbox_pills[0]
        .first()
        .copied()
        .expect("Wait1 should render source hurt capsules");
    let transn = mole_core::source_root_motion_position(
        MotionState::Wait,
        frame.player_source_pose_frames[0],
    )
    .unwrap_or(Vec3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    });
    let facing_sign = if frame.player_source_pose_model_facings[0] < 0 {
        -1.0
    } else {
        1.0
    };
    let expected_world_a = Vec2 {
        x: frame.player_positions[0].x
            + ((first_hurtbox.source.a.x - f64::from(transn.z)) * facing_sign * 1_000.0).round()
                as i32,
        y: frame.player_positions[0].y
            + ((first_hurtbox.source.a.y - f64::from(transn.y)) * 1_000.0).round() as i32,
    };

    assert_eq!(
        first_hurtbox.a,
        scene.transform.world_to_screen(expected_world_a),
        "ftdrawcommon/lbColl draw source capsules through x/y world positions and force z to fighter->cur_pos.z; the 2D parity overlay must not use source z as screen X"
    );
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
        [
            "P1 A15 WALKSLOW F0 D0 K0.0 H0 S0".to_string(),
            "P2 A20 DASH F0 D0 K0.0 H0 S0".to_string(),
        ]
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
fn frame_debug_log_reports_canonical_source_hit_confirms() {
    let world = World::for_two_players();
    let inputs = [PlayerInput::neutral(), PlayerInput::neutral()];
    let mut frame = RenderFrame::from_world(&world);
    frame.player_positions[1] = frame.player_positions[0];
    frame.player_source_positions[1] = frame.player_source_positions[0];
    frame.player_source_previous_positions[1] = frame.player_source_positions[1];
    frame.player_motion_states[0] = MotionState::Wait;
    frame.player_source_pose_motion_states[0] = MotionState::Wait;
    frame.player_source_pose_action_state_ids[0] = Some(MeleeActionStateId::new(65));
    frame.player_source_action_keys[0] = Some(SourceActionKey::new("AttackAirN"));
    frame.player_animation_frames[0] = 7;
    frame.player_source_pose_frames[0] = 7;
    frame.player_source_pose_model_facings[0] = 1;
    frame.player_grounded[1] = false;
    let scene = RenderScene::from_frame(&frame, 960, 540);

    let line = FrameDebugLog::from_frame_and_scene(&frame, &scene, inputs).to_json_line();
    let parsed: serde_json::Value = serde_json::from_str(&line).expect("debug log must be json");
    let confirms = parsed["source_hit_confirms"]
        .as_array()
        .expect("debug log should expose source hit confirms");
    let hitbox_one = confirms
        .iter()
        .find(|confirm| confirm["attacker_index"] == 0 && confirm["hitbox_id"] == 1)
        .expect("AttackAirN source frame 7 should log hitbox id 1 confirm");

    assert_eq!(hitbox_one["victim_index"], 1);
    assert_eq!(hitbox_one["action_state_id"], 65);
    assert_eq!(hitbox_one["source_action_key"], "AttackAirN");
    assert_eq!(hitbox_one["source_frame"], 7);
    assert_eq!(hitbox_one["hitbox"]["damage"], 5);
    assert_eq!(hitbox_one["hitbox"]["angle"], 78);
    assert_eq!(hitbox_one["hitbox"]["knockback_growth"], 100);
    assert_eq!(hitbox_one["hitbox"]["weight_set_knockback"], 40);
    assert_eq!(hitbox_one["hitbox"]["base_knockback"], 0);
    assert_eq!(hitbox_one["hitbox"]["element"], 0);
    assert_eq!(hitbox_one["hitbox"]["shield_damage"], 0);
    assert_eq!(hitbox_one["hitbox"]["hit_grounded"], true);
    assert_eq!(hitbox_one["hitbox"]["hit_aerial"], true);

    let damage_stages = parsed["source_damage_stages"]
        .as_array()
        .expect("debug log should expose source damage stages");
    let damage_stage = damage_stages
        .iter()
        .find(|stage| stage["attacker_index"] == 0 && stage["hitbox_id"] == 1)
        .expect("AttackAirN source frame 7 should log hitbox id 1 damage stage");
    assert_eq!(damage_stage["victim_index"], 1);
    assert_eq!(damage_stage["damage"], 5.0);
    assert_eq!(damage_stage["env_damage"], 5);
    assert_eq!(damage_stage["unk_count"], 5);
    assert_eq!(damage_stage["source_action_key"], "AttackAirN");

    let damage_results = parsed["source_damage_results"]
        .as_array()
        .expect("debug log should expose selected source damage results");
    let damage_result = damage_results
        .iter()
        .find(|result| result["victim_index"] == 1)
        .expect("AttackAirN source frame 7 should log selected player two damage result");
    assert_eq!(damage_result["attacker_index"], 0);
    assert_eq!(damage_result["source_action_key"], "AttackAirN");
    assert_eq!(damage_result["angle"], damage_result["hitbox"]["angle"]);
    assert_eq!(damage_result["element"], damage_result["hitbox"]["element"]);
    assert!(
        damage_result["knockback"]
            .as_f64()
            .expect("damage result knockback should be numeric")
            > 0.0
    );
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
        .contains(&"UDP TX 1 RX 1 DUP 1 MISS 1 RB 0".to_string()));
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
fn udp_runtime_stats_do_not_move_ack_or_remote_sequence_backwards() {
    let mut stats = UdpRuntimeStats::default();
    let newest =
        InputPacket::new(Frame(12), 1, PlayerInput::neutral(), 12).with_timing_probe(12, 8);
    let older = InputPacket::new(Frame(9), 1, PlayerInput::neutral(), 9).with_timing_probe(9, 5);

    stats.record_accept_at(Frame(13), PacketAcceptResult::Accepted, newest);
    stats.record_accept_at(Frame(13), PacketAcceptResult::Accepted, older);

    assert_eq!(stats.last_remote_frame, Some(Frame(12)));
    assert_eq!(stats.last_remote_sequence, Some(12));
    assert_eq!(stats.last_acked_sequence, Some(8));
    assert_eq!(stats.last_rtt_frames, Some(5));
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
fn input_trace_writer_stops_before_configured_size_cap() {
    let dir = std::env::temp_dir().join(format!(
        "mole-runtime-input-trace-cap-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos()
    ));
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
    let first_line_bytes = log.to_json_line().len() as u64 + 1;
    let mut writer = InputTraceWriter::create_in_dir_with_limits(&dir, first_line_bytes, 8)
        .expect("input trace writer should open a capped JSONL file");

    writer
        .write_line(&log)
        .expect("first trace line should fit exactly at the cap");
    writer
        .write_line(&log)
        .expect("extra trace lines past the cap should be skipped without crashing gameplay");

    let text = std::fs::read_to_string(writer.path()).expect("trace file should be readable");
    assert_eq!(text.lines().count(), 1);
    assert!(std::fs::metadata(writer.path()).unwrap().len() <= first_line_bytes);
}

#[test]
fn input_trace_writer_prunes_old_trace_files_before_creating_new_one() {
    let dir = std::env::temp_dir().join(format!(
        "mole-runtime-input-trace-prune-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("trace dir should be creatable");
    for index in 0..5 {
        std::fs::write(
            dir.join(format!("controller-input-trace-{index}.jsonl")),
            "{}\n",
        )
        .expect("seed trace should be writable");
    }
    std::fs::write(dir.join("input-debug-old.jsonl"), "{}\n")
        .expect("non-controller debug logs are owned by older tooling");

    let writer = InputTraceWriter::create_in_dir_with_limits(&dir, 1024, 3)
        .expect("input trace writer should prune and open a JSONL file");
    drop(writer);

    let mut controller_traces = std::fs::read_dir(&dir)
        .expect("trace dir should be readable")
        .map(|entry| entry.expect("dir entry should be readable").path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("controller-input-trace-"))
        })
        .collect::<Vec<_>>();
    controller_traces.sort();

    assert_eq!(controller_traces.len(), 3);
    assert!(!dir.join("controller-input-trace-0.jsonl").exists());
    assert!(!dir.join("controller-input-trace-1.jsonl").exists());
    assert!(dir.join("input-debug-old.jsonl").exists());
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

#[test]
fn render_and_source_collision_skip_slots_without_source_player_state_or_stocks() {
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let mut player = world.players()[0];
    player.player_state = PLAYER_STATE_NONE;
    player.stocks = 0;
    assert!(world.set_player_state_for_diagnostic(0, player));

    let frame = RenderFrame::from_world(&world);
    let collision_frame = source_collision_frame_from_frame(&frame);
    assert!(collision_frame.hits.iter().all(|hit| hit.owner_index != 0));
    assert!(collision_frame
        .hurts
        .iter()
        .all(|hurt| hurt.owner_index != 0));

    let scene = RenderScene::from_frame(&frame, 960, 540);
    assert_eq!(scene.players[0].width, 0);
    assert_eq!(scene.players[0].height, 0);
    assert_eq!(scene.players[0].color, RenderColor::TRANSPARENT);
    assert_eq!(scene.player_ecbs[0].color, RenderColor::TRANSPARENT);
    assert!(scene.player_hitbox_pills[0].is_empty());
    assert!(scene.player_hurtbox_pills[0].is_empty());
}
