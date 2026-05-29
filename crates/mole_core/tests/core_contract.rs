use mole_core::{
    input_common_data_field_sources, melee_units, melee_units_f32, step_world,
    CommonDataExtractError, CommonDataProvenance, EcbDiamond, FighterProfile,
    FighterProfileExtractError, Frame, GameCubeButtonState, GameCubePadStatus, MeleeCommonData,
    MeleeInputConfig, MeleeInputProcessor, MeleeInputSnapshot, MeleeInputThresholds,
    MeleeInputTimers, MeleeJumpInput, MotionState, PlayerInput, StageProfile, StageSurfaceKind,
    Vec2, WalkSpeedBucket, World, TICK_RATE_HZ, UCF_CARDINAL_AXIS, UCF_CARDINAL_SNAP_RANGE,
    UCF_SHIELD_DROP_DELTA, UCF_TILT_INTENT_DELTA, UCF_VERSION,
};

fn squared_magnitude(velocity: Vec2) -> i32 {
    velocity.x * velocity.x + velocity.y * velocity.y
}

fn close_to(left: i32, right: i32, tolerance: i32) -> bool {
    (left - right).abs() <= tolerance
}

fn advance_player_to_run(world: &mut World) {
    let dash_right = [
        PlayerInput::neutral().with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];

    step_world(world, Frame(0), &dash_right);
    for frame in 1..=15 {
        step_world(world, Frame(frame), &dash_right);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::Run);
}

#[test]
fn simulation_rate_is_sixty_hertz() {
    assert_eq!(TICK_RATE_HZ, 60);
}

#[test]
fn melee_units_use_milli_units_for_public_falcon_values() {
    assert_eq!(melee_units(2.3), 2_300);
    assert_eq!(melee_units(0.13), 130);
}

#[test]
fn default_stage_is_battlefield_sized_in_core_units() {
    let stage = StageProfile::battlefield_test();

    assert_eq!(stage.name, "battlefield_test");
    assert_eq!(stage.main_floor.left_x, melee_units_f32(-68.4));
    assert_eq!(stage.main_floor.right_x, melee_units_f32(68.4));
    assert_eq!(stage.soft_platforms.len(), 3);
    assert!(stage
        .soft_platforms
        .iter()
        .all(|surface| surface.kind == StageSurfaceKind::Soft));
}

#[test]
fn default_two_player_spawns_are_separated_in_battlefield_units() {
    let world = World::for_two_players();

    assert_eq!(
        world.players()[0].position,
        Vec2 {
            x: melee_units_f32(-20.0),
            y: 0
        }
    );
    assert_eq!(
        world.players()[1].position,
        Vec2 {
            x: melee_units_f32(20.0),
            y: 0
        }
    );
    assert!(
        (world.players()[1].position.x - world.players()[0].position.x)
            > FighterProfile::falcon_like().standing_height_units
    );
}

#[test]
fn ecb_diamond_uses_four_midpoint_vertices() {
    let ecb = EcbDiamond::from_bottom_center_and_size(Vec2 { x: 100, y: 0 }, 62_000, 136_000);

    assert_eq!(ecb.top, Vec2 { x: 100, y: 136_000 });
    assert_eq!(
        ecb.right,
        Vec2 {
            x: 31_100,
            y: 68_000
        }
    );
    assert_eq!(ecb.bottom, Vec2 { x: 100, y: 0 });
    assert_eq!(
        ecb.left,
        Vec2 {
            x: -30_900,
            y: 68_000
        }
    );
    assert_eq!(ecb.points(), [ecb.top, ecb.right, ecb.bottom, ecb.left]);
}

#[test]
fn falcon_like_profile_exposes_public_falcon_gameplay_values() {
    let profile = FighterProfile::falcon_like();

    assert_eq!(profile.reference_character, "captain_falcon");
    assert_eq!(profile.run_speed_per_tick, 2_300);
    assert_eq!(profile.initial_dash_speed_per_tick, 2_000);
    assert_eq!(profile.dash_run_accel_stick_per_tick, 10);
    assert_eq!(profile.dash_run_accel_base_per_tick, 150);
    assert_eq!(profile.walk_speed_per_tick, 850);
    assert_eq!(profile.traction_per_tick, 80);
    assert_eq!(profile.ground_to_air_jump_momentum_milli, 800);
    assert_eq!(profile.jump_horizontal_initial_velocity_per_tick, 400);
    assert_eq!(profile.jump_horizontal_max_velocity_per_tick, 1_000);
    assert_eq!(profile.air_jump_horizontal_velocity_per_tick, 400);
    assert_eq!(profile.max_jumps, 1);
    assert_eq!(profile.air_drift_stick_accel_per_tick, 40);
    assert_eq!(profile.air_drift_base_accel_per_tick, 20);
    assert_eq!(profile.air_drift_max_velocity_per_tick, 1_120);
    assert_eq!(profile.air_friction_per_tick, 10);
    assert_eq!(profile.air_max_horizontal_velocity_per_tick, 1_120);
    assert_eq!(profile.gravity_per_tick, 130);
    assert_eq!(profile.fall_speed_per_tick, 2_900);
    assert_eq!(profile.fast_fall_speed_per_tick, 3_500);
    assert_eq!(profile.full_hop_jump_force_per_tick, 3_100);
    assert_eq!(profile.short_hop_jump_force_per_tick, 1_900);
    assert_eq!(profile.air_jump_force_per_tick, 2_790);
    assert_eq!(profile.full_hop_height, 38_520);
    assert_eq!(profile.short_hop_height, 14_850);
    assert_eq!(profile.double_jump_height, 28_560);
    assert_eq!(profile.standing_height_units, 22_667);
    assert_eq!(profile.jumpsquat_frames, 4);
    assert_eq!(profile.dash_frames, 15);
    assert_eq!(profile.normal_landing_lag_ticks, 4);
}

#[test]
fn extracted_ftco_dat_attrs_reads_big_endian_fighter_profile_fields() {
    let mut bytes = vec![0_u8; 0x188];

    put_f32_be(&mut bytes, 0x04, 0.02);
    put_f32_be(&mut bytes, 0x08, 0.85);
    put_f32_be(&mut bytes, 0x18, 0.08);
    put_f32_be(&mut bytes, 0x1c, 2.0);
    put_f32_be(&mut bytes, 0x20, 0.011);
    put_f32_be(&mut bytes, 0x24, 0.151);
    put_f32_be(&mut bytes, 0x28, 2.3);
    put_f32_be(&mut bytes, 0x34, 2.35);
    put_f32_be(&mut bytes, 0x38, 4.0);
    put_f32_be(&mut bytes, 0x3c, 0.44);
    put_f32_be(&mut bytes, 0x40, 3.1);
    put_f32_be(&mut bytes, 0x44, 0.81);
    put_f32_be(&mut bytes, 0x48, 1.05);
    put_f32_be(&mut bytes, 0x4c, 1.9);
    put_f32_be(&mut bytes, 0x50, 0.9);
    put_f32_be(&mut bytes, 0x54, 0.46);
    put_i32_be(&mut bytes, 0x58, 2);
    put_f32_be(&mut bytes, 0x5c, 0.13);
    put_f32_be(&mut bytes, 0x60, 2.9);
    put_f32_be(&mut bytes, 0x64, 0.047);
    put_f32_be(&mut bytes, 0x68, 0.023);
    put_f32_be(&mut bytes, 0x6c, 1.17);
    put_f32_be(&mut bytes, 0x70, 0.011);
    put_f32_be(&mut bytes, 0x74, 3.5);
    put_f32_be(&mut bytes, 0x78, 1.26);
    put_f32_be(&mut bytes, 0xe4, 5.0);

    let profile = FighterProfile::from_ftco_dat_attrs_bytes("captain_falcon", &bytes)
        .expect("synthetic ftCo_DatAttrs slice should extract");

    assert_eq!(profile.reference_character, "captain_falcon");
    assert_eq!(profile.walk_accel_per_tick, 20);
    assert_eq!(profile.walk_speed_per_tick, 850);
    assert_eq!(profile.traction_per_tick, 80);
    assert_eq!(profile.initial_dash_speed_per_tick, 2_000);
    assert_eq!(profile.dash_run_accel_stick_per_tick, 11);
    assert_eq!(profile.dash_run_accel_base_per_tick, 151);
    assert_eq!(profile.run_speed_per_tick, 2_300);
    assert_eq!(profile.ground_max_horizontal_velocity_per_tick, 2_350);
    assert_eq!(profile.jumpsquat_frames, 4);
    assert_eq!(profile.jump_horizontal_initial_velocity_per_tick, 440);
    assert_eq!(profile.ground_to_air_jump_momentum_milli, 810);
    assert_eq!(profile.jump_horizontal_max_velocity_per_tick, 1_050);
    assert_eq!(profile.full_hop_jump_force_per_tick, 3_100);
    assert_eq!(profile.short_hop_jump_force_per_tick, 1_900);
    assert_eq!(profile.air_jump_force_per_tick, 2_790);
    assert_eq!(profile.air_jump_horizontal_velocity_per_tick, 460);
    assert_eq!(profile.max_jumps, 2);
    assert_eq!(profile.gravity_per_tick, 130);
    assert_eq!(profile.fall_speed_per_tick, 2_900);
    assert_eq!(profile.air_drift_stick_accel_per_tick, 47);
    assert_eq!(profile.air_drift_base_accel_per_tick, 23);
    assert_eq!(profile.air_drift_max_velocity_per_tick, 1_170);
    assert_eq!(profile.air_friction_per_tick, 11);
    assert_eq!(profile.fast_fall_speed_per_tick, 3_500);
    assert_eq!(profile.air_max_horizontal_velocity_per_tick, 1_260);
    assert_eq!(profile.normal_landing_lag_ticks, 5);
}

#[test]
fn extracted_ftco_dat_attrs_reports_missing_source_field() {
    let bytes = vec![0_u8; 0x74];

    let err = FighterProfile::from_ftco_dat_attrs_bytes("captain_falcon", &bytes)
        .expect_err("slice ending before fast_fall_velocity should be rejected");

    assert_eq!(
        err,
        FighterProfileExtractError::TooShort {
            field: "fast_fall_velocity",
            offset: 0x74,
            required_len: 0x78,
            actual_len: 0x74,
        }
    );
}

#[test]
fn airborne_falcon_profile_uses_profile_gravity_and_fall_speed() {
    let mut world = World::for_two_players();
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    step_world(&mut world, Frame(0), &jump);

    for frame in 1..20 {
        step_world(&mut world, Frame(frame), &neutral);
        if !world.players()[0].grounded {
            let before = world.players()[0].velocity.y;
            step_world(&mut world, Frame(frame + 1), &neutral);
            let profile = FighterProfile::falcon_like();
            assert_eq!(
                world.players()[0].velocity.y,
                (before - profile.gravity_per_tick).max(-profile.fall_speed_per_tick)
            );
            return;
        }
    }

    panic!("expected Falcon profile jump to leave the ground");
}

#[test]
fn input_threshold_defaults_come_from_provisional_common_data() {
    let common = MeleeCommonData::provisional_mole();
    let thresholds = common.input_thresholds();

    assert_eq!(MeleeInputConfig::default(), common.input_config());
    assert_eq!(MeleeInputThresholds::default(), thresholds);
    assert_eq!(common.trigger_threshold, 1);
    assert_eq!(common.trigger_timer_threshold, 140);
    assert_eq!(thresholds.walk_x, 20);
    assert_eq!(common.walk_slow_x, 20);
    assert_eq!(common.walk_middle_x, 50);
    assert_eq!(common.walk_fast_x, 90);
    assert_eq!(thresholds.walk_slow_x, 20);
    assert_eq!(thresholds.walk_middle_x, 50);
    assert_eq!(thresholds.walk_fast_x, 90);
    assert_eq!(thresholds.dash_x, 80);
    assert_eq!(thresholds.dash_tap_window, 3);
    assert_eq!(common.z_shield_analog, 49);
    assert_eq!(thresholds.z_shield_analog, 49);
    assert_eq!(common.dash_early_action_window, 1);
    assert_eq!(common.dash_late_action_window, 15);
    assert_eq!(thresholds.tap_jump_y, 80);
    assert_eq!(thresholds.escape_x, 80);
    assert_eq!(thresholds.aerial_neutral_x, 40);
    assert_eq!(thresholds.aerial_neutral_y, 40);
    assert_eq!(thresholds.aerial_vertical_angle_tan_milli, 1000);
    assert_eq!(common.air_jump_backward_x, 20);
    assert_eq!(common.escapeair_iasa_timer_ticks, 15);
    assert_eq!(common.escapeair_animation_ticks, 20);
    assert_eq!(common.escapeair_deadzone_x, 20);
    assert_eq!(common.escapeair_deadzone_y, 20);
    assert_eq!(common.escapeair_force, 800);
    assert_eq!(common.escapeair_decay_percent, 90);
    assert_eq!(common.escapeair_landing_lag_ticks, 10);
    assert_eq!(common.guard_on_catch_dash_window, 4);
    assert_eq!(common.run_turn_run_no_interrupt_frames, 1);
}

#[test]
fn input_common_data_sources_track_melee_field_offsets() {
    let sources = input_common_data_field_sources();

    let dash = sources
        .iter()
        .find(|source| source.rust_name == "dash_x")
        .expect("dash_x common-data source should be recorded");
    assert_eq!(dash.source_name, "x3C");
    assert_eq!(dash.offset, 0x3c);
    assert_eq!(dash.provenance, CommonDataProvenance::ProvisionalMole);

    let walk_slow = sources
        .iter()
        .find(|source| source.rust_name == "walk_slow_x")
        .expect("walk_slow_x common-data source should be recorded");
    assert_eq!(walk_slow.source_name, "x28");
    assert_eq!(walk_slow.offset, 0x28);
    assert_eq!(walk_slow.provenance, CommonDataProvenance::ProvisionalMole);

    let walk_middle = sources
        .iter()
        .find(|source| source.rust_name == "walk_middle_x")
        .expect("walk_middle_x common-data source should be recorded");
    assert_eq!(walk_middle.source_name, "x2C");
    assert_eq!(walk_middle.offset, 0x2c);
    assert_eq!(
        walk_middle.provenance,
        CommonDataProvenance::ProvisionalMole
    );

    let walk_fast = sources
        .iter()
        .find(|source| source.rust_name == "walk_fast_x")
        .expect("walk_fast_x common-data source should be recorded");
    assert_eq!(walk_fast.source_name, "x30");
    assert_eq!(walk_fast.offset, 0x30);
    assert_eq!(walk_fast.provenance, CommonDataProvenance::ProvisionalMole);

    let dash_window = sources
        .iter()
        .find(|source| source.rust_name == "dash_tap_window")
        .expect("dash_tap_window common-data source should be recorded");
    assert_eq!(dash_window.source_name, "x40");
    assert_eq!(dash_window.offset, 0x40);

    let tap_jump = sources
        .iter()
        .find(|source| source.rust_name == "tap_jump_y")
        .expect("tap_jump_y common-data source should be recorded");
    assert_eq!(tap_jump.source_name, "tap_jump_threshold");
    assert_eq!(tap_jump.offset, 0x70);

    let z_shield_analog = sources
        .iter()
        .find(|source| source.rust_name == "z_shield_analog")
        .expect("z_shield_analog common-data source should be recorded");
    assert_eq!(z_shield_analog.source_name, "x14");
    assert_eq!(z_shield_analog.offset, 0x14);

    let trigger_timer_threshold = sources
        .iter()
        .find(|source| source.rust_name == "trigger_timer_threshold")
        .expect("trigger_timer_threshold common-data source should be recorded");
    assert_eq!(trigger_timer_threshold.source_name, "x18");
    assert_eq!(trigger_timer_threshold.offset, 0x18);

    let aerial_angle = sources
        .iter()
        .find(|source| source.rust_name == "aerial_vertical_angle_tan_milli")
        .expect("aerial vertical angle common-data source should be recorded");
    assert_eq!(aerial_angle.source_name, "x20_radians");
    assert_eq!(aerial_angle.offset, 0x20);

    let aerial_neutral_x = sources
        .iter()
        .find(|source| source.rust_name == "aerial_neutral_x")
        .expect("aerial_neutral_x common-data source should be recorded");
    assert_eq!(aerial_neutral_x.source_name, "xDC");
    assert_eq!(aerial_neutral_x.offset, 0xdc);

    let aerial_neutral_y = sources
        .iter()
        .find(|source| source.rust_name == "aerial_neutral_y")
        .expect("aerial_neutral_y common-data source should be recorded");
    assert_eq!(aerial_neutral_y.source_name, "xE0");
    assert_eq!(aerial_neutral_y.offset, 0xe0);

    let air_jump_backward_x = sources
        .iter()
        .find(|source| source.rust_name == "air_jump_backward_x")
        .expect("air_jump_backward_x common-data source should be recorded");
    assert_eq!(air_jump_backward_x.source_name, "x78");
    assert_eq!(air_jump_backward_x.offset, 0x78);

    let escape_x = sources
        .iter()
        .find(|source| source.rust_name == "escape_x")
        .expect("escape_x common-data source should be recorded");
    assert_eq!(escape_x.source_name, "x31C");
    assert_eq!(escape_x.offset, 0x31c);

    let escape_y = sources
        .iter()
        .find(|source| source.rust_name == "escape_y")
        .expect("escape_y common-data source should be recorded");
    assert_eq!(escape_y.source_name, "x314");
    assert_eq!(escape_y.offset, 0x314);

    let escapeair_deadzone_x = sources
        .iter()
        .find(|source| source.rust_name == "escapeair_deadzone_x")
        .expect("escapeair_deadzone_x common-data source should be recorded");
    assert_eq!(escapeair_deadzone_x.source_name, "escapeair_deadzone.x");
    assert_eq!(escapeair_deadzone_x.offset, 0x32c);

    let escapeair_deadzone_y = sources
        .iter()
        .find(|source| source.rust_name == "escapeair_deadzone_y")
        .expect("escapeair_deadzone_y common-data source should be recorded");
    assert_eq!(escapeair_deadzone_y.source_name, "escapeair_deadzone.y");
    assert_eq!(escapeair_deadzone_y.offset, 0x330);

    let escapeair_force = sources
        .iter()
        .find(|source| source.rust_name == "escapeair_force")
        .expect("escapeair_force common-data source should be recorded");
    assert_eq!(escapeair_force.source_name, "escapeair_force");
    assert_eq!(escapeair_force.offset, 0x338);

    let escapeair_iasa_timer_ticks = sources
        .iter()
        .find(|source| source.rust_name == "escapeair_iasa_timer_ticks")
        .expect("escapeair_iasa_timer_ticks common-data source should be recorded");
    assert_eq!(escapeair_iasa_timer_ticks.source_name, "x334");
    assert_eq!(escapeair_iasa_timer_ticks.offset, 0x334);

    let escapeair_decay = sources
        .iter()
        .find(|source| source.rust_name == "escapeair_decay_percent")
        .expect("escapeair_decay_percent common-data source should be recorded");
    assert_eq!(escapeair_decay.source_name, "escapeair_decay");
    assert_eq!(escapeair_decay.offset, 0x33c);

    let escapeair_landing_lag = sources
        .iter()
        .find(|source| source.rust_name == "escapeair_landing_lag")
        .expect("escapeair_landing_lag common-data source should be recorded");
    assert_eq!(escapeair_landing_lag.source_name, "x344");
    assert_eq!(escapeair_landing_lag.offset, 0x344);

    let guard_on_catch_dash_window = sources
        .iter()
        .find(|source| source.rust_name == "guard_on_catch_dash_window")
        .expect("guard_on_catch_dash_window common-data source should be recorded");
    assert_eq!(guard_on_catch_dash_window.source_name, "x68");
    assert_eq!(guard_on_catch_dash_window.offset, 0x68);

    let run_turn_run_no_interrupt = sources
        .iter()
        .find(|source| source.rust_name == "run_turn_run_no_interrupt_frames")
        .expect("run TurnRun no-interrupt source should be recorded");
    assert_eq!(run_turn_run_no_interrupt.source_name, "x430");
    assert_eq!(run_turn_run_no_interrupt.offset, 0x430);

    let dash_early_action_window = sources
        .iter()
        .find(|source| source.rust_name == "dash_early_action_window")
        .expect("dash_early_action_window common-data source should be recorded");
    assert_eq!(dash_early_action_window.source_name, "x44");
    assert_eq!(dash_early_action_window.offset, 0x44);

    let dash_defensive_action_window = sources
        .iter()
        .find(|source| source.rust_name == "dash_defensive_action_window")
        .expect("dash_defensive_action_window common-data source should be recorded");
    assert_eq!(dash_defensive_action_window.source_name, "x48");
    assert_eq!(dash_defensive_action_window.offset, 0x48);

    let dash_late_action_window = sources
        .iter()
        .find(|source| source.rust_name == "dash_late_action_window")
        .expect("dash_late_action_window common-data source should be recorded");
    assert_eq!(dash_late_action_window.source_name, "x4C");
    assert_eq!(dash_late_action_window.offset, 0x4c);
}

#[test]
fn extracted_plco_common_data_reads_big_endian_values_from_source_offsets() {
    let mut bytes = vec![0_u8; 0x444];

    put_f32_be(&mut bytes, 0x08, 0.37);
    put_f32_be(&mut bytes, 0x0c, 0.38);
    put_f32_be(&mut bytes, 0x10, 0.12);
    put_f32_be(&mut bytes, 0x14, 0.31);
    put_f32_be(&mut bytes, 0x18, 0.55);
    put_f32_be(&mut bytes, 0x20, std::f32::consts::FRAC_PI_4);
    put_f32_be(&mut bytes, 0x24, 0.21);
    put_f32_be(&mut bytes, 0x28, 0.31);
    put_f32_be(&mut bytes, 0x2c, 0.52);
    put_f32_be(&mut bytes, 0x30, 0.74);
    put_f32_be(&mut bytes, 0x34, 0.24);
    put_f32_be(&mut bytes, 0x3c, 0.82);
    put_i32_be(&mut bytes, 0x40, 5);
    put_f32_be(&mut bytes, 0x44, 6.0);
    put_f32_be(&mut bytes, 0x48, 7.0);
    put_f32_be(&mut bytes, 0x4c, 16.0);
    put_f32_be(&mut bytes, 0x68, 4.0);
    put_f32_be(&mut bytes, 0x70, 0.81);
    put_i32_be(&mut bytes, 0x74, 4);
    put_f32_be(&mut bytes, 0x78, 0.22);
    put_f32_be(&mut bytes, 0x7c, 0.41);
    put_f32_be(&mut bytes, 0x88, 0.83);
    put_i32_be(&mut bytes, 0x8c, 2);
    put_f32_be(&mut bytes, 0x90, 0.35);
    put_f32_be(&mut bytes, 0x98, 0.25);
    put_f32_be(&mut bytes, 0xac, 0.26);
    put_f32_be(&mut bytes, 0xdc, 0.43);
    put_f32_be(&mut bytes, 0xe0, 0.44);
    put_f32_be(&mut bytes, 0x314, 0.84);
    put_i32_be(&mut bytes, 0x318, 3);
    put_f32_be(&mut bytes, 0x31c, 0.85);
    put_i32_be(&mut bytes, 0x320, 4);
    put_f32_be(&mut bytes, 0x32c, 0.20);
    put_f32_be(&mut bytes, 0x330, 0.25);
    put_i32_be(&mut bytes, 0x334, 15);
    put_f32_be(&mut bytes, 0x338, 0.812);
    put_f32_be(&mut bytes, 0x33c, 0.91);
    put_f32_be(&mut bytes, 0x344, 10.0);
    put_f32_be(&mut bytes, 0x430, 2.0);

    let common =
        MeleeCommonData::from_plco_bytes(&bytes).expect("synthetic PlCo slice should extract");

    assert_eq!(common.tap_x_threshold, 47);
    assert_eq!(common.tap_y_threshold, 48);
    assert_eq!(common.trigger_deadzone, 31);
    assert_eq!(common.z_shield_analog, 79);
    assert_eq!(common.trigger_timer_threshold, 140);
    assert_eq!(common.aerial_vertical_angle_tan_milli, 1000);
    assert_eq!(common.walk_x, 27);
    assert_eq!(common.walk_slow_x, 39);
    assert_eq!(common.walk_middle_x, 66);
    assert_eq!(common.walk_fast_x, 94);
    assert_eq!(common.turn_x, 30);
    assert_eq!(common.dash_x, 104);
    assert_eq!(common.dash_tap_window, 5);
    assert_eq!(common.dash_early_action_window, 6);
    assert_eq!(common.dash_defensive_action_window, 7);
    assert_eq!(common.dash_late_action_window, 16);
    assert_eq!(common.guard_on_catch_dash_window, 4);
    assert_eq!(common.tap_jump_y, 103);
    assert_eq!(common.tap_jump_window, 4);
    assert_eq!(common.air_jump_backward_x, 28);
    assert_eq!(common.tap_jump_release_y, 52);
    assert_eq!(common.fast_fall_y, 105);
    assert_eq!(common.fast_fall_window, 2);
    assert_eq!(common.crouch_y, 44);
    assert_eq!(common.tilt_x, 32);
    assert_eq!(common.tilt_y, 33);
    assert_eq!(common.aerial_neutral_x, 55);
    assert_eq!(common.aerial_neutral_y, 56);
    assert_eq!(common.escape_y, 107);
    assert_eq!(common.escape_y_tap_window, 3);
    assert_eq!(common.escape_x, 108);
    assert_eq!(common.escape_x_tap_window, 4);
    assert_eq!(common.escapeair_deadzone_x, 25);
    assert_eq!(common.escapeair_deadzone_y, 32);
    assert_eq!(common.escapeair_iasa_timer_ticks, 15);
    assert_eq!(common.escapeair_animation_ticks, 20);
    assert_eq!(common.escapeair_force, 812);
    assert_eq!(common.escapeair_decay_percent, 91);
    assert_eq!(common.escapeair_landing_lag_ticks, 10);
    assert_eq!(common.run_turn_run_no_interrupt_frames, 2);
}

