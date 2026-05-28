use std::path::Path;

use mole_core::{
    step_world, FighterProfile, Frame, GameCubeButtonState, GameCubePadStatus, MotionState,
    PlayerInput, Vec2, WalkSpeedBucket, World,
};
use mole_runtime::{
    legacy_animation_for_motion_state, map_gamecube_pad_to_player_input, map_physical_input,
    native_replay_path, parse_wup_report, DebugOverlay, DolphinMoleVisualProfile, FixedStepClock,
    InputReadout, InputSource, LegacyAnimationKey, LegacySpriteCue, PhysicalInput, RenderColor,
    RenderFrame, RenderRect, RenderScene, RenderTransform, ReplayCapture, UdpRuntimeConfig,
    UdpRuntimeStats, WupInputMapper, WupPort, LEGACY_DOLPHIN_MOLE_ANIMATIONS,
};
use mole_transport::{InputPacket, PacketAcceptResult};

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
    assert_eq!(world.players()[0].motion_state, MotionState::WalkMiddle);
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

    let core_snapshot = world.snapshot();
    let mut render_frame = RenderFrame::from_snapshot(core_snapshot);

    assert_eq!(render_frame.frame, core_snapshot.frame);
    assert_eq!(
        render_frame.player_motion_states[0],
        MotionState::WalkMiddle
    );
    assert_eq!(
        render_frame.player_state_frames[0],
        core_snapshot.players[0].state_frame
    );
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
            x: 448,
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
            x: 458,
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
                left_trigger: 72,
                buttons: GameCubeButtonState::empty(),
                ..GameCubePadStatus::neutral()
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ]);

    assert_eq!(moved[0].stick_x(), 124);
    assert_eq!(moved[0].stick_y(), -125);
    assert_eq!(moved[0].left_trigger_analog(), 65);
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

    assert_eq!(moved[0].stick_x(), 101);
    assert_eq!(moved[0].stick_y(), -105);
    assert_eq!(moved[0].c_stick_x(), -107);
    assert_eq!(moved[0].c_stick_y(), 102);
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

    assert_eq!(moved[0].c_stick_x(), -107);
    assert_eq!(moved[0].c_stick_y(), 102);
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
                left_trigger: 72,
                right_trigger: 205,
                buttons: GameCubeButtonState::empty().with_r(true),
                ..GameCubePadStatus::neutral()
            },
        },
        WupPort::default(),
        WupPort::default(),
        WupPort::default(),
    ]);

    assert_eq!(moved[0].left_trigger_analog(), 65);
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
                left_trigger: 72,
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
    assert_eq!(moved_p1.lstick, (124, -125));
    assert_eq!(moved_p1.cstick, (127, -128));
    assert_eq!(moved_p1.left_trigger, 65);
    assert_eq!(moved_p1.right_trigger, 0);
    assert!(moved_p1.pressed.a());
    assert!(moved_p1.shield_pressed);
    assert_eq!(moved_p1.x_tap_timer, 0);
    assert_eq!(moved_p1.trigger_timer, 0xfe);

    let held = mapper.map_ports_to_melee_snapshots(moved_ports);
    let held_p1 = held[0].expect("connected port should produce player one snapshot");

    assert_eq!(held_p1.prev_lstick, (124, -125));
    assert_eq!(held_p1.lstick, (124, -125));
    assert!(held_p1.held.a());
    assert!(!held_p1.pressed.a());
    assert!(!held_p1.shield_pressed);
    assert_eq!(held_p1.x_tap_timer, 1);
    assert_eq!(held_p1.trigger_timer, 0xfe);
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
fn wup_input_mapper_carries_ucf_tilt_intent_facts_from_native_history() {
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

    assert!(p1.ucf_x_tilt_intent);
    assert_eq!(p1.facts(Default::default()).ucf_dashback_direction, 1);
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
                stick_y: 0,
                c_stick_x: 255,
                c_stick_y: 0,
                left_trigger: 72,
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
    assert!(line.contains("\"ucf_version\":\"0.84\""));
    assert!(line.contains("\"lstick_x\":124"));
    assert!(line.contains("\"lstick_y\":-125"));
    assert!(line.contains("\"prev_lstick_x\":0"));
    assert!(line.contains("\"x_tap_timer\":0"));
    assert!(line.contains("\"trigger_timer\":254"));
    assert!(line.contains("\"trigger_timer\":254,\"left_trigger\":65,\"right_trigger\":0,"));
    assert!(line.contains("\"tilt_direction_x\":1"));
    assert!(line.contains("\"horizontal_smash_direction\":1"));
    assert!(line.contains("\"dash_direction\":1"));
    assert!(line.contains("\"ucf_x_tilt_intent\":true"));
    assert!(line.contains("\"ucf_dashback_direction\":1"));
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
    assert!(line.contains("\"fast_fall\":true"));
    assert!(line.contains("\"attack_pressed\":false"));
    assert!(line.contains("\"neutral_attack_pressed\":false"));
    assert!(line.contains("\"smash_attack_direction_x\":0"));
    assert!(line.contains("\"shield_pressed\":true"));
    assert!(line.contains("\"analog_shield\":65"));
    assert!(line.contains("\"analog_shield_pressed\":true"));
    assert!(line.contains("\"digital_shield_pressed\":false"));
    assert!(line.contains("\"air_dodge_pressed\":false"));
    assert!(line.contains("\"left_trigger_analog_held\":true"));
    assert!(line.contains("\"right_trigger_analog_held\":false"));
    assert!(line.contains("\"left_trigger_analog_pressed\":true"));
    assert!(line.contains("\"right_trigger_analog_pressed\":false"));
    assert!(line.contains("\"cstick_x\":127"));
    assert!(line.contains("\"cstick_y\":-128"));
    assert!(line.contains("\"cstick_smash_direction_x\":1"));
    assert!(line.contains("\"cstick_smash_direction_y\":0"));
}

struct ScriptedInputSource {
    input: PlayerInput,
}

impl InputSource for ScriptedInputSource {
    fn poll_inputs(&mut self, _frame: Frame) -> [PlayerInput; 2] {
        [self.input, PlayerInput::neutral()]
    }
}