#[test]
fn extracted_plco_common_data_reports_the_missing_source_field() {
    let bytes = vec![0_u8; 0x430];

    let err = MeleeCommonData::from_plco_bytes(&bytes)
        .expect_err("slice ending before x430 should be rejected");

    assert_eq!(
        err,
        CommonDataExtractError::TooShort {
            field: "x430",
            offset: 0x430,
            required_len: 0x434,
            actual_len: 0x430,
        }
    );
}

fn put_f32_be(bytes: &mut [u8], offset: usize, value: f32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}

fn put_i32_be(bytes: &mut [u8], offset: usize, value: i32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}

#[test]
fn packed_input_round_trips_buttons_and_axes() {
    let input = PlayerInput::neutral()
        .with_left_stick(80, -32)
        .with_attack(true)
        .with_jump(true);

    assert_eq!(PlayerInput::from_bits(input.bits()), input);
}

#[test]
fn packed_input_round_trips_c_stick_and_dpad() {
    let input = PlayerInput::neutral()
        .with_c_stick(-64, 96)
        .with_dpad_up(true)
        .with_dpad_left(true);

    assert_eq!(input.c_stick_x(), -64);
    assert_eq!(input.c_stick_y(), 96);
    assert!(input.dpad_up());
    assert!(!input.dpad_down());
    assert!(input.dpad_left());
    assert!(!input.dpad_right());
    assert_eq!(PlayerInput::from_bits(input.bits()), input);
}

#[test]
fn packed_input_round_trips_split_triggers() {
    let input = PlayerInput::neutral()
        .with_left_trigger_analog(42)
        .with_right_trigger_analog(201)
        .with_left_trigger_digital(true);

    assert_eq!(input.left_trigger_analog(), 42);
    assert_eq!(input.right_trigger_analog(), 201);
    assert!(input.left_trigger_digital());
    assert!(!input.right_trigger_digital());
    assert!(input.shield());
    assert!(!input.explicit_shield());
    assert_eq!(PlayerInput::from_bits(input.bits()), input);
}

#[test]
fn packed_input_treats_light_analog_trigger_as_shield_held() {
    let input = PlayerInput::neutral().with_left_trigger_analog(42);

    assert!(input.trigger_active());
    assert!(input.shield());
    assert!(!input.explicit_shield());
    assert!(!input.left_trigger_digital());
    assert!(!input.right_trigger_digital());
}

#[test]
fn packed_input_round_trips_separate_jump_buttons() {
    let input = PlayerInput::neutral().with_jump_secondary(true);

    assert!(!input.jump_primary());
    assert!(input.jump_secondary());
    assert!(input.jump());
    assert_eq!(PlayerInput::from_bits(input.bits()), input);
}

#[test]
fn gamecube_pad_status_defaults_to_console_neutral_values() {
    let pad = GameCubePadStatus::neutral();

    assert_eq!(pad.stick_x, 128);
    assert_eq!(pad.stick_y, 128);
    assert_eq!(pad.c_stick_x, 128);
    assert_eq!(pad.c_stick_y, 128);
    assert_eq!(pad.left_trigger, 0);
    assert_eq!(pad.right_trigger, 0);
    assert_eq!(pad.main_stick_i16(), (0, 0));
    assert_eq!(pad.c_stick_i16(), (0, 0));
    assert_eq!(pad.main_stick_i8(), (0, 0));
    assert_eq!(pad.buttons.bits(), 0);
}

#[test]
fn gamecube_pad_status_preserves_native_byte_range() {
    let pad = GameCubePadStatus {
        stick_x: 255,
        stick_y: 0,
        c_stick_x: 0,
        c_stick_y: 255,
        left_trigger: 12,
        right_trigger: 250,
        buttons: GameCubeButtonState::empty().with_a(true).with_l(true),
    };

    assert_eq!(pad.main_stick_i16(), (32_512, -32_768));
    assert_eq!(pad.c_stick_i16(), (-32_768, 32_512));
    assert_eq!(pad.main_stick_i8(), (127, -128));
    assert_eq!(pad.left_trigger, 12);
    assert_eq!(pad.right_trigger, 250);
    assert!(pad.buttons.a());
    assert!(pad.buttons.l());
    assert!(!pad.buttons.r());
}

#[test]
fn melee_input_snapshot_tracks_edges_previous_sticks_and_tap_timers() {
    let mut processor = MeleeInputProcessor::new(MeleeInputConfig {
        tap_x_threshold: 36,
        tap_y_threshold: 36,
        trigger_threshold: 64,
        trigger_timer_threshold: 64,
        ..MeleeInputConfig::default()
    });

    let neutral = processor.update(GameCubePadStatus::neutral());

    assert_eq!(neutral.lstick, (0, 0));
    assert_eq!(neutral.prev_lstick, (0, 0));
    assert_eq!(neutral.x_tap_timer, 0xfe);
    assert_eq!(neutral.y_tap_timer, 0xfe);
    assert_eq!(neutral.trigger_timer, 0xfe);
    assert_eq!(neutral.pressed.bits(), 0);
    assert_eq!(neutral.released.bits(), 0);

    let first_right = processor.update(GameCubePadStatus {
        stick_x: 168,
        left_trigger: 80,
        buttons: GameCubeButtonState::empty().with_a(true),
        ..GameCubePadStatus::neutral()
    });

    assert_eq!(first_right.lstick, (40, 0));
    assert_eq!(first_right.prev_lstick, (0, 0));
    assert!(first_right.held.a());
    assert!(first_right.pressed.a());
    assert!(!first_right.released.a());
    assert!(first_right.shield_held);
    assert!(first_right.shield_pressed);
    assert!(!first_right.shield_released);
    assert_eq!(first_right.left_trigger, 80);
    assert_eq!(first_right.right_trigger, 0);
    assert_eq!(first_right.x_tap_timer, 0);
    assert_eq!(first_right.y_tap_timer, 0xfe);
    assert_eq!(first_right.trigger_timer, 0);

    let held_right = processor.update(GameCubePadStatus {
        stick_x: 168,
        left_trigger: 80,
        buttons: GameCubeButtonState::empty().with_a(true),
        ..GameCubePadStatus::neutral()
    });

    assert_eq!(held_right.lstick, (40, 0));
    assert_eq!(held_right.prev_lstick, (40, 0));
    assert!(held_right.held.a());
    assert!(!held_right.pressed.a());
    assert!(!held_right.released.a());
    assert!(held_right.shield_held);
    assert!(!held_right.shield_pressed);
    assert_eq!(held_right.x_tap_timer, 1);
    assert_eq!(held_right.trigger_timer, 1);

    let released = processor.update(GameCubePadStatus::neutral());

    assert_eq!(released.lstick, (0, 0));
    assert_eq!(released.prev_lstick, (40, 0));
    assert!(!released.held.a());
    assert!(!released.pressed.a());
    assert!(released.released.a());
    assert!(!released.shield_held);
    assert!(!released.shield_pressed);
    assert!(released.shield_released);
    assert_eq!(released.x_tap_timer, 0xfe);
    assert_eq!(released.trigger_timer, 0xfe);
}

#[test]
fn melee_input_snapshot_splits_analog_shield_hold_from_x18_trigger_timer() {
    let mut processor = MeleeInputProcessor::new(MeleeInputConfig {
        trigger_threshold: 1,
        trigger_timer_threshold: 140,
        ..MeleeInputConfig::default()
    });

    let light_trigger = processor.update(GameCubePadStatus {
        left_trigger: 49,
        ..GameCubePadStatus::neutral()
    });

    assert!(light_trigger.shield_held);
    assert!(light_trigger.shield_pressed);
    assert!(light_trigger.left_trigger_analog_held);
    assert!(light_trigger.left_trigger_analog_pressed);
    assert_eq!(light_trigger.trigger_timer, 0xfe);

    let strong_trigger = processor.update(GameCubePadStatus {
        left_trigger: 140,
        ..GameCubePadStatus::neutral()
    });
    let held_strong_trigger = processor.update(GameCubePadStatus {
        left_trigger: 140,
        ..GameCubePadStatus::neutral()
    });

    assert!(strong_trigger.shield_held);
    assert!(!strong_trigger.shield_pressed);
    assert_eq!(strong_trigger.trigger_timer, 0);
    assert_eq!(held_strong_trigger.trigger_timer, 1);
}

#[test]
fn melee_input_snapshot_preserves_c_stick_dpad_and_split_triggers() {
    let mut processor = MeleeInputProcessor::new(MeleeInputConfig::default());

    let snapshot = processor.update(GameCubePadStatus {
        c_stick_x: 255,
        c_stick_y: 0,
        left_trigger: 12,
        right_trigger: 200,
        buttons: GameCubeButtonState::from_bits(
            (1 << 4) | (1 << 5) | (1 << 6) | (1 << 7) | (1 << 10),
        ),
        ..GameCubePadStatus::neutral()
    });

    assert_eq!(snapshot.cstick, (127, -128));
    assert_eq!(snapshot.prev_cstick, (0, 0));
    assert_eq!(snapshot.left_trigger, 12);
    assert_eq!(snapshot.right_trigger, 200);
    assert!(snapshot.held.dpad_left());
    assert!(snapshot.held.dpad_right());
    assert!(snapshot.held.dpad_down());
    assert!(snapshot.held.dpad_up());
    assert!(snapshot.held.r());
    assert!(!snapshot.held.l());
    assert!(snapshot.shield_held);
    assert!(snapshot.shield_pressed);
}

#[test]
fn melee_input_processor_applies_pad_cleanup_before_timers_and_edges() {
    let mut processor = MeleeInputProcessor::new(MeleeInputConfig {
        tap_x_threshold: 36,
        tap_y_threshold: 36,
        trigger_threshold: 64,
        trigger_timer_threshold: 64,
        main_stick_deadzone: 4,
        c_stick_deadzone: 5,
        trigger_deadzone: 8,
    });

    let drift = processor.update(GameCubePadStatus {
        stick_x: 131,
        stick_y: 126,
        c_stick_x: 132,
        c_stick_y: 124,
        left_trigger: 7,
        right_trigger: 7,
        ..GameCubePadStatus::neutral()
    });

    assert_eq!(drift.lstick, (0, 0));
    assert_eq!(drift.cstick, (0, 0));
    assert_eq!(drift.left_trigger, 0);
    assert_eq!(drift.right_trigger, 0);
    assert!(!drift.shield_held);
    assert_eq!(drift.x_tap_timer, 0xfe);
    assert_eq!(drift.y_tap_timer, 0xfe);
    assert_eq!(drift.trigger_timer, 0xfe);

    let first_clean_move = processor.update(GameCubePadStatus {
        stick_x: 168,
        stick_y: 127,
        c_stick_x: 128,
        c_stick_y: 0,
        left_trigger: 80,
        right_trigger: 7,
        ..GameCubePadStatus::neutral()
    });

    assert_eq!(first_clean_move.prev_lstick, (0, 0));
    assert_eq!(first_clean_move.lstick, (40, 0));
    assert_eq!(first_clean_move.cstick, (0, -128));
    assert_eq!(first_clean_move.left_trigger, 80);
    assert_eq!(first_clean_move.right_trigger, 0);
    assert!(first_clean_move.shield_pressed);
    assert_eq!(first_clean_move.x_tap_timer, 0);
    assert_eq!(first_clean_move.trigger_timer, 0);
}

#[test]
fn ucf_0_84_constants_match_source_units() {
    assert_eq!(UCF_VERSION, "0.84");
    assert_eq!(UCF_CARDINAL_AXIS, 80);
    assert_eq!(UCF_CARDINAL_SNAP_RANGE, 6);
    assert_eq!(UCF_TILT_INTENT_DELTA, 75);
    assert_eq!(UCF_SHIELD_DROP_DELTA, 44);
}

#[test]
fn melee_input_processor_applies_ucf_cardinals_before_snapshot_output() {
    let mut processor = MeleeInputProcessor::new(MeleeInputConfig::default());

    let snapshot = processor.update(GameCubePadStatus {
        stick_x: 208,
        stick_y: 134,
        c_stick_x: 134,
        c_stick_y: 208,
        ..GameCubePadStatus::neutral()
    });

    assert_eq!(snapshot.lstick, (127, 0));
    assert_eq!(snapshot.cstick, (0, 127));
}

#[test]
fn melee_input_processor_leaves_non_ucf_cardinal_diagonals_unchanged() {
    let mut processor = MeleeInputProcessor::new(MeleeInputConfig::default());

    let snapshot = processor.update(GameCubePadStatus {
        stick_x: 208,
        stick_y: 135,
        c_stick_x: 135,
        c_stick_y: 208,
        ..GameCubePadStatus::neutral()
    });

    assert_eq!(snapshot.lstick, (80, 7));
    assert_eq!(snapshot.cstick, (7, 80));
}

#[test]
fn melee_input_processor_exposes_ucf_tilt_intent_from_native_pad_buffer() {
    let mut processor = MeleeInputProcessor::new(MeleeInputConfig::default());

    processor.update(GameCubePadStatus::neutral());
    let tilt_frame = processor.update(GameCubePadStatus {
        stick_x: 168,
        ..GameCubePadStatus::neutral()
    });
    let dash_frame = processor.update(GameCubePadStatus {
        stick_x: 255,
        ..GameCubePadStatus::neutral()
    });

    assert!(!tilt_frame.ucf_x_tilt_intent);
    assert!(dash_frame.ucf_x_tilt_intent);
    assert_eq!(
        dash_frame
            .facts(MeleeInputThresholds::default())
            .ucf_dashback_direction,
        1
    );
}

#[test]
fn melee_input_facts_fold_ucf_dashback_into_canonical_dash_direction() {
    let snapshot = MeleeInputSnapshot {
        ucf_x_tilt_intent: true,
        ..snapshot_with_timers((127, 0), (0, 0), 0xfe, 0xfe)
    };
    let facts = snapshot.facts(MeleeInputThresholds::default());

    assert_eq!(facts.horizontal_smash_direction, 0);
    assert_eq!(facts.dash_direction, 1);
    assert_eq!(facts.ucf_dashback_direction, 1);
}

#[test]
fn melee_input_processor_exposes_ucf_shield_drop_intent_from_native_pad_buffer() {
    let mut processor = MeleeInputProcessor::new(MeleeInputConfig::default());

    processor.update(GameCubePadStatus::neutral());
    let shallow_down = processor.update(GameCubePadStatus {
        stick_y: 104,
        ..GameCubePadStatus::neutral()
    });
    let shield_drop_frame = processor.update(GameCubePadStatus {
        stick_y: 48,
        ..GameCubePadStatus::neutral()
    });

    assert!(!shallow_down.ucf_shield_drop_tilt_intent);
    assert!(shield_drop_frame.ucf_shield_drop_tilt_intent);
    assert!(
        shield_drop_frame
            .facts(MeleeInputThresholds::default())
            .ucf_shield_drop
    );
}

#[test]
fn melee_input_processor_handles_full_opposite_stick_delta_without_overflow() {
    let mut processor = MeleeInputProcessor::new(MeleeInputConfig::default());

    processor.update(GameCubePadStatus {
        stick_x: 255,
        stick_y: 255,
        ..GameCubePadStatus::neutral()
    });
    processor.update(GameCubePadStatus::neutral());
    let snapshot = processor.update(GameCubePadStatus {
        stick_x: 0,
        stick_y: 0,
        ..GameCubePadStatus::neutral()
    });

    assert!(snapshot.ucf_x_tilt_intent);
    assert!(snapshot.ucf_shield_drop_tilt_intent);
}

#[test]
fn player_input_can_carry_ucf_facts_for_rollback_owned_input() {
    let input = PlayerInput::neutral()
        .with_left_stick(127, -80)
        .with_ucf_x_tilt_intent(true)
        .with_ucf_shield_drop_tilt_intent(true);

    assert_eq!(PlayerInput::from_bits(input.bits()), input);

    let snapshot = input.melee_snapshot(PlayerInput::neutral(), MeleeInputTimers::expired());
    let facts = snapshot.facts(MeleeInputThresholds::default());

    assert!(snapshot.ucf_x_tilt_intent);
    assert!(snapshot.ucf_shield_drop_tilt_intent);
    assert_eq!(facts.ucf_dashback_direction, 1);
    assert!(facts.ucf_shield_drop);
}

#[test]
fn melee_input_facts_classify_walk_crouch_dash_and_jump_windows() {
    let thresholds = MeleeInputThresholds {
        walk_x: 20,
        walk_slow_x: 20,
        walk_middle_x: 50,
        walk_fast_x: 90,
        dash_x: 80,
        dash_tap_window: 2,
        turn_x: 24,
        tilt_x: 24,
        tilt_y: 24,
        smash_y: 80,
        crouch_y: 36,
        tap_jump_y: 80,
        tap_jump_window: 2,
        tap_jump_release_y: 40,
        fast_fall_y: 80,
        fast_fall_window: 2,
        c_stick: 40,
        aerial_neutral_x: 40,
        aerial_neutral_y: 40,
        aerial_vertical_angle_tan_milli: 1000,
        z_shield_analog: 49,
        escape_x: 80,
        escape_x_tap_window: 2,
        escape_y: 80,
        escape_y_tap_window: 2,
        special_side_x: 40,
        special_vertical_y: 40,
    };

    let mut walk_processor = MeleeInputProcessor::new(MeleeInputConfig {
        tap_x_threshold: 36,
        tap_y_threshold: 36,
        trigger_threshold: 64,
        ..MeleeInputConfig::default()
    });
    walk_processor.update(GameCubePadStatus::neutral());
    let walk_down = walk_processor.update(GameCubePadStatus {
        stick_x: 168,
        stick_y: 80,
        ..GameCubePadStatus::neutral()
    });
    let walk_facts = walk_down.facts(thresholds);

    assert_eq!(walk_facts.walk_direction, 1);
    assert_eq!(walk_facts.dash_direction, 0);
    assert!(walk_facts.crouch);
    assert!(!walk_facts.jump_pressed);

    let mut dash_processor = MeleeInputProcessor::new(MeleeInputConfig {
        tap_x_threshold: 36,
        tap_y_threshold: 36,
        trigger_threshold: 64,
        ..MeleeInputConfig::default()
    });
    dash_processor.update(GameCubePadStatus::neutral());
    let dash_jump_pad = GameCubePadStatus {
        stick_x: 220,
        stick_y: 220,
        buttons: GameCubeButtonState::empty().with_x(true),
        ..GameCubePadStatus::neutral()
    };
    let dash_jump = dash_processor.update(dash_jump_pad);
    let dash_jump_facts = dash_jump.facts(thresholds);

    assert_eq!(dash_jump_facts.dash_direction, 1);
    assert!(dash_jump_facts.tap_jump);
    assert!(dash_jump_facts.button_jump_pressed);
    assert!(dash_jump_facts.jump_pressed);

    let mut held_dash_jump = dash_jump;
    for _ in 0..3 {
        held_dash_jump = dash_processor.update(dash_jump_pad);
    }
    let held_facts = held_dash_jump.facts(thresholds);

    assert_eq!(held_facts.dash_direction, 0);
    assert!(!held_facts.tap_jump);
    assert!(!held_facts.button_jump_pressed);
    assert!(!held_facts.jump_pressed);
}

#[test]
fn melee_input_facts_classify_walk_speed_bucket_from_current_stick() {
    let slow =
        snapshot_with_timers((30, 0), (0, 0), 0xfe, 0xfe).facts(MeleeInputThresholds::default());
    let middle =
        snapshot_with_timers((64, 0), (0, 0), 0xfe, 0xfe).facts(MeleeInputThresholds::default());
    let fast =
        snapshot_with_timers((100, 0), (0, 0), 0xfe, 0xfe).facts(MeleeInputThresholds::default());

    assert_eq!(slow.walk_direction, 1);
    assert_eq!(middle.walk_direction, 1);
    assert_eq!(fast.walk_direction, 1);
    assert_eq!(slow.walk_speed_bucket, WalkSpeedBucket::Slow);
    assert_eq!(middle.walk_speed_bucket, WalkSpeedBucket::Middle);
    assert_eq!(fast.walk_speed_bucket, WalkSpeedBucket::Fast);
}

#[test]
fn melee_input_facts_classify_walk_speed_bucket_from_common_data_thresholds() {
    let thresholds = MeleeInputThresholds {
        walk_x: 25,
        walk_slow_x: 25,
        walk_middle_x: 72,
        walk_fast_x: 108,
        dash_x: 120,
        ..MeleeInputThresholds::default()
    };

    let slow = snapshot_with_timers((64, 0), (0, 0), 0xfe, 0xfe).facts(thresholds);
    let middle = snapshot_with_timers((100, 0), (0, 0), 0xfe, 0xfe).facts(thresholds);
    let fast = snapshot_with_timers((112, 0), (0, 0), 0xfe, 0xfe).facts(thresholds);

    assert_eq!(slow.walk_direction, 1);
    assert_eq!(middle.walk_direction, 1);
    assert_eq!(fast.walk_direction, 1);
    assert_eq!(slow.walk_speed_bucket, WalkSpeedBucket::Slow);
    assert_eq!(middle.walk_speed_bucket, WalkSpeedBucket::Middle);
    assert_eq!(fast.walk_speed_bucket, WalkSpeedBucket::Fast);
    assert_eq!(slow.dash_direction, 0);
    assert_eq!(middle.dash_direction, 0);
    assert_eq!(fast.dash_direction, 0);
}

#[test]
fn melee_input_facts_track_fast_fall_as_downward_tap_intent() {
    let thresholds = MeleeInputThresholds {
        walk_x: 20,
        walk_slow_x: 20,
        walk_middle_x: 50,
        walk_fast_x: 90,
        dash_x: 80,
        dash_tap_window: 2,
        turn_x: 24,
        tilt_x: 24,
        tilt_y: 24,
        smash_y: 80,
        crouch_y: 36,
        tap_jump_y: 80,
        tap_jump_window: 2,
        tap_jump_release_y: 40,
        fast_fall_y: 80,
        fast_fall_window: 2,
        c_stick: 40,
        aerial_neutral_x: 40,
        aerial_neutral_y: 40,
        aerial_vertical_angle_tan_milli: 1000,
        z_shield_analog: 49,
        escape_x: 80,
        escape_x_tap_window: 2,
        escape_y: 80,
        escape_y_tap_window: 2,
        special_side_x: 40,
        special_vertical_y: 40,
    };
    let mut processor = MeleeInputProcessor::new(MeleeInputConfig {
        tap_x_threshold: 36,
        tap_y_threshold: 36,
        trigger_threshold: 64,
        ..MeleeInputConfig::default()
    });

    processor.update(GameCubePadStatus::neutral());
    let down_tap_pad = GameCubePadStatus {
        stick_y: 30,
        ..GameCubePadStatus::neutral()
    };
    let down_tap = processor.update(down_tap_pad);
    let down_tap_facts = down_tap.facts(thresholds);

    assert_eq!(down_tap.y_tap_timer, 0);
    assert!(down_tap_facts.fast_fall);
    assert!(!down_tap_facts.tap_jump);

    let held_down_one_frame = processor.update(down_tap_pad);
    let held_down_one_frame_facts = held_down_one_frame.facts(thresholds);

    assert_eq!(held_down_one_frame.y_tap_timer, 1);
    assert!(held_down_one_frame_facts.fast_fall);

    let held_down_two_frames = processor.update(down_tap_pad);
    let held_down_two_frames_facts = held_down_two_frames.facts(thresholds);

    assert_eq!(held_down_two_frames.y_tap_timer, 2);
    assert!(!held_down_two_frames_facts.fast_fall);

    let mut up_processor = MeleeInputProcessor::new(MeleeInputConfig {
        tap_x_threshold: 36,
        tap_y_threshold: 36,
        trigger_threshold: 64,
        ..MeleeInputConfig::default()
    });
    up_processor.update(GameCubePadStatus::neutral());
    let up_tap = up_processor.update(GameCubePadStatus {
        stick_y: 220,
        ..GameCubePadStatus::neutral()
    });
    let up_tap_facts = up_tap.facts(thresholds);

    assert!(up_tap_facts.tap_jump);
    assert!(!up_tap_facts.fast_fall);
}

#[test]
fn melee_input_facts_use_exclusive_dash_tap_window_like_common_data() {
    let thresholds = MeleeInputThresholds {
        dash_tap_window: 3,
        ..MeleeInputThresholds::default()
    };

    let inside_window = snapshot_with_timers((90, 90), (0, 0), 2, 2).facts(thresholds);
    let at_boundary = snapshot_with_timers((90, 90), (0, 0), 3, 3).facts(thresholds);

    assert_eq!(inside_window.dash_direction, 1);
    assert!(inside_window.tap_jump);
    assert_eq!(at_boundary.dash_direction, 0);
    assert!(!at_boundary.tap_jump);
}

#[test]
fn melee_input_facts_use_exclusive_shield_escape_tap_windows() {
    let thresholds = MeleeInputThresholds {
        escape_x_tap_window: 3,
        escape_y_tap_window: 3,
        ..MeleeInputThresholds::default()
    };

    let roll_inside = snapshot_with_timers((90, 0), (0, 0), 2, 254).facts(thresholds);
    let roll_at_boundary = snapshot_with_timers((90, 0), (0, 0), 3, 254).facts(thresholds);
    let spot_inside = snapshot_with_timers((0, -90), (0, 0), 254, 2).facts(thresholds);
    let spot_at_boundary = snapshot_with_timers((0, -90), (0, 0), 254, 3).facts(thresholds);

    assert_eq!(roll_inside.roll_direction, 1);
    assert_eq!(roll_at_boundary.roll_direction, 0);
    assert!(spot_inside.spot_dodge);
    assert!(!spot_at_boundary.spot_dodge);
}

#[test]
fn melee_input_facts_separate_tilts_from_facing_aware_smash_turns() {
    let thresholds = MeleeInputThresholds {
        walk_x: 20,
        walk_slow_x: 20,
        walk_middle_x: 50,
        walk_fast_x: 90,
        dash_x: 80,
        dash_tap_window: 2,
        turn_x: 24,
        tilt_x: 24,
        tilt_y: 24,
        smash_y: 80,
        crouch_y: 36,
        tap_jump_y: 80,
        tap_jump_window: 2,
        tap_jump_release_y: 40,
        fast_fall_y: 80,
        fast_fall_window: 2,
        c_stick: 40,
        aerial_neutral_x: 40,
        aerial_neutral_y: 40,
        aerial_vertical_angle_tan_milli: 1000,
        z_shield_analog: 49,
        escape_x: 80,
        escape_x_tap_window: 2,
        escape_y: 80,
        escape_y_tap_window: 2,
        special_side_x: 40,
        special_vertical_y: 40,
    };

    let mut tilt_processor = MeleeInputProcessor::new(MeleeInputConfig {
        tap_x_threshold: 36,
        tap_y_threshold: 36,
        trigger_threshold: 64,
        ..MeleeInputConfig::default()
    });
    tilt_processor.update(GameCubePadStatus::neutral());
    let tilt = tilt_processor.update(GameCubePadStatus {
        stick_x: 168,
        stick_y: 100,
        ..GameCubePadStatus::neutral()
    });
    let tilt_facts = tilt.facts(thresholds);

    assert_eq!(tilt_facts.tilt_direction, (1, -1));
    assert_eq!(tilt_facts.horizontal_smash_direction, 0);
    assert_eq!(tilt_facts.forward_dash_direction(1), 0);
    assert_eq!(tilt_facts.smash_turn_direction(1), 0);

    let mut smash_processor = MeleeInputProcessor::new(MeleeInputConfig {
        tap_x_threshold: 36,
        tap_y_threshold: 36,
        trigger_threshold: 64,
        ..MeleeInputConfig::default()
    });
    smash_processor.update(GameCubePadStatus::neutral());
    let smash_left_pad = GameCubePadStatus {
        stick_x: 30,
        ..GameCubePadStatus::neutral()
    };

    let smash_left = smash_processor.update(smash_left_pad);
    let smash_left_facts = smash_left.facts(thresholds);

    assert_eq!(smash_left_facts.tilt_direction, (-1, 0));
    assert_eq!(smash_left_facts.horizontal_smash_direction, -1);
    assert_eq!(smash_left_facts.forward_dash_direction(-1), -1);
    assert_eq!(smash_left_facts.forward_dash_direction(1), 0);
    assert_eq!(smash_left_facts.smash_turn_direction(1), -1);
    assert_eq!(smash_left_facts.smash_turn_direction(-1), 0);

    let mut held_smash_left = smash_left;
    for _ in 0..3 {
        held_smash_left = smash_processor.update(smash_left_pad);
    }
    let held_facts = held_smash_left.facts(thresholds);

    assert_eq!(held_facts.tilt_direction, (-1, 0));
    assert_eq!(held_facts.horizontal_smash_direction, 0);
    assert_eq!(held_facts.smash_turn_direction(1), 0);
}

#[test]
fn melee_input_facts_classify_attack_intent_from_a_press_and_cstick() {
    let thresholds = MeleeInputThresholds {
        walk_x: 20,
        walk_slow_x: 20,
        walk_middle_x: 50,
        walk_fast_x: 90,
        dash_x: 80,
        dash_tap_window: 2,
        turn_x: 24,
        tilt_x: 24,
        tilt_y: 24,
        smash_y: 80,
        crouch_y: 36,
        tap_jump_y: 80,
        tap_jump_window: 2,
        tap_jump_release_y: 40,
        fast_fall_y: 80,
        fast_fall_window: 2,
        c_stick: 40,
        aerial_neutral_x: 40,
        aerial_neutral_y: 40,
        aerial_vertical_angle_tan_milli: 1000,
        z_shield_analog: 49,
        escape_x: 80,
        escape_x_tap_window: 2,
        escape_y: 80,
        escape_y_tap_window: 2,
        special_side_x: 40,
        special_vertical_y: 40,
    };

    let mut neutral_processor = MeleeInputProcessor::new(MeleeInputConfig {
        tap_x_threshold: 36,
        tap_y_threshold: 36,
        trigger_threshold: 64,
        ..MeleeInputConfig::default()
    });
    neutral_processor.update(GameCubePadStatus::neutral());
    let neutral_a = neutral_processor.update(GameCubePadStatus {
        buttons: GameCubeButtonState::empty().with_a(true),
        ..GameCubePadStatus::neutral()
    });
    let neutral_facts = neutral_a.facts(thresholds);

    assert!(neutral_facts.attack_pressed);
    assert!(neutral_facts.neutral_attack_pressed);
    assert_eq!(neutral_facts.tilt_attack_direction, (0, 0));
    assert_eq!(neutral_facts.smash_attack_direction, (0, 0));

    let mut tilt_processor = MeleeInputProcessor::new(MeleeInputConfig {
        tap_x_threshold: 36,
        tap_y_threshold: 36,
        trigger_threshold: 64,
        ..MeleeInputConfig::default()
    });
    tilt_processor.update(GameCubePadStatus::neutral());
    let tilt_a = tilt_processor.update(GameCubePadStatus {
        stick_x: 168,
        buttons: GameCubeButtonState::empty().with_a(true),
        ..GameCubePadStatus::neutral()
    });
    let tilt_facts = tilt_a.facts(thresholds);

    assert!(tilt_facts.attack_pressed);
    assert!(!tilt_facts.neutral_attack_pressed);
    assert_eq!(tilt_facts.tilt_attack_direction, (1, 0));
    assert_eq!(tilt_facts.smash_attack_direction, (0, 0));

    let mut side_diagonal_processor = MeleeInputProcessor::new(MeleeInputConfig {
        tap_x_threshold: 36,
        tap_y_threshold: 36,
        trigger_threshold: 64,
        ..MeleeInputConfig::default()
    });
    side_diagonal_processor.update(GameCubePadStatus::neutral());
    let side_diagonal = side_diagonal_processor.update(GameCubePadStatus {
        stick_x: 178,
        stick_y: 158,
        buttons: GameCubeButtonState::empty().with_a(true),
        ..GameCubePadStatus::neutral()
    });
    let side_diagonal_facts = side_diagonal.facts(thresholds);

    assert_eq!(side_diagonal_facts.tilt_attack_direction, (1, 0));

    let mut up_diagonal_processor = MeleeInputProcessor::new(MeleeInputConfig {
        tap_x_threshold: 36,
        tap_y_threshold: 36,
        trigger_threshold: 64,
        ..MeleeInputConfig::default()
    });
    up_diagonal_processor.update(GameCubePadStatus::neutral());
    let up_diagonal = up_diagonal_processor.update(GameCubePadStatus {
        stick_x: 158,
        stick_y: 178,
        buttons: GameCubeButtonState::empty().with_a(true),
        ..GameCubePadStatus::neutral()
    });
    let up_diagonal_facts = up_diagonal.facts(thresholds);

    assert_eq!(up_diagonal_facts.tilt_attack_direction, (0, 1));

    let mut down_diagonal_processor = MeleeInputProcessor::new(MeleeInputConfig {
        tap_x_threshold: 36,
        tap_y_threshold: 36,
        trigger_threshold: 64,
        ..MeleeInputConfig::default()
    });
    down_diagonal_processor.update(GameCubePadStatus::neutral());
    let down_diagonal = down_diagonal_processor.update(GameCubePadStatus {
        stick_x: 158,
        stick_y: 78,
        buttons: GameCubeButtonState::empty().with_a(true),
        ..GameCubePadStatus::neutral()
    });
    let down_diagonal_facts = down_diagonal.facts(thresholds);

    assert_eq!(down_diagonal_facts.tilt_attack_direction, (0, -1));

    let mut smash_processor = MeleeInputProcessor::new(MeleeInputConfig {
        tap_x_threshold: 36,
        tap_y_threshold: 36,
        trigger_threshold: 64,
        ..MeleeInputConfig::default()
    });
    smash_processor.update(GameCubePadStatus::neutral());
    let smash_a = smash_processor.update(GameCubePadStatus {
        stick_x: 220,
        buttons: GameCubeButtonState::empty().with_a(true),
        ..GameCubePadStatus::neutral()
    });
    let smash_facts = smash_a.facts(thresholds);

    assert!(smash_facts.attack_pressed);
    assert_eq!(smash_facts.tilt_attack_direction, (0, 0));
    assert_eq!(smash_facts.smash_attack_direction, (1, 0));

    let held_smash_a = smash_processor.update(GameCubePadStatus {
        stick_x: 220,
        buttons: GameCubeButtonState::empty().with_a(true),
        ..GameCubePadStatus::neutral()
    });
    let held_smash_facts = held_smash_a.facts(thresholds);

    assert!(!held_smash_facts.attack_pressed);
    assert_eq!(held_smash_facts.smash_attack_direction, (0, 0));

    let mut cstick_processor = MeleeInputProcessor::new(MeleeInputConfig {
        tap_x_threshold: 36,
        tap_y_threshold: 36,
        trigger_threshold: 64,
        ..MeleeInputConfig::default()
    });
    cstick_processor.update(GameCubePadStatus::neutral());
    let cstick = cstick_processor.update(GameCubePadStatus {
        c_stick_x: 255,
        c_stick_y: 255,
        ..GameCubePadStatus::neutral()
    });
    let cstick_facts = cstick.facts(thresholds);

    assert!(!cstick_facts.attack_pressed);
    assert_eq!(cstick_facts.cstick_direction, (1, 1));
    assert_eq!(cstick_facts.cstick_smash_direction, (1, 0));

    let held_cstick = cstick_processor.update(GameCubePadStatus {
        c_stick_x: 255,
        c_stick_y: 255,
        ..GameCubePadStatus::neutral()
    });
    let held_cstick_facts = held_cstick.facts(thresholds);

    assert_eq!(held_cstick_facts.cstick_direction, (1, 1));
    assert_eq!(held_cstick_facts.cstick_smash_direction, (0, 0));

    cstick_processor.update(GameCubePadStatus::neutral());
    let retriggered_cstick = cstick_processor.update(GameCubePadStatus {
        c_stick_x: 255,
        c_stick_y: 255,
        ..GameCubePadStatus::neutral()
    });
    let retriggered_cstick_facts = retriggered_cstick.facts(thresholds);

    assert_eq!(retriggered_cstick_facts.cstick_direction, (1, 1));
    assert_eq!(retriggered_cstick_facts.cstick_smash_direction, (1, 0));

    cstick_processor.update(GameCubePadStatus {
        c_stick_x: 255,
        c_stick_y: 128,
        ..GameCubePadStatus::neutral()
    });
    let vertical_only_cstick = cstick_processor.update(GameCubePadStatus {
        c_stick_x: 255,
        c_stick_y: 255,
        ..GameCubePadStatus::neutral()
    });
    let vertical_only_cstick_facts = vertical_only_cstick.facts(thresholds);

    assert_eq!(vertical_only_cstick_facts.cstick_direction, (1, 1));
    assert_eq!(vertical_only_cstick_facts.cstick_smash_direction, (0, 1));
}

#[test]
fn melee_input_facts_track_jump_source_and_short_hop_release() {
    let thresholds = MeleeInputThresholds {
        walk_x: 20,
        walk_slow_x: 20,
        walk_middle_x: 50,
        walk_fast_x: 90,
        dash_x: 80,
        dash_tap_window: 2,
        turn_x: 24,
        tilt_x: 24,
        tilt_y: 24,
        smash_y: 80,
        crouch_y: 36,
        tap_jump_y: 80,
        tap_jump_window: 2,
        tap_jump_release_y: 40,
        fast_fall_y: 80,
        fast_fall_window: 2,
        c_stick: 40,
        aerial_neutral_x: 40,
        aerial_neutral_y: 40,
        aerial_vertical_angle_tan_milli: 1000,
        z_shield_analog: 49,
        escape_x: 80,
        escape_x_tap_window: 2,
        escape_y: 80,
        escape_y_tap_window: 2,
        special_side_x: 40,
        special_vertical_y: 40,
    };
    let mut processor = MeleeInputProcessor::new(MeleeInputConfig {
        tap_x_threshold: 36,
        tap_y_threshold: 36,
        trigger_threshold: 64,
        ..MeleeInputConfig::default()
    });

    processor.update(GameCubePadStatus::neutral());
    let tap_jump_first = processor.update(GameCubePadStatus {
        stick_y: 220,
        c_stick_y: 220,
        buttons: GameCubeButtonState::empty().with_x(true),
        ..GameCubePadStatus::neutral()
    });
    let tap_jump_facts = tap_jump_first.facts(thresholds);

    assert_eq!(tap_jump_facts.jump_input, MeleeJumpInput::LStick);
    assert!(tap_jump_facts.tap_jump);
    assert!(tap_jump_facts.button_jump_pressed);
    assert!(tap_jump_facts.button_jump_held);
    assert!(tap_jump_facts.cstick_jump);
    assert!(!tap_jump_facts.short_hop_released_for(MeleeJumpInput::LStick));
    assert!(!tap_jump_facts.short_hop_released_for(MeleeJumpInput::XY));
    assert!(!tap_jump_facts.short_hop_released_for(MeleeJumpInput::CStick));

    let released_to_neutral = processor.update(GameCubePadStatus::neutral());
    let released_facts = released_to_neutral.facts(thresholds);

    assert_eq!(released_facts.jump_input, MeleeJumpInput::None);
    assert!(!released_facts.button_jump_held);
    assert!(released_facts.lstick_jump_released);
    assert!(released_facts.cstick_jump_released);
    assert!(released_facts.short_hop_released_for(MeleeJumpInput::LStick));
    assert!(released_facts.short_hop_released_for(MeleeJumpInput::XY));
    assert!(released_facts.short_hop_released_for(MeleeJumpInput::CStick));
    assert!(!released_facts.short_hop_released_for(MeleeJumpInput::None));
}

#[test]
fn melee_input_facts_map_z_to_a_plus_pseudo_shield_without_air_dodge() {
    let thresholds = MeleeInputThresholds::default();
    let z_snapshot = PlayerInput::neutral()
        .with_grab(true)
        .melee_snapshot(PlayerInput::neutral(), MeleeInputTimers::expired());
    let z_facts = z_snapshot.facts(thresholds);

    assert!(z_snapshot.held.z());
    assert!(!z_snapshot.held.a());
    assert!(z_facts.source_held.z());
    assert!(z_facts.source_held.a());
    assert!(z_facts.source_held.lr());
    assert!(!z_facts.source_held.l());
    assert!(!z_facts.source_held.r());
    assert!(z_facts.source_pressed.z());
    assert!(z_facts.source_pressed.a());
    assert!(z_facts.source_pressed.lr());
    assert!(!z_facts.source_pressed.l());
    assert!(!z_facts.source_pressed.r());
    assert_eq!(z_facts.analog_shield, 49);
    assert!(z_facts.grab_pressed);
    assert!(z_facts.attack_pressed);
    assert!(z_facts.neutral_attack_pressed);
    assert!(z_facts.shield_held);
    assert!(z_facts.shield_pressed);
    assert!(!z_facts.digital_shield_pressed);
    assert!(!z_facts.air_dodge_pressed);
}

#[test]
fn melee_input_facts_map_z_after_digital_lr_shield_amount() {
    let thresholds = MeleeInputThresholds::default();
    let input = PlayerInput::neutral()
        .with_grab(true)
        .with_left_trigger_digital(true);
    let snapshot = input.melee_snapshot(PlayerInput::neutral(), MeleeInputTimers::expired());
    let facts = snapshot.facts(thresholds);

    assert!(snapshot.held.z());
    assert!(snapshot.held.l());
    assert!(facts.source_held.z());
    assert!(facts.source_held.a());
    assert!(facts.source_held.l());
    assert!(facts.source_held.lr());
    assert!(facts.digital_shield_pressed);
    assert_eq!(facts.analog_shield, 49);
}

#[test]
fn melee_input_facts_expose_source_lr_edges_after_trigger_synthesis() {
    let thresholds = MeleeInputThresholds::default();
    let mut processor = MeleeInputProcessor::new(MeleeInputConfig {
        trigger_threshold: 1,
        trigger_timer_threshold: 140,
        ..MeleeInputConfig::default()
    });

    let first = processor.update(GameCubePadStatus {
        left_trigger: 49,
        ..GameCubePadStatus::neutral()
    });
    let first_facts = first.facts(thresholds);

    assert!(first_facts.source_held.lr());
    assert!(first_facts.source_pressed.lr());
    assert!(!first_facts.source_released.lr());
    assert!(!first_facts.source_held.l());
    assert!(!first_facts.source_pressed.l());

    let held = processor.update(GameCubePadStatus {
        left_trigger: 49,
        ..GameCubePadStatus::neutral()
    });
    let held_facts = held.facts(thresholds);

    assert!(held_facts.source_held.lr());
    assert!(!held_facts.source_pressed.lr());
    assert!(!held_facts.source_released.lr());

    let released = processor.update(GameCubePadStatus::neutral());
    let released_facts = released.facts(thresholds);

    assert!(!released_facts.source_held.lr());
    assert!(!released_facts.source_pressed.lr());
    assert!(released_facts.source_released.lr());
}

#[test]
fn melee_input_source_lr_uses_cleaned_nonzero_trigger_before_shield_threshold() {
    let thresholds = MeleeInputThresholds::default();
    let mut processor = MeleeInputProcessor::new(MeleeInputConfig {
        trigger_threshold: 64,
        trigger_deadzone: 0,
        ..MeleeInputConfig::default()
    });

    let light = processor.update(GameCubePadStatus {
        left_trigger: 12,
        ..GameCubePadStatus::neutral()
    });
    let light_facts = light.facts(thresholds);

    assert!(!light_facts.shield_held);
    assert!(!light_facts.left_trigger_analog_held);
    assert!(light_facts.source_held.lr());
    assert!(light_facts.source_pressed.lr());

    let released = processor.update(GameCubePadStatus::neutral());
    let released_facts = released.facts(thresholds);

    assert!(!released_facts.source_held.lr());
    assert!(released_facts.source_released.lr());
}

#[test]
fn melee_input_facts_preserve_cstick_dpad_and_trigger_clicks() {
    let thresholds = MeleeInputThresholds {
        walk_x: 20,
        walk_slow_x: 20,
        walk_middle_x: 50,
        walk_fast_x: 90,
        dash_x: 80,
        dash_tap_window: 2,
        turn_x: 24,
        tilt_x: 24,
        tilt_y: 24,
        smash_y: 80,
        crouch_y: 36,
        tap_jump_y: 80,
        tap_jump_window: 2,
        tap_jump_release_y: 40,
        fast_fall_y: 80,
        fast_fall_window: 2,
        c_stick: 40,
        aerial_neutral_x: 40,
        aerial_neutral_y: 40,
        aerial_vertical_angle_tan_milli: 1000,
        z_shield_analog: 49,
        escape_x: 80,
        escape_x_tap_window: 2,
        escape_y: 80,
        escape_y_tap_window: 2,
        special_side_x: 40,
        special_vertical_y: 40,
    };
    let mut processor = MeleeInputProcessor::new(MeleeInputConfig {
        tap_x_threshold: 36,
        tap_y_threshold: 36,
        trigger_threshold: 64,
        ..MeleeInputConfig::default()
    });

    let pad = GameCubePadStatus {
        c_stick_x: 255,
        c_stick_y: 0,
        left_trigger: 70,
        buttons: GameCubeButtonState::from_bits((1 << 4) | (1 << 7) | (1 << 11)),
        ..GameCubePadStatus::neutral()
    };

    let first = processor.update(pad);
    let first_facts = first.facts(thresholds);

    assert_eq!(first_facts.cstick_direction, (1, -1));
    assert_eq!(first.left_trigger, 70);
    assert_eq!(first_facts.analog_shield, u8::MAX);
    assert!(first_facts.shield_pressed);
    assert!(first_facts.left_trigger_digital_pressed);
    assert!(!first_facts.right_trigger_digital_pressed);
    assert!(first_facts.dpad_up);
    assert!(first_facts.dpad_left);
    assert!(!first_facts.dpad_down);
    assert!(!first_facts.dpad_right);

    let held = processor.update(pad);
    let held_facts = held.facts(thresholds);

    assert!(held_facts.shield_held);
    assert!(!held_facts.shield_pressed);
    assert!(!held_facts.left_trigger_digital_pressed);
}

#[test]
fn melee_input_facts_separate_analog_shield_from_digital_airdodge_press() {
    let thresholds = MeleeInputThresholds::default();
    let mut processor = MeleeInputProcessor::new(MeleeInputConfig {
        trigger_threshold: 64,
        ..MeleeInputConfig::default()
    });

    let analog_left = processor.update(GameCubePadStatus {
        left_trigger: 90,
        ..GameCubePadStatus::neutral()
    });
    let analog_left_facts = analog_left.facts(thresholds);

    assert!(analog_left_facts.shield_pressed);
    assert!(analog_left_facts.analog_shield_pressed);
    assert!(!analog_left_facts.digital_shield_pressed);
    assert!(!analog_left_facts.air_dodge_pressed);
    assert!(analog_left_facts.left_trigger_analog_held);
    assert!(!analog_left_facts.right_trigger_analog_held);

    let right_digital = processor.update(GameCubePadStatus {
        left_trigger: 90,
        buttons: GameCubeButtonState::empty().with_r(true),
        ..GameCubePadStatus::neutral()
    });
    let right_digital_facts = right_digital.facts(thresholds);

    assert!(right_digital_facts.shield_held);
    assert!(!right_digital_facts.shield_pressed);
    assert!(!right_digital_facts.analog_shield_pressed);
    assert!(right_digital_facts.digital_shield_pressed);
    assert!(right_digital_facts.right_trigger_digital_pressed);
    assert!(right_digital_facts.air_dodge_pressed);
}

#[test]
fn melee_input_facts_map_held_digital_lr_to_full_shield_amount() {
    let thresholds = MeleeInputThresholds::default();
    let input = PlayerInput::neutral()
        .with_right_trigger_analog(12)
        .with_right_trigger_digital(true);
    let snapshot = input.melee_snapshot(PlayerInput::neutral(), MeleeInputTimers::expired());
    let facts = snapshot.facts(thresholds);

    assert_eq!(snapshot.right_trigger, 12);
    assert!(facts.right_trigger_digital_pressed);
    assert_eq!(facts.analog_shield, u8::MAX);
}

#[test]
fn melee_input_facts_preserve_same_side_digital_click_while_analog_trigger_held() {
    let thresholds = MeleeInputThresholds::default();
    let mut processor = MeleeInputProcessor::new(MeleeInputConfig {
        trigger_threshold: 64,
        ..MeleeInputConfig::default()
    });

    processor.update(GameCubePadStatus {
        left_trigger: 90,
        ..GameCubePadStatus::neutral()
    });
    let bottomed_left = processor.update(GameCubePadStatus {
        left_trigger: 90,
        buttons: GameCubeButtonState::empty().with_l(true),
        ..GameCubePadStatus::neutral()
    });
    let facts = bottomed_left.facts(thresholds);

    assert!(facts.shield_held);
    assert!(!facts.shield_pressed);
    assert!(!facts.analog_shield_pressed);
    assert!(facts.digital_shield_pressed);
    assert!(facts.left_trigger_digital_pressed);
    assert!(facts.air_dodge_pressed);
}

#[test]
fn same_start_and_inputs_produce_same_checksum() {
    let mut a = World::for_two_players();
    let mut b = World::for_two_players();
    let inputs = [
        PlayerInput::neutral().with_left_stick(127, 0),
        PlayerInput::neutral(),
    ];

    for frame in 0..120 {
        step_world(&mut a, Frame(frame), &inputs);
        step_world(&mut b, Frame(frame), &inputs);
    }

    assert_eq!(a.checksum(), b.checksum());
}

#[test]
fn world_owns_melee_input_timers_for_rollback() {
    let mut world = World::for_two_players();
    let neutral = PlayerInput::neutral();
    let right = PlayerInput::neutral().with_left_stick(40, 0);
    let right_trigger = PlayerInput::neutral().with_right_trigger_analog(140);

    assert_eq!(world.input_timers()[0].x_tap, 0xfe);
    assert_eq!(world.input_timers()[0].y_tap, 0xfe);
    assert_eq!(world.input_timers()[0].trigger, 0xfe);

    step_world(&mut world, Frame(0), &[right, neutral]);

    assert_eq!(world.input_timers()[0].x_tap, 0);
    assert_eq!(world.input_timers()[0].y_tap, 0xfe);
    assert_eq!(world.input_timers()[0].trigger, 0xfe);

    step_world(&mut world, Frame(1), &[right, neutral]);

    assert_eq!(world.input_timers()[0].x_tap, 1);

    step_world(&mut world, Frame(2), &[right_trigger, neutral]);

    assert_eq!(world.input_timers()[0].x_tap, 0xfe);
    assert_eq!(world.input_timers()[0].trigger, 0);

    step_world(&mut world, Frame(3), &[right_trigger, neutral]);

    assert_eq!(world.input_timers()[0].trigger, 1);
}

#[test]
fn world_snapshot_exposes_render_state_without_mutating_core() {
    let mut world = World::for_two_players();
    let inputs = [
        PlayerInput::neutral().with_left_stick(64, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &inputs);

    let mut snapshot = world.snapshot();
    let player = snapshot.players[0];

    assert_eq!(snapshot.frame, world.frame());
    assert_eq!(snapshot.checksum, world.checksum());
    assert_eq!(player.position, world.players()[0].position);
    assert_eq!(player.facing, world.players()[0].facing);
    assert_eq!(player.motion_state, MotionState::WalkMiddle);
    assert_eq!(player.state_frame, world.players()[0].motion_frame);
    assert_eq!(player.animation_frame, world.players()[0].attack_frame);
    assert_eq!(player.debug_input_facts.walk_direction, 1);
    assert_eq!(
        player.debug_input_facts.walk_speed_bucket,
        WalkSpeedBucket::Middle
    );

    snapshot.players[0].position.x += 777;

    assert_ne!(snapshot.players[0].position, world.players()[0].position);
}

#[test]
fn world_derives_melee_snapshot_from_rollback_owned_input_state() {
    let mut world = World::for_two_players();
    let neutral = PlayerInput::neutral();
    let input = PlayerInput::neutral()
        .with_left_stick(90, 0)
        .with_c_stick(0, 90)
        .with_attack(true)
        .with_right_trigger_analog(80)
        .with_right_trigger_digital(true)
        .with_dpad_up(true);

    let snapshot = world
        .melee_input_snapshot(0, input)
        .expect("player one snapshot should exist");
    let facts = snapshot.facts(MeleeInputThresholds::default());

    assert_eq!(snapshot.lstick, (90, 0));
    assert_eq!(snapshot.prev_lstick, (0, 0));
    assert_eq!(snapshot.cstick, (0, 90));
    assert_eq!(snapshot.prev_cstick, (0, 0));
    assert_eq!(snapshot.right_trigger, 80);
    assert!(snapshot.held.a());
    assert!(snapshot.held.r());
    assert!(snapshot.held.dpad_up());
    assert!(snapshot.pressed.a());
    assert!(snapshot.pressed.r());
    assert_eq!(snapshot.x_tap_timer, 0);
    assert_eq!(snapshot.trigger_timer, 0);
    assert_eq!(facts.dash_direction, 1);
    assert!(facts.attack_pressed);
    assert!(facts.air_dodge_pressed);
    assert_eq!(facts.jump_input, MeleeJumpInput::CStick);

    step_world(&mut world, Frame(0), &[input, neutral]);
    let held_snapshot = world
        .melee_input_snapshot(0, input)
        .expect("player one held snapshot should exist");
    let held_facts = held_snapshot.facts(MeleeInputThresholds::default());

    assert_eq!(held_snapshot.prev_lstick, (90, 0));
    assert_eq!(held_snapshot.x_tap_timer, 1);
    assert_eq!(held_snapshot.trigger_timer, 1);
    assert!(held_snapshot.held.a());
    assert!(!held_snapshot.pressed.a());
    assert!(!held_facts.attack_pressed);
    assert!(!held_facts.air_dodge_pressed);
}

#[test]
fn world_melee_snapshot_preserves_y_only_jump_button() {
    let world = World::for_two_players();
    let input = PlayerInput::neutral().with_jump_secondary(true);

    let snapshot = world
        .melee_input_snapshot(0, input)
        .expect("player one snapshot should exist");

    assert!(!snapshot.held.x());
    assert!(snapshot.held.y());
    assert!(!snapshot.pressed.x());
    assert!(snapshot.pressed.y());
    assert_eq!(
        snapshot.facts(MeleeInputThresholds::default()).jump_input,
        MeleeJumpInput::XY
    );
}

#[test]
fn world_melee_snapshot_treats_light_analog_trigger_as_shield_not_air_dodge() {
    let world = World::for_two_players();
    let input = PlayerInput::neutral().with_left_trigger_analog(42);

    let snapshot = world
        .melee_input_snapshot(0, input)
        .expect("player one snapshot should exist");
    let facts = snapshot.facts(MeleeInputThresholds::default());

    assert!(snapshot.shield_held);
    assert!(snapshot.shield_pressed);
    assert!(snapshot.left_trigger_analog_held);
    assert!(snapshot.left_trigger_analog_pressed);
    assert_eq!(snapshot.trigger_timer, 0xfe);
    assert_eq!(facts.analog_shield, 42);
    assert!(facts.analog_shield_pressed);
    assert!(!facts.digital_shield_pressed);
    assert!(!facts.air_dodge_pressed);
}

#[test]
fn grounded_jump_waits_through_falcon_jumpsquat_before_takeoff() {
    let mut world = World::for_two_players();
    let start_y = world.players()[0].position.y;
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &jump);

    assert!(world.players()[0].grounded);
    assert_eq!(world.players()[0].position.y, start_y);
    assert_eq!(world.players()[0].velocity.y, 0);

    for frame in 1..3 {
        step_world(&mut world, Frame(frame), &jump);
        assert!(world.players()[0].grounded);
        assert_eq!(world.players()[0].position.y, start_y);
    }

    step_world(&mut world, Frame(3), &jump);

    assert!(!world.players()[0].grounded);
    assert!(world.players()[0].position.y > start_y);
    assert!(world.players()[0].velocity.y > 0);
}

#[test]
fn ground_jump_takeoff_enters_forward_or_backward_jump_state_from_stick() {
    let mut forward = World::for_two_players();
    let mut backward = World::for_two_players();
    let forward_jump = [
        PlayerInput::neutral()
            .with_jump(true)
            .with_left_stick(20, 0),
        PlayerInput::neutral(),
    ];
    let backward_jump = [
        PlayerInput::neutral()
            .with_jump(true)
            .with_left_stick(-30, 0),
        PlayerInput::neutral(),
    ];

    for frame in 0..4 {
        step_world(&mut forward, Frame(frame), &forward_jump);
        step_world(&mut backward, Frame(frame), &backward_jump);
    }

    assert!(!forward.players()[0].grounded);
    assert_eq!(forward.players()[0].motion_state, MotionState::JumpF);
    assert!(!backward.players()[0].grounded);
    assert_eq!(backward.players()[0].motion_state, MotionState::JumpB);
}

#[test]
fn releasing_jump_during_jumpsquat_selects_short_hop_velocity() {
    let mut full_hop = World::for_two_players();
    let mut short_hop = World::for_two_players();
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    step_world(&mut full_hop, Frame(0), &jump);
    step_world(&mut short_hop, Frame(0), &jump);
    for frame in 1..4 {
        step_world(&mut full_hop, Frame(frame), &jump);
        step_world(&mut short_hop, Frame(frame), &neutral);
    }

    assert!(!full_hop.players()[0].grounded);
    assert!(!short_hop.players()[0].grounded);
    assert!(short_hop.players()[0].velocity.y < full_hop.players()[0].velocity.y);
    assert!(short_hop.players()[0].velocity.y > 0);
}

#[test]
fn ground_jump_first_airborne_tick_uses_falcon_jump_force_before_gravity() {
    let mut full_hop = World::for_two_players();
    let mut short_hop = World::for_two_players();
    let profile = FighterProfile::falcon_like();
    let start_y = full_hop.players()[0].position.y;
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    step_world(&mut full_hop, Frame(0), &jump);
    step_world(&mut short_hop, Frame(0), &jump);
    for frame in 1..4 {
        step_world(&mut full_hop, Frame(frame), &jump);
        step_world(&mut short_hop, Frame(frame), &neutral);
    }

    assert_eq!(
        full_hop.players()[0].position.y - start_y,
        profile.full_hop_jump_force_per_tick
    );
    assert_eq!(
        full_hop.players()[0].velocity.y,
        profile.full_hop_jump_force_per_tick - profile.gravity_per_tick
    );
    assert_eq!(
        short_hop.players()[0].position.y - start_y,
        profile.short_hop_jump_force_per_tick
    );
    assert_eq!(
        short_hop.players()[0].velocity.y,
        profile.short_hop_jump_force_per_tick - profile.gravity_per_tick
    );
}

#[test]
fn full_hop_apex_uses_falcon_profile_height() {
    let mut world = World::for_two_players();
    let profile = FighterProfile::falcon_like();
    let start_y = world.players()[0].position.y;
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let mut max_y = start_y;

    for frame in 0..120 {
        step_world(&mut world, Frame(frame), &jump);
        max_y = max_y.max(world.players()[0].position.y);
        if !world.players()[0].grounded && world.players()[0].velocity.y <= 0 {
            break;
        }
    }

    assert!(
        close_to(
            max_y - start_y,
            profile.full_hop_height,
            profile.gravity_per_tick
        ),
        "full hop apex should be near Falcon profile height; got {}, expected {}",
        max_y - start_y,
        profile.full_hop_height
    );
}

#[test]
fn short_hop_apex_uses_falcon_profile_height() {
    let mut world = World::for_two_players();
    let profile = FighterProfile::falcon_like();
    let start_y = world.players()[0].position.y;
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let mut max_y = start_y;

    step_world(&mut world, Frame(0), &jump);
    for frame in 1..120 {
        step_world(&mut world, Frame(frame), &neutral);
        max_y = max_y.max(world.players()[0].position.y);
        if !world.players()[0].grounded && world.players()[0].velocity.y <= 0 {
            break;
        }
    }

    assert!(
        close_to(
            max_y - start_y,
            profile.short_hop_height,
            profile.gravity_per_tick
        ),
        "short hop apex should be near Falcon profile height; got {}, expected {}",
        max_y - start_y,
        profile.short_hop_height
    );
}

#[test]
fn jump_takeoff_adds_horizontal_velocity_from_stick_and_ground_speed() {
    let mut standing_jump = World::for_two_players();
    let mut dash_jump = World::for_two_players();
    let forward_jump = [
        PlayerInput::neutral()
            .with_jump(true)
            .with_left_stick(80, 0),
        PlayerInput::neutral(),
    ];
    let dash_right = [
        PlayerInput::neutral().with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];

    for frame in 0..4 {
        step_world(&mut standing_jump, Frame(frame), &forward_jump);
    }

    step_world(&mut dash_jump, Frame(0), &dash_right);
    let dash_velocity = dash_jump.players()[0].velocity.x;
    for frame in 1..5 {
        step_world(&mut dash_jump, Frame(frame), &forward_jump);
    }

    assert!(!standing_jump.players()[0].grounded);
    assert!(!dash_jump.players()[0].grounded);
    assert!(standing_jump.players()[0].velocity.x > 0);
    assert!(dash_velocity > standing_jump.players()[0].velocity.x);
    assert!(dash_jump.players()[0].velocity.x > standing_jump.players()[0].velocity.x);
}

#[test]
fn air_drift_preserves_jump_horizontal_velocity_without_snapping_to_neutral() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];
    let jump_right = [
        PlayerInput::neutral()
            .with_jump(true)
            .with_left_stick(80, 0),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    step_world(&mut world, Frame(0), &dash_right);
    for frame in 1..5 {
        step_world(&mut world, Frame(frame), &jump_right);
    }

    let takeoff_velocity = world.players()[0].velocity.x;
    assert!(takeoff_velocity > 0);

    step_world(&mut world, Frame(5), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::JumpF);
    assert!(world.players()[0].velocity.x > 0);
    assert!(world.players()[0].velocity.x < takeoff_velocity);
}

#[test]
fn ground_jump_horizontal_velocity_uses_profile_source_fields() {
    let profile = FighterProfile {
        jump_horizontal_initial_velocity_per_tick: 700,
        ground_to_air_jump_momentum_milli: 800,
        jump_horizontal_max_velocity_per_tick: 550,
        ..FighterProfile::falcon_like()
    };
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let jump_right = [
        PlayerInput::neutral()
            .with_jump(true)
            .with_left_stick(100, 0),
        PlayerInput::neutral(),
    ];

    for frame in 0..4 {
        step_world(&mut world, Frame(frame), &jump_right);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::JumpF);
    assert_eq!(world.players()[0].velocity.x, 550);
}

#[test]
fn air_drift_uses_profile_source_accel_base_target_and_friction() {
    let profile = FighterProfile {
        jump_horizontal_initial_velocity_per_tick: 0,
        air_drift_stick_accel_per_tick: 40,
        air_drift_base_accel_per_tick: 20,
        air_drift_max_velocity_per_tick: 1_120,
        air_friction_per_tick: 10,
        air_max_horizontal_velocity_per_tick: 1_120,
        ..FighterProfile::falcon_like()
    };
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let drift_right = [
        PlayerInput::neutral().with_left_stick(100, 0),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    for frame in 0..4 {
        step_world(&mut world, Frame(frame), &jump);
    }
    step_world(&mut world, Frame(4), &drift_right);

    assert_eq!(world.players()[0].velocity.x, 60);

    step_world(&mut world, Frame(5), &neutral);

    assert_eq!(world.players()[0].velocity.x, 50);
}

#[test]
fn aerial_jump_horizontal_velocity_uses_profile_source_field() {
    let profile = FighterProfile {
        air_jump_horizontal_velocity_per_tick: 730,
        max_jumps: 1,
        ..FighterProfile::falcon_like()
    };
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let double_jump_right = [
        PlayerInput::neutral()
            .with_jump(true)
            .with_left_stick(100, 0),
        PlayerInput::neutral(),
    ];

    for frame in 0..4 {
        step_world(&mut world, Frame(frame), &jump);
    }
    step_world(&mut world, Frame(4), &neutral);
    step_world(&mut world, Frame(5), &double_jump_right);

    assert_eq!(world.players()[0].motion_state, MotionState::JumpAerialF);
    assert_eq!(world.players()[0].velocity.x, 730);
    assert_eq!(world.players()[0].jumps_remaining, 0);
}

#[test]
fn jumpsquat_accepts_up_special_before_takeoff() {
    let mut world = World::for_two_players();
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let up_special = [
        PlayerInput::neutral()
            .with_special(true)
            .with_left_stick(0, 80),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &jump);
    step_world(&mut world, Frame(1), &up_special);

    assert_eq!(world.players()[0].motion_state, MotionState::SpecialHi);
}

#[test]
fn jumpsquat_accepts_jump_cancel_grab_before_up_smash() {
    let mut world = World::for_two_players();
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let grab_and_up_smash = [
        PlayerInput::neutral()
            .with_grab(true)
            .with_attack(true)
            .with_left_stick(0, 90),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &jump);
    step_world(&mut world, Frame(1), &grab_and_up_smash);

    assert_eq!(world.players()[0].motion_state, MotionState::Catch);
}

#[test]
fn jumpsquat_accepts_jump_cancel_up_smash() {
    let mut world = World::for_two_players();
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let up_smash = [
        PlayerInput::neutral()
            .with_attack(true)
            .with_left_stick(0, 90),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &jump);
    step_world(&mut world, Frame(1), &up_smash);

    assert_eq!(world.players()[0].motion_state, MotionState::AttackHi4);
}

#[test]
fn held_jump_does_not_spend_air_jump_on_next_tick() {
    let mut world = World::for_two_players();
    let held_jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &held_jump);

    assert!(world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::KneeBend);
    assert_eq!(world.players()[0].jumps_remaining, 1);

    step_world(&mut world, Frame(1), &held_jump);

    assert!(world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::KneeBend);
    assert_eq!(world.players()[0].motion_frame, 2);
    assert_eq!(world.players()[0].jumps_remaining, 1);

    step_world(&mut world, Frame(2), &held_jump);
    step_world(&mut world, Frame(3), &held_jump);
    let takeoff_velocity = world.players()[0].velocity.y;

    assert!(!world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::JumpF);
    assert_eq!(world.players()[0].jumps_remaining, 1);

    step_world(&mut world, Frame(4), &held_jump);

    assert_eq!(world.players()[0].jumps_remaining, 1);
    assert!(world.players()[0].velocity.y < takeoff_velocity);
}

#[test]
fn fresh_jump_repress_spends_air_jump() {
    let mut world = World::for_two_players();
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    step_world(&mut world, Frame(0), &jump);
    step_world(&mut world, Frame(1), &neutral);

    let knee_bend_frame = world.players()[0].motion_frame;

    step_world(&mut world, Frame(2), &jump);

    assert_eq!(world.players()[0].motion_state, MotionState::KneeBend);
    assert_eq!(world.players()[0].jumps_remaining, 1);
    assert!(world.players()[0].motion_frame > knee_bend_frame);

    step_world(&mut world, Frame(3), &jump);

    assert_eq!(world.players()[0].motion_state, MotionState::JumpF);
    assert_eq!(world.players()[0].jumps_remaining, 1);

    step_world(&mut world, Frame(4), &neutral);
    step_world(&mut world, Frame(5), &jump);

    assert_eq!(world.players()[0].jumps_remaining, 0);
    assert!(world.players()[0].velocity.y > 0);
}

#[test]
fn shield_held_enters_guard_and_jump_uses_jumpsquat() {
    let mut world = World::for_two_players();
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];
    let shield_jump = [
        PlayerInput::neutral()
            .with_left_trigger_analog(80)
            .with_jump(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &shield);

    assert!(world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::GuardOn);

    step_world(&mut world, Frame(1), &shield_jump);

    assert!(world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::KneeBend);

    step_world(&mut world, Frame(2), &shield_jump);
    step_world(&mut world, Frame(3), &shield_jump);
    step_world(&mut world, Frame(4), &shield_jump);

    assert!(!world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::JumpF);
}

#[test]
fn shield_jump_height_is_selected_by_normal_jumpsquat_release_timing() {
    let mut full_hop = World::for_two_players();
    let mut short_hop = World::for_two_players();
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];
    let shield_jump = [
        PlayerInput::neutral()
            .with_left_trigger_analog(80)
            .with_jump(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut full_hop, Frame(0), &shield);
    step_world(&mut short_hop, Frame(0), &shield);

    step_world(&mut full_hop, Frame(1), &shield_jump);
    step_world(&mut short_hop, Frame(1), &shield_jump);

    assert_eq!(full_hop.players()[0].motion_state, MotionState::KneeBend);
    assert_eq!(short_hop.players()[0].motion_state, MotionState::KneeBend);

    for frame in 2..=4 {
        step_world(&mut full_hop, Frame(frame), &shield_jump);
        step_world(&mut short_hop, Frame(frame), &shield);
    }

    assert_eq!(full_hop.players()[0].motion_state, MotionState::JumpF);
    assert_eq!(short_hop.players()[0].motion_state, MotionState::JumpF);
    assert!(full_hop.players()[0].velocity.y > short_hop.players()[0].velocity.y);
    assert!(short_hop.players()[0].velocity.y > 0);
}

#[test]
fn steady_guard_jump_height_uses_normal_jumpsquat_release_timing() {
    let mut full_hop = World::for_two_players();
    let mut short_hop = World::for_two_players();
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];
    let shield_jump = [
        PlayerInput::neutral()
            .with_left_trigger_analog(80)
            .with_jump(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut full_hop, Frame(0), &shield);
    step_world(&mut short_hop, Frame(0), &shield);
    for frame in 1..=5 {
        step_world(&mut full_hop, Frame(frame), &shield);
        step_world(&mut short_hop, Frame(frame), &shield);
    }

    assert_eq!(full_hop.players()[0].motion_state, MotionState::Guard);
    assert_eq!(short_hop.players()[0].motion_state, MotionState::Guard);

    step_world(&mut full_hop, Frame(6), &shield_jump);
    step_world(&mut short_hop, Frame(6), &shield_jump);

    assert_eq!(full_hop.players()[0].motion_state, MotionState::KneeBend);
    assert_eq!(short_hop.players()[0].motion_state, MotionState::KneeBend);

    for frame in 7..=9 {
        step_world(&mut full_hop, Frame(frame), &shield_jump);
        step_world(&mut short_hop, Frame(frame), &shield);
    }

    assert_eq!(full_hop.players()[0].motion_state, MotionState::JumpF);
    assert_eq!(short_hop.players()[0].motion_state, MotionState::JumpF);
    assert!(full_hop.players()[0].velocity.y > short_hop.players()[0].velocity.y);
    assert!(short_hop.players()[0].velocity.y > 0);
}

#[test]
fn shield_tap_jump_stores_lstick_source_and_release_short_hops() {
    let mut world = World::for_two_players();
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];
    let shield_tap_jump = [
        PlayerInput::neutral()
            .with_left_trigger_analog(80)
            .with_left_stick(0, 90),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &shield);
    step_world(&mut world, Frame(1), &shield_tap_jump);

    assert_eq!(world.players()[0].motion_state, MotionState::KneeBend);
    assert_eq!(world.players()[0].jump_input, MeleeJumpInput::LStick);

    for frame in 2..=4 {
        step_world(&mut world, Frame(frame), &shield);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::JumpF);
    assert!(world.players()[0].velocity.y > 0);
}

#[test]
fn shield_cstick_jump_stores_cstick_source_and_release_short_hops() {
    let mut world = World::for_two_players();
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];
    let shield_cstick_jump = [
        PlayerInput::neutral()
            .with_left_trigger_analog(80)
            .with_c_stick(0, 90),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &shield);
    step_world(&mut world, Frame(1), &shield_cstick_jump);

    assert_eq!(world.players()[0].motion_state, MotionState::KneeBend);
    assert_eq!(world.players()[0].jump_input, MeleeJumpInput::CStick);

    for frame in 2..=4 {
        step_world(&mut world, Frame(frame), &shield);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::JumpF);
    assert!(world.players()[0].velocity.y > 0);
}

#[test]
fn held_cstick_jump_does_not_spend_air_jump_without_normal_jump_input() {
    let mut world = World::for_two_players();
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let jump_takeoff_with_cstick_up = [
        PlayerInput::neutral().with_jump(true).with_c_stick(0, 90),
        PlayerInput::neutral(),
    ];
    let held_cstick_up = [
        PlayerInput::neutral().with_c_stick(0, 90),
        PlayerInput::neutral(),
    ];

    for frame in 0..=2 {
        step_world(&mut world, Frame(frame), &jump);
    }
    step_world(&mut world, Frame(3), &jump_takeoff_with_cstick_up);

    assert_eq!(world.players()[0].motion_state, MotionState::JumpF);
    assert_eq!(world.players()[0].jumps_remaining, 1);

    step_world(&mut world, Frame(4), &held_cstick_up);

    assert_eq!(world.players()[0].motion_state, MotionState::JumpF);
    assert_eq!(world.players()[0].jumps_remaining, 1);
}

#[test]
fn held_cstick_jump_does_not_cancel_grounded_action_iasa_as_normal_jump() {
    let mut world = World::for_two_players();
    let jab = [
        PlayerInput::neutral().with_attack(true),
        PlayerInput::neutral(),
    ];
    let held_cstick_up = [
        PlayerInput::neutral().with_c_stick(0, 90),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &jab);
    assert_eq!(world.players()[0].motion_state, MotionState::Attack1);

    for frame in 1..=16 {
        step_world(&mut world, Frame(frame), &held_cstick_up);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::Attack1);
}

#[test]
fn shield_down_tap_enters_spotdodge_before_roll_or_grab() {
    let mut world = World::for_two_players();
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];
    let shield_down_side_attack = [
        PlayerInput::neutral()
            .with_left_trigger_analog(80)
            .with_left_stick(90, -90)
            .with_attack(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &shield);
    step_world(&mut world, Frame(1), &shield_down_side_attack);

    assert_eq!(world.players()[0].motion_state, MotionState::EscapeN);
}

#[test]
fn shield_horizontal_tap_enters_facing_aware_roll() {
    let mut forward = World::for_two_players();
    let mut back = World::for_two_players();
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];
    let shield_right = [
        PlayerInput::neutral()
            .with_left_trigger_analog(80)
            .with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];
    let shield_left = [
        PlayerInput::neutral()
            .with_left_trigger_analog(80)
            .with_left_stick(-90, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut forward, Frame(0), &shield);
    step_world(&mut back, Frame(0), &shield);
    step_world(&mut forward, Frame(1), &shield_right);
    step_world(&mut back, Frame(1), &shield_left);

    assert_eq!(forward.players()[0].motion_state, MotionState::EscapeF);
    assert_eq!(back.players()[0].motion_state, MotionState::EscapeB);
}

#[test]
fn shield_opposite_soft_hold_turns_facing_without_leaving_guard() {
    let mut world = World::for_two_players();
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];
    let shield_left = [
        PlayerInput::neutral()
            .with_left_trigger_analog(80)
            .with_left_stick(-40, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &shield);

    for frame in 1..5 {
        step_world(&mut world, Frame(frame), &shield_left);
        assert!(matches!(
            world.players()[0].motion_state,
            MotionState::GuardOn | MotionState::Guard
        ));
        assert_eq!(world.players()[0].facing, 1);
    }

    step_world(&mut world, Frame(5), &shield_left);

    assert_eq!(world.players()[0].motion_state, MotionState::Guard);
    assert_eq!(world.players()[0].facing, -1);
}

#[test]
fn shield_turn_updates_facing_for_next_roll_direction() {
    let mut world = World::for_two_players();
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];
    let shield_left = [
        PlayerInput::neutral()
            .with_left_trigger_analog(80)
            .with_left_stick(-40, 0),
        PlayerInput::neutral(),
    ];
    let shield_right_roll = [
        PlayerInput::neutral()
            .with_left_trigger_analog(80)
            .with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &shield);
    for frame in 1..=5 {
        step_world(&mut world, Frame(frame), &shield_left);
    }

    assert_eq!(world.players()[0].facing, -1);

    step_world(&mut world, Frame(6), &shield_right_roll);

    assert_eq!(world.players()[0].motion_state, MotionState::EscapeB);
}

#[test]
fn shield_turn_can_be_interrupted_by_normal_jump_squat() {
    let mut world = World::for_two_players();
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];
    let shield_left = [
        PlayerInput::neutral()
            .with_left_trigger_analog(80)
            .with_left_stick(-40, 0),
        PlayerInput::neutral(),
    ];
    let shield_jump = [
        PlayerInput::neutral()
            .with_left_trigger_analog(80)
            .with_jump(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &shield);
    step_world(&mut world, Frame(1), &shield_left);
    step_world(&mut world, Frame(2), &shield_left);
    step_world(&mut world, Frame(3), &shield_jump);

    assert_eq!(world.players()[0].motion_state, MotionState::KneeBend);

    step_world(&mut world, Frame(4), &shield_jump);
    step_world(&mut world, Frame(5), &shield_jump);
    step_world(&mut world, Frame(6), &shield_jump);

    assert_eq!(world.players()[0].motion_state, MotionState::JumpF);
    assert!(world.players()[0].velocity.y > 0);
}

#[test]
fn shield_grab_beats_jump_after_dodge_checks() {
    let mut attack_grab = World::for_two_players();
    let mut z_grab = World::for_two_players();
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];
    let shield_attack_jump = [
        PlayerInput::neutral()
            .with_left_trigger_analog(80)
            .with_attack(true)
            .with_jump(true),
        PlayerInput::neutral(),
    ];
    let shield_z_jump = [
        PlayerInput::neutral()
            .with_left_trigger_analog(80)
            .with_grab(true)
            .with_jump(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut attack_grab, Frame(0), &shield);
    step_world(&mut z_grab, Frame(0), &shield);
    step_world(&mut attack_grab, Frame(1), &shield_attack_jump);
    step_world(&mut z_grab, Frame(1), &shield_z_jump);

    assert_eq!(attack_grab.players()[0].motion_state, MotionState::Catch);
    assert_eq!(z_grab.players()[0].motion_state, MotionState::Catch);
}

#[test]
fn standing_guard_on_attack_routes_to_normal_catch_without_dash_window() {
    let mut world = World::for_two_players();
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];
    let shield_attack = [
        PlayerInput::neutral()
            .with_left_trigger_analog(80)
            .with_attack(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &shield);
    step_world(&mut world, Frame(1), &shield_attack);

    assert_eq!(world.players()[0].motion_state, MotionState::Catch);
}

#[test]
fn run_guard_on_attack_routes_to_catch_dash_while_window_is_active() {
    let mut world = World::for_two_players();
    let shield = [
        PlayerInput::neutral()
            .with_left_stick(80, 0)
            .with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];
    let shield_attack = [
        PlayerInput::neutral()
            .with_left_stick(80, 0)
            .with_left_trigger_analog(80)
            .with_attack(true),
        PlayerInput::neutral(),
    ];

    advance_player_to_run(&mut world);
    step_world(&mut world, Frame(16), &shield);

    assert_eq!(world.players()[0].motion_state, MotionState::GuardOn);

    step_world(&mut world, Frame(17), &shield_attack);

    assert_eq!(world.players()[0].motion_state, MotionState::CatchDash);
}

#[test]
fn shield_release_enters_guard_off_before_wait() {
    let mut world = World::for_two_players();
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    step_world(&mut world, Frame(0), &shield);
    step_world(&mut world, Frame(1), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::GuardOff);
}

#[test]
fn guard_off_returns_to_wait_after_shield_drop_lag() {
    let mut world = World::for_two_players();
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    step_world(&mut world, Frame(0), &shield);
    step_world(&mut world, Frame(1), &neutral);

    assert_current_action_returns_to_wait_after_frames(&mut world, 1, MotionState::GuardOff, 15);
}

#[test]
fn guard_off_can_jump_before_wait() {
    let mut world = World::for_two_players();
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &shield);
    step_world(&mut world, Frame(1), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::GuardOff);

    step_world(&mut world, Frame(2), &jump);

    assert_eq!(world.players()[0].motion_state, MotionState::KneeBend);
}

#[test]
fn guard_off_without_reflect_gate_checks_spotdodge_before_offense() {
    let mut world = World::for_two_players();
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let down_attack = [
        PlayerInput::neutral()
            .with_left_stick(0, -90)
            .with_attack(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &shield);
    step_world(&mut world, Frame(1), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::GuardOff);

    step_world(&mut world, Frame(2), &down_attack);

    assert_eq!(world.players()[0].motion_state, MotionState::EscapeN);
}

#[test]
fn guard_off_does_not_roll_or_dash_from_horizontal_tap() {
    let mut world = World::for_two_players();
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let horizontal_tap = [
        PlayerInput::neutral().with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &shield);
    step_world(&mut world, Frame(1), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::GuardOff);

    step_world(&mut world, Frame(2), &horizontal_tap);

    assert_eq!(world.players()[0].motion_state, MotionState::GuardOff);
}

#[test]
fn digital_trigger_on_takeoff_tick_does_not_airdodge_during_jumpsquat() {
    let mut world = World::for_two_players();
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let jump_with_digital_trigger = [
        PlayerInput::neutral()
            .with_jump(true)
            .with_right_trigger_digital(true)
            .with_left_stick(80, 80),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &jump);
    step_world(&mut world, Frame(1), &jump);
    step_world(&mut world, Frame(2), &jump);
    step_world(&mut world, Frame(3), &jump_with_digital_trigger);

    assert!(!world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::JumpF);
}

#[test]
fn fresh_digital_trigger_first_airborne_frame_enters_escape_air() {
    let mut world = World::for_two_players();
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let air_dodge = [
        PlayerInput::neutral()
            .with_right_trigger_digital(true)
            .with_left_stick(80, -80),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &jump);
    step_world(&mut world, Frame(1), &jump);
    step_world(&mut world, Frame(2), &jump);
    step_world(&mut world, Frame(3), &jump);
    step_world(&mut world, Frame(4), &air_dodge);

    assert!(!world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::EscapeAir);
    assert!(world.players()[0].velocity.x > 0);
    assert!(world.players()[0].velocity.y < 0);
}

#[test]
fn escape_air_stick_inside_deadzone_has_no_self_velocity() {
    let mut world = World::for_two_players();
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let air_dodge = [
        PlayerInput::neutral()
            .with_right_trigger_digital(true)
            .with_left_stick(
                MeleeCommonData::provisional_mole().escapeair_deadzone_x - 1,
                -(MeleeCommonData::provisional_mole().escapeair_deadzone_y - 1),
            ),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &jump);
    step_world(&mut world, Frame(1), &jump);
    step_world(&mut world, Frame(2), &jump);
    step_world(&mut world, Frame(3), &jump);
    step_world(&mut world, Frame(4), &air_dodge);

    assert_eq!(world.players()[0].motion_state, MotionState::EscapeAir);
    assert_eq!(world.players()[0].velocity.x, 0);
    assert_eq!(world.players()[0].velocity.y, 0);
}

#[test]
fn escape_air_uses_fixed_force_along_stick_angle() {
    let mut right = World::for_two_players();
    let mut diagonal = World::for_two_players();
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let right_air_dodge = [
        PlayerInput::neutral()
            .with_right_trigger_digital(true)
            .with_left_stick(127, 0),
        PlayerInput::neutral(),
    ];
    let diagonal_air_dodge = [
        PlayerInput::neutral()
            .with_right_trigger_digital(true)
            .with_left_stick(127, 127),
        PlayerInput::neutral(),
    ];

    for frame in 0..4 {
        step_world(&mut right, Frame(frame), &jump);
        step_world(&mut diagonal, Frame(frame), &jump);
    }
    step_world(&mut right, Frame(4), &right_air_dodge);
    step_world(&mut diagonal, Frame(4), &diagonal_air_dodge);

    let common = MeleeCommonData::provisional_mole();
    let force = common.escapeair_force * common.escapeair_decay_percent / 100;
    assert_eq!(right.players()[0].motion_state, MotionState::EscapeAir);
    assert_eq!(right.players()[0].velocity.x, force);
    assert_eq!(right.players()[0].velocity.y, 0);
    assert!(diagonal.players()[0].velocity.x > 0);
    assert!(diagonal.players()[0].velocity.y > 0);
    assert!(close_to(
        squared_magnitude(diagonal.players()[0].velocity),
        squared_magnitude(right.players()[0].velocity),
        force
    ));
}

#[test]
fn escape_air_self_velocity_decays_on_entry_frame() {
    let mut world = World::for_two_players();
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let right_air_dodge = [
        PlayerInput::neutral()
            .with_right_trigger_digital(true)
            .with_left_stick(127, 0),
        PlayerInput::neutral(),
    ];

    for frame in 0..4 {
        step_world(&mut world, Frame(frame), &jump);
    }
    step_world(&mut world, Frame(4), &right_air_dodge);

    let common = MeleeCommonData::provisional_mole();
    assert_eq!(world.players()[0].motion_state, MotionState::EscapeAir);
    assert_eq!(
        world.players()[0].velocity.x,
        common.escapeair_force * common.escapeair_decay_percent / 100
    );
}

#[test]
fn neutral_escape_air_does_not_apply_falling_gravity_during_action_phase() {
    let mut world = World::for_two_players();
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let neutral_air_dodge = [
        PlayerInput::neutral().with_right_trigger_digital(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &jump);
    step_world(&mut world, Frame(1), &jump);
    step_world(&mut world, Frame(2), &jump);
    step_world(&mut world, Frame(3), &jump);
    step_world(&mut world, Frame(4), &neutral_air_dodge);

    assert_eq!(world.players()[0].motion_state, MotionState::EscapeAir);
    assert_eq!(world.players()[0].velocity.y, 0);

    let height = world.players()[0].position.y;
    step_world(&mut world, Frame(5), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::EscapeAir);
    assert_eq!(world.players()[0].velocity.y, 0);
    assert_eq!(world.players()[0].position.y, height);
}

#[test]
fn escape_air_self_velocity_decays_during_action_phase() {
    let mut world = World::for_two_players();
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let right_air_dodge = [
        PlayerInput::neutral()
            .with_right_trigger_digital(true)
            .with_left_stick(80, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &jump);
    step_world(&mut world, Frame(1), &jump);
    step_world(&mut world, Frame(2), &jump);
    step_world(&mut world, Frame(3), &jump);
    step_world(&mut world, Frame(4), &right_air_dodge);

    let first_escape_velocity = world.players()[0].velocity.x;
    assert!(first_escape_velocity > 0);

    step_world(&mut world, Frame(5), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::EscapeAir);
    assert!(world.players()[0].velocity.x > 0);
    assert!(world.players()[0].velocity.x < first_escape_velocity);
    assert_eq!(
        world.players()[0].velocity.x,
        first_escape_velocity * MeleeCommonData::provisional_mole().escapeair_decay_percent / 100
    );
}

#[test]
fn escape_air_animation_end_enters_fall_special_while_airborne() {
    let mut world = World::for_two_players();
    let common = MeleeCommonData::provisional_mole();
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let up_air_dodge = [
        PlayerInput::neutral()
            .with_right_trigger_digital(true)
            .with_left_stick(80, 127),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &jump);
    step_world(&mut world, Frame(1), &jump);
    step_world(&mut world, Frame(2), &jump);
    step_world(&mut world, Frame(3), &jump);
    for frame in 4..8 {
        step_world(&mut world, Frame(frame), &neutral);
    }
    step_world(&mut world, Frame(8), &up_air_dodge);

    assert_eq!(world.players()[0].motion_state, MotionState::EscapeAir);
    assert_eq!(
        world.players()[0].escape_air_iasa_timer,
        common.escapeair_iasa_timer_ticks
    );
    let iasa_timer_end = 8 + common.escapeair_iasa_timer_ticks as u32;
    for frame in 9..=iasa_timer_end {
        step_world(&mut world, Frame(frame), &neutral);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::EscapeAir);
    assert_eq!(world.players()[0].escape_air_iasa_timer, 0);

    let escape_air_end = 8 + common.escapeair_animation_ticks as u32;
    for frame in (iasa_timer_end + 1)..=escape_air_end {
        step_world(&mut world, Frame(frame), &neutral);
    }

    assert!(!world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::FallSpecial);
}

#[test]
fn escape_air_iasa_timer_expiring_does_not_end_motion_state() {
    let mut world = World::for_two_players();
    let common = MeleeCommonData::provisional_mole();
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let up_air_dodge = [
        PlayerInput::neutral()
            .with_right_trigger_digital(true)
            .with_left_stick(80, 127),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &jump);
    step_world(&mut world, Frame(1), &jump);
    step_world(&mut world, Frame(2), &jump);
    step_world(&mut world, Frame(3), &jump);
    for frame in 4..8 {
        step_world(&mut world, Frame(frame), &neutral);
    }
    step_world(&mut world, Frame(8), &up_air_dodge);

    let iasa_timer_end = 8 + common.escapeair_iasa_timer_ticks as u32;
    for frame in 9..=iasa_timer_end {
        step_world(&mut world, Frame(frame), &neutral);
    }

    assert_eq!(world.players()[0].escape_air_iasa_timer, 0);
    assert_eq!(world.players()[0].motion_state, MotionState::EscapeAir);
}

#[test]
fn escape_air_animation_end_enters_fall_special_after_iasa_timer() {
    let mut world = World::for_two_players();
    let common = MeleeCommonData::provisional_mole();
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let up_air_dodge = [
        PlayerInput::neutral()
            .with_right_trigger_digital(true)
            .with_left_stick(80, 127),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &jump);
    step_world(&mut world, Frame(1), &jump);
    step_world(&mut world, Frame(2), &jump);
    step_world(&mut world, Frame(3), &jump);
    for frame in 4..8 {
        step_world(&mut world, Frame(frame), &neutral);
    }
    step_world(&mut world, Frame(8), &up_air_dodge);

    let escape_air_end = 8 + common.escapeair_animation_ticks as u32;
    for frame in 9..=escape_air_end {
        step_world(&mut world, Frame(frame), &neutral);
    }

    assert!(!world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::FallSpecial);
}

#[test]
fn fall_special_landing_enters_landing_fall_special() {
    let mut world = World::for_two_players();
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let up_air_dodge = [
        PlayerInput::neutral()
            .with_right_trigger_digital(true)
            .with_left_stick(80, 127),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &jump);
    step_world(&mut world, Frame(1), &jump);
    step_world(&mut world, Frame(2), &jump);
    step_world(&mut world, Frame(3), &jump);
    for frame in 4..8 {
        step_world(&mut world, Frame(frame), &neutral);
    }
    step_world(&mut world, Frame(8), &up_air_dodge);
    let escape_air_end = 8 + MeleeCommonData::provisional_mole().escapeair_animation_ticks as u32;
    for frame in 9..=escape_air_end {
        step_world(&mut world, Frame(frame), &neutral);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::FallSpecial);

    for frame in escape_air_end + 1..160 {
        step_world(&mut world, Frame(frame), &neutral);
        if world.players()[0].grounded {
            break;
        }
    }

    assert!(world.players()[0].grounded);
    assert_eq!(
        world.players()[0].motion_state,
        MotionState::LandingFallSpecial
    );
}

#[test]
fn ordinary_airborne_landing_enters_landing_not_wait() {
    let mut world = World::for_two_players();
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    advance_to_air(&mut world);
    let mut frame = 4;
    while !world.players()[0].grounded && frame < 120 {
        step_world(&mut world, Frame(frame), &neutral);
        frame += 1;
    }

    assert!(world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::Landing);
    assert_eq!(world.players()[0].motion_frame, 0);
    assert_eq!(world.players()[0].velocity.y, 0);
}

#[test]
fn held_shield_does_not_skip_ordinary_landing_lag() {
    let mut world = World::for_two_players();
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];

    advance_to_air(&mut world);
    let mut frame = 4;
    while !world.players()[0].grounded && frame < 120 {
        step_world(&mut world, Frame(frame), &shield);
        frame += 1;
    }

    assert_eq!(world.players()[0].motion_state, MotionState::Landing);

    for expected_motion_frame in 1..4 {
        step_world(&mut world, Frame(frame), &shield);
        frame += 1;
        assert_eq!(world.players()[0].motion_state, MotionState::Landing);
        assert_eq!(world.players()[0].motion_frame, expected_motion_frame);
    }

    step_world(&mut world, Frame(frame), &shield);
    frame += 1;

    assert_eq!(world.players()[0].motion_state, MotionState::Wait);

    step_world(&mut world, Frame(frame), &shield);

    assert_eq!(world.players()[0].motion_state, MotionState::GuardOn);
}

#[test]
fn ordinary_landing_duration_uses_player_profile_normal_landing_lag() {
    let profile = FighterProfile {
        normal_landing_lag_ticks: 2,
        ..FighterProfile::falcon_like()
    };
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    advance_to_air(&mut world);
    let mut frame = 4;
    while !world.players()[0].grounded && frame < 120 {
        step_world(&mut world, Frame(frame), &neutral);
        frame += 1;
    }

    assert!(world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::Landing);
    assert_eq!(world.players()[0].motion_frame, 0);

    step_world(&mut world, Frame(frame), &neutral);
    frame += 1;

    assert_eq!(world.players()[0].motion_state, MotionState::Landing);
    assert_eq!(world.players()[0].motion_frame, 1);

    step_world(&mut world, Frame(frame), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::Wait);
    assert_eq!(world.players()[0].motion_frame, 0);
}

#[test]
fn fall_special_accepts_air_jump_like_source_iasa() {
    let mut world = World::for_two_players();
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let up_air_dodge = [
        PlayerInput::neutral()
            .with_right_trigger_digital(true)
            .with_left_stick(80, 127),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &jump);
    step_world(&mut world, Frame(1), &jump);
    step_world(&mut world, Frame(2), &jump);
    step_world(&mut world, Frame(3), &jump);
    for frame in 4..8 {
        step_world(&mut world, Frame(frame), &neutral);
    }
    step_world(&mut world, Frame(8), &up_air_dodge);
    let escape_air_end = 8 + MeleeCommonData::provisional_mole().escapeair_animation_ticks as u32;
    for frame in 9..=escape_air_end {
        step_world(&mut world, Frame(frame), &neutral);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::FallSpecial);
    assert_eq!(world.players()[0].jumps_remaining, 1);

    step_world(&mut world, Frame(escape_air_end + 1), &jump);

    assert_eq!(world.players()[0].motion_state, MotionState::JumpAerialF);
    assert_eq!(world.players()[0].jumps_remaining, 0);
    assert!(world.players()[0].velocity.y > 0);
}

#[test]
fn aerial_jump_enters_jump_aerial_forward_state() {
    let mut world = World::for_two_players();
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let double_jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &jump);
    step_world(&mut world, Frame(1), &jump);
    step_world(&mut world, Frame(2), &jump);
    step_world(&mut world, Frame(3), &jump);
    step_world(&mut world, Frame(4), &neutral);
    step_world(&mut world, Frame(5), &double_jump);

    assert_eq!(world.players()[0].motion_state, MotionState::JumpAerialF);
    assert_eq!(world.players()[0].jumps_remaining, 0);
    assert!(world.players()[0].velocity.y > 0);
}

#[test]
fn aerial_jump_first_tick_uses_falcon_air_jump_force_before_gravity() {
    let mut world = World::for_two_players();
    let profile = FighterProfile::falcon_like();
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let double_jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &jump);
    step_world(&mut world, Frame(1), &jump);
    step_world(&mut world, Frame(2), &jump);
    step_world(&mut world, Frame(3), &jump);
    step_world(&mut world, Frame(4), &neutral);

    let before_y = world.players()[0].position.y;
    step_world(&mut world, Frame(5), &double_jump);

    assert_eq!(world.players()[0].motion_state, MotionState::JumpAerialF);
    assert_eq!(
        world.players()[0].position.y - before_y,
        profile.air_jump_force_per_tick
    );
    assert_eq!(
        world.players()[0].velocity.y,
        profile.air_jump_force_per_tick - profile.gravity_per_tick
    );
}

#[test]
fn aerial_jump_with_hard_back_stick_enters_jump_aerial_back_state() {
    let mut world = World::for_two_players();
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let back_double_jump = [
        PlayerInput::neutral()
            .with_jump(true)
            .with_left_stick(-80, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &jump);
    step_world(&mut world, Frame(1), &jump);
    step_world(&mut world, Frame(2), &jump);
    step_world(&mut world, Frame(3), &jump);
    step_world(&mut world, Frame(4), &neutral);
    step_world(&mut world, Frame(5), &back_double_jump);

    assert_eq!(world.players()[0].motion_state, MotionState::JumpAerialB);
    assert_eq!(world.players()[0].jumps_remaining, 0);
    assert!(world.players()[0].velocity.x < 0);
    assert!(world.players()[0].velocity.y > 0);
}

#[test]
fn escape_air_landing_enters_landing_fall_special_instead_of_wait() {
    let mut world = World::for_two_players();
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let down_air_dodge = [
        PlayerInput::neutral()
            .with_right_trigger_digital(true)
            .with_left_stick(80, -127),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &jump);
    step_world(&mut world, Frame(1), &jump);
    step_world(&mut world, Frame(2), &jump);
    step_world(&mut world, Frame(3), &jump);
    step_world(&mut world, Frame(4), &down_air_dodge);
    for frame in 5..80 {
        if world.players()[0].grounded {
            break;
        }
        step_world(
            &mut world,
            Frame(frame),
            &[PlayerInput::neutral(), PlayerInput::neutral()],
        );
    }

    let player = world.players()[0];
    assert!(player.grounded);
    assert_eq!(player.motion_state, MotionState::LandingFallSpecial);
    assert!(player.velocity.x > 0);
    assert_eq!(player.velocity.y, 0);
}

#[test]
fn landing_fall_special_preserves_slide_before_returning_to_wait() {
    let mut world = World::for_two_players();
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let down_air_dodge = [
        PlayerInput::neutral()
            .with_right_trigger_digital(true)
            .with_left_stick(80, -127),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    step_world(&mut world, Frame(0), &jump);
    step_world(&mut world, Frame(1), &jump);
    step_world(&mut world, Frame(2), &jump);
    step_world(&mut world, Frame(3), &jump);
    step_world(&mut world, Frame(4), &down_air_dodge);
    let mut frame = 5;
    while !world.players()[0].grounded && frame < 80 {
        step_world(&mut world, Frame(frame), &neutral);
        frame += 1;
    }

    let landing_velocity = world.players()[0].velocity.x;
    for frame in frame..frame + 4 {
        step_world(&mut world, Frame(frame), &neutral);
    }

    assert_eq!(
        world.players()[0].motion_state,
        MotionState::LandingFallSpecial
    );
    assert!(world.players()[0].velocity.x > 0);
    assert!(world.players()[0].velocity.x < landing_velocity);

    for frame in frame + 4..frame + 10 {
        step_world(&mut world, Frame(frame), &neutral);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::Wait);
}

#[test]
fn shield_jump_digital_trigger_on_takeoff_tick_does_not_escape_air() {
    let mut world = World::for_two_players();
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];
    let shield_jump = [
        PlayerInput::neutral()
            .with_left_trigger_analog(80)
            .with_jump(true),
        PlayerInput::neutral(),
    ];
    let shield_jump_with_other_digital = [
        PlayerInput::neutral()
            .with_left_trigger_analog(80)
            .with_jump(true)
            .with_right_trigger_digital(true)
            .with_left_stick(80, -80),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &shield);
    step_world(&mut world, Frame(1), &shield_jump);
    step_world(&mut world, Frame(2), &shield_jump);
    step_world(&mut world, Frame(3), &shield_jump);
    step_world(&mut world, Frame(4), &shield_jump_with_other_digital);

    assert!(!world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::JumpF);
}

#[test]
fn shield_jump_first_airborne_fresh_other_digital_trigger_enters_escape_air() {
    let mut world = World::for_two_players();
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];
    let shield_jump = [
        PlayerInput::neutral()
            .with_left_trigger_analog(80)
            .with_jump(true),
        PlayerInput::neutral(),
    ];
    let air_dodge = [
        PlayerInput::neutral()
            .with_left_trigger_analog(80)
            .with_right_trigger_digital(true)
            .with_left_stick(80, -80),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &shield);
    step_world(&mut world, Frame(1), &shield_jump);
    step_world(&mut world, Frame(2), &shield_jump);
    step_world(&mut world, Frame(3), &shield_jump);
    step_world(&mut world, Frame(4), &shield_jump);
    step_world(&mut world, Frame(5), &air_dodge);

    assert!(!world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::EscapeAir);
    assert!(world.players()[0].velocity.x > 0);
    assert!(world.players()[0].velocity.y < 0);
}

#[test]
fn shield_jump_first_airborne_fresh_same_trigger_digital_enters_escape_air() {
    let mut world = World::for_two_players();
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];
    let shield_jump = [
        PlayerInput::neutral()
            .with_left_trigger_analog(80)
            .with_jump(true),
        PlayerInput::neutral(),
    ];
    let bottomed_left_air_dodge = [
        PlayerInput::neutral()
            .with_left_trigger_analog(80)
            .with_left_trigger_digital(true)
            .with_left_stick(80, -80),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &shield);
    step_world(&mut world, Frame(1), &shield_jump);
    step_world(&mut world, Frame(2), &shield_jump);
    step_world(&mut world, Frame(3), &shield_jump);
    step_world(&mut world, Frame(4), &shield_jump);
    step_world(&mut world, Frame(5), &bottomed_left_air_dodge);

    assert!(!world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::EscapeAir);
    assert!(world.players()[0].velocity.x > 0);
    assert!(world.players()[0].velocity.y < 0);
}

#[test]
fn shield_jump_first_airborne_air_dodge_beats_aerial_attack() {
    let mut world = World::for_two_players();
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];
    let shield_jump = [
        PlayerInput::neutral()
            .with_left_trigger_analog(80)
            .with_jump(true),
        PlayerInput::neutral(),
    ];
    let attack_air_dodge = [
        PlayerInput::neutral()
            .with_attack(true)
            .with_right_trigger_digital(true)
            .with_left_stick(80, -80),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &shield);
    step_world(&mut world, Frame(1), &shield_jump);
    step_world(&mut world, Frame(2), &shield_jump);
    step_world(&mut world, Frame(3), &shield_jump);
    step_world(&mut world, Frame(4), &shield_jump);
    step_world(&mut world, Frame(5), &attack_air_dodge);

    assert!(!world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::EscapeAir);
}

#[test]
fn shield_jump_first_airborne_b_special_beats_air_dodge() {
    let mut world = World::for_two_players();
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];
    let shield_jump = [
        PlayerInput::neutral()
            .with_left_trigger_analog(80)
            .with_jump(true),
        PlayerInput::neutral(),
    ];
    let side_b_air_dodge = [
        PlayerInput::neutral()
            .with_special(true)
            .with_right_trigger_digital(true)
            .with_left_stick(-80, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &shield);
    step_world(&mut world, Frame(1), &shield_jump);
    step_world(&mut world, Frame(2), &shield_jump);
    step_world(&mut world, Frame(3), &shield_jump);
    step_world(&mut world, Frame(4), &shield_jump);
    step_world(&mut world, Frame(5), &side_b_air_dodge);

    assert!(!world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::SpecialAirS);
    assert_eq!(world.players()[0].facing, -1);
}

#[test]
fn shield_jump_held_digital_trigger_after_takeoff_tick_needs_new_edge() {
    let mut world = World::for_two_players();
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];
    let shield_jump = [
        PlayerInput::neutral()
            .with_left_trigger_analog(80)
            .with_jump(true),
        PlayerInput::neutral(),
    ];
    let shield_jump_with_other_digital = [
        PlayerInput::neutral()
            .with_left_trigger_analog(80)
            .with_jump(true)
            .with_right_trigger_digital(true)
            .with_left_stick(80, -80),
        PlayerInput::neutral(),
    ];
    let held_other_digital = [
        PlayerInput::neutral()
            .with_left_trigger_analog(80)
            .with_right_trigger_digital(true)
            .with_left_stick(80, -80),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &shield);
    step_world(&mut world, Frame(1), &shield_jump);
    step_world(&mut world, Frame(2), &shield_jump);
    step_world(&mut world, Frame(3), &shield_jump);
    step_world(&mut world, Frame(4), &shield_jump_with_other_digital);
    step_world(&mut world, Frame(5), &held_other_digital);

    assert!(!world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::JumpF);
}

#[test]
fn analog_trigger_first_airborne_frame_does_not_escape_air() {
    let mut world = World::for_two_players();
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let analog_shield = [
        PlayerInput::neutral()
            .with_right_trigger_analog(80)
            .with_left_stick(80, -80),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &jump);
    step_world(&mut world, Frame(1), &jump);
    step_world(&mut world, Frame(2), &jump);
    step_world(&mut world, Frame(3), &jump);
    step_world(&mut world, Frame(4), &analog_shield);

    assert!(!world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::JumpF);
}

#[test]
fn held_digital_trigger_after_takeoff_tick_does_not_airdodge_without_new_edge() {
    let mut world = World::for_two_players();
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let jump_with_digital_trigger = [
        PlayerInput::neutral()
            .with_jump(true)
            .with_right_trigger_digital(true)
            .with_left_stick(80, -80),
        PlayerInput::neutral(),
    ];
    let held_digital_trigger = [
        PlayerInput::neutral()
            .with_right_trigger_digital(true)
            .with_left_stick(80, -80),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &jump);
    step_world(&mut world, Frame(1), &jump);
    step_world(&mut world, Frame(2), &jump);
    step_world(&mut world, Frame(3), &jump_with_digital_trigger);
    step_world(&mut world, Frame(4), &held_digital_trigger);

    assert!(!world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::JumpF);
}

#[test]
fn b_special_direction_in_air_matches_air_priority() {
    let mut up = World::for_two_players();
    let mut down = World::for_two_players();
    let mut side = World::for_two_players();
    let mut neutral = World::for_two_players();
    let mut diagonal_up_first = World::for_two_players();
    let mut diagonal_down_first = World::for_two_players();
    let up_special = [
        PlayerInput::neutral()
            .with_special(true)
            .with_left_stick(0, 90),
        PlayerInput::neutral(),
    ];
    let down_special = [
        PlayerInput::neutral()
            .with_special(true)
            .with_left_stick(0, -90),
        PlayerInput::neutral(),
    ];
    let side_special = [
        PlayerInput::neutral()
            .with_special(true)
            .with_left_stick(-90, 0),
        PlayerInput::neutral(),
    ];
    let neutral_special = [
        PlayerInput::neutral().with_special(true),
        PlayerInput::neutral(),
    ];
    let diagonal_up_special = [
        PlayerInput::neutral()
            .with_special(true)
            .with_left_stick(-90, 90),
        PlayerInput::neutral(),
    ];
    let diagonal_down_special = [
        PlayerInput::neutral()
            .with_special(true)
            .with_left_stick(-90, -90),
        PlayerInput::neutral(),
    ];

    advance_to_air(&mut up);
    advance_to_air(&mut down);
    advance_to_air(&mut side);
    advance_to_air(&mut neutral);
    advance_to_air(&mut diagonal_up_first);
    advance_to_air(&mut diagonal_down_first);
    step_world(&mut up, Frame(4), &up_special);
    step_world(&mut down, Frame(4), &down_special);
    step_world(&mut side, Frame(4), &side_special);
    step_world(&mut neutral, Frame(4), &neutral_special);
    step_world(&mut diagonal_up_first, Frame(4), &diagonal_up_special);
    step_world(&mut diagonal_down_first, Frame(4), &diagonal_down_special);

    assert_eq!(up.players()[0].motion_state, MotionState::SpecialAirHi);
    assert_eq!(down.players()[0].motion_state, MotionState::SpecialAirLw);
    assert_eq!(side.players()[0].motion_state, MotionState::SpecialAirS);
    assert_eq!(side.players()[0].facing, -1);
    assert_eq!(neutral.players()[0].motion_state, MotionState::SpecialAirN);
    assert_eq!(
        diagonal_up_first.players()[0].motion_state,
        MotionState::SpecialAirHi
    );
    assert_eq!(
        diagonal_down_first.players()[0].motion_state,
        MotionState::SpecialAirLw
    );
}

#[test]
fn air_special_has_priority_over_air_jump_and_escape_air() {
    let mut jump_priority = World::for_two_players();
    let mut escape_priority = World::for_two_players();
    let b_with_jump = [
        PlayerInput::neutral()
            .with_special(true)
            .with_jump(true)
            .with_left_stick(0, 90),
        PlayerInput::neutral(),
    ];
    let b_with_digital_trigger = [
        PlayerInput::neutral()
            .with_special(true)
            .with_right_trigger_digital(true)
            .with_left_stick(-90, 0),
        PlayerInput::neutral(),
    ];

    advance_to_air(&mut jump_priority);
    advance_to_air(&mut escape_priority);
    step_world(&mut jump_priority, Frame(4), &b_with_jump);
    step_world(&mut escape_priority, Frame(4), &b_with_digital_trigger);

    assert_eq!(
        jump_priority.players()[0].motion_state,
        MotionState::SpecialAirHi
    );
    assert_eq!(
        escape_priority.players()[0].motion_state,
        MotionState::SpecialAirS
    );
}

#[test]
fn a_press_air_attack_direction_matches_stick_angle_and_facing() {
    let mut neutral = World::for_two_players();
    let mut forward = World::for_two_players();
    let mut back = World::for_two_players();
    let mut up = World::for_two_players();
    let mut down = World::for_two_players();
    let neutral_air = [
        PlayerInput::neutral().with_attack(true),
        PlayerInput::neutral(),
    ];
    let forward_air = [
        PlayerInput::neutral()
            .with_attack(true)
            .with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];
    let back_air = [
        PlayerInput::neutral()
            .with_attack(true)
            .with_left_stick(-90, 0),
        PlayerInput::neutral(),
    ];
    let up_air = [
        PlayerInput::neutral()
            .with_attack(true)
            .with_left_stick(0, 90),
        PlayerInput::neutral(),
    ];
    let down_air = [
        PlayerInput::neutral()
            .with_attack(true)
            .with_left_stick(0, -90),
        PlayerInput::neutral(),
    ];

    advance_to_air(&mut neutral);
    advance_to_air(&mut forward);
    advance_to_air(&mut back);
    advance_to_air(&mut up);
    advance_to_air(&mut down);
    step_world(&mut neutral, Frame(4), &neutral_air);
    step_world(&mut forward, Frame(4), &forward_air);
    step_world(&mut back, Frame(4), &back_air);
    step_world(&mut up, Frame(4), &up_air);
    step_world(&mut down, Frame(4), &down_air);

    assert_eq!(neutral.players()[0].motion_state, MotionState::AttackAirN);
    assert_eq!(forward.players()[0].motion_state, MotionState::AttackAirF);
    assert_eq!(back.players()[0].motion_state, MotionState::AttackAirB);
    assert_eq!(up.players()[0].motion_state, MotionState::AttackAirHi);
    assert_eq!(down.players()[0].motion_state, MotionState::AttackAirLw);
}

#[test]
fn fresh_cstick_air_attack_works_without_a_and_overrides_main_stick() {
    let mut cstick_only = World::for_two_players();
    let mut cstick_over_a = World::for_two_players();
    let cstick_up = [
        PlayerInput::neutral().with_c_stick(0, 90),
        PlayerInput::neutral(),
    ];
    let a_forward_cstick_back = [
        PlayerInput::neutral()
            .with_attack(true)
            .with_left_stick(90, 0)
            .with_c_stick(-90, 0),
        PlayerInput::neutral(),
    ];

    advance_to_air(&mut cstick_only);
    advance_to_air(&mut cstick_over_a);
    step_world(&mut cstick_only, Frame(4), &cstick_up);
    step_world(&mut cstick_over_a, Frame(4), &a_forward_cstick_back);

    assert_eq!(
        cstick_only.players()[0].motion_state,
        MotionState::AttackAirHi
    );
    assert_eq!(
        cstick_over_a.players()[0].motion_state,
        MotionState::AttackAirB
    );
}

#[test]
fn aerial_attack_direction_uses_source_xdc_xe0_neutral_zone() {
    let thresholds = MeleeInputThresholds {
        c_stick: 40,
        aerial_neutral_x: 60,
        aerial_neutral_y: 60,
        ..MeleeInputThresholds::default()
    };
    let a_with_main_inside_aerial_neutral = MeleeInputSnapshot {
        lstick: (50, 0),
        pressed: GameCubeButtonState::empty().with_a(true),
        ..snapshot_with_timers((50, 0), (0, 0), 254, 254)
    };
    let cstick_below_aerial_edge = MeleeInputSnapshot {
        cstick: (50, 0),
        prev_cstick: (0, 0),
        ..snapshot_with_timers((0, 0), (50, 0), 254, 254)
    };

    let a_facts = a_with_main_inside_aerial_neutral.facts(thresholds);
    let cstick_facts = cstick_below_aerial_edge.facts(thresholds);

    assert!(a_facts.air_attack_pressed);
    assert_eq!(a_facts.air_attack_direction, (0, 0));
    assert!(!cstick_facts.air_attack_pressed);
    assert_eq!(cstick_facts.air_attack_direction, (0, 0));
}

#[test]
fn aerial_attack_direction_uses_source_angle_gate_before_vertical_aerials() {
    let thresholds = MeleeInputThresholds {
        aerial_neutral_x: 40,
        aerial_neutral_y: 40,
        aerial_vertical_angle_tan_milli: 1500,
        ..MeleeInputThresholds::default()
    };
    let shallow_up_diagonal = MeleeInputSnapshot {
        lstick: (60, 80),
        pressed: GameCubeButtonState::empty().with_a(true),
        ..snapshot_with_timers((60, 80), (0, 0), 254, 254)
    };
    let steep_up_diagonal = MeleeInputSnapshot {
        lstick: (50, 80),
        pressed: GameCubeButtonState::empty().with_a(true),
        ..snapshot_with_timers((50, 80), (0, 0), 254, 254)
    };

    let shallow_facts = shallow_up_diagonal.facts(thresholds);
    let steep_facts = steep_up_diagonal.facts(thresholds);

    assert_eq!(shallow_facts.air_attack_direction, (1, 0));
    assert_eq!(steep_facts.air_attack_direction, (0, 1));
}

#[test]
fn air_dodge_beats_aerial_attack_and_aerial_attack_beats_air_jump() {
    let mut escape_priority = World::for_two_players();
    let mut attack_priority = World::for_two_players();
    let attack_with_trigger = [
        PlayerInput::neutral()
            .with_attack(true)
            .with_right_trigger_digital(true)
            .with_left_stick(90, -80),
        PlayerInput::neutral(),
    ];
    let attack_with_jump = [
        PlayerInput::neutral()
            .with_attack(true)
            .with_jump(true)
            .with_left_stick(0, 90),
        PlayerInput::neutral(),
    ];

    advance_to_air(&mut escape_priority);
    advance_to_air(&mut attack_priority);
    step_world(&mut escape_priority, Frame(4), &attack_with_trigger);
    step_world(&mut attack_priority, Frame(4), &attack_with_jump);

    assert_eq!(
        escape_priority.players()[0].motion_state,
        MotionState::EscapeAir
    );
    assert_eq!(
        attack_priority.players()[0].motion_state,
        MotionState::AttackAirHi
    );
}

#[test]
fn airborne_z_without_item_or_tether_enters_aerial_attack_not_generic_catch() {
    let mut world = World::for_two_players();
    let grab = [
        PlayerInput::neutral().with_grab(true),
        PlayerInput::neutral(),
    ];

    advance_to_air(&mut world);
    step_world(&mut world, Frame(4), &grab);

    assert_eq!(world.players()[0].motion_state, MotionState::AttackAirN);
}

#[test]
fn fresh_forward_dash_tap_from_wait_enters_dash_and_consumes_x_tap() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    assert_eq!(world.players()[0].facing, 1);
    assert!(world.players()[0].velocity.x > 0);
    assert_eq!(world.input_timers()[0].x_tap, 0xfe);
}

#[test]
fn fresh_opposite_dash_tap_from_wait_enters_turn_not_dash() {
    let mut world = World::for_two_players();
    let dash_left = [
        PlayerInput::neutral().with_left_stick(-90, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_left);

    assert_eq!(world.players()[0].motion_state, MotionState::Turn);
    assert_eq!(world.players()[0].facing, 1);
    assert_eq!(world.players()[0].velocity.x, 0);

    step_world(
        &mut world,
        Frame(1),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    assert_eq!(world.players()[0].motion_state, MotionState::Turn);
    assert_eq!(world.players()[0].facing, -1);
}

#[test]
fn fresh_opposite_dash_tap_from_wait_beats_crouch() {
    let mut world = World::for_two_players();
    let dash_left_down = [
        PlayerInput::neutral().with_left_stick(-90, -90),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_left_down);

    assert_eq!(world.players()[0].motion_state, MotionState::Turn);
    assert_eq!(world.players()[0].facing, 1);
}

#[test]
fn soft_stick_from_wait_enters_walk_not_dash() {
    let mut world = World::for_two_players();
    let walk_right = [
        PlayerInput::neutral().with_left_stick(40, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &walk_right);

    assert_eq!(world.players()[0].motion_state, MotionState::WalkSlow);
    assert_eq!(world.players()[0].facing, 1);
    assert!(world.players()[0].velocity.x > 0);
    assert_eq!(world.input_timers()[0].x_tap, 0);
}

#[test]
fn wait_enters_walk_slow_from_soft_forward_stick() {
    let mut world = World::for_two_players();
    let walk = [
        PlayerInput::neutral().with_left_stick(30, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &walk);

    assert_eq!(world.players()[0].motion_state, MotionState::WalkSlow);
    assert_eq!(world.players()[0].facing, 1);
    assert_eq!(world.players()[0].motion_frame, 0);
    assert!(world.players()[0].velocity.x > 0);
    assert!(world.players()[0].velocity.x < 30 * 6);
}

#[test]
fn wait_enters_walk_middle_from_mid_forward_stick() {
    let mut world = World::for_two_players();
    let walk = [
        PlayerInput::neutral().with_left_stick(64, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &walk);

    assert_eq!(world.players()[0].motion_state, MotionState::WalkMiddle);
    assert_eq!(world.players()[0].facing, 1);
    assert_eq!(world.players()[0].motion_frame, 0);
    assert!(world.players()[0].velocity.x > 0);
    assert!(world.players()[0].velocity.x < 64 * 6);
}

#[test]
fn wait_enters_walk_fast_after_dash_tap_window_expires() {
    let mut world = World::for_two_players();
    let hard_walk = [
        PlayerInput::neutral().with_left_stick(100, 0),
        PlayerInput::neutral(),
    ];
    let shield_hard_walk = [
        PlayerInput::neutral()
            .with_left_stick(100, 0)
            .with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &shield_hard_walk);
    step_world(&mut world, Frame(1), &hard_walk);
    for frame in 2..=16 {
        step_world(&mut world, Frame(frame), &hard_walk);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::Wait);

    step_world(&mut world, Frame(17), &hard_walk);

    assert_eq!(world.players()[0].motion_state, MotionState::WalkFast);
    assert_eq!(world.players()[0].facing, 1);
    assert_eq!(world.players()[0].motion_frame, 0);
    assert!(world.players()[0].velocity.x > 0);
    assert!(world.players()[0].velocity.x <= 100 * 6);
}

#[test]
fn rollback_owned_input_snapshots_deterministically_select_walk_bands() {
    for (stick_x, expected_state) in [
        (30, MotionState::WalkSlow),
        (64, MotionState::WalkMiddle),
        (100, MotionState::WalkFast),
    ] {
        let mut a = World::for_two_players();
        let mut b = World::for_two_players();
        let walk = [
            PlayerInput::neutral().with_left_stick(stick_x, 0),
            PlayerInput::neutral(),
        ];

        if expected_state == MotionState::WalkFast {
            let shield_walk = [
                PlayerInput::neutral()
                    .with_left_stick(stick_x, 0)
                    .with_left_trigger_analog(80),
                PlayerInput::neutral(),
            ];

            step_world(&mut a, Frame(0), &shield_walk);
            step_world(&mut b, Frame(0), &shield_walk);
            for frame in 1..=17 {
                step_world(&mut a, Frame(frame), &walk);
                step_world(&mut b, Frame(frame), &walk);
            }
        } else {
            step_world(&mut a, Frame(0), &walk);
            step_world(&mut b, Frame(0), &walk);
        }

        assert_eq!(a.players()[0].motion_state, expected_state);
        assert_eq!(b.players()[0].motion_state, expected_state);
        assert_eq!(a.players()[0].facing, b.players()[0].facing);
        assert_eq!(a.players()[0].velocity, b.players()[0].velocity);
        assert_eq!(a.checksum(), b.checksum());
    }
}

#[test]
fn walk_accelerates_toward_analog_target_instead_of_snapping() {
    let mut world = World::for_two_players();
    let walk_right = [
        PlayerInput::neutral().with_left_stick(40, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &walk_right);

    let first_walk_velocity = world.players()[0].velocity.x;
    assert_eq!(world.players()[0].motion_state, MotionState::WalkSlow);
    assert!(first_walk_velocity > 0);
    assert!(first_walk_velocity < 40 * 6);

    step_world(&mut world, Frame(1), &walk_right);

    assert_eq!(world.players()[0].motion_state, MotionState::WalkSlow);
    assert!(world.players()[0].velocity.x > first_walk_velocity);
    assert!(world.players()[0].velocity.x <= 40 * 6);
}

#[test]
fn walk_velocity_uses_player_profile_attributes() {
    let steady_profile = FighterProfile {
        walk_target_speed_per_stick: 4,
        walk_initial_accel_per_stick: 1,
        walk_accel_per_tick: 12,
        walk_friction_per_tick: 30,
        ..FighterProfile::falcon_like()
    };
    let quick_profile = FighterProfile {
        walk_target_speed_per_stick: 8,
        walk_initial_accel_per_stick: 2,
        walk_accel_per_tick: 24,
        walk_friction_per_tick: 60,
        ..FighterProfile::falcon_like()
    };
    let mut steady = World::for_two_players_with_profiles([steady_profile, steady_profile]);
    let mut quick = World::for_two_players_with_profiles([quick_profile, quick_profile]);
    let walk_right = [
        PlayerInput::neutral().with_left_stick(40, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut steady, Frame(0), &walk_right);
    step_world(&mut quick, Frame(0), &walk_right);

    assert_eq!(steady.players()[0].motion_state, MotionState::WalkSlow);
    assert_eq!(quick.players()[0].motion_state, MotionState::WalkSlow);
    assert_eq!(steady.players()[0].velocity.x, 52);
    assert_eq!(quick.players()[0].velocity.x, 104);
    assert_ne!(steady.checksum(), quick.checksum());

    step_world(&mut steady, Frame(1), &walk_right);
    step_world(&mut quick, Frame(1), &walk_right);

    assert_eq!(steady.players()[0].velocity.x, 104);
    assert_eq!(quick.players()[0].velocity.x, 208);
}

#[test]
fn walk_state_catch_has_priority_over_special_and_attack() {
    let mut world = World::for_two_players();
    let walk_right = [
        PlayerInput::neutral().with_left_stick(40, 0),
        PlayerInput::neutral(),
    ];
    let catch_special_attack = [
        PlayerInput::neutral()
            .with_grab(true)
            .with_special(true)
            .with_attack(true)
            .with_left_stick(80, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &walk_right);
    step_world(&mut world, Frame(1), &catch_special_attack);

    assert_eq!(world.players()[0].motion_state, MotionState::Catch);
}

#[test]
fn walk_state_accepts_specials_in_source_priority_order() {
    let mut side = World::for_two_players();
    let mut up = World::for_two_players();
    let mut neutral = World::for_two_players();
    let mut down = World::for_two_players();
    let walk_right = [
        PlayerInput::neutral().with_left_stick(40, 0),
        PlayerInput::neutral(),
    ];
    let side_special = [
        PlayerInput::neutral()
            .with_special(true)
            .with_left_stick(80, 0),
        PlayerInput::neutral(),
    ];
    let up_special = [
        PlayerInput::neutral()
            .with_special(true)
            .with_left_stick(0, 80),
        PlayerInput::neutral(),
    ];
    let neutral_special = [
        PlayerInput::neutral().with_special(true),
        PlayerInput::neutral(),
    ];
    let down_special = [
        PlayerInput::neutral()
            .with_special(true)
            .with_left_stick(0, -80),
        PlayerInput::neutral(),
    ];

    for world in [&mut side, &mut up, &mut neutral, &mut down] {
        step_world(world, Frame(0), &walk_right);
    }
    step_world(&mut side, Frame(1), &side_special);
    step_world(&mut up, Frame(1), &up_special);
    step_world(&mut neutral, Frame(1), &neutral_special);
    step_world(&mut down, Frame(1), &down_special);

    assert_eq!(side.players()[0].motion_state, MotionState::SpecialS);
    assert_eq!(up.players()[0].motion_state, MotionState::SpecialHi);
    assert_eq!(neutral.players()[0].motion_state, MotionState::SpecialN);
    assert_eq!(down.players()[0].motion_state, MotionState::SpecialLw);
}

#[test]
fn walk_state_accepts_attack_before_continuing_walk() {
    let mut world = World::for_two_players();
    let walk_right = [
        PlayerInput::neutral().with_left_stick(40, 0),
        PlayerInput::neutral(),
    ];
    let side_tilt = [
        PlayerInput::neutral()
            .with_attack(true)
            .with_left_stick(40, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &walk_right);
    step_world(&mut world, Frame(1), &side_tilt);

    assert_eq!(world.players()[0].motion_state, MotionState::AttackS3);
}

#[test]
fn walk_state_fresh_forward_dash_tap_enters_dash() {
    let mut world = World::for_two_players();
    let soft_walk = [
        PlayerInput::neutral().with_left_stick(40, 0),
        PlayerInput::neutral(),
    ];
    let full_dash = [
        PlayerInput::neutral().with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &soft_walk);
    step_world(&mut world, Frame(1), &full_dash);

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    assert_eq!(world.players()[0].facing, 1);
}

#[test]
fn walk_state_fresh_opposite_dash_tap_enters_turn() {
    let mut world = World::for_two_players();
    let soft_walk = [
        PlayerInput::neutral().with_left_stick(40, 0),
        PlayerInput::neutral(),
    ];
    let full_left = [
        PlayerInput::neutral().with_left_stick(-90, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &soft_walk);
    step_world(&mut world, Frame(1), &full_left);

    assert_eq!(world.players()[0].motion_state, MotionState::Turn);
    assert_eq!(world.players()[0].facing, 1);
}

#[test]
fn walk_state_soft_opposite_stick_exits_to_wait_without_flipping_facing() {
    let mut world = World::for_two_players();
    let walk_right = [
        PlayerInput::neutral().with_left_stick(40, 0),
        PlayerInput::neutral(),
    ];
    let soft_left = [
        PlayerInput::neutral().with_left_stick(-40, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &walk_right);
    step_world(&mut world, Frame(1), &soft_left);

    assert_eq!(world.players()[0].motion_state, MotionState::Wait);
    assert_eq!(world.players()[0].facing, 1);
    assert_eq!(world.players()[0].velocity.x, 0);
}

#[test]
fn walk_state_slow_rise_to_full_stick_keeps_walking() {
    let mut world = World::for_two_players();
    let soft_walk = [
        PlayerInput::neutral().with_left_stick(40, 0),
        PlayerInput::neutral(),
    ];
    let full_stick = [
        PlayerInput::neutral().with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &soft_walk);
    step_world(&mut world, Frame(1), &soft_walk);
    step_world(&mut world, Frame(2), &soft_walk);
    step_world(&mut world, Frame(3), &full_stick);

    assert_eq!(world.players()[0].motion_state, MotionState::WalkFast);
    assert_eq!(world.players()[0].facing, 1);
}

#[test]
fn walk_state_down_input_enters_squat_after_dash_check() {
    let mut world = World::for_two_players();
    let walk_right = [
        PlayerInput::neutral().with_left_stick(40, 0),
        PlayerInput::neutral(),
    ];
    let diagonal_down_walk = [
        PlayerInput::neutral().with_left_stick(40, -90),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &walk_right);
    step_world(&mut world, Frame(1), &diagonal_down_walk);

    assert_eq!(world.players()[0].motion_state, MotionState::Squat);
}

#[test]
fn soft_opposite_stick_from_wait_enters_turn_not_walk() {
    let mut world = World::for_two_players();
    let soft_left = [
        PlayerInput::neutral().with_left_stick(-40, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &soft_left);

    assert_eq!(world.players()[0].motion_state, MotionState::Turn);
    assert_eq!(world.players()[0].velocity.x, 0);
}

#[test]
fn standing_turn_delays_facing_flip_until_profile_flip_frame() {
    let mut world = World::for_two_players();
    let soft_left = [
        PlayerInput::neutral().with_left_stick(-40, 0),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    step_world(&mut world, Frame(0), &soft_left);

    assert_eq!(world.players()[0].motion_state, MotionState::Turn);
    assert_eq!(world.players()[0].facing, 1);

    for frame in 1..5 {
        step_world(&mut world, Frame(frame), &neutral);
        assert_eq!(world.players()[0].facing, 1);
    }

    step_world(&mut world, Frame(5), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::Turn);
    assert_eq!(world.players()[0].facing, -1);
}

#[test]
fn standing_turn_fresh_outward_dash_tap_dashes_on_turn_frame() {
    let mut world = World::for_two_players();
    let soft_left = [
        PlayerInput::neutral().with_left_stick(-40, 0),
        PlayerInput::neutral(),
    ];
    let full_left = [
        PlayerInput::neutral().with_left_stick(-90, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &soft_left);
    step_world(&mut world, Frame(1), &full_left);

    assert_eq!(world.players()[0].motion_state, MotionState::Turn);
    assert_eq!(world.players()[0].facing, 1);

    for frame in 2..5 {
        step_world(&mut world, Frame(frame), &full_left);
        assert_eq!(world.players()[0].motion_state, MotionState::Turn);
    }

    step_world(&mut world, Frame(5), &full_left);

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    assert_eq!(world.players()[0].facing, -1);
    assert_eq!(world.input_timers()[0].x_tap, 0xfe);
}

#[test]
fn turn_state_accepts_grounded_action_inputs_before_shield_jump_or_walk() {
    let mut world = World::for_two_players();
    let soft_left = [
        PlayerInput::neutral().with_left_stick(-40, 0),
        PlayerInput::neutral(),
    ];
    let turn_side_special = [
        PlayerInput::neutral()
            .with_left_stick(-40, 0)
            .with_special(true)
            .with_grab(true)
            .with_attack(true)
            .with_left_trigger_analog(80)
            .with_jump(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &soft_left);
    step_world(&mut world, Frame(1), &turn_side_special);

    assert_eq!(world.players()[0].motion_state, MotionState::SpecialS);
}

#[test]
fn turn_offense_before_facing_flip_uses_facing_after() {
    let mut world = World::for_two_players();
    let soft_left = [
        PlayerInput::neutral().with_left_stick(-40, 0),
        PlayerInput::neutral(),
    ];
    let jab = [
        PlayerInput::neutral().with_attack(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &soft_left);
    assert_eq!(world.players()[0].facing, 1);

    step_world(&mut world, Frame(1), &jab);

    assert_eq!(world.players()[0].motion_state, MotionState::Attack1);
    assert_eq!(world.players()[0].facing, -1);
}

#[test]
fn neutral_special_during_turn_does_not_enter_special_n() {
    let mut world = World::for_two_players();
    let soft_left = [
        PlayerInput::neutral().with_left_stick(-40, 0),
        PlayerInput::neutral(),
    ];
    let neutral_special = [
        PlayerInput::neutral().with_special(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &soft_left);
    step_world(&mut world, Frame(1), &neutral_special);

    assert_eq!(world.players()[0].motion_state, MotionState::Turn);
}

#[test]
fn neutral_special_latched_during_turn_replays_with_current_stick_on_turn_frame() {
    let mut world = World::for_two_players();
    let soft_left = [
        PlayerInput::neutral().with_left_stick(-40, 0),
        PlayerInput::neutral(),
    ];
    let neutral_special = [
        PlayerInput::neutral().with_special(true),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    step_world(&mut world, Frame(0), &soft_left);
    step_world(&mut world, Frame(1), &neutral_special);

    assert_eq!(world.players()[0].motion_state, MotionState::Turn);

    for frame in 2..5 {
        step_world(&mut world, Frame(frame), &neutral);
        assert_eq!(world.players()[0].motion_state, MotionState::Turn);
    }

    step_world(&mut world, Frame(5), &soft_left);

    assert_eq!(world.players()[0].motion_state, MotionState::SpecialS);
    assert_eq!(world.players()[0].facing, -1);
}

#[test]
fn turn_returns_to_wait_after_falcon_turn_frames() {
    let mut world = World::for_two_players();
    let soft_left = [
        PlayerInput::neutral().with_left_stick(-40, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &soft_left);

    assert_current_action_returns_to_wait_after_frames(&mut world, 0, MotionState::Turn, 11);
}

#[test]
fn walk_state_dash_strength_on_last_valid_tap_frame_enters_dash() {
    let mut world = World::for_two_players();
    let soft_walk = [
        PlayerInput::neutral().with_left_stick(40, 0),
        PlayerInput::neutral(),
    ];
    let mid_walk = [
        PlayerInput::neutral().with_left_stick(60, 0),
        PlayerInput::neutral(),
    ];
    let full_walk = [
        PlayerInput::neutral().with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &soft_walk);
    step_world(&mut world, Frame(1), &mid_walk);
    step_world(&mut world, Frame(2), &full_walk);

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    assert_eq!(world.players()[0].facing, 1);
    assert_eq!(world.input_timers()[0].x_tap, 0xfe);
}

#[test]
fn fresh_opposite_dash_tap_during_dash_enters_turn() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let dash_left = [
        PlayerInput::neutral().with_left_stick(-90, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    step_world(&mut world, Frame(1), &neutral);
    step_world(&mut world, Frame(2), &dash_left);

    assert_eq!(world.players()[0].motion_state, MotionState::Turn);
    assert_eq!(world.players()[0].facing, 1);
    assert_eq!(world.players()[0].velocity.x, 0);
    assert_eq!(world.input_timers()[0].x_tap, 0);

    step_world(&mut world, Frame(3), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::Turn);
    assert_eq!(world.players()[0].facing, -1);
}

#[test]
fn smash_turn_can_dash_out_on_the_turn_frame_if_stick_is_still_outward() {
    let mut world = World::for_two_players();
    let dash_left = [
        PlayerInput::neutral().with_left_stick(-90, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_left);

    assert_eq!(world.players()[0].motion_state, MotionState::Turn);
    assert_eq!(world.players()[0].facing, 1);

    step_world(&mut world, Frame(1), &dash_left);

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    assert_eq!(world.players()[0].facing, -1);
}

#[test]
fn smash_turn_dash_out_requires_dash_threshold_not_run_threshold() {
    let mut world = World::for_two_players();
    let dash_left = [
        PlayerInput::neutral().with_left_stick(-90, 0),
        PlayerInput::neutral(),
    ];
    let below_dash_left = [
        PlayerInput::neutral().with_left_stick(-70, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_left);
    step_world(&mut world, Frame(1), &below_dash_left);

    assert_eq!(world.players()[0].motion_state, MotionState::Turn);
    assert_eq!(world.players()[0].facing, -1);
}

#[test]
fn dash_uses_current_stick_for_moonwalk_like_acceleration_after_opposite_tap_ages_out() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];
    let soft_left = [
        PlayerInput::neutral().with_left_stick(-40, 0),
        PlayerInput::neutral(),
    ];
    let mid_left = [
        PlayerInput::neutral().with_left_stick(-60, 0),
        PlayerInput::neutral(),
    ];
    let near_left = [
        PlayerInput::neutral().with_left_stick(-70, 0),
        PlayerInput::neutral(),
    ];
    let full_left = [
        PlayerInput::neutral().with_left_stick(-90, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    step_world(&mut world, Frame(1), &soft_left);
    step_world(&mut world, Frame(2), &mid_left);
    step_world(&mut world, Frame(3), &near_left);
    step_world(&mut world, Frame(4), &full_left);

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    assert_eq!(world.players()[0].facing, 1);
    assert!(world.players()[0].velocity.x > 0);
    assert_eq!(world.input_timers()[0].x_tap, 3);

    for frame in 5..=13 {
        step_world(&mut world, Frame(frame), &full_left);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    assert!(world.players()[0].velocity.x < 0);
}

#[test]
fn dash_neutral_stick_applies_dash_friction_before_dash_ends() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    step_world(&mut world, Frame(0), &dash_right);
    let dash_velocity = world.players()[0].velocity.x;
    step_world(&mut world, Frame(1), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    assert!(world.players()[0].velocity.x > 0);
    assert!(world.players()[0].velocity.x < dash_velocity);
}

#[test]
fn dash_acceleration_uses_profile_source_accel_and_stick_scaled_target() {
    let profile = FighterProfile {
        initial_dash_speed_per_tick: 0,
        dash_run_accel_stick_per_tick: 20,
        dash_run_accel_base_per_tick: 50,
        run_speed_per_tick: 1_000,
        ground_max_horizontal_velocity_per_tick: 1_200,
        traction_per_tick: 30,
        ..FighterProfile::falcon_like()
    };
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let dash_right = [
        PlayerInput::neutral().with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    step_world(&mut world, Frame(1), &dash_right);

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    assert_eq!(world.players()[0].velocity.x, 68);
}

#[test]
fn dash_state_accepts_side_special_before_shield_or_jump() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];
    let side_special = [
        PlayerInput::neutral()
            .with_left_stick(80, 0)
            .with_special(true)
            .with_left_trigger_analog(80)
            .with_jump(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    step_world(&mut world, Frame(1), &side_special);

    assert_eq!(world.players()[0].motion_state, MotionState::SpecialS);
}

#[test]
fn dash_state_dash_grab_beats_dash_attack_and_guard() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];
    let shield_attack = [
        PlayerInput::neutral()
            .with_left_stick(80, 0)
            .with_left_trigger_analog(80)
            .with_attack(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    step_world(&mut world, Frame(1), &shield_attack);

    assert_eq!(world.players()[0].motion_state, MotionState::CatchDash);
}

#[test]
fn dash_state_early_defensive_window_shield_enters_escape_forward() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];
    let shield = [
        PlayerInput::neutral()
            .with_left_stick(80, 0)
            .with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    step_world(&mut world, Frame(1), &shield);

    assert_eq!(world.players()[0].motion_state, MotionState::EscapeF);
}

#[test]
fn dash_state_after_defensive_window_shield_enters_guard_on() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];
    let shield = [
        PlayerInput::neutral()
            .with_left_stick(80, 0)
            .with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    step_world(&mut world, Frame(1), &dash_right);
    step_world(&mut world, Frame(2), &shield);

    assert_eq!(world.players()[0].motion_state, MotionState::GuardOn);
}

#[test]
fn dash_state_late_window_opposite_dash_tap_beats_guard() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let shield_dash_left = [
        PlayerInput::neutral()
            .with_left_stick(-90, 0)
            .with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    step_world(&mut world, Frame(1), &neutral);
    step_world(&mut world, Frame(2), &shield_dash_left);

    assert_eq!(world.players()[0].motion_state, MotionState::Turn);
    assert_eq!(world.players()[0].facing, 1);
    assert_eq!(world.input_timers()[0].x_tap, 0);
}

#[test]
fn dash_state_z_grab_enters_catch_dash() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];
    let z_grab = [
        PlayerInput::neutral()
            .with_left_stick(80, 0)
            .with_grab(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    step_world(&mut world, Frame(1), &z_grab);

    assert_eq!(world.players()[0].motion_state, MotionState::CatchDash);
}

#[test]
fn dash_state_early_window_attack_pressed_without_shield_stays_dash() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];
    let dash_attack = [
        PlayerInput::neutral().with_attack(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    step_world(&mut world, Frame(1), &dash_attack);

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
}

#[test]
fn dash_state_early_window_forward_attack_enters_side_smash() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];
    let forward_attack = [
        PlayerInput::neutral()
            .with_left_stick(80, 0)
            .with_attack(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    step_world(&mut world, Frame(1), &forward_attack);

    assert_eq!(world.players()[0].motion_state, MotionState::AttackS4);
}

#[test]
fn dash_state_early_window_cstick_side_enters_side_smash() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];
    let cstick_side = [
        PlayerInput::neutral().with_c_stick(-90, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    step_world(&mut world, Frame(1), &cstick_side);

    assert_eq!(world.players()[0].motion_state, MotionState::AttackS4);
}

#[test]
fn dash_state_late_window_attack_pressed_without_shield_enters_attack_dash() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];
    let dash_attack = [
        PlayerInput::neutral().with_attack(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    step_world(&mut world, Frame(1), &dash_right);
    step_world(&mut world, Frame(2), &dash_attack);

    assert_eq!(world.players()[0].motion_state, MotionState::AttackDash);
}

#[test]
fn dash_holding_forward_exits_to_run_after_falcon_dash_frames() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    for frame in 1..=15 {
        step_world(&mut world, Frame(frame), &dash_right);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::Run);
    assert_eq!(world.players()[0].facing, 1);
    assert!(world.players()[0].velocity.x > 0);
}

#[test]
fn run_state_accepts_specials_in_source_priority_order() {
    let mut side = World::for_two_players();
    let mut up = World::for_two_players();
    let mut neutral = World::for_two_players();
    let mut down = World::for_two_players();
    let side_special = [
        PlayerInput::neutral()
            .with_left_stick(80, 0)
            .with_special(true)
            .with_left_trigger_analog(80)
            .with_jump(true),
        PlayerInput::neutral(),
    ];
    let up_special = [
        PlayerInput::neutral()
            .with_left_stick(0, 80)
            .with_special(true)
            .with_left_trigger_analog(80)
            .with_jump(true),
        PlayerInput::neutral(),
    ];
    let neutral_special = [
        PlayerInput::neutral()
            .with_special(true)
            .with_left_trigger_analog(80)
            .with_jump(true),
        PlayerInput::neutral(),
    ];
    let down_special = [
        PlayerInput::neutral()
            .with_left_stick(0, -80)
            .with_special(true)
            .with_left_trigger_analog(80)
            .with_jump(true),
        PlayerInput::neutral(),
    ];

    for world in [&mut side, &mut up, &mut neutral, &mut down] {
        advance_player_to_run(world);
    }
    step_world(&mut side, Frame(16), &side_special);
    step_world(&mut up, Frame(16), &up_special);
    step_world(&mut neutral, Frame(16), &neutral_special);
    step_world(&mut down, Frame(16), &down_special);

    assert_eq!(side.players()[0].motion_state, MotionState::SpecialS);
    assert_eq!(up.players()[0].motion_state, MotionState::SpecialHi);
    assert_eq!(neutral.players()[0].motion_state, MotionState::SpecialN);
    assert_eq!(down.players()[0].motion_state, MotionState::SpecialLw);
}

#[test]
fn run_state_dash_grab_beats_dash_attack_and_guard() {
    let mut world = World::for_two_players();
    let shield_attack = [
        PlayerInput::neutral()
            .with_left_stick(80, 0)
            .with_left_trigger_analog(80)
            .with_attack(true),
        PlayerInput::neutral(),
    ];

    advance_player_to_run(&mut world);
    step_world(&mut world, Frame(16), &shield_attack);

    assert_eq!(world.players()[0].motion_state, MotionState::CatchDash);
}

#[test]
fn run_state_z_grab_enters_catch_dash() {
    let mut world = World::for_two_players();
    let z_grab = [
        PlayerInput::neutral()
            .with_left_stick(80, 0)
            .with_grab(true),
        PlayerInput::neutral(),
    ];

    advance_player_to_run(&mut world);
    step_world(&mut world, Frame(16), &z_grab);

    assert_eq!(world.players()[0].motion_state, MotionState::CatchDash);
}

#[test]
fn run_state_attack_pressed_without_shield_enters_dash_attack() {
    let mut world = World::for_two_players();
    let dash_attack = [
        PlayerInput::neutral()
            .with_left_stick(80, 0)
            .with_attack(true),
        PlayerInput::neutral(),
    ];

    advance_player_to_run(&mut world);
    step_world(&mut world, Frame(16), &dash_attack);

    assert_eq!(world.players()[0].motion_state, MotionState::AttackDash);
}

#[test]
fn dash_releasing_to_neutral_exits_to_wait_after_falcon_dash_frames() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    step_world(&mut world, Frame(0), &dash_right);
    for frame in 1..=15 {
        step_world(&mut world, Frame(frame), &neutral);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::Wait);
    assert_eq!(world.players()[0].velocity.x, 0);
}

#[test]
fn holding_opposite_after_moonwalk_exits_dash_to_walk_not_run() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];
    let soft_left = [
        PlayerInput::neutral().with_left_stick(-40, 0),
        PlayerInput::neutral(),
    ];
    let mid_left = [
        PlayerInput::neutral().with_left_stick(-60, 0),
        PlayerInput::neutral(),
    ];
    let near_left = [
        PlayerInput::neutral().with_left_stick(-70, 0),
        PlayerInput::neutral(),
    ];
    let full_left = [
        PlayerInput::neutral().with_left_stick(-90, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    step_world(&mut world, Frame(1), &soft_left);
    step_world(&mut world, Frame(2), &mid_left);
    step_world(&mut world, Frame(3), &near_left);
    for frame in 4..=15 {
        step_world(&mut world, Frame(frame), &full_left);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::WalkFast);
    assert_eq!(world.players()[0].facing, -1);

    step_world(&mut world, Frame(16), &full_left);

    assert_eq!(world.players()[0].motion_state, MotionState::WalkFast);
    assert_eq!(world.players()[0].facing, -1);
    assert!(world.players()[0].velocity.x < 0);
}

#[test]
fn walk_after_moonwalk_carry_speed_decays_toward_walk_target_instead_of_snapping() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];
    let soft_left = [
        PlayerInput::neutral().with_left_stick(-40, 0),
        PlayerInput::neutral(),
    ];
    let mid_left = [
        PlayerInput::neutral().with_left_stick(-60, 0),
        PlayerInput::neutral(),
    ];
    let near_left = [
        PlayerInput::neutral().with_left_stick(-70, 0),
        PlayerInput::neutral(),
    ];
    let full_left = [
        PlayerInput::neutral().with_left_stick(-90, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    step_world(&mut world, Frame(1), &soft_left);
    step_world(&mut world, Frame(2), &mid_left);
    step_world(&mut world, Frame(3), &near_left);
    for frame in 4..=15 {
        step_world(&mut world, Frame(frame), &full_left);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::WalkFast);
    let carried_velocity = world.players()[0].velocity.x;

    step_world(&mut world, Frame(16), &full_left);

    assert_eq!(world.players()[0].motion_state, MotionState::WalkFast);
    let walk_target = -90 * FighterProfile::falcon_like().walk_target_speed_per_stick;
    assert!(
        (world.players()[0].velocity.x - walk_target).abs()
            < (carried_velocity - walk_target).abs()
    );
    assert_ne!(world.players()[0].velocity.x, walk_target);
}

#[test]
fn run_neutral_stick_enters_run_brake_without_zeroing_velocity() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    step_world(&mut world, Frame(0), &dash_right);
    for frame in 1..=15 {
        step_world(&mut world, Frame(frame), &dash_right);
    }
    let run_velocity = world.players()[0].velocity.x;

    step_world(&mut world, Frame(16), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::RunBrake);
    assert_eq!(world.players()[0].facing, 1);
    assert!(world.players()[0].velocity.x > 0);
    assert!(world.players()[0].velocity.x < run_velocity);
}

#[test]
fn run_opposite_stick_enters_turn_run_without_opposite_acceleration_same_tick() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];
    let dash_left = [
        PlayerInput::neutral().with_left_stick(-90, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    for frame in 1..=15 {
        step_world(&mut world, Frame(frame), &dash_right);
    }
    let run_velocity = world.players()[0].velocity.x;

    step_world(&mut world, Frame(16), &dash_left);

    assert_eq!(world.players()[0].motion_state, MotionState::TurnRun);
    assert_eq!(world.players()[0].facing, 1);
    assert!(world.players()[0].velocity.x > 0);
    assert!(world.players()[0].velocity.x < run_velocity);
}

#[test]
fn turn_run_keeps_old_facing_until_velocity_crosses_zero() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];
    let dash_left = [
        PlayerInput::neutral().with_left_stick(-90, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    for frame in 1..=15 {
        step_world(&mut world, Frame(frame), &dash_right);
    }
    step_world(&mut world, Frame(16), &dash_left);
    step_world(&mut world, Frame(17), &dash_left);

    assert_eq!(world.players()[0].motion_state, MotionState::TurnRun);
    assert_eq!(world.players()[0].facing, 1);
    assert!(world.players()[0].velocity.x > 0);
}

#[test]
fn turn_run_shield_input_does_not_cancel_before_source_jump_iasa() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];
    let dash_left = [
        PlayerInput::neutral().with_left_stick(-90, 0),
        PlayerInput::neutral(),
    ];
    let shield_left = [
        PlayerInput::neutral()
            .with_left_stick(-90, 0)
            .with_left_trigger_analog(70),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    for frame in 1..=15 {
        step_world(&mut world, Frame(frame), &dash_right);
    }
    step_world(&mut world, Frame(16), &dash_left);
    step_world(&mut world, Frame(17), &shield_left);

    assert_eq!(world.players()[0].motion_state, MotionState::TurnRun);
    assert_eq!(world.players()[0].facing, 1);
}

#[test]
fn run_entered_from_turn_run_keeps_source_no_interrupt_window() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];
    let dash_left = [
        PlayerInput::neutral().with_left_stick(-90, 0),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    step_world(&mut world, Frame(0), &dash_right);
    for frame in 1..=15 {
        step_world(&mut world, Frame(frame), &dash_right);
    }
    step_world(&mut world, Frame(16), &dash_left);

    let mut run_entry_frame = None;
    for frame in 17..=40 {
        step_world(&mut world, Frame(frame), &dash_left);
        if world.players()[0].motion_state == MotionState::Run {
            run_entry_frame = Some(frame);
            break;
        }
    }
    let run_entry_frame = run_entry_frame.expect("turn-run should enter run after crossing zero");

    assert_eq!(world.players()[0].motion_state, MotionState::Run);
    assert_eq!(world.players()[0].facing, -1);

    step_world(&mut world, Frame(run_entry_frame + 1), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::Run);
    assert!(world.players()[0].velocity.x <= 0);
}

#[test]
fn run_brake_forward_or_soft_stick_keeps_braking_until_source_exit() {
    let mut forward = World::for_two_players();
    let mut soft = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let soft_right = [
        PlayerInput::neutral().with_left_stick(40, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut forward, Frame(0), &dash_right);
    step_world(&mut soft, Frame(0), &dash_right);
    for frame in 1..=15 {
        step_world(&mut forward, Frame(frame), &dash_right);
        step_world(&mut soft, Frame(frame), &dash_right);
    }
    step_world(&mut forward, Frame(16), &neutral);
    step_world(&mut soft, Frame(16), &neutral);

    assert_eq!(forward.players()[0].motion_state, MotionState::RunBrake);
    assert_eq!(soft.players()[0].motion_state, MotionState::RunBrake);

    step_world(&mut forward, Frame(17), &dash_right);
    step_world(&mut soft, Frame(17), &soft_right);

    assert_eq!(forward.players()[0].motion_state, MotionState::RunBrake);
    assert_eq!(soft.players()[0].motion_state, MotionState::RunBrake);
}

#[test]
fn down_stick_from_wait_enters_squat() {
    let mut world = World::for_two_players();
    let down = [
        PlayerInput::neutral().with_left_stick(0, -80),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &down);

    assert_eq!(world.players()[0].motion_state, MotionState::Squat);
    assert_eq!(world.players()[0].velocity.x, 0);
}

#[test]
fn diagonal_down_walk_input_from_wait_enters_squat_not_walk() {
    let mut world = World::for_two_players();
    let down_forward = [
        PlayerInput::neutral().with_left_stick(40, -80),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &down_forward);

    assert_eq!(world.players()[0].motion_state, MotionState::Squat);
}

#[test]
fn dash_threshold_input_keeps_priority_over_crouch_from_wait() {
    let mut world = World::for_two_players();
    let dash_down_forward = [
        PlayerInput::neutral().with_left_stick(90, -80),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_down_forward);

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
}

#[test]
fn crouch_has_priority_over_turn_from_wait() {
    let mut world = World::for_two_players();
    let down_back = [
        PlayerInput::neutral().with_left_stick(-40, -80),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &down_back);

    assert_eq!(world.players()[0].motion_state, MotionState::Squat);
}

#[test]
fn squat_release_returns_to_wait_and_jump_or_shield_take_priority() {
    let mut release = World::for_two_players();
    let mut jump = World::for_two_players();
    let mut shield = World::for_two_players();
    let down = [
        PlayerInput::neutral().with_left_stick(0, -80),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let down_jump = [
        PlayerInput::neutral()
            .with_left_stick(0, -80)
            .with_jump(true),
        PlayerInput::neutral(),
    ];
    let down_shield = [
        PlayerInput::neutral()
            .with_left_stick(0, -80)
            .with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];

    step_world(&mut release, Frame(0), &down);
    step_world(&mut jump, Frame(0), &down);
    step_world(&mut shield, Frame(0), &down);

    step_world(&mut release, Frame(1), &neutral);
    step_world(&mut jump, Frame(1), &down_jump);
    step_world(&mut shield, Frame(1), &down_shield);

    assert_eq!(release.players()[0].motion_state, MotionState::Wait);
    assert_eq!(jump.players()[0].motion_state, MotionState::KneeBend);
    assert_eq!(shield.players()[0].motion_state, MotionState::GuardOn);
}

#[test]
fn special_from_squat_has_priority_over_grab_attack_shield_and_jump() {
    let mut world = World::for_two_players();
    let down = [
        PlayerInput::neutral().with_left_stick(0, -80),
        PlayerInput::neutral(),
    ];
    let down_special_grab_attack_shield_jump = [
        PlayerInput::neutral()
            .with_left_stick(0, -80)
            .with_special(true)
            .with_grab(true)
            .with_attack(true)
            .with_left_trigger_analog(80)
            .with_jump(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &down);
    step_world(&mut world, Frame(1), &down_special_grab_attack_shield_jump);

    assert_eq!(world.players()[0].motion_state, MotionState::SpecialLw);
}

#[test]
fn grab_from_squat_has_priority_over_attack_and_shield() {
    let mut world = World::for_two_players();
    let down = [
        PlayerInput::neutral().with_left_stick(0, -80),
        PlayerInput::neutral(),
    ];
    let down_grab_attack_shield = [
        PlayerInput::neutral()
            .with_left_stick(0, -80)
            .with_grab(true)
            .with_attack(true)
            .with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &down);
    step_world(&mut world, Frame(1), &down_grab_attack_shield);

    assert_eq!(world.players()[0].motion_state, MotionState::Catch);
}

#[test]
fn held_crouch_a_press_after_y_tap_window_enters_down_tilt() {
    let mut world = World::for_two_players();
    let down = [
        PlayerInput::neutral().with_left_stick(0, -80),
        PlayerInput::neutral(),
    ];
    let down_attack = [
        PlayerInput::neutral()
            .with_left_stick(0, -80)
            .with_attack(true),
        PlayerInput::neutral(),
    ];

    for frame in 0..=3 {
        step_world(&mut world, Frame(frame), &down);
    }
    step_world(&mut world, Frame(4), &down_attack);

    assert_eq!(world.players()[0].motion_state, MotionState::AttackLw3);
}

#[test]
fn fresh_cstick_corner_from_squat_enters_side_smash() {
    let mut world = World::for_two_players();
    let down = [
        PlayerInput::neutral().with_left_stick(0, -80),
        PlayerInput::neutral(),
    ];
    let down_cstick_corner = [
        PlayerInput::neutral()
            .with_left_stick(0, -80)
            .with_c_stick(90, 90),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &down);
    step_world(&mut world, Frame(1), &down_cstick_corner);

    assert_eq!(world.players()[0].motion_state, MotionState::AttackS4);
}

#[test]
fn special_from_wait_has_priority_over_grab_and_attack() {
    let mut world = World::for_two_players();
    let special_grab_attack = [
        PlayerInput::neutral()
            .with_special(true)
            .with_grab(true)
            .with_attack(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &special_grab_attack);

    assert_eq!(world.players()[0].motion_state, MotionState::SpecialN);
}

#[test]
fn b_special_direction_from_wait_matches_ground_priority() {
    let mut side = World::for_two_players();
    let mut up = World::for_two_players();
    let mut down = World::for_two_players();
    let mut down_boundary = World::for_two_players();
    let mut diagonal_side_first = World::for_two_players();
    let side_special = [
        PlayerInput::neutral()
            .with_special(true)
            .with_left_stick(-90, 0),
        PlayerInput::neutral(),
    ];
    let up_special = [
        PlayerInput::neutral()
            .with_special(true)
            .with_left_stick(0, 90),
        PlayerInput::neutral(),
    ];
    let down_special = [
        PlayerInput::neutral()
            .with_special(true)
            .with_left_stick(0, -90),
        PlayerInput::neutral(),
    ];
    let down_boundary_special = [
        PlayerInput::neutral()
            .with_special(true)
            .with_left_stick(0, -MeleeCommonData::provisional_mole().special_vertical_y),
        PlayerInput::neutral(),
    ];
    let diagonal_special = [
        PlayerInput::neutral()
            .with_special(true)
            .with_left_stick(-90, 90),
        PlayerInput::neutral(),
    ];

    step_world(&mut side, Frame(0), &side_special);
    step_world(&mut up, Frame(0), &up_special);
    step_world(&mut down, Frame(0), &down_special);
    step_world(&mut down_boundary, Frame(0), &down_boundary_special);
    step_world(&mut diagonal_side_first, Frame(0), &diagonal_special);

    assert_eq!(side.players()[0].motion_state, MotionState::SpecialS);
    assert_eq!(side.players()[0].facing, -1);
    assert_eq!(up.players()[0].motion_state, MotionState::SpecialHi);
    assert_eq!(down.players()[0].motion_state, MotionState::SpecialLw);
    assert_eq!(down_boundary.players()[0].motion_state, MotionState::Squat);
    assert_eq!(
        diagonal_side_first.players()[0].motion_state,
        MotionState::SpecialS
    );
    assert_eq!(diagonal_side_first.players()[0].facing, -1);
}

#[test]
fn grab_from_wait_has_priority_over_smash_and_shield() {
    let mut world = World::for_two_players();
    let grab_smash_shield = [
        PlayerInput::neutral()
            .with_grab(true)
            .with_attack(true)
            .with_left_stick(90, 0)
            .with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &grab_smash_shield);

    assert_eq!(world.players()[0].motion_state, MotionState::Catch);
}

#[test]
fn a_press_attack_priority_resolves_jab_tilt_and_smash_before_movement() {
    let mut jab = World::for_two_players();
    let mut ftilt = World::for_two_players();
    let mut fsmash = World::for_two_players();
    let mut shallow_diagonal = World::for_two_players();
    let mut up_diagonal = World::for_two_players();
    let mut down_diagonal = World::for_two_players();
    let neutral_attack = [
        PlayerInput::neutral()
            .with_attack(true)
            .with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];
    let tilt_attack = [
        PlayerInput::neutral()
            .with_attack(true)
            .with_left_stick(40, 0),
        PlayerInput::neutral(),
    ];
    let smash_attack = [
        PlayerInput::neutral()
            .with_attack(true)
            .with_jump(true)
            .with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];
    let shallow_diagonal_attack = [
        PlayerInput::neutral()
            .with_attack(true)
            .with_left_stick(50, 30),
        PlayerInput::neutral(),
    ];
    let up_diagonal_attack = [
        PlayerInput::neutral()
            .with_attack(true)
            .with_left_stick(30, 50),
        PlayerInput::neutral(),
    ];
    let down_diagonal_attack = [
        PlayerInput::neutral()
            .with_attack(true)
            .with_left_stick(30, -50),
        PlayerInput::neutral(),
    ];

    step_world(&mut jab, Frame(0), &neutral_attack);
    step_world(&mut ftilt, Frame(0), &tilt_attack);
    step_world(&mut fsmash, Frame(0), &smash_attack);
    step_world(&mut shallow_diagonal, Frame(0), &shallow_diagonal_attack);
    step_world(&mut up_diagonal, Frame(0), &up_diagonal_attack);
    step_world(&mut down_diagonal, Frame(0), &down_diagonal_attack);

    assert_eq!(jab.players()[0].motion_state, MotionState::Attack1);
    assert_eq!(ftilt.players()[0].motion_state, MotionState::AttackS3);
    assert_eq!(fsmash.players()[0].motion_state, MotionState::AttackS4);
    assert_eq!(
        shallow_diagonal.players()[0].motion_state,
        MotionState::AttackS3
    );
    assert_eq!(
        up_diagonal.players()[0].motion_state,
        MotionState::AttackHi3
    );
    assert_eq!(
        down_diagonal.players()[0].motion_state,
        MotionState::AttackLw3
    );
}

#[test]
fn vertical_a_press_attacks_resolve_before_jump_and_crouch() {
    let mut up_smash = World::for_two_players();
    let mut down_smash = World::for_two_players();
    let up_attack = [
        PlayerInput::neutral()
            .with_attack(true)
            .with_left_stick(0, 90),
        PlayerInput::neutral(),
    ];
    let down_attack = [
        PlayerInput::neutral()
            .with_attack(true)
            .with_left_stick(0, -90),
        PlayerInput::neutral(),
    ];

    step_world(&mut up_smash, Frame(0), &up_attack);
    step_world(&mut down_smash, Frame(0), &down_attack);

    assert_eq!(up_smash.players()[0].motion_state, MotionState::AttackHi4);
    assert_eq!(down_smash.players()[0].motion_state, MotionState::AttackLw4);
}

#[test]
fn cstick_smash_from_wait_uses_cstick_priority_without_a_press() {
    let mut side = World::for_two_players();
    let mut up = World::for_two_players();
    let cstick_side_corner = [
        PlayerInput::neutral().with_c_stick(90, 90),
        PlayerInput::neutral(),
    ];
    let cstick_up = [
        PlayerInput::neutral().with_c_stick(0, 90),
        PlayerInput::neutral(),
    ];

    step_world(&mut side, Frame(0), &cstick_side_corner);
    step_world(&mut up, Frame(0), &cstick_up);

    assert_eq!(side.players()[0].motion_state, MotionState::AttackS4);
    assert_eq!(up.players()[0].motion_state, MotionState::AttackHi4);
}

#[test]
fn grounded_a_side_smash_beats_simultaneous_cstick_up_smash() {
    let mut wait = World::for_two_players();
    let mut walk = World::for_two_players();
    let attack = [
        PlayerInput::neutral()
            .with_attack(true)
            .with_left_stick(90, 0)
            .with_c_stick(0, 90),
        PlayerInput::neutral(),
    ];
    let walk_start = [
        PlayerInput::neutral().with_left_stick(40, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut wait, Frame(0), &attack);

    step_world(&mut walk, Frame(0), &walk_start);
    step_world(&mut walk, Frame(1), &attack);

    assert_eq!(wait.players()[0].motion_state, MotionState::AttackS4);
    assert_eq!(walk.players()[0].motion_state, MotionState::AttackS4);
}

fn assert_action_returns_to_wait_after_frames(
    action_input: PlayerInput,
    action_state: MotionState,
    total_frames: u32,
) {
    let mut world = World::for_two_players();
    let action = [action_input, PlayerInput::neutral()];

    step_world(&mut world, Frame(0), &action);

    assert_current_action_returns_to_wait_after_frames(&mut world, 0, action_state, total_frames);
}

fn advance_to_air(world: &mut World) {
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];

    for frame in 0..=3 {
        step_world(world, Frame(frame), &jump);
    }

    assert!(!world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::JumpF);
}

fn assert_current_action_returns_to_wait_after_frames(
    world: &mut World,
    entry_frame: u32,
    action_state: MotionState,
    total_frames: u32,
) {
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    assert_eq!(world.players()[0].motion_state, action_state);

    for frame in (entry_frame + 1)..(entry_frame + total_frames) {
        step_world(world, Frame(frame), &neutral);
        assert_eq!(world.players()[0].motion_state, action_state);
    }

    step_world(world, Frame(entry_frame + total_frames), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::Wait);
}

fn snapshot_with_timers(
    lstick: (i8, i8),
    cstick: (i8, i8),
    x_tap_timer: u8,
    y_tap_timer: u8,
) -> MeleeInputSnapshot {
    MeleeInputSnapshot {
        lstick,
        prev_lstick: lstick,
        cstick,
        prev_cstick: cstick,
        left_trigger: 0,
        right_trigger: 0,
        prev_left_trigger: 0,
        prev_right_trigger: 0,
        held: GameCubeButtonState::empty(),
        pressed: GameCubeButtonState::empty(),
        released: GameCubeButtonState::empty(),
        shield_held: false,
        shield_pressed: false,
        shield_released: false,
        left_trigger_analog_held: false,
        right_trigger_analog_held: false,
        left_trigger_analog_pressed: false,
        right_trigger_analog_pressed: false,
        x_tap_timer,
        y_tap_timer,
        trigger_timer: 254,
        ucf_x_tilt_intent: false,
        ucf_shield_drop_tilt_intent: false,
    }
}

#[test]
fn grounded_action_states_end_after_falcon_total_frames() {
    assert_action_returns_to_wait_after_frames(
        PlayerInput::neutral().with_attack(true),
        MotionState::Attack1,
        21,
    );
    assert_action_returns_to_wait_after_frames(
        PlayerInput::neutral()
            .with_attack(true)
            .with_left_stick(90, 0),
        MotionState::AttackS4,
        64,
    );
    assert_action_returns_to_wait_after_frames(
        PlayerInput::neutral()
            .with_attack(true)
            .with_left_stick(40, 0),
        MotionState::AttackS3,
        29,
    );
    assert_action_returns_to_wait_after_frames(
        PlayerInput::neutral()
            .with_attack(true)
            .with_left_stick(0, 90),
        MotionState::AttackHi4,
        54,
    );
    assert_action_returns_to_wait_after_frames(
        PlayerInput::neutral()
            .with_attack(true)
            .with_left_stick(0, -90),
        MotionState::AttackLw4,
        49,
    );
    let up_tilt_input = PlayerInput::neutral()
        .with_attack(true)
        .with_left_stick(0, 40);
    assert_action_returns_to_wait_after_frames(up_tilt_input, MotionState::AttackHi3, 39);
    let mut down_tilt = World::for_two_players();
    let down = [
        PlayerInput::neutral().with_left_stick(0, -80),
        PlayerInput::neutral(),
    ];
    let down_attack = [
        PlayerInput::neutral()
            .with_left_stick(0, -80)
            .with_attack(true),
        PlayerInput::neutral(),
    ];
    for frame in 0..=3 {
        step_world(&mut down_tilt, Frame(frame), &down);
    }
    step_world(&mut down_tilt, Frame(4), &down_attack);
    assert_current_action_returns_to_wait_after_frames(
        &mut down_tilt,
        4,
        MotionState::AttackLw3,
        35,
    );
    assert_action_returns_to_wait_after_frames(
        PlayerInput::neutral().with_grab(true),
        MotionState::Catch,
        30,
    );
    assert_action_returns_to_wait_after_frames(
        PlayerInput::neutral().with_special(true),
        MotionState::SpecialN,
        99,
    );
}

#[test]
fn attack1_does_not_interrupt_before_falcon_iasa_frame() {
    let mut world = World::for_two_players();
    let jab = [
        PlayerInput::neutral().with_attack(true),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &jab);
    for frame in 1..15 {
        step_world(&mut world, Frame(frame), &neutral);
    }
    step_world(&mut world, Frame(15), &jump);

    assert_eq!(world.players()[0].motion_state, MotionState::Attack1);
}

#[test]
fn attack1_accepts_jump_on_falcon_iasa_frame() {
    let mut world = World::for_two_players();
    let jab = [
        PlayerInput::neutral().with_attack(true),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &jab);
    for frame in 1..=15 {
        step_world(&mut world, Frame(frame), &neutral);
    }
    step_world(&mut world, Frame(16), &jump);

    assert_eq!(world.players()[0].motion_state, MotionState::KneeBend);
}

#[test]
fn fsmash_does_not_interrupt_before_falcon_iasa_but_can_after() {
    let mut early = World::for_two_players();
    let mut iasa = World::for_two_players();
    let fsmash = [
        PlayerInput::neutral()
            .with_attack(true)
            .with_left_stick(90, 0),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut early, Frame(0), &fsmash);
    for frame in 1..59 {
        step_world(&mut early, Frame(frame), &neutral);
    }
    step_world(&mut early, Frame(59), &jump);

    assert_eq!(early.players()[0].motion_state, MotionState::AttackS4);

    step_world(&mut iasa, Frame(0), &fsmash);
    for frame in 1..=59 {
        step_world(&mut iasa, Frame(frame), &neutral);
    }
    step_world(&mut iasa, Frame(60), &jump);

    assert_eq!(iasa.players()[0].motion_state, MotionState::KneeBend);
}

#[test]
fn holding_down_during_upward_jump_does_not_fast_fall() {
    let mut neutral_jump = World::for_two_players();
    let mut down_jump = World::for_two_players();
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let jump_with_down = [
        PlayerInput::neutral()
            .with_jump(true)
            .with_left_stick(0, -127),
        PlayerInput::neutral(),
    ];

    step_world(&mut neutral_jump, Frame(0), &jump);
    step_world(&mut down_jump, Frame(0), &jump_with_down);

    assert_eq!(
        down_jump.players()[0].velocity.y,
        neutral_jump.players()[0].velocity.y
    );
}

#[test]
fn down_held_before_falling_does_not_buffer_fast_fall() {
    let mut neutral = World::for_two_players();
    let mut held_down = World::for_two_players();
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let neutral_input = [PlayerInput::neutral(), PlayerInput::neutral()];
    let down_input = [
        PlayerInput::neutral().with_left_stick(0, -127),
        PlayerInput::neutral(),
    ];

    step_world(&mut neutral, Frame(0), &jump);
    step_world(&mut held_down, Frame(0), &jump);

    for frame in 1..30 {
        step_world(&mut neutral, Frame(frame), &neutral_input);
        step_world(&mut held_down, Frame(frame), &down_input);
    }

    assert_eq!(
        held_down.players()[0].velocity.y,
        neutral.players()[0].velocity.y
    );
    assert_eq!(
        held_down.players()[0].position.y,
        neutral.players()[0].position.y
    );
}

#[test]
fn fresh_down_tap_while_falling_fast_falls_once() {
    let mut neutral = World::for_two_players();
    let mut fast_fall = World::for_two_players();
    let profile = FighterProfile::falcon_like();
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let neutral_input = [PlayerInput::neutral(), PlayerInput::neutral()];
    let down_input = [
        PlayerInput::neutral().with_left_stick(0, -127),
        PlayerInput::neutral(),
    ];

    step_world(&mut neutral, Frame(0), &jump);
    step_world(&mut fast_fall, Frame(0), &jump);

    let mut frame = 1;
    while neutral.players()[0].velocity.y >= 0 {
        step_world(&mut neutral, Frame(frame), &neutral_input);
        step_world(&mut fast_fall, Frame(frame), &neutral_input);
        frame += 1;
        assert!(
            frame < 120,
            "fighter should begin falling before frame 120; state={:?}, velocity={:?}, position={:?}",
            neutral.players()[0].motion_state,
            neutral.players()[0].velocity,
            neutral.players()[0].position
        );
    }

    let position_before_tap = fast_fall.players()[0].position.y;

    step_world(&mut neutral, Frame(frame), &neutral_input);
    step_world(&mut fast_fall, Frame(frame), &down_input);

    assert!(fast_fall.players()[0].fast_falling);
    assert_eq!(
        fast_fall.players()[0].velocity.y,
        -profile.fast_fall_speed_per_tick
    );
    assert_eq!(
        fast_fall.players()[0].position.y - position_before_tap,
        -profile.fast_fall_speed_per_tick
    );

    frame += 1;
    let position_before_held_down = fast_fall.players()[0].position.y;
    step_world(&mut fast_fall, Frame(frame), &down_input);

    assert_eq!(
        fast_fall.players()[0].velocity.y,
        -profile.fast_fall_speed_per_tick
    );
    assert_eq!(
        fast_fall.players()[0].position.y - position_before_held_down,
        -profile.fast_fall_speed_per_tick
    );
}
