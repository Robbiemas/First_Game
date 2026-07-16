use mole_core::collision::{
    source_damage_result_for_victim, source_grab_confirms, Capsule3, SourceCollisionCapsule,
    SourceCollisionFrame, SourceDamageResult, SourceDamageResultInput, SourceDamageStage,
    SourceHitboxAttributes, SourceHitboxFlags, SourceHitboxLifecycleId, SourceInstalledThrowHitbox,
    SourceThrowHitboxAttributes, Vec3, SOURCE_SHIELD_HURTBOX_ID,
};
use mole_core::{
    fighter_stick_axis_to_f32, has_source_ecb_pose_data_for_motion_state,
    input_common_data_field_sources, landing_contact_for_bottom_with_floor_skip,
    melee_action_state_id_for_motion_state, melee_units, melee_units_f32, milli_to_source_units,
    motion_state_for_runtime_variant, runtime_motion_state_for_source_key,
    source_ledge_grab_contact, source_root_motion_delta, source_root_motion_delta_for_action_key,
    source_root_motion_frame_count, source_root_motion_frame_count_for_action_key,
    source_root_motion_position, source_root_motion_position_for_action_key,
    source_special_action_binding_for_motion_state, source_special_action_binding_for_runtime_id,
    source_units_to_milli, step_world, step_world_with_source_runtime_data, CommonDataExtractError,
    CommonDataProvenance, EcbDiamond, EngineFeatureToggles, FighterActionFrames, FighterCameraBox,
    FighterEntryPlatformProfile, FighterProfile, FighterProfileExtractError, Frame,
    GameCubeButtonState, GameCubePadStatus, MatchPhase, MeleeActionStateId, MeleeCommonData,
    MeleeInputConfig, MeleeInputProcessor, MeleeInputSnapshot, MeleeInputThresholds,
    MeleeInputTimers, MeleeJumpInput, MotionState, PlayerInput, PlayerState, SourceActionKey,
    SourceActionPoseMetadata, SourceActionScriptEvent, SourceActionScriptEvents, SourceCapturePose,
    SourceDownBoundPose, SourcePosePoint, SourceVec2, SourceVec3, StageCollisionLineKind,
    StageLedge, StageLedgeSide, StageProfile, StageSpawnPoint, StageSurface, StageSurfaceKind,
    Vec2, WalkSpeedBucket, World, CANONICAL_SOURCE_ONLY_ACTION_BINDINGS, DEFAULT_STOCK_COUNT,
    PLAYER_STATE_IN_GAME, PLAYER_STATE_NONE, SOURCE_COLLISION_STATE_HURT_INTANGIBLE, TICK_RATE_HZ,
};
use std::{fs, path::Path};

fn installed_throw_hitbox(raw: SourceThrowHitboxAttributes) -> SourceInstalledThrowHitbox {
    let damage = raw.damage as f32;
    SourceInstalledThrowHitbox {
        hitbox: raw,
        damage,
        unk_count: damage as u16,
    }
}

fn translated_ecb(ecb: EcbDiamond, delta: Vec2) -> EcbDiamond {
    let translate = |point: Vec2| Vec2 {
        x: point.x + delta.x,
        y: point.y + delta.y,
    };
    EcbDiamond {
        top: translate(ecb.top),
        right: translate(ecb.right),
        bottom: translate(ecb.bottom),
        left: translate(ecb.left),
    }
}

#[test]
fn falcon_special_action_bindings_cover_decomp_runtime_table() {
    let expected = [
        (MotionState::SpecialN, 347, 301, "SpecialN", 100),
        (MotionState::SpecialAirN, 348, 302, "SpecialAirN", 100),
        (MotionState::SpecialSStart, 349, 303, "SpecialSStart", 80),
        (MotionState::SpecialS, 350, 304, "SpecialS", 25),
        (
            MotionState::SpecialAirSStart,
            351,
            305,
            "SpecialAirSStart",
            80,
        ),
        (MotionState::SpecialAirS, 352, 306, "SpecialAirS", 45),
        (MotionState::SpecialHi, 353, 307, "SpecialHi", 65),
        (MotionState::SpecialAirHi, 354, 308, "SpecialAirHi", 65),
        (MotionState::SpecialLw, 357, 311, "SpecialLw", 40),
        (MotionState::SpecialAirLw, 359, 313, "SpecialAirLw", 30),
    ];

    for (motion_state, runtime_id, source_id, key, total_frames) in expected {
        let binding = source_special_action_binding_for_motion_state(motion_state)
            .expect("mapped Falcon special motion state should have native source binding");

        assert_eq!(binding.action_state_id, MeleeActionStateId::new(runtime_id));
        assert_eq!(binding.source_action_table_id, source_id);
        assert_eq!(binding.source_action_key, SourceActionKey::new(key));
        assert_eq!(binding.total_frames, total_frames);
        assert_eq!(
            source_special_action_binding_for_runtime_id(MeleeActionStateId::new(runtime_id)),
            Some(binding)
        );
    }
}

#[test]
fn falcon_special_action_bindings_preserve_decomp_only_special_states() {
    let expected = [
        (355, 309, "SpecialHiCatch", 16),
        (356, 310, "SpecialHiThrow", 60),
        (358, 312, "SpecialLwEnd", 30),
        (360, 314, "SpecialAirLwEnd", 45),
        (361, 316, "SpecialAirLwEndAir", 29),
        (362, 315, "SpecialLwEndAir", 30),
        (363, 317, "SpecialHiThrow", 60),
    ];

    for (runtime_id, source_id, key, total_frames) in expected {
        let binding =
            source_special_action_binding_for_runtime_id(MeleeActionStateId::new(runtime_id))
                .expect("decomp-only Falcon special state should be retained in native table");

        assert_eq!(binding.action_state_id, MeleeActionStateId::new(runtime_id));
        assert_eq!(binding.source_action_table_id, source_id);
        assert_eq!(binding.source_action_key, SourceActionKey::new(key));
        assert_eq!(binding.total_frames, total_frames);
        assert!(binding.motion_state.is_none());
    }
}

#[test]
fn source_bound_pass_advances_source_animation_like_ftco_pass_anim() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.grounded = false;
    player.motion_state = MotionState::Pass;
    player.motion_state_alias = None;
    player.melee_action_state_id = Some(MeleeActionStateId::new(244));
    player.source_action_key = Some(SourceActionKey::new("Pass"));
    player.source_action_total_frames = 15;
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world_with_source_runtime_data(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
        |_| None,
        |action_state_id| match action_state_id.get() {
            244 => Some(15),
            _ => None,
        },
    );

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::Pass);
    assert_eq!(player.motion_frame, 1);
    assert_eq!(player.motion_anim_frame_milli, 1_000);
    assert_eq!(player.source_motion_anim_frame.to_bits(), 1.0_f32.to_bits());
    assert_eq!(world.snapshot().players[0].source_pose_frame, 1);
}

#[test]
fn escape_air_advances_source_animation_while_waiting_for_ftanim_completion() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.grounded = false;
    player.motion_state = MotionState::EscapeAir;
    player.motion_state_alias = None;
    player.melee_action_state_id = Some(MeleeActionStateId::new(236));
    player.source_action_key = Some(SourceActionKey::new("EscapeAir"));
    player.source_action_total_frames = 40;
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.escape_air_iasa_timer = 10;
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world_with_source_runtime_data(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
        |_| None,
        |action_state_id| match action_state_id.get() {
            236 => Some(40),
            _ => None,
        },
    );

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::EscapeAir);
    assert_eq!(player.motion_frame, 1);
    assert_eq!(player.motion_anim_frame_milli, 1_000);
    assert_eq!(player.source_motion_anim_frame.to_bits(), 1.0_f32.to_bits());
    assert_eq!(world.snapshot().players[0].source_pose_frame, 1);
}

#[test]
fn source_bound_render_snapshot_uses_source_animation_frame_for_pose_selection() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.motion_state = MotionState::Pass;
    player.melee_action_state_id = Some(MeleeActionStateId::new(244));
    player.source_action_key = Some(SourceActionKey::new("Pass"));
    player.source_action_total_frames = 15;
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(4.0);
    assert!(world.set_player_state_for_diagnostic(0, player));

    let snapshot = world.snapshot().players[0];
    assert_eq!(snapshot.source_pose_motion_state, MotionState::Pass);
    assert_eq!(
        snapshot.source_pose_action_key,
        Some(SourceActionKey::new("Pass"))
    );
    assert_eq!(snapshot.source_pose_frame, 4);
    assert_eq!(snapshot.animation_frame, 4);
    assert_eq!(snapshot.animation_frame_milli, 4_000);
}

#[test]
fn source_bound_wait_snapshot_uses_cur_anim_frame_not_local_motion_frame() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.motion_state = MotionState::Wait;
    player.melee_action_state_id = Some(MeleeActionStateId::new(14));
    player.source_action_key = Some(SourceActionKey::new("Wait1"));
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(4.0);
    assert!(world.set_player_state_for_diagnostic(0, player));

    let snapshot = world.snapshot().players[0];
    assert_eq!(snapshot.source_pose_motion_state, MotionState::Wait);
    assert_eq!(
        snapshot.source_pose_action_key,
        Some(SourceActionKey::new("Wait1"))
    );
    assert_eq!(
        snapshot.animation_frame, 4,
        "source-backed render must sample decomp cur_anim_frame, not local motion_frame"
    );
    assert_eq!(snapshot.animation_frame_milli, 4_000);
}

fn read_be_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn read_be_f32(bytes: &[u8], offset: usize) -> f32 {
    f32::from_bits(read_be_u32(bytes, offset))
}

fn squared_magnitude(velocity: Vec2) -> i32 {
    velocity.x * velocity.x + velocity.y * velocity.y
}

fn close_to(left: i32, right: i32, tolerance: i32) -> bool {
    (left - right).abs() <= tolerance
}

fn source_floor_line_for_surface_at_x(
    stage: StageProfile,
    surface: StageSurface,
    source_x: f32,
) -> u16 {
    let melee_stage = stage
        .melee_stage_profile()
        .expect("diagnostic stage should have source collision data");
    melee_stage
        .collision
        .lines
        .iter()
        .enumerate()
        .find_map(|(line_id, line)| {
            if !matches!(
                line.kind,
                StageCollisionLineKind::Floor | StageCollisionLineKind::SoftFloor
            ) {
                return None;
            }
            let scaled = melee_stage.collision.scaled_line(line_id)?;
            let same_y = source_units_to_milli(scaled.y0) == surface.y
                && source_units_to_milli(scaled.y1) == surface.y;
            let contains_x = source_x >= scaled.x0.min(scaled.x1) - 0.1
                && source_x <= scaled.x0.max(scaled.x1) + 0.1;
            (same_y && contains_x).then_some(line_id as u16)
        })
        .expect("source floor line should cover diagnostic surface position")
}

#[test]
fn falcon_camera_box_matches_ftdata_x3c_source_values() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let raw_path = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("crate should live under workspace/crates/mole_core")
        .join("resources/melee/raw/PlCa.dat");
    let raw =
        fs::read(raw_path).expect("Captain Falcon DAT should be available for extraction parity");
    let ft_data_offset = 39_428usize;
    let camera_box_offset = read_be_u32(&raw, 0x20 + ft_data_offset + 0x3c) as usize;
    let extracted = FighterCameraBox {
        x0: mole_core::SourceVec3 {
            x: read_be_f32(&raw, 0x20 + camera_box_offset),
            y: read_be_f32(&raw, 0x20 + camera_box_offset + 0x04),
            z: read_be_f32(&raw, 0x20 + camera_box_offset + 0x08),
        },
        xc: mole_core::SourceVec3 {
            x: read_be_f32(&raw, 0x20 + camera_box_offset + 0x0c),
            y: read_be_f32(&raw, 0x20 + camera_box_offset + 0x10),
            z: read_be_f32(&raw, 0x20 + camera_box_offset + 0x14),
        },
    };

    assert_eq!(camera_box_offset, 30_920);
    assert_eq!(FighterProfile::FALCON_LIKE.camera_box, extracted);
}

fn enable_custom_shield_turn(world: &mut World) {
    world.set_engine_features(
        EngineFeatureToggles::parity().with_shield_turnaround_during_guard(true),
    );
}

#[test]
fn fighter_stick_axis_recovers_hsd_80_step_normalization_from_slippi_export() {
    assert_eq!(fighter_stick_axis_to_f32(127).to_bits(), 1.0_f32.to_bits());
    assert_eq!(
        fighter_stick_axis_to_f32(-127).to_bits(),
        (-1.0_f32).to_bits()
    );
    assert_eq!(fighter_stick_axis_to_f32(0).to_bits(), 0.0_f32.to_bits());
    assert_eq!(
        fighter_stick_axis_to_f32(125).to_bits(),
        (79.0_f32 / 80.0_f32).to_bits(),
        "Slippi frame 355 exports main_stick 0.9875 as i8 125, and fighter math must recover that HSD-normalized value"
    );
    assert_eq!(
        fighter_stick_axis_to_f32(-94).to_bits(),
        (-59.0_f32 / 80.0_f32).to_bits(),
        "Slippi frame 355 exports main_stick -0.7375 as i8 -94, and replay physics should not drift from re-dividing by 127"
    );
    assert_eq!(
        fighter_stick_axis_to_f32(95).to_bits(),
        (60.0_f32 / 80.0_f32).to_bits(),
        "The 0.75 floor-edge threshold should remain exactly representable after export"
    );
}

#[test]
fn catch_ground_physics_preserves_gr_vel_and_applies_source_x64_friction() {
    let stage = StageProfile::battlefield();
    let mut world = World::for_two_players_on_stage(stage);
    let common = world.common_data();
    let start_x = 10.0_f32;
    let initial_gr_vel = 2.0_f32;

    let mut player = world.players()[0];
    player.set_motion_state_alias(MotionState::Catch);
    player.motion_frame = 0;
    player.grounded = true;
    player.source_position = SourceVec2 { x: start_x, y: 0.0 };
    player.position = player.source_position.to_milli();
    player.ground_velocity_x = initial_gr_vel;
    player.source_self_velocity_x = initial_gr_vel;
    player.source_self_velocity_y = 0.0;
    player.velocity.x = source_units_to_milli(initial_gr_vel);
    player.velocity.y = 0;
    player.set_source_floor_for_diagnostic(
        Some(0),
        Some(source_floor_line_for_surface_at_x(
            stage,
            stage.main_floor,
            start_x,
        )),
    );
    assert!(world.set_player_state_for_diagnostic(0, player));

    let mut other = world.players()[1];
    other.source_position = SourceVec2 { x: 60.0, y: 0.0 };
    other.position = other.source_position.to_milli();
    assert!(world.set_player_state_for_diagnostic(1, other));

    step_world(&mut world, Frame(0), &[PlayerInput::neutral(); 2]);

    let player = world.players()[0];
    let expected = initial_gr_vel
        - common.catch_ground_friction_multiplier * FighterProfile::FALCON_LIKE.ground_friction;
    assert_eq!(player.motion_state, MotionState::Catch);
    assert!((player.ground_velocity_x - expected).abs() < 0.00001);
    assert!((player.source_self_velocity_x - expected).abs() < 0.00001);
    assert!((player.source_position.x - (start_x + expected)).abs() < 0.00001);
    assert_eq!(player.velocity.x, source_units_to_milli(expected));
}

#[test]
fn catch_entry_preserves_ground_velocity_like_ftco_800d8c54() {
    let stage = StageProfile::battlefield();
    let mut world = World::for_two_players_on_stage(stage);
    let common = world.common_data();
    let start_x = 10.0_f32;
    let initial_gr_vel = 2.0_f32;

    let mut player = world.players()[0];
    player.set_motion_state_alias(MotionState::Wait);
    player.motion_frame = 0;
    player.grounded = true;
    player.source_position = SourceVec2 { x: start_x, y: 0.0 };
    player.position = player.source_position.to_milli();
    player.ground_velocity_x = initial_gr_vel;
    player.source_self_velocity_x = initial_gr_vel;
    player.velocity.x = source_units_to_milli(initial_gr_vel);
    player.set_source_floor_for_diagnostic(
        Some(0),
        Some(source_floor_line_for_surface_at_x(
            stage,
            stage.main_floor,
            start_x,
        )),
    );
    assert!(world.set_player_state_for_diagnostic(0, player));

    let mut other = world.players()[1];
    other.source_position = SourceVec2 { x: 60.0, y: 0.0 };
    other.position = other.source_position.to_milli();
    assert!(world.set_player_state_for_diagnostic(1, other));

    step_world(
        &mut world,
        Frame(0),
        &[
            PlayerInput::neutral().with_grab(true),
            PlayerInput::neutral(),
        ],
    );

    let player = world.players()[0];
    let expected = initial_gr_vel
        - common.catch_ground_friction_multiplier * FighterProfile::FALCON_LIKE.ground_friction;
    assert_eq!(player.motion_state, MotionState::Catch);
    assert!((player.ground_velocity_x - expected).abs() < 0.00001);
    assert!((player.source_position.x - (start_x + expected)).abs() < 0.00001);
}

#[test]
fn standing_grab_enters_catch_preserving_decomp_facing_dir_for_both_facings() {
    for facing in [1, -1] {
        let mut world = World::for_two_players();
        let mut player = world.players()[0];
        player.set_motion_state_alias(MotionState::Wait);
        player.facing = facing;
        player.source_motion_entry_facing = facing;
        player.grounded = true;
        assert!(world.set_player_state_for_diagnostic(0, player));

        step_world(
            &mut world,
            Frame(0),
            &[
                PlayerInput::neutral().with_grab(true),
                PlayerInput::neutral(),
            ],
        );

        let player = world.players()[0];
        assert_eq!(
            player.motion_state,
            MotionState::Catch,
            "ftCo_Catch_CheckInput routes standing grab through ftCo_MS_Catch"
        );
        assert_eq!(
            player.facing, facing,
            "ftCo_800D8C54 must not flip facing_dir on standing grab entry"
        );
        assert_eq!(player.source_motion_entry_facing, facing);
    }
}

#[test]
fn run_grab_enters_catch_dash_preserving_decomp_facing_dir_for_both_facings() {
    for facing in [1, -1] {
        let mut world = World::for_two_players();
        let mut player = world.players()[0];
        player.set_motion_state_alias(MotionState::Run);
        player.facing = facing;
        player.source_motion_entry_facing = facing;
        player.grounded = true;
        player.motion_frame = 4;
        assert!(world.set_player_state_for_diagnostic(0, player));

        step_world(
            &mut world,
            Frame(0),
            &[
                PlayerInput::neutral()
                    .with_left_stick(80 * facing, 0)
                    .with_grab(true),
                PlayerInput::neutral(),
            ],
        );

        let player = world.players()[0];
        assert_eq!(
            player.motion_state,
            MotionState::CatchDash,
            "ftCo_800D8A38 routes run grab through ftCo_MS_CatchDash"
        );
        assert_eq!(
            player.facing, facing,
            "ftCo_800D8C54 must not flip facing_dir on dash grab entry"
        );
        assert_eq!(player.source_motion_entry_facing, facing);
    }
}

fn dash_stick_x() -> i8 {
    MeleeCommonData::provisional_mole().dash_x
}

fn crouch_stick_y() -> i8 {
    -MeleeCommonData::provisional_mole().crouch_y
}

fn shield_analog() -> u8 {
    MeleeCommonData::provisional_mole().z_shield_analog
}

fn spot_dodge_stick_y() -> i8 {
    MeleeCommonData::provisional_mole()
        .escape_y
        .saturating_sub(1)
}

fn run_stick_x() -> i8 {
    MeleeCommonData::provisional_mole().run_x
}

fn turn_run_stick_x() -> i8 {
    -MeleeCommonData::provisional_mole().turn_run_x
}

fn falcon_escape_air_action_frames() -> u32 {
    50
}

fn falcon_escape_air_skip_decay_frame() -> u32 {
    30
}

fn falcon_pass_action_frames() -> u8 {
    30
}

fn source_dash_iasa_decay(velocity_x: i32, common: MeleeCommonData) -> i32 {
    let velocity = milli_to_source_units(velocity_x);
    source_units_to_milli(velocity - velocity * common.dash_velocity_decay)
}

fn source_general_grounded_friction_velocity(
    velocity_x: i32,
    profile: FighterProfile,
    common: MeleeCommonData,
) -> i32 {
    let velocity = milli_to_source_units(velocity_x);
    let mut friction = profile.ground_friction;
    if velocity.abs() > profile.walk_max_velocity {
        friction *= common.high_speed_ground_friction_multiplier;
    }

    source_units_to_milli(source_apply_ground_friction_to_zero_f32(velocity, friction))
}

fn source_root_motion_delta_milli_for_profile(
    motion_state: MotionState,
    source_frame: u8,
    profile: FighterProfile,
) -> i32 {
    let delta = source_root_motion_delta(motion_state, source_frame)
        .expect("source root motion delta should be extracted");
    source_units_to_milli(delta.z * profile.model_scaling)
}

fn source_action_root_motion_delta_milli_for_profile(
    source_action_key: SourceActionKey,
    source_frame: u8,
    profile: FighterProfile,
) -> i32 {
    let delta = source_root_motion_delta_for_action_key(source_action_key, source_frame)
        .expect("source action root motion delta should be extracted");
    source_units_to_milli(delta.z * profile.model_scaling)
}

fn ledge_facing_for_test(side: StageLedgeSide) -> i8 {
    match side {
        StageLedgeSide::Left => 1,
        StageLedgeSide::Right => -1,
    }
}

fn source_cliff_position_from_transn(ledge_x: i32, ledge_y: i32, facing: i8, frame: u8) -> Vec2 {
    let position = source_root_motion_position(MotionState::CliffCatch, frame)
        .expect("CliffCatch TransN position should be extracted");
    Vec2 {
        x: ledge_x + source_units_to_milli(position.z * f32::from(facing)),
        y: ledge_y + source_units_to_milli(position.y),
    }
}

fn source_scaled_cliff_position_from_transn(
    ledge_x: i32,
    ledge_y: i32,
    facing: i8,
    frame: u8,
    profile: FighterProfile,
) -> Vec2 {
    let position = source_root_motion_position(MotionState::CliffCatch, frame)
        .expect("CliffCatch TransN position should be extracted");
    Vec2 {
        x: ledge_x + source_units_to_milli(position.z * profile.model_scaling * f32::from(facing)),
        y: ledge_y + source_units_to_milli(position.y * profile.model_scaling),
    }
}

fn source_cliff_wait_position_from_transn(ledge_x: i32, ledge_y: i32, facing: i8) -> Vec2 {
    let position = source_root_motion_position(MotionState::CliffWait, 1)
        .expect("CliffWait TransN position should be extracted");
    Vec2 {
        x: ledge_x + source_units_to_milli(position.z * f32::from(facing)),
        y: ledge_y + source_units_to_milli(position.y),
    }
}

fn source_scaled_cliff_wait_position_from_transn(
    ledge_x: i32,
    ledge_y: i32,
    facing: i8,
    profile: FighterProfile,
) -> Vec2 {
    let position = source_root_motion_position(MotionState::CliffWait, 1)
        .expect("CliffWait TransN position should be extracted");
    Vec2 {
        x: ledge_x + source_units_to_milli(position.z * profile.model_scaling * f32::from(facing)),
        y: ledge_y + source_units_to_milli(position.y * profile.model_scaling),
    }
}

fn source_scaled_cliff_wait_source_position_from_transn(
    ledge_x: i32,
    ledge_y: i32,
    facing: i8,
    profile: FighterProfile,
) -> mole_core::SourceVec2 {
    let position = source_root_motion_position(MotionState::CliffWait, 1)
        .expect("CliffWait TransN position should be extracted");
    mole_core::SourceVec2 {
        x: milli_to_source_units(ledge_x) + position.z * profile.model_scaling * f32::from(facing),
        y: milli_to_source_units(ledge_y) + position.y * profile.model_scaling,
    }
}

fn source_general_grounded_friction_velocity_f32(
    velocity_x: f32,
    profile: FighterProfile,
    common: MeleeCommonData,
) -> f32 {
    let mut friction = profile.ground_friction;
    if velocity_x.abs() > profile.walk_max_velocity {
        friction *= common.high_speed_ground_friction_multiplier;
    }

    source_apply_ground_friction_to_zero_f32(velocity_x, friction)
}

fn source_general_grounded_friction_velocity_after_ticks_f32(
    mut velocity_x: f32,
    ticks: u8,
    profile: FighterProfile,
    common: MeleeCommonData,
) -> f32 {
    for _ in 0..ticks {
        velocity_x = source_general_grounded_friction_velocity_f32(velocity_x, profile, common);
    }
    velocity_x
}

fn source_dash_to_turn_frame_velocity(
    velocity_x: i32,
    profile: FighterProfile,
    common: MeleeCommonData,
) -> i32 {
    source_general_grounded_friction_velocity(
        source_dash_iasa_decay(velocity_x, common),
        profile,
        common,
    )
}

fn source_stick_scaled_velocity(stick_x: i32, full_stick_velocity: f32) -> i32 {
    source_units_to_milli(source_stick_scaled_velocity_f32(
        stick_x,
        full_stick_velocity,
    ))
}

fn source_stick_scaled_velocity_f32(stick_x: i32, full_stick_velocity: f32) -> f32 {
    fighter_stick_axis_to_f32(stick_x.clamp(-127, 127) as i8) * full_stick_velocity
}

fn source_run_ground_friction(profile: FighterProfile, common: MeleeCommonData) -> i32 {
    source_units_to_milli(source_run_ground_friction_f32(profile, common))
}

fn source_run_ground_friction_f32(profile: FighterProfile, common: MeleeCommonData) -> f32 {
    profile.ground_friction * common.run_ground_friction_multiplier
}

fn source_ground_accel_toward_target(
    current_velocity: i32,
    mut accel: i32,
    target_velocity: i32,
    friction_per_tick: i32,
    max_velocity: i32,
) -> i32 {
    let friction_per_tick = friction_per_tick.abs();
    if target_velocity == 0 {
        return if current_velocity > friction_per_tick {
            current_velocity - friction_per_tick
        } else if current_velocity < -friction_per_tick {
            current_velocity + friction_per_tick
        } else {
            0
        };
    }

    if current_velocity * accel >= 0 {
        if accel > 0 && current_velocity + accel > target_velocity {
            accel = -friction_per_tick;
            if current_velocity + accel < target_velocity {
                accel = target_velocity - current_velocity;
            }
        } else if accel < 0 && current_velocity + accel < target_velocity {
            accel = friction_per_tick;
            if current_velocity + accel > target_velocity {
                accel = target_velocity - current_velocity;
            }
        }
    }

    (current_velocity + accel).clamp(-max_velocity, max_velocity)
}

fn source_ground_accel_toward_target_f32(
    current_velocity: f32,
    mut accel: f32,
    target_velocity: f32,
    friction_per_tick: f32,
    max_velocity: f32,
) -> f32 {
    let friction_per_tick = friction_per_tick.abs();
    if target_velocity == 0.0 {
        return source_apply_ground_friction_to_zero_f32(current_velocity, friction_per_tick);
    }

    if current_velocity * accel >= 0.0 {
        if accel > 0.0 && current_velocity + accel > target_velocity {
            accel = -friction_per_tick;
            if current_velocity + accel < target_velocity {
                accel = target_velocity - current_velocity;
            }
        } else if accel < 0.0 && current_velocity + accel < target_velocity {
            accel = friction_per_tick;
            if current_velocity + accel > target_velocity {
                accel = target_velocity - current_velocity;
            }
        }
    }

    (current_velocity + accel).clamp(-max_velocity, max_velocity)
}

fn source_apply_ground_friction_to_zero_f32(current_velocity: f32, friction: f32) -> f32 {
    let friction = friction.abs();
    if current_velocity > friction {
        current_velocity - friction
    } else if current_velocity < -friction {
        current_velocity + friction
    } else {
        0.0
    }
}

fn source_remaining_velocity_taper_f32(
    current_velocity: f32,
    accel: f32,
    target_velocity: f32,
    taper: f32,
) -> f32 {
    if target_velocity == 0.0 || taper == 1.0 {
        return accel;
    }

    let ratio = current_velocity / target_velocity;
    if !(ratio > 0.0 && ratio < 1.0) {
        return accel;
    }

    let remaining = (target_velocity - current_velocity).abs();
    let denominator = target_velocity.abs();
    let scaled_abs = accel.abs() * remaining * taper / denominator;
    scaled_abs * accel.signum()
}

fn source_dash_run_accel_and_target_f32(profile: FighterProfile, stick_x: i32) -> (f32, f32) {
    let stick = fighter_stick_axis_to_f32(stick_x.clamp(-127, 127) as i8);
    let base_accel = if stick > 0.0 {
        profile.dash_run_acceleration_b
    } else {
        -profile.dash_run_acceleration_b
    };
    (
        stick * profile.dash_run_acceleration_a + base_accel,
        stick * profile.dash_run_terminal_velocity,
    )
}

fn source_dash_phys_velocity_f32(
    velocity_x: f32,
    stick_x: i32,
    profile: FighterProfile,
    common: MeleeCommonData,
) -> f32 {
    let (accel, target_velocity) = source_dash_run_accel_and_target_f32(profile, stick_x);
    source_ground_accel_toward_target_f32(
        velocity_x,
        accel,
        target_velocity,
        source_run_ground_friction_f32(profile, common),
        profile.ground_max_horizontal_velocity,
    )
}

fn source_dash_phys_render_velocity_from_source(
    velocity_x: f32,
    stick_x: i32,
    profile: FighterProfile,
    common: MeleeCommonData,
) -> i32 {
    source_units_to_milli(source_dash_phys_velocity_f32(
        velocity_x, stick_x, profile, common,
    ))
}

fn source_run_phys_render_velocity_from_source(
    current_velocity: f32,
    stick_x: i32,
    profile: FighterProfile,
    common: MeleeCommonData,
) -> i32 {
    let (accel, target_velocity) = source_dash_run_accel_and_target_f32(profile, stick_x);
    let accel = source_remaining_velocity_taper_f32(
        current_velocity,
        accel,
        target_velocity,
        common.run_accel_taper,
    );
    source_units_to_milli(source_ground_accel_toward_target_f32(
        current_velocity,
        accel,
        target_velocity,
        source_run_ground_friction_f32(profile, common),
        profile.ground_max_horizontal_velocity,
    ))
}

fn source_walk_phys_render_velocity_from_source(
    current_velocity: f32,
    stick_x: i32,
    profile: FighterProfile,
    common: MeleeCommonData,
) -> i32 {
    let target_velocity = source_stick_scaled_velocity_f32(stick_x, profile.walk_max_velocity);
    let mut accel = source_stick_scaled_velocity_f32(stick_x, profile.walk_initial_velocity);
    if stick_x > 0 {
        accel += profile.walk_accel;
    } else if stick_x < 0 {
        accel -= profile.walk_accel;
    }
    let accel = source_remaining_velocity_taper_f32(
        current_velocity,
        accel,
        target_velocity,
        common.walk_accel_taper,
    );
    source_units_to_milli(source_ground_accel_toward_target_f32(
        current_velocity,
        accel,
        target_velocity,
        profile.ground_friction,
        profile.ground_max_horizontal_velocity,
    ))
}

fn source_turn_run_phys_velocity(
    velocity_x: i32,
    stick_x: i32,
    accel_mul: i8,
    profile: FighterProfile,
    common: MeleeCommonData,
) -> i32 {
    let (mut accel, target_velocity) = source_dash_run_accel_and_target(profile, stick_x);

    if accel != 0 && accel_mul as i32 * accel < 0 {
        if accel > 0 {
            if velocity_x + accel > target_velocity {
                accel -= source_run_ground_friction(profile, common);
                if velocity_x + accel < target_velocity {
                    accel = target_velocity - velocity_x;
                }
            }
        } else if velocity_x + accel < target_velocity {
            accel += source_run_ground_friction(profile, common);
            if velocity_x + accel > target_velocity {
                accel = target_velocity - velocity_x;
            }
        }
        velocity_x + accel
    } else {
        let friction = source_run_ground_friction(profile, common);
        if velocity_x > friction {
            velocity_x - friction
        } else if velocity_x < -friction {
            velocity_x + friction
        } else {
            0
        }
    }
}

fn source_dash_phys_velocity(
    velocity_x: i32,
    stick_x: i32,
    profile: FighterProfile,
    common: MeleeCommonData,
) -> i32 {
    let (accel, target_velocity) = source_dash_run_accel_and_target(profile, stick_x);
    source_ground_accel_toward_target(
        velocity_x,
        accel,
        target_velocity,
        source_run_ground_friction(profile, common),
        source_units_to_milli(profile.ground_max_horizontal_velocity),
    )
}

fn source_air_drift_velocity(velocity_x: i32, stick_x: i32, profile: FighterProfile) -> i32 {
    let current_velocity = milli_to_source_units(velocity_x);
    source_units_to_milli(source_air_drift_velocity_f32(
        current_velocity,
        stick_x,
        profile,
    ))
}

fn source_air_drift_velocity_f32(
    current_velocity: f32,
    stick_x: i32,
    profile: FighterProfile,
) -> f32 {
    let target_velocity = source_stick_scaled_velocity_f32(stick_x, profile.air_drift_max);
    if target_velocity == 0.0 {
        return source_apply_air_friction_to_zero_f32(current_velocity, profile.aerial_friction);
    }

    let stick_accel = source_stick_scaled_velocity_f32(stick_x, profile.air_drift_stick_multiplier);
    let base_accel = stick_x.signum() as f32 * profile.aerial_drift_base;
    current_velocity
        + source_air_accel_for_velocity_f32(
            current_velocity,
            stick_accel + base_accel,
            target_velocity,
            profile.aerial_friction,
            profile.air_max_horizontal_velocity,
        )
}

fn source_apply_air_friction_to_zero_f32(current_velocity: f32, friction: f32) -> f32 {
    let friction = friction.abs();
    if current_velocity > friction {
        current_velocity - friction
    } else if current_velocity < -friction {
        current_velocity + friction
    } else {
        0.0
    }
}

fn source_air_accel_for_velocity_f32(
    velocity_x: f32,
    mut accel: f32,
    target_velocity: f32,
    friction: f32,
    max_horizontal_velocity: f32,
) -> f32 {
    if velocity_x * accel >= 0.0 {
        if accel > 0.0 && velocity_x + accel > target_velocity {
            accel = -friction;
            if velocity_x + accel < target_velocity {
                accel = target_velocity - velocity_x;
            }
            if velocity_x + accel > max_horizontal_velocity {
                accel = max_horizontal_velocity - velocity_x;
            }
        } else if accel < 0.0 && velocity_x + accel < target_velocity {
            accel = friction;
            if velocity_x + accel > target_velocity {
                accel = target_velocity - velocity_x;
            }
            if velocity_x + accel < -max_horizontal_velocity {
                accel = -max_horizontal_velocity - velocity_x;
            }
        }
    }
    accel
}

fn source_dash_run_accel_and_target(profile: FighterProfile, stick_x: i32) -> (i32, i32) {
    let stick = fighter_stick_axis_to_f32(stick_x.clamp(-127, 127) as i8);
    let base_accel = if stick > 0.0 {
        profile.dash_run_acceleration_b
    } else {
        -profile.dash_run_acceleration_b
    };
    (
        ((stick * profile.dash_run_acceleration_a + base_accel) * 1000.0).round() as i32,
        (stick * profile.dash_run_terminal_velocity * 1000.0).round() as i32,
    )
}

fn advance_player_to_run(world: &mut World) {
    let dash_right = [
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];

    step_world(world, Frame(0), &dash_right);
    for frame in 1..=16 {
        step_world(world, Frame(frame), &dash_right);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::Run);
}

fn quick_turn_run_profile() -> FighterProfile {
    FighterProfile {
        action_frames: FighterActionFrames {
            dash_total_frames: 3,
            dash_cmd_var0_set_frame: 1,
            turn_run_total_frames: 5,
            turn_run_cmd_var1_frame: 2,
            ..FighterActionFrames::falcon_like()
        },
        dash_frames: 1,
        dash_initial_velocity: 0.3,
        dash_run_acceleration_a: 1.2,
        dash_run_acceleration_b: 0.0,
        dash_run_terminal_velocity: 2.3,
        ground_friction: 0.02,
        ground_max_horizontal_velocity: 3.0,
        max_run_brake_frames: Some(30),
        ..FighterProfile::falcon_like()
    }
}

fn advance_quick_profile_to_run(world: &mut World, stick_x: i8) {
    let run_input = [
        PlayerInput::neutral().with_left_stick(stick_x, 0),
        PlayerInput::neutral(),
    ];

    step_world(world, Frame(0), &run_input);
    step_world(world, Frame(1), &run_input);

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
    let stage = StageProfile::battlefield();

    assert_eq!(stage.name, "battlefield");
    assert_eq!(stage.main_floor.left_x, melee_units_f32(-68.4));
    assert_eq!(stage.main_floor.right_x, melee_units_f32(68.4));
    assert_eq!(stage.soft_platforms.len(), 3);
    assert!(stage
        .soft_platforms
        .iter()
        .all(|surface| surface.kind == StageSurfaceKind::Soft));
}

#[test]
fn extracted_battlefield_stage_preserves_decomp_map_collision_data() {
    let stage = mole_core::MeleeStageProfile::battlefield();

    assert_eq!(stage.id, "battlefield");
    assert_eq!(stage.name, "Battlefield");
    assert_eq!(stage.source.dat_file, "GrNBa.dat");
    assert_eq!(
        stage.collision.scale.to_bits(),
        0.800000011920929_f32.to_bits()
    );
    assert_eq!(stage.collision.vertices.len(), 26);
    assert_eq!(stage.collision.lines.len(), 23);
    assert_eq!(stage.collision.joints.len(), 1);
    assert_eq!(stage.collision.joints[0].floor_start, 0);
    assert_eq!(stage.collision.joints[0].floor_count, 6);
    assert_eq!(stage.collision.joints[0].ceiling_start, 6);
    assert_eq!(stage.collision.joints[0].ceiling_count, 5);
    assert_eq!(stage.collision.joints[0].right_wall_start, 11);
    assert_eq!(stage.collision.joints[0].right_wall_count, 6);
    assert_eq!(stage.collision.joints[0].left_wall_start, 17);
    assert_eq!(stage.collision.joints[0].left_wall_count, 6);

    let main_floor_left = stage.collision.lines[0];
    assert_eq!(main_floor_left.kind, StageCollisionLineKind::Floor);
    assert_eq!(main_floor_left.v0_idx, 6);
    assert_eq!(main_floor_left.v1_idx, 25);
    assert_eq!(main_floor_left.hi_flags, 0x1);
    assert_eq!(main_floor_left.lo_flags, 0x200);
    let main_floor_left_scaled = stage.collision.scaled_line(0).unwrap();
    assert_eq!(
        main_floor_left_scaled.x0.to_bits(),
        (-85.5_f32 * stage.collision.scale).to_bits()
    );
    assert_eq!(
        main_floor_left_scaled.x1.to_bits(),
        (-75.0_f32 * stage.collision.scale).to_bits()
    );
    assert_eq!(main_floor_left_scaled.y0.to_bits(), 0.0_f32.to_bits());
    assert_eq!(main_floor_left_scaled.x0_milli, -68400);
    assert_eq!(main_floor_left_scaled.x1_milli, -60000);

    let left_platform = stage.collision.lines[2];
    assert_eq!(left_platform.kind, StageCollisionLineKind::SoftFloor);
    assert!(left_platform.passable);
    let left_platform_scaled = stage.collision.scaled_line(2).unwrap();
    assert_eq!(
        left_platform_scaled.x0.to_bits(),
        (-72.0_f32 * stage.collision.scale).to_bits()
    );
    assert_eq!(
        left_platform_scaled.x1.to_bits(),
        (-25.0_f32 * stage.collision.scale).to_bits()
    );
    assert_eq!(
        left_platform_scaled.y0.to_bits(),
        (34.0_f32 * stage.collision.scale).to_bits()
    );
    assert_eq!(left_platform_scaled.x0_milli, -57600);
    assert_eq!(left_platform_scaled.x1_milli, -20000);
    assert_eq!(left_platform_scaled.y0_milli, 27200);

    let center_ceiling = stage.collision.lines[8];
    assert_eq!(center_ceiling.kind, StageCollisionLineKind::Ceiling);
    assert_eq!(center_ceiling.v0_idx, 13);
    assert_eq!(center_ceiling.v1_idx, 11);
    assert_eq!(center_ceiling.hi_flags, 0x2);

    let right_ledge_wall = stage.collision.lines[16];
    assert_eq!(right_ledge_wall.kind, StageCollisionLineKind::RightWall);
    let right_ledge_wall_scaled = stage.collision.scaled_line(16).unwrap();
    assert_eq!(right_ledge_wall_scaled.x0_milli, 68400);
    assert_eq!(right_ledge_wall_scaled.x1_milli, 64980);
    assert_eq!(right_ledge_wall_scaled.y0_milli, 0);
    assert_eq!(right_ledge_wall_scaled.y1_milli, -6000);

    let left_ledge_wall = stage.collision.lines[17];
    assert_eq!(left_ledge_wall.kind, StageCollisionLineKind::LeftWall);
    let left_ledge_wall_scaled = stage.collision.scaled_line(17).unwrap();
    assert_eq!(
        left_ledge_wall_scaled.x0.to_bits(),
        (-81.2249984741211_f32 * stage.collision.scale).to_bits()
    );
    assert_eq!(left_ledge_wall_scaled.x0_milli, -64980);
    assert_eq!(left_ledge_wall_scaled.x1_milli, -68400);
    assert_eq!(left_ledge_wall_scaled.y0_milli, -6000);
    assert_eq!(left_ledge_wall_scaled.y1_milli, 0);
}

#[test]
fn extracted_battlefield_stage_promotes_map_head_runtime_objects() {
    let stage = mole_core::MeleeStageProfile::battlefield();
    let map_head = stage.map_head;

    assert_eq!(map_head.stage_dat_offset, 0x27C);
    assert_eq!(map_head.entries_offset, 0x78);
    assert_eq!(map_head.entry_count, 7);
    assert_eq!(map_head.entries.len(), 7);
    assert_eq!(map_head.joints.len(), 73);
    assert_eq!(map_head.internal_count, 4);

    let first_entry = map_head.entries[0];
    assert_eq!(first_entry.joint_root_offset, 0x34270);
    assert_eq!(first_entry.joint_root_index, Some(0));
    assert_eq!(first_entry.camera_desc_offset, 0x341C4);

    let root = map_head.joints[first_entry.joint_root_index.unwrap() as usize];
    assert_eq!(root.node_offset, first_entry.joint_root_offset);
    assert_eq!(root.child_index, Some(1));
    assert_eq!(root.next_index, None);
    assert_eq!(root.scale.x.to_bits(), 1.0_f32.to_bits());
    assert_eq!(root.scale.y.to_bits(), 1.0_f32.to_bits());
    assert_eq!(root.scale.z.to_bits(), 1.0_f32.to_bits());

    let first_child = map_head.joints[root.child_index.unwrap() as usize];
    assert_eq!(first_child.node_offset, 0x342B0);
    assert_eq!(first_child.next_index, Some(2));
    assert_eq!(first_child.position.x.to_bits(), 0.0_f32.to_bits());

    let visible_stage_node = map_head.joints[first_child.next_index.unwrap() as usize];
    assert_eq!(visible_stage_node.node_offset, 0x342F0);
    assert_eq!(
        visible_stage_node.position.x.to_bits(),
        (-200.0_f32).to_bits()
    );
    assert_eq!(visible_stage_node.position.y.to_bits(), 170.0_f32.to_bits());
}

#[test]
fn battlefield_compat_stage_profile_is_projected_from_extracted_collision() {
    let extracted = mole_core::MeleeStageProfile::battlefield();
    let projected = extracted.compat_stage_profile();

    assert_eq!(projected, StageProfile::battlefield());
    assert_eq!(projected.main_floor, extracted.main_floor);
    assert_eq!(projected.soft_platforms, extracted.soft_platforms);
    assert_eq!(projected.blast_zones, extracted.blast_zones);
    assert_eq!(projected.spawn_points, extracted.spawn_points);
}

#[test]
fn battlefield_melee_stage_profile_promotes_decomp_stage_runtime_metadata() {
    let stage = mole_core::MeleeStageProfile::battlefield();

    assert_eq!(stage.ledges.len(), 2);
    assert_eq!(stage.ledges[0].index, stage.ledges[0].line_index);
    assert_eq!(stage.ledges[0].line_index, 0);
    assert_eq!(stage.ledges[0].side, StageLedgeSide::Left);
    assert_eq!(stage.ledges[0].x_milli, -68400);
    assert_eq!(stage.ledges[1].index, stage.ledges[1].line_index);
    assert_eq!(stage.ledges[1].line_index, 5);
    assert_eq!(stage.ledges[1].side, StageLedgeSide::Right);
    assert_eq!(stage.ledges[1].x_milli, 68400);
    assert_eq!(stage.dynamic_collision.line_count, 0);
    assert_eq!(stage.callbacks.stage_data_symbol, "grNBa_803E7E38");
    assert_eq!(stage.callbacks.callback_table_symbol, "grNBa_803E7DA0");
    assert_eq!(stage.callbacks.object_callbacks.len(), 7);
    assert_eq!(stage.callbacks.on_touch_line, "grBattle_OnTouchLine");
    assert_eq!(
        stage.callbacks.on_check_shadow_render,
        "grBattle_OnCheckShadowRender"
    );
    assert!(close_to(
        source_units_to_milli(stage.camera.cam_bounds.left),
        -160000,
        1
    ));
    assert!(close_to(
        source_units_to_milli(stage.camera.cam_bounds.right),
        160000,
        1
    ));
    assert!(close_to(
        source_units_to_milli(stage.camera.cam_bounds.top),
        136000,
        1
    ));
    assert!(close_to(
        source_units_to_milli(stage.camera.cam_bounds.bottom),
        -47200,
        1
    ));
    assert_eq!(stage.camera.cam_vertical_tilt.to_bits(), 30.0_f32.to_bits());
    assert_eq!(
        stage.camera.cam_pan_degrees.to_bits(),
        (-10.0_f32).to_bits()
    );
    assert_eq!(stage.camera.cam_zoom_rate.to_bits(), 83.0_f32.to_bits());
    assert!(close_to(
        source_units_to_milli(stage.source_blast_zones.left),
        -224000,
        1
    ));
    assert!(close_to(
        source_units_to_milli(stage.source_blast_zones.right),
        224000,
        1
    ));
    assert!(close_to(
        source_units_to_milli(stage.source_blast_zones.top),
        200000,
        1
    ));
    assert!(close_to(
        source_units_to_milli(stage.source_blast_zones.bottom),
        -108800,
        1
    ));
    assert_eq!(stage.blast_zones.left_x, -224000);
    assert_eq!(stage.blast_zones.right_x, 224000);
    assert_eq!(stage.blast_zones.top_y, 200000);
    assert_eq!(stage.blast_zones.bottom_y, -108800);
}

#[test]
fn airborne_fighter_near_battlefield_ledge_enters_source_cliff_catch() {
    let mut world = World::for_two_players_on_stage(StageProfile::battlefield());
    let ledge = world.stage().ledges[0];
    assert_eq!(ledge.side, StageLedgeSide::Left);
    let mut player = world.players()[0];
    player.set_motion_state_alias(MotionState::Fall);
    player.grounded = false;
    player.position = Vec2 {
        x: ledge.x_milli - 1_500,
        y: ledge.y_milli - 12_000,
    };
    player.source_position = mole_core::SourceVec2::from_milli(player.position);
    player.velocity.y = -1_000;
    player.source_self_velocity_y = -1.0;
    player.jumps_remaining = 0;
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(&mut world, Frame(0), &[PlayerInput::neutral(); 2]);

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::CliffCatch);
    assert_eq!(
        player.melee_action_state_id,
        Some(MeleeActionStateId::new(252))
    );
    assert_eq!(
        player.source_action_key,
        Some(SourceActionKey::new("CliffCatch"))
    );
    assert_eq!(player.facing, 1);
    assert_eq!(player.source_cliff_ledge_id, Some(ledge.index));
    assert_eq!(
        player.position,
        source_scaled_cliff_position_from_transn(
            ledge.x_milli,
            ledge.y_milli,
            player.facing,
            2,
            player.profile
        )
    );
    assert_eq!(
        player.jumps_remaining,
        player.profile.reusable_air_jumps(),
        "ftCliffCommon_80081370 calls ftCommon_8007D5D4 while entering CliffCatch; x1968_jumpsUsed becomes 1, so Falcon's reusable double jump is restored while hanging"
    );
}

#[test]
fn airborne_fighter_facing_away_from_battlefield_ledge_does_not_cliff_catch() {
    let mut world = World::for_two_players_on_stage(StageProfile::battlefield());
    let ledge = world.stage().ledges[0];
    assert_eq!(ledge.side, StageLedgeSide::Left);
    let mut player = world.players()[0];
    player.set_motion_state_alias(MotionState::Fall);
    player.grounded = false;
    player.facing = -1;
    player.position = Vec2 {
        x: ledge.x_milli - 1_500,
        y: ledge.y_milli - 12_000,
    };
    player.source_position = mole_core::SourceVec2::from_milli(player.position);
    player.velocity.y = -1_000;
    player.source_self_velocity_y = -1.0;
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(&mut world, Frame(0), &[PlayerInput::neutral(); 2]);

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::Fall);
    assert_eq!(player.source_cliff_ledge_id, None);
}

#[test]
fn jumpf_near_battlefield_left_lip_uses_source_air_map_collision_position_correction() {
    let mut world = World::for_two_players_on_stage(StageProfile::battlefield());
    let mut player = world.players()[0];
    player.set_motion_state_alias(MotionState::JumpF);
    player.motion_frame = 31;
    player.motion_anim_frame_milli = 31_000;
    player.grounded = false;
    player.facing = 1;
    player.position = Vec2 {
        x: -70_893,
        y: -3_680,
    };
    player.source_position = mole_core::SourceVec2::from_milli(player.position);
    player.source_self_velocity_x = -0.4085;
    player.source_self_velocity_y = -2.13;
    player.velocity = Vec2 {
        x: source_units_to_milli(player.source_self_velocity_x),
        y: source_units_to_milli(player.source_self_velocity_y),
    };
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(&mut world, Frame(1759), &[PlayerInput::neutral(); 2]);

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::JumpF);
    assert_eq!(player.motion_frame, 32);
    assert_eq!(player.position.y, -5_940);
    assert_eq!(player.velocity.x, -399);
    assert_eq!(player.velocity.y, -2_260);
    assert_eq!(player.position.x, -71_842);
}

#[test]
fn grounded_specialhi_x2_b1_keeps_ground_when_floor_support_remains() {
    let mut world = World::for_two_players_on_stage(StageProfile::battlefield());
    let platform = world.stage().soft_platforms[0];
    let mut player = world.players()[0];
    player.set_motion_state_alias(MotionState::SpecialHi);
    player.motion_frame = 35;
    player.motion_anim_frame_milli = 35_000;
    player.grounded = true;
    player.facing = 1;
    player.captain_special_hi_x2_b1 = true;
    player.position = Vec2 {
        x: -40_000,
        y: platform.y,
    };
    player.source_position = mole_core::SourceVec2::from_milli(player.position);
    player.set_source_floor_for_diagnostic(
        Some(1),
        Some(source_floor_line_for_surface_at_x(
            world.stage(),
            platform,
            player.source_position.x,
        )),
    );
    player.source_self_velocity_x = 0.0;
    player.source_self_velocity_y = 0.0;
    player.velocity = Vec2 { x: 0, y: 0 };
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(&mut world, Frame(1901), &[PlayerInput::neutral(); 2]);

    let player = world.players()[0];
    assert_eq!(
        player.motion_state,
        MotionState::SpecialHi,
        "ftCa_SpecialHi_Coll only calls ftCommon_8007D5D4 after ft_80082708 loses ground support; x2_b1 alone must not force ground-to-air"
    );
    assert!(
        player.grounded,
        "grounded SpecialHi should keep support when the current floor remains valid"
    );
    assert_eq!(player.ecb_bottom_lock_timer, 0);
}

#[test]
fn jumpaerialf_near_battlefield_left_lip_uses_source_coll_data_mode6_wall_correction() {
    let mut world = World::for_two_players_on_stage(StageProfile::battlefield());
    let mut player = world.players()[0];
    player.set_motion_state_alias(MotionState::JumpAerialF);
    player.motion_frame = 8;
    player.motion_anim_frame_milli = 8_000;
    player.grounded = false;
    player.facing = 1;
    player.position = Vec2 {
        x: -66_494,
        y: -14_466,
    };
    player.source_position = mole_core::SourceVec2::from_milli(player.position);
    player.source_self_velocity_x = 0.700;
    player.source_self_velocity_y = 1.620;
    player.velocity = Vec2 {
        x: source_units_to_milli(player.source_self_velocity_x),
        y: source_units_to_milli(player.source_self_velocity_y),
    };
    assert!(world.set_player_state_for_diagnostic(0, player));
    world.set_input_history_for_diagnostic(
        [
            PlayerInput::neutral()
                .with_jump(true)
                .with_left_stick(116, -51),
            PlayerInput::neutral(),
        ],
        [MeleeInputTimers::expired(); 2],
    );

    let input = [
        PlayerInput::neutral()
            .with_jump(true)
            .with_left_stick(117, -44),
        PlayerInput::neutral(),
    ];
    step_world(&mut world, Frame(1824), &input);

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::JumpAerialF);
    assert_eq!(player.motion_frame, 9);
    assert_eq!(player.velocity.x, 757);
    assert_eq!(player.velocity.y, 1_490);
    assert_eq!(player.position.y, -12_976);
    assert_eq!(
        player.position.x, -67_215,
        "mpColl_800471F8 loads ECB mode 6 and drives air wall correction through persistent CollData state without over-projecting the left lip"
    );
}

#[test]
fn jumpaerialf_second_left_lip_sweep_uses_source_wall_projection_at_live_jobj_pose() {
    let mut world = World::for_two_players_on_stage(StageProfile::battlefield());
    let mut player = world.players()[0];
    player.set_motion_state_alias(MotionState::JumpAerialF);
    player.motion_frame = 9;
    player.motion_anim_frame_milli = 9_000;
    player.grounded = false;
    player.facing = 1;
    player.position = Vec2 {
        x: -67_214,
        y: -12_976,
    };
    player.source_position = mole_core::SourceVec2 {
        x: -67.21435546875,
        y: -12.976238250732422,
    };
    player.source_self_velocity_x = 0.756_749_987_602_233_9;
    player.source_self_velocity_y = 1.489_999_294_281_005_9;
    player.velocity = Vec2 {
        x: source_units_to_milli(player.source_self_velocity_x),
        y: source_units_to_milli(player.source_self_velocity_y),
    };
    assert!(world.set_player_state_for_diagnostic(0, player));
    world.set_input_history_for_diagnostic(
        [
            PlayerInput::neutral()
                .with_jump(true)
                .with_left_stick(117, -44),
            PlayerInput::neutral(),
        ],
        [MeleeInputTimers::expired(); 2],
    );

    let input = [
        PlayerInput::neutral()
            .with_jump(true)
            .with_left_stick(122, 0),
        PlayerInput::neutral(),
    ];
    step_world(&mut world, Frame(1825), &input);

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::JumpAerialF);
    assert_eq!(player.motion_frame, 10);
    assert_eq!(player.velocity.x, 815);
    assert_eq!(player.velocity.y, 1_360);
    assert_eq!(player.position.y, -11_616);
    assert_eq!(
        player.position.x, -69_146,
        "mpColl_80046224_LeftWall must project the live JObj ECB pose against source Battlefield wall geometry without milli-rounding or stale ECB timing"
    );
}

#[test]
fn jumpaerialf_first_left_lip_sweep_does_not_project_before_source_wall_hit() {
    let mut world = World::for_two_players_on_stage(StageProfile::battlefield());
    let mut player = world.players()[0];
    player.set_motion_state_alias(MotionState::JumpAerialF);
    player.motion_frame = 7;
    player.motion_anim_frame_milli = 7_000;
    player.grounded = false;
    player.facing = 1;
    player.position = Vec2 {
        x: -67_194,
        y: -16_086,
    };
    player.source_position = mole_core::SourceVec2::from_milli(player.position);
    player.source_self_velocity_x = 0.6432499885559082;
    player.source_self_velocity_y = 1.7499990463256836;
    player.velocity = Vec2 {
        x: source_units_to_milli(player.source_self_velocity_x),
        y: source_units_to_milli(player.source_self_velocity_y),
    };
    assert!(world.set_player_state_for_diagnostic(0, player));
    world.set_input_history_for_diagnostic(
        [
            PlayerInput::neutral()
                .with_jump(true)
                .with_left_stick(114, -54),
            PlayerInput::neutral(),
        ],
        [MeleeInputTimers::expired(); 2],
    );

    let input = [
        PlayerInput::neutral()
            .with_jump(true)
            .with_left_stick(116, -51),
        PlayerInput::neutral(),
    ];
    step_world(&mut world, Frame(1823), &input);

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::JumpAerialF);
    assert_eq!(player.motion_frame, 8);
    assert_eq!(player.velocity.x, 700);
    assert_eq!(player.velocity.y, 1_620);
    assert_eq!(player.position.y, -14_466);
    assert_eq!(
        player.position.x, -66_494,
        "mpLib_800515A0_LeftWall must not promote a connected wall endpoint before the source sweep reports a wall hit"
    );
}

#[test]
fn source_cliff_root_motion_samples_match_falcon_transn() {
    assert_eq!(
        source_cliff_position_from_transn(0, 0, 1, 1),
        Vec2 {
            x: -6_320,
            y: -20_479
        }
    );
    assert_eq!(
        source_cliff_position_from_transn(0, 0, 1, 8),
        Vec2 {
            x: -2_566,
            y: -24_891
        }
    );
    assert_eq!(
        source_cliff_wait_position_from_transn(0, 0, 1),
        Vec2 {
            x: -2_529,
            y: -23_811
        }
    );
}

#[test]
fn airborne_fighter_inside_battlefield_floor_edge_does_not_cliff_catch() {
    let mut world = World::for_two_players_on_stage(StageProfile::battlefield());
    let ledge = world.stage().ledges[0];
    assert_eq!(ledge.side, StageLedgeSide::Left);
    let profile = FighterProfile::FALCON_LIKE;
    let mut player = world.players()[0];
    player.set_motion_state_alias(MotionState::Fall);
    player.grounded = false;
    player.position = Vec2 {
        x: ledge.x_milli + profile.ledge_snap_x_milli - 500,
        y: ledge.y_milli + profile.ledge_snap_y_milli,
    };
    player.source_position = mole_core::SourceVec2::from_milli(player.position);
    player.velocity.y = -1_000;
    player.source_self_velocity_y = -1.0;
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(&mut world, Frame(0), &[PlayerInput::neutral(); 2]);

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::Fall);
    assert_eq!(player.source_cliff_ledge_id, None);
}

#[test]
fn airborne_fighter_near_battlefield_platform_edge_does_not_cliff_catch() {
    let mut world = World::for_two_players_on_stage(StageProfile::battlefield());
    let stage = world.stage();
    let left_platform = stage.soft_platforms[0];
    let mut player = world.players()[0];
    player.set_motion_state_alias(MotionState::Fall);
    player.grounded = false;
    player.position = Vec2 {
        x: left_platform.left_x,
        y: left_platform.y - 10_000,
    };
    player.source_position = mole_core::SourceVec2::from_milli(player.position);
    player.velocity.y = -1_000;
    player.source_self_velocity_y = -1.0;
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(&mut world, Frame(0), &[PlayerInput::neutral(); 2]);

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::Fall);
    assert_eq!(player.source_cliff_ledge_id, None);
}

#[test]
fn airborne_fighter_near_any_battlefield_platform_endpoint_does_not_cliff_catch() {
    let stage = StageProfile::battlefield();
    let cases = [
        (
            "left platform left endpoint",
            stage.soft_platforms[0].left_x,
            stage.soft_platforms[0].y,
            1,
        ),
        (
            "left platform right endpoint",
            stage.soft_platforms[0].right_x,
            stage.soft_platforms[0].y,
            -1,
        ),
        (
            "right platform left endpoint",
            stage.soft_platforms[1].left_x,
            stage.soft_platforms[1].y,
            1,
        ),
        (
            "right platform right endpoint",
            stage.soft_platforms[1].right_x,
            stage.soft_platforms[1].y,
            -1,
        ),
        (
            "top platform left endpoint",
            stage.soft_platforms[2].left_x,
            stage.soft_platforms[2].y,
            1,
        ),
        (
            "top platform right endpoint",
            stage.soft_platforms[2].right_x,
            stage.soft_platforms[2].y,
            -1,
        ),
    ];

    for (label, x, y, facing) in cases {
        let mut world = World::for_two_players_on_stage(stage);
        let mut player = world.players()[0];
        player.set_motion_state_alias(MotionState::Fall);
        player.grounded = false;
        player.facing = facing;
        player.position = Vec2 { x, y: y - 10_000 };
        player.source_position = mole_core::SourceVec2::from_milli(player.position);
        player.velocity.y = -1_000;
        player.source_self_velocity_y = -1.0;
        assert!(world.set_player_state_for_diagnostic(0, player));

        step_world(&mut world, Frame(0), &[PlayerInput::neutral(); 2]);

        let player = world.players()[0];
        assert_eq!(player.motion_state, MotionState::Fall, "{label}");
        assert_eq!(
            player.source_cliff_ledge_id, None,
            "{label} must not be treated as a source ledge"
        );
    }
}

#[test]
fn source_ledge_grab_contact_requires_source_line_ledge_flag() {
    static UNFLAGGED_MAIN_FLOOR_SEAM: [StageLedge; 1] = [StageLedge {
        index: 99,
        line_index: 1,
        side: StageLedgeSide::Left,
        x_milli: -60_000,
        y_milli: 0,
    }];

    let mut stage = StageProfile::battlefield();
    stage.ledges = &UNFLAGGED_MAIN_FLOOR_SEAM;
    let current_root_position = Vec2 {
        x: -61_000,
        y: -10_000,
    };
    let current_ecb = EcbDiamond {
        top: Vec2 {
            x: -61_000,
            y: 5_000,
        },
        right: Vec2 {
            x: -59_500,
            y: -5_000,
        },
        bottom: current_root_position,
        left: Vec2 {
            x: -62_500,
            y: -5_000,
        },
    };

    let contact = source_ledge_grab_contact(
        stage,
        current_root_position,
        current_root_position,
        current_ecb,
        4_000,
        0,
        20_000,
    );

    assert!(contact.is_none());
}

#[test]
fn source_ledge_grab_contact_uses_baked_stage_ledge_list_as_runtime_authority() {
    let mut stage = StageProfile::battlefield();
    stage.ledges = &[];
    let current_root_position = Vec2 {
        x: -69_900,
        y: -12_000,
    };
    let current_ecb = EcbDiamond {
        top: Vec2 {
            x: current_root_position.x,
            y: current_root_position.y + 8_000,
        },
        right: Vec2 {
            x: current_root_position.x + 4_000,
            y: current_root_position.y + 4_000,
        },
        bottom: current_root_position,
        left: Vec2 {
            x: current_root_position.x - 4_000,
            y: current_root_position.y + 4_000,
        },
    };

    let contact = source_ledge_grab_contact(
        stage,
        current_root_position,
        current_root_position,
        current_ecb,
        FighterProfile::FALCON_LIKE.ledge_snap_x_milli,
        FighterProfile::FALCON_LIKE.ledge_snap_y_milli,
        FighterProfile::FALCON_LIKE.ledge_snap_height_milli,
    );

    assert!(
        contact.is_none(),
        "runtime ledge detection must consume the baked StageProfile ledge list, not rediscover arbitrary collision-line endpoints"
    );
}

#[test]
fn source_cliff_catch_animation_end_enters_cliff_wait_on_same_ledge() {
    let mut world = World::for_two_players_on_stage(StageProfile::battlefield());
    let ledge = world.stage().ledges[0];
    let mut player = world.players()[0];
    player.set_motion_state_alias(MotionState::CliffCatch);
    player.grounded = false;
    player.facing = 1;
    player.source_cliff_ledge_id = Some(ledge.index);
    player.position = source_cliff_position_from_transn(ledge.x_milli, ledge.y_milli, 1, 1);
    player.source_position = mole_core::SourceVec2::from_milli(player.position);
    assert!(world.set_player_state_for_diagnostic(0, player));

    for frame in 0..7 {
        step_world(&mut world, Frame(frame), &[PlayerInput::neutral(); 2]);
    }

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::CliffWait);
    assert_eq!(
        player.melee_action_state_id,
        Some(MeleeActionStateId::new(253))
    );
    assert_eq!(
        player.source_action_key,
        Some(SourceActionKey::new("CliffWait1"))
    );
    assert_eq!(player.source_cliff_ledge_id, Some(ledge.index));
    assert_eq!(
        player.position,
        source_scaled_cliff_wait_position_from_transn(
            ledge.x_milli,
            ledge.y_milli,
            player.facing,
            player.profile
        )
    );
    let expected_source_position = source_scaled_cliff_wait_source_position_from_transn(
        ledge.x_milli,
        ledge.y_milli,
        player.facing,
        player.profile,
    );
    assert!(
        (player.source_position.x - expected_source_position.x).abs() < 0.000_001
            && (player.source_position.y - expected_source_position.y).abs() < 0.000_001,
        "ftCo_CliffCatch_Phys stores float cur_pos from ledge vertex plus scaled TransN without a milli round-trip"
    );
}

#[test]
fn source_cliff_catch_exits_on_last_source_sample_frame() {
    let mut world = World::for_two_players_on_stage(StageProfile::battlefield());
    let ledge = world.stage().ledges[0];
    let mut player = world.players()[0];
    player.set_motion_state_alias(MotionState::CliffCatch);
    player.grounded = false;
    player.facing = 1;
    player.motion_frame = 6;
    player.source_cliff_ledge_id = Some(ledge.index);
    player.position = source_cliff_position_from_transn(ledge.x_milli, ledge.y_milli, 1, 7);
    player.source_position = mole_core::SourceVec2::from_milli(player.position);
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(&mut world, Frame(0), &[PlayerInput::neutral(); 2]);

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::CliffWait);
    assert_eq!(
        player.melee_action_state_id,
        Some(MeleeActionStateId::new(253))
    );
    assert_eq!(
        player.position,
        source_scaled_cliff_wait_position_from_transn(
            ledge.x_milli,
            ledge.y_milli,
            player.facing,
            player.profile
        )
    );
}

#[test]
fn source_cliff_catch_rejects_ledge_occupied_by_another_fighter() {
    let mut world = World::for_two_players_on_stage(StageProfile::battlefield());
    let ledge = world.stage().ledges[0];

    let mut owner = world.players()[0];
    owner.set_motion_state_alias(MotionState::CliffWait);
    owner.grounded = false;
    owner.facing = 1;
    owner.source_cliff_ledge_id = Some(ledge.index);
    owner.source_cliff_wait_timer = 30;
    owner.position = source_cliff_wait_position_from_transn(ledge.x_milli, ledge.y_milli, 1);
    owner.source_position = mole_core::SourceVec2::from_milli(owner.position);
    assert!(world.set_player_state_for_diagnostic(0, owner));

    let mut challenger = world.players()[1];
    challenger.set_motion_state_alias(MotionState::Fall);
    challenger.grounded = false;
    challenger.position = Vec2 {
        x: ledge.x_milli - 1_500,
        y: ledge.y_milli - 12_000,
    };
    challenger.source_position = mole_core::SourceVec2::from_milli(challenger.position);
    challenger.velocity.y = -1_000;
    challenger.source_self_velocity_y = -1.0;
    assert!(world.set_player_state_for_diagnostic(1, challenger));

    step_world(&mut world, Frame(0), &[PlayerInput::neutral(); 2]);

    let owner = world.players()[0];
    assert_eq!(owner.motion_state, MotionState::CliffWait);
    assert_eq!(owner.source_cliff_ledge_id, Some(ledge.index));
    let challenger = world.players()[1];
    assert_eq!(challenger.motion_state, MotionState::Fall);
    assert_eq!(challenger.source_cliff_ledge_id, None);
}

#[test]
fn source_ledge_cooldown_blocks_immediate_regrab_and_ticks_down() {
    let mut world = World::for_two_players_on_stage(StageProfile::battlefield());
    let ledge = world.stage().ledges[0];
    let mut player = world.players()[0];
    player.set_motion_state_alias(MotionState::Fall);
    player.grounded = false;
    player.source_ledge_cooldown_timer = 5;
    player.position = Vec2 {
        x: ledge.x_milli - 1_500,
        y: ledge.y_milli - 12_000,
    };
    player.source_position = mole_core::SourceVec2::from_milli(player.position);
    player.velocity.y = -1_000;
    player.source_self_velocity_y = -1.0;
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(&mut world, Frame(0), &[PlayerInput::neutral(); 2]);

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::Fall);
    assert_eq!(player.source_cliff_ledge_id, None);
    assert_eq!(player.source_ledge_cooldown_timer, 4);
}

#[test]
fn source_cliff_wait_uses_plco_timer_and_hurt_intangibility() {
    let common = MeleeCommonData {
        cliff_quick_percent_threshold: 100,
        cliff_wait_low_percent_ticks: 12,
        cliff_wait_high_percent_ticks: 7,
        cliff_wait_hurt_intangible_ticks: 5,
        ..MeleeCommonData::provisional_mole()
    };
    let mut world = World::for_two_players_on_stage_with_profiles_and_common_data(
        StageProfile::battlefield(),
        [FighterProfile::FALCON_LIKE; 2],
        common,
    );
    let ledge = world.stage().ledges[0];
    let mut player = world.players()[0];
    player.set_motion_state_alias(MotionState::CliffCatch);
    player.grounded = false;
    player.facing = 1;
    player.damage_percent = 99.0;
    player.source_cliff_ledge_id = Some(ledge.index);
    player.position = source_cliff_position_from_transn(ledge.x_milli, ledge.y_milli, 1, 1);
    player.source_position = mole_core::SourceVec2::from_milli(player.position);
    assert!(world.set_player_state_for_diagnostic(0, player));

    for frame in 0..7 {
        step_world(&mut world, Frame(frame), &[PlayerInput::neutral(); 2]);
    }

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::CliffWait);
    assert_eq!(player.source_cliff_wait_timer, 12);
    assert_eq!(player.source_hurt_intangible_timer, 5);
}

fn source_world_with_player_in_cliff_wait(common: MeleeCommonData, damage_percent: f32) -> World {
    let mut world = World::for_two_players_on_stage_with_profiles_and_common_data(
        StageProfile::battlefield(),
        [FighterProfile::FALCON_LIKE; 2],
        common,
    );
    let ledge = world.stage().ledges[0];
    let mut player = world.players()[0];
    player.set_motion_state_alias(MotionState::CliffWait);
    player.grounded = false;
    player.facing = 1;
    player.damage_percent = damage_percent;
    player.source_cliff_stick_gate = true;
    player.source_cliff_wait_timer = 30;
    player.source_cliff_ledge_id = Some(ledge.index);
    player.position = source_cliff_wait_position_from_transn(ledge.x_milli, ledge.y_milli, 1);
    player.source_position = mole_core::SourceVec2::from_milli(player.position);
    assert!(world.set_player_state_for_diagnostic(0, player));
    world
}

#[test]
fn source_cliff_wait_arms_gate_then_away_down_releases_to_fall_with_cooldown() {
    let common = MeleeCommonData {
        cliff_option_stick_threshold: 32,
        ledge_cooldown_ticks: 9,
        ..MeleeCommonData::provisional_mole()
    };
    let mut world = World::for_two_players_on_stage_with_profiles_and_common_data(
        StageProfile::battlefield(),
        [FighterProfile::FALCON_LIKE; 2],
        common,
    );
    let ledge = world.stage().ledges[0];
    let mut player = world.players()[0];
    player.set_motion_state_alias(MotionState::CliffWait);
    player.grounded = false;
    player.facing = 1;
    player.source_cliff_wait_timer = 30;
    player.source_cliff_ledge_id = Some(ledge.index);
    player.position = source_cliff_wait_position_from_transn(ledge.x_milli, ledge.y_milli, 1);
    player.source_position = mole_core::SourceVec2::from_milli(player.position);
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(&mut world, Frame(0), &[PlayerInput::neutral(); 2]);

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::CliffWait);
    assert_eq!(player.source_cliff_wait_timer, 29);
    assert_eq!(player.source_ledge_cooldown_timer, 0);

    step_world(
        &mut world,
        Frame(1),
        &[
            PlayerInput::neutral().with_left_stick(-90, -80),
            PlayerInput::neutral(),
        ],
    );

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::Fall);
    assert_eq!(player.source_cliff_ledge_id, None);
    assert_eq!(player.source_ledge_cooldown_timer, 9);
}

#[test]
fn source_cliff_wait_a_or_b_enters_decomp_cliff_attack_percent_variant() {
    let common = MeleeCommonData {
        cliff_quick_percent_threshold: 100,
        ..MeleeCommonData::provisional_mole()
    };
    let mut quick = source_world_with_player_in_cliff_wait(common, 25.0);
    let mut slow = source_world_with_player_in_cliff_wait(common, 125.0);

    step_world(
        &mut quick,
        Frame(0),
        &[
            PlayerInput::neutral().with_attack(true),
            PlayerInput::neutral(),
        ],
    );
    step_world(
        &mut slow,
        Frame(0),
        &[
            PlayerInput::neutral().with_special(true),
            PlayerInput::neutral(),
        ],
    );

    let quick_player = quick.players()[0];
    assert_eq!(quick_player.motion_state, MotionState::CliffAttackQuick);
    assert_eq!(
        quick_player.source_action_key,
        Some(SourceActionKey::new("CliffAttackQuick"))
    );
    assert_eq!(
        melee_action_state_id_for_motion_state(quick_player.motion_state).get(),
        257
    );
    let slow_player = slow.players()[0];
    assert_eq!(slow_player.motion_state, MotionState::CliffAttackSlow);
    assert_eq!(
        slow_player.source_action_key,
        Some(SourceActionKey::new("CliffAttackSlow"))
    );
    assert_eq!(
        melee_action_state_id_for_motion_state(slow_player.motion_state).get(),
        256
    );
}

#[test]
fn source_cliff_wait_lr_enters_decomp_cliff_escape() {
    let common = MeleeCommonData::provisional_mole();
    let mut world = source_world_with_player_in_cliff_wait(common, 10.0);

    step_world(
        &mut world,
        Frame(0),
        &[
            PlayerInput::neutral().with_right_trigger_digital(true),
            PlayerInput::neutral(),
        ],
    );

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::CliffEscapeQuick);
    assert_eq!(
        player.source_action_key,
        Some(SourceActionKey::new("CliffEscapeQuick"))
    );
    assert_eq!(
        melee_action_state_id_for_motion_state(player.motion_state).get(),
        259
    );
}

#[test]
fn source_cliff_wait_escape_entry_samples_and_anchors_new_action_pose() {
    let common = MeleeCommonData::provisional_mole();
    let mut world = source_world_with_player_in_cliff_wait(common, 10.0);
    let ledge = world.stage().ledges[0];

    step_world(
        &mut world,
        Frame(0),
        &[
            PlayerInput::neutral().with_right_trigger_digital(true),
            PlayerInput::neutral(),
        ],
    );

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::CliffEscapeQuick);
    let entry_sampled_pose =
        source_root_motion_position_for_action_key(SourceActionKey::new("CliffEscapeQuick"), 2)
            .expect("CliffEscapeQuick sampled entry TransN position should be extracted");
    let expected_source_position = SourceVec2 {
        x: milli_to_source_units(ledge.x_milli)
            + entry_sampled_pose.z * player.profile.model_scaling * f32::from(player.facing),
        y: milli_to_source_units(ledge.y_milli)
            + entry_sampled_pose.y * player.profile.model_scaling,
    };
    assert!(
        (player.source_position.x - expected_source_position.x).abs() < 0.000_001
            && (player.source_position.y - expected_source_position.y).abs() < 0.000_001,
        "ftCo_8009B040 calls ftAnim_8006EBA4 and ftCo_CliffCatch_Phys before returning from CliffWait IASA"
    );
}

#[test]
fn source_root_motion_motion_state_lookup_covers_decomp_cliff_actions() {
    let cliff_actions = [
        (MotionState::CliffAttackQuick, "CliffAttackQuick"),
        (MotionState::CliffAttackSlow, "CliffAttackSlow"),
        (MotionState::CliffClimbQuick, "CliffClimbQuick"),
        (MotionState::CliffClimbSlow, "CliffClimbSlow"),
        (MotionState::CliffEscapeQuick, "CliffEscapeQuick"),
        (MotionState::CliffEscapeSlow, "CliffEscapeSlow"),
        (MotionState::CliffJumpQuick1, "CliffJumpQuick1"),
        (MotionState::CliffJumpQuick2, "CliffJumpQuick2"),
        (MotionState::CliffJumpSlow1, "CliffJumpSlow1"),
        (MotionState::CliffJumpSlow2, "CliffJumpSlow2"),
    ];

    for (motion_state, source_key) in cliff_actions {
        let source_key = SourceActionKey::new(source_key);
        assert_eq!(
            source_root_motion_frame_count(motion_state),
            source_root_motion_frame_count_for_action_key(source_key),
            "{motion_state:?} must expose the same compact TransN frame count by MotionState that extraction exposes by source action key"
        );
        assert_eq!(
            source_root_motion_delta(motion_state, 1),
            source_root_motion_delta_for_action_key(source_key, 1),
            "{motion_state:?} must expose the same first-frame TransN delta by MotionState"
        );
        assert_eq!(
            source_root_motion_position(motion_state, 1),
            source_root_motion_position_for_action_key(source_key, 1),
            "{motion_state:?} must expose the same first-frame TransN position by MotionState"
        );
    }
}

#[test]
fn source_cliff_escape_quick_continues_with_cliff_climb_physics() {
    let mut world = World::for_two_players_on_stage(StageProfile::battlefield());
    let ledge = world.stage().ledges[0];
    let mut player = world.players()[0];
    player.set_motion_state_alias(MotionState::CliffEscapeQuick);
    player.grounded = false;
    player.facing = ledge_facing_for_test(ledge.side);
    player.source_cliff_ledge_id = Some(ledge.index);
    let entry_pose =
        source_root_motion_position_for_action_key(SourceActionKey::new("CliffEscapeQuick"), 1)
            .expect("CliffEscapeQuick TransN source-action position should be extracted");
    player.source_position = SourceVec2 {
        x: milli_to_source_units(ledge.x_milli)
            + entry_pose.z * player.profile.model_scaling * f32::from(player.facing),
        y: milli_to_source_units(ledge.y_milli) + entry_pose.y * player.profile.model_scaling,
    };
    player.position = Vec2 {
        x: source_units_to_milli(player.source_position.x),
        y: source_units_to_milli(player.source_position.y),
    };
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(&mut world, Frame(0), &[PlayerInput::neutral(); 2]);

    let player = world.players()[0];
    assert_eq!(
        player.motion_state,
        MotionState::CliffEscapeQuick,
        "ftCo_CliffEscape_Phys delegates to ftCo_CliffClimb_Phys; a valid ledge must not drop to Fall on the first continuation frame"
    );
    assert_eq!(player.source_cliff_ledge_id, Some(ledge.index));
    let next_pose =
        source_root_motion_position_for_action_key(SourceActionKey::new("CliffEscapeQuick"), 2)
            .expect("CliffEscapeQuick second-frame TransN position should be extracted");
    let expected_source_position = SourceVec2 {
        x: milli_to_source_units(ledge.x_milli)
            + next_pose.z * player.profile.model_scaling * f32::from(player.facing),
        y: milli_to_source_units(ledge.y_milli) + next_pose.y * player.profile.model_scaling,
    };
    assert!(
        (player.source_position.x - expected_source_position.x).abs() < 0.000_001
            && (player.source_position.y - expected_source_position.y).abs() < 0.000_001,
        "ftCo_CliffClimb_Phys stores cur_pos from ledge vertex plus the current TransN pose"
    );
}

#[test]
fn source_cliff_escape_ground_commit_uses_transn_offset_velocity() {
    let mut world = World::for_two_players_on_stage(StageProfile::battlefield());
    let ledge = world.stage().ledges[1];
    let mut player = world.players()[0];
    player.set_motion_state_alias(MotionState::CliffEscapeQuick);
    player.grounded = false;
    player.facing = ledge_facing_for_test(ledge.side);
    player.source_cliff_ledge_id = Some(ledge.index);
    player.motion_frame = 15;
    player.set_source_motion_anim_frame(16.0);
    let setup_pose =
        source_root_motion_position_for_action_key(SourceActionKey::new("CliffEscapeQuick"), 17)
            .expect("CliffEscapeQuick setup TransN position should be extracted");
    player.source_position = SourceVec2 {
        x: milli_to_source_units(ledge.x_milli)
            + setup_pose.z * player.profile.model_scaling * f32::from(player.facing),
        y: milli_to_source_units(ledge.y_milli) + setup_pose.y * player.profile.model_scaling,
    };
    player.position = Vec2 {
        x: source_units_to_milli(player.source_position.x),
        y: source_units_to_milli(player.source_position.y),
    };
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(&mut world, Frame(0), &[PlayerInput::neutral(); 2]);

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::CliffEscapeQuick);
    assert!(
        player.grounded,
        "ftCo_CliffClimb_Phys calls ftCommon_8007D7FC when TransN z/y are nonnegative"
    );
    let commit_delta =
        source_root_motion_delta_for_action_key(SourceActionKey::new("CliffEscapeQuick"), 18)
            .expect("CliffEscapeQuick commit TransN offset should be extracted");
    let expected_ground_velocity_x =
        commit_delta.z * player.profile.model_scaling * f32::from(player.facing);
    assert!(
        (player.ground_velocity_x - expected_ground_velocity_x).abs() < 0.000_001
            && (player.source_self_velocity_x - expected_ground_velocity_x).abs() < 0.000_001,
        "ftCommon_8007D6A4 seeds self_vel.x from x6A4_transNOffset.z, calls ftCommon_ClampGrVel on the old gr_vel, then assigns gr_vel = self_vel.x; actual gr_vel={} self_vel.x={} expected={}",
        player.ground_velocity_x,
        player.source_self_velocity_x,
        expected_ground_velocity_x
    );
}

#[test]
fn source_cliff_escape_completion_falls_through_wait_iasa_on_same_frame() {
    let mut world = World::for_two_players_on_stage(StageProfile::battlefield());
    let ledge = world.stage().ledges[1];
    let mut player = world.players()[0];
    player.set_motion_state_alias(MotionState::CliffEscapeQuick);
    player.grounded = true;
    player.facing = ledge_facing_for_test(ledge.side);
    player.source_cliff_ledge_id = Some(ledge.index);
    player.motion_frame = 47;
    player.set_source_motion_anim_frame(48.0);
    player.ground_velocity_x = milli_to_source_units(4);
    player.source_self_velocity_x = player.ground_velocity_x;
    player.velocity.x = 4;
    assert_eq!(
        player.source_action_total_frames, 49,
        "CliffEscapeQuick duration must come from the extracted figatree frame count"
    );
    assert!(world.set_player_state_for_diagnostic(0, player));
    let held_left = PlayerInput::neutral().with_left_stick(-127, 0);
    let mut timers = [MeleeInputTimers::expired(); 2];
    timers[0].x_tap = world.common_data().dash_tap_window;
    world.set_input_history_for_diagnostic([held_left, PlayerInput::neutral()], timers);

    step_world(
        &mut world,
        Frame(0),
        &[
            PlayerInput::neutral().with_left_stick(-124, 0),
            PlayerInput::neutral(),
        ],
    );

    let player = world.players()[0];
    assert_eq!(
        player.motion_state,
        MotionState::WalkSlow,
        "ftCo_CliffClimb_Anim calls ftCommon_8007D92C on animation completion; grounded ft_8008A2BC enters Wait and same-frame Wait_IASA can consume held walk"
    );
    assert_eq!(player.motion_frame, 0);
    assert_eq!(player.source_cliff_ledge_id, None);
    assert_eq!(player.velocity.x, -242);
}

#[test]
fn source_cliff_wait_jump_input_enters_decomp_cliff_jump1() {
    let common = MeleeCommonData::provisional_mole();
    let mut world = source_world_with_player_in_cliff_wait(common, 10.0);

    step_world(
        &mut world,
        Frame(0),
        &[
            PlayerInput::neutral().with_jump(true),
            PlayerInput::neutral(),
        ],
    );

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::CliffJumpQuick1);
    assert_eq!(
        player.source_action_key,
        Some(SourceActionKey::new("CliffJumpQuick1"))
    );
    assert_eq!(
        melee_action_state_id_for_motion_state(player.motion_state).get(),
        262
    );
}

#[test]
fn source_cliff_wait_toward_after_gate_enters_decomp_cliff_climb() {
    let common = MeleeCommonData {
        cliff_option_stick_threshold: 32,
        ..MeleeCommonData::provisional_mole()
    };
    let mut world = source_world_with_player_in_cliff_wait(common, 10.0);

    step_world(&mut world, Frame(0), &[PlayerInput::neutral(); 2]);
    step_world(
        &mut world,
        Frame(1),
        &[
            PlayerInput::neutral().with_left_stick(90, 80),
            PlayerInput::neutral(),
        ],
    );

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::CliffClimbQuick);
    assert_eq!(
        player.source_action_key,
        Some(SourceActionKey::new("CliffClimbQuick"))
    );
    assert_eq!(
        melee_action_state_id_for_motion_state(player.motion_state).get(),
        255
    );
}

#[test]
fn source_cliff_wait_drop_applies_fall_physics_on_transition_frame() {
    let common = MeleeCommonData {
        cliff_option_stick_threshold: 32,
        ledge_cooldown_ticks: 9,
        ..MeleeCommonData::provisional_mole()
    };
    let mut world = World::for_two_players_on_stage_with_profiles_and_common_data(
        StageProfile::battlefield(),
        [FighterProfile::FALCON_LIKE; 2],
        common,
    );
    let ledge = world.stage().ledges[0];
    let mut player = world.players()[0];
    player.set_motion_state_alias(MotionState::CliffWait);
    player.grounded = false;
    player.facing = 1;
    player.source_cliff_wait_timer = 30;
    player.source_cliff_ledge_id = Some(ledge.index);
    player.position = source_cliff_wait_position_from_transn(ledge.x_milli, ledge.y_milli, 1);
    player.source_position = mole_core::SourceVec2::from_milli(player.position);
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(&mut world, Frame(0), &[PlayerInput::neutral(); 2]);
    let held_position = world.players()[0].position;

    step_world(
        &mut world,
        Frame(1),
        &[
            PlayerInput::neutral().with_left_stick(-48, -80),
            PlayerInput::neutral(),
        ],
    );

    let player = world.players()[0];
    let expected_velocity_y = source_units_to_milli(-player.profile.gravity);
    let stick_x = fighter_stick_axis_to_f32(-48);
    let expected_source_velocity_x =
        stick_x * player.profile.air_drift_stick_multiplier - player.profile.aerial_drift_base;
    let expected_velocity_x = source_units_to_milli(expected_source_velocity_x);
    assert_eq!(player.motion_state, MotionState::Fall);
    assert_eq!(
        player.velocity.x, expected_velocity_x,
        "ftCo_80090780 enters Fall, then Fall_Phys/ft_80084DB0 must run ftCommon_8007D268 horizontal air drift on the same ledge-drop frame"
    );
    assert_eq!(player.velocity.y, expected_velocity_y);
    assert_eq!(player.position.x, held_position.x + expected_velocity_x);
    assert_eq!(player.position.y, held_position.y + expected_velocity_y);
    assert_eq!(player.source_cliff_ledge_id, None);
    assert_eq!(player.source_ledge_cooldown_timer, 9);
}

#[test]
fn battlefield_stage_exposes_ordered_collision_surfaces() {
    let stage = StageProfile::battlefield();
    let surfaces = stage.collision_surfaces();

    assert_eq!(surfaces.len(), 4);
    assert_eq!(surfaces[0], stage.main_floor);
    assert_eq!(surfaces[1], stage.soft_platforms[0]);
    assert_eq!(surfaces[2], stage.soft_platforms[1]);
    assert_eq!(surfaces[3], stage.soft_platforms[2]);
}

#[test]
fn battlefield_stage_records_slippi_neutral_spawn_points() {
    let stage = StageProfile::battlefield_test();

    assert_eq!(
        stage.spawn_points,
        [
            StageSpawnPoint {
                x: melee_units_f32(-38.8),
                y: melee_units_f32(35.2),
                facing: 1,
            },
            StageSpawnPoint {
                x: melee_units_f32(38.8),
                y: melee_units_f32(35.2),
                facing: -1,
            },
            StageSpawnPoint {
                x: 0,
                y: melee_units_f32(8.0),
                facing: 1,
            },
            StageSpawnPoint {
                x: 0,
                y: melee_units_f32(62.4),
                facing: 1,
            },
        ]
    );
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
fn world_owns_battlefield_stage_for_deterministic_contact() {
    let world = World::for_two_players();
    let stage = world.stage();

    assert_eq!(stage.name, "battlefield");
    assert_eq!(stage.main_floor.name, "main_floor");
    assert_eq!(stage.soft_platforms.len(), 3);
}

#[test]
fn slippi_battlefield_match_start_uses_entry_spawns_and_source_stagger() {
    let world = World::for_slippi_battlefield_singles_match_start();

    assert_eq!(
        world.players()[0].position,
        Vec2 {
            x: melee_units_f32(-38.8),
            y: melee_units_f32(35.2),
        }
    );
    assert_eq!(world.players()[0].facing, 1);
    assert_eq!(world.players()[0].motion_state, MotionState::Entry);
    assert_eq!(world.players()[0].entry_timer, 5);
    assert!(!world.players()[0].grounded);

    assert_eq!(
        world.players()[1].position,
        Vec2 {
            x: melee_units_f32(38.8),
            y: melee_units_f32(35.2),
        }
    );
    assert_eq!(world.players()[1].facing, -1);
    assert_eq!(world.players()[1].motion_state, MotionState::Entry);
    assert_eq!(world.players()[1].entry_timer, 10);
    assert!(!world.players()[1].grounded);
}

#[test]
fn entry_states_follow_decomp_timer_schedule_from_match_start() {
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    for frame in 0..=5 {
        step_world(&mut world, Frame(frame), &neutral);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::EntryStart);
    assert_eq!(
        world.players()[0].entry_timer,
        MeleeCommonData::provisional_mole().entry_start_ticks - 1
    );
    assert_eq!(world.players()[1].motion_state, MotionState::Entry);
    assert_eq!(world.players()[1].entry_timer, 4);

    for frame in 6..=10 {
        step_world(&mut world, Frame(frame), &neutral);
    }

    assert_eq!(world.players()[1].motion_state, MotionState::EntryStart);
    assert_eq!(
        world.players()[1].entry_timer,
        MeleeCommonData::provisional_mole().entry_start_ticks - 1
    );

    for frame in 11..=34 {
        step_world(&mut world, Frame(frame), &neutral);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::EntryEnd);
    assert_eq!(
        world.players()[0].entry_timer,
        MeleeCommonData::provisional_mole().entry_end_ticks
    );

    for frame in 35..=64 {
        step_world(&mut world, Frame(frame), &neutral);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::Fall);
    assert_eq!(world.players()[0].entry_timer, 0);
}

#[test]
fn slippi_battlefield_spawn_fall_lands_on_the_same_frame_as_replay_oracle() {
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    for frame in 0..=75 {
        step_world(&mut world, Frame(frame), &neutral);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::Fall);
    assert_eq!(world.players()[0].position.y, melee_units_f32(25.114_899));

    step_world(&mut world, Frame(76), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::Landing);
    assert_eq!(world.players()[0].position.y, melee_units_f32(27.200_1));
}

#[test]
fn fighter_ecb_uses_decomp_top_right_bottom_left_vertices() {
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
fn fighter_ecb_source_default_matches_mpcoll_initial_ecb() {
    let ecb = EcbDiamond::SOURCE_DEFAULT;

    assert_eq!(ecb.top, Vec2 { x: 0, y: 8_000 });
    assert_eq!(ecb.right, Vec2 { x: 4_000, y: 4_000 });
    assert_eq!(ecb.bottom, Vec2 { x: 0, y: 0 });
    assert_eq!(
        ecb.left,
        Vec2 {
            x: -4_000,
            y: 4_000
        }
    );
    assert_eq!(
        ecb.debug_root_cross(
            Vec2 {
                x: 10_000,
                y: 20_000
            },
            1_000
        ),
        [
            Vec2 {
                x: 9_000,
                y: 20_000
            },
            Vec2 {
                x: 11_000,
                y: 20_000
            },
            Vec2 {
                x: 10_000,
                y: 19_000
            },
            Vec2 {
                x: 10_000,
                y: 21_000
            },
        ]
    );
}

#[test]
fn battlefield_stage_surface_friction_multiplier_is_baked_in_as_float() {
    let stage = StageProfile::battlefield_test();

    assert_eq!(
        stage.main_floor.friction_multiplier.to_bits(),
        1.0_f32.to_bits()
    );
    assert_eq!(
        stage.soft_platforms[0].friction_multiplier.to_bits(),
        1.0_f32.to_bits()
    );
    assert_eq!(
        stage.soft_platforms[1].friction_multiplier.to_bits(),
        1.0_f32.to_bits()
    );
    assert_eq!(
        stage.soft_platforms[2].friction_multiplier.to_bits(),
        1.0_f32.to_bits()
    );
}

#[test]
fn vertical_stage_contact_lands_on_main_floor_when_crossing_downward() {
    let stage = StageProfile::battlefield_test();
    let contact = mole_core::landing_contact_for_bottom(
        stage,
        Vec2 { x: 0, y: 2_000 },
        Vec2 { x: 0, y: -500 },
        false,
    )
    .expect("downward crossing should contact main floor");

    assert_eq!(contact.surface.name, "main_floor");
    assert_eq!(contact.surface.kind, StageSurfaceKind::Solid);
    assert_eq!(contact.y, 0);
}

#[test]
fn vertical_stage_contact_lands_on_soft_platform_when_enabled() {
    let stage = StageProfile::battlefield_test();
    let platform = stage.soft_platforms[2];
    let contact = mole_core::landing_contact_for_bottom(
        stage,
        Vec2 {
            x: 0,
            y: platform.y + 2_000,
        },
        Vec2 {
            x: 0,
            y: platform.y - 500,
        },
        false,
    )
    .expect("downward crossing should contact top platform");

    assert_eq!(contact.surface.name, "top_platform");
    assert_eq!(contact.surface.kind, StageSurfaceKind::Soft);
    assert_eq!(contact.y, platform.y);
}

#[test]
fn vertical_stage_contact_ignores_soft_platform_when_dropping_through() {
    let stage = StageProfile::battlefield_test();
    let platform = stage.soft_platforms[2];
    let contact = mole_core::landing_contact_for_bottom(
        stage,
        Vec2 {
            x: 0,
            y: platform.y + 2_000,
        },
        Vec2 {
            x: 0,
            y: platform.y - 500,
        },
        true,
    );

    assert!(contact.is_none());
}

#[test]
fn stage_floor_support_requires_surface_range_and_matching_height() {
    let stage = StageProfile::battlefield_test();
    let floor = stage.main_floor;

    assert!(mole_core::has_floor_support(
        stage,
        Vec2 {
            x: floor.left_x,
            y: floor.y,
        }
    ));
    assert!(mole_core::has_floor_support(
        stage,
        Vec2 {
            x: floor.right_x,
            y: floor.y,
        }
    ));
    assert!(!mole_core::has_floor_support(
        stage,
        Vec2 {
            x: floor.left_x - 1,
            y: floor.y,
        }
    ));
    assert!(!mole_core::has_floor_support(
        stage,
        Vec2 {
            x: 0,
            y: floor.y + 1,
        }
    ));
}

#[test]
fn stage_floor_support_reports_surface_kind() {
    let stage = StageProfile::battlefield_test();
    let floor = stage.main_floor;
    let platform = stage.soft_platforms[0];

    let main_support = mole_core::floor_surface_for_bottom(
        stage,
        Vec2 {
            x: floor.left_x,
            y: floor.y,
        },
    )
    .expect("main floor support should be returned");
    assert_eq!(main_support.name, "main_floor");
    assert_eq!(main_support.kind, StageSurfaceKind::Solid);

    let platform_support = mole_core::floor_surface_for_bottom(
        stage,
        Vec2 {
            x: platform.right_x,
            y: platform.y,
        },
    )
    .expect("soft platform support should be returned");
    assert_eq!(platform_support.name, "left_platform");
    assert_eq!(platform_support.kind, StageSurfaceKind::Soft);

    assert_eq!(
        mole_core::floor_surface_for_bottom(
            stage,
            Vec2 {
                x: platform.right_x + 1,
                y: platform.y,
            },
        ),
        None
    );
}

#[test]
fn falcon_like_profile_exposes_public_falcon_gameplay_values() {
    let profile = FighterProfile::falcon_like();

    assert_eq!(profile.reference_character, "captain_falcon");
    assert_eq!(
        profile.captain_special_attrs.specialhi_horz_vel.to_bits(),
        0.8500000238418579_f32.to_bits()
    );
    assert_eq!(
        profile
            .captain_special_attrs
            .specialhi_air_friction_mul
            .to_bits(),
        1.100000023841858_f32.to_bits()
    );
    assert_eq!(
        profile
            .captain_special_attrs
            .specialhi_freefall_air_spd_mul
            .to_bits(),
        0.7200000286102295_f32.to_bits()
    );
    assert_eq!(
        profile
            .captain_special_attrs
            .specialhi_landing_lag
            .to_bits(),
        30.0_f32.to_bits()
    );
    assert_eq!(
        profile.captain_special_attrs.specialhi_input_var.to_bits(),
        0.22499999403953552_f32.to_bits()
    );
    assert_eq!(profile.captain_special_attrs.specialhi_air_var, 0);
    assert_eq!(profile.captain_special_attrs.speciallw_unk1, 4);
    assert_eq!(
        profile
            .captain_special_attrs
            .speciallw_air_landing_traction
            .to_bits(),
        3.0_f32.to_bits()
    );
    assert_eq!(profile.action_frames.attack1_total_frames, 21);
    assert_eq!(profile.action_frames.attack1_iasa_frame, 16);
    assert_eq!(profile.action_frames.attack_dash_total_frames, 39);
    assert_eq!(profile.action_frames.attack_dash_iasa_frame, 38);
    assert_eq!(profile.action_frames.attack_air_n_landing_lag_set_frame, 4);
    assert_eq!(
        profile.action_frames.attack_air_n_landing_lag_clear_frame,
        34
    );
    assert_eq!(profile.action_frames.attack_air_f_landing_lag_set_frame, 7);
    assert_eq!(
        profile.action_frames.attack_air_f_landing_lag_clear_frame,
        35
    );
    assert_eq!(profile.action_frames.attack_air_b_landing_lag_set_frame, 7);
    assert_eq!(
        profile.action_frames.attack_air_b_landing_lag_clear_frame,
        21
    );
    assert_eq!(profile.action_frames.attack_air_hi_landing_lag_set_frame, 0);
    assert_eq!(
        profile.action_frames.attack_air_hi_landing_lag_clear_frame,
        22
    );
    assert_eq!(profile.action_frames.attack_air_lw_landing_lag_set_frame, 4);
    assert_eq!(
        profile.action_frames.attack_air_lw_landing_lag_clear_frame,
        36
    );
    assert_eq!(profile.dash_initial_velocity.to_bits(), 2.0_f32.to_bits());
    assert_eq!(
        profile.dash_run_terminal_velocity.to_bits(),
        2.299999952316284_f32.to_bits()
    );
    assert_eq!(
        profile.ground_friction.to_bits(),
        0.07999999821186066_f32.to_bits()
    );
    assert_eq!(profile.max_jumps, 2);
    assert_eq!(profile.fast_fall_velocity.to_bits(), 3.5_f32.to_bits());
    assert_eq!(profile.full_hop_height, 38_520);
    assert_eq!(profile.short_hop_height, 14_850);
    assert_eq!(profile.double_jump_height, 28_560);
    assert_eq!(
        profile.entry_platform,
        FighterEntryPlatformProfile::COMMON_TROPHY_PLATFORM
    );
    assert_eq!(
        profile.entry_platform.source_model_symbol,
        "Fighter_804D6514"
    );
    assert_eq!(
        profile.entry_platform.decomp_ref,
        ".research/doldecomp-melee/src/melee/ft/ft_0C31.c::ftCo_800C6408"
    );
    assert_eq!(
        profile.entry_platform.vertical_offset_ratio.to_bits(),
        1.497345_f32.to_bits()
    );
    assert_eq!(profile.entry_platform.accessory.symbol, "Fighter_804D6514");
    assert_eq!(profile.entry_platform.accessory.source_dat, "PlCo.dat");
    assert_eq!(
        profile.entry_platform.accessory.root_symbol,
        "ftLoadCommonData"
    );
    assert_eq!(profile.entry_platform.accessory.pointer_table_slot, 16);
    assert_eq!(
        profile.entry_platform.accessory.joint_root_data_offset,
        0x15528
    );
    assert_eq!(profile.entry_platform.accessory.joint_count, 1);
    assert_eq!(
        profile.entry_platform.accessory.mesh_bounds.min.x.to_bits(),
        (-6.96875_f32).to_bits()
    );
    assert_eq!(
        profile.entry_platform.accessory.mesh_bounds.max.x.to_bits(),
        6.96875_f32.to_bits()
    );
    assert_eq!(profile.entry_platform_offset_y, 1_647);
    assert_eq!(profile.standing_height_units, 22_667);
    assert_eq!(profile.jumpsquat_frames, 4);
    assert_eq!(profile.dash_frames, 15);
    assert_eq!(profile.max_run_brake_frames, Some(30));
    assert_eq!(profile.weight.to_bits(), 104.0_f32.to_bits());
    assert_eq!(profile.standing_turn_direction_change_frames, 6);
    assert_eq!(profile.standing_turn_total_frames, 11);
    assert_eq!(profile.normal_landing_lag_ticks, 4);
    assert_eq!(profile.landing_air_n_lag_ticks, 15);
    assert_eq!(profile.landing_air_f_lag_ticks, 19);
    assert_eq!(profile.landing_air_b_lag_ticks, 18);
    assert_eq!(profile.landing_air_hi_lag_ticks, 15);
    assert_eq!(profile.landing_air_lw_lag_ticks, 24);
}

#[test]
fn rust_motion_states_all_have_source_ecb_pose_data() {
    const KNOWN_LEDGE_POSE_DATA_GAPS: &[&str] = &[
        "CliffClimbSlow",
        "CliffClimbQuick",
        "CliffAttackSlow",
        "CliffAttackQuick",
        "CliffEscapeSlow",
        "CliffEscapeQuick",
        "CliffJumpSlow1",
        "CliffJumpSlow2",
        "CliffJumpQuick1",
        "CliffJumpQuick2",
    ];
    let mut missing = Vec::new();
    for source_key in mole_core::RUST_MOTION_STATE_VARIANTS {
        let Some(runtime_variant) = runtime_motion_state_for_source_key(source_key) else {
            let motion_state = motion_state_for_runtime_variant(source_key)
                .unwrap_or_else(|| panic!("{source_key} should parse as MotionState"));
            assert!(
                mole_core::is_source_dead_motion_state(motion_state)
                    || mole_core::is_source_rebirth_motion_state(motion_state)
                    || motion_state == MotionState::Sleep,
                "{source_key} should either resolve to sampled source ECB or be a documented source common lifecycle state"
            );
            continue;
        };
        let motion_state = motion_state_for_runtime_variant(runtime_variant)
            .unwrap_or_else(|| panic!("{runtime_variant} should parse as MotionState"));
        if !has_source_ecb_pose_data_for_motion_state(motion_state) {
            missing.push(*source_key);
        }
    }
    assert!(
        missing == KNOWN_LEDGE_POSE_DATA_GAPS,
        "unexpected generated ftData.x44 + live JObj ECB coverage gap set: {missing:?}"
    );
}

#[test]
fn falcon_like_profile_keeps_ftco_dat_attr_floats_as_source_f32() {
    fn assert_source_f32(value: f32, expected: f32) {
        assert_eq!(value.to_bits(), expected.to_bits());
    }

    let profile = FighterProfile::falcon_like();

    assert_source_f32(profile.walk_initial_velocity, 0.15000000596046448);
    assert_source_f32(profile.walk_accel, 0.10000000149011612);
    assert_source_f32(profile.walk_max_velocity, 0.8500000238418579);
    assert_source_f32(profile.slow_walk_max_velocity, 0.16500000655651093);
    assert_source_f32(profile.mid_walk_point, 0.40700000524520874);
    assert_source_f32(profile.fast_walk_min, 0.659600019454956);
    assert_source_f32(profile.ground_friction, 0.07999999821186066);
    assert_source_f32(profile.dash_initial_velocity, 2.0);
    assert_source_f32(profile.run_animation_scaling, 2.3299999237060547);
    assert_source_f32(profile.ground_max_horizontal_velocity, 3.0);
    assert_source_f32(profile.jump_horizontal_initial_velocity, 0.949999988079071);
    assert_source_f32(profile.jump_vertical_initial_velocity, 3.0999999046325684);
    assert_source_f32(profile.ground_to_air_jump_momentum_multiplier, 0.75);
    assert_source_f32(profile.jump_horizontal_max_velocity, 2.0999999046325684);
    assert_source_f32(profile.hop_vertical_initial_velocity, 1.899999976158142);
    assert_source_f32(profile.air_jump_vertical_multiplier, 0.8999999761581421);
    assert_source_f32(profile.air_jump_horizontal_multiplier, 0.8999999761581421);
    assert_source_f32(profile.gravity, 0.12999999523162842);
    assert_source_f32(profile.terminal_velocity, 2.9000000953674316);
    assert_source_f32(profile.air_drift_stick_multiplier, 0.03999999910593033);
    assert_source_f32(profile.aerial_drift_base, 0.019999999552965164);
    assert_source_f32(profile.air_drift_max, 1.1200000047683716);
    assert_source_f32(profile.aerial_friction, 0.009999999776482582);
    assert_source_f32(profile.fast_fall_velocity, 3.5);
    assert_source_f32(profile.air_max_horizontal_velocity, 3.0);
    assert_source_f32(profile.initial_shield_size, 15.0);
    assert_source_f32(profile.shield_break_initial_velocity, 2.700000047683716);
    assert_source_f32(profile.weight, 104.0);
    assert_eq!(profile.weight_independent_throws_mask, 0x07);
    assert_source_f32(profile.player_nudge_body_center_x, 0.0);
    assert_source_f32(profile.player_nudge_body_half_width, 3.5);
}

#[test]
fn extracted_ftco_dat_attrs_reads_big_endian_fighter_profile_fields() {
    let mut bytes = vec![0_u8; 0x188];

    put_f32_be(&mut bytes, 0x00, 0.151);
    put_f32_be(&mut bytes, 0x04, 0.02);
    put_f32_be(&mut bytes, 0x08, 0.85);
    put_f32_be(&mut bytes, 0x0c, 0.166);
    put_f32_be(&mut bytes, 0x10, 0.407);
    put_f32_be(&mut bytes, 0x14, 0.660);
    put_f32_be(&mut bytes, 0x18, 0.08);
    put_f32_be(&mut bytes, 0x1c, 2.0);
    put_f32_be(&mut bytes, 0x20, 0.011);
    put_f32_be(&mut bytes, 0x24, 0.151);
    put_f32_be(&mut bytes, 0x28, 2.3);
    put_f32_be(&mut bytes, 0x2c, 2.331);
    put_f32_be(&mut bytes, 0x30, 8.0);
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
    put_f32_be(&mut bytes, 0x84, 7.0);
    put_f32_be(&mut bytes, 0x88, 104.0);
    bytes[0x180] = 0x07;
    put_f32_be(&mut bytes, 0x8c, 0.97);
    put_f32_be(&mut bytes, 0x90, 15.0);
    put_f32_be(&mut bytes, 0x94, 2.6);
    put_f32_be(&mut bytes, 0xe4, 5.0);
    put_f32_be(&mut bytes, 0xe8, 11.0);
    put_f32_be(&mut bytes, 0xec, 12.0);
    put_f32_be(&mut bytes, 0xf0, 13.0);
    put_f32_be(&mut bytes, 0xf4, 14.0);
    put_f32_be(&mut bytes, 0xf8, 15.0);
    put_f32_be(&mut bytes, 0x110, 1.1);

    let profile = FighterProfile::from_ftco_dat_attrs_bytes("captain_falcon", &bytes)
        .expect("synthetic ftCo_DatAttrs slice should extract");
    let assert_source_f32 = |actual: f32, expected: f32| {
        assert_eq!(actual.to_bits(), expected.to_bits());
    };

    assert_eq!(profile.reference_character, "captain_falcon");
    assert_source_f32(profile.walk_initial_velocity, 0.151);
    assert_source_f32(profile.walk_accel, 0.02);
    assert_source_f32(profile.walk_max_velocity, 0.85);
    assert_source_f32(profile.slow_walk_max_velocity, 0.166);
    assert_source_f32(profile.mid_walk_point, 0.407);
    assert_source_f32(profile.fast_walk_min, 0.660);
    assert_source_f32(profile.ground_friction, 0.08);
    assert_source_f32(profile.dash_initial_velocity, 2.0);
    assert_source_f32(profile.dash_run_acceleration_a, 0.011);
    assert_source_f32(profile.dash_run_acceleration_b, 0.151);
    assert_source_f32(profile.dash_run_terminal_velocity, 2.3);
    assert_source_f32(profile.run_animation_scaling, 2.331);
    assert_eq!(profile.max_run_brake_frames, Some(8));
    assert_source_f32(profile.ground_max_horizontal_velocity, 2.35);
    assert_eq!(profile.jumpsquat_frames, 4);
    assert_source_f32(profile.jump_horizontal_initial_velocity, 0.44);
    assert_source_f32(profile.ground_to_air_jump_momentum_multiplier, 0.81);
    assert_source_f32(profile.jump_horizontal_max_velocity, 1.05);
    assert_source_f32(profile.jump_vertical_initial_velocity, 3.1);
    assert_source_f32(profile.hop_vertical_initial_velocity, 1.9);
    assert_source_f32(profile.air_jump_vertical_multiplier, 0.9);
    assert_source_f32(profile.air_jump_horizontal_multiplier, 0.46);
    assert_eq!(profile.max_jumps, 2);
    assert_source_f32(profile.gravity, 0.13);
    assert_source_f32(profile.terminal_velocity, 2.9);
    assert_source_f32(profile.air_drift_stick_multiplier, 0.047);
    assert_source_f32(profile.aerial_drift_base, 0.023);
    assert_source_f32(profile.air_drift_max, 1.17);
    assert_source_f32(profile.aerial_friction, 0.011);
    assert_source_f32(profile.fast_fall_velocity, 3.5);
    assert_source_f32(profile.air_max_horizontal_velocity, 1.26);
    assert_source_f32(profile.weight, 104.0);
    assert_eq!(profile.weight_independent_throws_mask, 0x07);
    assert_source_f32(profile.model_scaling, 0.97);
    assert_source_f32(profile.initial_shield_size, 15.0);
    assert_source_f32(profile.shield_break_initial_velocity, 2.6);
    assert_eq!(profile.standing_turn_direction_change_frames, 7);
    assert_eq!(profile.standing_turn_total_frames, 11);
    assert_eq!(profile.entry_platform_offset_y, 1_647);
    assert_eq!(profile.normal_landing_lag_ticks, 5);
    assert_eq!(profile.landing_air_n_lag_ticks, 11);
    assert_eq!(profile.landing_air_f_lag_ticks, 12);
    assert_eq!(profile.landing_air_b_lag_ticks, 13);
    assert_eq!(profile.landing_air_hi_lag_ticks, 14);
    assert_eq!(profile.landing_air_lw_lag_ticks, 15);
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
fn extracted_ftcaptain_dat_attrs_keep_native_special_values() {
    let mut bytes = vec![0_u8; 0x8c];
    put_f32_be(&mut bytes, 0x00, 0.125);
    put_f32_be(&mut bytes, 0x04, 0.625);
    put_f32_be(&mut bytes, 0x08, 30.0);
    put_f32_be(&mut bytes, 0x0c, 1.95);
    put_f32_be(&mut bytes, 0x10, 0.92);
    put_f32_be(&mut bytes, 0x40, 1.1);
    put_f32_be(&mut bytes, 0x44, 0.85);
    put_f32_be(&mut bytes, 0x48, 0.72);
    put_f32_be(&mut bytes, 0x4c, 30.0);
    put_f32_be(&mut bytes, 0x58, 0.225);
    put_i32_be(&mut bytes, 0x64, 18);
    put_u32_be(&mut bytes, 0x6c, 4);
    put_f32_be(&mut bytes, 0x88, 3.0);

    let attrs = mole_core::CaptainSpecialAttrs::from_ftcaptain_dat_attrs_bytes(&bytes)
        .expect("synthetic ftCaptain_DatAttrs slice should extract");

    assert_eq!(
        attrs.specialn_stick_range_y_neg.to_bits(),
        0.125_f32.to_bits()
    );
    assert_eq!(
        attrs.specialn_stick_range_y_pos.to_bits(),
        0.625_f32.to_bits()
    );
    assert_eq!(attrs.specialn_angle_diff.to_bits(), 30.0_f32.to_bits());
    assert_eq!(attrs.specialn_vel_x.to_bits(), 1.95_f32.to_bits());
    assert_eq!(attrs.specialn_vel_mul.to_bits(), 0.92_f32.to_bits());
    assert_eq!(
        attrs.specialhi_air_friction_mul.to_bits(),
        1.1_f32.to_bits()
    );
    assert_eq!(attrs.specialhi_horz_vel.to_bits(), 0.85_f32.to_bits());
    assert_eq!(
        attrs.specialhi_freefall_air_spd_mul.to_bits(),
        0.72_f32.to_bits()
    );
    assert_eq!(attrs.specialhi_landing_lag.to_bits(), 30.0_f32.to_bits());
    assert_eq!(attrs.specialhi_input_var.to_bits(), 0.225_f32.to_bits());
    assert_eq!(attrs.specialhi_air_var, 18);
    assert_eq!(attrs.speciallw_unk1, 4);
    assert_eq!(
        attrs.speciallw_air_landing_traction.to_bits(),
        3.0_f32.to_bits()
    );
}

#[test]
fn extracted_profile_carries_ftdata_x50_player_nudge_body_box() {
    let profile = FighterProfile::from_ftdata_x50_player_nudge_body_box(
        FighterProfile::falcon_like(),
        0.25,
        4.75,
    );

    assert_eq!(
        profile.player_nudge_body_center_x.to_bits(),
        0.25_f32.to_bits()
    );
    assert_eq!(
        profile.player_nudge_body_half_width.to_bits(),
        4.75_f32.to_bits()
    );
}

#[test]
fn grounded_fighters_compute_player_nudge_before_ground_velocity_like_ftcommon_8007e0e4() {
    let mut world = World::for_two_players();
    let mut left = PlayerState::new(-1_000, 0, 1);
    left.set_motion_state_alias(MotionState::Wait);
    left.grounded = true;

    let mut right = PlayerState::new(1_000, 0, -1);
    right.set_motion_state_alias(MotionState::Wait);
    right.grounded = true;

    assert!(
        milli_to_source_units(right.position.x - left.position.x)
            < left.profile.player_nudge_body_half_width
                + right.profile.player_nudge_body_half_width
    );

    assert!(world.set_player_state_for_diagnostic(0, left));
    assert!(world.set_player_state_for_diagnostic(1, right));
    step_world(
        &mut world,
        Frame(1),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    let players = world.players();
    assert_eq!(
        players[0].player_nudge_x.to_bits(),
        (-MeleeCommonData::provisional_mole().player_nudge_x).to_bits(),
        "ftCommon_8007DD7C gives the left overlapping fighter negative xF8_playerNudgeVel.x"
    );
    assert_eq!(
        players[1].player_nudge_x.to_bits(),
        MeleeCommonData::provisional_mole().player_nudge_x.to_bits(),
        "ftCommon_8007DD7C gives the right overlapping fighter positive xF8_playerNudgeVel.x"
    );
    assert_eq!(
        players[0].position.x,
        -1_000 - source_units_to_milli(MeleeCommonData::provisional_mole().player_nudge_x)
    );
    assert_eq!(
        players[1].position.x,
        1_000 + source_units_to_milli(MeleeCommonData::provisional_mole().player_nudge_x)
    );
    assert_eq!(players[0].velocity.x, 0);
    assert_eq!(players[1].velocity.x, 0);
}

#[test]
fn player_nudge_uses_source_floor_line_next_prev_like_ftcommon_8007dd7c() {
    let mut world = World::for_two_players();
    let common = MeleeCommonData::provisional_mole();

    let mut left = PlayerState::new(-62_000, 1, 1);
    left.set_motion_state_alias(MotionState::Wait);
    left.grounded = true;
    left.set_source_floor_for_diagnostic(Some(0), Some(0));

    let mut right = PlayerState::new(-60_000, 1, -1);
    right.set_motion_state_alias(MotionState::Wait);
    right.grounded = true;
    right.set_source_floor_for_diagnostic(Some(0), Some(1));

    assert!(world.set_player_state_for_diagnostic(0, left));
    assert!(world.set_player_state_for_diagnostic(1, right));
    step_world(
        &mut world,
        Frame(1),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    let players = world.players();
    assert_eq!(
        players[0].player_nudge_x.to_bits(),
        (-common.player_nudge_x).to_bits(),
        "ftCommon_8007DD7C accepts cur_gnd == mpLineGetNext(arg_gnd)"
    );
    assert_eq!(
        players[1].player_nudge_x.to_bits(),
        common.player_nudge_x.to_bits(),
        "ftCommon_8007DD7C accepts cur_gnd == mpLineGetPrev(arg_gnd)"
    );
}

#[test]
fn player_nudge_applies_source_depth_like_fighter_cur_pos_z() {
    let mut world = World::for_two_players();
    let common = MeleeCommonData::provisional_mole();

    let mut left = PlayerState::new(-1_000, 0, 1);
    left.set_motion_state_alias(MotionState::Wait);
    left.grounded = true;

    let mut right = PlayerState::new(1_000, 0, -1);
    right.set_motion_state_alias(MotionState::Wait);
    right.grounded = true;

    assert!(world.set_player_state_for_diagnostic(0, left));
    assert!(world.set_player_state_for_diagnostic(1, right));

    step_world(
        &mut world,
        Frame(1),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );
    assert_eq!(
        world.players()[0].source_position_z.to_bits(),
        (-common.player_nudge_z).to_bits(),
        "fighter.c applies xF8_playerNudgeVel.y to cur_pos.z"
    );
    assert_eq!(
        world.players()[1].source_position_z.to_bits(),
        common.player_nudge_z.to_bits(),
        "fighter.c applies xF8_playerNudgeVel.y to cur_pos.z"
    );

    step_world(
        &mut world,
        Frame(2),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );
    assert_eq!(
        world.players()[0].source_position_z.to_bits(),
        (-common.player_nudge_z * 2.0).to_bits(),
        "ftCommon_8007F8B4 feeds the updated source cur_pos.z into the next body-push frame"
    );
    assert_eq!(
        world.players()[1].source_position_z.to_bits(),
        (common.player_nudge_z * 2.0).to_bits(),
        "ftCommon_8007F8B4 feeds the updated source cur_pos.z into the next body-push frame"
    );
}

#[test]
fn player_nudge_sees_same_frame_jump_squat_exit_before_push_velocity() {
    let profile = FighterProfile::falcon_like();
    let mut world = World::for_two_players_with_profiles([profile; 2]);

    let mut jumper = PlayerState::new_with_profile(4_761, 0, 1, profile);
    jumper.motion_state = MotionState::KneeBend;
    jumper.motion_frame = profile.jumpsquat_frames - 1;
    jumper.motion_anim_frame_milli = i32::from(jumper.motion_frame) * 1_000;
    jumper.grounded = true;
    assert!(world.set_player_state_for_diagnostic(0, jumper));

    let mut runner = PlayerState::new_with_profile(11_264, 0, -1, profile);
    runner.motion_state = MotionState::Dash;
    runner.motion_frame = 14;
    runner.motion_anim_frame_milli = 14_000;
    runner.motion_cmd_var0 = 1;
    runner.grounded = true;
    runner.ground_velocity_x = -2.271_250_009_536_743;
    runner.source_self_velocity_x = runner.ground_velocity_x;
    runner.velocity.x = source_units_to_milli(runner.ground_velocity_x);
    assert!(world.set_player_state_for_diagnostic(1, runner));

    let previous_inputs = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral().with_left_stick(-125, 0),
    ];
    world.set_input_history_for_diagnostic(previous_inputs, [MeleeInputTimers::expired(); 2]);

    step_world(
        &mut world,
        Frame(0),
        &[
            PlayerInput::neutral().with_jump(true),
            PlayerInput::neutral().with_left_stick(-125, 0),
        ],
    );

    let players = world.players();
    assert!(!players[0].grounded);
    assert_eq!(players[0].motion_state, MotionState::JumpF);
    assert_eq!(players[1].motion_state, MotionState::Run);
    assert_eq!(
        players[1].player_nudge_x.to_bits(),
        0.0_f32.to_bits(),
        "Fighter_8006A360 runs anim_cb before ftCommon_8007E0E4; once the other fighter exits KneeBend, ftCommon_8007DD7C skips it as airborne"
    );
    assert_eq!(
        players[1].position.x,
        8_993,
        "P2 should move by carried Dash/Run ground velocity only; a stale frame-start player nudge adds PlCo.x450 and lands at 9293"
    );
}

#[test]
fn earlier_player_nudge_sees_later_player_before_same_priority_anim_callback() {
    let profile = FighterProfile::falcon_like();
    let common = MeleeCommonData::provisional_mole();
    let mut world = World::for_two_players_with_profiles([profile; 2]);

    let mut dasher = PlayerState::new_with_profile(11_540, 0, -1, profile);
    dasher.motion_state = MotionState::Dash;
    dasher.motion_frame = 1;
    dasher.motion_anim_frame_milli = 1_000;
    dasher.grounded = true;
    dasher.ground_velocity_x = -2.160_000_085_830_688_5;
    dasher.source_self_velocity_x = dasher.ground_velocity_x;
    dasher.velocity.x = source_units_to_milli(dasher.ground_velocity_x);
    assert!(world.set_player_state_for_diagnostic(0, dasher));

    let mut jumper = PlayerState::new_with_profile(10_521, 0, 1, profile);
    jumper.motion_state = MotionState::KneeBend;
    jumper.motion_frame = profile.jumpsquat_frames - 1;
    jumper.motion_anim_frame_milli = i32::from(jumper.motion_frame) * 1_000;
    jumper.grounded = true;
    assert!(world.set_player_state_for_diagnostic(1, jumper));

    let previous_inputs = [
        PlayerInput::neutral().with_left_stick(-127, 0),
        PlayerInput::neutral().with_jump(true),
    ];
    world.set_input_history_for_diagnostic(previous_inputs, [MeleeInputTimers::expired(); 2]);

    step_world(
        &mut world,
        Frame(0),
        &[
            PlayerInput::neutral().with_left_stick(-127, 0),
            PlayerInput::neutral().with_jump(true),
        ],
    );

    let players = world.players();
    assert_eq!(
        players[0].player_nudge_x.to_bits(),
        common.player_nudge_x.to_bits(),
        "Fighter_8006A360 is a per-fighter priority-1 proc; player 0 computes ftCommon_8007E0E4 before player 1's same-priority KneeBend Anim callback makes it airborne"
    );
    assert!(!players[1].grounded);
}

#[test]
fn grounded_source_position_accumulates_sub_milli_velocity_before_projection() {
    let profile = FighterProfile {
        ground_friction: 0.0,
        ground_max_horizontal_velocity: 3.0,
        ..FighterProfile::falcon_like()
    };
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let mut player = world.players()[0];
    let initial_position_x = player.position.x;
    let source_velocity = 1.0_f32 / 3.0;
    player.ground_velocity_x = source_velocity;
    player.source_self_velocity_x = source_velocity;
    player.velocity.x = source_units_to_milli(source_velocity);
    assert!(world.set_player_state_for_diagnostic(0, player));

    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    for frame in 0..3 {
        step_world(&mut world, Frame(frame), &neutral);
    }

    assert_eq!(
        world.players()[0].position.x,
        initial_position_x + source_units_to_milli(source_velocity * 3.0),
        "Melee carries fighter position as source float state; milli projection must not round each simulation tick"
    );
}

#[test]
fn diagnostic_state_import_preserves_explicit_source_float_position() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.position = Vec2 {
        x: 1_000,
        y: -7_000,
    };
    player.source_position = mole_core::SourceVec2 {
        x: 1.2345,
        y: -6.78925,
    };
    assert!(world.set_player_state_for_diagnostic(0, player));

    let player = world.players()[0];
    assert_eq!(player.source_position.x, 1.2345);
    assert_eq!(player.source_position.y, -6.78925);
    assert_eq!(
        player.position,
        player.source_position.to_milli(),
        "source float position is the canonical diagnostic state; milli position is only its projection"
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
                source_units_to_milli(
                    (milli_to_source_units(before) - profile.gravity)
                        .max(-profile.terminal_velocity),
                )
            );
            return;
        }
    }

    panic!("expected Falcon profile jump to leave the ground");
}

#[test]
fn input_threshold_defaults_match_extracted_plco_common_data() {
    let common = MeleeCommonData::provisional_mole();
    let config = common.input_config();
    let thresholds = common.input_thresholds();

    assert_eq!(MeleeInputConfig::default(), config);
    assert_eq!(MeleeInputThresholds::default(), thresholds);
    assert_eq!(common.trigger_threshold, 1);
    assert_eq!(common.trigger_timer_threshold, 64);
    assert_eq!(config.main_stick_deadzone_x, 36);
    assert_eq!(config.main_stick_deadzone_y, 36);
    assert_eq!(config.c_stick_deadzone_x, 36);
    assert_eq!(config.c_stick_deadzone_y, 36);
    assert_eq!(config.trigger_deadzone, 77);
    assert_eq!(common.tap_x_threshold, 32);
    assert_eq!(common.tap_y_threshold, 32);
    assert_eq!(thresholds.walk_x, 23);
    assert_eq!(common.walk_slow_x, 20);
    assert_eq!(common.walk_middle_x, 50);
    assert_eq!(common.walk_fast_x, 90);
    assert_eq!(thresholds.walk_slow_x, 20);
    assert_eq!(thresholds.walk_middle_x, 50);
    assert_eq!(thresholds.walk_fast_x, 90);
    assert_eq!(thresholds.turn_x, -32);
    assert_eq!(common.turn_run_x, -48);
    assert_eq!(thresholds.dash_x, 102);
    assert_eq!(thresholds.dash_tap_window, 2);
    assert_eq!(common.z_shield_analog, 89);
    assert_eq!(thresholds.z_shield_analog, 89);
    assert_eq!(common.dash_early_action_window, 4);
    assert_eq!(common.dash_defensive_action_window, 3);
    assert_eq!(common.dash_late_action_window, 20);
    assert_eq!(common.dash_velocity_decay.to_bits(), 0.75_f32.to_bits());
    assert_eq!(common.run_x, 79);
    assert_eq!(
        common.run_brake_animation_pause_velocity.to_bits(),
        0.0_f32.to_bits()
    );
    assert_eq!(common.run_turn_run_no_interrupt_frames, 10);
    assert_eq!(common.tap_jump_window, 4);
    assert_eq!(common.tap_jump_release_y, 38);
    assert_eq!(common.fast_fall_y, 84);
    assert_eq!(common.fast_fall_window, 4);
    assert_eq!(common.lcancel_window, 7);
    assert_eq!(common.lcancel_divisor.to_bits(), 2.0_f32.to_bits());
    assert_eq!(thresholds.tap_jump_y, 84);
    assert_eq!(thresholds.escape_x, 89);
    assert_eq!(common.escape_x_tap_window, 4);
    assert_eq!(thresholds.escape_y, -89);
    assert_eq!(common.escape_y_tap_window, 4);
    assert_eq!(thresholds.aerial_neutral_x, 32);
    assert_eq!(thresholds.aerial_neutral_y, 32);
    assert_eq!(thresholds.aerial_vertical_angle_tan_milli, 1192);
    assert_eq!(common.tilt_x, 32);
    assert_eq!(common.tilt_y, 32);
    assert_eq!(common.throw_down_y, -32);
    assert_eq!(common.crouch_y, 87);
    assert_eq!(common.crouch_release_y, 79);
    assert_eq!(common.air_jump_backward_x, 16);
    assert_eq!(common.escapeair_iasa_timer_ticks, 3);
    assert_eq!(common.escapeair_animation_ticks, 50);
    assert_eq!(common.escapeair_deadzone_x, 32);
    assert_eq!(common.escapeair_deadzone_y, 32);
    assert_eq!(
        common.escapeair_force.to_bits(),
        3.0999999046325684_f32.to_bits()
    );
    assert_eq!(
        common.escapeair_decay.to_bits(),
        0.8999999761581421_f32.to_bits()
    );
    assert_eq!(common.escapeair_landing_lag_ticks, 10);
    assert_eq!(
        common.walk_middle_velocity_ratio.to_bits(),
        0.4000000059604645_f32.to_bits()
    );
    assert_eq!(
        common.walk_fast_velocity_ratio.to_bits(),
        0.800000011920929_f32.to_bits()
    );
    assert_eq!(common.walk_accel_taper.to_bits(), 0.5_f32.to_bits());
    assert_eq!(
        common.run_accel_taper.to_bits(),
        0.4000000059604645_f32.to_bits()
    );
    assert_eq!(
        common.run_ground_friction_multiplier.to_bits(),
        1.0_f32.to_bits()
    );
    assert_eq!(
        common.catch_ground_friction_multiplier.to_bits(),
        1.0_f32.to_bits()
    );
    assert_eq!(
        common.high_speed_ground_friction_multiplier.to_bits(),
        2.0_f32.to_bits()
    );
    assert_eq!(
        common.animation_velocity_scale.to_bits(),
        1.2999999523162842_f32.to_bits()
    );
    assert_eq!(
        common.fall_animation_drift_threshold.to_bits(),
        0.10000000149011612_f32.to_bits()
    );
    assert_eq!(
        common.air_speed_clamp_friction.to_bits(),
        0.029999999329447746_f32.to_bits()
    );
    assert_eq!(
        common.special_air_drift_stick_threshold.to_bits(),
        0.10000000149011612_f32.to_bits()
    );
    assert_eq!(common.fall_animation_blend.to_bits(), 0.5_f32.to_bits());
    assert_eq!(
        common.landing_wait_y_velocity_threshold.to_bits(),
        1.0_f32.to_bits()
    );
    assert_eq!(common.fallspecial_platform_landing_y, -71);
    assert_eq!(common.platform_pass_y, 84);
    assert_eq!(common.platform_pass_y_tap_window, 6);
    assert_eq!(
        common.pass_initial_y_velocity.to_bits(),
        (-0.5_f32).to_bits()
    );
    assert_eq!(common.platform_drop_delay_ticks, 2);
    assert_eq!(common.rebirth_ticks, 60);
    assert_eq!(common.rebirth_wait_ticks, 240);
    assert_eq!(common.rebirth_hurt_intangible_ticks, 120);
    assert_eq!(common.top_blast_fall_ko_chance, 16);
    assert_eq!(common.dead_wait_ticks, 60);
    assert_eq!(common.dead_up_star_wait_ticks, 1);
    assert_eq!(common.dead_up_star_rise_ticks, 130);
    assert_eq!(common.dead_up_star_exit_ticks, 45);
    assert_eq!(common.dead_up_fall_wait_ticks, 1);
    assert_eq!(common.dead_up_fall_anim_ticks, 50);
    assert_eq!(common.dead_up_fall_hit_camera_ticks, 3);
    assert_eq!(common.dead_up_fall_drift_ticks, 40);
    assert_eq!(common.dead_up_fall_exit_ticks, 35);
    assert_eq!(common.entry_start_ticks, 30);
    assert_eq!(common.entry_end_ticks, 30);
    assert_eq!(
        common.entry_initial_scale_y.to_bits(),
        0.009999999776482582_f32.to_bits()
    );
    assert_eq!(common.entry_collision_landing_lag_ticks, 120);
    assert_eq!(common.guard_on_catch_dash_window, 3);
    assert_eq!(common.run_turn_run_no_interrupt_frames, 10);
    assert_eq!(common.passive_input_age_threshold, 40);
    assert_eq!(common.passive_window_max.to_bits(), 20.0_f32.to_bits());
    assert_eq!(
        common.passive_stand_stick_x.to_bits(),
        0.20000000298023224_f32.to_bits()
    );
    assert_eq!(common.down_stand_stick_y, 25);
    assert_eq!(common.down_wait_timer.to_bits(), 220.0_f32.to_bits());
    assert_eq!(
        common.player_nudge_x.to_bits(),
        0.30000001192092896_f32.to_bits()
    );
    assert_eq!(
        common.player_nudge_z.to_bits(),
        0.10000000149011612_f32.to_bits()
    );
    assert_eq!(
        common.player_nudge_z_clamp.to_bits(),
        1.399999976158142_f32.to_bits()
    );
    assert_eq!(
        common.transformed_player_nudge_z.to_bits(),
        0.20000000298023224_f32.to_bits()
    );
    assert_eq!(
        common.transformed_player_nudge_z_clamp.to_bits(),
        3.799999952316284_f32.to_bits()
    );
    assert_eq!(common.shield_start_health.to_bits(), 60.0_f32.to_bits());
    assert_eq!(common.shield_release_lockout_frames, 8);
    assert_eq!(
        common.shield_hold_drain.to_bits(),
        0.14000000059604645_f32.to_bits()
    );
    assert_eq!(
        common.shield_regen.to_bits(),
        0.07000000029802322_f32.to_bits()
    );
    assert_eq!(
        common.shield_break_reset_health.to_bits(),
        30.0_f32.to_bits()
    );
    assert_eq!(
        common.shield_hold_lightshield_min.to_bits(),
        0.10000000149011612_f32.to_bits()
    );
    assert_eq!(
        common.shield_hold_lightshield_max.to_bits(),
        2.0_f32.to_bits()
    );
    assert_eq!(
        common.shield_break_furafura_percent_base.to_bits(),
        400.0_f32.to_bits()
    );
    assert_eq!(
        common.shield_break_furafura_timer_base.to_bits(),
        90.0_f32.to_bits()
    );
    assert_eq!(
        common.shield_break_furafura_timer_decrement.to_bits(),
        1.0_f32.to_bits()
    );
    assert_eq!(
        common.shield_break_furafura_mash_decrement.to_bits(),
        3.0_f32.to_bits()
    );
}

#[test]
fn grounded_common_scalars_remain_source_f32_not_milli_aliases() {
    let common = MeleeCommonData::provisional_mole();

    assert_eq!(
        common.walk_middle_velocity_ratio.to_bits(),
        0.4000000059604645_f32.to_bits()
    );
    assert_eq!(
        common.walk_fast_velocity_ratio.to_bits(),
        0.800000011920929_f32.to_bits()
    );
    assert_eq!(common.walk_accel_taper.to_bits(), 0.5_f32.to_bits());
    assert_eq!(
        common.run_accel_taper.to_bits(),
        0.4000000059604645_f32.to_bits()
    );
    assert_eq!(
        common.run_ground_friction_multiplier.to_bits(),
        1.0_f32.to_bits()
    );
    assert_eq!(
        common.high_speed_ground_friction_multiplier.to_bits(),
        2.0_f32.to_bits()
    );
    assert_eq!(
        common.run_brake_animation_pause_velocity.to_bits(),
        0.0_f32.to_bits()
    );
    assert_eq!(
        common.animation_velocity_scale.to_bits(),
        1.2999999523162842_f32.to_bits()
    );
    assert_eq!(common.dash_velocity_decay.to_bits(), 0.75_f32.to_bits());
    assert_eq!(
        common.escapeair_force.to_bits(),
        3.0999999046325684_f32.to_bits()
    );
    assert_eq!(
        common.escapeair_decay.to_bits(),
        0.8999999761581421_f32.to_bits()
    );
    assert_eq!(
        common.fall_animation_drift_threshold.to_bits(),
        0.10000000149011612_f32.to_bits()
    );
    assert_eq!(
        common.air_speed_clamp_friction.to_bits(),
        0.029999999329447746_f32.to_bits()
    );
    assert_eq!(
        common.special_air_drift_stick_threshold.to_bits(),
        0.10000000149011612_f32.to_bits()
    );
    assert_eq!(common.fall_animation_blend.to_bits(), 0.5_f32.to_bits());
    assert_eq!(
        common.landing_wait_y_velocity_threshold.to_bits(),
        1.0_f32.to_bits()
    );
    assert_eq!(
        common.pass_initial_y_velocity.to_bits(),
        (-0.5_f32).to_bits()
    );
    assert_eq!(
        common
            .damage_landing_down_bound_knockback_threshold
            .to_bits(),
        5.0_f32.to_bits()
    );
    assert_eq!(
        common.damage_landing_basic_knockback_threshold.to_bits(),
        0.5_f32.to_bits()
    );
    assert_eq!(
        common.entry_initial_scale_y.to_bits(),
        0.009999999776482582_f32.to_bits()
    );
    assert_eq!(common.passive_window_max.to_bits(), 20.0_f32.to_bits());
    assert_eq!(
        common.passive_stand_stick_x.to_bits(),
        0.20000000298023224_f32.to_bits()
    );
}

#[test]
fn falcon_grounded_action_frames_include_turn_run_and_run_brake_durations() {
    let frames = FighterActionFrames::falcon_like();

    assert_eq!(frames.turn_run_total_frames, 22);
    assert_eq!(frames.run_brake_total_frames, 28);
}

#[test]
fn input_common_data_sources_track_melee_field_offsets() {
    let sources = input_common_data_field_sources();

    let main_stick_deadzone_x = sources
        .iter()
        .find(|source| source.rust_name == "main_stick_deadzone_x")
        .expect("main stick X deadzone source should be recorded");
    assert_eq!(main_stick_deadzone_x.source_name, "x0");
    assert_eq!(main_stick_deadzone_x.offset, 0x00);
    assert_eq!(
        main_stick_deadzone_x.provenance,
        CommonDataProvenance::ExtractedPlCo
    );

    let main_stick_deadzone_y = sources
        .iter()
        .find(|source| source.rust_name == "main_stick_deadzone_y")
        .expect("main stick Y deadzone source should be recorded");
    assert_eq!(main_stick_deadzone_y.source_name, "x4");
    assert_eq!(main_stick_deadzone_y.offset, 0x04);
    assert_eq!(
        main_stick_deadzone_y.provenance,
        CommonDataProvenance::ExtractedPlCo
    );

    let dash = sources
        .iter()
        .find(|source| source.rust_name == "dash_x")
        .expect("dash_x common-data source should be recorded");
    assert_eq!(dash.source_name, "x3C");
    assert_eq!(dash.offset, 0x3c);
    assert_eq!(dash.provenance, CommonDataProvenance::ExtractedPlCo);

    let throw_down = sources
        .iter()
        .find(|source| source.rust_name == "throw_down_y")
        .expect("throw_down_y common-data source should be recorded");
    assert_eq!(throw_down.source_name, "xB0");
    assert_eq!(throw_down.offset, 0xb0);
    assert_eq!(throw_down.provenance, CommonDataProvenance::ExtractedPlCo);

    let walk_slow = sources
        .iter()
        .find(|source| source.rust_name == "walk_slow_x")
        .expect("walk_slow_x common-data source should be recorded");
    assert_eq!(walk_slow.source_name, "provisional_walk_slow_x");
    assert_eq!(walk_slow.offset, 0);
    assert_eq!(walk_slow.provenance, CommonDataProvenance::ProvisionalMole);

    let walk_middle = sources
        .iter()
        .find(|source| source.rust_name == "walk_middle_x")
        .expect("walk_middle_x common-data source should be recorded");
    assert_eq!(walk_middle.source_name, "provisional_walk_middle_x");
    assert_eq!(walk_middle.offset, 0);
    assert_eq!(
        walk_middle.provenance,
        CommonDataProvenance::ProvisionalMole
    );

    let walk_fast = sources
        .iter()
        .find(|source| source.rust_name == "walk_fast_x")
        .expect("walk_fast_x common-data source should be recorded");
    assert_eq!(walk_fast.source_name, "provisional_walk_fast_x");
    assert_eq!(walk_fast.offset, 0);
    assert_eq!(walk_fast.provenance, CommonDataProvenance::ProvisionalMole);

    let walk_middle_velocity_ratio = sources
        .iter()
        .find(|source| source.rust_name == "walk_middle_velocity_ratio")
        .expect("walk middle velocity ratio source should be recorded");
    assert_eq!(walk_middle_velocity_ratio.source_name, "x28");
    assert_eq!(walk_middle_velocity_ratio.offset, 0x28);

    let walk_fast_velocity_ratio = sources
        .iter()
        .find(|source| source.rust_name == "walk_fast_velocity_ratio")
        .expect("walk fast velocity ratio source should be recorded");
    assert_eq!(walk_fast_velocity_ratio.source_name, "x2C");
    assert_eq!(walk_fast_velocity_ratio.offset, 0x2c);

    let walk_accel_taper = sources
        .iter()
        .find(|source| source.rust_name == "walk_accel_taper")
        .expect("walk acceleration taper source should be recorded");
    assert_eq!(walk_accel_taper.source_name, "x30");
    assert_eq!(walk_accel_taper.offset, 0x30);

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

    for (rust_name, source_name, offset) in [
        ("shield_start_health", "x260_startShieldHealth", 0x260),
        ("shield_release_lockout_frames", "x268", 0x268),
        ("shield_hold_drain", "x278", 0x278),
        ("shield_regen", "x27C", 0x27c),
        ("shield_break_reset_health", "x280_unkShieldHealth", 0x280),
        ("shield_hit_drain_damage_scale", "x284", 0x284),
        ("shield_hit_drain_base", "x288", 0x288),
        ("shield_setoff_duration_damage_scale", "x28C", 0x28c),
        ("shield_setoff_duration_base", "x290", 0x290),
        ("shield_setoff_pushback_scale", "x294", 0x294),
        ("shield_setoff_pushback_cap", "x298", 0x298),
        (
            "shield_setoff_nonreflect_pushback_multiplier",
            "x2BC",
            0x2bc,
        ),
        ("shield_hit_lightshield_min", "x2DC", 0x2dc),
        ("shield_hit_lightshield_max", "x2E0", 0x2e0),
        ("shield_setoff_lightshield_min", "x2E4", 0x2e4),
        ("shield_setoff_lightshield_max", "x2E8", 0x2e8),
        ("shield_hold_lightshield_min", "x2EC", 0x2ec),
        ("shield_hold_lightshield_max", "x2F0", 0x2f0),
        ("shield_break_furafura_percent_base", "x2F8", 0x2f8),
        ("shield_break_furafura_timer_base", "x2FC", 0x2fc),
        ("shield_break_furafura_timer_decrement", "x300", 0x300),
        ("shield_break_furafura_mash_decrement", "x304", 0x304),
        ("shield_aim_smoothing", "x44C", 0x44c),
    ] {
        let source = sources
            .iter()
            .find(|source| source.rust_name == rust_name)
            .expect("shield common-data source should be recorded");
        assert_eq!(source.source_name, source_name);
        assert_eq!(source.offset, offset);
        assert_eq!(source.provenance, CommonDataProvenance::ExtractedPlCo);
    }

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

    let lcancel_window = sources
        .iter()
        .find(|source| source.rust_name == "lcancel_window")
        .expect("L-cancel window common-data source should be recorded");
    assert_eq!(lcancel_window.source_name, "xE4");
    assert_eq!(lcancel_window.offset, 0xe4);

    let lcancel_divisor = sources
        .iter()
        .find(|source| source.rust_name == "lcancel_divisor")
        .expect("L-cancel divisor common-data source should be recorded");
    assert_eq!(lcancel_divisor.source_name, "xE8");
    assert_eq!(lcancel_divisor.offset, 0xe8);

    let air_jump_backward_x = sources
        .iter()
        .find(|source| source.rust_name == "air_jump_backward_x")
        .expect("air_jump_backward_x common-data source should be recorded");
    assert_eq!(air_jump_backward_x.source_name, "x78");
    assert_eq!(air_jump_backward_x.offset, 0x78);

    let crouch_release_y = sources
        .iter()
        .find(|source| source.rust_name == "crouch_release_y")
        .expect("crouch_release_y common-data source should be recorded");
    assert_eq!(crouch_release_y.source_name, "x94");
    assert_eq!(crouch_release_y.offset, 0x94);

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
        .find(|source| source.rust_name == "escapeair_decay")
        .expect("escapeair_decay common-data source should be recorded");
    assert_eq!(escapeair_decay.source_name, "escapeair_decay");
    assert_eq!(escapeair_decay.offset, 0x33c);

    let landing_wait_y_velocity_threshold = sources
        .iter()
        .find(|source| source.rust_name == "landing_wait_y_velocity_threshold")
        .expect("ordinary airborne landing callback threshold should be recorded");
    assert_eq!(landing_wait_y_velocity_threshold.source_name, "x310");
    assert_eq!(landing_wait_y_velocity_threshold.offset, 0x310);
    assert_eq!(
        landing_wait_y_velocity_threshold.provenance,
        CommonDataProvenance::ExtractedPlCo
    );

    let escapeair_landing_lag = sources
        .iter()
        .find(|source| source.rust_name == "escapeair_landing_lag")
        .expect("escapeair_landing_lag common-data source should be recorded");
    assert_eq!(escapeair_landing_lag.source_name, "x344");
    assert_eq!(escapeair_landing_lag.offset, 0x344);

    let high_speed_ground_friction_multiplier = sources
        .iter()
        .find(|source| source.rust_name == "high_speed_ground_friction_multiplier")
        .expect("high-speed ground friction multiplier source should be recorded");
    assert_eq!(high_speed_ground_friction_multiplier.source_name, "x6C");
    assert_eq!(high_speed_ground_friction_multiplier.offset, 0x6c);

    let fallspecial_platform_landing_y = sources
        .iter()
        .find(|source| source.rust_name == "fallspecial_platform_landing_y")
        .expect("fallspecial_platform_landing_y common-data source should be recorded");
    assert_eq!(fallspecial_platform_landing_y.source_name, "x25C");
    assert_eq!(fallspecial_platform_landing_y.offset, 0x25c);

    let platform_pass_y = sources
        .iter()
        .find(|source| source.rust_name == "platform_pass_y")
        .expect("platform_pass_y common-data source should be recorded");
    assert_eq!(platform_pass_y.source_name, "x464");
    assert_eq!(platform_pass_y.offset, 0x464);

    let platform_pass_y_tap_window = sources
        .iter()
        .find(|source| source.rust_name == "platform_pass_y_tap_window")
        .expect("platform_pass_y_tap_window common-data source should be recorded");
    assert_eq!(platform_pass_y_tap_window.source_name, "x468");
    assert_eq!(platform_pass_y_tap_window.offset, 0x468);

    let pass_initial_y_velocity = sources
        .iter()
        .find(|source| source.rust_name == "pass_initial_y_velocity")
        .expect("pass_initial_y_velocity common-data source should be recorded");
    assert_eq!(pass_initial_y_velocity.source_name, "x46C");
    assert_eq!(pass_initial_y_velocity.offset, 0x46c);

    let platform_drop_delay_ticks = sources
        .iter()
        .find(|source| source.rust_name == "platform_drop_delay_ticks")
        .expect("platform_drop_delay_ticks common-data source should be recorded");
    assert_eq!(platform_drop_delay_ticks.source_name, "x470");
    assert_eq!(platform_drop_delay_ticks.offset, 0x470);

    let entry_start_ticks = sources
        .iter()
        .find(|source| source.rust_name == "entry_start_ticks")
        .expect("entry_start_ticks common-data source should be recorded");
    assert_eq!(entry_start_ticks.source_name, "x6BC");
    assert_eq!(entry_start_ticks.offset, 0x6bc);

    let entry_end_ticks = sources
        .iter()
        .find(|source| source.rust_name == "entry_end_ticks")
        .expect("entry_end_ticks common-data source should be recorded");
    assert_eq!(entry_end_ticks.source_name, "x6C0");
    assert_eq!(entry_end_ticks.offset, 0x6c0);

    let entry_initial_scale_y = sources
        .iter()
        .find(|source| source.rust_name == "entry_initial_scale_y")
        .expect("entry_initial_scale_y common-data source should be recorded");
    assert_eq!(entry_initial_scale_y.source_name, "x6C4");
    assert_eq!(entry_initial_scale_y.offset, 0x6c4);

    let entry_collision_landing_lag = sources
        .iter()
        .find(|source| source.rust_name == "entry_collision_landing_lag_ticks")
        .expect("entry_collision_landing_lag_ticks common-data source should be recorded");
    assert_eq!(entry_collision_landing_lag.source_name, "x6C8");
    assert_eq!(entry_collision_landing_lag.offset, 0x6c8);

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

    let run_brake_animation_pause = sources
        .iter()
        .find(|source| source.rust_name == "run_brake_animation_pause_velocity")
        .expect("RunBrake animation pause velocity source should be recorded");
    assert_eq!(run_brake_animation_pause.source_name, "x42C");
    assert_eq!(run_brake_animation_pause.offset, 0x42c);

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

    let dash_velocity_decay = sources
        .iter()
        .find(|source| source.rust_name == "dash_velocity_decay")
        .expect("dash velocity decay source should be recorded");
    assert_eq!(dash_velocity_decay.source_name, "x54");
    assert_eq!(dash_velocity_decay.offset, 0x54);

    let run_x = sources
        .iter()
        .find(|source| source.rust_name == "run_x")
        .expect("run_x common-data source should be recorded");
    assert_eq!(run_x.source_name, "x58_someLStickXThreshold");
    assert_eq!(run_x.offset, 0x58);

    let turn_run_x = sources
        .iter()
        .find(|source| source.rust_name == "turn_run_x")
        .expect("turn_run_x common-data source should be recorded");
    assert_eq!(turn_run_x.source_name, "x38_someLStickXThreshold");
    assert_eq!(turn_run_x.offset, 0x38);

    let run_accel_taper = sources
        .iter()
        .find(|source| source.rust_name == "run_accel_taper")
        .expect("run acceleration taper common-data source should be recorded");
    assert_eq!(run_accel_taper.source_name, "x5C");
    assert_eq!(run_accel_taper.offset, 0x5c);

    let run_ground_friction_multiplier = sources
        .iter()
        .find(|source| source.rust_name == "run_ground_friction_multiplier")
        .expect("run ground friction multiplier source should be recorded");
    assert_eq!(
        run_ground_friction_multiplier.source_name,
        "x60_someFrictionMul"
    );
    assert_eq!(run_ground_friction_multiplier.offset, 0x60);

    let catch_ground_friction_multiplier = sources
        .iter()
        .find(|source| source.rust_name == "catch_ground_friction_multiplier")
        .expect("catch ground friction multiplier source should be recorded");
    assert_eq!(catch_ground_friction_multiplier.source_name, "x64");
    assert_eq!(catch_ground_friction_multiplier.offset, 0x64);

    let animation_velocity_scale = sources
        .iter()
        .find(|source| source.rust_name == "animation_velocity_scale")
        .expect("animation velocity scale source should be recorded");
    assert_eq!(animation_velocity_scale.source_name, "x440");
    assert_eq!(animation_velocity_scale.offset, 0x440);
    let fall_animation_drift_threshold = sources
        .iter()
        .find(|source| source.rust_name == "fall_animation_drift_threshold")
        .expect("fall animation drift threshold source should be recorded");
    assert_eq!(fall_animation_drift_threshold.source_name, "x444");
    assert_eq!(fall_animation_drift_threshold.offset, 0x444);
    let fall_animation_blend = sources
        .iter()
        .find(|source| source.rust_name == "fall_animation_blend")
        .expect("fall animation blend source should be recorded");
    assert_eq!(fall_animation_blend.source_name, "x448");
    assert_eq!(fall_animation_blend.offset, 0x448);

    for (rust_name, source_name, offset) in [
        ("knockback_weight_multiplier", "xF4", 0xf4),
        ("knockback_decay", "xF8", 0xf8),
        ("damage_knockback_velocity_scale", "x100", 0x100),
        (
            "damage_knockback_frame_decay",
            "x204_knockbackFrameDecay",
            0x204,
        ),
        ("knockback_cap", "x108", 0x108),
        ("throw_knockback_weight", "x10C", 0x10c),
        ("knockback_damage_scale", "x110", 0x110),
        ("knockback_hit_count_scale", "x114", 0x114),
        ("knockback_weight_set_damage", "x118", 0x118),
        ("knockback_result_scale", "x11C", 0x11c),
        ("knockback_result_offset", "x120", 0x120),
        ("damage_sakurai_air_angle_radians", "x144_radians", 0x144),
        ("damage_ground_knockback_friction_multiplier", "x200", 0x200),
        ("damage_sakurai_ground_angle_degrees", "x148", 0x148),
        ("damage_sakurai_ground_min_knockback", "x14C", 0x14c),
        ("damage_sakurai_ground_max_knockback", "x150", 0x150),
        ("damage_duration_scale", "x154", 0x154),
        ("damage_motion_tier_1_threshold", "x158", 0x158),
        ("damage_motion_tier_2_threshold", "x15C", 0x15c),
        ("damage_motion_tier_3_threshold", "x160", 0x160),
        ("damage_ground_knockback_init_clamp", "x164", 0x164),
        ("damage_fly_top_angle_min_radians", "x234", 0x234),
        ("damage_fly_top_angle_max_radians", "x238", 0x238),
        ("damage_fly_top_random_percent_threshold", "x23C", 0x23c),
        ("damage_fly_top_random_chance", "x240", 0x240),
        ("hitlag_max_frames", "x194_unkHitLagFrames", 0x194),
        ("hitlag_damage_scale", "x198", 0x198),
        ("hitlag_base_frames", "x19C", 0x19c),
        ("hitlag_crouch_multiplier", "x1A0", 0x1a0),
        ("hitlag_electric_multiplier", "x1A4", 0x1a4),
        ("di_angle_degrees", "x1A8", 0x1a8),
        ("trigger_di_knockback_multiplier", "x1AC", 0x1ac),
        ("air_speed_clamp_friction", "x1FC", 0x1fc),
        ("passive_input_age_threshold", "x1C", 0x1c),
        (
            "damage_landing_down_bound_knockback_threshold",
            "x1E0",
            0x1e0,
        ),
        ("damage_landing_basic_knockback_threshold", "x1E4", 0x1e4),
        ("down_stand_stick_y", "x244", 0x244),
        ("passive_window_max", "x250", 0x250),
        ("passive_stand_stick_x", "x254", 0x254),
        ("special_air_drift_stick_threshold", "x258", 0x258),
        ("sdi_min_stick_mag", "sdi_min_stick_mag", 0x4b0),
        ("sdi_stick_window", "sdi_stick_window", 0x4b4),
        ("sdi_pos_scale", "sdi_pos_scale", 0x4b8),
        ("asdi_pos_scale", "x4BC", 0x4bc),
        ("down_wait_timer", "x424", 0x424),
        ("rebirth_ticks", "x5D0", 0x5d0),
        ("rebirth_wait_ticks", "x5D4", 0x5d4),
        ("rebirth_hurt_intangible_ticks", "x5D8", 0x5d8),
        ("dead_wait_ticks", "x500", 0x500),
        ("dead_up_star_wait_ticks", "x504", 0x504),
        ("dead_up_star_rise_ticks", "x508", 0x508),
        ("dead_up_star_exit_ticks", "x50C", 0x50c),
        ("top_blast_fall_ko_chance", "x520", 0x520),
        ("dead_up_fall_wait_ticks", "x524", 0x524),
        ("dead_up_fall_anim_ticks", "x528", 0x528),
        ("dead_up_fall_hit_camera_ticks", "x52C", 0x52c),
        ("dead_up_fall_drift_ticks", "x530", 0x530),
        ("dead_up_fall_exit_ticks", "x534", 0x534),
    ] {
        let source = sources
            .iter()
            .find(|source| source.rust_name == rust_name)
            .unwrap_or_else(|| panic!("{rust_name} common-data source should be recorded"));
        assert_eq!(source.source_name, source_name);
        assert_eq!(source.offset, offset);
        assert_eq!(source.provenance, CommonDataProvenance::ExtractedPlCo);
    }
}

#[test]
fn extracted_plco_common_data_reads_big_endian_values_from_source_offsets() {
    let mut bytes = vec![0_u8; 0x6cc];

    put_f32_be(&mut bytes, 0x00, 0.28);
    put_f32_be(&mut bytes, 0x04, 0.29);
    put_f32_be(&mut bytes, 0x08, 0.37);
    put_f32_be(&mut bytes, 0x0c, 0.38);
    put_f32_be(&mut bytes, 0x10, 0.12);
    put_f32_be(&mut bytes, 0x14, 0.31);
    put_f32_be(&mut bytes, 0x18, 0.55);
    put_i32_be(&mut bytes, 0x1c, 6);
    put_f32_be(&mut bytes, 0x20, std::f32::consts::FRAC_PI_4);
    put_f32_be(&mut bytes, 0x24, 0.21);
    put_f32_be(&mut bytes, 0x28, 0.31);
    put_f32_be(&mut bytes, 0x2c, 0.52);
    put_f32_be(&mut bytes, 0x30, 0.74);
    put_f32_be(&mut bytes, 0x34, 0.24);
    put_f32_be(&mut bytes, 0x38, -0.35);
    put_f32_be(&mut bytes, 0x3c, 0.82);
    put_i32_be(&mut bytes, 0x40, 5);
    put_f32_be(&mut bytes, 0x44, 6.0);
    put_f32_be(&mut bytes, 0x48, 7.0);
    put_f32_be(&mut bytes, 0x4c, 16.0);
    put_f32_be(&mut bytes, 0x54, 0.73);
    put_f32_be(&mut bytes, 0x58, 0.66);
    put_f32_be(&mut bytes, 0x5c, 0.42);
    put_f32_be(&mut bytes, 0x60, 1.25);
    put_f32_be(&mut bytes, 0x64, 0.95);
    put_f32_be(&mut bytes, 0x68, 4.0);
    put_f32_be(&mut bytes, 0x6c, 1.75);
    put_f32_be(&mut bytes, 0x70, 0.81);
    put_i32_be(&mut bytes, 0x74, 4);
    put_f32_be(&mut bytes, 0x78, 0.22);
    put_f32_be(&mut bytes, 0x7c, 0.41);
    put_f32_be(&mut bytes, 0x88, 0.83);
    put_i32_be(&mut bytes, 0x8c, 2);
    put_f32_be(&mut bytes, 0x90, 0.35);
    put_f32_be(&mut bytes, 0x94, 0.28);
    put_f32_be(&mut bytes, 0x98, 0.25);
    put_f32_be(&mut bytes, 0xac, 0.26);
    put_f32_be(&mut bytes, 0xb0, -0.27);
    put_f32_be(&mut bytes, 0xdc, 0.43);
    put_f32_be(&mut bytes, 0xe0, 0.44);
    put_i32_be(&mut bytes, 0xe4, 9);
    put_f32_be(&mut bytes, 0xe8, 3.5);
    put_f32_be(&mut bytes, 0xf4, 0.011);
    put_f32_be(&mut bytes, 0xf8, 2.25);
    put_f32_be(&mut bytes, 0x100, 0.032);
    put_f32_be(&mut bytes, 0x108, 2400.0);
    put_f32_be(&mut bytes, 0x10c, 109.36);
    put_f32_be(&mut bytes, 0x110, 0.12);
    put_f32_be(&mut bytes, 0x114, 0.07);
    put_f32_be(&mut bytes, 0x118, 11.0);
    put_f32_be(&mut bytes, 0x11c, 1.6);
    put_f32_be(&mut bytes, 0x120, 19.0);
    put_f32_be(&mut bytes, 0x144, 0.79);
    put_f32_be(&mut bytes, 0x148, 45.0);
    put_f32_be(&mut bytes, 0x14c, 30.0);
    put_f32_be(&mut bytes, 0x150, 33.0);
    put_f32_be(&mut bytes, 0x154, 0.5);
    put_f32_be(&mut bytes, 0x158, 9.0);
    put_f32_be(&mut bytes, 0x15c, 20.0);
    put_f32_be(&mut bytes, 0x160, 31.0);
    put_f32_be(&mut bytes, 0x164, 8.5);
    put_f32_be(&mut bytes, 0x234, 1.2217305);
    put_f32_be(&mut bytes, 0x238, 1.9198622);
    put_i32_be(&mut bytes, 0x23c, 100);
    put_f32_be(&mut bytes, 0x240, 0.3);
    put_f32_be(&mut bytes, 0x194, 18.0);
    put_f32_be(&mut bytes, 0x198, 0.25);
    put_f32_be(&mut bytes, 0x19c, 4.0);
    put_f32_be(&mut bytes, 0x1a0, 0.75);
    put_f32_be(&mut bytes, 0x1a4, 1.25);
    put_f32_be(&mut bytes, 0x1a8, 19.0);
    put_f32_be(&mut bytes, 0x1ac, 0.75);
    put_f32_be(&mut bytes, 0x1fc, 0.04);
    put_f32_be(&mut bytes, 0x200, 1.5);
    put_f32_be(&mut bytes, 0x204, 0.625);
    put_f32_be(&mut bytes, 0x1e0, 4.5);
    put_f32_be(&mut bytes, 0x1e4, 8.0);
    put_f32_be(&mut bytes, 0x244, 72.0 / 127.0);
    put_f32_be(&mut bytes, 0x250, 9.5);
    put_f32_be(&mut bytes, 0x254, 1.25);
    put_f32_be(&mut bytes, 0x258, 0.125);
    put_f32_be(&mut bytes, 0x25c, -0.62);
    put_f32_be(&mut bytes, 0x260, 61.0);
    put_f32_be(&mut bytes, 0x268, 9.0);
    put_f32_be(&mut bytes, 0x278, 0.2);
    put_f32_be(&mut bytes, 0x27c, 0.08);
    put_f32_be(&mut bytes, 0x280, 31.0);
    put_f32_be(&mut bytes, 0x284, 1.2);
    put_f32_be(&mut bytes, 0x288, 0.3);
    put_f32_be(&mut bytes, 0x28c, 1.55);
    put_f32_be(&mut bytes, 0x290, 2.25);
    put_f32_be(&mut bytes, 0x294, 0.22);
    put_f32_be(&mut bytes, 0x298, 1.75);
    put_f32_be(&mut bytes, 0x2bc, 0.65);
    put_f32_be(&mut bytes, 0x2dc, 0.11);
    put_f32_be(&mut bytes, 0x2e0, 0.33);
    put_f32_be(&mut bytes, 0x2e4, 0.055);
    put_f32_be(&mut bytes, 0x2e8, 0.77);
    put_f32_be(&mut bytes, 0x2ec, 0.12);
    put_f32_be(&mut bytes, 0x2f0, 2.2);
    put_f32_be(&mut bytes, 0x2f8, 402.0);
    put_f32_be(&mut bytes, 0x2fc, 92.0);
    put_f32_be(&mut bytes, 0x300, 1.25);
    put_f32_be(&mut bytes, 0x304, 3.5);
    put_f32_be(&mut bytes, 0x308, 34.0 / 127.0);
    put_f32_be(&mut bytes, 0x310, 0.875);
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
    put_i32_be(&mut bytes, 0x348, 9);
    put_f32_be(&mut bytes, 0x354, 30.0);
    put_f32_be(&mut bytes, 0x358, 8.0);
    put_f32_be(&mut bytes, 0x35c, 9.0);
    put_f32_be(&mut bytes, 0x360, 15.0);
    put_f32_be(&mut bytes, 0x364, 4.0);
    put_f32_be(&mut bytes, 0x368, 1.6);
    put_f32_be(&mut bytes, 0x370, 1.0);
    put_f32_be(&mut bytes, 0x374, 1.25);
    put_f32_be(&mut bytes, 0x378, 2.5);
    put_f32_be(&mut bytes, 0x37c, 0.0125);
    put_f32_be(&mut bytes, 0x3a4, 1.0);
    put_f32_be(&mut bytes, 0x3a8, 6.0);
    put_f32_be(&mut bytes, 0x3ac, 16.0);
    put_f32_be(&mut bytes, 0x3b0, 10.0);
    put_f32_be(&mut bytes, 0x3b4, 2.0);
    put_f32_be(&mut bytes, 0x3c4, 1.0);
    put_f32_be(&mut bytes, 0x424, 61.0);
    put_f32_be(&mut bytes, 0x42c, 1.25);
    put_f32_be(&mut bytes, 0x430, 2.0);
    put_f32_be(&mut bytes, 0x440, 1.31);
    put_f32_be(&mut bytes, 0x444, 0.18);
    put_f32_be(&mut bytes, 0x448, 0.42);
    put_f32_be(&mut bytes, 0x44c, 0.625);
    put_f32_be(&mut bytes, 0x450, 0.19);
    put_f32_be(&mut bytes, 0x454, 0.21);
    put_f32_be(&mut bytes, 0x458, 1.9);
    put_f32_be(&mut bytes, 0x45c, 0.37);
    put_f32_be(&mut bytes, 0x460, 2.7);
    put_f32_be(&mut bytes, 0x464, 0.63);
    put_f32_be(&mut bytes, 0x468, 5.0);
    put_f32_be(&mut bytes, 0x46c, -1.25);
    put_f32_be(&mut bytes, 0x470, 6.0);
    put_f32_be(&mut bytes, 0x480, 0.66);
    put_i32_be(&mut bytes, 0x488, 101);
    put_f32_be(&mut bytes, 0x48c, 642.0);
    put_f32_be(&mut bytes, 0x490, 481.0);
    put_f32_be(&mut bytes, 0x494, 0.26);
    put_i32_be(&mut bytes, 0x498, 31);
    put_i32_be(&mut bytes, 0x49c, 32);
    put_f32_be(&mut bytes, 0x4b0, 0.8);
    put_i32_be(&mut bytes, 0x4b4, 6);
    put_f32_be(&mut bytes, 0x4b8, 7.0);
    put_f32_be(&mut bytes, 0x4bc, 3.5);
    put_i32_be(&mut bytes, 0x500, 10);
    put_i32_be(&mut bytes, 0x504, 11);
    put_i32_be(&mut bytes, 0x508, 12);
    put_i32_be(&mut bytes, 0x50c, 13);
    put_i32_be(&mut bytes, 0x520, 77);
    put_i32_be(&mut bytes, 0x524, 14);
    put_i32_be(&mut bytes, 0x528, 15);
    put_i32_be(&mut bytes, 0x52c, 16);
    put_i32_be(&mut bytes, 0x530, 17);
    put_i32_be(&mut bytes, 0x534, 18);
    put_i32_be(&mut bytes, 0x5d0, 22);
    put_i32_be(&mut bytes, 0x5d4, 9);
    put_i32_be(&mut bytes, 0x5d8, 123);
    put_i32_be(&mut bytes, 0x6bc, 31);
    put_i32_be(&mut bytes, 0x6c0, 32);
    put_f32_be(&mut bytes, 0x6c4, 0.02);
    put_i32_be(&mut bytes, 0x6c8, 121);

    let common =
        MeleeCommonData::from_plco_bytes(&bytes).expect("synthetic PlCo slice should extract");

    assert_eq!(common.tap_x_threshold, 47);
    assert_eq!(common.tap_y_threshold, 48);
    assert_eq!(common.main_stick_deadzone_x, 36);
    assert_eq!(common.main_stick_deadzone_y, 37);
    assert_eq!(common.c_stick_deadzone_x, 36);
    assert_eq!(common.c_stick_deadzone_y, 37);
    assert_eq!(common.trigger_deadzone, 31);
    assert_eq!(common.z_shield_analog, 79);
    assert_eq!(common.trigger_timer_threshold, 140);
    assert_eq!(common.aerial_vertical_angle_tan_milli, 1000);
    assert_eq!(common.walk_x, 27);
    assert_eq!(common.walk_slow_x, 20);
    assert_eq!(common.walk_middle_x, 50);
    assert_eq!(common.walk_fast_x, 90);
    assert_eq!(common.turn_x, 30);
    assert_eq!(common.turn_run_x, -44);
    assert_eq!(common.dash_x, 104);
    assert_eq!(common.dash_tap_window, 5);
    assert_eq!(common.dash_early_action_window, 6);
    assert_eq!(common.dash_defensive_action_window, 7);
    assert_eq!(common.dash_late_action_window, 16);
    assert_eq!(common.dash_velocity_decay.to_bits(), 0.73_f32.to_bits());
    assert_eq!(common.run_x, 84);
    assert_eq!(
        common.walk_middle_velocity_ratio.to_bits(),
        0.31_f32.to_bits()
    );
    assert_eq!(
        common.walk_fast_velocity_ratio.to_bits(),
        0.52_f32.to_bits()
    );
    assert_eq!(common.walk_accel_taper.to_bits(), 0.74_f32.to_bits());
    assert_eq!(common.run_accel_taper.to_bits(), 0.42_f32.to_bits());
    assert_eq!(
        common.run_ground_friction_multiplier.to_bits(),
        1.25_f32.to_bits()
    );
    assert_eq!(
        common.catch_ground_friction_multiplier.to_bits(),
        0.95_f32.to_bits()
    );
    assert_eq!(common.guard_on_catch_dash_window, 4);
    assert_eq!(
        common.high_speed_ground_friction_multiplier.to_bits(),
        1.75_f32.to_bits()
    );
    assert_eq!(
        common.run_brake_animation_pause_velocity.to_bits(),
        1.25_f32.to_bits()
    );
    assert_eq!(
        common.animation_velocity_scale.to_bits(),
        1.31_f32.to_bits()
    );
    assert_eq!(
        common.fall_animation_drift_threshold.to_bits(),
        0.18000000715255737_f32.to_bits()
    );
    assert_eq!(
        common.fall_animation_blend.to_bits(),
        0.41999998688697815_f32.to_bits()
    );
    assert_eq!(common.shield_aim_smoothing.to_bits(), 0.625_f32.to_bits());
    assert_eq!(
        common.landing_wait_y_velocity_threshold.to_bits(),
        0.875_f32.to_bits()
    );
    assert_eq!(common.tap_jump_y, 103);
    assert_eq!(common.tap_jump_window, 4);
    assert_eq!(common.air_jump_backward_x, 28);
    assert_eq!(common.tap_jump_release_y, 52);
    assert_eq!(common.fast_fall_y, 105);
    assert_eq!(common.fast_fall_window, 2);
    assert_eq!(common.lcancel_window, 9);
    assert_eq!(common.lcancel_divisor.to_bits(), 3.5_f32.to_bits());
    assert_eq!(
        common.knockback_weight_multiplier.to_bits(),
        0.011_f32.to_bits()
    );
    assert_eq!(common.knockback_decay.to_bits(), 2.25_f32.to_bits());
    assert_eq!(common.knockback_cap.to_bits(), 2400.0_f32.to_bits());
    assert_eq!(
        common.throw_knockback_weight.to_bits(),
        109.36_f32.to_bits()
    );
    assert_eq!(common.knockback_damage_scale.to_bits(), 0.12_f32.to_bits());
    assert_eq!(
        common.knockback_hit_count_scale.to_bits(),
        0.07_f32.to_bits()
    );
    assert_eq!(
        common.knockback_weight_set_damage.to_bits(),
        11.0_f32.to_bits()
    );
    assert_eq!(common.knockback_result_scale.to_bits(), 1.6_f32.to_bits());
    assert_eq!(common.knockback_result_offset.to_bits(), 19.0_f32.to_bits());
    assert_eq!(
        common.damage_knockback_velocity_scale.to_bits(),
        0.032_f32.to_bits()
    );
    assert_eq!(
        common.damage_ground_knockback_friction_multiplier.to_bits(),
        1.5_f32.to_bits()
    );
    assert_eq!(
        common.damage_knockback_frame_decay.to_bits(),
        0.625_f32.to_bits()
    );
    assert_eq!(
        common.damage_sakurai_air_angle_radians.to_bits(),
        0.79_f32.to_bits()
    );
    assert_eq!(
        common.damage_sakurai_ground_angle_degrees.to_bits(),
        45.0_f32.to_bits()
    );
    assert_eq!(
        common.damage_sakurai_ground_min_knockback.to_bits(),
        30.0_f32.to_bits()
    );
    assert_eq!(
        common.damage_sakurai_ground_max_knockback.to_bits(),
        33.0_f32.to_bits()
    );
    assert_eq!(common.damage_duration_scale.to_bits(), 0.5_f32.to_bits());
    assert_eq!(
        common.damage_motion_tier_1_threshold.to_bits(),
        9.0_f32.to_bits()
    );
    assert_eq!(
        common.damage_motion_tier_2_threshold.to_bits(),
        20.0_f32.to_bits()
    );
    assert_eq!(
        common.damage_motion_tier_3_threshold.to_bits(),
        31.0_f32.to_bits()
    );
    assert_eq!(
        common.damage_ground_knockback_init_clamp.to_bits(),
        8.5_f32.to_bits()
    );
    assert_eq!(
        common.damage_fly_top_angle_min_radians.to_bits(),
        1.2217305_f32.to_bits()
    );
    assert_eq!(
        common.damage_fly_top_angle_max_radians.to_bits(),
        1.9198622_f32.to_bits()
    );
    assert_eq!(common.damage_fly_top_random_percent_threshold, 100);
    assert_eq!(
        common.damage_fly_top_random_chance.to_bits(),
        0.3_f32.to_bits()
    );
    assert_eq!(
        common
            .damage_landing_down_bound_knockback_threshold
            .to_bits(),
        4.5_f32.to_bits()
    );
    assert_eq!(
        common.damage_landing_basic_knockback_threshold.to_bits(),
        8.0_f32.to_bits()
    );
    assert_eq!(common.hitlag_max_frames.to_bits(), 18.0_f32.to_bits());
    assert_eq!(common.hitlag_damage_scale.to_bits(), 0.25_f32.to_bits());
    assert_eq!(common.hitlag_base_frames.to_bits(), 4.0_f32.to_bits());
    assert_eq!(
        common.hitlag_crouch_multiplier.to_bits(),
        0.75_f32.to_bits()
    );
    assert_eq!(
        common.hitlag_electric_multiplier.to_bits(),
        1.25_f32.to_bits()
    );
    assert_eq!(common.di_angle_degrees.to_bits(), 19.0_f32.to_bits());
    assert_eq!(
        common.trigger_di_knockback_multiplier.to_bits(),
        0.75_f32.to_bits()
    );
    assert_eq!(
        common.air_speed_clamp_friction.to_bits(),
        0.03999999910593033_f32.to_bits()
    );
    assert_eq!(common.passive_input_age_threshold, 6);
    assert_eq!(common.passive_window_max.to_bits(), 9.5_f32.to_bits());
    assert_eq!(common.passive_stand_stick_x.to_bits(), 1.25_f32.to_bits());
    assert_eq!(
        common.special_air_drift_stick_threshold.to_bits(),
        0.125_f32.to_bits()
    );
    assert_eq!(common.down_stand_stick_y, 72);
    assert_eq!(common.down_wait_timer.to_bits(), 61.0_f32.to_bits());
    assert_eq!(common.crouch_y, 44);
    assert_eq!(common.crouch_release_y, 36);
    assert_eq!(common.tilt_x, 32);
    assert_eq!(common.tilt_y, 33);
    assert_eq!(common.throw_down_y, -34);
    assert_eq!(common.aerial_neutral_x, 55);
    assert_eq!(common.aerial_neutral_y, 56);
    assert_eq!(common.fallspecial_platform_landing_y, -79);
    assert_eq!(common.escape_y, 107);
    assert_eq!(common.escape_y_tap_window, 3);
    assert_eq!(common.escape_x, 108);
    assert_eq!(common.escape_x_tap_window, 4);
    assert_eq!(common.escapeair_deadzone_x, 25);
    assert_eq!(common.escapeair_deadzone_y, 32);
    assert_eq!(common.escapeair_iasa_timer_ticks, 15);
    assert_eq!(common.escapeair_animation_ticks, 50);
    assert_eq!(
        common.escapeair_force.to_bits(),
        0.8119999766349792_f32.to_bits()
    );
    assert_eq!(
        common.escapeair_decay.to_bits(),
        0.9100000262260437_f32.to_bits()
    );
    assert_eq!(common.escapeair_landing_lag_ticks, 10);
    assert_eq!(common.throw_collision_lockout_ticks, 9);
    assert_eq!(
        common.throw_weight_animation_scale.to_bits(),
        0.0125_f32.to_bits()
    );
    assert_eq!(common.grab_mash_stick_threshold, 34);
    assert_eq!(common.grab_timer_base.to_bits(), 30.0_f32.to_bits());
    assert_eq!(
        common.grab_timer_handicap_scale.to_bits(),
        8.0_f32.to_bits()
    );
    assert_eq!(
        common.grab_timer_handicap_offset.to_bits(),
        9.0_f32.to_bits()
    );
    assert_eq!(common.grab_timer_rank_scale.to_bits(), 15.0_f32.to_bits());
    assert_eq!(common.grab_timer_rank_offset.to_bits(), 4.0_f32.to_bits());
    assert_eq!(
        common.grab_timer_percent_scale.to_bits(),
        1.600000023841858_f32.to_bits()
    );
    assert_eq!(
        common.catch_cut_ground_velocity.to_bits(),
        1.0_f32.to_bits()
    );
    assert_eq!(common.capture_jump_velocity_x.to_bits(), 1.25_f32.to_bits());
    assert_eq!(common.capture_jump_velocity_y.to_bits(), 2.5_f32.to_bits());
    assert_eq!(common.grab_timer_decrement.to_bits(), 1.0_f32.to_bits());
    assert_eq!(
        common.grab_mash_timer_decrement.to_bits(),
        6.0_f32.to_bits()
    );
    assert_eq!(
        common.capture_wait_jump_input_window.to_bits(),
        16.0_f32.to_bits()
    );
    assert_eq!(
        common.capture_wait_mash_anim_timer.to_bits(),
        10.0_f32.to_bits()
    );
    assert_eq!(
        common.capture_wait_mash_anim_rate.to_bits(),
        2.0_f32.to_bits()
    );
    assert_eq!(
        common.capture_pulled_high_delta_y.to_bits(),
        1.0_f32.to_bits()
    );

    assert_eq!(common.run_turn_run_no_interrupt_frames, 2);
    assert_eq!(common.platform_pass_y, 80);
    assert_eq!(common.platform_pass_y_tap_window, 5);
    assert_eq!(
        common.pass_initial_y_velocity.to_bits(),
        (-1.25_f32).to_bits()
    );
    assert_eq!(common.platform_drop_delay_ticks, 6);
    assert_eq!(common.cliff_grab_block_stick_y, 84);
    assert_eq!(common.cliff_quick_percent_threshold, 101);
    assert_eq!(common.cliff_wait_low_percent_ticks, 642);
    assert_eq!(common.cliff_wait_high_percent_ticks, 481);
    assert_eq!(common.cliff_option_stick_threshold, 33);
    assert_eq!(common.ledge_cooldown_ticks, 31);
    assert_eq!(common.cliff_wait_hurt_intangible_ticks, 32);
    assert_eq!(common.sdi_min_stick_mag.to_bits(), 0.8_f32.to_bits());
    assert_eq!(common.sdi_stick_window, 6);
    assert_eq!(common.sdi_pos_scale.to_bits(), 7.0_f32.to_bits());
    assert_eq!(common.asdi_pos_scale.to_bits(), 3.5_f32.to_bits());
    assert_eq!(common.rebirth_ticks, 22);
    assert_eq!(common.rebirth_wait_ticks, 9);
    assert_eq!(common.rebirth_hurt_intangible_ticks, 123);
    assert_eq!(common.top_blast_fall_ko_chance, 77);
    assert_eq!(common.dead_wait_ticks, 10);
    assert_eq!(common.dead_up_star_wait_ticks, 11);
    assert_eq!(common.dead_up_star_rise_ticks, 12);
    assert_eq!(common.dead_up_star_exit_ticks, 13);
    assert_eq!(common.dead_up_fall_wait_ticks, 14);
    assert_eq!(common.dead_up_fall_anim_ticks, 15);
    assert_eq!(common.dead_up_fall_hit_camera_ticks, 16);
    assert_eq!(common.dead_up_fall_drift_ticks, 17);
    assert_eq!(common.dead_up_fall_exit_ticks, 18);
    assert_eq!(common.entry_start_ticks, 31);
    assert_eq!(common.entry_end_ticks, 32);
    assert_eq!(
        common.entry_initial_scale_y.to_bits(),
        0.019999999552965164_f32.to_bits()
    );
    assert_eq!(common.entry_collision_landing_lag_ticks, 121);
    assert_eq!(common.player_nudge_x.to_bits(), 0.19_f32.to_bits());
    assert_eq!(common.player_nudge_z.to_bits(), 0.21_f32.to_bits());
    assert_eq!(common.player_nudge_z_clamp.to_bits(), 1.9_f32.to_bits());
    assert_eq!(
        common.transformed_player_nudge_z.to_bits(),
        0.37_f32.to_bits()
    );
    assert_eq!(
        common.transformed_player_nudge_z_clamp.to_bits(),
        2.7_f32.to_bits()
    );
    assert_eq!(common.shield_start_health.to_bits(), 61.0_f32.to_bits());
    assert_eq!(common.shield_release_lockout_frames, 9);
    assert_eq!(common.shield_aim_smoothing.to_bits(), 0.625_f32.to_bits());
    assert_eq!(common.shield_hold_drain.to_bits(), 0.2_f32.to_bits());
    assert_eq!(common.shield_regen.to_bits(), 0.08_f32.to_bits());
    assert_eq!(
        common.shield_break_reset_health.to_bits(),
        31.0_f32.to_bits()
    );
    assert_eq!(
        common.shield_hit_drain_damage_scale.to_bits(),
        1.2_f32.to_bits()
    );
    assert_eq!(common.shield_hit_drain_base.to_bits(), 0.3_f32.to_bits());
    assert_eq!(
        common.shield_setoff_duration_damage_scale.to_bits(),
        1.55_f32.to_bits()
    );
    assert_eq!(
        common.shield_setoff_duration_base.to_bits(),
        2.25_f32.to_bits()
    );
    assert_eq!(
        common.shield_setoff_pushback_scale.to_bits(),
        0.22_f32.to_bits()
    );
    assert_eq!(
        common.shield_setoff_pushback_cap.to_bits(),
        1.75_f32.to_bits()
    );
    assert_eq!(
        common
            .shield_setoff_nonreflect_pushback_multiplier
            .to_bits(),
        0.65_f32.to_bits()
    );
    assert_eq!(
        common.shield_hit_lightshield_min.to_bits(),
        0.11_f32.to_bits()
    );
    assert_eq!(
        common.shield_hit_lightshield_max.to_bits(),
        0.33_f32.to_bits()
    );
    assert_eq!(
        common.shield_setoff_lightshield_min.to_bits(),
        0.055_f32.to_bits()
    );
    assert_eq!(
        common.shield_setoff_lightshield_max.to_bits(),
        0.77_f32.to_bits()
    );
    assert_eq!(
        common.shield_hold_lightshield_min.to_bits(),
        0.12_f32.to_bits()
    );
    assert_eq!(
        common.shield_hold_lightshield_max.to_bits(),
        2.2_f32.to_bits()
    );
    assert_eq!(
        common.shield_break_furafura_percent_base.to_bits(),
        402.0_f32.to_bits()
    );
    assert_eq!(
        common.shield_break_furafura_timer_base.to_bits(),
        92.0_f32.to_bits()
    );
    assert_eq!(
        common.shield_break_furafura_timer_decrement.to_bits(),
        1.25_f32.to_bits()
    );
    assert_eq!(
        common.shield_break_furafura_mash_decrement.to_bits(),
        3.5_f32.to_bits()
    );
}

#[test]
fn extracted_escapeair_decay_preserves_milli_precision_in_velocity_path() {
    let mut bytes = synthetic_plco_common_data_bytes();
    put_f32_be(&mut bytes, 0x338, 1.2);
    put_f32_be(&mut bytes, 0x33c, 0.915);
    let common =
        MeleeCommonData::from_plco_bytes(&bytes).expect("synthetic PlCo slice should extract");
    let mut world = World::for_two_players_with_common_data(common);
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

    for frame in 0..=4 {
        step_world(&mut world, Frame(frame), &jump);
    }
    step_world(&mut world, Frame(5), &right_air_dodge);

    assert_eq!(world.players()[0].motion_state, MotionState::EscapeAir);
    assert_eq!(world.players()[0].velocity.x, 1_098);
}

#[test]
fn extracted_plco_common_data_reports_the_missing_source_field() {
    let bytes = vec![0_u8; 0xe4];

    let err = MeleeCommonData::from_plco_bytes(&bytes)
        .expect_err("slice ending before xE4 should be rejected");

    assert_eq!(
        err,
        CommonDataExtractError::TooShort {
            field: "xE4",
            offset: 0xe4,
            required_len: 0xe8,
            actual_len: 0xe4,
        }
    );
}

fn put_f32_be(bytes: &mut [u8], offset: usize, value: f32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}

fn put_i32_be(bytes: &mut [u8], offset: usize, value: i32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}

fn put_u32_be(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}

fn synthetic_plco_common_data_bytes() -> Vec<u8> {
    let mut bytes = vec![0_u8; 0x6cc];

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
    put_f32_be(&mut bytes, 0x38, -0.35);
    put_f32_be(&mut bytes, 0x3c, 0.82);
    put_i32_be(&mut bytes, 0x40, 5);
    put_f32_be(&mut bytes, 0x44, 6.0);
    put_f32_be(&mut bytes, 0x48, 7.0);
    put_f32_be(&mut bytes, 0x4c, 16.0);
    put_f32_be(&mut bytes, 0x58, 0.66);
    put_f32_be(&mut bytes, 0x68, 4.0);
    put_f32_be(&mut bytes, 0x70, 0.81);
    put_i32_be(&mut bytes, 0x74, 4);
    put_f32_be(&mut bytes, 0x78, 0.22);
    put_f32_be(&mut bytes, 0x7c, 0.41);
    put_f32_be(&mut bytes, 0x88, 0.83);
    put_i32_be(&mut bytes, 0x8c, 2);
    put_f32_be(&mut bytes, 0x90, 0.35);
    put_f32_be(&mut bytes, 0x94, 0.28);
    put_f32_be(&mut bytes, 0x98, 0.25);
    put_f32_be(&mut bytes, 0xac, 0.26);
    put_f32_be(&mut bytes, 0xb0, -0.27);
    put_f32_be(&mut bytes, 0xdc, 0.43);
    put_f32_be(&mut bytes, 0xe0, 0.44);
    put_i32_be(&mut bytes, 0xe4, 7);
    put_f32_be(&mut bytes, 0xe8, 2.0);
    put_f32_be(&mut bytes, 0x10c, 100.0);
    put_f32_be(&mut bytes, 0x1fc, 0.04);
    put_f32_be(&mut bytes, 0x204, 0.5);
    put_f32_be(&mut bytes, 0x258, 0.125);
    put_f32_be(&mut bytes, 0x25c, -0.62);
    put_f32_be(&mut bytes, 0x260, 61.0);
    put_f32_be(&mut bytes, 0x268, 9.0);
    put_f32_be(&mut bytes, 0x278, 0.2);
    put_f32_be(&mut bytes, 0x27c, 0.08);
    put_f32_be(&mut bytes, 0x280, 31.0);
    put_f32_be(&mut bytes, 0x284, 1.2);
    put_f32_be(&mut bytes, 0x288, 0.3);
    put_f32_be(&mut bytes, 0x28c, 1.5);
    put_f32_be(&mut bytes, 0x290, 2.0);
    put_f32_be(&mut bytes, 0x294, 0.2);
    put_f32_be(&mut bytes, 0x298, 2.0);
    put_f32_be(&mut bytes, 0x2bc, 0.6);
    put_f32_be(&mut bytes, 0x2dc, 0.11);
    put_f32_be(&mut bytes, 0x2e0, 0.33);
    put_f32_be(&mut bytes, 0x2e4, 0.05);
    put_f32_be(&mut bytes, 0x2e8, 0.7);
    put_f32_be(&mut bytes, 0x2ec, 0.12);
    put_f32_be(&mut bytes, 0x2f0, 2.2);
    put_f32_be(&mut bytes, 0x2f8, 402.0);
    put_f32_be(&mut bytes, 0x2fc, 92.0);
    put_f32_be(&mut bytes, 0x300, 1.25);
    put_f32_be(&mut bytes, 0x304, 3.5);
    put_f32_be(&mut bytes, 0x308, 32.0 / 127.0);
    put_f32_be(&mut bytes, 0x310, 0.875);
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
    put_i32_be(&mut bytes, 0x348, 9);
    put_f32_be(&mut bytes, 0x354, 30.0);
    put_f32_be(&mut bytes, 0x358, 8.0);
    put_f32_be(&mut bytes, 0x35c, 9.0);
    put_f32_be(&mut bytes, 0x360, 15.0);
    put_f32_be(&mut bytes, 0x364, 4.0);
    put_f32_be(&mut bytes, 0x368, 1.6);
    put_f32_be(&mut bytes, 0x370, 1.0);
    put_f32_be(&mut bytes, 0x374, 1.0);
    put_f32_be(&mut bytes, 0x378, 2.0);
    put_f32_be(&mut bytes, 0x37c, 0.01);
    put_f32_be(&mut bytes, 0x3a4, 1.0);
    put_f32_be(&mut bytes, 0x3a8, 6.0);
    put_f32_be(&mut bytes, 0x3ac, 16.0);
    put_f32_be(&mut bytes, 0x3b0, 10.0);
    put_f32_be(&mut bytes, 0x3b4, 2.0);
    put_f32_be(&mut bytes, 0x3c4, 1.0);
    put_f32_be(&mut bytes, 0x430, 2.0);
    put_f32_be(&mut bytes, 0x464, 0.63);
    put_f32_be(&mut bytes, 0x468, 5.0);
    put_f32_be(&mut bytes, 0x46c, -1.25);
    put_f32_be(&mut bytes, 0x470, 6.0);
    put_i32_be(&mut bytes, 0x5d0, 22);
    put_i32_be(&mut bytes, 0x5d4, 9);
    put_i32_be(&mut bytes, 0x5d8, 123);
    put_i32_be(&mut bytes, 0x520, 77);
    put_i32_be(&mut bytes, 0x6bc, 31);
    put_i32_be(&mut bytes, 0x6c0, 32);
    put_f32_be(&mut bytes, 0x6c4, 0.02);
    put_i32_be(&mut bytes, 0x6c8, 121);

    bytes
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
fn packed_input_melee_snapshot_applies_trigger_deadzone_before_facts() {
    let common = MeleeCommonData::provisional_mole();
    let input = PlayerInput::neutral().with_left_trigger_analog(51);
    let snapshot = input.melee_snapshot_with_config(
        PlayerInput::neutral(),
        MeleeInputTimers::expired(),
        common.input_config(),
    );
    let facts = snapshot.facts(common.input_thresholds());

    assert_eq!(snapshot.left_trigger, 0);
    assert!(!snapshot.shield_held);
    assert!(!facts.shield_held);
}

#[test]
fn packed_input_melee_snapshot_keeps_digital_lr_as_shield_after_trigger_cleaning() {
    let common = MeleeCommonData::provisional_mole();
    let input = PlayerInput::neutral().with_left_trigger_digital(true);
    let snapshot = input.melee_snapshot_with_config(
        PlayerInput::neutral(),
        MeleeInputTimers::expired(),
        common.input_config(),
    );
    let facts = snapshot.facts(common.input_thresholds());

    assert!(snapshot.shield_held);
    assert!(facts.shield_held);
    assert!(facts.digital_shield_held);
    assert!(facts.source_held.lr());
}

#[test]
fn melee_input_timer_counts_z_as_source_lr_for_lcancel() {
    let timers = MeleeInputTimers::expired().update(
        PlayerInput::neutral(),
        PlayerInput::neutral().with_grab(true),
    );

    assert_eq!(timers.trigger, 0);
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
        trigger_deadzone: 0,
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
        left_trigger: shield_analog(),
        right_trigger: 200,
        buttons: GameCubeButtonState::from_bits(
            (1 << 4) | (1 << 5) | (1 << 6) | (1 << 7) | (1 << 10),
        ),
        ..GameCubePadStatus::neutral()
    });

    assert_eq!(snapshot.cstick, (127, -128));
    assert_eq!(snapshot.prev_cstick, (0, 0));
    assert_eq!(snapshot.left_trigger, shield_analog());
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
        main_stick_deadzone_x: 4,
        main_stick_deadzone_y: 4,
        c_stick_deadzone_x: 5,
        c_stick_deadzone_y: 5,
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
fn melee_input_processor_preserves_vanilla_diagonals_without_ucf_cardinal_snap() {
    let mut processor = MeleeInputProcessor::new(MeleeInputConfig {
        main_stick_deadzone_x: 0,
        main_stick_deadzone_y: 0,
        c_stick_deadzone_x: 0,
        c_stick_deadzone_y: 0,
        ..MeleeInputConfig::default()
    });

    let snapshot = processor.update(GameCubePadStatus {
        stick_x: 208,
        stick_y: 133,
        c_stick_x: 133,
        c_stick_y: 208,
        ..GameCubePadStatus::neutral()
    });

    assert_eq!(snapshot.lstick, (80, 5));
    assert_eq!(snapshot.cstick, (5, 80));
}

#[test]
fn melee_input_processor_deadzone_cleanup_does_not_apply_ucf_cardinal_snap() {
    let mut processor = MeleeInputProcessor::new(MeleeInputConfig::default());

    let snapshot = processor.update(GameCubePadStatus {
        stick_x: 216,
        stick_y: 150,
        c_stick_x: 150,
        c_stick_y: 216,
        ..GameCubePadStatus::neutral()
    });
    let facts = snapshot.facts(MeleeInputThresholds::default());

    assert_eq!(snapshot.lstick, (88, 0));
    assert_eq!(snapshot.cstick, (0, 88));
    assert_eq!(facts.dash_direction, 0);
}

#[test]
fn melee_input_processor_handles_full_opposite_stick_delta_without_overflow_or_ucf_flags() {
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

    assert_eq!(snapshot.lstick, (-128, -128));
    assert_eq!(snapshot.prev_lstick, (0, 0));
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
        tap_jump_window: 3,
        ..MeleeInputThresholds::default()
    };

    let inside_window = snapshot_with_timers((dash_stick_x(), 90), (0, 0), 2, 2).facts(thresholds);
    let at_boundary = snapshot_with_timers((dash_stick_x(), 90), (0, 0), 3, 3).facts(thresholds);

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
fn shield_spotdodge_requires_fixed_byte_y_past_extracted_escape_boundary() {
    let thresholds = MeleeCommonData::provisional_mole().input_thresholds();
    let at_extracted_boundary =
        snapshot_with_timers((0, thresholds.escape_y), (0, 0), 254, 1).facts(thresholds);
    let past_extracted_boundary =
        snapshot_with_timers((0, thresholds.escape_y - 1), (0, 0), 254, 1).facts(thresholds);

    assert!(!at_extracted_boundary.main_stick_spot_dodge);
    assert!(past_extracted_boundary.main_stick_spot_dodge);
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
        stick_y: 90,
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
    assert_eq!(z_facts.analog_shield, shield_analog());
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
    assert_eq!(facts.analog_shield, shield_analog());
}

#[test]
fn melee_input_facts_expose_source_lr_edges_after_trigger_synthesis() {
    let thresholds = MeleeInputThresholds::default();
    let mut processor = MeleeInputProcessor::new(MeleeInputConfig {
        trigger_threshold: 1,
        trigger_timer_threshold: 140,
        trigger_deadzone: 0,
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
fn source_lr_digital_press_timers_match_fighter_input_timer_block() {
    let mut world = World::for_two_players();

    step_world(
        &mut world,
        Frame(0),
        &[
            PlayerInput::neutral().with_left_trigger_digital(true),
            PlayerInput::neutral(),
        ],
    );

    let first = world.players()[0];
    assert_eq!(first.source_lr_digital_press_timer, 0);
    assert_eq!(first.source_previous_lr_digital_press_timer, 0xff);

    step_world(
        &mut world,
        Frame(1),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    let aged = world.players()[0];
    assert_eq!(aged.source_lr_digital_press_timer, 1);
    assert_eq!(aged.source_previous_lr_digital_press_timer, 0xff);

    step_world(
        &mut world,
        Frame(2),
        &[
            PlayerInput::neutral().with_right_trigger_digital(true),
            PlayerInput::neutral(),
        ],
    );

    let second = world.players()[0];
    assert_eq!(second.source_lr_digital_press_timer, 0);
    assert_eq!(second.source_previous_lr_digital_press_timer, 1);
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
        trigger_deadzone: 0,
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

    assert_eq!(snapshot.right_trigger, 0);
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
fn airborne_wait_falls_through_explicit_fall_state_not_air_umbrella() {
    let mut world = World::for_two_players();
    let mut airborne = PlayerState::new(0, 20_000, 1);
    airborne.grounded = false;
    airborne.motion_state = MotionState::Wait;
    airborne.velocity.y = -400;
    assert!(world.set_player_state_for_diagnostic(0, airborne));

    step_world(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    assert_eq!(world.players()[0].motion_state, MotionState::Fall);
}

#[test]
fn common_data_participates_in_checksum_for_rollback() {
    let base = World::for_two_players_with_common_data(MeleeCommonData::provisional_mole());
    let altered = World::for_two_players_with_common_data(MeleeCommonData {
        escapeair_force: 1.234,
        ..MeleeCommonData::provisional_mole()
    });

    assert_ne!(base.checksum(), altered.checksum());
}

#[test]
fn shield_aim_smoothing_participates_in_common_data_checksum_for_rollback() {
    let base = World::for_two_players_with_common_data(MeleeCommonData::provisional_mole());
    let altered = World::for_two_players_with_common_data(MeleeCommonData {
        shield_aim_smoothing: 0.625,
        ..MeleeCommonData::provisional_mole()
    });

    assert_ne!(base.checksum(), altered.checksum());
}

#[test]
fn common_data_promotes_decomp_stale_move_damage_reductions() {
    let common = MeleeCommonData::provisional_mole();

    assert_eq!(
        common
            .stale_move_damage_reductions
            .map(|value| value.to_bits()),
        [
            0.09000000357627869_f32.to_bits(),
            0.07999999821186066_f32.to_bits(),
            0.07000000029802322_f32.to_bits(),
            0.05999999865889549_f32.to_bits(),
            0.05000000074505806_f32.to_bits(),
            0.03999999910593033_f32.to_bits(),
            0.029999999329447746_f32.to_bits(),
            0.019999999552965164_f32.to_bits(),
            0.009999999776482582_f32.to_bits(),
        ]
    );
}

#[test]
fn common_data_promotes_decomp_throw_knockback_weight() {
    let common = MeleeCommonData::provisional_mole();

    assert_eq!(
        common.throw_knockback_weight.to_bits(),
        100.0_f32.to_bits(),
        "ftCo_800DDDE4 passes p_ftCommonData->x10C to ftColl_80079AB0 for throw-release knockback"
    );
}

#[test]
fn source_locomotion_motion_vars_participate_in_checksum_and_snapshot() {
    let base = World::for_two_players();
    let mut altered = base.clone();
    let mut player = altered.players()[0];
    player.walk_anim_velocity_x = 1.105;
    player.walk_accel_mul_milli = 1_000;
    player.motion_cmd_var0 = 1;
    player.motion_cmd_var1 = 1;
    player.dash_x0 = 2.0;
    player.run_brake_x0 = true;
    player.run_brake_frames_remaining = 7;
    player.turn_run_x14 = true;
    player.motion_anim_rate_milli = 0;

    assert!(altered.set_player_state_for_diagnostic(0, player));
    assert_ne!(base.checksum(), altered.checksum());

    let snapshot = altered.snapshot().players[0];
    assert_eq!(snapshot.walk_anim_velocity_x.to_bits(), 1.105_f32.to_bits());
    assert_eq!(snapshot.walk_accel_mul_milli, 1_000);
    assert_eq!(snapshot.motion_cmd_var0, 1);
    assert_eq!(snapshot.motion_cmd_var1, 1);
    assert_eq!(snapshot.dash_x0.to_bits(), 2.0_f32.to_bits());
    assert!(snapshot.run_brake_x0);
    assert_eq!(snapshot.run_brake_frames_remaining, 7);
    assert!(snapshot.turn_run_x14);
    assert_eq!(snapshot.motion_anim_rate_milli, 0);
}

fn source_collision_frame_for_one_hit(
    lifecycle_id: u64,
    hitbox: SourceHitboxAttributes,
) -> SourceCollisionFrame {
    SourceCollisionFrame {
        hits: vec![SourceCollisionCapsule::new(
            0,
            lifecycle_id,
            Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 2.0),
        )
        .with_owner_grounded(false)
        .with_source_pose(
            Some(MeleeActionStateId::new(65)),
            Some(SourceActionKey::new("AttackAirN")),
            7,
        )
        .with_hitbox_lifecycle(SourceHitboxLifecycleId::new(lifecycle_id))
        .with_hitbox_attributes(hitbox)],
        hurts: vec![SourceCollisionCapsule::new(
            1,
            10,
            Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 2.0),
        )
        .with_owner_grounded(false)],
    }
}

#[test]
fn source_shield_confirm_drains_shield_health_like_fighter_processhit_8006d1ec() {
    let common = MeleeCommonData {
        shield_start_health: 60.0,
        shield_hit_drain_damage_scale: 1.25,
        shield_hit_drain_base: 2.0,
        shield_hit_lightshield_min: 0.2,
        shield_hit_lightshield_max: 0.8,
        ..MeleeCommonData::PROVISIONAL
    };
    let mut world = World::for_two_players_with_common_data(common);
    let mut defender = world.players()[1];
    defender.grounded = true;
    defender.shield_health = 50.0;
    defender.lightshield_amount = 0.5;
    defender.source_shield_collision_active = true;
    defender.source_shield_hit_active = true;
    assert!(world.set_player_state_for_diagnostic(1, defender));
    let hitbox = SourceHitboxAttributes {
        bone: 14,
        hit_group: 0,
        damage: 12,
        angle: 361,
        knockback_growth: 20,
        weight_set_knockback: 0,
        base_knockback: 0,
        element: 0,
        shield_damage: 3,
        hit_grounded: true,
        hit_aerial: true,
    };
    let collision_frame = SourceCollisionFrame {
        hits: vec![SourceCollisionCapsule::new(
            0,
            1,
            Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 2.0),
        )
        .with_owner_grounded(true)
        .with_source_pose(
            Some(MeleeActionStateId::new(65)),
            Some(SourceActionKey::new("AttackAirN")),
            7,
        )
        .with_hitbox_lifecycle(SourceHitboxLifecycleId::new(1))
        .with_hitbox_attributes(hitbox)],
        hurts: vec![SourceCollisionCapsule::new(
            1,
            SOURCE_SHIELD_HURTBOX_ID,
            Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 2.0),
        )
        .with_owner_grounded(true)],
    };

    let step = world.apply_source_collision_frame(&collision_frame);

    let lightshield_scale = 0.5
        * (common.shield_hit_lightshield_max - common.shield_hit_lightshield_min)
        + common.shield_hit_lightshield_min;
    let expected_drain = common.shield_hit_drain_damage_scale * (15.0 * (1.0 - lightshield_scale))
        + common.shield_hit_drain_base;
    assert_eq!(step.geometry_confirms.len(), 1);
    assert_eq!(step.confirms.len(), 0);
    assert_eq!(step.stages.len(), 0);
    assert_eq!(step.results.len(), 0);
    assert!(
        (world.players()[1].shield_health - (50.0 - expected_drain)).abs() < 0.00001,
        "shield hurtbox confirms should drain shield health without routing normal damage; got {} expected {}",
        world.players()[1].shield_health,
        50.0 - expected_drain
    );
    assert_eq!(world.players()[1].damage_percent, 0.0);
    assert_eq!(world.players()[1].damage_percent_temp, 0.0);
}

#[test]
fn source_guard_shield_confirm_enters_guard_setoff_with_decomp_pushback() {
    let mut world = World::for_two_players();
    let mut attacker = world.players()[0];
    attacker.source_position = SourceVec2 { x: 1.0, y: 0.0 };
    assert!(world.set_player_state_for_diagnostic(0, attacker));

    let mut defender = world.players()[1];
    defender.set_motion_state_alias(MotionState::Guard);
    defender.grounded = true;
    defender.source_position = SourceVec2 { x: 0.0, y: 0.0 };
    defender.shield_health = 60.0;
    defender.lightshield_amount = 1.0;
    defender.source_shield_collision_active = true;
    defender.source_shield_hit_active = true;
    assert!(world.set_player_state_for_diagnostic(1, defender));

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
    let collision_frame = SourceCollisionFrame {
        hits: vec![SourceCollisionCapsule::new(
            0,
            1,
            Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 2.0),
        )
        .with_owner_grounded(false)
        .with_source_pose(
            Some(MeleeActionStateId::new(65)),
            Some(SourceActionKey::new("AttackAirN")),
            7,
        )
        .with_hitbox_lifecycle(SourceHitboxLifecycleId::new(1))
        .with_hitbox_attributes(hitbox)],
        hurts: vec![SourceCollisionCapsule::new(
            1,
            SOURCE_SHIELD_HURTBOX_ID,
            Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 2.0),
        )
        .with_owner_grounded(true)],
    };

    let step = world.apply_source_collision_frame(&collision_frame);
    let defender = world.players()[1];

    assert_eq!(step.geometry_confirms.len(), 1);
    assert!(step.stages.is_empty());
    assert!(step.results.is_empty());
    assert_eq!(
        defender.melee_action_state_id,
        Some(MeleeActionStateId::new(181))
    );
    assert_eq!(
        defender.source_action_key,
        Some(SourceActionKey::new("GuardDamage"))
    );
    assert_eq!(defender.motion_state, MotionState::GuardSetOff);
    assert_eq!(source_units_to_milli(defender.ground_velocity_x), -510);
    assert_eq!(defender.velocity.x, -510);
    assert_eq!(defender.velocity.y, 0);
    assert!(defender.source_shield_collision_active);
    assert!(defender.source_shield_hit_active);
}

#[test]
fn source_only_throw_action_with_stale_guard_alias_does_not_enter_guard_setoff() {
    let mut world = World::for_two_players();
    let mut defender = world.players()[1];
    defender.set_motion_state_alias(MotionState::GuardOn);
    defender.melee_action_state_id = Some(MeleeActionStateId::new(241));
    defender.source_action_key = Some(SourceActionKey::new("TCaptainThrowHi"));
    defender.motion_state_alias = None;
    defender.grounded = false;
    defender.source_shield_collision_active = true;
    defender.source_shield_hit_active = true;
    assert!(world.set_player_state_for_diagnostic(1, defender));

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
    let collision_frame = SourceCollisionFrame {
        hits: vec![SourceCollisionCapsule::new(
            0,
            1,
            Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 2.0),
        )
        .with_owner_grounded(false)
        .with_source_pose(
            Some(MeleeActionStateId::new(65)),
            Some(SourceActionKey::new("AttackAirN")),
            7,
        )
        .with_hitbox_attributes(hitbox)],
        hurts: vec![SourceCollisionCapsule::new(
            1,
            SOURCE_SHIELD_HURTBOX_ID,
            Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 2.0),
        )
        .with_owner_grounded(false)],
    };

    let step = world.apply_source_collision_frame(&collision_frame);
    let defender = world.players()[1];

    assert_eq!(step.geometry_confirms.len(), 1);
    assert_eq!(
        defender.melee_action_state_id,
        Some(MeleeActionStateId::new(241))
    );
    assert_eq!(
        defender.source_action_key,
        Some(SourceActionKey::new("TCaptainThrowHi"))
    );
    assert_eq!(defender.motion_state, MotionState::GuardOn);
}

#[test]
fn source_shield_confirm_does_not_record_stale_move_before_body_damage() {
    let mut world = World::for_two_players();
    let mut attacker = world.players()[0];
    attacker.set_motion_state_alias(MotionState::AttackAirN);
    assert!(world.set_player_state_for_diagnostic(0, attacker));
    let mut defender = world.players()[1];
    defender.grounded = true;
    defender.shield_health = 60.0;
    defender.source_shield_collision_active = true;
    defender.source_shield_hit_active = true;
    assert!(world.set_player_state_for_diagnostic(1, defender));
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
    let shield_only = SourceCollisionFrame {
        hits: vec![SourceCollisionCapsule::new(
            0,
            1,
            Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 2.0),
        )
        .with_owner_grounded(false)
        .with_source_pose(
            Some(MeleeActionStateId::new(65)),
            Some(SourceActionKey::new("AttackAirN")),
            7,
        )
        .with_hitbox_lifecycle(SourceHitboxLifecycleId::new(1))
        .with_hitbox_attributes(hitbox)],
        hurts: vec![SourceCollisionCapsule::new(
            1,
            SOURCE_SHIELD_HURTBOX_ID,
            Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 2.0),
        )
        .with_owner_grounded(true)],
    };

    let shield_step = world.apply_source_collision_frame(&shield_only);
    assert_eq!(shield_step.geometry_confirms.len(), 1);
    assert_eq!(shield_step.stages.len(), 0);

    let mut defender = world.players()[1];
    defender.source_shield_collision_active = false;
    defender.source_shield_hit_active = false;
    assert!(world.set_player_state_for_diagnostic(1, defender));
    let body_hit = source_collision_frame_for_one_hit(2, hitbox);
    let body_step = world.apply_source_collision_frame(&body_hit);

    assert_eq!(body_step.stages.len(), 1);
    assert_eq!(
        body_step.stages[0].damage.to_bits(),
        10.0_f32.to_bits(),
        "plStale_UpdateStaleMovesFromFighter is reached from damage finalization paths such as ftColl_8007BE3C, not from the earlier shield-only overlap gate"
    );
}

#[test]
fn source_shield_break_routes_to_shieldbreakfly_like_ftco_80098b20() {
    let common = MeleeCommonData {
        shield_break_reset_health: 30.0,
        shield_hit_drain_damage_scale: 1.0,
        shield_hit_drain_base: 0.0,
        shield_hit_lightshield_min: 0.0,
        shield_hit_lightshield_max: 0.0,
        ..MeleeCommonData::PROVISIONAL
    };
    let mut world = World::for_two_players_with_common_data(common);
    let mut defender = world.players()[1];
    defender.grounded = true;
    defender.shield_health = 1.0;
    defender.lightshield_amount = 0.0;
    defender.source_shield_collision_active = true;
    defender.source_shield_hit_active = true;
    assert!(world.set_player_state_for_diagnostic(1, defender));
    let hitbox = SourceHitboxAttributes {
        bone: 14,
        hit_group: 0,
        damage: 5,
        angle: 361,
        knockback_growth: 20,
        weight_set_knockback: 0,
        base_knockback: 0,
        element: 0,
        shield_damage: 0,
        hit_grounded: true,
        hit_aerial: true,
    };
    let collision_frame = SourceCollisionFrame {
        hits: vec![SourceCollisionCapsule::new(
            0,
            1,
            Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 2.0),
        )
        .with_owner_grounded(true)
        .with_source_pose(
            Some(MeleeActionStateId::new(65)),
            Some(SourceActionKey::new("AttackAirN")),
            7,
        )
        .with_hitbox_lifecycle(SourceHitboxLifecycleId::new(1))
        .with_hitbox_attributes(hitbox)],
        hurts: vec![SourceCollisionCapsule::new(
            1,
            SOURCE_SHIELD_HURTBOX_ID,
            Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 2.0),
        )
        .with_owner_grounded(true)],
    };

    let step = world.apply_source_collision_frame(&collision_frame);
    let defender = world.players()[1];

    assert_eq!(step.confirms.len(), 0);
    assert_eq!(defender.shield_health.to_bits(), 30.0_f32.to_bits());
    assert_eq!(
        defender.melee_action_state_id,
        Some(MeleeActionStateId::new(205)),
        "ftCo_800925A4 calls ftCo_80098B20 when shield health crosses below zero"
    );
    assert_eq!(defender.motion_state, MotionState::ShieldBreakFly);
    assert!(!defender.grounded, "ftCo_80098B20 calls ftCommon_8007D5D4");
    assert_eq!(defender.source_self_velocity_x.to_bits(), 0.0_f32.to_bits());
    assert_eq!(
        defender.source_self_velocity_y.to_bits(),
        defender.profile.shield_break_initial_velocity.to_bits()
    );
    assert_eq!(
        defender.velocity,
        Vec2 {
            x: 0,
            y: source_units_to_milli(defender.profile.shield_break_initial_velocity)
        }
    );
    assert!(
        !defender.source_shield_collision_active
            && !defender.source_shield_hit_active
            && !defender.source_shield_hit_update_pos
    );
}

#[test]
fn held_shield_drain_routes_to_shieldbreakfly_like_ftco_800925a4() {
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
    player.set_source_motion_anim_frame(4.0);
    player.shield_health = 1.0;
    player.lightshield_amount = 0.0;
    player.source_shield_collision_active = true;
    player.source_shield_hit_active = true;
    player.source_shield_hit_update_pos = true;
    assert!(world.set_player_state_for_diagnostic(0, player));

    let inputs = [
        PlayerInput::neutral().with_left_trigger_digital(true),
        PlayerInput::neutral(),
    ];
    step_world(&mut world, Frame(0), &inputs);
    let player = world.players()[0];

    assert_eq!(
        player.melee_action_state_id,
        Some(MeleeActionStateId::new(205)),
        "ftCo_800925A4 calls ftCo_80098B20 when held shield drain crosses below zero"
    );
    assert_eq!(player.motion_state, MotionState::ShieldBreakFly);
    assert!(!player.grounded, "ftCo_80098B20 calls ftCommon_8007D5D4");
    assert_eq!(player.shield_health.to_bits(), 30.0_f32.to_bits());
    assert_eq!(player.source_self_velocity_x.to_bits(), 0.0_f32.to_bits());
    assert_eq!(
        player.source_self_velocity_y.to_bits(),
        player.profile.shield_break_initial_velocity.to_bits()
    );
    assert_eq!(
        player.velocity,
        Vec2 {
            x: 0,
            y: source_units_to_milli(player.profile.shield_break_initial_velocity)
        }
    );
}

#[test]
fn shieldbreakfly_keeps_previous_model_pose_like_fighter_change_motion_state_no_anim() {
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

    let snapshot = world.snapshot().players[0];
    assert_eq!(snapshot.motion_state, MotionState::ShieldBreakFly);
    assert_eq!(
        snapshot.source_pose_action_key,
        Some(SourceActionKey::new("Guard")),
        "ftCo_80098B20 enters ShieldBreakFly, but Fighter_ChangeMotionState anim_id == -1 handling clears animation controllers without removing the model pose"
    );
    assert_eq!(
        snapshot.source_pose_frame, 10,
        "the retained pose should remain the last Guard skeleton pose until a fresh source animation replaces it"
    );
}

#[test]
fn source_shield_break_sets_purin_x2222_b3_like_ftco_80098b20() {
    let common = MeleeCommonData {
        shield_break_reset_health: 30.0,
        shield_hit_drain_damage_scale: 1.0,
        shield_hit_drain_base: 0.0,
        shield_hit_lightshield_min: 0.0,
        shield_hit_lightshield_max: 0.0,
        ..MeleeCommonData::PROVISIONAL
    };
    let purin_profile = FighterProfile {
        reference_character: "purin",
        ..FighterProfile::FALCON_LIKE
    };
    let mut world = World::for_two_players_on_stage_with_profiles_and_common_data(
        StageProfile::battlefield(),
        [FighterProfile::FALCON_LIKE, purin_profile],
        common,
    );
    let mut defender = world.players()[1];
    defender.grounded = true;
    defender.shield_health = 1.0;
    defender.lightshield_amount = 0.0;
    defender.source_shield_collision_active = true;
    defender.source_shield_hit_active = true;
    assert!(world.set_player_state_for_diagnostic(1, defender));
    let hitbox = SourceHitboxAttributes {
        bone: 14,
        hit_group: 0,
        damage: 5,
        angle: 361,
        knockback_growth: 20,
        weight_set_knockback: 0,
        base_knockback: 0,
        element: 0,
        shield_damage: 0,
        hit_grounded: true,
        hit_aerial: true,
    };
    let collision_frame = SourceCollisionFrame {
        hits: vec![SourceCollisionCapsule::new(
            0,
            1,
            Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 2.0),
        )
        .with_owner_grounded(true)
        .with_source_pose(
            Some(MeleeActionStateId::new(65)),
            Some(SourceActionKey::new("AttackAirN")),
            7,
        )
        .with_hitbox_lifecycle(SourceHitboxLifecycleId::new(1))
        .with_hitbox_attributes(hitbox)],
        hurts: vec![SourceCollisionCapsule::new(
            1,
            SOURCE_SHIELD_HURTBOX_ID,
            Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 2.0),
        )
        .with_owner_grounded(true)],
    };

    world.apply_source_collision_frame(&collision_frame);
    let defender = world.players()[1];

    assert_eq!(defender.motion_state, MotionState::ShieldBreakFly);
    assert!(
        defender.source_x2222_b3,
        "ftCo_80098B20 sets x2222_b3 only when fp->kind == FTKIND_PURIN"
    );
}

#[test]
fn source_shield_break_furafura_initializes_timer_like_ftco_80099010() {
    let common = MeleeCommonData {
        shield_break_reset_health: 30.0,
        shield_break_furafura_percent_base: 120.0,
        shield_break_furafura_timer_base: 90.0,
        ..MeleeCommonData::PROVISIONAL
    };
    let mut world = World::for_two_players_with_common_data(common);
    let mut player = world.players()[0];
    player.motion_state = MotionState::ShieldBreakStandU;
    player.set_motion_state_alias(MotionState::ShieldBreakStandU);
    player.motion_state_alias = None;
    player.melee_action_state_id = Some(MeleeActionStateId::new(209));
    player.source_action_total_frames = 1;
    player.motion_frame = 1;
    player.set_source_motion_anim_frame(1.0);
    player.damage_percent = 35.0;
    player.shield_health = 0.0;
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(&mut world, Frame(0), &[PlayerInput::neutral(); 2]);

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::Furafura);
    assert_eq!(
        player.melee_action_state_id,
        Some(MeleeActionStateId::new(211)),
        "ftCo_80099010 changes to ftCo_MS_Furafura after ShieldBreakStand ends"
    );
    assert_eq!(player.shield_health.to_bits(), 30.0_f32.to_bits());
    assert_eq!(
        player.source_grab_timer.to_bits(),
        (90.0_f32 + (120.0_f32 - 35.0_f32)).to_bits(),
        "ftCo_80099010 initializes grab_timer to max(x2F8 - percent, 0) + x2FC"
    );
    assert_eq!(player.source_grab_mash_x, 0);
    assert_eq!(player.source_grab_mash_y, 0);
}

#[test]
fn source_furafura_anim_resets_shield_and_exits_when_timer_expires() {
    let common = MeleeCommonData {
        shield_break_reset_health: 30.0,
        shield_break_furafura_timer_decrement: 1.0,
        shield_break_furafura_mash_decrement: 3.0,
        ..MeleeCommonData::PROVISIONAL
    };
    let mut world = World::for_two_players_with_common_data(common);
    let mut player = world.players()[0];
    player.motion_state = MotionState::Furafura;
    player.set_motion_state_alias(MotionState::Furafura);
    player.motion_state_alias = None;
    player.melee_action_state_id = Some(MeleeActionStateId::new(211));
    player.source_action_key = Some(SourceActionKey::new("FuraFura"));
    player.source_action_total_frames = 120;
    player.source_grab_timer = 1.0;
    player.shield_health = 2.0;
    player.grounded = true;
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(&mut world, Frame(0), &[PlayerInput::neutral(); 2]);

    let player = world.players()[0];
    assert_eq!(
        player.motion_state,
        MotionState::Wait,
        "ftCo_Furafura_Anim exits through ft_8008A2BC once grab_timer reaches zero"
    );
    assert_eq!(
        player.shield_health.to_bits(),
        30.0_f32.to_bits(),
        "ftCo_Furafura_Anim rewrites shield_health from x280 every frame before exit"
    );
}

#[test]
fn source_shieldbreakfly_lands_into_down_like_ft_80082c74_callback() {
    let common = MeleeCommonData::PROVISIONAL;
    let mut world = World::for_two_players_on_stage_with_profiles_and_common_data(
        StageProfile::battlefield(),
        [FighterProfile::FALCON_LIKE, FighterProfile::FALCON_LIKE],
        common,
    );
    let mut player = world.players()[0];
    player.motion_state = MotionState::ShieldBreakFly;
    player.set_motion_state_alias(MotionState::ShieldBreakFly);
    player.motion_state_alias = None;
    player.melee_action_state_id = Some(MeleeActionStateId::new(205));
    player.source_self_velocity_y = -5.0;
    player.velocity.y = source_units_to_milli(-5.0);
    player.grounded = false;
    player.position.x = 0;
    player.position.y = 10000;
    player.source_position.x = 0.0;
    player.source_position.y = 10.0;
    player.source_down_bound_pose = Some(SourceDownBoundPose {
        hip_mtx_0_1: 0.0,
        hip_mtx_0_2: 0.0,
        hip_mtx_1_1: 1.0,
        hip_mtx_1_2: 0.0,
    });
    assert!(world.set_player_state_for_diagnostic(0, player));

    for frame in 0..8 {
        step_world(&mut world, Frame(frame), &[PlayerInput::neutral(); 2]);
        if matches!(
            world.players()[0].motion_state,
            MotionState::ShieldBreakDownU | MotionState::ShieldBreakDownD
        ) {
            break;
        }
    }

    let player = world.players()[0];
    assert!(
        matches!(
            player.motion_state,
            MotionState::ShieldBreakDownU | MotionState::ShieldBreakDownD
        ),
        "ShieldBreakFly_Coll uses ft_80082C74(..., ftCo_80098E3C) to enter ShieldBreakDown on ground contact; got {:?}, grounded={}, y={}, source_y={}, vel_y={}, self_vel_y={}",
        player.motion_state,
        player.grounded,
        player.position.y,
        player.source_position.y,
        player.velocity.y,
        player.source_self_velocity_y
    );
    assert!(player.grounded);
}

#[test]
fn source_powershield_b2_timer_skips_normal_shield_damage_like_ftcoll_8007a06c() {
    let common = MeleeCommonData {
        shield_hit_drain_damage_scale: 1.0,
        shield_hit_drain_base: 3.0,
        shield_hit_lightshield_min: 0.0,
        shield_hit_lightshield_max: 0.0,
        ..MeleeCommonData::PROVISIONAL
    };
    let mut world = World::for_two_players_with_common_data(common);
    let mut defender = world.players()[1];
    defender.grounded = true;
    defender.shield_health = 50.0;
    defender.lightshield_amount = 0.0;
    defender.source_shield_collision_active = true;
    defender.source_shield_hit_active = true;
    defender.source_x221c_b2 = true;
    defender.source_guard_reflect_damage_skip_timer = 2.0;
    assert!(world.set_player_state_for_diagnostic(1, defender));
    let hitbox = SourceHitboxAttributes {
        bone: 14,
        hit_group: 0,
        damage: 10,
        angle: 361,
        knockback_growth: 20,
        weight_set_knockback: 0,
        base_knockback: 0,
        element: 0,
        shield_damage: 0,
        hit_grounded: true,
        hit_aerial: true,
    };
    let collision_frame = SourceCollisionFrame {
        hits: vec![SourceCollisionCapsule::new(
            0,
            1,
            Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 2.0),
        )
        .with_owner_grounded(true)
        .with_source_pose(
            Some(MeleeActionStateId::new(65)),
            Some(SourceActionKey::new("AttackAirN")),
            7,
        )
        .with_hitbox_lifecycle(SourceHitboxLifecycleId::new(1))
        .with_hitbox_attributes(hitbox)],
        hurts: vec![SourceCollisionCapsule::new(
            1,
            SOURCE_SHIELD_HURTBOX_ID,
            Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 2.0),
        )
        .with_owner_grounded(true)],
    };

    let step = world.apply_source_collision_frame(&collision_frame);

    assert_eq!(step.geometry_confirms.len(), 1);
    assert_eq!(step.confirms.len(), 0);
    assert_eq!(
        world.players()[1].shield_health.to_bits(),
        50.0_f32.to_bits(),
        "ftcoll.c only subtracts shield health when fp->x221C_b2 is false"
    );
}

const SOURCE_HIT_ELEMENT_CATCH: u8 = 8;

fn source_collision_frame_for_one_ground_grab() -> SourceCollisionFrame {
    source_collision_frame_for_ground_grab_pose(
        Some(MeleeActionStateId::new(212)),
        Some(SourceActionKey::new("Catch")),
        6,
    )
}

fn source_collision_frame_for_ground_grab_pose(
    action_state_id: Option<MeleeActionStateId>,
    source_action_key: Option<SourceActionKey>,
    source_frame: u8,
) -> SourceCollisionFrame {
    let hitbox = SourceHitboxAttributes {
        bone: 14,
        hit_group: 0,
        damage: 0,
        angle: 0,
        knockback_growth: 0,
        weight_set_knockback: 0,
        base_knockback: 0,
        element: SOURCE_HIT_ELEMENT_CATCH,
        shield_damage: 0,
        hit_grounded: true,
        hit_aerial: false,
    };
    SourceCollisionFrame {
        hits: vec![SourceCollisionCapsule::new(
            0,
            1,
            Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 2.0),
        )
        .with_owner_grounded(true)
        .with_source_pose(action_state_id, source_action_key, source_frame)
        .with_hitbox_lifecycle(SourceHitboxLifecycleId::new(1))
        .with_hitbox_attributes(hitbox)],
        hurts: vec![SourceCollisionCapsule::new(
            1,
            10,
            Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 2.0),
        )
        .with_owner_grounded(true)],
    }
}

#[test]
fn source_grab_confirms_use_current_hurt_capsule_not_previous_damage_root() {
    let hitbox = SourceHitboxAttributes {
        bone: 14,
        hit_group: 0,
        damage: 0,
        angle: 0,
        knockback_growth: 0,
        weight_set_knockback: 0,
        base_knockback: 0,
        element: SOURCE_HIT_ELEMENT_CATCH,
        shield_damage: 0,
        hit_grounded: true,
        hit_aerial: false,
    };
    let frame = SourceCollisionFrame {
        hits: vec![SourceCollisionCapsule::new(
            0,
            1,
            Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 1.0),
        )
        .with_owner_grounded(true)
        .with_source_pose(
            Some(MeleeActionStateId::new(212)),
            Some(SourceActionKey::new("Catch")),
            6,
        )
        .with_hitbox_lifecycle(SourceHitboxLifecycleId::new(6_u64 << 32))
        .with_hitbox_attributes(hitbox)],
        hurts: vec![SourceCollisionCapsule::new(
            1,
            10,
            Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 1.0),
        )
        .with_previous_capsule(Capsule3::new(
            Vec3::new(20.0, 0.0, 0.0),
            Vec3::new(20.0, 1.0, 0.0),
            1.0,
        ))
        .with_owner_grounded(true)],
    };

    let confirms = source_grab_confirms(&frame);

    assert_eq!(
        confirms.len(),
        1,
        "ftColl_80078A2C catch collision tests the victim's current hurt capsule; the normal-hit previous/current ordering path must not make grabs reuse stale damage roots"
    );
}

#[test]
fn world_routes_catch_hitboxes_through_source_grab_callbacks_not_damage() {
    let mut world = World::for_two_players();
    let mut grabber = world.players()[0];
    grabber.set_motion_state_alias(MotionState::Catch);
    grabber.grounded = true;
    grabber.ground_velocity_x = 1.25;
    grabber.ground_accel_x = 0.25;
    grabber.ground_accel_x2 = 0.125;
    grabber.source_self_velocity_x = 1.25;
    grabber.velocity.x = 1_250;
    grabber.motion_frame = 5;
    grabber.motion_anim_frame_milli = 5_000;
    assert!(world.set_player_state_for_diagnostic(0, grabber));

    let mut victim = world.players()[1];
    victim.set_motion_state_alias(MotionState::GuardOn);
    victim.grounded = true;
    assert!(world.set_player_state_for_diagnostic(1, victim));

    let step = world.apply_source_collision_frame(&source_collision_frame_for_one_ground_grab());

    assert!(step.confirms.is_empty());
    assert!(step.stages.is_empty());
    assert_eq!(step.applied_stage_count, 0);
    assert_eq!(step.grab_confirms.len(), 1);
    assert_eq!(step.grab_confirms[0].grabber_index, 0);
    assert_eq!(step.grab_confirms[0].victim_index, 1);

    let players = world.players();
    assert_eq!(
        players[0].melee_action_state_id,
        Some(MeleeActionStateId::new(213))
    );
    assert_eq!(
        players[0].source_action_key,
        Some(SourceActionKey::new("Catch"))
    );
    assert_eq!(
        players[0].motion_frame, 5,
        "fn_800D9CE8 keeps Catch source animation timing when entering CatchPull"
    );
    assert_eq!(players[0].motion_anim_frame_milli, 5_000);
    assert_eq!(players[0].source_victim_index, Some(1));
    assert_eq!(players[0].source_x1a5c_index, Some(1));
    assert!(players[0].source_x221b_b5);
    assert_eq!(
        players[0].ground_velocity_x.to_bits(),
        0.0_f32.to_bits(),
        "fn_800DA1D8/fn_800D9CE8 source grab path must clear fp->gr_vel"
    );
    assert_eq!(players[0].ground_accel_x.to_bits(), 0.0_f32.to_bits());
    assert_eq!(players[0].ground_accel_x2.to_bits(), 0.0_f32.to_bits());
    assert_eq!(
        players[0].source_self_velocity_x.to_bits(),
        0.0_f32.to_bits()
    );
    assert_eq!(players[0].velocity.x, 0);

    assert_eq!(
        players[1].melee_action_state_id,
        Some(MeleeActionStateId::new(226))
    );
    assert_eq!(
        players[1].source_action_key,
        Some(SourceActionKey::new("CapturePulledLw"))
    );
    assert_eq!(players[1].source_victim_index, Some(0));
    assert_eq!(players[1].source_x1a5c_index, Some(0));
    assert!(!players[1].source_x221b_b5);
}

#[test]
fn source_normal_grab_capture_faces_victim_toward_grabber_for_both_facings() {
    for grabber_facing in [1, -1] {
        let mut world = World::for_two_players();
        let mut grabber = world.players()[0];
        grabber.set_motion_state_alias(MotionState::Catch);
        grabber.grounded = true;
        grabber.facing = grabber_facing;
        grabber.source_motion_entry_facing = grabber_facing;
        assert!(world.set_player_state_for_diagnostic(0, grabber));

        let mut victim = world.players()[1];
        victim.set_motion_state_alias(MotionState::Wait);
        victim.grounded = true;
        victim.facing = grabber_facing;
        victim.source_motion_entry_facing = grabber_facing;
        assert!(world.set_player_state_for_diagnostic(1, victim));

        let step =
            world.apply_source_collision_frame(&source_collision_frame_for_one_ground_grab());

        assert_eq!(step.grab_confirms.len(), 1);
        let players = world.players();
        assert_eq!(players[0].source_victim_index, Some(1));
        assert_eq!(
            players[1].melee_action_state_id,
            Some(MeleeActionStateId::new(226))
        );
        assert_eq!(
            players[1].source_action_key,
            Some(SourceActionKey::new("CapturePulledLw"))
        );
        assert_eq!(
            players[1].facing, -grabber_facing,
            "normal grab capture should face the victim toward the grabber"
        );
        assert_eq!(players[1].source_motion_entry_facing, -grabber_facing);
    }
}

#[test]
fn source_catch_pull_collision_cannot_reconfirm_and_reset_grab() {
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
    grabber.motion_frame = 4;
    grabber.motion_anim_frame_milli = 4_000;
    assert!(world.set_player_state_for_diagnostic(0, grabber));

    let mut victim = world.players()[1];
    victim.motion_state_alias = None;
    victim.motion_state = MotionState::GuardOn;
    victim.melee_action_state_id = Some(MeleeActionStateId::new(226));
    victim.source_action_key = Some(SourceActionKey::new("CapturePulledLw"));
    victim.source_action_total_frames = 20;
    victim.source_victim_index = Some(0);
    victim.source_x1a5c_index = Some(0);
    victim.source_x221b_b5 = false;
    victim.grounded = true;
    assert!(world.set_player_state_for_diagnostic(1, victim));

    let step = world.apply_source_collision_frame_with_action_total_frames(
        &source_collision_frame_for_ground_grab_pose(
            Some(MeleeActionStateId::new(213)),
            Some(SourceActionKey::new("Catch")),
            5,
        ),
        |action_state_id| match action_state_id.get() {
            213 => Some(30),
            226 => Some(20),
            _ => None,
        },
    );

    assert!(
        step.grab_confirms.is_empty(),
        "ftCo_CatchPull_Coll is not a fresh grab-confirm callback; Pull must not re-enter itself"
    );
    let players = world.players();
    assert_eq!(
        players[0].melee_action_state_id,
        Some(MeleeActionStateId::new(213))
    );
    assert_eq!(players[0].motion_frame, 4);
    assert_eq!(players[0].motion_anim_frame_milli, 4_000);
    assert_eq!(
        players[1].melee_action_state_id,
        Some(MeleeActionStateId::new(226))
    );
}

#[test]
fn source_only_capture_pulled_lw_ticks_source_callback_not_legacy_guard_on() {
    let mut world = World::for_two_players();
    let mut victim = world.players()[1];
    victim.set_motion_state_alias(MotionState::GuardOn);
    victim.motion_state_alias = None;
    victim.melee_action_state_id = Some(MeleeActionStateId::new(226));
    victim.source_action_key = Some(SourceActionKey::new("CapturePulledLw"));
    victim.source_action_total_frames = 4;
    victim.source_victim_index = Some(0);
    victim.source_x1a5c_index = Some(0);
    victim.grounded = true;
    victim.source_position = SourceVec2 { x: 46.05, y: 0.0 };
    victim.position = Vec2 { x: 46_050, y: 0 };
    assert!(world.set_player_state_for_diagnostic(1, victim));

    step_world_with_source_runtime_data(
        &mut world,
        Frame(1),
        &[PlayerInput::neutral(); 2],
        |_| None,
        |action_state_id| match action_state_id.get() {
            226 => Some(4),
            227 => Some(60),
            _ => None,
        },
    );

    let victim = world.players()[1];
    assert_eq!(
        victim.melee_action_state_id,
        Some(MeleeActionStateId::new(226)),
        "CapturePulledLw must stay on the source-only capture callback path, not legacy GuardOn"
    );
    assert_eq!(
        victim.source_action_key,
        Some(SourceActionKey::new("CapturePulledLw"))
    );
    assert_eq!(victim.motion_frame, 1);
    assert_eq!(victim.motion_anim_frame_milli, 1_000);
    assert_eq!(victim.source_position.x.to_bits(), 46.05_f32.to_bits());
    assert_eq!(victim.position.x, 46_050);
}

#[test]
fn source_catch_pull_throw_flag_enters_catch_wait_and_victim_capture_wait_lw() {
    let mut world = World::for_two_players();
    let mut thrower = world.players()[0];
    thrower.motion_state_alias = None;
    thrower.motion_state = MotionState::Catch;
    thrower.melee_action_state_id = Some(MeleeActionStateId::new(213));
    thrower.source_action_key = Some(SourceActionKey::new("Catch"));
    thrower.source_action_total_frames = 30;
    thrower.source_victim_index = Some(1);
    thrower.source_x1a5c_index = Some(1);
    thrower.source_x221b_b5 = true;
    thrower.grounded = true;
    thrower.ground_velocity_x = 1.25;
    thrower.source_self_velocity_x = 1.25;
    thrower.velocity.x = 1_250;
    thrower.motion_throw_flags = 1 << 3;
    assert!(world.set_player_state_for_diagnostic(0, thrower));

    let mut victim = world.players()[1];
    victim.motion_state_alias = None;
    victim.motion_state = MotionState::GuardOn;
    victim.melee_action_state_id = Some(MeleeActionStateId::new(226));
    victim.source_action_key = Some(SourceActionKey::new("CapturePulledLw"));
    victim.source_action_total_frames = 20;
    victim.source_victim_index = Some(0);
    victim.source_x1a5c_index = Some(0);
    victim.source_x221b_b5 = false;
    victim.grounded = true;
    assert!(world.set_player_state_for_diagnostic(1, victim));

    step_world_with_source_runtime_data(
        &mut world,
        Frame(1),
        &[PlayerInput::neutral(); 2],
        |_| None,
        |action_state_id| match action_state_id.get() {
            213 => Some(30),
            216 => Some(60),
            226 => Some(20),
            227 => Some(35),
            _ => None,
        },
    );

    let players = world.players();
    assert_eq!(
        players[0].melee_action_state_id,
        Some(MeleeActionStateId::new(216)),
        "ftCo_CatchPull_Anim should call fn_800DA1D8 when throw_flags_b3 is set"
    );
    assert_eq!(
        players[0].source_action_key,
        Some(SourceActionKey::new("CatchWait"))
    );
    assert_eq!(players[0].motion_frame, 0);
    assert_eq!(players[0].motion_throw_flags & (1 << 3), 0);
    assert_eq!(players[0].ground_velocity_x.to_bits(), 0.0_f32.to_bits());
    assert_eq!(
        players[0].source_self_velocity_x.to_bits(),
        0.0_f32.to_bits()
    );
    assert_eq!(players[0].velocity.x, 0);

    assert_eq!(
        players[1].melee_action_state_id,
        Some(MeleeActionStateId::new(227)),
        "fn_800DA1D8 should route the victim through fn_800DB6C8 -> CaptureWaitLw"
    );
    assert_eq!(
        players[1].source_action_key,
        Some(SourceActionKey::new("CaptureWaitLw"))
    );
    assert_eq!(players[1].motion_frame, 0);
    assert_eq!(players[1].source_victim_index, Some(0));
}

#[test]
fn source_catch_pull_transition_refreshes_capture_anchor_before_victim_phys() {
    let mut world = World::for_two_players();
    let mut thrower = world.players()[0];
    thrower.motion_state_alias = None;
    thrower.motion_state = MotionState::Catch;
    thrower.melee_action_state_id = Some(MeleeActionStateId::new(213));
    thrower.source_action_key = Some(SourceActionKey::new("Catch"));
    thrower.source_action_total_frames = 30;
    thrower.source_victim_index = Some(1);
    thrower.source_x1a5c_index = Some(1);
    thrower.source_x221b_b5 = true;
    thrower.grounded = true;
    thrower.source_position = SourceVec2 { x: 10.0, y: 0.0 };
    thrower.position = thrower.source_position.to_milli();
    thrower.motion_throw_flags = 1 << 3;
    assert!(world.set_player_state_for_diagnostic(0, thrower));

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

    step_world_with_source_runtime_data(
        &mut world,
        Frame(1),
        &[
            PlayerInput::neutral(),
            PlayerInput::neutral().with_left_stick(81, -97),
        ],
        |player| {
            let source_action_key = player.source_action_key?;
            let capture_pose = match source_action_key.as_str() {
                "Catch" => SourceCapturePose {
                    capture_anchor: SourcePosePoint {
                        x: 9.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    ..SourceCapturePose::default()
                },
                "CatchWait" => SourceCapturePose {
                    capture_anchor: SourcePosePoint {
                        x: 3.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    ..SourceCapturePose::default()
                },
                "CapturePulledLw" => SourceCapturePose {
                    xrotn: SourcePosePoint::ZERO,
                    ..SourceCapturePose::default()
                },
                _ => return None,
            };
            Some(SourceActionPoseMetadata {
                capture_pose: Some(capture_pose),
                ..SourceActionPoseMetadata::default()
            })
        },
        |action_state_id| match action_state_id.get() {
            213 => Some(30),
            216 => Some(60),
            226 => Some(20),
            227 => Some(35),
            _ => None,
        },
    );

    let players = world.players();
    assert_eq!(
        players[0].melee_action_state_id,
        Some(MeleeActionStateId::new(216))
    );
    assert_eq!(
        players[1].melee_action_state_id,
        Some(MeleeActionStateId::new(227))
    );
    assert!((players[1].source_position.x - 13.0).abs() <= 0.000_01,
        "fn_800DA1D8 stores the new CatchWait capture joint before the later CapturePulledLw_Phys joint delta; victim_x={:.6}",
        players[1].source_position.x
    );
    assert_eq!(players[1].position, players[1].source_position.to_milli());
}

#[test]
fn source_catch_pull_baked_throw_flag_event_enters_catch_wait() {
    let mut world = World::for_two_players();
    let mut thrower = world.players()[0];
    thrower.motion_state_alias = None;
    thrower.motion_state = MotionState::Catch;
    thrower.melee_action_state_id = Some(MeleeActionStateId::new(213));
    thrower.source_action_key = Some(SourceActionKey::new("Catch"));
    thrower.source_action_total_frames = 30;
    thrower.source_victim_index = Some(1);
    thrower.source_x1a5c_index = Some(1);
    thrower.source_x221b_b5 = true;
    thrower.grounded = true;
    assert!(world.set_player_state_for_diagnostic(0, thrower));

    let mut victim = world.players()[1];
    victim.motion_state_alias = None;
    victim.motion_state = MotionState::GuardOn;
    victim.melee_action_state_id = Some(MeleeActionStateId::new(226));
    victim.source_action_key = Some(SourceActionKey::new("CapturePulledLw"));
    victim.source_action_total_frames = 20;
    victim.source_victim_index = Some(0);
    victim.source_x1a5c_index = Some(0);
    assert!(world.set_player_state_for_diagnostic(1, victim));

    step_world_with_source_runtime_data(
        &mut world,
        Frame(1),
        &[PlayerInput::neutral(); 2],
        |player| {
            if player.source_action_key == Some(SourceActionKey::new("Catch")) {
                return Some(SourceActionPoseMetadata {
                    script_events: SourceActionScriptEvents::single(
                        SourceActionScriptEvent::SetThrowFlag {
                            hit_idx: 0,
                            flag_bit: Some(3),
                        },
                    ),
                    ..SourceActionPoseMetadata::default()
                });
            }
            None
        },
        |action_state_id| match action_state_id.get() {
            213 => Some(30),
            216 => Some(60),
            226 => Some(20),
            227 => Some(35),
            _ => None,
        },
    );

    let players = world.players();
    assert_eq!(
        players[0].melee_action_state_id,
        Some(MeleeActionStateId::new(216)),
        "ftAction_800718A4 SetThrowFlag(hit_idx=0) should set throw_flags_b3 before CatchPull_Anim checks it"
    );
    assert_eq!(players[0].motion_throw_flags & (1 << 3), 0);
    assert_eq!(
        players[1].melee_action_state_id,
        Some(MeleeActionStateId::new(227))
    );
}

#[test]
fn source_set_throw_hitbox_event_populates_persistent_xdf4_state() {
    let mut world = World::for_two_players();
    let mut thrower = world.players()[0];
    thrower.motion_state_alias = None;
    thrower.motion_state = MotionState::GuardOn;
    thrower.melee_action_state_id = Some(MeleeActionStateId::new(221));
    thrower.source_action_key = Some(SourceActionKey::new("ThrowHi"));
    thrower.source_action_total_frames = 45;
    thrower.source_victim_index = Some(1);
    assert!(world.set_player_state_for_diagnostic(0, thrower));

    let source_throw_hitbox = SourceThrowHitboxAttributes {
        hitbox_idx: 1,
        damage: 13,
        angle: 90,
        hit_x24: 100,
        hit_x28: 30,
        hit_x2c: 45,
        element: 3,
        sfx_severity: 5,
        sfx_kind: 7,
    };

    step_world_with_source_runtime_data(
        &mut world,
        Frame(1),
        &[PlayerInput::neutral(); 2],
        |player| {
            if player.source_action_key == Some(SourceActionKey::new("ThrowHi")) {
                return Some(SourceActionPoseMetadata {
                    script_events: SourceActionScriptEvents::single(
                        SourceActionScriptEvent::SetThrowHitbox(source_throw_hitbox),
                    ),
                    ..SourceActionPoseMetadata::default()
                });
            }
            None
        },
        |action_state_id| match action_state_id.get() {
            221 => Some(45),
            _ => None,
        },
    );

    assert_eq!(
        world.players()[0].source_throw_hitboxes[1].map(|hitbox| hitbox.hitbox),
        Some(source_throw_hitbox),
        "ftAction_80071E04 writes raw throw hitbox fields into the installed fp->xDF4 capsule"
    );
    assert_eq!(
        world.players()[0].source_throw_hitboxes[1].map(|hitbox| hitbox.damage.to_bits()),
        Some((source_throw_hitbox.damage as f32).to_bits()),
        "fresh xDF4 install keeps ftColl_8007ABD0 HitCapsule.damage as f32"
    );
}

#[test]
fn source_capture_pulled_lw_applies_decomp_capture_joint_delta() {
    let common = MeleeCommonData {
        capture_pulled_high_delta_y: 10.0,
        ..MeleeCommonData::PROVISIONAL
    };
    let mut world = World::for_two_players_on_stage_with_profiles_and_common_data(
        StageProfile::battlefield(),
        [FighterProfile::falcon_like(); 2],
        common,
    );
    let mut grabber = world.players()[0];
    grabber.motion_state_alias = None;
    grabber.motion_state = MotionState::Catch;
    grabber.melee_action_state_id = Some(MeleeActionStateId::new(216));
    grabber.source_action_key = Some(SourceActionKey::new("CatchWait"));
    grabber.source_action_total_frames = 30;
    grabber.source_victim_index = Some(1);
    grabber.source_x1a5c_index = Some(1);
    grabber.source_position = SourceVec2 { x: 10.0, y: 5.0 };
    grabber.position = grabber.source_position.to_milli();
    assert!(world.set_player_state_for_diagnostic(0, grabber));

    let mut victim = world.players()[1];
    victim.motion_state_alias = None;
    victim.motion_state = MotionState::GuardOn;
    victim.melee_action_state_id = Some(MeleeActionStateId::new(226));
    victim.source_action_key = Some(SourceActionKey::new("CapturePulledLw"));
    victim.source_action_total_frames = 20;
    victim.source_victim_index = Some(0);
    victim.source_x1a5c_index = Some(0);
    victim.source_position = SourceVec2 { x: 20.0, y: -1.0 };
    victim.position = victim.source_position.to_milli();
    assert!(world.set_player_state_for_diagnostic(1, victim));

    step_world_with_source_runtime_data(
        &mut world,
        Frame(1),
        &[PlayerInput::neutral(); 2],
        |player| {
            let source_action_key = player.source_action_key?;
            let capture_pose = match source_action_key.as_str() {
                "CatchWait" => SourceCapturePose {
                    capture_anchor: SourcePosePoint {
                        x: 3.25,
                        y: 4.5,
                        z: 0.0,
                    },
                    xrotn: SourcePosePoint::ZERO,
                    ..SourceCapturePose::default()
                },
                "CapturePulledLw" => SourceCapturePose {
                    capture_anchor: SourcePosePoint::ZERO,
                    xrotn: SourcePosePoint {
                        x: -2.0,
                        y: 1.5,
                        z: 0.0,
                    },
                    ..SourceCapturePose::default()
                },
                _ => return None,
            };
            Some(SourceActionPoseMetadata {
                capture_pose: Some(capture_pose),
                ..SourceActionPoseMetadata::default()
            })
        },
        |action_state_id| match action_state_id.get() {
            216 => Some(30),
            226 => Some(20),
            _ => None,
        },
    );

    let victim = world.players()[1];
    assert!(
        (victim.source_position.x - 11.25).abs() <= 0.000_01,
        "ftCo_CapturePulledLw_Phys samples both capturedamage.x18 and XRotN through lb_8000B1CC before applying the source delta"
    );
    assert!((victim.source_position.y - 8.0).abs() <= 0.000_01);
    assert_eq!(victim.position, victim.source_position.to_milli());
}

#[test]
fn source_capture_pulled_lw_mirrors_decomp_capture_joint_delta_for_left_facing_fighters() {
    let mut world = World::for_two_players();
    let mut grabber = world.players()[0];
    grabber.motion_state_alias = None;
    grabber.motion_state = MotionState::Catch;
    grabber.melee_action_state_id = Some(MeleeActionStateId::new(216));
    grabber.source_action_key = Some(SourceActionKey::new("CatchWait"));
    grabber.source_action_total_frames = 30;
    grabber.source_victim_index = Some(1);
    grabber.source_x1a5c_index = Some(1);
    grabber.facing = -1;
    grabber.source_position = SourceVec2 { x: 10.0, y: 5.0 };
    grabber.position = grabber.source_position.to_milli();
    assert!(world.set_player_state_for_diagnostic(0, grabber));

    let mut victim = world.players()[1];
    victim.motion_state_alias = None;
    victim.motion_state = MotionState::GuardOn;
    victim.melee_action_state_id = Some(MeleeActionStateId::new(226));
    victim.source_action_key = Some(SourceActionKey::new("CapturePulledLw"));
    victim.source_action_total_frames = 20;
    victim.source_victim_index = Some(0);
    victim.source_x1a5c_index = Some(0);
    victim.facing = -1;
    victim.source_position = SourceVec2 { x: 20.0, y: -1.0 };
    victim.position = victim.source_position.to_milli();
    assert!(world.set_player_state_for_diagnostic(1, victim));

    step_world_with_source_runtime_data(
        &mut world,
        Frame(1),
        &[PlayerInput::neutral(); 2],
        |player| {
            let source_action_key = player.source_action_key?;
            let capture_pose = match source_action_key.as_str() {
                "CatchWait" => SourceCapturePose {
                    capture_anchor: SourcePosePoint {
                        x: 3.25,
                        y: 4.5,
                        z: 0.0,
                    },
                    ..SourceCapturePose::default()
                },
                "CapturePulledLw" => SourceCapturePose {
                    xrotn: SourcePosePoint {
                        x: -2.0,
                        y: 1.5,
                        z: 0.0,
                    },
                    ..SourceCapturePose::default()
                },
                _ => return None,
            };
            Some(SourceActionPoseMetadata {
                capture_pose: Some(capture_pose),
                ..SourceActionPoseMetadata::default()
            })
        },
        |action_state_id| match action_state_id.get() {
            216 => Some(30),
            226 => Some(20),
            _ => None,
        },
    );

    let victim = world.players()[1];
    assert!((victim.source_position.x - 4.75).abs() <= 0.000_01,
        "lb_8000B1CC samples the grabber's faced capturedamage JObj world position before subtracting the victim's faced XRotN JObj world sample"
    );
    assert!((victim.source_position.y - 8.0).abs() <= 0.000_01);
    assert_eq!(victim.position, victim.source_position.to_milli());
}

#[test]
fn source_capture_pulled_lw_faces_both_capturedamage_and_victim_xrotn_joints() {
    let mut world = World::for_two_players();
    let mut grabber = world.players()[0];
    grabber.motion_state_alias = None;
    grabber.motion_state = MotionState::Catch;
    grabber.melee_action_state_id = Some(MeleeActionStateId::new(216));
    grabber.source_action_key = Some(SourceActionKey::new("CatchWait"));
    grabber.source_action_total_frames = 30;
    grabber.source_victim_index = Some(1);
    grabber.source_x1a5c_index = Some(1);
    grabber.facing = -1;
    grabber.source_position = SourceVec2 { x: 10.0, y: 5.0 };
    grabber.position = grabber.source_position.to_milli();
    assert!(world.set_player_state_for_diagnostic(0, grabber));

    let mut victim = world.players()[1];
    victim.motion_state_alias = None;
    victim.motion_state = MotionState::GuardOn;
    victim.melee_action_state_id = Some(MeleeActionStateId::new(226));
    victim.source_action_key = Some(SourceActionKey::new("CapturePulledLw"));
    victim.source_action_total_frames = 20;
    victim.source_victim_index = Some(0);
    victim.source_x1a5c_index = Some(0);
    victim.facing = -1;
    victim.source_position = SourceVec2 { x: 20.0, y: -1.0 };
    victim.position = victim.source_position.to_milli();
    assert!(world.set_player_state_for_diagnostic(1, victim));

    step_world_with_source_runtime_data(
        &mut world,
        Frame(1),
        &[PlayerInput::neutral(); 2],
        |player| {
            let source_action_key = player.source_action_key?;
            let capture_pose = match source_action_key.as_str() {
                "CatchWait" => SourceCapturePose {
                    capture_anchor: SourcePosePoint {
                        x: 3.25,
                        y: 4.5,
                        z: 0.0,
                    },
                    ..SourceCapturePose::default()
                },
                "CapturePulledLw" => SourceCapturePose {
                    xrotn: SourcePosePoint {
                        x: -2.0,
                        y: 1.5,
                        z: 0.0,
                    },
                    ..SourceCapturePose::default()
                },
                _ => return None,
            };
            Some(SourceActionPoseMetadata {
                capture_pose: Some(capture_pose),
                ..SourceActionPoseMetadata::default()
            })
        },
        |action_state_id| match action_state_id.get() {
            216 => Some(30),
            226 => Some(20),
            _ => None,
        },
    );

    let victim = world.players()[1];
    assert!(
        (victim.source_position.x - 4.75).abs() <= 0.000_01,
        "ftCo_CapturePulledLw_Phys samples both JObjs through lb_8000B1CC, so the victim XRotN sample must also be mirrored by the victim model facing; victim_x={:.6}",
        victim.source_position.x
    );
    assert!((victim.source_position.y - 8.0).abs() <= 0.000_01);
    assert_eq!(victim.position, victim.source_position.to_milli());
}

#[test]
fn source_capture_joint_delta_samples_posed_jobj_world_facing() {
    let mut world = World::for_two_players();
    let mut grabber = world.players()[0];
    grabber.motion_state_alias = None;
    grabber.motion_state = MotionState::TurnRun;
    grabber.melee_action_state_id = Some(MeleeActionStateId::new(216));
    grabber.source_action_key = Some(SourceActionKey::new("CatchWait"));
    grabber.source_action_total_frames = 30;
    grabber.source_victim_index = Some(1);
    grabber.source_x1a5c_index = Some(1);
    grabber.facing = -1;
    grabber.turn_run_accel_mul = 1;
    grabber.source_position = SourceVec2 { x: 10.0, y: 5.0 };
    grabber.position = grabber.source_position.to_milli();
    assert!(world.set_player_state_for_diagnostic(0, grabber));

    let mut victim = world.players()[1];
    victim.motion_state_alias = None;
    victim.motion_state = MotionState::GuardOn;
    victim.melee_action_state_id = Some(MeleeActionStateId::new(226));
    victim.source_action_key = Some(SourceActionKey::new("CapturePulledLw"));
    victim.source_action_total_frames = 20;
    victim.source_victim_index = Some(0);
    victim.source_x1a5c_index = Some(0);
    victim.source_position = SourceVec2 { x: 20.0, y: -1.0 };
    victim.position = victim.source_position.to_milli();
    assert!(world.set_player_state_for_diagnostic(1, victim));

    step_world_with_source_runtime_data(
        &mut world,
        Frame(1),
        &[PlayerInput::neutral(); 2],
        |player| {
            let source_action_key = player.source_action_key?;
            let capture_pose = match source_action_key.as_str() {
                "CatchWait" => SourceCapturePose {
                    capture_anchor: SourcePosePoint {
                        x: 3.25,
                        y: 4.5,
                        z: 0.0,
                    },
                    ..SourceCapturePose::default()
                },
                "CapturePulledLw" => SourceCapturePose {
                    xrotn: SourcePosePoint {
                        x: -2.0,
                        y: 1.5,
                        z: 0.0,
                    },
                    ..SourceCapturePose::default()
                },
                _ => return None,
            };
            Some(SourceActionPoseMetadata {
                capture_pose: Some(capture_pose),
                ..SourceActionPoseMetadata::default()
            })
        },
        |action_state_id| match action_state_id.get() {
            216 => Some(30),
            226 => Some(20),
            _ => None,
        },
    );

    let victim = world.players()[1];
    assert!(
        (victim.source_position.x - 11.25).abs() <= 0.000_01,
        "ftCo_CapturePulledLw_Phys samples both JObj world positions with lb_8000B1CC before applying the capture delta; victim_x={:.6}",
        victim.source_position.x
    );
    assert!((victim.source_position.y - 8.0).abs() <= 0.000_01);
    assert_eq!(victim.position, victim.source_position.to_milli());
}

#[test]
fn source_capture_pulled_lw_samples_post_anim_callback_jobj_pose() {
    let mut world = World::for_two_players();
    let mut grabber = world.players()[0];
    grabber.motion_state_alias = None;
    grabber.motion_state = MotionState::Catch;
    grabber.melee_action_state_id = Some(MeleeActionStateId::new(216));
    grabber.source_action_key = Some(SourceActionKey::new("CatchWait"));
    grabber.source_action_total_frames = 30;
    grabber.source_victim_index = Some(1);
    grabber.source_x1a5c_index = Some(1);
    grabber.source_position = SourceVec2 { x: 10.0, y: 0.0 };
    grabber.position = grabber.source_position.to_milli();
    grabber.motion_anim_frame_milli = 0;
    grabber.motion_anim_rate_milli = 1_000;
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
    victim.motion_anim_frame_milli = 0;
    victim.motion_anim_rate_milli = 1_000;
    assert!(world.set_player_state_for_diagnostic(1, victim));

    step_world_with_source_runtime_data(
        &mut world,
        Frame(1),
        &[PlayerInput::neutral(); 2],
        |player| {
            let source_action_key = player.source_action_key?;
            let capture_pose = match source_action_key.as_str() {
                "CatchWait" => SourceCapturePose {
                    capture_anchor: SourcePosePoint {
                        x: if player.motion_anim_frame_milli >= 1_000 {
                            3.0
                        } else {
                            0.0
                        },
                        y: 0.0,
                        z: 0.0,
                    },
                    ..SourceCapturePose::default()
                },
                "CapturePulledLw" => SourceCapturePose {
                    xrotn: SourcePosePoint::ZERO,
                    ..SourceCapturePose::default()
                },
                _ => return None,
            };
            Some(SourceActionPoseMetadata {
                capture_pose: Some(capture_pose),
                ..SourceActionPoseMetadata::default()
            })
        },
        |action_state_id| match action_state_id.get() {
            216 => Some(30),
            226 => Some(20),
            _ => None,
        },
    );

    let victim = world.players()[1];
    assert!((victim.source_position.x - 13.0).abs() <= 0.000_01,
        "ftCo_CapturePulledLw_Phys samples live JObj world positions after the action animation callback has advanced the current pose"
    );
    assert_eq!(victim.position, victim.source_position.to_milli());
}

#[test]
fn source_capture_pulled_lw_runs_source_ground_collision_after_joint_delta() {
    let mut world = World::for_two_players_on_stage(StageProfile::battlefield());
    let mut grabber = world.players()[0];
    grabber.motion_state_alias = None;
    grabber.motion_state = MotionState::Catch;
    grabber.melee_action_state_id = Some(MeleeActionStateId::new(216));
    grabber.source_action_key = Some(SourceActionKey::new("CatchWait"));
    grabber.source_action_total_frames = 30;
    grabber.source_victim_index = Some(1);
    grabber.source_x1a5c_index = Some(1);
    grabber.source_position = SourceVec2 { x: 10.0, y: 0.0 };
    grabber.position = grabber.source_position.to_milli();
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
    victim.source_position = SourceVec2 { x: 10.0, y: 0.0 };
    victim.position = victim.source_position.to_milli();
    victim.set_source_floor_for_diagnostic(
        Some(0),
        Some(source_floor_line_for_surface_at_x(
            StageProfile::battlefield(),
            StageProfile::battlefield().main_floor,
            victim.source_position.x,
        )),
    );
    assert!(world.set_player_state_for_diagnostic(1, victim));

    step_world_with_source_runtime_data(
        &mut world,
        Frame(1),
        &[PlayerInput::neutral(); 2],
        |player| {
            let source_action_key = player.source_action_key?;
            let capture_pose = match source_action_key.as_str() {
                "CatchWait" => SourceCapturePose {
                    capture_anchor: SourcePosePoint {
                        x: 0.0,
                        y: -4.0,
                        z: 0.0,
                    },
                    xrotn: SourcePosePoint::ZERO,
                    ..SourceCapturePose::default()
                },
                "CapturePulledLw" => SourceCapturePose {
                    capture_anchor: SourcePosePoint::ZERO,
                    xrotn: SourcePosePoint::ZERO,
                    ..SourceCapturePose::default()
                },
                _ => return None,
            };
            Some(SourceActionPoseMetadata {
                capture_pose: Some(capture_pose),
                ..SourceActionPoseMetadata::default()
            })
        },
        |action_state_id| match action_state_id.get() {
            216 => Some(30),
            226 => Some(20),
            _ => None,
        },
    );

    let victim = world.players()[1];
    assert_eq!(
        victim.melee_action_state_id,
        Some(MeleeActionStateId::new(226))
    );
    assert!((victim.source_position.y - 0.0001).abs() <= 0.000_01,
        "ftCo_CapturePulledLw_Coll calls ft_8008403C, whose ft_80082708/mpColl_8004B108 path must reconcile grounded capture deltas against source floor collision"
    );
    assert_eq!(victim.position.y, 0);
}

#[test]
fn source_only_thrown_states_bind_to_captain_victim_throw_actions() {
    let expected = [
        (239, 262, "TCaptainThrowF"),
        (240, 263, "TCaptainThrowB"),
        (241, 264, "TCaptainThrowHi"),
        (242, 265, "TCaptainThrowLw"),
    ];

    for (runtime_id, source_table_id, source_key) in expected {
        let binding = CANONICAL_SOURCE_ONLY_ACTION_BINDINGS
            .iter()
            .find(|binding| binding.action_state_id == MeleeActionStateId::new(runtime_id))
            .copied()
            .expect("ftCo_MS_Thrown* runtime ids should bind to source victim throw actions");
        assert_eq!(binding.source_action_table_id, source_table_id);
        assert_eq!(binding.source_action_key, SourceActionKey::new(source_key));
    }
}

fn configure_source_catch_wait_throw_pair(world: &mut World, facing: i8) {
    let mut thrower = world.players()[0];
    thrower.motion_state_alias = None;
    thrower.motion_state = MotionState::Catch;
    thrower.melee_action_state_id = Some(MeleeActionStateId::new(216));
    thrower.source_action_key = Some(SourceActionKey::new("CatchWait"));
    thrower.source_action_total_frames = 30;
    thrower.source_victim_index = Some(1);
    thrower.source_x1a5c_index = Some(1);
    thrower.source_x221b_b5 = true;
    thrower.facing = facing;
    thrower.motion_cmd_var0 = 0x55;
    thrower.motion_throw_flags = 0xff;
    assert!(world.set_player_state_for_diagnostic(0, thrower));

    let mut victim = world.players()[1];
    victim.motion_state_alias = None;
    victim.motion_state = MotionState::GuardOn;
    victim.melee_action_state_id = Some(MeleeActionStateId::new(227));
    victim.source_action_key = Some(SourceActionKey::new("CaptureWaitLw"));
    victim.source_action_total_frames = 35;
    victim.source_victim_index = Some(0);
    victim.source_x1a5c_index = Some(0);
    victim.source_x221b_b5 = false;
    victim.source_grab_timer = 120.0;
    victim.facing = -facing;
    victim.motion_anim_frame_milli = 7_000;
    assert!(world.set_player_state_for_diagnostic(1, victim));
}

fn step_source_catch_wait_throw_selection(world: &mut World, frame: u32, input: PlayerInput) {
    step_world_with_source_runtime_data(
        world,
        Frame(frame),
        &[input, PlayerInput::neutral()],
        |_| None,
        |action_state_id| match action_state_id.get() {
            216 => Some(30),
            219 => Some(38),
            220 => Some(52),
            221 => Some(44),
            222 => Some(49),
            227 => Some(35),
            239 => Some(24),
            240 => Some(18),
            241 => Some(15),
            242 => Some(49),
            _ => None,
        },
    );
}

fn assert_source_throw_selected(
    world: &World,
    throw_state_id: u16,
    throw_key: &'static str,
    victim_state_id: u16,
    victim_key: &'static str,
) {
    let players = world.players();
    assert_eq!(
        players[0].melee_action_state_id,
        Some(MeleeActionStateId::new(throw_state_id))
    );
    assert_eq!(
        players[0].source_action_key,
        Some(SourceActionKey::new(throw_key))
    );
    assert_eq!(players[0].motion_state_alias, None);
    assert_eq!(players[0].motion_frame, 0);
    assert_eq!(players[0].motion_cmd_var0, 0);
    assert_eq!(players[0].motion_throw_flags, 0);
    assert_eq!(players[0].source_victim_index, Some(1));

    assert_eq!(
        players[1].melee_action_state_id,
        Some(MeleeActionStateId::new(victim_state_id)),
        "ftCo_800DD4B0 should route victim_msid = throw_index + 239"
    );
    assert_eq!(
        players[1].source_action_key,
        Some(SourceActionKey::new(victim_key))
    );
    assert_eq!(players[1].motion_state_alias, None);
    assert_eq!(players[1].motion_frame, 0);
    assert_eq!(players[1].source_victim_index, Some(0));
    assert_eq!(players[1].facing, players[0].facing);
}

#[test]
fn source_catch_wait_attack_press_enters_catch_attack_like_fn_800da4c0() {
    let mut world = World::for_two_players();
    configure_source_catch_wait_throw_pair(&mut world, 1);
    step_source_catch_wait_throw_selection(&mut world, 1, PlayerInput::neutral().with_attack(true));

    let players = world.players();
    assert_eq!(
        players[0].melee_action_state_id,
        Some(MeleeActionStateId::new(217)),
        "ftCo_CatchWait_IASA should call fn_800DA4FC when input.x668 has the A press bit"
    );
    assert_eq!(
        players[0].source_action_key,
        Some(SourceActionKey::new("CatchAttack"))
    );
    assert_eq!(players[0].motion_state_alias, None);
    assert_eq!(players[0].motion_frame, 0);
    assert_eq!(players[0].motion_anim_frame_milli, 0);
    assert_eq!(players[0].ground_velocity_x.to_bits(), 0.0_f32.to_bits());
    assert_eq!(players[0].source_victim_index, Some(1));
    assert_eq!(
        players[1].melee_action_state_id,
        Some(MeleeActionStateId::new(227)),
        "pummel entry keeps the victim in CaptureWaitLw rather than a throw state"
    );
    assert_eq!(players[1].source_victim_index, Some(0));
}

#[test]
fn source_catch_attack_anim_end_returns_to_catch_wait_like_fn_800da2b0() {
    let mut world = World::for_two_players();
    let mut thrower = world.players()[0];
    thrower.motion_state_alias = None;
    thrower.motion_state = MotionState::Catch;
    thrower.melee_action_state_id = Some(MeleeActionStateId::new(217));
    thrower.source_action_key = Some(SourceActionKey::new("CatchAttack"));
    thrower.source_action_total_frames = 24;
    thrower.source_victim_index = Some(1);
    thrower.source_x1a5c_index = Some(1);
    thrower.source_x221b_b5 = true;
    thrower.motion_frame = 23;
    thrower.set_source_motion_anim_frame(23.0);
    assert!(world.set_player_state_for_diagnostic(0, thrower));

    let mut victim = world.players()[1];
    victim.motion_state_alias = None;
    victim.motion_state = MotionState::GuardOn;
    victim.melee_action_state_id = Some(MeleeActionStateId::new(227));
    victim.source_action_key = Some(SourceActionKey::new("CaptureWaitLw"));
    victim.source_action_total_frames = 35;
    victim.source_victim_index = Some(0);
    victim.source_x1a5c_index = Some(0);
    victim.source_x221b_b5 = false;
    assert!(world.set_player_state_for_diagnostic(1, victim));

    step_world_with_source_runtime_data(
        &mut world,
        Frame(1),
        &[PlayerInput::neutral(); 2],
        |_| None,
        |action_state_id| match action_state_id.get() {
            217 => Some(24),
            227 => Some(35),
            216 => Some(30),
            _ => None,
        },
    );

    let players = world.players();
    assert_eq!(
        players[0].melee_action_state_id,
        Some(MeleeActionStateId::new(216))
    );
    assert_eq!(
        players[0].source_action_key,
        Some(SourceActionKey::new("CatchWait"))
    );
    assert_eq!(players[0].motion_frame, 0);
    assert_eq!(players[0].motion_anim_frame_milli, 0);
    assert_eq!(players[0].source_victim_index, Some(1));
    assert_eq!(
        players[1].melee_action_state_id,
        Some(MeleeActionStateId::new(227))
    );
}

#[test]
fn source_catch_attack_hitbox_damages_held_victim_without_throw_release() {
    let mut world = World::for_two_players();
    let mut thrower = world.players()[0];
    thrower.motion_state_alias = None;
    thrower.motion_state = MotionState::Catch;
    thrower.melee_action_state_id = Some(MeleeActionStateId::new(217));
    thrower.source_action_key = Some(SourceActionKey::new("CatchAttack"));
    thrower.source_action_total_frames = 24;
    thrower.source_victim_index = Some(1);
    thrower.source_x1a5c_index = Some(1);
    assert!(world.set_player_state_for_diagnostic(0, thrower));

    let mut victim = world.players()[1];
    victim.motion_state_alias = None;
    victim.motion_state = MotionState::GuardOn;
    victim.melee_action_state_id = Some(MeleeActionStateId::new(227));
    victim.source_action_key = Some(SourceActionKey::new("CaptureWaitLw"));
    victim.source_action_total_frames = 35;
    victim.source_victim_index = Some(0);
    victim.source_x1a5c_index = Some(0);
    victim.source_x2226_b2 = true;
    assert!(world.set_player_state_for_diagnostic(1, victim));

    let hitbox = SourceHitboxAttributes {
        bone: 8,
        hit_group: 0,
        damage: 3,
        angle: 361,
        knockback_growth: 20,
        weight_set_knockback: 0,
        base_knockback: 0,
        element: 0,
        shield_damage: 0,
        hit_grounded: true,
        hit_aerial: true,
    };
    let collision_frame = SourceCollisionFrame {
        hits: vec![SourceCollisionCapsule::new(
            0,
            1,
            Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 2.0),
        )
        .with_source_pose(
            Some(MeleeActionStateId::new(217)),
            Some(SourceActionKey::new("CatchAttack")),
            4,
        )
        .with_hitbox_lifecycle(SourceHitboxLifecycleId::new(4_u64 << 32))
        .with_hitbox_attributes(hitbox)],
        hurts: vec![SourceCollisionCapsule::new(
            1,
            10,
            Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 2.0),
        )],
    };

    let step = world.apply_source_collision_frame(&collision_frame);

    assert_eq!(step.confirms.len(), 1);
    assert_eq!(step.stages.len(), 1);
    assert_eq!(
        step.results.len(),
        0,
        "ftCo_CatchAttack pummel should stage held-victim damage without routing the captured victim through normal damage knockback"
    );
    assert_eq!(step.applied_stage_count, 1);
    assert_eq!(world.apply_source_damage_results(&step.results), 0);
    assert_eq!(world.players()[1].damage_applied, 3);
    assert_eq!(world.commit_staged_source_damage(), 1);
    assert_eq!(world.players()[1].damage_percent, 3.0);
    assert_eq!(
        world.players()[1].melee_action_state_id,
        Some(MeleeActionStateId::new(227)),
        "CatchAttack pummel damage should not release the victim from CaptureWaitLw"
    );
    assert_eq!(world.players()[1].source_knockback_velocity_x, 0.0);
    assert_eq!(world.players()[1].source_knockback_velocity_y, 0.0);
    assert_eq!(world.players()[1].source_ground_knockback_velocity, 0.0);
    assert!(
        !world.players()[1].source_allow_sdi,
        "ftCo_CatchAttack stages pummel damage on the held victim without normal hitlag SDI"
    );
    assert_eq!(world.players()[0].source_victim_index, Some(1));
    assert_eq!(world.players()[1].source_victim_index, Some(0));
}

#[test]
fn source_capture_wait_natural_timer_releases_like_ftco_capturewait_anim() {
    let common = MeleeCommonData {
        grab_timer_decrement: 1.0,
        grab_mash_timer_decrement: 8.0,
        ..MeleeCommonData::PROVISIONAL
    };
    let mut world = World::for_two_players_with_common_data(common);
    configure_source_catch_wait_throw_pair(&mut world, 1);
    let mut victim = world.players()[1];
    victim.source_grab_timer = 1.0;
    assert!(world.set_player_state_for_diagnostic(1, victim));

    step_world_with_source_runtime_data(
        &mut world,
        Frame(1),
        &[PlayerInput::neutral(); 2],
        |_| None,
        |action_state_id| match action_state_id.get() {
            216 => Some(30),
            218 => Some(20),
            227 => Some(35),
            229 => Some(18),
            _ => None,
        },
    );

    let players = world.players();
    assert_eq!(
        players[0].melee_action_state_id,
        Some(MeleeActionStateId::new(218)),
        "ftCo_CaptureWaitHi_Anim calls ftCo_800DA698(grabber, false) when grab_timer expires"
    );
    assert_eq!(
        players[0].source_action_key,
        Some(SourceActionKey::new("CatchCut"))
    );
    assert_eq!(
        players[1].melee_action_state_id,
        Some(MeleeActionStateId::new(229)),
        "normal timeout should route the victim through ftCo_CaptureCut_Enter"
    );
    assert_eq!(
        players[1].source_action_key,
        Some(SourceActionKey::new("CaptureCut"))
    );
    assert_eq!(players[0].source_victim_index, None);
    assert_eq!(players[1].source_victim_index, None);
}

#[test]
fn source_capture_wait_xy_window_and_tap_jump_release_enter_capture_jump() {
    let common = MeleeCommonData {
        grab_timer_decrement: 1.0,
        capture_wait_jump_input_window: 16.0,
        capture_jump_velocity_x: 1.0,
        capture_jump_velocity_y: 2.0,
        ..MeleeCommonData::PROVISIONAL
    };
    for (first_input, release_input) in [
        (
            PlayerInput::neutral().with_jump(true),
            PlayerInput::neutral(),
        ),
        (
            PlayerInput::neutral(),
            PlayerInput::neutral().with_left_stick(0, 90),
        ),
    ] {
        let mut world = World::for_two_players_with_common_data(common);
        configure_source_catch_wait_throw_pair(&mut world, 1);
        let mut victim = world.players()[1];
        victim.source_grab_timer = 2.0;
        assert!(world.set_player_state_for_diagnostic(1, victim));

        for (frame, input) in [(1, first_input), (2, release_input)] {
            step_world_with_source_runtime_data(
                &mut world,
                Frame(frame),
                &[PlayerInput::neutral(), input],
                |_| None,
                |action_state_id| match action_state_id.get() {
                    216 => Some(30),
                    218 => Some(20),
                    227 => Some(35),
                    228 => Some(20),
                    _ => None,
                },
            );
        }

        let players = world.players();
        assert_eq!(
            players[1].melee_action_state_id,
            Some(MeleeActionStateId::new(228)),
            "fn_800DC070 should route expired CaptureWait through CaptureJump for early X/Y or release-frame tap jump"
        );
        assert_eq!(
            players[1].source_action_key,
            Some(SourceActionKey::new("CaptureJump"))
        );
        assert_eq!(players[1].source_self_velocity_x, -players[1].facing as f32);
        assert_eq!(players[1].source_self_velocity_y, 2.0);
        assert_eq!(
            players[0].source_action_key,
            Some(SourceActionKey::new("CatchCut"))
        );
    }
}

#[test]
fn source_capture_wait_mash_decrements_timer_like_ftcommon_grabmash() {
    let common = MeleeCommonData {
        grab_timer_decrement: 1.0,
        grab_mash_timer_decrement: 8.0,
        grab_mash_stick_threshold: 32,
        ..MeleeCommonData::PROVISIONAL
    };
    let mut neutral = World::for_two_players_with_common_data(common);
    configure_source_catch_wait_throw_pair(&mut neutral, 1);
    let mut mashed = neutral.clone();
    for world in [&mut neutral, &mut mashed] {
        let mut victim = world.players()[1];
        victim.source_grab_timer = 120.0;
        assert!(world.set_player_state_for_diagnostic(1, victim));
    }

    for frame in 1..=2 {
        step_world_with_source_runtime_data(
            &mut neutral,
            Frame(frame),
            &[PlayerInput::neutral(); 2],
            |_| None,
            |action_state_id| match action_state_id.get() {
                216 => Some(30),
                227 => Some(35),
                _ => None,
            },
        );
    }
    step_world_with_source_runtime_data(
        &mut mashed,
        Frame(1),
        &[
            PlayerInput::neutral(),
            PlayerInput::neutral().with_attack(true),
        ],
        |_| None,
        |action_state_id| match action_state_id.get() {
            216 => Some(30),
            227 => Some(35),
            _ => None,
        },
    );
    step_world_with_source_runtime_data(
        &mut mashed,
        Frame(2),
        &[
            PlayerInput::neutral(),
            PlayerInput::neutral().with_left_stick(90, 0),
        ],
        |_| None,
        |action_state_id| match action_state_id.get() {
            216 => Some(30),
            227 => Some(35),
            _ => None,
        },
    );

    assert_eq!(neutral.players()[1].source_grab_timer, 118.0);
    assert_eq!(
        mashed.players()[1].source_grab_timer, 102.0,
        "ftCommon_GrabMash subtracts x3A8 once for button presses and once when the stored stick direction changes"
    );
    assert!(mashed.players()[1].source_capture_wait_mashed);
}

#[test]
fn source_pummel_end_mash_release_beats_fresh_throw_but_not_held_cstick_down_buffer() {
    let common = MeleeCommonData {
        grab_timer_decrement: 1.0,
        grab_mash_timer_decrement: 8.0,
        ..MeleeCommonData::PROVISIONAL
    };
    for (previous_thrower_input, release_thrower_input, expected_throw) in [
        (
            PlayerInput::neutral(),
            PlayerInput::neutral().with_left_stick(0, 90),
            false,
        ),
        (
            PlayerInput::neutral().with_c_stick(0, -90),
            PlayerInput::neutral().with_c_stick(0, -90),
            true,
        ),
    ] {
        let mut world = World::for_two_players_with_common_data(common);
        configure_source_catch_wait_throw_pair(&mut world, 1);
        world.set_input_history_for_diagnostic(
            [previous_thrower_input, PlayerInput::neutral()],
            [MeleeInputTimers::expired(); 2],
        );

        let mut thrower = world.players()[0];
        thrower.melee_action_state_id = Some(MeleeActionStateId::new(217));
        thrower.source_action_key = Some(SourceActionKey::new("CatchAttack"));
        thrower.source_action_total_frames = 1;
        thrower.motion_frame = 0;
        thrower.set_source_motion_anim_frame(0.0);
        assert!(world.set_player_state_for_diagnostic(0, thrower));
        let mut victim = world.players()[1];
        victim.source_grab_timer = 1.0;
        assert!(world.set_player_state_for_diagnostic(1, victim));

        step_world_with_source_runtime_data(
            &mut world,
            Frame(1),
            &[release_thrower_input, PlayerInput::neutral()],
            |_| None,
            |action_state_id| match action_state_id.get() {
                216 => Some(30),
                217 => Some(1),
                218 => Some(20),
                222 => Some(49),
                227 => Some(35),
                229 => Some(18),
                242 => Some(49),
                _ => None,
            },
        );

        if expected_throw {
            assert_eq!(
                world.players()[0].source_action_key,
                Some(SourceActionKey::new("ThrowLw"))
            );
            assert_eq!(
                world.players()[1].source_action_key,
                Some(SourceActionKey::new("TCaptainThrowLw"))
            );
        } else {
            assert_eq!(
                world.players()[0].source_action_key,
                Some(SourceActionKey::new("CatchCut"))
            );
            assert_eq!(
                world.players()[1].source_action_key,
                Some(SourceActionKey::new("CaptureCut"))
            );
        }
    }
}

#[test]
fn source_catch_wait_main_stick_selects_all_four_decomp_throw_directions() {
    for (input, throw_state_id, throw_key, victim_state_id, victim_key) in [
        (
            PlayerInput::neutral().with_left_stick(90, 0),
            219,
            "ThrowF",
            239,
            "TCaptainThrowF",
        ),
        (
            PlayerInput::neutral().with_left_stick(-90, 0),
            220,
            "ThrowB",
            240,
            "TCaptainThrowB",
        ),
        (
            PlayerInput::neutral().with_left_stick(0, 90),
            221,
            "ThrowHi",
            241,
            "TCaptainThrowHi",
        ),
        (
            PlayerInput::neutral().with_left_stick(0, -90),
            222,
            "ThrowLw",
            242,
            "TCaptainThrowLw",
        ),
    ] {
        let mut world = World::for_two_players();
        configure_source_catch_wait_throw_pair(&mut world, 1);
        step_source_catch_wait_throw_selection(&mut world, 1, input);
        assert_source_throw_selected(
            &world,
            throw_state_id,
            throw_key,
            victim_state_id,
            victim_key,
        );
    }
}

#[test]
fn source_catch_wait_side_throw_selection_uses_facing_dir_like_ftco_800dd1e4() {
    for (facing, input, throw_state_id, throw_key, victim_state_id, victim_key) in [
        (
            -1,
            PlayerInput::neutral().with_left_stick(-90, 0),
            219,
            "ThrowF",
            239,
            "TCaptainThrowF",
        ),
        (
            -1,
            PlayerInput::neutral().with_left_stick(90, 0),
            220,
            "ThrowB",
            240,
            "TCaptainThrowB",
        ),
    ] {
        let mut world = World::for_two_players();
        configure_source_catch_wait_throw_pair(&mut world, facing);
        step_source_catch_wait_throw_selection(&mut world, 1, input);
        assert_source_throw_selected(
            &world,
            throw_state_id,
            throw_key,
            victim_state_id,
            victim_key,
        );
    }
}

#[test]
fn source_catch_wait_cstick_selects_all_four_decomp_throw_directions() {
    for (input, throw_state_id, throw_key, victim_state_id, victim_key) in [
        (
            PlayerInput::neutral().with_c_stick(90, 0),
            219,
            "ThrowF",
            239,
            "TCaptainThrowF",
        ),
        (
            PlayerInput::neutral().with_c_stick(-90, 0),
            220,
            "ThrowB",
            240,
            "TCaptainThrowB",
        ),
        (
            PlayerInput::neutral().with_c_stick(0, 90),
            221,
            "ThrowHi",
            241,
            "TCaptainThrowHi",
        ),
    ] {
        let mut world = World::for_two_players();
        configure_source_catch_wait_throw_pair(&mut world, 1);
        step_source_catch_wait_throw_selection(&mut world, 1, input);
        assert_source_throw_selected(
            &world,
            throw_state_id,
            throw_key,
            victim_state_id,
            victim_key,
        );
    }

    let mut world = World::for_two_players();
    configure_source_catch_wait_throw_pair(&mut world, 1);
    let cstick_down = PlayerInput::neutral().with_c_stick(0, -90);
    step_source_catch_wait_throw_selection(&mut world, 1, cstick_down);
    assert_eq!(
        world.players()[0].melee_action_state_id,
        Some(MeleeActionStateId::new(216)),
        "ftCo_800DF878 does not select down throw from a fresh C-stick down crossing"
    );
    step_source_catch_wait_throw_selection(&mut world, 2, cstick_down);
    assert_source_throw_selected(&world, 222, "ThrowLw", 242, "TCaptainThrowLw");
}

#[test]
fn source_catch_wait_only_cstick_down_buffers_as_a_held_throw_direction() {
    for (held_input, expected_throw) in [
        (PlayerInput::neutral().with_c_stick(90, 0), None),
        (PlayerInput::neutral().with_c_stick(-90, 0), None),
        (PlayerInput::neutral().with_c_stick(0, 90), None),
        (PlayerInput::neutral().with_c_stick(0, -90), Some("ThrowLw")),
    ] {
        let mut world = World::for_two_players();
        step_world(&mut world, Frame(0), &[held_input, PlayerInput::neutral()]);
        configure_source_catch_wait_throw_pair(&mut world, 1);
        step_source_catch_wait_throw_selection(&mut world, 1, held_input);

        match expected_throw {
            Some(source_key) => assert_eq!(
                world.players()[0].source_action_key,
                Some(SourceActionKey::new(source_key))
            ),
            None => assert_eq!(
                world.players()[0].melee_action_state_id,
                Some(MeleeActionStateId::new(216)),
                "decomp edge-crossing C-stick throw directions should not trigger from held input"
            ),
        }
    }
}

#[test]
fn source_catch_wait_fresh_up_input_enters_throw_hi_and_victim_thrown_hi() {
    let mut world = World::for_two_players();
    let mut thrower = world.players()[0];
    thrower.motion_state_alias = None;
    thrower.motion_state = MotionState::Catch;
    thrower.melee_action_state_id = Some(MeleeActionStateId::new(216));
    thrower.source_action_key = Some(SourceActionKey::new("CatchWait"));
    thrower.source_action_total_frames = 30;
    thrower.source_victim_index = Some(1);
    thrower.source_x1a5c_index = Some(1);
    thrower.source_x221b_b5 = true;
    thrower.facing = 1;
    thrower.motion_cmd_var0 = 0x55;
    thrower.motion_throw_flags = 0xff;
    assert!(world.set_player_state_for_diagnostic(0, thrower));

    let mut victim = world.players()[1];
    victim.motion_state_alias = None;
    victim.motion_state = MotionState::GuardOn;
    victim.melee_action_state_id = Some(MeleeActionStateId::new(227));
    victim.source_action_key = Some(SourceActionKey::new("CaptureWaitLw"));
    victim.source_action_total_frames = 35;
    victim.source_victim_index = Some(0);
    victim.source_x1a5c_index = Some(0);
    victim.source_x221b_b5 = false;
    victim.source_grab_timer = 120.0;
    victim.facing = -1;
    victim.motion_anim_frame_milli = 7_000;
    assert!(world.set_player_state_for_diagnostic(1, victim));

    let fresh_up = [
        PlayerInput::neutral().with_left_stick(0, 90),
        PlayerInput::neutral(),
    ];
    step_world_with_source_runtime_data(
        &mut world,
        Frame(1),
        &fresh_up,
        |_| None,
        |action_state_id| match action_state_id.get() {
            216 => Some(30),
            221 => Some(44),
            227 => Some(35),
            241 => Some(15),
            _ => None,
        },
    );

    let players = world.players();
    assert_eq!(
        players[0].melee_action_state_id,
        Some(MeleeActionStateId::new(221)),
        "ftCo_CatchWait_IASA -> ftCo_800DD1E4 should select ThrowHi from a fresh up threshold crossing"
    );
    assert_eq!(
        players[0].source_action_key,
        Some(SourceActionKey::new("ThrowHi"))
    );
    assert_eq!(players[0].motion_state_alias, None);
    assert_eq!(players[0].motion_frame, 0);
    assert_eq!(
        players[0].motion_anim_frame_milli, 1_000,
        "ftCo_800DD398 calls ftAnim_8006EBA4 immediately after entering the throw motion"
    );
    assert_eq!(players[0].motion_cmd_var0, 0);
    assert_eq!(players[0].motion_throw_flags, 0);
    assert_eq!(players[0].source_victim_index, Some(1));

    assert_eq!(
        players[1].melee_action_state_id,
        Some(MeleeActionStateId::new(241)),
        "ftCo_800DD398 should enter victim_msid = throw_index + 239"
    );
    assert_eq!(
        players[1].source_action_key,
        Some(SourceActionKey::new("TCaptainThrowHi"))
    );
    assert_eq!(players[1].motion_state_alias, None);
    assert_eq!(players[1].motion_frame, 0);
    assert_eq!(
        players[1].motion_anim_frame_milli, 1_000,
        "ftCo_800DE3FC calls ftAnim_8006EBA4 immediately after entering the thrown motion"
    );
    assert_eq!(players[1].source_victim_index, Some(0));
    assert_eq!(players[1].facing, players[0].facing);
}

#[test]
fn source_catch_wait_fresh_down_input_enters_throw_lw_and_victim_thrown_lw() {
    let mut world = World::for_two_players();
    let mut thrower = world.players()[0];
    thrower.motion_state_alias = None;
    thrower.motion_state = MotionState::Catch;
    thrower.melee_action_state_id = Some(MeleeActionStateId::new(216));
    thrower.source_action_key = Some(SourceActionKey::new("CatchWait"));
    thrower.source_action_total_frames = 30;
    thrower.source_victim_index = Some(1);
    thrower.source_x1a5c_index = Some(1);
    thrower.source_x221b_b5 = true;
    thrower.facing = 1;
    thrower.motion_cmd_var0 = 0x55;
    thrower.motion_throw_flags = 0xff;
    assert!(world.set_player_state_for_diagnostic(0, thrower));

    let mut victim = world.players()[1];
    victim.motion_state_alias = None;
    victim.motion_state = MotionState::GuardOn;
    victim.melee_action_state_id = Some(MeleeActionStateId::new(227));
    victim.source_action_key = Some(SourceActionKey::new("CaptureWaitLw"));
    victim.source_action_total_frames = 35;
    victim.source_victim_index = Some(0);
    victim.source_x1a5c_index = Some(0);
    victim.source_x221b_b5 = false;
    victim.source_grab_timer = 120.0;
    victim.facing = -1;
    victim.motion_anim_frame_milli = 7_000;
    assert!(world.set_player_state_for_diagnostic(1, victim));

    let fresh_down = [
        PlayerInput::neutral().with_left_stick(0, -90),
        PlayerInput::neutral(),
    ];
    step_world_with_source_runtime_data(
        &mut world,
        Frame(1),
        &fresh_down,
        |_| None,
        |action_state_id| match action_state_id.get() {
            216 => Some(30),
            222 => Some(49),
            227 => Some(35),
            242 => Some(49),
            _ => None,
        },
    );

    let players = world.players();
    assert_eq!(
        players[0].melee_action_state_id,
        Some(MeleeActionStateId::new(222)),
        "ftCo_CatchWait_IASA -> ftCo_800DD1E4 should select ThrowLw from a fresh down threshold crossing"
    );
    assert_eq!(
        players[0].source_action_key,
        Some(SourceActionKey::new("ThrowLw"))
    );
    assert_eq!(players[0].motion_state_alias, None);
    assert_eq!(players[0].motion_frame, 0);
    assert_eq!(players[0].motion_cmd_var0, 0);
    assert_eq!(players[0].motion_throw_flags, 0);
    assert_eq!(players[0].source_victim_index, Some(1));

    assert_eq!(
        players[1].melee_action_state_id,
        Some(MeleeActionStateId::new(242)),
        "ftCo_800DD398 should enter victim_msid = throw_index + 239 for down throw"
    );
    assert_eq!(
        players[1].source_action_key,
        Some(SourceActionKey::new("TCaptainThrowLw"))
    );
    assert_eq!(players[1].motion_state_alias, None);
    assert_eq!(players[1].motion_frame, 0);
    assert_eq!(players[1].source_victim_index, Some(0));
    assert_eq!(players[1].facing, players[0].facing);
}

#[test]
fn source_throw_hi_entry_runs_thrown_accessory_position_on_new_source_pose() {
    let common = MeleeCommonData {
        throw_collision_lockout_ticks: 9,
        ..MeleeCommonData::provisional_mole()
    };
    let mut world = World::for_two_players_with_common_data(common);
    let mut thrower = world.players()[0];
    thrower.motion_state_alias = None;
    thrower.motion_state = MotionState::Catch;
    thrower.melee_action_state_id = Some(MeleeActionStateId::new(216));
    thrower.source_action_key = Some(SourceActionKey::new("CatchWait"));
    thrower.source_action_total_frames = 30;
    thrower.source_victim_index = Some(1);
    thrower.source_x1a5c_index = Some(1);
    thrower.source_x221b_b5 = true;
    thrower.facing = 1;
    thrower.source_position = SourceVec2 { x: 30.0, y: 2.0 };
    thrower.position = thrower.source_position.to_milli();
    assert!(world.set_player_state_for_diagnostic(0, thrower));

    let mut victim = world.players()[1];
    victim.motion_state_alias = None;
    victim.motion_state = MotionState::GuardOn;
    victim.melee_action_state_id = Some(MeleeActionStateId::new(227));
    victim.source_action_key = Some(SourceActionKey::new("CaptureWaitLw"));
    victim.source_action_total_frames = 35;
    victim.source_victim_index = Some(0);
    victim.source_x1a5c_index = Some(0);
    victim.source_grab_timer = 120.0;
    victim.facing = -1;
    victim.source_x1a70 = SourceVec3 {
        x: 0.0,
        y: -4.25,
        z: 3.5,
    };
    victim.source_x34_scale_y = 1.25;
    victim.source_position = SourceVec2 { x: -5.0, y: -5.0 };
    victim.position = victim.source_position.to_milli();
    assert!(world.set_player_state_for_diagnostic(1, victim));

    let fresh_up = [
        PlayerInput::neutral().with_left_stick(0, 90),
        PlayerInput::neutral(),
    ];
    step_world_with_source_runtime_data(
        &mut world,
        Frame(1),
        &fresh_up,
        |player| {
            let source_action_key = player.source_action_key?;
            let capture_pose = match source_action_key.as_str() {
                "ThrowHi" => SourceCapturePose {
                    transn2: SourcePosePoint {
                        x: 4.0,
                        y: 6.0,
                        z: 0.0,
                    },
                    ..SourceCapturePose::default()
                },
                "TCaptainThrowHi" => {
                    let entry_sampled = player.motion_anim_frame_milli >= 1_000;
                    SourceCapturePose {
                        xrotn: SourcePosePoint {
                            x: 40.0,
                            y: 11.0,
                            z: 0.0,
                        },
                        x1a70: SourcePosePoint {
                            x: 0.0,
                            y: if entry_sampled { -3.25 } else { -2.25 },
                            z: if entry_sampled { 2.5 } else { 1.5 },
                        },
                        ..SourceCapturePose::default()
                    }
                }
                _ => SourceCapturePose::default(),
            };
            Some(SourceActionPoseMetadata {
                capture_pose: Some(capture_pose),
                ..SourceActionPoseMetadata::default()
            })
        },
        |action_state_id| match action_state_id.get() {
            216 => Some(30),
            221 => Some(44),
            227 => Some(35),
            241 => Some(15),
            _ => None,
        },
    );

    let players = world.players();
    let thrower = players[0];
    let victim = players[1];
    let expected_x = 30.0 + 4.0 + 3.5 * victim.source_x34_scale_y;
    let expected_y = 2.0 + 6.0 + -4.25 * victim.source_x34_scale_y;
    assert_eq!(
        thrower.source_hurt_collision_state, 1,
        "ftCo_800DD398 calls ftColl_8007B7A4 on the thrower, which sets x198C to 1 when x1990 is clear"
    );
    assert_eq!(
        thrower.source_hurt_collision_lockout_timer, common.throw_collision_lockout_ticks,
        "ftColl_8007B7A4 stores p_ftCommonData->x348 in x1994"
    );
    assert_eq!(
        victim.melee_action_state_id,
        Some(MeleeActionStateId::new(241))
    );
    assert!(
        victim.source_x2226_b2,
        "ftCo_800DB368 sets x2226_b2 before ftCo_800DE508 is installed"
    );
    assert_eq!(
        victim.source_thrown_hitbox_owner_index,
        Some(0),
        "Fighter_ChangeMotionState with arg3=thrower calls ftColl_8007B8CC during ftCo_800DE3FC, so x1064_thrownHitbox.owner is live before throw release"
    );
    assert_eq!(
        victim.source_thrown_hitbox_team_unk, 0,
        "ftColl_8007B8CC copies x119C_teamUnk from the thrower team at thrown entry"
    );
    assert_eq!(
        victim.source_thrown_hitbox_grabber_player_id,
        Some(0),
        "ftColl_8007B8CC copies grabber_unk1 from the thrower player id at thrown entry"
    );
    assert!(
        (victim.source_position.x - expected_x).abs() <= 0.000_01,
        "ftCo_800DE508 reads the thrown fighter's FtPart_XRotN after ftCo_800DB368 constrains it to the thrower's TransN2"
    );
    assert!((victim.source_position.y - expected_y).abs() <= 0.000_01);
    assert_eq!(victim.position, victim.source_position.to_milli());
}

#[test]
fn source_hurt_collision_lockout_ticks_down_like_fighter_8006a360() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.source_hurt_collision_state = 1;
    player.source_hurt_collision_lockout_timer = 2;
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(1),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    assert_eq!(world.players()[0].source_hurt_collision_lockout_timer, 1);
    assert_eq!(world.players()[0].source_hurt_collision_state, 1);

    step_world(
        &mut world,
        Frame(2),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    assert_eq!(world.players()[0].source_hurt_collision_lockout_timer, 0);
    assert_eq!(
        world.players()[0].source_hurt_collision_state,
        0,
        "Fighter_8006A360 clears x198C when x1994 expires and x2221_b0/x1990 are clear"
    );
}

#[test]
fn source_hurt_collision_state_blocks_victim_damage_after_attacker_hitlag() {
    let mut world = World::for_two_players();
    let mut attacker = world.players()[0];
    attacker.melee_action_state_id = Some(MeleeActionStateId::new(44));
    attacker.source_action_key = Some(SourceActionKey::new("Attack11"));
    attacker.source_action_total_frames = 19;
    assert!(world.set_player_state_for_diagnostic(0, attacker));

    let mut victim = world.players()[1];
    victim.melee_action_state_id = Some(MeleeActionStateId::new(14));
    victim.source_action_key = Some(SourceActionKey::new("Wait1"));
    victim.source_hurt_collision_state = 1;
    victim.damage_percent = 12.0;
    victim.damage_percent_temp = 12.0;
    assert!(world.set_player_state_for_diagnostic(1, victim));

    let hitbox = SourceHitboxAttributes {
        bone: 14,
        hit_group: 0,
        damage: 4,
        angle: 90,
        knockback_growth: 100,
        weight_set_knockback: 0,
        base_knockback: 0,
        element: 0,
        shield_damage: 0,
        hit_grounded: true,
        hit_aerial: true,
    };
    let collision_frame = SourceCollisionFrame {
        hits: vec![SourceCollisionCapsule::new(
            0,
            1,
            Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 2.0),
        )
        .with_source_pose(
            Some(MeleeActionStateId::new(44)),
            Some(SourceActionKey::new("Attack11")),
            3,
        )
        .with_hitbox_lifecycle(SourceHitboxLifecycleId::new(0x2c00_0000))
        .with_hitbox_attributes(hitbox)],
        hurts: vec![SourceCollisionCapsule::new(
            1,
            10,
            Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 2.0),
        )],
    };

    let step = world.apply_source_collision_frame(&collision_frame);

    assert!(
        world.players()[0].hitlag_frames > 0,
        "ftColl_80076ED8 writes attacker dmg.x1914 before the victim x198C damage gate"
    );
    assert!(step.confirms.is_empty());
    assert!(step.stages.is_empty());
    assert_eq!(step.applied_stage_count, 0);
    assert_eq!(
        world.players()[1].damage_percent.to_bits(),
        12.0_f32.to_bits()
    );
    assert_eq!(
        world.players()[1].melee_action_state_id,
        Some(MeleeActionStateId::new(14))
    );
}

#[test]
fn source_thrown_hi_uses_ftco_800de508_constrained_xrotn_position_each_tick() {
    let mut world = World::for_two_players();
    let mut thrower = world.players()[0];
    thrower.motion_state_alias = None;
    thrower.motion_state = MotionState::Catch;
    thrower.melee_action_state_id = Some(MeleeActionStateId::new(221));
    thrower.source_action_key = Some(SourceActionKey::new("ThrowHi"));
    thrower.source_action_total_frames = 44;
    thrower.source_victim_index = Some(1);
    thrower.source_x1a5c_index = Some(1);
    thrower.facing = -1;
    thrower.source_position = SourceVec2 { x: 40.0, y: 3.0 };
    thrower.position = thrower.source_position.to_milli();
    assert!(world.set_player_state_for_diagnostic(0, thrower));

    let mut victim = world.players()[1];
    victim.motion_state_alias = None;
    victim.motion_state = MotionState::GuardOn;
    victim.melee_action_state_id = Some(MeleeActionStateId::new(241));
    victim.source_action_key = Some(SourceActionKey::new("TCaptainThrowHi"));
    victim.source_action_total_frames = 15;
    victim.source_victim_index = Some(0);
    victim.source_x1a5c_index = Some(0);
    victim.source_x2226_b2 = true;
    victim.facing = -1;
    victim.source_x1a70 = SourceVec3 {
        x: 0.0,
        y: -6.0,
        z: 4.0,
    };
    victim.source_x34_scale_y = 0.75;
    victim.source_position = SourceVec2 { x: -10.0, y: -10.0 };
    victim.position = victim.source_position.to_milli();
    assert!(world.set_player_state_for_diagnostic(1, victim));

    step_world_with_source_runtime_data(
        &mut world,
        Frame(1),
        &[PlayerInput::neutral(); 2],
        |player| {
            let source_action_key = player.source_action_key?;
            let capture_pose = match source_action_key.as_str() {
                "ThrowHi" => SourceCapturePose {
                    transn2: SourcePosePoint {
                        x: 2.5,
                        y: 5.0,
                        z: 0.0,
                    },
                    ..SourceCapturePose::default()
                },
                "TCaptainThrowHi" => SourceCapturePose {
                    xrotn: SourcePosePoint {
                        x: 60.0,
                        y: 21.0,
                        z: 0.0,
                    },
                    x1a70: SourcePosePoint {
                        x: 0.0,
                        y: -0.75,
                        z: 1.25,
                    },
                    ..SourceCapturePose::default()
                },
                _ => return None,
            };
            Some(SourceActionPoseMetadata {
                capture_pose: Some(capture_pose),
                ..SourceActionPoseMetadata::default()
            })
        },
        |action_state_id| match action_state_id.get() {
            221 => Some(44),
            241 => Some(15),
            _ => None,
        },
    );

    let players = world.players();
    let victim = players[1];
    let expected_x = 40.0 - 2.5 - 4.0 * victim.source_x34_scale_y;
    let expected_y = 3.0 + 5.0 + -6.0 * victim.source_x34_scale_y;
    assert!(
        victim.source_x2226_b2,
        "ftCo_800DB368 keeps x2226_b2 set while ftCo_800DE508 positions the thrown victim"
    );
    assert!((victim.source_position.x - expected_x).abs() <= 0.000_01);
    assert!((victim.source_position.y - expected_y).abs() <= 0.000_01);
    assert_eq!(victim.position, victim.source_position.to_milli());
}

#[test]
fn source_throw_cmd_var0_stops_thrower_and_arms_thrown_anim_timer() {
    let mut world = World::for_two_players();
    let mut thrower = world.players()[0];
    thrower.motion_state_alias = None;
    thrower.motion_state = MotionState::Catch;
    thrower.melee_action_state_id = Some(MeleeActionStateId::new(221));
    thrower.source_action_key = Some(SourceActionKey::new("ThrowHi"));
    thrower.source_action_total_frames = 44;
    thrower.source_victim_index = Some(1);
    thrower.source_x1a5c_index = Some(1);
    thrower.facing = 1;
    thrower.motion_frame = 10;
    thrower.set_source_motion_anim_frame(10.0);
    thrower.source_position = SourceVec2 { x: 30.0, y: 2.0 };
    thrower.position = thrower.source_position.to_milli();
    assert!(world.set_player_state_for_diagnostic(0, thrower));

    let mut victim = world.players()[1];
    victim.motion_state_alias = None;
    victim.motion_state = MotionState::GuardOn;
    victim.melee_action_state_id = Some(MeleeActionStateId::new(241));
    victim.source_action_key = Some(SourceActionKey::new("TCaptainThrowHi"));
    victim.source_action_total_frames = 15;
    victim.source_victim_index = Some(0);
    victim.source_x1a5c_index = Some(0);
    victim.source_x2226_b2 = true;
    victim.facing = 1;
    victim.motion_frame = 10;
    victim.set_source_motion_anim_frame(10.0);
    victim.source_x1a70 = SourceVec3 {
        x: 0.0,
        y: -4.25,
        z: 3.5,
    };
    victim.source_position = SourceVec2 { x: -5.0, y: -5.0 };
    victim.position = victim.source_position.to_milli();
    assert!(world.set_player_state_for_diagnostic(1, victim));

    let source_pose_metadata = |player: &PlayerState| {
        let source_action_key = player.source_action_key?;
        let script_events = if source_action_key.as_str() == "ThrowHi" && !player.source_throw_x4 {
            SourceActionScriptEvents::single(SourceActionScriptEvent::SetCmdVar {
                cmd_var: 0,
                value: 1,
            })
        } else {
            SourceActionScriptEvents::empty()
        };
        let capture_pose = match source_action_key.as_str() {
            "ThrowHi" => SourceCapturePose {
                transn2: SourcePosePoint {
                    x: 4.0,
                    y: 6.0,
                    z: 0.0,
                },
                ..SourceCapturePose::default()
            },
            "TCaptainThrowHi" => SourceCapturePose {
                xrotn: SourcePosePoint {
                    x: 80.0,
                    y: 30.0,
                    z: 0.0,
                },
                ..SourceCapturePose::default()
            },
            _ => SourceCapturePose::default(),
        };
        Some(SourceActionPoseMetadata {
            capture_pose: Some(capture_pose),
            script_events,
            ..SourceActionPoseMetadata::default()
        })
    };
    let total_frames = |action_state_id: MeleeActionStateId| match action_state_id.get() {
        221 => Some(44),
        241 => Some(15),
        _ => None,
    };

    step_world_with_source_runtime_data(
        &mut world,
        Frame(1),
        &[PlayerInput::neutral(); 2],
        source_pose_metadata,
        total_frames,
    );

    let players = world.players();
    assert!(players[0].source_throw_x4);
    assert_eq!(players[0].motion_cmd_var0, 0);
    assert_eq!(players[0].motion_anim_rate_milli, 0);
    assert!(players[1].source_thrown_unk_bool);
    assert_eq!(
        players[1].source_thrown_anim_timer.to_bits(),
        11.0f32.to_bits()
    );
    assert_eq!(
        players[1].motion_anim_rate_milli, 1_000,
        "ftCo_800DE920 stores the timer when the thrown fighter has not reached the thrower's current frame"
    );

    step_world_with_source_runtime_data(
        &mut world,
        Frame(2),
        &[PlayerInput::neutral(); 2],
        source_pose_metadata,
        total_frames,
    );

    let players = world.players();
    assert_eq!(players[1].motion_anim_frame_milli, 11_000);
    assert_eq!(players[1].motion_anim_rate_milli, 0);
    assert_eq!(
        players[1].source_thrown_anim_timer.to_bits(),
        0.0f32.to_bits()
    );
    assert!((players[1].source_position.x - (30.0 + 4.0 + 3.5)).abs() <= 0.000_01);
    assert!((players[1].source_position.y - (2.0 + 6.0 - 4.25)).abs() <= 0.000_01);
}

#[test]
fn source_throw_hitboxes_damage_own_held_victim_without_kb_before_b3_release() {
    let mut world = World::for_two_players();
    let mut thrower = world.players()[0];
    thrower.motion_state_alias = None;
    thrower.motion_state = MotionState::Catch;
    thrower.melee_action_state_id = Some(MeleeActionStateId::new(221));
    thrower.source_action_key = Some(SourceActionKey::new("ThrowHi"));
    thrower.source_action_total_frames = 44;
    thrower.source_victim_index = Some(1);
    thrower.source_x1a5c_index = Some(1);
    assert!(world.set_player_state_for_diagnostic(0, thrower));

    let mut victim = world.players()[1];
    victim.motion_state_alias = None;
    victim.motion_state = MotionState::GuardOn;
    victim.melee_action_state_id = Some(MeleeActionStateId::new(241));
    victim.source_action_key = Some(SourceActionKey::new("TCaptainThrowHi"));
    victim.source_action_total_frames = 15;
    victim.source_victim_index = Some(0);
    victim.source_x1a5c_index = Some(0);
    victim.source_x2226_b2 = true;
    victim.grounded = false;
    assert!(world.set_player_state_for_diagnostic(1, victim));

    let hitbox = SourceHitboxAttributes {
        bone: 14,
        hit_group: 0,
        damage: 4,
        angle: 90,
        knockback_growth: 100,
        weight_set_knockback: 0,
        base_knockback: 0,
        element: 0,
        shield_damage: 0,
        hit_grounded: true,
        hit_aerial: true,
    };
    let collision_frame = SourceCollisionFrame {
        hits: vec![SourceCollisionCapsule::new(
            0,
            1,
            Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 2.0),
        )
        .with_owner_grounded(true)
        .with_source_pose(
            Some(MeleeActionStateId::new(221)),
            Some(SourceActionKey::new("ThrowHi")),
            11,
        )
        .with_hitbox_lifecycle(SourceHitboxLifecycleId::new(11_u64 << 32))
        .with_hitbox_attributes(hitbox)],
        hurts: vec![SourceCollisionCapsule::new(
            1,
            10,
            Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 2.0),
        )
        .with_owner_grounded(false)],
    };

    let step = world.apply_source_collision_frame(&collision_frame);

    assert_eq!(
        step.confirms.len(),
        1,
        "ftColl_80078C70 allows ThrowHi frame-11 hitboxes to confirm against fp->victim_gobj before throw_flags_b3"
    );
    assert_eq!(
        step.stages.len(),
        1,
        "ftColl_80076ED8 stages x1838_percentTemp for the held victim before b3 release"
    );
    assert_eq!(step.applied_stage_count, 1);
    assert_eq!(world.players()[1].damage_percent_temp, 4.0);
    assert_eq!(world.players()[1].damage_applied, 4);
    assert!(
        step.results.is_empty(),
        "Fighter_ProcessHit_8006D1EC takes the non-kb x1838_percentTemp commit path for the held throw victim before b3 release"
    );
    assert!(
        world.players()[0].hitlag_frames > 0,
        "ftColl_80076ED8 still writes thrower dmg.x1914 hitlag when the throw hitbox intersects the held victim"
    );
    assert!(
        world.players()[0].source_x2219_b5,
        "Fighter_ProcessHit_8006D1EC sets x2219_b5 on the thrower after dmg.x1914"
    );
    assert!(
        world.players()[1].source_x2219_b5,
        "Fighter_UnkRecursiveFunc_8006D044 propagates x2219_b5 through fp->x1A5C to the held victim"
    );
    assert_eq!(
        world.players()[1].melee_action_state_id,
        Some(MeleeActionStateId::new(241)),
        "the pre-release throw hit must not enter a normal damage action"
    );
    assert_eq!(world.commit_staged_source_damage(), 1);
    assert_eq!(world.players()[1].damage_percent, 4.0);
    assert_eq!(world.players()[1].damage_percent_temp, 0.0);

    let mut thrower = world.players()[0];
    thrower.hitlag_frames = 0;
    thrower.source_x2219_b5 = false;
    assert!(world.set_player_state_for_diagnostic(0, thrower));
    let mut victim = world.players()[1];
    victim.hitlag_frames = 0;
    victim.source_x2219_b5 = false;
    assert!(world.set_player_state_for_diagnostic(1, victim));

    let raw_throw_hitbox = SourceThrowHitboxAttributes {
        hitbox_idx: 0,
        damage: 10,
        angle: 90,
        hit_x24: 100,
        hit_x28: 0,
        hit_x2c: 120,
        element: 0,
        sfx_severity: 0,
        sfx_kind: 0,
    };
    step_world_with_source_runtime_data(
        &mut world,
        Frame(2),
        &[PlayerInput::neutral(); 2],
        |player| {
            if player.source_action_key == Some(SourceActionKey::new("ThrowHi")) {
                return Some(SourceActionPoseMetadata {
                    script_events: SourceActionScriptEvents::single(
                        SourceActionScriptEvent::SetThrowHitbox(raw_throw_hitbox),
                    ),
                    ..SourceActionPoseMetadata::default()
                });
            }
            None
        },
        |action_state_id| match action_state_id.get() {
            221 => Some(44),
            241 => Some(15),
            _ => None,
        },
    );
    let installed = world.players()[0].source_throw_hitboxes[0]
        .expect("ThrowHi script event should install fp->xDF4[0]");
    let expected_damage = 10.0_f32 * (1.0 - world.common_data().stale_move_damage_reductions[0]);
    assert_eq!(
        installed.damage.to_bits(),
        expected_damage.to_bits(),
        "ftColl_8007891C records stale from held throw-hit confirmation even though generic damage is suppressed before b3"
    );
}

#[test]
fn source_spawn_hitbox_skip_bit_blocks_throw_hitbox_when_x1064_owner_is_absent() {
    let mut world = World::for_two_players();
    let mut thrower = world.players()[0];
    thrower.motion_state_alias = None;
    thrower.motion_state = MotionState::Catch;
    thrower.melee_action_state_id = Some(MeleeActionStateId::new(221));
    thrower.source_action_key = Some(SourceActionKey::new("ThrowHi"));
    thrower.source_action_total_frames = 44;
    thrower.source_victim_index = Some(1);
    thrower.source_x1a5c_index = Some(1);
    thrower.facing = -1;
    thrower.source_position = SourceVec2 { x: 40.0, y: 3.0 };
    thrower.position = thrower.source_position.to_milli();
    assert!(world.set_player_state_for_diagnostic(0, thrower));

    let mut victim = world.players()[1];
    victim.motion_state_alias = None;
    victim.motion_state = MotionState::GuardOn;
    victim.melee_action_state_id = Some(MeleeActionStateId::new(241));
    victim.source_action_key = Some(SourceActionKey::new("TCaptainThrowHi"));
    victim.source_action_total_frames = 15;
    victim.source_victim_index = Some(0);
    victim.source_x1a5c_index = Some(0);
    victim.source_x2226_b2 = true;
    victim.facing = -1;
    victim.grounded = false;
    victim.source_x1a70 = SourceVec3 {
        x: 0.0,
        y: -6.0,
        z: 4.0,
    };
    victim.source_position = SourceVec2 { x: 39.0, y: -3.0 };
    victim.position = victim.source_position.to_milli();
    victim.motion_frame = 10;
    victim.set_source_motion_anim_frame(10.0);
    assert!(world.set_player_state_for_diagnostic(1, victim));

    let hitbox = SourceHitboxAttributes {
        bone: 14,
        hit_group: 0,
        damage: 4,
        angle: 90,
        knockback_growth: 100,
        weight_set_knockback: 0,
        base_knockback: 0,
        element: 0,
        shield_damage: 0,
        hit_grounded: true,
        hit_aerial: true,
    };
    let collision_frame = SourceCollisionFrame {
        hits: vec![SourceCollisionCapsule::new(
            0,
            1,
            Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 2.0),
        )
        .with_owner_grounded(true)
        .with_source_pose(
            Some(MeleeActionStateId::new(221)),
            Some(SourceActionKey::new("ThrowHi")),
            11,
        )
        .with_hitbox_lifecycle(SourceHitboxLifecycleId::new(11_u64 << 32))
        .with_hitbox_attributes(hitbox)
        .with_hitbox_flags(
            SourceHitboxFlags::none().with_skip_if_thrown_hitbox_owner_absent(true),
        )],
        hurts: vec![SourceCollisionCapsule::new(
            1,
            10,
            Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 2.0),
        )
        .with_owner_grounded(false)],
    };

    let step = world.apply_source_collision_frame(&collision_frame);

    assert!(
        step.confirms.is_empty(),
        "ftAction_8007121C skips spawn-hitbox commands with xF_b4 when fp->x1064_thrownHitbox.owner is NULL"
    );
    assert!(step.stages.is_empty());
    assert_eq!(step.applied_stage_count, 0);
    assert_eq!(
        world.players()[0].hitlag_frames,
        0,
        "a skipped source hitbox must not write attacker dmg.x1914 hitlag"
    );
    assert_eq!(
        world.players()[1].hitlag_frames,
        0,
        "a skipped source hitbox must not enter held-victim hitlag"
    );
}

#[test]
fn source_throw_hi_b3_release_routes_through_ftco_800dd724_throw_damage() {
    let mut world = World::for_two_players();
    let stored_victim_input = PlayerInput::neutral().with_left_stick(44, -117);
    let current_victim_input = PlayerInput::neutral().with_left_stick(81, -97);
    step_world(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), stored_victim_input],
    );

    let mut thrower = world.players()[0];
    thrower.motion_state_alias = None;
    thrower.motion_state = MotionState::Catch;
    thrower.melee_action_state_id = Some(MeleeActionStateId::new(221));
    thrower.source_action_key = Some(SourceActionKey::new("ThrowHi"));
    thrower.source_action_total_frames = 44;
    thrower.source_victim_index = Some(1);
    thrower.source_x1a5c_index = Some(1);
    thrower.source_x221b_b5 = true;
    thrower.facing = 1;
    thrower.source_position = SourceVec2 { x: 37.0, y: 0.0 };
    thrower.position = thrower.source_position.to_milli();
    thrower.source_throw_hitboxes[0] = Some(installed_throw_hitbox(SourceThrowHitboxAttributes {
        hitbox_idx: 0,
        damage: 4,
        angle: 90,
        hit_x24: 100,
        hit_x28: 0,
        hit_x2c: 120,
        element: 0,
        sfx_severity: 0,
        sfx_kind: 0,
    }));
    assert!(world.set_player_state_for_diagnostic(0, thrower));

    let mut victim = world.players()[1];
    victim.motion_state_alias = None;
    victim.motion_state = MotionState::GuardOn;
    victim.melee_action_state_id = Some(MeleeActionStateId::new(241));
    victim.source_action_key = Some(SourceActionKey::new("TCaptainThrowHi"));
    victim.source_action_total_frames = 15;
    victim.source_victim_index = Some(0);
    victim.source_x1a5c_index = Some(0);
    victim.source_x2226_b2 = true;
    victim.facing = 1;
    victim.source_x1a70 = SourceVec3 {
        x: 0.0,
        y: -4.0,
        z: 3.0,
    };
    victim.source_x34_scale_y = 1.25;
    victim.grounded = false;
    victim.source_position = SourceVec2 { x: 47.0, y: -1.0 };
    victim.position = victim.source_position.to_milli();
    assert!(world.set_player_state_for_diagnostic(1, victim));

    step_world_with_source_runtime_data(
        &mut world,
        Frame(1),
        &[PlayerInput::neutral(), current_victim_input],
        |player| {
            if player.source_action_key == Some(SourceActionKey::new("ThrowHi")) {
                return Some(SourceActionPoseMetadata {
                    capture_pose: Some(SourceCapturePose {
                        transn2: SourcePosePoint {
                            x: 5.0,
                            y: 8.0,
                            z: 0.0,
                        },
                        ..SourceCapturePose::default()
                    }),
                    script_events: SourceActionScriptEvents::single(
                        SourceActionScriptEvent::SetThrowFlag {
                            hit_idx: 0,
                            flag_bit: Some(3),
                        },
                    ),
                    ..SourceActionPoseMetadata::default()
                });
            }
            None
        },
        |action_state_id| match action_state_id.get() {
            221 => Some(44),
            241 => Some(15),
            90 => Some(40),
            _ => None,
        },
    );

    let players = world.players();
    let expected_release_x = 37.0 + 5.0 + players[1].source_x1a70.z * players[1].source_x34_scale_y;
    let expected_release_y = 8.0 + players[1].source_x1a70.y * players[1].source_x34_scale_y;
    assert_eq!(
        players[1].melee_action_state_id,
        Some(MeleeActionStateId::new(90)),
        "ftCo_800DD724 consumes throw_flags_b3, calls ftCo_800DE2A8, then ftCo_800DE7C0/ftCo_8008DCE0 enters DamageFlyTop for a 90-degree up-throw hit"
    );
    assert_eq!(players[1].source_action_key, None);
    assert_eq!(
        players[1].motion_frame, 1,
        "when the thrower gobj runs before the victim gobj, ftCo_800DD724 enters damage before the victim's same-frame damage tick"
    );
    assert_eq!(players[1].motion_throw_flags & (1 << 3), 0);
    assert_eq!(players[1].source_victim_index, None);
    assert_eq!(players[1].source_x1a5c_index, None);
    assert_eq!(
        players[1].source_thrown_hitbox_owner_index,
        Some(0),
        "ftCo_800DE7C0 installs x21EC=fn_800DE798, then Fighter_ChangeMotionState invokes it before damage anim setup"
    );
    assert_eq!(
        players[1].source_thrown_hitbox_grabber_player_id,
        Some(0),
        "ftColl_8007B8CC records grabber_unk1 from the thrower player id"
    );
    assert!(!players[1].source_x2226_b2);
    assert_eq!(
        world.input_timers()[1].x_tap,
        0xfe,
        "ftCo_8008DCE0 initializes x670_timer_lstick_tilt_x to 0xFE on damage entry, preventing release-frame held stick from becoming immediate SDI"
    );
    assert_eq!(
        world.input_timers()[1].y_tap,
        0xfe,
        "ftCo_8008DCE0 initializes x671_timer_lstick_tilt_y to 0xFE on damage entry, preventing release-frame held stick from becoming immediate SDI"
    );
    let damage_speed =
        players[1].damage_knockback * world.common_data().damage_knockback_velocity_scale;
    let source_di_velocity = |input: PlayerInput| {
        let stick_x = fighter_stick_axis_to_f32(input.stick_x());
        let stick_y = fighter_stick_axis_to_f32(input.stick_y());
        let kb_x = 0.0_f32;
        let kb_y = damage_speed;
        let kb_vel_x_neg = -kb_x;
        let kb_mag_sq = kb_vel_x_neg * kb_vel_x_neg + kb_y * kb_y;
        let f3 = kb_y * stick_x + kb_vel_x_neg * stick_y;
        let mut f30 = f3 * f3 / kb_mag_sq;
        let cross_z = kb_x * stick_y - kb_y * stick_x;
        if cross_z < 0.0 {
            f30 = -f30;
        }
        let angle = kb_y.atan2(kb_x) + world.common_data().di_angle_degrees.to_radians() * f30;
        SourceVec2 {
            x: damage_speed * angle.cos(),
            y: damage_speed * angle.sin(),
        }
    };
    let expected_kb = source_di_velocity(stored_victim_input);
    let current_input_kb = source_di_velocity(current_victim_input);
    assert!(
        (players[1].source_knockback_velocity_x - expected_kb.x).abs() <= 0.002_1,
        "ftCo_800DE7C0 calls ftCo_8008E5A4 with the victim fighter's stored lstick before that victim's same-frame input refresh; expected prior-input x={:.6} actual x={:.6} current-input x would be {:.6}",
        expected_kb.x,
        players[1].source_knockback_velocity_x,
        current_input_kb.x
    );
    assert!(
        (players[1].source_knockback_velocity_y - expected_kb.y).abs() <= 0.052,
        "direct throw DI should expose pre-decay x8c_kb_vel on the damage entry frame; expected prior-input y={:.6} actual y={:.6}",
        expected_kb.y,
        players[1].source_knockback_velocity_y
    );
    assert!(
        (players[1].source_position.x - (expected_release_x + expected_kb.x)).abs() <= 0.002_1,
        "release frame position should use the same pre-decay direct-throw DI X velocity"
    );
    assert!(
        (players[1].source_position.y
            - (expected_release_y + players[1].source_self_velocity_y + expected_kb.y))
            .abs()
            <= 0.052,
        "release frame position should use damage gravity plus pre-decay direct-throw DI Y velocity"
    );
    assert_eq!(
        players[1].source_self_velocity_x.to_bits(),
        0.0_f32.to_bits()
    );
    assert!(
        (players[1].source_self_velocity_y + players[1].profile.gravity).abs() <= 0.000_01,
        "ftCo_DamageFly_Phys applies gravity on the same damage-entry update"
    );
    assert_eq!(players[1].hitlag_frames, 0);
    assert!(!players[1].source_allow_sdi);

    assert_eq!(players[0].motion_throw_flags & (1 << 3), 0);
    assert_eq!(players[0].source_victim_index, None);
    assert_eq!(players[0].source_x1a5c_index, None);
    assert!(!players[0].source_x221b_b5);
}

#[test]
fn source_throw_hi_b3_release_samples_post_anim_transn2_like_ftco_800ddde4() {
    let mut world = World::for_two_players();

    let mut thrower = world.players()[0];
    thrower.motion_state_alias = None;
    thrower.motion_state = MotionState::Catch;
    thrower.melee_action_state_id = Some(MeleeActionStateId::new(221));
    thrower.source_action_key = Some(SourceActionKey::new("ThrowHi"));
    thrower.source_action_total_frames = 44;
    thrower.source_victim_index = Some(1);
    thrower.source_x1a5c_index = Some(1);
    thrower.facing = 1;
    thrower.source_position = SourceVec2 { x: 100.0, y: 0.0 };
    thrower.position = thrower.source_position.to_milli();
    thrower.motion_frame = 13;
    thrower.set_source_motion_anim_frame(14.0);
    thrower.source_throw_hitboxes[0] = Some(installed_throw_hitbox(SourceThrowHitboxAttributes {
        hitbox_idx: 0,
        damage: 4,
        angle: 90,
        hit_x24: 100,
        hit_x28: 0,
        hit_x2c: 120,
        element: 0,
        sfx_severity: 0,
        sfx_kind: 0,
    }));
    assert!(world.set_player_state_for_diagnostic(0, thrower));

    let mut victim = world.players()[1];
    victim.motion_state_alias = None;
    victim.motion_state = MotionState::GuardOn;
    victim.melee_action_state_id = Some(MeleeActionStateId::new(241));
    victim.source_action_key = Some(SourceActionKey::new("TCaptainThrowHi"));
    victim.source_action_total_frames = 15;
    victim.source_victim_index = Some(0);
    victim.source_x1a5c_index = Some(0);
    victim.source_x2226_b2 = true;
    victim.facing = 1;
    victim.grounded = false;
    victim.source_x1a70 = SourceVec3 {
        x: 0.0,
        y: -3.0,
        z: 2.0,
    };
    victim.source_x34_scale_y = 1.0;
    victim.source_position = SourceVec2 { x: -50.0, y: -50.0 };
    victim.position = victim.source_position.to_milli();
    assert!(world.set_player_state_for_diagnostic(1, victim));

    step_world_with_source_runtime_data(
        &mut world,
        Frame(1),
        &[PlayerInput::neutral(); 2],
        |player| {
            if player.source_action_key != Some(SourceActionKey::new("ThrowHi")) {
                return None;
            }
            let anim_frame = player.source_motion_anim_frame;
            let script_events = if (anim_frame - 14.0).abs() <= f32::EPSILON {
                SourceActionScriptEvents::single(SourceActionScriptEvent::SetThrowFlag {
                    hit_idx: 0,
                    flag_bit: Some(3),
                })
            } else {
                SourceActionScriptEvents::empty()
            };
            Some(SourceActionPoseMetadata {
                capture_pose: Some(SourceCapturePose {
                    transn2: SourcePosePoint {
                        x: 10.0 + anim_frame,
                        y: 20.0 + anim_frame,
                        z: 0.0,
                    },
                    ..SourceCapturePose::default()
                }),
                script_events,
                ..SourceActionPoseMetadata::default()
            })
        },
        |action_state_id| match action_state_id.get() {
            221 => Some(44),
            241 => Some(15),
            90 => Some(40),
            _ => None,
        },
    );

    let players = world.players();
    let victim = players[1];
    let expected_post_anim_release = SourceVec2 {
        x: 100.0 + 10.0 + 15.0 + victim.source_x1a70.z,
        y: 20.0 + 15.0 + victim.source_x1a70.y,
    };
    let release_base = SourceVec2 {
        x: victim.source_position.x
            - victim.source_self_velocity_x
            - victim.source_knockback_velocity_x,
        y: victim.source_position.y
            - victim.source_self_velocity_y
            - victim.source_knockback_velocity_y,
    };

    assert_eq!(
        victim.melee_action_state_id,
        Some(MeleeActionStateId::new(90))
    );
    assert!(
        (release_base.x - expected_post_anim_release.x).abs() <= 0.000_01,
        "ftCo_800DDDE4 samples FtPart_TransN2 after the throw action animation callback advances; release_base.x={:.6} expected={:.6}",
        release_base.x,
        expected_post_anim_release.x
    );
    assert!(
        (release_base.y - expected_post_anim_release.y).abs() <= 0.000_01,
        "ftCo_800DDDE4 samples FtPart_TransN2 after the throw action animation callback advances; release_base.y={:.6} expected={:.6}",
        release_base.y,
        expected_post_anim_release.y
    );
}

#[test]
fn source_throw_hi_b3_release_mirrors_transn2_for_left_facing_thrower() {
    let mut world = World::for_two_players();

    let mut thrower = world.players()[0];
    thrower.motion_state_alias = None;
    thrower.motion_state = MotionState::Catch;
    thrower.melee_action_state_id = Some(MeleeActionStateId::new(221));
    thrower.source_action_key = Some(SourceActionKey::new("ThrowHi"));
    thrower.source_action_total_frames = 44;
    thrower.source_victim_index = Some(1);
    thrower.source_x1a5c_index = Some(1);
    thrower.facing = -1;
    thrower.source_position = SourceVec2 { x: 100.0, y: 0.0 };
    thrower.position = thrower.source_position.to_milli();
    thrower.motion_frame = 13;
    thrower.set_source_motion_anim_frame(14.0);
    thrower.source_throw_hitboxes[0] = Some(installed_throw_hitbox(SourceThrowHitboxAttributes {
        hitbox_idx: 0,
        damage: 4,
        angle: 90,
        hit_x24: 100,
        hit_x28: 0,
        hit_x2c: 120,
        element: 0,
        sfx_severity: 0,
        sfx_kind: 0,
    }));
    assert!(world.set_player_state_for_diagnostic(0, thrower));

    let mut victim = world.players()[1];
    victim.motion_state_alias = None;
    victim.motion_state = MotionState::GuardOn;
    victim.melee_action_state_id = Some(MeleeActionStateId::new(241));
    victim.source_action_key = Some(SourceActionKey::new("TCaptainThrowHi"));
    victim.source_action_total_frames = 15;
    victim.source_victim_index = Some(0);
    victim.source_x1a5c_index = Some(0);
    victim.source_x2226_b2 = true;
    victim.facing = -1;
    victim.grounded = false;
    victim.source_x1a70 = SourceVec3 {
        x: 0.0,
        y: -3.0,
        z: 2.0,
    };
    victim.source_x34_scale_y = 1.0;
    victim.source_position = SourceVec2 { x: -50.0, y: -50.0 };
    victim.position = victim.source_position.to_milli();
    assert!(world.set_player_state_for_diagnostic(1, victim));

    step_world_with_source_runtime_data(
        &mut world,
        Frame(1),
        &[PlayerInput::neutral(); 2],
        |player| {
            if player.source_action_key != Some(SourceActionKey::new("ThrowHi")) {
                return None;
            }
            let anim_frame = player.source_motion_anim_frame;
            let script_events = if (anim_frame - 14.0).abs() <= f32::EPSILON {
                SourceActionScriptEvents::single(SourceActionScriptEvent::SetThrowFlag {
                    hit_idx: 0,
                    flag_bit: Some(3),
                })
            } else {
                SourceActionScriptEvents::empty()
            };
            Some(SourceActionPoseMetadata {
                capture_pose: Some(SourceCapturePose {
                    transn2: SourcePosePoint {
                        x: 10.0 + anim_frame,
                        y: 20.0 + anim_frame,
                        z: 0.0,
                    },
                    ..SourceCapturePose::default()
                }),
                script_events,
                ..SourceActionPoseMetadata::default()
            })
        },
        |action_state_id| match action_state_id.get() {
            221 => Some(44),
            241 => Some(15),
            90 => Some(40),
            _ => None,
        },
    );

    let victim = world.players()[1];
    let expected_release_x = 100.0 - (10.0 + 15.0) - victim.source_x1a70.z;
    let release_base_x = victim.source_position.x
        - victim.source_self_velocity_x
        - victim.source_knockback_velocity_x;

    assert_eq!(
        victim.melee_action_state_id,
        Some(MeleeActionStateId::new(90))
    );
    assert!((release_base_x - expected_release_x).abs() <= 0.000_01,
        "ftCo_800DDDE4 samples FtPart_TransN2 as a faced world JObj position before adding facing_dir * x1A70.z"
    );
}

#[test]
fn source_throw_hi_anim_end_enters_wait_like_ftcommon_8007d92c() {
    let mut world = World::for_two_players();

    let mut thrower = world.players()[0];
    thrower.motion_state_alias = None;
    thrower.motion_state = MotionState::Catch;
    thrower.melee_action_state_id = Some(MeleeActionStateId::new(221));
    thrower.source_action_key = Some(SourceActionKey::new("ThrowHi"));
    thrower.source_action_total_frames = 44;
    thrower.motion_frame = 42;
    thrower.set_source_motion_anim_frame(43.0);
    thrower.grounded = true;
    thrower.source_victim_index = None;
    thrower.source_x1a5c_index = None;
    thrower.source_throw_hitboxes[0] = Some(installed_throw_hitbox(SourceThrowHitboxAttributes {
        hitbox_idx: 0,
        damage: 4,
        angle: 90,
        hit_x24: 100,
        hit_x28: 0,
        hit_x2c: 120,
        element: 0,
        sfx_severity: 0,
        sfx_kind: 0,
    }));
    assert!(world.set_player_state_for_diagnostic(0, thrower));

    step_world_with_source_runtime_data(
        &mut world,
        Frame(1),
        &[PlayerInput::neutral(); 2],
        |player| {
            if player.source_action_key == Some(SourceActionKey::new("ThrowHi")) {
                Some(SourceActionPoseMetadata::default())
            } else {
                None
            }
        },
        |action_state_id| match action_state_id.get() {
            14 => Some(49),
            221 => Some(44),
            _ => None,
        },
    );

    let player = world.players()[0];
    assert_eq!(
        player.melee_action_state_id,
        Some(MeleeActionStateId::new(14)),
        "ftCo_ThrowHi_Anim calls ftCommon_8007D92C when ftAnim_IsFramesRemaining reaches zero"
    );
    assert_eq!(player.motion_state, MotionState::Wait);
    assert_eq!(player.motion_state_alias, Some(MotionState::Wait));
    assert_eq!(player.motion_frame, 0);
    assert_eq!(player.source_motion_anim_frame.to_bits(), 0.0_f32.to_bits());
    assert_eq!(player.motion_throw_flags, 0);
    assert_eq!(player.source_throw_hitboxes, [None; 2]);
}

#[test]
fn world_source_collision_stages_stale_scaled_damage_before_recording_confirm() {
    let mut world = World::for_two_players();
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
    let mut attacker = world.players()[0];
    attacker.set_motion_state_alias(MotionState::AttackAirN);
    assert!(world.set_player_state_for_diagnostic(0, attacker));

    let fresh = world.apply_source_collision_frame(&source_collision_frame_for_one_hit(1, hitbox));
    assert_eq!(fresh.stages.len(), 1);
    assert_eq!(fresh.stages[0].damage.to_bits(), 10.0_f32.to_bits());
    assert_eq!(fresh.stages[0].env_damage, 10);
    assert_eq!(fresh.stages[0].unk_count, 10);

    let mut attacker = world.players()[0];
    attacker.set_motion_state_alias(MotionState::Wait);
    assert!(world.set_player_state_for_diagnostic(0, attacker));
    let mut attacker = world.players()[0];
    attacker.set_motion_state_alias(MotionState::AttackAirN);
    assert!(world.set_player_state_for_diagnostic(0, attacker));

    let stale = world.apply_source_collision_frame(&source_collision_frame_for_one_hit(2, hitbox));
    assert_eq!(stale.stages.len(), 1);
    assert!(
        (stale.stages[0].damage - 9.099999).abs() < 0.00001,
        "expected stale-scaled damage from Fighter_804D6548[0], got {:?}",
        stale.stages[0]
    );
    assert_eq!(stale.stages[0].env_damage, 9);
    assert_eq!(stale.stages[0].unk_count, 10);
}

#[test]
fn world_applies_source_damage_stages_to_rollback_owned_damage_fields() {
    let mut world = World::for_two_players();
    let before_checksum = world.checksum();
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
    let stages = [
        SourceDamageStage {
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
        },
        SourceDamageStage {
            attacker_index: 0,
            victim_index: 1,
            hitbox_id: 2,
            hurtbox_id: 10,
            action_state_id: Some(MeleeActionStateId::new(65)),
            source_action_key: Some(SourceActionKey::new("AttackAirN")),
            source_frame: Some(7),
            damaged_hurt_height: 1,
            damage: 3.0,
            env_damage: 3,
            unk_count: 3,
            hitbox: SourceHitboxAttributes {
                damage: 3,
                ..hitbox
            },
        },
    ];

    assert_eq!(world.apply_source_damage_stages(&stages), 2);

    let snapshot = world.snapshot();
    assert_eq!(snapshot.players[1].damage_percent, 0.0);
    assert_eq!(snapshot.players[1].damage_percent_temp, 8.0);
    assert_eq!(snapshot.players[1].damage_applied, 5);
    assert_ne!(before_checksum, world.checksum());
}

#[test]
fn world_commits_staged_source_damage_like_fighter_take_damage_and_clears_stage_fields() {
    let mut world = World::for_two_players();
    let mut player = world.players()[1];
    player.damage_percent = 997.0;
    assert!(world.set_player_state_for_diagnostic(1, player));
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
    world.apply_source_damage_stages(&stages);
    let staged_checksum = world.checksum();

    assert_eq!(world.commit_staged_source_damage(), 1);

    let snapshot = world.snapshot();
    assert_eq!(snapshot.players[1].damage_percent, 999.0);
    assert_eq!(snapshot.players[1].damage_percent_temp, 0.0);
    assert_eq!(snapshot.players[1].damage_applied, 0);
    assert_ne!(staged_checksum, world.checksum());
}

#[test]
fn world_source_damage_results_include_current_frame_staged_percent_temp_like_ftcoll() {
    let world = World::for_two_players();
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

    let result = world
        .source_damage_results_for_stages(&stages)
        .into_iter()
        .find(|result| result.stage.victim_index == 1)
        .expect("staged normal-path hit should produce a selected damage result");
    let without_current_stage = source_damage_result_for_victim(
        world.common_data(),
        &stages,
        SourceDamageResultInput {
            victim_index: 1,
            victim_percent: 0.0,
            victim_percent_temp: 0.0,
            victim_weight: FighterProfile::falcon_like().weight,
            stage: 1.0,
            attack: 1.0,
            defense: 1.0,
        },
    )
    .expect("same stage should produce a comparison result");
    let with_current_stage = source_damage_result_for_victim(
        world.common_data(),
        &stages,
        SourceDamageResultInput {
            victim_index: 1,
            victim_percent: 0.0,
            victim_percent_temp: 10.0,
            victim_weight: FighterProfile::falcon_like().weight,
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
    assert!(result.knockback > without_current_stage.knockback);
}

#[test]
fn source_damage_application_stores_decomp_kb_velocity_separately_from_self_velocity() {
    let mut world = World::for_two_players();
    let mut victim = world.players()[1];
    victim.grounded = false;
    victim.position.x = 1_000;
    victim.source_position = mole_core::SourceVec2::from_milli(victim.position);
    assert!(world.set_player_state_for_diagnostic(1, victim));

    let result = SourceDamageResult {
        stage: SourceDamageStage {
            attacker_index: 0,
            victim_index: 1,
            hitbox_id: 1,
            hurtbox_id: 10,
            action_state_id: Some(MeleeActionStateId::new(72)),
            source_action_key: Some(SourceActionKey::new("AttackAirLw")),
            source_frame: Some(16),
            damaged_hurt_height: 1,
            damage: 12.0,
            env_damage: 12,
            unk_count: 12,
            hitbox: SourceHitboxAttributes {
                bone: 14,
                hit_group: 0,
                damage: 12,
                angle: 270,
                knockback_growth: 100,
                weight_set_knockback: 40,
                base_knockback: 0,
                element: 0,
                shield_damage: 0,
                hit_grounded: true,
                hit_aerial: true,
            },
        },
        knockback: 80.0,
        angle: 270,
        element: 0,
    };

    assert_eq!(world.apply_source_damage_results(&[result]), 1);

    let victim = world.players()[1];
    assert_eq!(victim.source_self_velocity_x.to_bits(), 0.0_f32.to_bits());
    assert_eq!(victim.source_self_velocity_y.to_bits(), 0.0_f32.to_bits());
    assert!(victim.source_knockback_velocity_x.abs() < 0.00001);
    assert!(victim.source_knockback_velocity_y < 0.0);
    assert_eq!(
        victim.velocity.y,
        source_units_to_milli(victim.source_knockback_velocity_y)
    );
}

#[test]
fn source_damage_entry_uses_ftcoll_dir_and_raw_angle_cosine() {
    let mut world = World::for_two_players();
    let mut attacker = world.players()[0];
    attacker.source_position.x = 0.0;
    attacker.source_position.y = 0.0;
    attacker.position = attacker.source_position.to_milli();
    assert!(world.set_player_state_for_diagnostic(0, attacker));

    let mut victim = world.players()[1];
    victim.grounded = false;
    victim.facing = 1;
    victim.source_position.x = 1.0;
    victim.source_position.y = 0.0;
    victim.position = victim.source_position.to_milli();
    assert!(world.set_player_state_for_diagnostic(1, victim));

    let result = SourceDamageResult {
        stage: SourceDamageStage {
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
            hitbox: SourceHitboxAttributes {
                bone: 14,
                hit_group: 0,
                damage: 5,
                angle: 120,
                knockback_growth: 100,
                weight_set_knockback: 40,
                base_knockback: 0,
                element: 0,
                shield_damage: 0,
                hit_grounded: true,
                hit_aerial: true,
            },
        },
        knockback: 40.0,
        angle: 120,
        element: 0,
    };

    assert_eq!(world.apply_source_damage_results(&[result]), 1);

    let victim = world.players()[1];
    let speed = result.knockback * world.common_data().damage_knockback_velocity_scale;
    let expected_x = -speed * (120.0_f32.to_radians().cos()) * -1.0;
    assert_eq!(victim.facing, -1);
    assert!((victim.source_knockback_velocity_x - expected_x).abs() < 0.00001);
    assert!(victim.source_knockback_velocity_x < 0.0);
}

#[test]
fn grounded_source_damage_entry_projects_knockback_to_floor_tangent_like_ftco_8008dce0() {
    let stage = StageProfile::battlefield();
    let mut world = World::for_two_players_on_stage(stage);
    let mut attacker = world.players()[0];
    attacker.source_position.x = 0.0;
    attacker.source_position.y = 0.0;
    attacker.position = attacker.source_position.to_milli();
    assert!(world.set_player_state_for_diagnostic(0, attacker));

    let mut victim = world.players()[1];
    victim.grounded = true;
    victim.facing = 1;
    victim.source_position.x = 1.0;
    victim.source_position.y = 0.0;
    victim.position = victim.source_position.to_milli();
    victim.set_source_floor_for_diagnostic(
        Some(0),
        Some(source_floor_line_for_surface_at_x(
            stage,
            stage.main_floor,
            1.0,
        )),
    );
    assert!(world.set_player_state_for_diagnostic(1, victim));

    let result = SourceDamageResult {
        stage: SourceDamageStage {
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
            hitbox: SourceHitboxAttributes {
                bone: 14,
                hit_group: 0,
                damage: 5,
                angle: 315,
                knockback_growth: 100,
                weight_set_knockback: 40,
                base_knockback: 0,
                element: 0,
                shield_damage: 0,
                hit_grounded: true,
                hit_aerial: true,
            },
        },
        knockback: 40.0,
        angle: 315,
        element: 0,
    };

    assert_eq!(world.apply_source_damage_results(&[result]), 1);

    let victim = world.players()[1];
    let speed = result.knockback * world.common_data().damage_knockback_velocity_scale;
    let expected_tangent_kb = speed * 315.0_f32.to_radians().cos();
    assert!(victim.grounded);
    assert_eq!(
        victim.melee_action_state_id,
        Some(MeleeActionStateId::new(79))
    );
    assert!((victim.source_ground_knockback_velocity - expected_tangent_kb).abs() < 0.00001);
    assert!((victim.source_knockback_velocity_x - expected_tangent_kb).abs() < 0.00001);
    assert!(victim.source_knockback_velocity_y.abs() < 0.00001);
    assert_eq!(victim.velocity.y, 0);
}

#[test]
fn source_damage_action_uses_pre_hit_ground_state_and_damaged_hurt_height() {
    let mut world = World::for_two_players();
    let mut victim = world.players()[1];
    victim.grounded = true;
    victim.position.x = 1_000;
    victim.source_position = mole_core::SourceVec2::from_milli(victim.position);
    assert!(world.set_player_state_for_diagnostic(1, victim));

    let result = SourceDamageResult {
        stage: SourceDamageStage {
            attacker_index: 0,
            victim_index: 1,
            hitbox_id: 1,
            hurtbox_id: 0,
            action_state_id: Some(MeleeActionStateId::new(65)),
            source_action_key: Some(SourceActionKey::new("AttackAirN")),
            source_frame: Some(7),
            damaged_hurt_height: 1,
            damage: 5.0,
            env_damage: 5,
            unk_count: 5,
            hitbox: SourceHitboxAttributes {
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
            },
        },
        knockback: 40.0,
        angle: 78,
        element: 0,
    };

    assert_eq!(world.apply_source_damage_results(&[result]), 1);

    let victim = world.players()[1];
    assert_eq!(
        victim.melee_action_state_id,
        Some(MeleeActionStateId::new(79))
    );
    assert!(!victim.grounded);
}

#[test]
fn source_damage_entry_resets_action_pose_before_hitlag_freezes_update() {
    let stage = StageProfile::battlefield();
    let right_platform = stage.soft_platforms[1];
    let mut world = World::for_two_players_on_stage(stage);

    let mut attacker = world.players()[0];
    attacker.source_position.x = 45.0;
    attacker.source_position.y = milli_to_source_units(right_platform.y);
    attacker.position = attacker.source_position.to_milli();
    assert!(world.set_player_state_for_diagnostic(0, attacker));

    let mut victim = world.players()[1];
    victim.grounded = true;
    victim.facing = -1;
    victim.motion_state = MotionState::Guard;
    victim.motion_state_alias = Some(MotionState::Guard);
    victim.melee_action_state_id = Some(MeleeActionStateId::new(179));
    victim.source_action_key = Some(SourceActionKey::new("Guard"));
    victim.source_action_total_frames = 60;
    victim.motion_frame = 28;
    victim.set_source_motion_anim_frame(28.0);
    victim.motion_anim_rate_milli = 333;
    victim.source_shield_collision_active = true;
    victim.source_position.x = 48.0;
    victim.source_position.y = milli_to_source_units(right_platform.y);
    victim.position = victim.source_position.to_milli();
    victim.set_source_floor_for_diagnostic(
        Some(2),
        Some(source_floor_line_for_surface_at_x(
            stage,
            right_platform,
            victim.source_position.x,
        )),
    );
    assert!(world.set_player_state_for_diagnostic(1, victim));

    let result = SourceDamageResult {
        stage: SourceDamageStage {
            attacker_index: 0,
            victim_index: 1,
            hitbox_id: 1,
            hurtbox_id: 9,
            action_state_id: Some(MeleeActionStateId::new(68)),
            source_action_key: Some(SourceActionKey::new("AttackAirHi")),
            source_frame: Some(7),
            damaged_hurt_height: 0,
            damage: 12.0,
            env_damage: 12,
            unk_count: 12,
            hitbox: SourceHitboxAttributes {
                bone: 14,
                hit_group: 0,
                damage: 12,
                angle: 361,
                knockback_growth: 100,
                weight_set_knockback: 0,
                base_knockback: 10,
                element: 0,
                shield_damage: 0,
                hit_grounded: true,
                hit_aerial: true,
            },
        },
        knockback: 69.31373,
        angle: 361,
        element: 0,
    };

    assert_eq!(
        world.apply_source_damage_results_with_action_total_frames(&[result], |id| {
            (id == MeleeActionStateId::new(83)).then_some(40)
        }),
        1
    );

    let victim = world.players()[1];
    assert_eq!(
        victim.melee_action_state_id,
        Some(MeleeActionStateId::new(83))
    );
    assert!(victim.hitlag_frames > 0);
    assert_eq!(victim.source_action_key, None);
    assert_eq!(victim.source_action_total_frames, 40);
    assert_eq!(victim.motion_state_alias, None);
    assert_eq!(victim.motion_frame, 0);
    assert_eq!(victim.source_motion_anim_frame.to_bits(), 0.0_f32.to_bits());
    assert_eq!(victim.motion_anim_frame_milli, 0);
    assert_eq!(
        victim.motion_anim_rate_milli, 1_000,
        "ftCo_Damage calls Fighter_ChangeMotionState(... anim_start=0.0F, anim_rate=1.0F, blend=0.0F); hitlag freezes animation through the fighter freeze flag, not by zeroing frame_speed_mul"
    );
    assert!(!victim.source_shield_collision_active);
    let snapshot = world.snapshot().players[1];
    assert_eq!(
        snapshot.source_pose_action_state_id,
        Some(MeleeActionStateId::new(83))
    );
    assert_eq!(snapshot.source_pose_frame, 0);
}

#[test]
fn source_damage_hitlag_applies_sdi_from_plco_window_like_ftco_damage_every_hitlag() {
    let common = MeleeCommonData {
        sdi_min_stick_mag: 0.7,
        sdi_stick_window: 4,
        sdi_pos_scale: 6.0,
        ..MeleeCommonData::provisional_mole()
    };
    let mut world = World::for_two_players_on_stage_with_profiles_and_common_data(
        StageProfile::battlefield(),
        [FighterProfile::FALCON_LIKE; 2],
        common,
    );
    let mut victim = world.players()[1];
    victim.grounded = false;
    victim.motion_state_alias = None;
    victim.motion_state = MotionState::Fall;
    victim.melee_action_state_id = Some(MeleeActionStateId::new(84));
    victim.source_action_key = Some(SourceActionKey::new("DamageAir1"));
    victim.hitlag_frames = 2;
    victim.source_allow_sdi = true;
    victim.position = Vec2 { x: 0, y: 0 };
    victim.source_position = mole_core::SourceVec2::from_milli(victim.position);
    assert!(world.set_player_state_for_diagnostic(1, victim));
    world.set_input_history_for_diagnostic(
        [
            PlayerInput::neutral(),
            PlayerInput::neutral().with_left_stick(127, 0),
        ],
        [
            MeleeInputTimers::expired(),
            MeleeInputTimers {
                x_tap: 0,
                y_tap: 0,
                trigger: 0xff,
            },
        ],
    );

    step_world(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    let victim = world.players()[1];
    assert_eq!(victim.hitlag_frames, 1);
    assert_eq!(victim.position.x, 6_000);
    assert_eq!(world.input_timers()[1].x_tap, 0xfe);
}

#[test]
fn source_damage_exit_hitlag_applies_asdi_and_di_to_source_kb_velocity() {
    let common = MeleeCommonData {
        sdi_min_stick_mag: 0.7,
        sdi_stick_window: 4,
        sdi_pos_scale: 6.0,
        asdi_pos_scale: 3.0,
        di_angle_degrees: 18.0,
        trigger_di_knockback_multiplier: 1.0,
        damage_knockback_frame_decay: 0.0,
        ..MeleeCommonData::provisional_mole()
    };
    let mut world = World::for_two_players_on_stage_with_profiles_and_common_data(
        StageProfile::battlefield(),
        [FighterProfile::FALCON_LIKE; 2],
        common,
    );
    let mut victim = world.players()[1];
    victim.grounded = false;
    victim.motion_state_alias = None;
    victim.motion_state = MotionState::Fall;
    victim.melee_action_state_id = Some(MeleeActionStateId::new(84));
    victim.source_action_key = Some(SourceActionKey::new("DamageAir1"));
    victim.source_action_total_frames = 40;
    victim.position.y = 50_000;
    victim.hitlag_frames = 1;
    victim.source_allow_sdi = true;
    victim.source_knockback_velocity_x = 1.0;
    victim.source_knockback_velocity_y = 0.0;
    victim.velocity.x = source_units_to_milli(victim.source_knockback_velocity_x);
    victim.source_position = mole_core::SourceVec2::from_milli(victim.position);
    let pre_hitlag_source_y = victim.source_position.y;
    assert!(world.set_player_state_for_diagnostic(1, victim));
    world.set_input_history_for_diagnostic(
        [
            PlayerInput::neutral(),
            PlayerInput::neutral().with_left_stick(0, 127),
        ],
        [
            MeleeInputTimers::expired(),
            MeleeInputTimers {
                x_tap: 0,
                y_tap: 0,
                trigger: 0xff,
            },
        ],
    );

    step_world(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    let victim = world.players()[1];
    assert_eq!(victim.hitlag_frames, 0);
    assert!(!victim.source_allow_sdi);
    let expected_control_source_y = pre_hitlag_source_y
        + fighter_stick_axis_to_f32(127) * (common.sdi_pos_scale + common.asdi_pos_scale);
    assert!(
        victim.source_position.y > expected_control_source_y,
        "HSD gobj proc priority runs Fighter_8006A1BC before Fighter_procUpdate, so the hitlag exit frame applies SDI, ASDI/DI, and then same-frame damage air physics"
    );
    let knockback_angle = victim
        .source_knockback_velocity_y
        .atan2(victim.source_knockback_velocity_x);
    assert!(
        (knockback_angle - 18.0_f32.to_radians()).abs() < 0.0001,
        "post-physics knockback angle was {knockback_angle}, velocity=({}, {})",
        victim.source_knockback_velocity_x,
        victim.source_knockback_velocity_y
    );
    assert!(
        (victim
            .source_knockback_velocity_x
            .hypot(victim.source_knockback_velocity_y)
            - 1.0)
            .abs()
            < 0.0001
    );
}

#[test]
fn source_damage_exit_hitlag_uses_previous_proc_update_input_like_fighter_8006a1bc() {
    let common = MeleeCommonData {
        sdi_min_stick_mag: 0.7,
        sdi_stick_window: 4,
        sdi_pos_scale: 0.0,
        asdi_pos_scale: 0.0,
        di_angle_degrees: 18.0,
        trigger_di_knockback_multiplier: 1.0,
        damage_knockback_frame_decay: 0.0,
        ..MeleeCommonData::provisional_mole()
    };
    let mut world = World::for_two_players_on_stage_with_profiles_and_common_data(
        StageProfile::battlefield(),
        [FighterProfile::FALCON_LIKE; 2],
        common,
    );
    let mut victim = world.players()[1];
    victim.grounded = false;
    victim.motion_state_alias = None;
    victim.motion_state = MotionState::Fall;
    victim.melee_action_state_id = Some(MeleeActionStateId::new(84));
    victim.source_action_key = Some(SourceActionKey::new("DamageAir1"));
    victim.source_action_total_frames = 40;
    victim.position.y = 50_000;
    victim.hitlag_frames = 1;
    victim.source_allow_sdi = true;
    victim.source_knockback_velocity_x = 1.0;
    victim.source_knockback_velocity_y = 1.0;
    victim.source_position = SourceVec2::from_milli(victim.position);
    assert!(world.set_player_state_for_diagnostic(1, victim));
    assert!(world.players()[1].source_allow_sdi);
    assert_eq!(world.players()[1].hitlag_frames, 1);
    world.set_input_history_for_diagnostic(
        [
            PlayerInput::neutral(),
            PlayerInput::neutral().with_left_stick(-127, 0),
        ],
        [MeleeInputTimers::expired(); 2],
    );

    step_world(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    let victim = world.players()[1];
    assert_eq!(victim.hitlag_frames, 0);
    assert!(
        victim.source_knockback_velocity_y > victim.source_knockback_velocity_x,
        "priority-0 Fighter_8006A1BC runs before Fighter_procUpdate refreshes fp->input, so exit DI should read the previous held-left input instead of the current neutral row; velocity=({}, {})",
        victim.source_knockback_velocity_x,
        victim.source_knockback_velocity_y
    );
}

#[test]
fn source_damage_fly_anim_exit_enters_damage_fall_after_lockout_clears() {
    let mut world = World::for_two_players();
    let mut victim = world.players()[1];
    victim.grounded = false;
    victim.motion_state_alias = None;
    victim.motion_state = MotionState::Fall;
    victim.melee_action_state_id = Some(MeleeActionStateId::new(88));
    victim.source_action_key = Some(SourceActionKey::new("DamageFlyN"));
    victim.motion_frame = 10;
    victim.motion_anim_frame_milli = 10_000;
    victim.damage_hitstun_frames = 1;
    victim.source_action_total_frames = 12;
    victim.source_self_velocity_y = -2.9;
    victim.source_knockback_velocity_y = 0.511_696_9;
    victim.velocity.y =
        source_units_to_milli(victim.source_self_velocity_y + victim.source_knockback_velocity_y);
    assert!(world.set_player_state_for_diagnostic(1, victim));

    step_world(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    let still_damage = world.players()[1];
    assert_eq!(still_damage.damage_hitstun_frames, 0);
    assert_eq!(still_damage.motion_state_alias, None);
    assert_eq!(
        still_damage.melee_action_state_id,
        Some(MeleeActionStateId::new(88))
    );
    assert_eq!(still_damage.motion_frame, 11);

    step_world(
        &mut world,
        Frame(1),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    let exited = world.players()[1];
    assert_eq!(
        exited.melee_action_state_id,
        Some(MeleeActionStateId::new(38)),
        "ftCo_DamageFly_Anim calls ftCo_80090780 when ftAnim_IsFramesRemaining is false; the airborne handoff is ftCo_MS_DamageFall, not ftCo_MS_Fall"
    );
    assert_eq!(
        exited.source_action_key,
        Some(SourceActionKey::new("DamageFall"))
    );
    assert_eq!(exited.motion_state_alias, None);
    assert_eq!(
        exited.velocity.y,
        source_units_to_milli(exited.source_self_velocity_y),
        "ftCo_80090780 preserves fp->self_vel.y as the exported self velocity; knockback remains separate in x8c"
    );
}

#[test]
fn damage_fall_air_drift_does_not_seed_self_velocity_from_projected_knockback_export() {
    let mut world = World::for_two_players();
    let mut victim = world.players()[1];
    victim.grounded = false;
    victim.motion_state_alias = None;
    victim.motion_state = MotionState::DamageFall;
    victim.melee_action_state_id = Some(MeleeActionStateId::new(38));
    victim.source_action_key = Some(SourceActionKey::new("DamageFall"));
    victim.source_action_total_frames = 0;
    victim.motion_frame = 0;
    victim.motion_anim_frame_milli = 0;
    victim.source_self_velocity_x = 0.0;
    victim.source_self_velocity_y = -2.9;
    victim.source_knockback_velocity_x = 0.739_683_99;
    victim.source_knockback_velocity_y = 0.541_835;
    victim.velocity = Vec2 {
        x: source_units_to_milli(victim.source_knockback_velocity_x),
        y: source_units_to_milli(
            victim.source_self_velocity_y + victim.source_knockback_velocity_y,
        ),
    };
    assert!(world.set_player_state_for_diagnostic(1, victim));
    world.set_input_history_for_diagnostic(
        [
            PlayerInput::neutral(),
            PlayerInput::neutral().with_left_stick(-125, 0),
        ],
        [MeleeInputTimers::expired(); 2],
    );

    step_world(
        &mut world,
        Frame(0),
        &[
            PlayerInput::neutral(),
            PlayerInput::neutral().with_left_stick(-125, 0),
        ],
    );

    let drifted = world.players()[1];
    let expected_self_x = source_air_drift_velocity_f32(0.0, -125, drifted.profile);
    assert!(
        (drifted.source_self_velocity_x - expected_self_x).abs() < 0.00001,
        "ftCo_DamageFall_Phys -> ft_80084DB0 applies air drift from fp->self_vel.x; exported/composed knockback velocity must not back-fill fp->self_vel.x"
    );
}

#[test]
fn source_damage_unlocked_air_physics_uses_normal_fall_drift_before_anim_exit() {
    let mut world = World::for_two_players();
    let mut victim = world.players()[1];
    victim.grounded = false;
    victim.motion_state_alias = None;
    victim.motion_state = MotionState::Fall;
    victim.melee_action_state_id = Some(MeleeActionStateId::new(84));
    victim.source_action_key = Some(SourceActionKey::new("DamageAir1"));
    victim.motion_frame = 5;
    victim.motion_anim_frame_milli = 5_000;
    victim.damage_hitstun_frames = 0;
    victim.source_action_total_frames = 12;
    victim.velocity = Vec2 { x: 0, y: 0 };
    assert!(world.set_player_state_for_diagnostic(1, victim));

    step_world(
        &mut world,
        Frame(0),
        &[
            PlayerInput::neutral(),
            PlayerInput::neutral().with_left_stick(127, 0),
        ],
    );

    let drifted = world.players()[1];
    assert_eq!(drifted.motion_state_alias, None);
    assert_eq!(
        drifted.melee_action_state_id,
        Some(MeleeActionStateId::new(84))
    );
    assert_eq!(
        drifted.velocity.x,
        source_air_drift_velocity(0, 127, drifted.profile)
    );
    assert_eq!(
        drifted.velocity.y,
        source_units_to_milli(-drifted.profile.gravity)
    );
}

#[test]
fn source_damage_hitstun_air_physics_decays_x8c_knockback_like_fighter_proc_update() {
    let mut world = World::for_two_players();
    let common = world.common_data();
    let mut victim = world.players()[1];
    victim.grounded = false;
    victim.motion_state_alias = None;
    victim.motion_state = MotionState::Fall;
    victim.melee_action_state_id = Some(MeleeActionStateId::new(84));
    victim.source_action_key = Some(SourceActionKey::new("DamageAir1"));
    victim.motion_frame = 5;
    victim.motion_anim_frame_milli = 5_000;
    victim.damage_hitstun_frames = 4;
    victim.source_action_total_frames = 12;
    victim.source_position = SourceVec2 { x: 10.0, y: 20.0 };
    victim.position = Vec2 {
        x: 10_000,
        y: 20_000,
    };
    victim.source_self_velocity_x = 0.0;
    victim.source_self_velocity_y = 0.0;
    victim.source_knockback_velocity_x = 3.0;
    victim.source_knockback_velocity_y = 4.0;
    victim.velocity = Vec2 { x: 3_000, y: 4_000 };
    assert!(world.set_player_state_for_diagnostic(1, victim));

    step_world(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    let moved = world.players()[1];
    let angle = 4.0_f32.atan2(3.0);
    let expected_kb_x = 3.0 - common.damage_knockback_frame_decay * angle.cos();
    let expected_kb_y = 4.0 - common.damage_knockback_frame_decay * angle.sin();
    let expected_self_y = -moved.profile.gravity;
    assert_eq!(moved.source_self_velocity_x.to_bits(), 0.0_f32.to_bits());
    assert_eq!(
        moved.source_self_velocity_y.to_bits(),
        expected_self_y.to_bits()
    );
    assert!((moved.source_knockback_velocity_x - expected_kb_x).abs() < 0.00001);
    assert!((moved.source_knockback_velocity_y - expected_kb_y).abs() < 0.00001);
    assert_eq!(
        moved.position.x,
        source_units_to_milli(10.0 + expected_kb_x)
    );
    assert_eq!(
        moved.position.y,
        source_units_to_milli(20.0 + expected_self_y + expected_kb_y)
    );
}

#[test]
fn source_damage_entry_frame_decays_x8c_knockback_on_first_airborne_update() {
    let mut world = World::for_two_players();
    let common = world.common_data();
    let mut victim = world.players()[1];
    victim.grounded = false;
    victim.motion_state_alias = None;
    victim.motion_state = MotionState::Fall;
    victim.melee_action_state_id = Some(MeleeActionStateId::new(90));
    victim.source_action_key = Some(SourceActionKey::new("DamageFlyTop"));
    victim.motion_frame = 0;
    victim.motion_anim_frame_milli = 0;
    victim.damage_hitstun_frames = 4;
    victim.source_action_total_frames = 40;
    victim.source_position = SourceVec2 { x: 10.0, y: 20.0 };
    victim.position = Vec2 {
        x: 10_000,
        y: 20_000,
    };
    victim.source_self_velocity_x = 0.0;
    victim.source_self_velocity_y = 0.0;
    victim.source_knockback_velocity_x = 3.0;
    victim.source_knockback_velocity_y = 4.0;
    victim.velocity = Vec2 { x: 3_000, y: 4_000 };
    assert!(world.set_player_state_for_diagnostic(1, victim));

    step_world(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    let moved = world.players()[1];
    let angle = 4.0_f32.atan2(3.0);
    let expected_kb_x = 3.0 - common.damage_knockback_frame_decay * angle.cos();
    let expected_kb_y = 4.0 - common.damage_knockback_frame_decay * angle.sin();
    let expected_self_y = -moved.profile.gravity;
    assert!(
        (moved.source_knockback_velocity_x - expected_kb_x).abs() < 0.00001,
        "Fighter_procUpdate decays fp->x8c_kb_vel on the first airborne damage update; motion_frame == 0 is not a generic same-tick skip"
    );
    assert!(
        (moved.source_knockback_velocity_y - expected_kb_y).abs() < 0.00001,
        "Fighter_procUpdate decays fp->x8c_kb_vel on the first airborne damage update; motion_frame == 0 is not a generic same-tick skip"
    );
    assert_eq!(
        moved.position.x,
        source_units_to_milli(10.0 + expected_kb_x)
    );
    assert_eq!(
        moved.position.y,
        source_units_to_milli(20.0 + expected_self_y + expected_kb_y)
    );
}

#[test]
fn grounded_landing_applies_persistent_x8c_knockback_like_fighter_proc_update() {
    let mut world = World::for_two_players();
    let common = world.common_data();
    let floor = world.stage().main_floor;

    let mut inactive = world.players()[0];
    inactive.player_state = PLAYER_STATE_NONE;
    assert!(world.set_player_state_for_diagnostic(0, inactive));

    let mut player = world.players()[1];
    player.grounded = true;
    player.motion_state = MotionState::Landing;
    player.motion_state_alias = Some(MotionState::Landing);
    player.melee_action_state_id = Some(MeleeActionStateId::new(42));
    player.motion_frame = 0;
    player.motion_anim_frame_milli = 0;
    player.source_position = SourceVec2 { x: 10.0, y: 0.0 };
    player.position = Vec2 {
        x: source_units_to_milli(player.source_position.x),
        y: floor.y,
    };
    player.ground_velocity_x = 0.0;
    player.ground_accel_x = 0.0;
    player.ground_accel_x2 = 0.0;
    player.source_self_velocity_x = 0.0;
    player.source_self_velocity_y = 0.0;
    player.source_knockback_velocity_x = 1.0;
    player.source_knockback_velocity_y = 0.0;
    player.source_ground_knockback_velocity = 0.0;
    player.velocity = Vec2 { x: 0, y: 0 };
    assert!(world.set_player_state_for_diagnostic(1, player));

    step_world(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    let moved = world.players()[1];
    let expected_ground_kb =
        1.0 - moved.profile.ground_friction * common.damage_ground_knockback_friction_multiplier;
    assert_eq!(
        moved.source_ground_knockback_velocity.to_bits(),
        expected_ground_kb.to_bits()
    );
    assert_eq!(
        moved.source_knockback_velocity_x.to_bits(),
        expected_ground_kb.to_bits()
    );
    assert_eq!(moved.source_knockback_velocity_y, 0.0);
    assert_eq!(
        moved.position.x,
        source_units_to_milli(10.0 + expected_ground_kb)
    );
}

#[test]
fn source_damage_floor_contact_with_basic_knockback_enters_landing_like_ftco_damage_coll() {
    let mut world = World::for_two_players();
    let floor = world.stage().main_floor;
    let mut victim = world.players()[1];
    victim.grounded = false;
    victim.motion_state_alias = None;
    victim.motion_state = MotionState::Fall;
    victim.melee_action_state_id = Some(MeleeActionStateId::new(84));
    victim.source_action_key = Some(SourceActionKey::new("DamageAir1"));
    victim.motion_frame = 0;
    victim.motion_anim_frame_milli = 0;
    victim.damage_hitstun_frames = 10;
    victim.source_action_total_frames = 20;
    victim.position = Vec2 {
        x: floor.left_x + 10_000,
        y: 0,
    };
    victim.source_position = SourceVec2::from_milli(victim.position);
    victim.source_knockback_velocity_x = 1.0;
    victim.source_self_velocity_y = -4.0;
    victim.velocity = Vec2 {
        x: source_units_to_milli(victim.source_knockback_velocity_x),
        y: source_units_to_milli(victim.source_self_velocity_y),
    };
    assert!(world.set_player_state_for_diagnostic(1, victim));
    let source_snapshot = world.snapshot().players[1];
    let source_root = world.players()[1].position;
    let source_bottom_offset_y = source_snapshot.active_ecb.bottom.y - source_root.y;
    let mut victim = world.players()[1];
    victim.position.y = floor.y - source_bottom_offset_y + 500;
    victim.source_position = SourceVec2::from_milli(victim.position);
    victim.set_source_collision_ecb_from_world_for_diagnostic(translated_ecb(
        source_snapshot.active_ecb,
        Vec2 {
            x: victim.position.x - source_root.x,
            y: victim.position.y - source_root.y,
        },
    ));
    assert!(world.set_player_state_for_diagnostic(1, victim));

    step_world(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    let landed = world.players()[1];
    let landed_snapshot = world.snapshot().players[1];
    assert!(
        landed.grounded,
        "expected DamageAir1 floor contact, got action {:?} key {:?} pos {:?} source_pos {:?} vel {:?} self_y {} kb {:?} coll_last {:?} coll_cur {:?} ecb_bottom {:?} floor {:?} env {:#x}",
        landed.melee_action_state_id,
        landed.source_action_key,
        landed.position,
        landed.source_position,
        landed.velocity,
        landed.source_self_velocity_y,
        (
            landed.source_knockback_velocity_x,
            landed.source_knockback_velocity_y
        ),
        landed_snapshot.source_coll_last_pos,
        landed_snapshot.source_coll_cur_pos,
        landed_snapshot.active_ecb.bottom,
        landed.source_floor_for_diagnostic(),
        landed_snapshot.source_coll_env_flags,
    );
    assert_eq!(landed.position.y, floor.y);
    assert_eq!(landed.motion_state, MotionState::Landing);
    assert_eq!(landed.motion_state_alias, Some(MotionState::Landing));
    assert_eq!(
        landed.melee_action_state_id,
        Some(MeleeActionStateId::new(42))
    );
    assert_eq!(landed.landing_lag_ticks, 0);
}

#[test]
fn source_damage_floor_contact_with_high_knockback_enters_down_bound_like_ftco_damage_coll() {
    let mut world = World::for_two_players();
    let floor = world.stage().main_floor;
    let mut victim = world.players()[1];
    victim.grounded = false;
    victim.motion_state_alias = None;
    victim.motion_state = MotionState::Fall;
    victim.melee_action_state_id = Some(MeleeActionStateId::new(84));
    victim.source_action_key = Some(SourceActionKey::new("DamageAir1"));
    victim.motion_frame = 0;
    victim.motion_anim_frame_milli = 0;
    victim.damage_hitstun_frames = 10;
    victim.source_action_total_frames = 20;
    victim.source_down_bound_pose = Some(SourceDownBoundPose {
        hip_mtx_0_1: 0.0,
        hip_mtx_0_2: 0.0,
        hip_mtx_1_1: 1.0,
        hip_mtx_1_2: -1.0,
    });
    victim.position = Vec2 {
        x: floor.left_x + 10_000,
        y: 0,
    };
    victim.source_position = SourceVec2::from_milli(victim.position);
    victim.source_knockback_velocity_x = 20.0;
    victim.source_knockback_velocity_y = -20.0;
    victim.velocity = Vec2 {
        x: source_units_to_milli(victim.source_knockback_velocity_x),
        y: source_units_to_milli(victim.source_knockback_velocity_y),
    };
    assert!(world.set_player_state_for_diagnostic(1, victim));
    let source_snapshot = world.snapshot().players[1];
    let source_root = world.players()[1].position;
    let source_bottom_offset_y = source_snapshot.active_ecb.bottom.y - source_root.y;
    let mut victim = world.players()[1];
    victim.position.y = floor.y - source_bottom_offset_y + 500;
    victim.source_position = SourceVec2::from_milli(victim.position);
    victim.set_source_collision_ecb_from_world_for_diagnostic(translated_ecb(
        source_snapshot.active_ecb,
        Vec2 {
            x: victim.position.x - source_root.x,
            y: victim.position.y - source_root.y,
        },
    ));
    assert!(world.set_player_state_for_diagnostic(1, victim));

    step_world(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    let bounded = world.players()[1];
    let bounded_snapshot = world.snapshot().players[1];
    assert!(
        bounded.grounded,
        "expected DamageAir1 high-kb floor contact, got action {:?} key {:?} pos {:?} source_pos {:?} vel {:?} self_y {} kb {:?} coll_last {:?} coll_cur {:?} ecb_bottom {:?} floor {:?} env {:#x}",
        bounded.melee_action_state_id,
        bounded.source_action_key,
        bounded.position,
        bounded.source_position,
        bounded.velocity,
        bounded.source_self_velocity_y,
        (
            bounded.source_knockback_velocity_x,
            bounded.source_knockback_velocity_y
        ),
        bounded_snapshot.source_coll_last_pos,
        bounded_snapshot.source_coll_cur_pos,
        bounded_snapshot.active_ecb.bottom,
        bounded.source_floor_for_diagnostic(),
        bounded_snapshot.source_coll_env_flags,
    );
    assert_eq!(bounded.position.y, floor.y);
    assert_eq!(bounded.motion_state_alias, None);
    assert_eq!(
        bounded.melee_action_state_id,
        Some(MeleeActionStateId::new(183))
    );
    assert_eq!(
        bounded.source_action_key,
        Some(SourceActionKey::new("DownBoundU"))
    );
    assert_eq!(bounded.motion_frame, 0);
    assert_eq!(
        bounded.velocity.y,
        source_units_to_milli(bounded.source_self_velocity_y + bounded.source_knockback_velocity_y),
        "ftCommon_8007D7FC grounds the fighter without clearing self_vel.y; exported velocity remains the composed self_vel.y + x8c_kb_vel.y channel"
    );
}

#[test]
fn source_damage_fly_floor_contact_with_recent_digital_lr_enters_passive_like_ftco_8009872c() {
    let mut world = World::for_two_players();
    let floor = world.stage().main_floor;
    let mut victim = world.players()[1];
    victim.grounded = false;
    victim.motion_state_alias = None;
    victim.motion_state = MotionState::Fall;
    victim.melee_action_state_id = Some(MeleeActionStateId::new(91));
    victim.source_action_key = Some(SourceActionKey::new("DamageFlyRoll"));
    victim.motion_frame = 0;
    victim.motion_anim_frame_milli = 0;
    victim.damage_hitstun_frames = 10;
    victim.source_action_total_frames = 30;
    victim.source_down_bound_pose = Some(SourceDownBoundPose {
        hip_mtx_0_1: 0.0,
        hip_mtx_0_2: 0.0,
        hip_mtx_1_1: 1.0,
        hip_mtx_1_2: -1.0,
    });
    victim.position = Vec2 {
        x: floor.left_x + 10_000,
        y: 0,
    };
    victim.source_position = SourceVec2::from_milli(victim.position);
    victim.source_knockback_velocity_x = 1.0;
    victim.source_self_velocity_y = -4.0;
    victim.velocity = Vec2 {
        x: source_units_to_milli(victim.source_knockback_velocity_x),
        y: source_units_to_milli(victim.source_self_velocity_y),
    };
    assert!(world.set_player_state_for_diagnostic(1, victim));
    let source_snapshot = world.snapshot().players[1];
    let source_root = world.players()[1].position;
    let source_bottom_offset_y = source_snapshot.active_ecb.bottom.y - source_root.y;
    let mut victim = world.players()[1];
    victim.position.y = floor.y - source_bottom_offset_y + 500;
    victim.source_position = SourceVec2::from_milli(victim.position);
    victim.set_source_collision_ecb_from_world_for_diagnostic(translated_ecb(
        source_snapshot.active_ecb,
        Vec2 {
            x: victim.position.x - source_root.x,
            y: victim.position.y - source_root.y,
        },
    ));
    assert!(world.set_player_state_for_diagnostic(1, victim));

    step_world_with_source_runtime_data(
        &mut world,
        Frame(0),
        &[
            PlayerInput::neutral(),
            PlayerInput::neutral().with_left_trigger_digital(true),
        ],
        |_| None,
        |action_state_id| match action_state_id.get() {
            199 => Some(26),
            183 | 191 => Some(26),
            _ => None,
        },
    );

    let passive = world.players()[1];
    let passive_snapshot = world.snapshot().players[1];
    assert!(
        passive.grounded,
        "expected DamageFly floor contact before Passive, got action {:?} key {:?} pos {:?} source_pos {:?} vel {:?} self_y {} kb {:?} coll_last {:?} coll_cur {:?} ecb_bottom {:?} floor {:?} env {:#x}",
        passive.melee_action_state_id,
        passive.source_action_key,
        passive.position,
        passive.source_position,
        passive.velocity,
        passive.source_self_velocity_y,
        (
            passive.source_knockback_velocity_x,
            passive.source_knockback_velocity_y
        ),
        passive_snapshot.source_coll_last_pos,
        passive_snapshot.source_coll_cur_pos,
        passive_snapshot.active_ecb.bottom,
        passive.source_floor_for_diagnostic(),
        passive_snapshot.source_coll_env_flags,
    );
    assert_eq!(passive.position.y, floor.y);
    assert_eq!(
        passive.melee_action_state_id,
        Some(MeleeActionStateId::new(199))
    );
    assert_eq!(
        passive.source_action_key,
        Some(SourceActionKey::new("Passive"))
    );
    assert_eq!(passive.motion_state_alias, None);
    assert_eq!(passive.source_action_total_frames, 26);
    assert_eq!(passive.motion_frame, 0);
    assert_eq!(
        passive.velocity.y,
        source_units_to_milli(
            passive.source_self_velocity_y + passive.source_knockback_velocity_y
        ),
        "ftCo_800987D0 grounds through ftCommon_8007D7FC, then ftCommon_8007CCE8 projects attack knockback onto the floor without clearing self_vel.y"
    );
}

#[test]
fn source_damage_fly_floor_contact_checks_passive_stand_before_passive_like_ftco_80098928() {
    for (stick_x, expected_id, expected_key) in
        [(127, 200, "PassiveStandF"), (-127, 201, "PassiveStandB")]
    {
        let common = MeleeCommonData {
            passive_stand_stick_x: 0.5,
            ..MeleeCommonData::PROVISIONAL
        };
        let mut world = World::for_two_players_with_common_data(common);
        let floor = world.stage().main_floor;
        let mut victim = world.players()[1];
        victim.grounded = false;
        victim.motion_state_alias = None;
        victim.motion_state = MotionState::Fall;
        victim.facing = 1;
        victim.melee_action_state_id = Some(MeleeActionStateId::new(91));
        victim.source_action_key = Some(SourceActionKey::new("DamageFlyRoll"));
        victim.motion_frame = 0;
        victim.motion_anim_frame_milli = 0;
        victim.damage_hitstun_frames = 10;
        victim.source_action_total_frames = 30;
        victim.source_down_bound_pose = Some(SourceDownBoundPose {
            hip_mtx_0_1: 0.0,
            hip_mtx_0_2: 0.0,
            hip_mtx_1_1: 1.0,
            hip_mtx_1_2: -1.0,
        });
        victim.position = Vec2 {
            x: (floor.left_x + floor.right_x) / 2,
            y: 0,
        };
        victim.velocity = Vec2 {
            x: source_units_to_milli(1.0),
            y: source_units_to_milli(-20.0),
        };
        assert!(world.set_player_state_for_diagnostic(1, victim));
        let source_bottom_offset_y =
            world.snapshot().players[1].active_ecb.bottom.y - world.players()[1].position.y;
        let mut victim = world.players()[1];
        victim.position.y = floor.y - source_bottom_offset_y + 500;
        victim.source_position.y = milli_to_source_units(victim.position.y);
        assert!(world.set_player_state_for_diagnostic(1, victim));

        step_world_with_source_runtime_data(
            &mut world,
            Frame(0),
            &[
                PlayerInput::neutral(),
                PlayerInput::neutral()
                    .with_left_trigger_digital(true)
                    .with_left_stick(stick_x, 0),
            ],
            |_| None,
            |action_state_id| match action_state_id.get() {
                199 => Some(26),
                200 | 201 => Some(40),
                183 | 191 => Some(26),
                _ => None,
            },
        );

        let passive = world.players()[1];
        assert!(
            passive.grounded,
            "expected floor contact before PassiveStand, got action {:?} key {:?} pos {:?} vel {:?}",
            passive.melee_action_state_id,
            passive.source_action_key,
            passive.position,
            passive.velocity
        );
        assert_eq!(passive.position.y, floor.y);
        assert_eq!(
            passive.melee_action_state_id,
            Some(MeleeActionStateId::new(expected_id))
        );
        assert_eq!(
            passive.source_action_key,
            Some(SourceActionKey::new(expected_key))
        );
        assert_eq!(passive.motion_state_alias, None);
        assert_eq!(passive.source_action_total_frames, 40);
        assert_eq!(passive.motion_frame, 0);
        assert_eq!(passive.velocity.y, 0);
    }
}

#[test]
fn source_passive_stand_b_phys_uses_source_transn_like_ft_80084fa8() {
    let mut world = World::for_two_players();
    let floor = world.stage().main_floor;
    let mut player = world.players()[1];
    player.grounded = true;
    player.motion_state = MotionState::GuardOn;
    player.motion_state_alias = None;
    player.melee_action_state_id = Some(MeleeActionStateId::new(201));
    player.source_action_key = Some(SourceActionKey::new("PassiveStandB"));
    player.source_action_total_frames = 40;
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.facing = -1;
    player.source_motion_entry_facing = -1;
    player.position = Vec2 {
        x: floor.left_x + 30_000,
        y: floor.y,
    };
    player.source_position.x = milli_to_source_units(player.position.x);
    player.source_position.y = milli_to_source_units(player.position.y);
    player.velocity = Vec2 { x: 0, y: 0 };
    player.ground_velocity_x = 0.0;
    player.source_self_velocity_x = 0.0;
    player.source_self_velocity_y = 0.0;
    assert!(world.set_player_state_for_diagnostic(1, player));
    let before_x = world.players()[1].position.x;
    let expected_velocity = -source_action_root_motion_delta_milli_for_profile(
        SourceActionKey::new("PassiveStandB"),
        2,
        FighterProfile::falcon_like(),
    );

    step_world_with_source_runtime_data(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
        |_| None,
        |action_state_id| match action_state_id.get() {
            201 => Some(40),
            _ => None,
        },
    );

    let player = world.players()[1];
    assert_eq!(
        player.melee_action_state_id,
        Some(MeleeActionStateId::new(201))
    );
    assert_eq!(
        player.source_action_key,
        Some(SourceActionKey::new("PassiveStandB"))
    );
    assert_eq!(player.motion_frame, 1);
    assert_eq!(player.velocity.x, expected_velocity);
    assert_eq!(player.position.x - before_x, expected_velocity);
    assert_eq!(
        source_units_to_milli(player.ground_velocity_x),
        expected_velocity
    );
    assert_eq!(
        source_units_to_milli(player.source_self_velocity_x),
        expected_velocity
    );
}

#[test]
fn source_passive_stand_b_phys_composes_source_knockback_like_fighter_proc_update() {
    let common = MeleeCommonData::provisional_mole();
    let mut world = World::for_two_players_with_common_data(common);
    let floor = world.stage().main_floor;
    let mut player = world.players()[1];
    player.grounded = true;
    player.motion_state = MotionState::GuardOn;
    player.motion_state_alias = None;
    player.melee_action_state_id = Some(MeleeActionStateId::new(201));
    player.source_action_key = Some(SourceActionKey::new("PassiveStandB"));
    player.source_action_total_frames = 40;
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.facing = -1;
    player.source_motion_entry_facing = -1;
    player.position = Vec2 {
        x: floor.left_x + 30_000,
        y: floor.y,
    };
    player.source_position.x = milli_to_source_units(player.position.x);
    player.source_position.y = milli_to_source_units(player.position.y);
    player.velocity = Vec2 { x: 0, y: 0 };
    player.ground_velocity_x = 0.0;
    player.source_self_velocity_x = 0.0;
    player.source_self_velocity_y = 0.0;
    player.source_knockback_velocity_x = 1.0;
    player.source_knockback_velocity_y = 0.0;
    player.source_ground_knockback_velocity = 0.0;
    assert!(world.set_player_state_for_diagnostic(1, player));
    let before_x = world.players()[1].position.x;
    let root_velocity = -source_action_root_motion_delta_milli_for_profile(
        SourceActionKey::new("PassiveStandB"),
        2,
        FighterProfile::falcon_like(),
    );
    let expected_knockback_source = source_apply_ground_friction_to_zero_f32(
        1.0,
        FighterProfile::falcon_like().ground_friction
            * common.damage_ground_knockback_friction_multiplier,
    );
    let expected_knockback = source_units_to_milli(expected_knockback_source);

    step_world_with_source_runtime_data(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
        |_| None,
        |action_state_id| match action_state_id.get() {
            201 => Some(40),
            _ => None,
        },
    );

    let player = world.players()[1];
    assert_eq!(
        player.position.x - before_x,
        root_velocity + expected_knockback
    );
    assert_eq!(
        source_units_to_milli(player.source_knockback_velocity_x),
        expected_knockback
    );
    assert_eq!(
        source_units_to_milli(player.ground_velocity_x),
        root_velocity
    );
}

#[test]
fn source_passive_stand_b_coll_uses_ft_80084104_floor_endpoint_clamp() {
    let mut world = World::for_two_players();
    let floor = world.stage().main_floor;
    let mut player = world.players()[1];
    player.grounded = true;
    player.motion_state = MotionState::GuardOn;
    player.motion_state_alias = None;
    player.melee_action_state_id = Some(MeleeActionStateId::new(201));
    player.source_action_key = Some(SourceActionKey::new("PassiveStandB"));
    player.source_action_total_frames = 40;
    player.motion_frame = 9;
    player.set_source_motion_anim_frame(9.0);
    player.facing = -1;
    player.source_motion_entry_facing = -1;
    player.position = Vec2 {
        x: floor.right_x - 100,
        y: floor.y,
    };
    player.source_position.x = milli_to_source_units(player.position.x);
    player.source_position.y = milli_to_source_units(player.position.y);
    player.velocity = Vec2 { x: 0, y: 0 };
    player.ground_velocity_x = 0.0;
    player.source_self_velocity_x = 0.0;
    player.source_self_velocity_y = 0.0;
    player.set_source_floor_for_diagnostic(Some(0), Some(5));
    assert!(world.set_player_state_for_diagnostic(1, player));

    step_world_with_source_runtime_data(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
        |_| None,
        |action_state_id| match action_state_id.get() {
            201 => Some(40),
            _ => None,
        },
    );

    let player = world.players()[1];
    assert!(
        player.grounded,
        "got action {:?} key {:?} pos {:?} source_pos {:?} floor {:?} env_flags {:#x}",
        player.melee_action_state_id,
        player.source_action_key,
        player.position,
        player.source_position,
        player.source_floor_for_diagnostic(),
        world.snapshot().players[1].source_coll_env_flags,
    );
    assert_eq!(
        player.melee_action_state_id,
        Some(MeleeActionStateId::new(201))
    );
    assert_eq!(
        player.source_action_key,
        Some(SourceActionKey::new("PassiveStandB"))
    );
    assert_eq!(player.position.x, floor.right_x);
    assert_eq!(player.position.y, floor.y);
}

#[test]
fn source_passive_stand_b_completed_pre_frame_runs_wait_jump_input() {
    let mut world = World::for_two_players();
    let floor = world.stage().main_floor;
    let mut player = world.players()[0];
    player.grounded = true;
    player.motion_state = MotionState::GuardOn;
    player.motion_state_alias = None;
    player.melee_action_state_id = Some(MeleeActionStateId::new(201));
    player.source_action_key = Some(SourceActionKey::new("PassiveStandB"));
    player.source_action_total_frames = 40;
    player.motion_frame = 39;
    player.set_source_motion_anim_frame(39.0);
    player.facing = -1;
    player.source_motion_entry_facing = -1;
    player.position = Vec2 {
        x: (floor.left_x + floor.right_x) / 2,
        y: floor.y,
    };
    player.source_position.x = milli_to_source_units(player.position.x);
    player.source_position.y = milli_to_source_units(player.position.y);
    player.velocity = Vec2 { x: 0, y: 0 };
    assert!(world.set_player_state_for_diagnostic(0, player));

    let jump = [
        PlayerInput::neutral().with_jump_secondary(true),
        PlayerInput::neutral(),
    ];

    step_world_with_source_runtime_data(
        &mut world,
        Frame(0),
        &jump,
        |_| None,
        |action_state_id| match action_state_id.get() {
            201 => Some(40),
            _ => None,
        },
    );

    assert_eq!(world.players()[0].motion_state, MotionState::KneeBend);
}

#[test]
fn source_damage_fly_floor_contact_falls_through_to_down_bound_like_ftco_damagefly_coll() {
    for (action_state_id, source_action_key) in [(87, "DamageFlyHi"), (91, "DamageFlyRoll")] {
        let mut world = World::for_two_players();
        let floor = world.stage().main_floor;
        let mut victim = world.players()[1];
        victim.grounded = false;
        victim.motion_state_alias = None;
        victim.motion_state = MotionState::Fall;
        victim.melee_action_state_id = Some(MeleeActionStateId::new(action_state_id));
        victim.source_action_key = Some(SourceActionKey::new(source_action_key));
        victim.motion_frame = 0;
        victim.motion_anim_frame_milli = 0;
        victim.damage_hitstun_frames = 10;
        victim.source_action_total_frames = 20;
        victim.source_down_bound_pose = Some(SourceDownBoundPose {
            hip_mtx_0_1: 0.0,
            hip_mtx_0_2: 0.0,
            hip_mtx_1_1: 1.0,
            hip_mtx_1_2: -1.0,
        });
        victim.position = Vec2 {
            x: floor.left_x + 10_000,
            y: 0,
        };
        victim.velocity = Vec2 {
            x: source_units_to_milli(1.0),
            y: source_units_to_milli(-4.0),
        };
        assert!(world.set_player_state_for_diagnostic(1, victim));
        let source_bottom_offset_y =
            world.snapshot().players[1].active_ecb.bottom.y - world.players()[1].position.y;
        let mut victim = world.players()[1];
        victim.position.y = floor.y - source_bottom_offset_y + 500;
        victim.source_position.y = milli_to_source_units(victim.position.y);
        assert!(world.set_player_state_for_diagnostic(1, victim));

        step_world_with_source_runtime_data(
            &mut world,
            Frame(0),
            &[PlayerInput::neutral(), PlayerInput::neutral()],
            |_| None,
            |action_state_id| match action_state_id.get() {
                183 | 191 => Some(26),
                _ => None,
            },
        );

        let bounded = world.players()[1];
        assert!(
            bounded.grounded,
            "{source_action_key} should snap to the floor"
        );
        assert_eq!(bounded.position.y, floor.y);
        assert_eq!(
            bounded.melee_action_state_id,
            Some(MeleeActionStateId::new(183)),
            "{source_action_key} should fall through passive checks to DownBoundU"
        );
        assert_eq!(
            bounded.source_action_key,
            Some(SourceActionKey::new("DownBoundU"))
        );
        assert_eq!(bounded.motion_state_alias, None);
        assert_eq!(bounded.motion_frame, 0);
        assert_eq!(bounded.velocity.y, 0);
    }
}

#[test]
fn source_down_bound_animation_end_enters_down_wait_like_ftco_80097e8c() {
    let common = MeleeCommonData {
        down_wait_timer: 61.0,
        ..MeleeCommonData::PROVISIONAL
    };
    let mut world = World::for_two_players_with_common_data(common);
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
    player.source_down_wait_timer = 0.0;
    player.position = Vec2 {
        x: floor.left_x + 10_000,
        y: floor.y,
    };
    assert!(world.set_player_state_for_diagnostic(1, player));

    step_world_with_source_runtime_data(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
        |_| None,
        |action_state_id| match action_state_id.get() {
            183 | 191 => Some(26),
            184 | 192 => Some(70),
            _ => None,
        },
    );

    let waiting = world.players()[1];
    assert!(waiting.grounded);
    assert_eq!(
        waiting.melee_action_state_id,
        Some(MeleeActionStateId::new(184))
    );
    assert_eq!(
        waiting.source_action_key,
        Some(SourceActionKey::new("DownWaitU"))
    );
    assert_eq!(waiting.motion_state_alias, None);
    assert_eq!(waiting.motion_frame, 0);
    assert_eq!(waiting.motion_anim_frame_milli, 0);
    assert_eq!(waiting.source_action_total_frames, 70);
    assert_eq!(waiting.source_down_wait_timer.to_bits(), 61.0_f32.to_bits());
}

#[test]
fn source_down_bound_ground_physics_decays_damage_knockback_like_xf0_ground_kb_vel() {
    let common = MeleeCommonData {
        damage_ground_knockback_friction_multiplier: 0.6375,
        ..MeleeCommonData::PROVISIONAL
    };
    let mut world = World::for_two_players_with_common_data(common);
    let floor = world.stage().main_floor;
    let mut player = world.players()[1];
    player.grounded = true;
    player.motion_state_alias = None;
    player.motion_state = MotionState::Fall;
    player.melee_action_state_id = Some(MeleeActionStateId::new(183));
    player.source_action_key = Some(SourceActionKey::new("DownBoundU"));
    player.source_action_total_frames = 26;
    player.motion_frame = 8;
    player.motion_anim_frame_milli = 8_000;
    player.position = Vec2 {
        x: floor.left_x + 10_000,
        y: floor.y,
    };
    player.source_position = SourceVec2::from_milli(player.position);
    player.ground_velocity_x = 0.0;
    player.velocity = Vec2 { x: 0, y: 0 };
    player.source_self_velocity_x = 0.0;
    player.source_self_velocity_y = 0.0;
    player.source_knockback_velocity_x = -0.396;
    player.source_knockback_velocity_y = 0.0;
    player.source_ground_knockback_velocity = 0.0;
    assert!(world.set_player_state_for_diagnostic(1, player));
    let before_x = world.players()[1].position.x;
    let expected_kb = source_apply_ground_friction_to_zero_f32(
        -0.396,
        FighterProfile::falcon_like().ground_friction
            * common.damage_ground_knockback_friction_multiplier,
    );

    step_world_with_source_runtime_data(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
        |_| None,
        |action_state_id| match action_state_id.get() {
            29 => Some(40),
            183 | 191 => Some(26),
            _ => None,
        },
    );

    let bounded = world.players()[1];
    assert_eq!(
        bounded.melee_action_state_id,
        Some(MeleeActionStateId::new(183))
    );
    assert!(bounded.grounded);
    assert_eq!(
        bounded.position.x - before_x,
        source_units_to_milli(expected_kb),
        "ftCo_DownBound_Phys calls ft_80084F3C while Fighter_procUpdate carries grounded x8c/xF0 damage knockback with x200 friction"
    );
    assert_eq!(bounded.ground_velocity_x.to_bits(), 0.0_f32.to_bits());
    assert_eq!(
        bounded.source_ground_knockback_velocity.to_bits(),
        expected_kb.to_bits()
    );
    assert_eq!(
        bounded.source_knockback_velocity_x.to_bits(),
        expected_kb.to_bits()
    );
    assert_eq!(
        bounded.source_knockback_velocity_y.to_bits(),
        0.0_f32.to_bits()
    );
}

#[test]
fn source_down_bound_phys_does_not_enter_fall_before_procmap_collision() {
    let mut stage = StageProfile::battlefield_test();
    stage.main_floor = StageSurface {
        name: "narrow_main_floor",
        kind: StageSurfaceKind::Solid,
        left_x: melee_units_f32(-2.0),
        right_x: melee_units_f32(2.0),
        y: 0,
        friction_multiplier: 1.0,
    };
    let mut world = World::for_two_players_on_stage(stage);
    let mut player = world.players()[1];
    player.grounded = true;
    player.motion_state_alias = None;
    player.motion_state = MotionState::Fall;
    player.melee_action_state_id = Some(MeleeActionStateId::new(183));
    player.source_action_key = Some(SourceActionKey::new("DownBoundU"));
    player.source_action_total_frames = 26;
    player.motion_frame = 5;
    player.motion_anim_frame_milli = 5_000;
    player.position = Vec2 {
        x: stage.main_floor.right_x - 100,
        y: stage.main_floor.y,
    };
    player.source_position = SourceVec2::from_milli(player.position);
    player.ground_velocity_x = 0.5;
    player.source_self_velocity_x = 0.5;
    player.velocity = Vec2 { x: 500, y: 0 };
    player.set_source_floor_for_diagnostic(
        Some(0),
        Some(source_floor_line_for_surface_at_x(
            stage,
            stage.main_floor,
            player.source_position.x,
        )),
    );
    assert!(world.set_player_state_for_diagnostic(1, player));

    step_world_with_source_runtime_data(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
        |_| None,
        |action_state_id| match action_state_id.get() {
            29 => Some(40),
            183 | 191 => Some(26),
            _ => None,
        },
    );

    let bounded = world.players()[1];
    assert_eq!(
        bounded.melee_action_state_id,
        Some(MeleeActionStateId::new(183)),
        "ftCo_DownBound_Phys is only ft_80084F3C; Fall_Enter belongs to the separate Fighter_procMap coll_cb"
    );
}

#[test]
fn source_down_wait_timer_expiry_enters_down_stand_like_ftco_downwait_anim() {
    for (wait_action_id, wait_key, stand_action_id, stand_key) in [
        (184, "DownWaitU", 186, "DownStandU"),
        (192, "DownWaitD", 194, "DownStandD"),
    ] {
        let mut world = World::for_two_players();
        let floor = world.stage().main_floor;
        let mut player = world.players()[1];
        player.grounded = true;
        player.motion_state_alias = None;
        player.motion_state = MotionState::Fall;
        player.melee_action_state_id = Some(MeleeActionStateId::new(wait_action_id));
        player.source_action_key = Some(SourceActionKey::new(wait_key));
        player.source_action_total_frames = 70;
        player.source_down_wait_timer = 1.0;
        player.motion_frame = 12;
        player.motion_anim_frame_milli = 12_000;
        player.position = Vec2 {
            x: floor.left_x + 10_000,
            y: floor.y,
        };
        assert!(world.set_player_state_for_diagnostic(1, player));

        step_world_with_source_runtime_data(
            &mut world,
            Frame(wait_action_id as u32),
            &[PlayerInput::neutral(), PlayerInput::neutral()],
            |_| None,
            |action_state_id| match action_state_id.get() {
                186 | 194 => Some(30),
                _ => None,
            },
        );

        let standing = world.players()[1];
        assert!(standing.grounded);
        assert_eq!(
            standing.melee_action_state_id,
            Some(MeleeActionStateId::new(stand_action_id))
        );
        assert_eq!(
            standing.source_action_key,
            Some(SourceActionKey::new(stand_key))
        );
        assert_eq!(standing.motion_state_alias, None);
        assert_eq!(standing.motion_frame, 0);
        assert_eq!(standing.motion_anim_frame_milli, 0);
        assert_eq!(standing.source_action_total_frames, 30);
        assert_eq!(standing.source_down_wait_timer.to_bits(), 0.0_f32.to_bits());
    }
}

#[test]
fn source_down_wait_fresh_lr_enters_down_stand_like_ftco_800980bc() {
    for (wait_action_id, wait_key, stand_action_id, stand_key) in [
        (184, "DownWaitU", 186, "DownStandU"),
        (192, "DownWaitD", 194, "DownStandD"),
    ] {
        let mut world = World::for_two_players();
        let floor = world.stage().main_floor;
        let mut player = world.players()[1];
        player.grounded = true;
        player.motion_state_alias = None;
        player.motion_state = MotionState::Fall;
        player.melee_action_state_id = Some(MeleeActionStateId::new(wait_action_id));
        player.source_action_key = Some(SourceActionKey::new(wait_key));
        player.source_action_total_frames = 70;
        player.source_down_wait_timer = 10.0;
        player.motion_frame = 12;
        player.motion_anim_frame_milli = 12_000;
        player.position = Vec2 {
            x: floor.left_x + 10_000,
            y: floor.y,
        };
        assert!(world.set_player_state_for_diagnostic(1, player));

        step_world_with_source_runtime_data(
            &mut world,
            Frame(wait_action_id as u32),
            &[
                PlayerInput::neutral(),
                PlayerInput::neutral().with_left_trigger_digital(true),
            ],
            |_| None,
            |action_state_id| match action_state_id.get() {
                186 | 194 => Some(30),
                _ => None,
            },
        );

        let standing = world.players()[1];
        assert!(standing.grounded);
        assert_eq!(
            standing.melee_action_state_id,
            Some(MeleeActionStateId::new(stand_action_id))
        );
        assert_eq!(
            standing.source_action_key,
            Some(SourceActionKey::new(stand_key))
        );
        assert_eq!(standing.motion_state_alias, None);
        assert_eq!(standing.motion_frame, 0);
        assert_eq!(standing.motion_anim_frame_milli, 0);
        assert_eq!(standing.source_action_total_frames, 30);
        assert_eq!(standing.source_down_wait_timer.to_bits(), 0.0_f32.to_bits());
    }
}

#[test]
fn source_down_wait_up_stick_angle_enters_down_stand_like_ftco_800980bc() {
    for (wait_action_id, wait_key, stand_action_id, stand_key) in [
        (184, "DownWaitU", 186, "DownStandU"),
        (192, "DownWaitD", 194, "DownStandD"),
    ] {
        let common = MeleeCommonData {
            aerial_vertical_angle_tan_milli: 1000,
            down_stand_stick_y: 72,
            ..MeleeCommonData::PROVISIONAL
        };
        let mut world = World::for_two_players_with_common_data(common);
        let floor = world.stage().main_floor;
        let mut player = world.players()[1];
        player.grounded = true;
        player.motion_state_alias = None;
        player.motion_state = MotionState::Fall;
        player.melee_action_state_id = Some(MeleeActionStateId::new(wait_action_id));
        player.source_action_key = Some(SourceActionKey::new(wait_key));
        player.source_action_total_frames = 70;
        player.source_down_wait_timer = 10.0;
        player.motion_frame = 12;
        player.motion_anim_frame_milli = 12_000;
        player.position = Vec2 {
            x: floor.left_x + 10_000,
            y: floor.y,
        };
        assert!(world.set_player_state_for_diagnostic(1, player));

        step_world_with_source_runtime_data(
            &mut world,
            Frame(wait_action_id as u32),
            &[
                PlayerInput::neutral(),
                PlayerInput::neutral().with_left_stick(80, 80),
            ],
            |_| None,
            |action_state_id| match action_state_id.get() {
                186 | 194 => Some(30),
                _ => None,
            },
        );

        let standing = world.players()[1];
        assert!(standing.grounded);
        assert_eq!(
            standing.melee_action_state_id,
            Some(MeleeActionStateId::new(stand_action_id))
        );
        assert_eq!(
            standing.source_action_key,
            Some(SourceActionKey::new(stand_key))
        );
        assert_eq!(standing.motion_state_alias, None);
        assert_eq!(standing.motion_frame, 0);
        assert_eq!(standing.motion_anim_frame_milli, 0);
        assert_eq!(standing.source_action_total_frames, 30);
        assert_eq!(standing.source_down_wait_timer.to_bits(), 0.0_f32.to_bits());
    }
}

#[test]
fn source_down_wait_fresh_attack_enters_down_attack_like_ftco_800984d4() {
    for (wait_action_id, wait_key, attack_action_id, attack_key) in [
        (184, "DownWaitU", 187, "DownAttackU"),
        (192, "DownWaitD", 195, "DownAttackD"),
    ] {
        let mut world = World::for_two_players();
        let floor = world.stage().main_floor;
        let mut player = world.players()[1];
        player.grounded = true;
        player.motion_state_alias = None;
        player.motion_state = MotionState::Fall;
        player.melee_action_state_id = Some(MeleeActionStateId::new(wait_action_id));
        player.source_action_key = Some(SourceActionKey::new(wait_key));
        player.source_action_total_frames = 70;
        player.source_down_wait_timer = 10.0;
        player.motion_frame = 12;
        player.motion_anim_frame_milli = 12_000;
        player.position = Vec2 {
            x: floor.left_x + 10_000,
            y: floor.y,
        };
        assert!(world.set_player_state_for_diagnostic(1, player));

        step_world_with_source_runtime_data(
            &mut world,
            Frame(wait_action_id as u32),
            &[
                PlayerInput::neutral(),
                PlayerInput::neutral().with_attack(true),
            ],
            |_| None,
            |action_state_id| match action_state_id.get() {
                187 | 195 => Some(40),
                _ => None,
            },
        );

        let attacking = world.players()[1];
        assert!(attacking.grounded);
        assert_eq!(
            attacking.melee_action_state_id,
            Some(MeleeActionStateId::new(attack_action_id))
        );
        assert_eq!(
            attacking.source_action_key,
            Some(SourceActionKey::new(attack_key))
        );
        assert_eq!(attacking.motion_state_alias, None);
        assert_eq!(attacking.motion_frame, 0);
        assert_eq!(attacking.motion_anim_frame_milli, 0);
        assert_eq!(attacking.source_action_total_frames, 40);
        assert_eq!(
            attacking.source_down_wait_timer.to_bits(),
            9.0_f32.to_bits()
        );
    }
}

#[test]
fn world_applies_source_collision_frame_through_decomp_damage_pipeline() {
    let mut world = World::for_two_players();
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
    let collision_frame = SourceCollisionFrame {
        hits: vec![SourceCollisionCapsule::new(
            0,
            1,
            Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), 1.0),
        )
        .with_owner_grounded(true)
        .with_source_pose(
            Some(MeleeActionStateId::new(65)),
            Some(SourceActionKey::new("AttackAirN")),
            7,
        )
        .with_hitbox_attributes(hitbox)],
        hurts: vec![SourceCollisionCapsule::new(
            1,
            10,
            Capsule3::new(Vec3::new(0.5, 0.0, 0.0), Vec3::new(1.5, 0.0, 0.0), 1.0),
        )
        .with_owner_grounded(false)],
    };

    let step = world.apply_source_collision_frame(&collision_frame);

    assert_eq!(step.confirms.len(), 1);
    assert_eq!(step.stages.len(), 1);
    assert_eq!(step.results.len(), 1);
    assert_eq!(step.applied_stage_count, 1);
    assert_eq!(step.stages[0].unk_count, 10);
    assert_eq!(step.results[0].stage.victim_index, 1);
    assert!(step.results[0].knockback > 0.0);
    assert_eq!(world.players()[1].damage_percent, 0.0);
    assert_eq!(world.players()[1].damage_percent_temp, 10.0);
    assert_eq!(world.players()[1].damage_applied, 10);
}

#[test]
fn source_hurt_intangible_state_suppresses_hit_confirms_like_ftcoll_x198c() {
    let mut world = World::for_two_players();
    let mut victim = world.players()[1];
    victim.apply_source_hurt_intangible_timer(12);
    assert!(world.set_player_state_for_diagnostic(1, victim));

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
    let collision_frame = SourceCollisionFrame {
        hits: vec![SourceCollisionCapsule::new(
            0,
            1,
            Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), 1.0),
        )
        .with_owner_grounded(true)
        .with_source_pose(
            Some(MeleeActionStateId::new(65)),
            Some(SourceActionKey::new("AttackAirN")),
            7,
        )
        .with_hitbox_attributes(hitbox)],
        hurts: vec![SourceCollisionCapsule::new(
            1,
            10,
            Capsule3::new(Vec3::new(0.5, 0.0, 0.0), Vec3::new(1.5, 0.0, 0.0), 1.0),
        )
        .with_owner_grounded(false)],
    };

    let step = world.apply_source_collision_frame(&collision_frame);

    assert_eq!(step.confirms.len(), 0);
    assert_eq!(step.stages.len(), 0);
    assert_eq!(step.results.len(), 0);
    assert_eq!(step.applied_stage_count, 0);
    assert_eq!(world.players()[1].damage_percent_temp, 0.0);
}

#[test]
fn world_source_collision_logs_hit_victims_by_hit_group_like_hitcapsule_victims_1() {
    let mut world = World::for_two_players();
    let strong_hitbox = SourceHitboxAttributes {
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
    let weak_hitbox = SourceHitboxAttributes {
        damage: 3,
        ..strong_hitbox
    };
    let collision_frame = SourceCollisionFrame {
        hits: vec![
            SourceCollisionCapsule::new(
                0,
                1,
                Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), 1.0),
            )
            .with_source_pose(
                Some(MeleeActionStateId::new(65)),
                Some(SourceActionKey::new("AttackAirN")),
                7,
            )
            .with_hitbox_attributes(strong_hitbox),
            SourceCollisionCapsule::new(
                0,
                2,
                Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), 1.0),
            )
            .with_source_pose(
                Some(MeleeActionStateId::new(65)),
                Some(SourceActionKey::new("AttackAirN")),
                7,
            )
            .with_hitbox_attributes(weak_hitbox),
        ],
        hurts: vec![SourceCollisionCapsule::new(
            1,
            10,
            Capsule3::new(Vec3::new(0.5, 0.0, 0.0), Vec3::new(1.5, 0.0, 0.0), 1.0),
        )
        .with_owner_grounded(false)],
    };

    let step = world.apply_source_collision_frame(&collision_frame);

    assert_eq!(step.confirms.len(), 1);
    assert_eq!(step.stages.len(), 1);
    assert_eq!(step.applied_stage_count, 1);
    assert_eq!(step.stages[0].hitbox_id, 1);
    assert_eq!(world.players()[1].damage_percent_temp, 5.0);
    assert_eq!(world.players()[1].damage_applied, 5);
}

#[test]
fn world_source_collision_keeps_hit_victim_log_while_active_hitbox_remains() {
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
    let collision_frame = SourceCollisionFrame {
        hits: vec![SourceCollisionCapsule::new(
            0,
            1,
            Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), 1.0),
        )
        .with_source_pose(
            Some(MeleeActionStateId::new(65)),
            Some(SourceActionKey::new("AttackAirN")),
            7,
        )
        .with_hitbox_attributes(hitbox)],
        hurts: vec![SourceCollisionCapsule::new(
            1,
            10,
            Capsule3::new(Vec3::new(0.5, 0.0, 0.0), Vec3::new(1.5, 0.0, 0.0), 1.0),
        )
        .with_owner_grounded(false)],
    };

    let first = world.apply_source_collision_frame(&collision_frame);
    let after_first_damage = world.players()[1].damage_percent_temp;
    let second = world.apply_source_collision_frame(&collision_frame);

    assert_eq!(first.applied_stage_count, 1);
    assert_eq!(after_first_damage, 5.0);
    assert!(second.confirms.is_empty());
    assert!(second.stages.is_empty());
    assert_eq!(second.applied_stage_count, 0);
    assert_eq!(world.players()[1].damage_percent_temp, after_first_damage);
}

#[test]
fn walk_records_source_x0_and_anim_rate_from_bucket_velocity() {
    let mut world = World::for_two_players();
    let stick_x = 80;
    let walk_right = [
        PlayerInput::neutral().with_left_stick(stick_x, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &walk_right);

    assert_eq!(world.players()[0].motion_state, MotionState::WalkSlow);
    assert_eq!(
        world.players()[0].walk_anim_velocity_x,
        source_stick_scaled_velocity_f32(
            stick_x as i32,
            world.players()[0].profile.walk_max_velocity
        ) * world.common_data().animation_velocity_scale
    );

    let first_walk_velocity = world.players()[0].ground_velocity_x;
    assert!(first_walk_velocity > 0.0);

    step_world(&mut world, Frame(1), &walk_right);

    assert_eq!(world.players()[0].motion_state, MotionState::WalkSlow);
    assert_eq!(
        world.players()[0].motion_anim_rate_milli,
        (first_walk_velocity * 1000.0 / world.players()[0].profile.slow_walk_max_velocity).round()
            as i32
    );
}

#[test]
fn walk_snapshot_pose_frame_advances_by_source_anim_rate_not_boolean_tick() {
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
    let source_anim_rate_milli = world.snapshot().players[0].motion_anim_rate_milli;
    assert!(source_anim_rate_milli >= 2_000);
    step_world(&mut world, Frame(3), &walk_right);

    let player = world.snapshot().players[0];
    assert_eq!(player.state_frame, 3);
    assert!(player.motion_anim_rate_milli >= 2_000);
    let expected_pose_frame = (2 + source_anim_rate_milli / 1_000) as u8;
    assert_eq!(
        player.animation_frame, expected_pose_frame,
        "render pose sampling should use the source animation rate, not just whether the rate is nonzero"
    );
}

#[test]
fn active_ecb_samples_the_source_pose_frame_not_state_age() {
    let mut state_age_world = World::for_two_players();
    let mut source_pose_world = World::for_two_players();
    let mut state_age_player = state_age_world.players()[0];
    state_age_player.motion_state = MotionState::WalkSlow;
    state_age_player.motion_frame = 3;
    state_age_player.motion_anim_frame_milli = 3_000;
    state_age_player.grounded = true;
    let mut source_pose_player = state_age_player;
    source_pose_player.motion_anim_frame_milli = 8_000;
    assert!(state_age_world.set_player_state_for_diagnostic(0, state_age_player));
    assert!(source_pose_world.set_player_state_for_diagnostic(0, source_pose_player));

    let state_age_snapshot = state_age_world.snapshot().players[0];
    let source_pose_snapshot = source_pose_world.snapshot().players[0];

    assert_eq!(
        state_age_snapshot.state_frame,
        source_pose_snapshot.state_frame
    );
    assert_ne!(
        state_age_snapshot.animation_frame,
        source_pose_snapshot.animation_frame
    );
    assert_ne!(
        state_age_snapshot.active_ecb, source_pose_snapshot.active_ecb,
        "Melee derives ECB from the current animated JObj pose, not gameplay state age alone"
    );
}

#[test]
fn catch_tick_advances_live_source_animation_pose() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.set_motion_state_alias(MotionState::Catch);
    player.grounded = true;
    player.motion_frame = 5;
    player.motion_anim_frame_milli = 5_000;
    player.motion_anim_rate_milli = 1_000;
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    let player = world.snapshot().players[0];
    assert_eq!(player.state_frame, 6);
    assert_eq!(
        player.animation_frame_milli, 6_000,
        "decomp Catch uses the normal ftAnim_8006EBA4 JObj clock; state age must not advance while the source pose stays frozen"
    );
    assert_eq!(player.animation_frame, 6);
}

#[test]
fn jumpf_active_ecb_samples_the_source_pose_frame_not_state_age() {
    let mut state_age_world = World::for_two_players();
    let mut source_pose_world = World::for_two_players();
    let mut state_age_player = state_age_world.players()[0];
    state_age_player.motion_state = MotionState::JumpF;
    state_age_player.motion_state_alias = Some(MotionState::JumpF);
    state_age_player.motion_frame = 3;
    state_age_player.motion_anim_frame_milli = 3_000;
    state_age_player.grounded = false;
    let mut source_pose_player = state_age_player;
    source_pose_player.motion_anim_frame_milli = 8_000;
    assert!(state_age_world.set_player_state_for_diagnostic(0, state_age_player));
    assert!(source_pose_world.set_player_state_for_diagnostic(0, source_pose_player));

    let state_age_snapshot = state_age_world.snapshot().players[0];
    let source_pose_snapshot = source_pose_world.snapshot().players[0];

    assert_eq!(
        state_age_snapshot.state_frame,
        source_pose_snapshot.state_frame
    );
    assert_ne!(
        state_age_snapshot.animation_frame,
        source_pose_snapshot.animation_frame
    );
    assert_ne!(
        state_age_snapshot.active_ecb, source_pose_snapshot.active_ecb,
        "Jump ECB must come from the animated source pose frame, not the integer gameplay state age"
    );
}

#[test]
fn charley_walk_negative_slide_stalls_run_pose_before_relay() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.motion_state = MotionState::Run;
    player.motion_frame = 9;
    player.motion_anim_frame_milli = 9_000;
    player.motion_anim_rate_milli = 1_000;
    player.facing = 1;
    player.run_no_interrupt_frames = 2;
    player.ground_velocity_x = -1.2;
    player.velocity.x = source_units_to_milli(-1.2);
    assert!(world.set_player_state_for_diagnostic(0, player));

    let charley_hold = [
        PlayerInput::neutral().with_left_stick(24, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &charley_hold);

    let player = world.snapshot().players[0];
    assert_eq!(player.motion_state, MotionState::Run);
    assert_eq!(player.motion_anim_rate_milli, 0);
    assert_eq!(
        player.animation_frame, 9,
        "Charley walk should visually stall the Run pose while gr_vel slides opposite facing, not advance one stale-rate pose frame"
    );
}

#[test]
fn walk_bucket_remap_preserves_source_motion_frame_and_resets_change_state_rate() {
    let profile = FighterProfile {
        walk_initial_velocity: 0.5,
        walk_accel: 0.0,
        walk_max_velocity: 1.0,
        slow_walk_max_velocity: 0.2,
        mid_walk_point: 0.4,
        fast_walk_min: 0.8,
        ground_friction: 0.0,
        ..FighterProfile::falcon_like()
    };
    let common = MeleeCommonData {
        walk_middle_velocity_ratio: 0.3,
        walk_fast_velocity_ratio: 0.8,
        animation_velocity_scale: 1.3,
        ..MeleeCommonData::provisional_mole()
    };
    let mut world = World::for_two_players_on_stage_with_profiles_and_common_data(
        StageProfile::battlefield_test(),
        [profile; 2],
        common,
    );
    let stick_x = 101;
    let walk_right = [
        PlayerInput::neutral().with_left_stick(stick_x, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &walk_right);
    assert_eq!(world.players()[0].motion_state, MotionState::WalkSlow);
    assert_eq!(world.players()[0].motion_frame, 0);
    let first_velocity = source_stick_scaled_velocity_f32(stick_x as i32, 0.5);
    assert_eq!(world.players()[0].ground_velocity_x, first_velocity);

    step_world(&mut world, Frame(1), &walk_right);

    assert_eq!(world.players()[0].motion_state, MotionState::WalkMiddle);
    assert_eq!(
        world.players()[0].motion_frame,
        1,
        "ftWalkCommon_800DFEC8 remaps buckets without resetting the animation phase"
    );
    assert_eq!(
        world.players()[0].motion_anim_rate_milli,
        1_000,
        "ftWalkCommon_800DFEC8 re-enters Walk through ftCo_Walk_Enter, whose motion-state change uses anim rate 1.0 until the next Walk_Anim pass"
    );
    assert_eq!(
        world.players()[0].walk_anim_velocity_x,
        source_stick_scaled_velocity_f32(stick_x as i32, profile.walk_max_velocity)
            * common.animation_velocity_scale
    );
}

#[test]
fn source_locomotion_action_frames_participate_in_profile_checksum() {
    let base = World::for_two_players();
    let action_frames = FighterActionFrames {
        turn_run_total_frames: FighterActionFrames::falcon_like()
            .turn_run_total_frames
            .saturating_add(1),
        ..FighterActionFrames::falcon_like()
    };
    let altered_profile = FighterProfile {
        action_frames,
        ..FighterProfile::falcon_like()
    };
    let altered =
        World::for_two_players_with_profiles([altered_profile, FighterProfile::falcon_like()]);

    assert_ne!(base.checksum(), altered.checksum());
}

#[test]
fn source_float_profile_fields_participate_in_profile_checksum_by_bits() {
    let base = World::for_two_players();
    let altered_profile = FighterProfile {
        dash_run_acceleration_a: f32::from_bits(
            FighterProfile::falcon_like()
                .dash_run_acceleration_a
                .to_bits()
                .wrapping_add(1),
        ),
        ..FighterProfile::falcon_like()
    };
    let altered =
        World::for_two_players_with_profiles([altered_profile, FighterProfile::falcon_like()]);

    assert_ne!(base.checksum(), altered.checksum());
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
    step_world(&mut world, Frame(1), &inputs);

    let mut snapshot = world.snapshot();
    let player = snapshot.players[0];

    assert_eq!(snapshot.frame, world.frame());
    assert_eq!(snapshot.checksum, world.checksum());
    assert_eq!(player.position, world.players()[0].position);
    assert_eq!(player.facing, world.players()[0].facing);
    assert_eq!(player.motion_state, MotionState::WalkSlow);
    assert_eq!(player.state_frame, world.players()[0].motion_frame);
    assert_eq!(player.animation_frame, world.players()[0].motion_frame);
    assert_eq!(player.debug_input_facts.walk_direction, 1);
    assert_eq!(
        player.debug_input_facts.walk_speed_bucket,
        WalkSpeedBucket::Middle
    );

    snapshot.players[0].position.x += 777;

    assert_ne!(snapshot.players[0].position, world.players()[0].position);
}

#[test]
fn world_carries_canonical_melee_action_identity_alongside_motion_alias() {
    let mut world = World::for_two_players();

    assert_eq!(
        world.players()[0].melee_action_state_id,
        Some(MeleeActionStateId::new(14))
    );
    assert_eq!(
        world.players()[0].source_action_key,
        Some(SourceActionKey::new("Wait1"))
    );
    assert_eq!(
        world.players()[0].motion_state_alias,
        Some(MotionState::Wait)
    );

    let mut player = world.players()[0];
    player.grounded = false;
    player.motion_state = MotionState::Fall;
    player.motion_frame = 0;
    player.position.y = melee_units_f32(30.0);
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(0),
        &[
            PlayerInput::neutral().with_attack(true),
            PlayerInput::neutral(),
        ],
    );

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::AttackAirN);
    assert_eq!(
        player.melee_action_state_id,
        Some(MeleeActionStateId::new(65))
    );
    assert_eq!(
        player.source_action_key,
        Some(SourceActionKey::new("AttackAirN"))
    );
    assert_eq!(player.motion_state_alias, Some(MotionState::AttackAirN));

    let snapshot = world.snapshot().players[0];
    assert_eq!(
        snapshot.melee_action_state_id,
        Some(MeleeActionStateId::new(65))
    );
    assert_eq!(
        snapshot.source_action_key,
        Some(SourceActionKey::new("AttackAirN"))
    );
    assert_eq!(snapshot.motion_state_alias, Some(MotionState::AttackAirN));

    let mut player = world.players()[0];
    player.set_motion_state_alias(MotionState::Entry);
    assert_eq!(
        player.melee_action_state_id,
        Some(MeleeActionStateId::new(322))
    );
    assert_eq!(
        player.source_action_key,
        Some(SourceActionKey::new("Entry"))
    );

    player.set_motion_state_alias(MotionState::EntryEnd);
    assert_eq!(
        player.melee_action_state_id,
        Some(MeleeActionStateId::new(324))
    );
    assert_eq!(
        player.source_action_key,
        Some(SourceActionKey::new("Entry"))
    );
}

#[test]
fn wait_anim_advances_source_pose_like_ftco_wait_anim() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.set_motion_state_alias(MotionState::Wait);
    player.motion_frame = 0;
    player.motion_anim_frame_milli = 0;
    player.motion_anim_rate_milli = 1_000;
    player.grounded = true;
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    let player = world.snapshot().players[0];
    assert_eq!(player.motion_state, MotionState::Wait);
    assert_eq!(player.state_frame, 1);
    assert_eq!(player.animation_frame, 1);
    assert_eq!(player.source_pose_frame, 1);
    assert_eq!(
        player.source_pose_action_key,
        Some(SourceActionKey::new("Wait1"))
    );
}

#[test]
fn world_derives_melee_snapshot_from_rollback_owned_input_state() {
    let mut world = World::for_two_players();
    let neutral = PlayerInput::neutral();
    let input = PlayerInput::neutral()
        .with_left_stick(dash_stick_x(), 0)
        .with_c_stick(0, 90)
        .with_attack(true)
        .with_right_trigger_analog(80)
        .with_right_trigger_digital(true)
        .with_dpad_up(true);

    let snapshot = world
        .melee_input_snapshot(0, input)
        .expect("player one snapshot should exist");
    let facts = snapshot.facts(MeleeInputThresholds::default());

    assert_eq!(snapshot.lstick, (dash_stick_x(), 0));
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

    assert_eq!(held_snapshot.prev_lstick, (dash_stick_x(), 0));
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
fn world_melee_snapshot_treats_analog_trigger_above_deadzone_as_shield_not_air_dodge() {
    let world = World::for_two_players();
    let input = PlayerInput::neutral().with_left_trigger_analog(90);

    let snapshot = world
        .melee_input_snapshot(0, input)
        .expect("player one snapshot should exist");
    let facts = snapshot.facts(MeleeInputThresholds::default());

    assert!(snapshot.shield_held);
    assert!(snapshot.shield_pressed);
    assert!(snapshot.left_trigger_analog_held);
    assert!(snapshot.left_trigger_analog_pressed);
    assert_eq!(snapshot.trigger_timer, 0);
    assert_eq!(facts.analog_shield, 90);
    assert!(facts.analog_shield_pressed);
    assert!(!facts.digital_shield_pressed);
    assert!(!facts.air_dodge_pressed);
}

#[test]
fn grounded_jump_waits_through_falcon_jumpsquat_before_takeoff() {
    let mut world = World::for_two_players();
    let profile = FighterProfile::falcon_like();
    let start_y = world.players()[0].position.y;
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &jump);

    assert!(world.players()[0].grounded);
    assert_eq!(world.players()[0].position.y, start_y);
    assert_eq!(world.players()[0].velocity.y, 0);
    assert_eq!(world.players()[0].motion_state, MotionState::KneeBend);
    assert_eq!(world.players()[0].motion_frame, 0);

    for frame in 1..=3 {
        step_world(&mut world, Frame(frame), &jump);
        assert!(world.players()[0].grounded);
        assert_eq!(world.players()[0].position.y, start_y);
        assert_eq!(world.players()[0].motion_state, MotionState::KneeBend);
        assert_eq!(world.players()[0].motion_frame, frame as u8);
    }

    step_world(&mut world, Frame(4), &jump);

    assert!(!world.players()[0].grounded);
    assert_eq!(
        world.players()[0].position.y,
        start_y + source_units_to_milli(profile.jump_vertical_initial_velocity)
    );
    assert!(world.players()[0].velocity.y > 0);
}

#[test]
fn ground_jump_takeoff_enters_forward_or_backward_jump_state_from_stick() {
    let mut forward = World::for_two_players();
    let mut backward = World::for_two_players();
    let forward_jump = [
        PlayerInput::neutral()
            .with_jump(true)
            .with_left_stick(60, 0),
        PlayerInput::neutral(),
    ];
    let backward_jump = [
        PlayerInput::neutral()
            .with_jump(true)
            .with_left_stick(-60, 0),
        PlayerInput::neutral(),
    ];

    for frame in 0..5 {
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
    for frame in 1..=4 {
        step_world(&mut full_hop, Frame(frame), &jump);
        step_world(&mut short_hop, Frame(frame), &neutral);
    }

    assert!(!full_hop.players()[0].grounded);
    assert!(!short_hop.players()[0].grounded);
    assert!(short_hop.players()[0].velocity.y < full_hop.players()[0].velocity.y);
    assert!(short_hop.players()[0].velocity.y > 0);
}

#[test]
fn ground_jump_first_post_takeoff_physics_tick_applies_falcon_gravity_before_translation() {
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
    step_world(&mut full_hop, Frame(4), &jump);
    step_world(&mut short_hop, Frame(4), &neutral);
    step_world(&mut full_hop, Frame(5), &neutral);
    step_world(&mut short_hop, Frame(5), &neutral);

    assert_eq!(
        full_hop.players()[0].position.y - start_y,
        source_units_to_milli(profile.jump_vertical_initial_velocity * 2.0 - profile.gravity)
    );
    assert_eq!(
        full_hop.players()[0].velocity.y,
        source_units_to_milli(profile.jump_vertical_initial_velocity - profile.gravity)
    );
    assert_eq!(
        short_hop.players()[0].position.y - start_y,
        source_units_to_milli(profile.hop_vertical_initial_velocity * 2.0 - profile.gravity)
    );
    assert_eq!(
        short_hop.players()[0].velocity.y,
        source_units_to_milli(profile.hop_vertical_initial_velocity - profile.gravity)
    );
}

#[test]
fn ground_jump_takeoff_state_change_translates_by_jump_velocity_without_first_tick_gravity() {
    let mut world = World::for_two_players();
    let profile = FighterProfile::falcon_like();
    let start_y = world.players()[0].position.y;
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    for frame in 0..=4 {
        step_world(&mut world, Frame(frame), &jump);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::JumpF);
    assert!(!world.players()[0].grounded);
    assert_eq!(
        world.players()[0].position.y - start_y,
        source_units_to_milli(profile.jump_vertical_initial_velocity)
    );
    assert_eq!(
        world.players()[0].velocity.y,
        source_units_to_milli(profile.jump_vertical_initial_velocity)
    );

    step_world(&mut world, Frame(5), &neutral);

    assert_eq!(
        world.players()[0].position.y - start_y,
        source_units_to_milli(profile.jump_vertical_initial_velocity * 2.0 - profile.gravity)
    );
    assert_eq!(
        world.players()[0].velocity.y,
        source_units_to_milli(profile.jump_vertical_initial_velocity - profile.gravity)
    );
}

#[test]
fn grounded_jump_entry_runs_knee_bend_phys_on_the_same_frame() {
    let mut world = World::for_two_players();
    let common = world.common_data();
    let mut player = world.players()[0];
    player.motion_state = MotionState::Wait;
    player.ground_velocity_x = player.profile.dash_run_terminal_velocity;
    player.velocity.x = source_units_to_milli(player.profile.dash_run_terminal_velocity);
    let start_x = player.position.x;
    let start_velocity = player.velocity.x;
    assert!(world.set_player_state_for_diagnostic(0, player));
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let expected_velocity =
        source_general_grounded_friction_velocity(start_velocity, player.profile, common);

    step_world(&mut world, Frame(0), &jump);

    assert_eq!(world.players()[0].motion_state, MotionState::KneeBend);
    assert_eq!(
        world.players()[0].velocity.x,
        expected_velocity,
        "ftCo_Jump_CheckInput enters KneeBend before ftCo_KneeBend_Phys runs"
    );
    assert_eq!(
        world.players()[0].position.x,
        start_x + expected_velocity,
        "ft_80084F3C contributes xE4 friction to same-frame ground movement"
    );
}

#[test]
fn ground_jump_takeoff_uses_previous_knee_bend_ground_velocity_without_takeoff_friction() {
    let mut world = World::for_two_players();
    let common = world.common_data();
    let mut player = world.players()[0];
    player.motion_state = MotionState::Wait;
    player.ground_velocity_x = player.profile.dash_run_terminal_velocity;
    player.velocity.x = source_units_to_milli(player.profile.dash_run_terminal_velocity);
    let mut expected_ground_velocity = player.ground_velocity_x;
    assert!(world.set_player_state_for_diagnostic(0, player));
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];

    for frame in 0..=3 {
        expected_ground_velocity = source_general_grounded_friction_velocity_f32(
            expected_ground_velocity,
            world.players()[0].profile,
            common,
        );
        step_world(&mut world, Frame(frame), &jump);
        assert_eq!(world.players()[0].motion_state, MotionState::KneeBend);
    }

    step_world(&mut world, Frame(4), &jump);

    assert_eq!(world.players()[0].motion_state, MotionState::JumpF);
    assert_eq!(
        world.players()[0].velocity.x,
        source_units_to_milli(
            expected_ground_velocity
                * world.players()[0]
                    .profile
                    .ground_to_air_jump_momentum_multiplier
        ),
        "ftCo_Jump_Enter scales the carried self_vel from the prior grounded frame"
    );
}

#[test]
fn ground_jump_takeoff_uses_pre_input_stick_from_knee_bend_anim_frame() {
    let profile = FighterProfile::falcon_like();
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let mut player = PlayerState::new_with_profile(0, 0, -1, profile);
    player.motion_state = MotionState::KneeBend;
    player.motion_frame = profile.jumpsquat_frames - 1;
    player.grounded = true;
    player.ground_velocity_x = milli_to_source_units(95);
    player.velocity.x = 95;
    world.set_player_state_for_diagnostic(0, player);
    world.set_input_history_for_diagnostic(
        [
            PlayerInput::neutral()
                .with_jump(true)
                .with_left_stick(102, -75),
            PlayerInput::neutral(),
        ],
        [MeleeInputTimers::expired(); 2],
    );

    let current_takeoff_input = [
        PlayerInput::neutral()
            .with_jump(true)
            .with_left_stick(110, -63),
        PlayerInput::neutral(),
    ];
    step_world(&mut world, Frame(0), &current_takeoff_input);

    let expected_velocity = source_units_to_milli(
        milli_to_source_units(95) * profile.ground_to_air_jump_momentum_multiplier
            + source_stick_scaled_velocity_f32(102, profile.jump_horizontal_initial_velocity),
    );
    assert_eq!(world.players()[0].motion_state, MotionState::JumpB);
    assert_eq!(
        world.players()[0].velocity.x, expected_velocity,
        "ftCo_KneeBend_Anim enters Jump before the frame's input callback, so ftCo_Jump_Enter uses the prior cleaned lstick.x for initial horizontal jump velocity"
    );
}

#[test]
fn ground_jump_takeoff_carries_full_walk_speed_after_source_jumpsquat_friction() {
    let mut world = World::for_two_players();
    let common = world.common_data();
    let soft_right = [
        PlayerInput::neutral().with_left_stick(40, 0),
        PlayerInput::neutral(),
    ];
    let full_walk_right = [
        PlayerInput::neutral().with_left_stick(127, 0),
        PlayerInput::neutral(),
    ];
    let neutral_jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];

    for frame in 0..=2 {
        step_world(&mut world, Frame(frame), &soft_right);
    }
    for frame in 3..=40 {
        step_world(&mut world, Frame(frame), &full_walk_right);
    }

    assert!(matches!(
        world.players()[0].motion_state,
        MotionState::WalkSlow | MotionState::WalkMiddle | MotionState::WalkFast
    ));
    let walk_velocity = world.players()[0].velocity.x;
    assert!(close_to(
        walk_velocity,
        source_units_to_milli(world.players()[0].profile.walk_max_velocity),
        10
    ));
    let expected_ground_velocity = source_general_grounded_friction_velocity_after_ticks_f32(
        world.players()[0].ground_velocity_x,
        world.players()[0].profile.jumpsquat_frames,
        world.players()[0].profile,
        common,
    );

    for frame in 41..=45 {
        step_world(&mut world, Frame(frame), &neutral_jump);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::JumpF);
    assert_eq!(
        world.players()[0].velocity.x,
        source_units_to_milli(
            expected_ground_velocity
                * world.players()[0]
                    .profile
                    .ground_to_air_jump_momentum_multiplier
        ),
        "full-speed walk carry should feed ftCo_Jump_Enter after KneeBend Phys friction"
    );
}

#[test]
fn ground_jump_takeoff_carries_moonwalk_followthrough_slide_without_custom_state() {
    let mut world = World::for_two_players();
    let common = world.common_data();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
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
        PlayerInput::neutral().with_left_stick(-dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let neutral_jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    step_world(&mut world, Frame(1), &soft_left);
    step_world(&mut world, Frame(2), &mid_left);
    step_world(&mut world, Frame(3), &near_left);
    for frame in 4..=27 {
        step_world(&mut world, Frame(frame), &full_left);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    step_world(&mut world, Frame(28), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::Wait);
    assert_eq!(world.players()[0].facing, 1);
    let moonwalk_carry_velocity = world.players()[0].velocity.x;
    assert_ne!(moonwalk_carry_velocity, 0);
    let expected_ground_velocity = source_general_grounded_friction_velocity_after_ticks_f32(
        world.players()[0].ground_velocity_x,
        world.players()[0].profile.jumpsquat_frames,
        world.players()[0].profile,
        common,
    );

    for frame in 29..=33 {
        step_world(&mut world, Frame(frame), &neutral_jump);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::JumpF);
    assert_eq!(
        world.players()[0].velocity.x,
        source_units_to_milli(
            expected_ground_velocity
                * world.players()[0]
                    .profile
                    .ground_to_air_jump_momentum_multiplier
        ),
        "moonwalk carry should remain ordinary gr_vel/self_vel jump carry, not a custom Moonwalk state"
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
            source_units_to_milli(profile.gravity)
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
            source_units_to_milli(profile.gravity)
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
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];

    for frame in 0..=4 {
        step_world(&mut standing_jump, Frame(frame), &forward_jump);
    }

    step_world(&mut dash_jump, Frame(0), &dash_right);
    let dash_velocity = dash_jump.players()[0].velocity.x;
    for frame in 1..=5 {
        step_world(&mut dash_jump, Frame(frame), &forward_jump);
    }

    assert!(!standing_jump.players()[0].grounded);
    assert!(!dash_jump.players()[0].grounded);
    assert!(standing_jump.players()[0].velocity.x > 0);
    assert!(dash_velocity > standing_jump.players()[0].velocity.x);
    assert!(dash_jump.players()[0].velocity.x > standing_jump.players()[0].velocity.x);
}

#[test]
fn jump_takeoff_carries_preserved_wait_slide_from_dash_end() {
    let profile = FighterProfile {
        ground_friction: 0.01,
        ..FighterProfile::falcon_like()
    };
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let dash_right = [
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let neutral_jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    for frame in 1..=29 {
        step_world(&mut world, Frame(frame), &neutral);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::Wait);
    let wait_slide_velocity = world.players()[0].velocity.x;
    assert!(wait_slide_velocity > 0);

    for frame in 30..=34 {
        step_world(&mut world, Frame(frame), &neutral_jump);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::JumpF);
    assert!(!world.players()[0].grounded);
    assert!(world.players()[0].velocity.x > 0);
    assert!(world.players()[0].velocity.x < wait_slide_velocity);
}

#[test]
fn air_drift_preserves_jump_horizontal_velocity_without_snapping_to_neutral() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
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
    for frame in 1..=5 {
        step_world(&mut world, Frame(frame), &jump_right);
    }

    let takeoff_velocity = world.players()[0].velocity.x;
    assert!(takeoff_velocity > 0);

    step_world(&mut world, Frame(6), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::JumpF);
    assert!(world.players()[0].velocity.x > 0);
    assert!(world.players()[0].velocity.x < takeoff_velocity);
}

#[test]
fn ground_jump_horizontal_velocity_uses_profile_source_fields() {
    let profile = FighterProfile {
        jump_horizontal_initial_velocity: 0.7,
        ground_to_air_jump_momentum_multiplier: 0.8,
        jump_horizontal_max_velocity: 0.55,
        ..FighterProfile::falcon_like()
    };
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let jump_right = [
        PlayerInput::neutral()
            .with_jump(true)
            .with_left_stick(100, 0),
        PlayerInput::neutral(),
    ];

    for frame in 0..=4 {
        step_world(&mut world, Frame(frame), &jump_right);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::JumpF);
    assert_eq!(world.players()[0].velocity.x, 550);
}

#[test]
fn air_drift_uses_profile_source_accel_base_target_and_friction() {
    let profile = FighterProfile {
        jump_horizontal_initial_velocity: 0.0,
        air_drift_stick_multiplier: 0.04,
        aerial_drift_base: 0.02,
        air_drift_max: 1.12,
        aerial_friction: 0.01,
        air_max_horizontal_velocity: 1.12,
        ..FighterProfile::falcon_like()
    };
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let drift_right = [
        PlayerInput::neutral().with_left_stick(127, 0),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    for frame in 0..=4 {
        step_world(&mut world, Frame(frame), &jump);
    }
    step_world(&mut world, Frame(5), &drift_right);

    assert_eq!(world.players()[0].velocity.x, 60);

    step_world(&mut world, Frame(6), &neutral);

    assert_eq!(world.players()[0].velocity.x, 50);
}

#[test]
fn air_drift_scales_native_full_stick_as_melee_one_point_zero() {
    let profile = FighterProfile {
        jump_horizontal_initial_velocity: 0.0,
        air_drift_stick_multiplier: 0.04,
        aerial_drift_base: 0.02,
        air_drift_max: 1.12,
        aerial_friction: 0.01,
        air_max_horizontal_velocity: 1.12,
        ..FighterProfile::falcon_like()
    };
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let full_drift_right = [
        PlayerInput::neutral().with_left_stick(127, 0),
        PlayerInput::neutral(),
    ];

    for frame in 0..=4 {
        step_world(&mut world, Frame(frame), &jump);
    }
    step_world(&mut world, Frame(5), &full_drift_right);

    assert_eq!(world.players()[0].velocity.x, 60);
}

#[test]
fn aerial_jump_horizontal_velocity_uses_profile_source_field_then_source_physics() {
    let profile = FighterProfile {
        air_jump_horizontal_multiplier: 0.73,
        max_jumps: 2,
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
            .with_left_stick(127, 0),
        PlayerInput::neutral(),
    ];

    for frame in 0..=4 {
        step_world(&mut world, Frame(frame), &jump);
    }
    step_world(&mut world, Frame(5), &neutral);
    step_world(&mut world, Frame(6), &double_jump_right);

    assert_eq!(world.players()[0].motion_state, MotionState::JumpAerialF);
    assert_eq!(world.players()[0].velocity.x, 790);
    assert_eq!(world.players()[0].jumps_remaining, 0);
}

#[test]
fn aerial_jump_transition_frame_applies_first_source_air_drift() {
    let profile = FighterProfile {
        air_jump_horizontal_multiplier: 0.73,
        air_drift_stick_multiplier: 0.04,
        aerial_drift_base: 0.02,
        air_drift_max: 1.12,
        aerial_friction: 0.01,
        air_max_horizontal_velocity: 1.12,
        max_jumps: 2,
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
            .with_left_stick(127, 0),
        PlayerInput::neutral(),
    ];

    for frame in 0..=4 {
        step_world(&mut world, Frame(frame), &jump);
    }
    step_world(&mut world, Frame(5), &neutral);

    let before_x = world.players()[0].position.x;
    step_world(&mut world, Frame(6), &double_jump_right);

    assert_eq!(world.players()[0].motion_state, MotionState::JumpAerialF);
    assert_eq!(world.players()[0].velocity.x, 790);
    assert_eq!(world.players()[0].position.x - before_x, 790);
}

#[test]
fn aerial_drift_uses_cleaned_melee_main_stick_deadzone() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.motion_state = MotionState::JumpAerialF;
    player.motion_state_alias = Some(MotionState::JumpAerialF);
    player.melee_action_state_id = Some(MeleeActionStateId::new(27));
    player.source_action_key = Some(SourceActionKey::new("JumpAerialF"));
    player.motion_frame = 5;
    player.grounded = false;
    player.position.y = melee_units_f32(80.0);
    player.velocity = Vec2 { x: 885, y: 2_010 };
    player.source_self_velocity_x = milli_to_source_units(885);
    player.source_self_velocity_y = milli_to_source_units(2_010);
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(0),
        &[
            PlayerInput::neutral().with_left_stick(-27, 0),
            PlayerInput::neutral(),
        ],
    );

    assert_eq!(world.players()[0].motion_state, MotionState::JumpAerialF);
    assert_eq!(world.players()[0].velocity.x, 875);
}

#[test]
fn aerial_jump_full_left_uses_melee_negative_full_stick_scale_then_source_physics() {
    let profile = FighterProfile {
        air_jump_horizontal_multiplier: 0.73,
        max_jumps: 2,
        ..FighterProfile::falcon_like()
    };
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let double_jump_left = [
        PlayerInput::neutral()
            .with_jump(true)
            .with_left_stick(-128, 0),
        PlayerInput::neutral(),
    ];

    for frame in 0..=4 {
        step_world(&mut world, Frame(frame), &jump);
    }
    step_world(&mut world, Frame(5), &neutral);
    step_world(&mut world, Frame(6), &double_jump_left);

    assert_eq!(world.players()[0].motion_state, MotionState::JumpAerialB);
    assert_eq!(world.players()[0].velocity.x, -790);
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
fn entering_specialhi_resets_source_pose_time_like_fighter_change_motion_state() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];

    player.set_motion_state_alias(MotionState::KneeBend);
    player.motion_frame = 1;
    player.motion_anim_frame_milli = 56_777;
    player.motion_anim_rate_milli = 1_333;
    player.grounded = true;
    player.position.y = world.stage().main_floor.y;
    player.source_position = mole_core::SourceVec2::from_milli(player.position);
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(1),
        &[
            PlayerInput::neutral()
                .with_special(true)
                .with_left_stick(0, 80),
            PlayerInput::neutral(),
        ],
    );

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::SpecialHi);
    assert_eq!(player.motion_frame, 0);
    assert_eq!(
        player.motion_anim_frame_milli, 0,
        "Fighter_ChangeMotionState(..., anim_start=0, anim_speed=1) must not carry prior pose time into SpecialHi ECB sampling"
    );
    assert_eq!(player.motion_anim_rate_milli, 1_000);
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
    assert_eq!(world.players()[0].motion_frame, 1);
    assert_eq!(world.players()[0].jumps_remaining, 1);

    step_world(&mut world, Frame(2), &held_jump);
    step_world(&mut world, Frame(3), &held_jump);
    step_world(&mut world, Frame(4), &held_jump);
    let takeoff_velocity = world.players()[0].velocity.y;

    assert!(!world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::JumpF);
    assert_eq!(world.players()[0].jumps_remaining, 1);

    step_world(&mut world, Frame(5), &held_jump);

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
    step_world(&mut world, Frame(4), &jump);

    assert_eq!(world.players()[0].motion_state, MotionState::JumpF);
    assert_eq!(world.players()[0].jumps_remaining, 1);

    step_world(&mut world, Frame(5), &neutral);
    step_world(&mut world, Frame(6), &jump);

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
    assert!(
        !world.players()[0].source_shield_collision_active
            && !world.players()[0].source_shield_hit_active
            && !world.players()[0].source_shield_hit_update_pos,
        "ftCo_KneeBend_Enter calls Fighter_ChangeMotionState, which clears x221A_b7/x221B_b0 so a held trigger cannot carry the shield object into jumpsquat"
    );

    step_world(&mut world, Frame(2), &shield_jump);
    step_world(&mut world, Frame(3), &shield_jump);
    step_world(&mut world, Frame(4), &shield_jump);
    step_world(&mut world, Frame(5), &shield_jump);

    assert!(!world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::JumpF);
    assert!(
        !world.players()[0].source_shield_collision_active
            && !world.players()[0].source_shield_hit_active
            && !world.players()[0].source_shield_hit_update_pos,
        "ftCo_Jump_Enter also runs through Fighter_ChangeMotionState and must not resurrect the guard shield object while shield remains held"
    );
}

#[test]
fn held_shield_drains_health_with_source_lightshield_scale() {
    let mut world = World::for_two_players();
    let shield = [
        PlayerInput::neutral().with_left_trigger_digital(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &shield);
    assert_eq!(
        world.players()[0].shield_health.to_bits(),
        world.common_data().shield_start_health.to_bits()
    );

    step_world(&mut world, Frame(1), &shield);

    let common = world.common_data();
    let expected =
        common.shield_start_health - common.shield_hold_drain * common.shield_hold_lightshield_max;
    assert_eq!(
        world.players()[0].shield_health.to_bits(),
        expected.to_bits()
    );
    assert_eq!(
        world.players()[0].lightshield_amount.to_bits(),
        1.0_f32.to_bits()
    );
}

#[test]
fn analog_lightshield_entry_initializes_lightshield_amount_like_ftco_800921dc() {
    let common = MeleeCommonData {
        trigger_deadzone: 77,
        ..MeleeCommonData::provisional_mole()
    };
    let mut world = World::for_two_players_with_common_data(common);
    let lightshield = [
        PlayerInput::neutral().with_left_trigger_analog(128),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &lightshield);

    let expected = (128.0_f32 / 255.0_f32) / (1.0_f32 - (77.0_f32 / 255.0_f32));
    assert_eq!(world.players()[0].motion_state, MotionState::GuardOn);
    assert_eq!(
        world.players()[0].lightshield_amount.to_bits(),
        expected.to_bits(),
        "ftCo_800921DC initializes lightshield_amount as input.x650 / (1 - x10) when GuardOn installs the shield object"
    );
}

#[test]
fn inactive_shield_regenerates_toward_source_max_health() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.shield_health = world.common_data().shield_start_health - 1.0;
    world.set_player_state_for_diagnostic(0, player);

    step_world(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    let expected = world.common_data().shield_start_health - 1.0 + world.common_data().shield_regen;
    assert_eq!(
        world.players()[0].shield_health.to_bits(),
        expected.to_bits()
    );
}

#[test]
fn guard_updates_source_shield_aim_angle_and_magnitude_like_ftco_80091bc4() {
    let mut world = World::for_two_players_with_common_data(MeleeCommonData {
        shield_aim_smoothing: 0.5,
        ..MeleeCommonData::provisional_mole()
    });
    let shield_up = [
        PlayerInput::neutral()
            .with_left_trigger_digital(true)
            .with_left_stick(0, 127),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &shield_up);
    assert_eq!(
        world.players()[0].source_shield_aim_angle_degrees.to_bits(),
        55.0_f32.to_bits(),
        "ftCo_800921DC resets mv.co.guard.x8, then immediately calls ftCo_80091E78/ftCo_80091BC4"
    );
    assert_eq!(
        world.players()[0].source_shield_aim_magnitude.to_bits(),
        0.5_f32.to_bits(),
        "ftCo_800921DC resets mv.co.guard.x4, then immediately calls ftCo_80091E78/ftCo_80091BC4"
    );

    step_world(&mut world, Frame(1), &shield_up);

    assert_eq!(
        world.players()[0].source_shield_aim_angle_degrees.to_bits(),
        77.5_f32.to_bits(),
        "ftCo_80091BC4 preserves prior x8 as offset for the next smoothing step"
    );
    assert_eq!(
        world.players()[0].source_shield_aim_magnitude.to_bits(),
        0.75_f32.to_bits(),
        "ftCo_80091BC4 smooths stick magnitude with the same x44C scalar"
    );
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

    for frame in 2..=5 {
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
    for frame in 1..=8 {
        step_world(&mut full_hop, Frame(frame), &shield);
        step_world(&mut short_hop, Frame(frame), &shield);
    }

    assert_eq!(full_hop.players()[0].motion_state, MotionState::Guard);
    assert_eq!(short_hop.players()[0].motion_state, MotionState::Guard);

    step_world(&mut full_hop, Frame(9), &shield_jump);
    step_world(&mut short_hop, Frame(9), &shield_jump);

    assert_eq!(full_hop.players()[0].motion_state, MotionState::KneeBend);
    assert_eq!(short_hop.players()[0].motion_state, MotionState::KneeBend);

    for frame in 10..=13 {
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

    for frame in 2..=5 {
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

    for frame in 2..=5 {
        step_world(&mut world, Frame(frame), &shield);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::JumpF);
    assert!(world.players()[0].velocity.y > 0);
}

#[test]
fn takeoff_frame_cstick_aerial_does_not_spend_air_jump_without_normal_jump_input() {
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

    for frame in 0..=3 {
        step_world(&mut world, Frame(frame), &jump);
    }
    step_world(&mut world, Frame(4), &jump_takeoff_with_cstick_up);

    assert_eq!(world.players()[0].motion_state, MotionState::AttackAirHi);
    assert_eq!(world.players()[0].jumps_remaining, 1);

    step_world(&mut world, Frame(5), &held_cstick_up);

    assert_eq!(world.players()[0].motion_state, MotionState::AttackAirHi);
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
fn shield_cstick_down_enters_spotdodge_and_clears_guard_shield_object() {
    let mut world = World::for_two_players();
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];
    let shield_cstick_down = [
        PlayerInput::neutral()
            .with_left_trigger_analog(80)
            .with_c_stick(0, spot_dodge_stick_y()),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &shield);

    assert_eq!(world.players()[0].motion_state, MotionState::GuardOn);
    assert!(world.players()[0].source_shield_collision_active);
    assert!(world.players()[0].source_shield_hit_active);

    step_world(&mut world, Frame(1), &shield_cstick_down);

    let player = world.players()[0];
    assert_eq!(
        player.motion_state,
        MotionState::EscapeN,
        "ftCo_GuardOn_IASA routes ftCo_8009980C before platform pass; ftCo_8009980C accepts C-stick down through ftCo_800DF8E8"
    );
    assert!(
        !player.source_shield_collision_active
            && !player.source_shield_hit_active
            && !player.source_shield_hit_update_pos,
        "ftCo_800998EC enters EscapeN with Fighter_ChangeMotionState, clearing the installed guard shield object"
    );
}

#[test]
fn spotdodge_phys_uses_source_ground_traction_instead_of_zeroing_velocity() {
    let common = MeleeCommonData::provisional_mole();
    let profile = FighterProfile::falcon_like();
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let mut player = PlayerState::new_with_profile(0, 0, 1, profile);
    player.motion_state = MotionState::EscapeN;
    player.grounded = true;
    player.ground_velocity_x = milli_to_source_units(408);
    player.velocity.x = 408;
    assert!(world.set_player_state_for_diagnostic(0, player));
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    step_world(&mut world, Frame(0), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::EscapeN);
    assert_eq!(
        world.players()[0].velocity.x,
        source_general_grounded_friction_velocity(408, profile, common),
        "ftCo_EscapeN_Phys calls ft_80084F3C; spot dodge should slide under source ground traction rather than hard-zeroing gr_vel"
    );
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
            .with_left_stick(dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];
    let shield_left = [
        PlayerInput::neutral()
            .with_left_trigger_analog(80)
            .with_left_stick(-dash_stick_x(), 0),
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
fn shield_cstick_side_enters_facing_aware_roll_and_clears_guard_shield_object() {
    let mut forward = World::for_two_players();
    let mut back = World::for_two_players();
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];
    let cstick_right = [
        PlayerInput::neutral()
            .with_left_trigger_analog(80)
            .with_c_stick(dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];
    let cstick_left = [
        PlayerInput::neutral()
            .with_left_trigger_analog(80)
            .with_c_stick(-dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut forward, Frame(0), &shield);
    step_world(&mut back, Frame(0), &shield);
    step_world(&mut forward, Frame(1), &cstick_right);
    step_world(&mut back, Frame(1), &cstick_left);

    assert_eq!(
        forward.players()[0].motion_state,
        MotionState::EscapeF,
        "ftCo_8009917C accepts C-stick X through ftCo_800DF8B0 and chooses EscapeF when stick_x * facing_dir >= 0"
    );
    assert_eq!(
        back.players()[0].motion_state,
        MotionState::EscapeB,
        "ftCo_8009917C accepts C-stick X through ftCo_800DF8B0 and chooses EscapeB when stick_x * facing_dir < 0"
    );
    for player in [forward.players()[0], back.players()[0]] {
        assert!(
            !player.source_shield_collision_active
                && !player.source_shield_hit_active
                && !player.source_shield_hit_update_pos,
            "EscapeF/EscapeB entry goes through Fighter_ChangeMotionState and must clear the installed guard shield object"
        );
    }
}

#[test]
fn parity_mode_does_not_turn_around_mid_shield() {
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

    for frame in 1..=8 {
        step_world(&mut world, Frame(frame), &shield_left);
        assert!(matches!(
            world.players()[0].motion_state,
            MotionState::GuardOn | MotionState::Guard
        ));
        assert_eq!(world.players()[0].facing, 1);
    }

    assert!(!world.engine_features().shield_turnaround_during_guard);
    assert_eq!(world.players()[0].motion_state, MotionState::Guard);
}

#[test]
fn shield_opposite_soft_hold_turns_facing_without_leaving_guard() {
    let mut world = World::for_two_players();
    enable_custom_shield_turn(&mut world);
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

    assert!(matches!(
        world.players()[0].motion_state,
        MotionState::GuardOn | MotionState::Guard
    ));
    assert_eq!(world.players()[0].facing, -1);

    for frame in 6..=8 {
        step_world(&mut world, Frame(frame), &shield_left);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::Guard);
}

#[test]
fn shield_turn_updates_facing_for_next_roll_direction() {
    let mut world = World::for_two_players();
    enable_custom_shield_turn(&mut world);
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
            .with_left_stick(dash_stick_x(), 0),
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
    enable_custom_shield_turn(&mut world);
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
    step_world(&mut world, Frame(7), &shield_jump);

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
fn shield_release_during_guard_on_waits_for_startup_then_enters_guard_off() {
    let mut world = World::for_two_players();
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    step_world(&mut world, Frame(0), &shield);
    step_world(&mut world, Frame(1), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::GuardOn);

    let entry_frame = advance_guard_on_release_to_guard_off(&mut world, 2);

    assert_eq!(world.players()[0].motion_state, MotionState::GuardOff);
    assert_eq!(entry_frame, 8);
}

#[test]
fn guard_on_release_exit_takes_priority_over_same_tick_z_catch() {
    let mut world = World::for_two_players();
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let released_z = [
        PlayerInput::neutral().with_grab(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &shield);
    for frame in 1..=7 {
        step_world(&mut world, Frame(frame), &neutral);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::GuardOn);
    assert_eq!(world.players()[0].motion_frame, 7);

    step_world(&mut world, Frame(8), &released_z);

    assert_eq!(world.players()[0].motion_state, MotionState::GuardOff);
}

#[test]
fn guard_off_returns_to_wait_after_shield_drop_lag() {
    let mut world = World::for_two_players();
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &shield);
    let guard_off_entry_frame = advance_guard_on_release_to_guard_off(&mut world, 1);

    assert_current_action_returns_to_wait_after_frames(
        &mut world,
        guard_off_entry_frame,
        MotionState::GuardOff,
        16,
    );
}

#[test]
fn guard_off_can_jump_before_wait() {
    let mut world = World::for_two_players();
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &shield);
    let guard_off_entry_frame = advance_guard_on_release_to_guard_off(&mut world, 1);

    assert_eq!(world.players()[0].motion_state, MotionState::GuardOff);

    step_world(&mut world, Frame(guard_off_entry_frame + 1), &jump);

    assert_eq!(world.players()[0].motion_state, MotionState::KneeBend);
}

#[test]
fn guard_off_without_reflect_gate_checks_spotdodge_before_offense() {
    let mut world = World::for_two_players();
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];
    let down_attack = [
        PlayerInput::neutral()
            .with_left_stick(0, -90)
            .with_attack(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &shield);
    let guard_off_entry_frame = advance_guard_on_release_to_guard_off(&mut world, 1);

    assert_eq!(world.players()[0].motion_state, MotionState::GuardOff);

    step_world(&mut world, Frame(guard_off_entry_frame + 1), &down_attack);

    assert_eq!(world.players()[0].motion_state, MotionState::EscapeN);
}

#[test]
fn guard_off_does_not_roll_or_dash_from_horizontal_tap() {
    let mut world = World::for_two_players();
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];
    let horizontal_tap = [
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &shield);
    let guard_off_entry_frame = advance_guard_on_release_to_guard_off(&mut world, 1);

    assert_eq!(world.players()[0].motion_state, MotionState::GuardOff);

    step_world(
        &mut world,
        Frame(guard_off_entry_frame + 1),
        &horizontal_tap,
    );

    assert_eq!(world.players()[0].motion_state, MotionState::GuardOff);
}

#[test]
fn digital_trigger_on_anim_takeoff_frame_can_airdodge_after_jump_state_entry() {
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
    step_world(&mut world, Frame(3), &jump);
    step_world(&mut world, Frame(4), &jump_with_digital_trigger);

    assert!(!world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::EscapeAir);
    assert!(world.players()[0].velocity.x > 0);
    assert!(world.players()[0].velocity.y > 0);
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
            .with_left_stick(80, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &jump);
    step_world(&mut world, Frame(1), &jump);
    step_world(&mut world, Frame(2), &jump);
    step_world(&mut world, Frame(3), &jump);
    step_world(&mut world, Frame(4), &jump);
    step_world(&mut world, Frame(5), &air_dodge);

    assert!(!world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::EscapeAir);
    assert!(world.players()[0].velocity.x > 0);
    assert_eq!(world.players()[0].velocity.y, 0);
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
    step_world(&mut world, Frame(4), &jump);
    step_world(&mut world, Frame(5), &air_dodge);

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

    for frame in 0..=4 {
        step_world(&mut right, Frame(frame), &jump);
        step_world(&mut diagonal, Frame(frame), &jump);
    }
    step_world(&mut right, Frame(5), &right_air_dodge);
    step_world(&mut diagonal, Frame(5), &diagonal_air_dodge);

    let common = MeleeCommonData::provisional_mole();
    let force = source_units_to_milli(common.escapeair_force * common.escapeair_decay);
    assert_eq!(right.players()[0].motion_state, MotionState::EscapeAir);
    assert_eq!(right.players()[0].velocity.x, force);
    assert_eq!(right.players()[0].velocity.y, 0);
    assert!(diagonal.players()[0].velocity.x > 0);
    assert!(diagonal.players()[0].velocity.y > 0);
    assert!(close_to(
        squared_magnitude(diagonal.players()[0].velocity),
        squared_magnitude(right.players()[0].velocity),
        force * 3
    ));
}

#[test]
fn escape_air_diagonal_uses_hsd_normalized_stick_angle_before_force() {
    let mut world = World::for_two_players();
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let diagonal_air_dodge = [
        PlayerInput::neutral()
            .with_left_trigger_digital(true)
            .with_left_trigger_analog(255)
            .with_left_stick(-83, -95),
        PlayerInput::neutral(),
    ];

    for frame in 0..=4 {
        step_world(&mut world, Frame(frame), &jump);
    }
    step_world(&mut world, Frame(5), &diagonal_air_dodge);

    assert_eq!(world.players()[0].motion_state, MotionState::EscapeAir);
    assert_eq!(
        world.players()[0].velocity.x,
        -1827,
        "ftCo_80099A9C uses the HSD nml_stick float angle before applying escapeair_force"
    );
    assert_eq!(
        world.players()[0].velocity.y,
        -2108,
        "ftCo_EscapeAir_Phys applies escapeair_decay on the entry frame"
    );
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

    for frame in 0..=4 {
        step_world(&mut world, Frame(frame), &jump);
    }
    step_world(&mut world, Frame(5), &right_air_dodge);

    let common = MeleeCommonData::provisional_mole();
    assert_eq!(world.players()[0].motion_state, MotionState::EscapeAir);
    assert_eq!(
        world.players()[0].velocity.x,
        source_units_to_milli(common.escapeair_force * common.escapeair_decay)
    );
}

#[test]
fn escape_air_entry_frame_translates_by_decayed_self_velocity() {
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

    for frame in 0..=4 {
        step_world(&mut world, Frame(frame), &jump);
    }
    let before_x = world.players()[0].position.x;
    step_world(&mut world, Frame(5), &right_air_dodge);

    let common = MeleeCommonData::provisional_mole();
    let decayed_force = source_units_to_milli(common.escapeair_force * common.escapeair_decay);
    assert_eq!(world.players()[0].motion_state, MotionState::EscapeAir);
    assert_eq!(world.players()[0].velocity.x, decayed_force);
    assert_eq!(world.players()[0].position.x - before_x, decayed_force);
}

#[test]
fn world_common_data_drives_escape_air_force_timer_and_decay() {
    let common = MeleeCommonData {
        escapeair_iasa_timer_ticks: 7,
        escapeair_deadzone_x: 10,
        escapeair_deadzone_y: 10,
        escapeair_force: 1.2,
        escapeair_decay: 0.5,
        ..MeleeCommonData::provisional_mole()
    };
    let mut world = World::for_two_players_with_common_data(common);
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let right_air_dodge = [
        PlayerInput::neutral()
            .with_right_trigger_digital(true)
            .with_left_stick(127, 0),
        PlayerInput::neutral(),
    ];

    for frame in 0..=4 {
        step_world(&mut world, Frame(frame), &jump);
    }
    step_world(&mut world, Frame(5), &right_air_dodge);

    assert_eq!(world.common_data(), common);
    assert_eq!(world.players()[0].motion_state, MotionState::EscapeAir);
    assert_eq!(world.players()[0].escape_air_iasa_timer, 7);
    assert_eq!(world.players()[0].velocity.x, 600);

    step_world(&mut world, Frame(6), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::EscapeAir);
    assert_eq!(world.players()[0].velocity.x, 300);
}

#[test]
fn world_common_data_drives_tap_timer_thresholds_for_dash() {
    let common = MeleeCommonData {
        tap_x_threshold: 100,
        dash_x: 80,
        walk_fast_x: 90,
        ..MeleeCommonData::provisional_mole()
    };
    let mut world = World::for_two_players_with_common_data(common);

    step_world(
        &mut world,
        Frame(0),
        &[
            PlayerInput::neutral().with_left_stick(90, 0),
            PlayerInput::neutral(),
        ],
    );

    assert_eq!(world.players()[0].motion_state, MotionState::WalkSlow);
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
    step_world(&mut world, Frame(4), &jump);
    step_world(&mut world, Frame(5), &neutral_air_dodge);

    assert_eq!(world.players()[0].motion_state, MotionState::EscapeAir);
    assert_eq!(world.players()[0].velocity.y, 0);

    let height = world.players()[0].position.y;
    step_world(&mut world, Frame(6), &neutral);

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
    step_world(&mut world, Frame(4), &jump);
    step_world(&mut world, Frame(5), &right_air_dodge);

    let first_escape_velocity = world.players()[0].velocity.x;
    assert!(first_escape_velocity > 0);

    step_world(&mut world, Frame(6), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::EscapeAir);
    assert!(world.players()[0].velocity.x > 0);
    assert!(world.players()[0].velocity.x < first_escape_velocity);
    assert_eq!(
        world.players()[0].velocity.x,
        source_units_to_milli(
            milli_to_source_units(first_escape_velocity)
                * MeleeCommonData::provisional_mole().escapeair_decay
        )
    );
}

#[test]
fn escape_air_script_skip_decay_frame_resumes_falling_physics() {
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

    for frame in 0..=4 {
        step_world(&mut world, Frame(frame), &jump);
    }
    let entry_frame = 5;
    step_world(&mut world, Frame(entry_frame), &neutral_air_dodge);

    let skip_frame = entry_frame + falcon_escape_air_skip_decay_frame() - 1;
    for frame in (entry_frame + 1)..skip_frame {
        step_world(&mut world, Frame(frame), &neutral);
        assert_eq!(world.players()[0].motion_state, MotionState::EscapeAir);
        assert_eq!(world.players()[0].motion_cmd_var0, 0);
        assert_eq!(world.players()[0].velocity.y, 0);
    }

    step_world(&mut world, Frame(skip_frame), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::EscapeAir);
    assert_eq!(world.players()[0].motion_cmd_var0, 1);
    assert_eq!(
        world.players()[0].velocity.y,
        -source_units_to_milli(world.players()[0].profile.gravity)
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

    let escape_air_end = 8 + falcon_escape_air_action_frames();
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
    let mut player = world.players()[0];
    player.position.y += melee_units_f32(200.0);
    assert!(world.set_player_state_for_diagnostic(0, player));

    let escape_air_end = 8 + falcon_escape_air_action_frames();
    for frame in 9..=escape_air_end {
        step_world(&mut world, Frame(frame), &neutral);
    }

    assert!(!world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::FallSpecial);
}

#[test]
fn escape_air_end_frame_translates_before_visible_fall_special_state() {
    let mut world = World::for_two_players();
    let start_y = melee_units_f32(100.0);
    let mut player = world.players()[0];
    let fall_velocity = -player.profile.terminal_velocity;
    player.set_motion_state_alias(MotionState::EscapeAir);
    player.motion_frame = (falcon_escape_air_action_frames() as u8).saturating_sub(2);
    player.motion_cmd_var0 = 1;
    player.position.y = start_y;
    player.source_position = mole_core::SourceVec2::from_milli(player.position);
    player.velocity.y = source_units_to_milli(fall_velocity);
    player.source_self_velocity_y = fall_velocity;
    player.grounded = false;
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::FallSpecial);
    assert_eq!(
        player.position.y,
        start_y + source_units_to_milli(fall_velocity)
    );
    assert_eq!(player.velocity.y, source_units_to_milli(fall_velocity));
}

#[test]
fn escape_air_to_fall_special_preserves_fastfall_like_ft_mf_keep_fastfall() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.set_motion_state_alias(MotionState::EscapeAir);
    player.motion_frame = (falcon_escape_air_action_frames() as u8).saturating_sub(2);
    player.motion_cmd_var0 = 1;
    player.fast_falling = true;
    player.position.y = melee_units_f32(100.0);
    player.source_position = mole_core::SourceVec2::from_milli(player.position);
    player.source_self_velocity_y = -player.profile.fast_fall_velocity;
    player.velocity.y = source_units_to_milli(player.source_self_velocity_y);
    player.grounded = false;
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::FallSpecial);
    assert!(player.fast_falling);
}

#[test]
fn escape_air_uses_action_table_duration_not_common_data_placeholder() {
    let common = MeleeCommonData {
        escapeair_animation_ticks: 3,
        ..MeleeCommonData::provisional_mole()
    };
    let mut world = World::for_two_players_with_common_data(common);
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

    for frame in 9..=(8 + common.escapeair_animation_ticks as u32) {
        step_world(&mut world, Frame(frame), &neutral);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::EscapeAir);

    // PlCaAJ action 44 (PlyCaptain5K_Share_ACTION_EscapeAir_figatree) is 50 frames.
    for frame in
        (9 + common.escapeair_animation_ticks as u32)..=(8 + falcon_escape_air_action_frames())
    {
        step_world(&mut world, Frame(frame), &neutral);
    }

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
    let escape_air_end = 8 + falcon_escape_air_action_frames();
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
fn ordinary_airborne_contact_above_source_landing_threshold_enters_wait() {
    let mut world = World::for_two_players();
    let floor_y = world.stage().main_floor.y;
    let mut player = world.players()[0];
    player.motion_state = MotionState::JumpAerialF;
    player.motion_frame = 7;
    player.position = Vec2 {
        x: 0,
        y: floor_y + 500,
    };
    player.velocity = Vec2 { x: 0, y: -850 };
    player.source_self_velocity_y = -0.85;
    player.grounded = false;
    player.ecb_bottom_offset_y = 0;
    player.ecb_bottom_lock_timer = 2;
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(1840),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    assert!(world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::Wait);
    assert_eq!(world.players()[0].motion_frame, 0);
}

#[test]
fn ordinary_airborne_contact_at_or_below_source_landing_threshold_enters_landing() {
    let mut world = World::for_two_players();
    let floor_y = world.stage().main_floor.y;
    let mut player = world.players()[0];
    player.motion_state = MotionState::JumpAerialF;
    player.motion_frame = 7;
    player.position = Vec2 {
        x: 0,
        y: floor_y + 500,
    };
    player.velocity = Vec2 { x: 0, y: -1_200 };
    player.source_self_velocity_y = -1.2;
    player.grounded = false;
    player.ecb_bottom_offset_y = 0;
    player.ecb_bottom_lock_timer = 2;
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(1840),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    assert!(world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::Landing);
    assert_eq!(world.players()[0].motion_frame, 0);
}

#[test]
fn aerial_attack_contact_without_cmd_var0_enters_ordinary_landing() {
    for (attack_state, inactive_frame) in [
        (MotionState::AttackAirN, 34),
        (MotionState::AttackAirF, 35),
        (MotionState::AttackAirB, 21),
        (MotionState::AttackAirHi, 22),
        (MotionState::AttackAirLw, 36),
    ] {
        let mut world = World::for_two_players();
        let mut player = world.players()[0];
        player.motion_state = attack_state;
        player.motion_frame = inactive_frame;
        player.motion_cmd_var0 = 0;
        player.position = Vec2 { x: 0, y: 1_000 };
        player.velocity = Vec2 { x: 0, y: -6_000 };
        player.grounded = false;
        assert!(world.set_player_state_for_diagnostic(0, player));

        for frame in 0..20 {
            step_world(
                &mut world,
                Frame(frame),
                &[PlayerInput::neutral(), PlayerInput::neutral()],
            );
            if world.players()[0].grounded {
                break;
            }
        }

        assert_eq!(world.players()[0].motion_state, MotionState::Landing);
        assert_eq!(world.players()[0].motion_frame, 0);
        assert!(world.players()[0].grounded);
    }
}

#[test]
fn aerial_attack_contact_with_cmd_var0_enters_directional_landing_air_state() {
    for (attack_state, landing_state) in [
        (MotionState::AttackAirN, MotionState::LandingAirN),
        (MotionState::AttackAirF, MotionState::LandingAirF),
        (MotionState::AttackAirB, MotionState::LandingAirB),
        (MotionState::AttackAirHi, MotionState::LandingAirHi),
        (MotionState::AttackAirLw, MotionState::LandingAirLw),
    ] {
        let mut world = World::for_two_players();
        let mut player = world.players()[0];
        player.motion_state = attack_state;
        player.motion_frame = 8;
        player.motion_cmd_var0 = 1;
        player.position = Vec2 { x: 0, y: 1_000 };
        player.velocity = Vec2 { x: 0, y: -6_000 };
        player.grounded = false;
        assert!(world.set_player_state_for_diagnostic(0, player));

        for frame in 0..20 {
            step_world(
                &mut world,
                Frame(frame),
                &[PlayerInput::neutral(), PlayerInput::neutral()],
            );
            if world.players()[0].grounded {
                break;
            }
        }

        assert_eq!(world.players()[0].motion_state, landing_state);
        assert_eq!(world.players()[0].motion_frame, 0);
        assert!(world.players()[0].grounded);
    }
}

#[test]
fn aerial_attack_tick_advances_source_animation_frame_for_collision_pose() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.motion_state = MotionState::AttackAirN;
    player.motion_state_alias = Some(MotionState::AttackAirN);
    player.melee_action_state_id = Some(MeleeActionStateId::new(65));
    player.source_action_key = Some(SourceActionKey::new("AttackAirN"));
    player.motion_frame = 2;
    player.motion_anim_frame_milli = 2_000;
    player.motion_anim_rate_milli = 1_000;
    player.position = Vec2 { x: 0, y: 100_000 };
    player.velocity = Vec2 { x: 0, y: 0 };
    player.grounded = false;
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::AttackAirN);
    assert_eq!(player.motion_frame, 3);
    assert_eq!(player.motion_anim_frame_milli, 3_000);
    assert_eq!(world.snapshot().players[0].source_pose_frame, 3);
}

#[test]
fn aerial_attack_anim_end_installs_fall_before_same_tick_airborne_iasa() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.set_motion_state_alias(MotionState::AttackAirLw);
    player.motion_frame = 43;
    player.set_source_motion_anim_frame(44.0);
    player.motion_anim_rate_milli = 1_000;
    player.position = Vec2 { x: 0, y: 100_000 };
    player.source_position.x = 0.0;
    player.source_position.y = 100.0;
    player.velocity = Vec2 { x: 0, y: 0 };
    player.source_self_velocity_x = 0.0;
    player.source_self_velocity_y = 0.0;
    player.grounded = false;
    player.jumps_remaining = 1;
    assert_eq!(player.source_action_total_frames, 45);
    assert!(world.set_player_state_for_diagnostic(0, player));

    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    step_world(&mut world, Frame(0), &jump);

    let player = world.players()[0];
    assert_eq!(
        player.motion_state,
        MotionState::JumpAerialF,
        "ftCo_AttackAir_Anim enters Fall when ftAnim_IsFramesRemaining is false; the later input callback must then be Fall_IASA, not stale AttackAir_IASA"
    );
    assert_eq!(
        player.melee_action_state_id,
        Some(MeleeActionStateId::new(27))
    );
    assert_eq!(
        player.source_action_key,
        Some(SourceActionKey::new("JumpAerialF"))
    );
}

#[test]
fn aerial_attack_allow_interrupt_command_allows_same_tick_air_jump_before_anim_end() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.set_motion_state_alias(MotionState::AttackAirLw);
    player.motion_frame = 36;
    player.set_source_motion_anim_frame(37.0);
    player.motion_anim_rate_milli = 1_000;
    player.position = Vec2 {
        x: 109_523,
        y: -69_200,
    };
    player.source_position.x = 109.522_926;
    player.source_position.y = -69.199_905;
    player.velocity = Vec2 { x: 852, y: -3_500 };
    player.source_self_velocity_x = 0.8515;
    player.source_self_velocity_y = -3.5;
    player.facing = 1;
    player.grounded = false;
    player.jumps_remaining = 1;
    assert_eq!(player.source_action_total_frames, 45);
    assert!(world.set_player_state_for_diagnostic(0, player));

    let inputs = [
        PlayerInput::neutral()
            .with_left_stick(-125, 0)
            .with_jump(true),
        PlayerInput::neutral(),
    ];
    let source_pose_metadata = |query: &PlayerState| {
        if query.source_action_key == Some(SourceActionKey::new("AttackAirLw"))
            && query.source_motion_anim_frame.to_bits() == 38.0_f32.to_bits()
        {
            Some(SourceActionPoseMetadata {
                script_events: SourceActionScriptEvents::single(
                    SourceActionScriptEvent::AllowInterrupt,
                ),
                ..SourceActionPoseMetadata::default()
            })
        } else {
            Some(SourceActionPoseMetadata::default())
        }
    };

    step_world_with_source_runtime_data(
        &mut world,
        Frame(2939),
        &inputs,
        source_pose_metadata,
        |action_state_id| match action_state_id.get() {
            69 => Some(45),
            _ => None,
        },
    );

    let player = world.players()[0];
    assert_eq!(
        player.motion_state,
        MotionState::JumpAerialB,
        "ftAction_80071950 sets fp->allow_interrupt on AttackAirLw command frame 38; AttackAirLw_IASA must then consume the same-row aerial jump before the animation ends"
    );
    assert_eq!(
        player.melee_action_state_id,
        Some(MeleeActionStateId::new(28))
    );
    assert_eq!(
        player.source_action_key,
        Some(SourceActionKey::new("JumpAerialB"))
    );
    assert_eq!(player.jumps_remaining, 0);
    assert_eq!(player.motion_frame, 0);
}

#[test]
fn aerial_attack_script_cmd_var0_uses_profile_source_event_frames() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.motion_state = MotionState::AttackAirN;
    player.motion_frame = 2;
    player.motion_cmd_var0 = 0;
    player.position = Vec2 { x: 0, y: 100_000 };
    player.velocity = Vec2 { x: 0, y: 0 };
    player.grounded = false;
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    assert_eq!(world.players()[0].motion_state, MotionState::AttackAirN);
    assert_eq!(world.players()[0].motion_cmd_var0, 1);

    let mut player = world.players()[0];
    player.motion_frame = 32;
    player.motion_cmd_var0 = 1;
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(1),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    assert_eq!(world.players()[0].motion_state, MotionState::AttackAirN);
    assert_eq!(world.players()[0].motion_cmd_var0, 0);
}

#[test]
fn aerial_landing_air_uses_empty_iasa_and_exits_by_scaled_animation_completion() {
    let profile = FighterProfile {
        normal_landing_lag_ticks: 1,
        landing_air_f_lag_ticks: 4,
        ..FighterProfile::falcon_like()
    };
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let mut player = world.players()[0];
    player.motion_state = MotionState::AttackAirF;
    player.motion_frame = 3;
    player.motion_cmd_var0 = 1;
    player.position = Vec2 { x: 0, y: 1_000 };
    player.velocity = Vec2 { x: 0, y: -6_000 };
    player.grounded = false;
    assert!(world.set_player_state_for_diagnostic(0, player));

    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let walk = [
        PlayerInput::neutral().with_left_stick(64, 0),
        PlayerInput::neutral(),
    ];
    let mut frame = 0;
    while !world.players()[0].grounded && frame < 20 {
        step_world(&mut world, Frame(frame), &neutral);
        frame += 1;
    }

    assert_eq!(world.players()[0].motion_state, MotionState::LandingAirF);
    assert_eq!(world.players()[0].motion_frame, 0);

    step_world(&mut world, Frame(frame), &walk);
    frame += 1;

    assert_eq!(world.players()[0].motion_state, MotionState::LandingAirF);
    assert_eq!(world.players()[0].motion_frame, 1);

    for _ in 0..3 {
        step_world(&mut world, Frame(frame), &walk);
        frame += 1;
    }

    assert_eq!(
        world.players()[0].motion_state,
        MotionState::WalkSlow,
        "LandingAir_IASA remains empty before completion, then ftCo_Landing_Anim falls through ft_8008A2BC so Wait_IASA can consume held walk on the completion frame"
    );
    assert_eq!(world.players()[0].motion_frame, 0);
}

#[test]
fn aerial_lcancel_scales_landing_air_lag_from_source_common_window() {
    let common = MeleeCommonData {
        lcancel_window: 7,
        lcancel_divisor: 2.0,
        ..MeleeCommonData::provisional_mole()
    };
    let profile = FighterProfile {
        landing_air_n_lag_ticks: 15,
        ..FighterProfile::falcon_like()
    };
    let mut world = World::for_two_players_on_stage_with_profiles_and_common_data(
        StageProfile::battlefield_test(),
        [profile; 2],
        common,
    );
    let mut player = world.players()[0];
    player.motion_state = MotionState::AttackAirN;
    player.motion_frame = 8;
    player.motion_cmd_var0 = 1;
    player.position = Vec2 { x: 0, y: 1_000 };
    player.velocity = Vec2 { x: 0, y: -6_000 };
    player.grounded = false;
    assert!(world.set_player_state_for_diagnostic(0, player));

    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let lcancel = [
        PlayerInput::neutral().with_left_trigger_digital(true),
        PlayerInput::neutral(),
    ];
    let mut frame = 0;
    while !world.players()[0].grounded && frame < 20 {
        step_world(&mut world, Frame(frame), &lcancel);
        frame += 1;
    }

    assert_eq!(world.players()[0].motion_state, MotionState::LandingAirN);
    assert_eq!(world.players()[0].motion_frame, 0);

    for _ in 0..6 {
        step_world(&mut world, Frame(frame), &neutral);
        frame += 1;
        assert_eq!(world.players()[0].motion_state, MotionState::LandingAirN);
    }

    step_world(&mut world, Frame(frame), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::Wait);
    assert_eq!(world.players()[0].motion_frame, 0);
}

#[test]
fn aerial_lcancel_uses_stored_source_lr_timer_when_trigger_released_on_contact() {
    let common = MeleeCommonData {
        lcancel_window: 7,
        lcancel_divisor: 2.0,
        ..MeleeCommonData::provisional_mole()
    };
    let profile = FighterProfile {
        landing_air_n_lag_ticks: 15,
        ..FighterProfile::falcon_like()
    };
    let mut world = World::for_two_players_on_stage_with_profiles_and_common_data(
        StageProfile::battlefield_test(),
        [profile; 2],
        common,
    );
    let mut player = world.players()[0];
    player.motion_state = MotionState::AttackAirN;
    player.motion_frame = 20;
    player.motion_cmd_var0 = 1;
    player.source_lr_digital_press_timer = 0;
    player.position = Vec2 { x: 0, y: 1_000 };
    player.velocity = Vec2 { x: 0, y: -6_000 };
    player.grounded = false;
    assert!(world.set_player_state_for_diagnostic(0, player));

    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let mut frame = 0;
    while !world.players()[0].grounded && frame < 20 {
        step_world(&mut world, Frame(frame), &neutral);
        frame += 1;
    }

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::LandingAirN);
    assert_eq!(
        player.landing_lag_ticks, 7,
        "ftCo_LandingAir_EnterWithLag scales lag from fp->x67F, not the current-frame trigger timer; releasing on the contact row must not erase an in-window L-cancel press"
    );
}

#[test]
fn airborne_special_contact_does_not_use_attack_air_lcancel_path() {
    let common = MeleeCommonData {
        lcancel_window: 7,
        lcancel_divisor: 2.0,
        ..MeleeCommonData::provisional_mole()
    };
    let mut world = World::for_two_players_with_common_data(common);
    let mut player = world.players()[0];
    player.motion_state = MotionState::SpecialAirN;
    player.motion_frame = 8;
    player.motion_cmd_var0 = 1;
    player.position = Vec2 { x: 0, y: 1_000 };
    player.velocity = Vec2 { x: 0, y: -6_000 };
    player.grounded = false;
    assert!(world.set_player_state_for_diagnostic(0, player));

    let lcancel_input = [
        PlayerInput::neutral().with_left_trigger_digital(true),
        PlayerInput::neutral(),
    ];
    for frame in 0..20 {
        step_world(&mut world, Frame(frame), &lcancel_input);
        if world.players()[0].grounded {
            break;
        }
    }

    assert_eq!(world.players()[0].motion_state, MotionState::Landing);
    assert_eq!(world.players()[0].landing_lag_ticks, 0);
}

#[test]
fn ordinary_airborne_contact_can_land_on_soft_platform() {
    let mut world = World::for_two_players();
    let stage = world.stage();
    let platform = stage.soft_platforms[0];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];

    for frame in 0..=4 {
        step_world(&mut world, Frame(frame), &jump);
    }

    let mut frame = 5;
    while !world.players()[0].grounded && frame < 120 {
        step_world(&mut world, Frame(frame), &neutral);
        frame += 1;
        if world.players()[0].grounded {
            break;
        }
    }

    assert!(world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::Landing);
    assert_eq!(world.players()[0].position.y, platform.y);
}

#[test]
fn ordinary_fall_holding_x25c_or_lower_skips_soft_platform() {
    let mut world = World::for_two_players();
    let stage = world.stage();
    let platform = stage.soft_platforms[2];
    let common = world.common_data();
    let mut player = world.players()[0];
    player.motion_state = MotionState::Fall;
    player.motion_frame = 4;
    player.position = Vec2 {
        x: (platform.left_x + platform.right_x) / 2,
        y: platform.y + melee_units(1.0),
    };
    player.velocity = Vec2 {
        x: melee_units_f32(1.03),
        y: -source_units_to_milli(player.profile.fast_fall_velocity),
    };
    player.fast_falling = true;
    player.grounded = false;
    player.ecb_bottom_offset_y = 0;
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(156),
        &[
            PlayerInput::neutral().with_left_stick(0, common.fallspecial_platform_landing_y),
            PlayerInput::neutral(),
        ],
    );

    assert!(!world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::Fall);
    assert!(world.players()[0].position.y < platform.y);
}

#[test]
fn fall_entry_from_ground_clamps_horizontal_velocity_to_air_drift_max() {
    let mut world = World::for_two_players();
    let main_floor = world.stage().main_floor;
    let mut player = world.players()[0];
    player.motion_state = MotionState::Wait;
    player.motion_frame = 0;
    player.position = Vec2 {
        x: main_floor.right_x - melee_units_f32(0.25),
        y: main_floor.y,
    };
    player.velocity = Vec2 {
        x: source_units_to_milli(player.profile.dash_run_terminal_velocity),
        y: 0,
    };
    player.facing = -1;
    player.ground_velocity_x = player.profile.dash_run_terminal_velocity;
    player.grounded = true;
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(168),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    assert_eq!(world.players()[0].motion_state, MotionState::Fall);
    assert_eq!(
        world.players()[0].velocity.x,
        source_units_to_milli(world.players()[0].profile.air_drift_max)
    );
}

#[test]
fn soft_platform_landing_keeps_platform_height_while_grounded() {
    let mut world = World::for_two_players();
    let stage = world.stage();
    let platform = stage.soft_platforms[0];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];

    for frame in 0..=4 {
        step_world(&mut world, Frame(frame), &jump);
    }

    let mut frame = 5;
    while !world.players()[0].grounded && frame < 120 {
        step_world(&mut world, Frame(frame), &neutral);
        frame += 1;
    }

    assert_eq!(world.players()[0].position.y, platform.y);

    for frame in frame..frame + 3 {
        step_world(&mut world, Frame(frame), &neutral);
        assert!(world.players()[0].grounded);
        assert_eq!(world.players()[0].position.y, platform.y);
    }
}

#[test]
fn shield_down_on_soft_platform_enters_pass_not_custom_drop_state() {
    let mut world = World::for_two_players();
    let (stage, mut frame) = land_player_one_on_left_platform(&mut world);
    let platform = stage.soft_platforms[0];
    let shield = [
        PlayerInput::neutral().with_left_trigger_digital(true),
        PlayerInput::neutral(),
    ];
    let shield_down = [
        PlayerInput::neutral()
            .with_left_trigger_digital(true)
            .with_left_stick(0, -MeleeCommonData::provisional_mole().platform_pass_y),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(frame), &shield);
    frame += 1;
    while world.players()[0].motion_state != MotionState::Guard && frame < 180 {
        step_world(&mut world, Frame(frame), &shield);
        frame += 1;
    }

    assert_eq!(world.players()[0].motion_state, MotionState::Guard);
    assert_eq!(world.players()[0].position.y, platform.y);

    step_world(&mut world, Frame(frame), &shield_down);

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::Pass);
    assert!(!player.grounded);
    assert_eq!(
        player.velocity.y,
        source_units_to_milli(
            MeleeCommonData::provisional_mole().pass_initial_y_velocity - player.profile.gravity
        )
    );
    assert!(player.position.y < platform.y);
    assert_ne!(format!("{:?}", player.motion_state), "ShieldDrop");
    assert_ne!(format!("{:?}", player.motion_state), "AxeDrop");
}

#[test]
fn shield_down_jump_on_soft_platform_uses_jumpsquat_before_pass_like_guard_iasa() {
    let mut world = World::for_two_players();
    let (stage, mut frame) = land_player_one_on_left_platform(&mut world);
    let platform = stage.soft_platforms[0];
    let common = MeleeCommonData::provisional_mole();
    let shield = [
        PlayerInput::neutral().with_left_trigger_digital(true),
        PlayerInput::neutral(),
    ];
    let shield_down_jump = [
        PlayerInput::neutral()
            .with_left_trigger_digital(true)
            .with_left_stick(0, -common.platform_pass_y)
            .with_jump(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(frame), &shield);
    frame += 1;
    while world.players()[0].motion_state != MotionState::Guard && frame < 180 {
        step_world(&mut world, Frame(frame), &shield);
        frame += 1;
    }

    assert_eq!(world.players()[0].motion_state, MotionState::Guard);
    assert_eq!(world.players()[0].position.y, platform.y);

    step_world(&mut world, Frame(frame), &shield_down_jump);

    let player = world.players()[0];
    assert_eq!(
        player.motion_state,
        MotionState::KneeBend,
        "ftCo_Guard_IASA calls ftCo_800CB024 before ftCo_8009A080, so shield-drop input plus jump must enter KneeBend rather than Pass"
    );
    assert!(player.grounded);
    assert_eq!(player.position.y, platform.y);
    assert!(!player.platform_pass_pending);
    assert!(
        !player.source_shield_collision_active
            && !player.source_shield_hit_active
            && !player.source_shield_hit_update_pos,
        "ftCo_KneeBend_Enter runs Fighter_ChangeMotionState, clearing x221A_b7/x221B_b0 and the shield hit update flag"
    );
}

#[test]
fn shield_drop_pass_entry_uses_source_ground_to_air_and_action_change_primitives() {
    let mut world = World::for_two_players();
    let (stage, mut frame) = land_player_one_on_left_platform(&mut world);
    let platform = stage.soft_platforms[0];
    let common = MeleeCommonData::provisional_mole();
    let shield = [
        PlayerInput::neutral().with_left_trigger_digital(true),
        PlayerInput::neutral(),
    ];
    let shield_down = [
        PlayerInput::neutral()
            .with_left_trigger_digital(true)
            .with_left_stick(0, -common.platform_pass_y),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(frame), &shield);
    frame += 1;
    while world.players()[0].motion_state != MotionState::Guard && frame < 180 {
        step_world(&mut world, Frame(frame), &shield);
        frame += 1;
    }

    let mut player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::Guard);
    assert_eq!(player.position.y, platform.y);
    player.jumps_remaining = player.profile.max_jumps;
    player.motion_anim_rate_milli = 333;
    player.set_source_floor_for_diagnostic(Some(1), Some(7));
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(&mut world, Frame(frame), &shield_down);

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::Pass);
    assert!(!player.grounded);
    assert_eq!(
        player.jumps_remaining,
        player.profile.max_jumps.saturating_sub(1),
        "ftCommon_8007D5D4 marks the ground-to-air transition as having used one jump"
    );
    assert_eq!(
        player.source_floor_for_diagnostic(),
        (Some(1), Some(7)),
        "ftCo_8009A228 calls mpUpdateFloorSkip after ftCommon_8007D5D4; mpUpdateFloorSkip copies floor.index into floor_skip and does not clear the current floor index"
    );
    assert_eq!(
        player.motion_anim_rate_milli, 1_000,
        "Fighter_ChangeMotionState(... ftCo_MS_Pass, anim_start=0, anim_speed=1) resets source pose speed"
    );
}

#[test]
fn world_common_data_drives_platform_pass_gate_and_velocity() {
    let common = MeleeCommonData {
        platform_pass_y: 50,
        platform_pass_y_tap_window: 3,
        pass_initial_y_velocity: -1.6,
        ..MeleeCommonData::provisional_mole()
    };
    let mut world = World::for_two_players_with_common_data(common);
    let (stage, mut frame) = land_player_one_on_left_platform(&mut world);
    let platform = stage.soft_platforms[0];
    let shield = [
        PlayerInput::neutral().with_left_trigger_digital(true),
        PlayerInput::neutral(),
    ];
    let shield_down = [
        PlayerInput::neutral()
            .with_left_trigger_digital(true)
            .with_left_stick(0, -common.platform_pass_y),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(frame), &shield);
    frame += 1;
    while world.players()[0].motion_state != MotionState::Guard && frame < 180 {
        step_world(&mut world, Frame(frame), &shield);
        frame += 1;
    }

    assert_eq!(world.players()[0].motion_state, MotionState::Guard);
    assert_eq!(world.players()[0].position.y, platform.y);

    step_world(&mut world, Frame(frame), &shield_down);

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::Pass);
    assert_eq!(
        player.velocity.y,
        source_units_to_milli(common.pass_initial_y_velocity - player.profile.gravity)
    );
    assert_eq!(
        player.position.y,
        platform.y + source_units_to_milli(common.pass_initial_y_velocity - player.profile.gravity)
    );
}

#[test]
fn shield_hard_down_on_soft_platform_enters_spotdodge_before_pass() {
    let mut world = World::for_two_players();
    let (stage, mut frame) = land_player_one_on_left_platform(&mut world);
    let platform = stage.soft_platforms[0];
    let shield = [
        PlayerInput::neutral().with_left_trigger_digital(true),
        PlayerInput::neutral(),
    ];
    let hard_shield_down = [
        PlayerInput::neutral()
            .with_left_trigger_digital(true)
            .with_left_stick(0, spot_dodge_stick_y()),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(frame), &shield);
    frame += 1;
    while world.players()[0].motion_state != MotionState::Guard && frame < 180 {
        step_world(&mut world, Frame(frame), &shield);
        frame += 1;
    }

    assert_eq!(world.players()[0].motion_state, MotionState::Guard);

    step_world(&mut world, Frame(frame), &hard_shield_down);

    let player = world.players()[0];
    assert_eq!(
        player.motion_state,
        MotionState::EscapeN,
        "ftCo_Guard_IASA calls ftCo_8009980C before ftCo_8009A080, so a hard down tap in shield must spot dodge before platform pass"
    );
    assert!(player.grounded);
    assert_eq!(player.position.y, platform.y);
    assert!(!player.platform_pass_pending);
    assert!(
        !player.source_shield_collision_active
            && !player.source_shield_hit_active
            && !player.source_shield_hit_update_pos,
        "EscapeN entry must clear the guard shield object through Fighter_ChangeMotionState"
    );
}

#[test]
fn ucf_shield_drop_amendment_routes_guard_hard_down_to_pass_on_soft_platform() {
    let mut world = World::for_two_players();
    let (stage, mut frame) = land_player_one_on_left_platform(&mut world);
    let platform = stage.soft_platforms[0];
    let common = MeleeCommonData::provisional_mole();
    let shield = [
        PlayerInput::neutral().with_left_trigger_digital(true),
        PlayerInput::neutral(),
    ];
    let ucf_hard_shield_drop = [
        PlayerInput::neutral()
            .with_left_trigger_digital(true)
            .with_left_stick(0, spot_dodge_stick_y())
            .with_ucf_shield_drop_amendment(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(frame), &shield);
    frame += 1;
    while world.players()[0].motion_state != MotionState::Guard && frame < 180 {
        step_world(&mut world, Frame(frame), &shield);
        frame += 1;
    }

    assert_eq!(world.players()[0].motion_state, MotionState::Guard);

    step_world(&mut world, Frame(frame), &ucf_hard_shield_drop);

    let player = world.players()[0];
    assert_eq!(
        player.motion_state,
        MotionState::Pass,
        "UCF Shield Drop hooks the Guard spotdodge/pass decision, so the amendment should route the hard-down shield-drop candidate through ftCo_8009A228 without changing the raw stick vector"
    );
    assert!(!player.grounded);
    assert_eq!(
        player.velocity.y,
        source_units_to_milli(common.pass_initial_y_velocity - player.profile.gravity)
    );
    assert!(player.position.y < platform.y);
}

#[test]
fn ucf_shield_drop_amendment_keeps_cstick_spotdodge_priority() {
    let mut world = World::for_two_players();
    let (stage, mut frame) = land_player_one_on_left_platform(&mut world);
    let platform = stage.soft_platforms[0];
    let shield = [
        PlayerInput::neutral().with_left_trigger_digital(true),
        PlayerInput::neutral(),
    ];
    let cstick_spotdodge_with_ucf_drop = [
        PlayerInput::neutral()
            .with_left_trigger_digital(true)
            .with_left_stick(84, -94)
            .with_c_stick(0, spot_dodge_stick_y())
            .with_ucf_shield_drop_amendment(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(frame), &shield);
    frame += 1;
    while world.players()[0].motion_state != MotionState::Guard && frame < 180 {
        step_world(&mut world, Frame(frame), &shield);
        frame += 1;
    }

    assert_eq!(world.players()[0].motion_state, MotionState::Guard);

    step_world(&mut world, Frame(frame), &cstick_spotdodge_with_ucf_drop);

    let player = world.players()[0];
    assert_eq!(
        player.motion_state,
        MotionState::EscapeN,
        "UCF Shield Drop.asm prioritizes C-stick spotdodge before suppressing the main-stick spotdodge route"
    );
    assert!(player.grounded);
    assert_eq!(player.position.y, platform.y);
}

#[test]
fn pass_from_shield_requires_soft_platform_support() {
    let mut world = World::for_two_players();
    let mut frame = 0;
    let shield = [
        PlayerInput::neutral().with_left_trigger_digital(true),
        PlayerInput::neutral(),
    ];
    let shield_pass_down = [
        PlayerInput::neutral()
            .with_left_trigger_digital(true)
            .with_left_stick(0, -MeleeCommonData::provisional_mole().platform_pass_y),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(frame), &shield);
    frame += 1;
    while world.players()[0].motion_state != MotionState::Guard && frame < 20 {
        step_world(&mut world, Frame(frame), &shield);
        frame += 1;
    }

    assert_eq!(world.players()[0].motion_state, MotionState::Guard);
    assert_eq!(world.players()[0].position.y, world.stage().main_floor.y);

    step_world(&mut world, Frame(frame), &shield_pass_down);

    assert_eq!(world.players()[0].motion_state, MotionState::Guard);
    assert!(world.players()[0].grounded);
    assert_ne!(world.players()[0].motion_state, MotionState::Pass);
}

#[test]
fn down_tap_from_squat_arms_platform_pass_without_shield() {
    let mut world = World::for_two_players();
    let (stage, mut frame) = land_player_one_on_left_platform(&mut world);
    let platform = stage.soft_platforms[0];
    let common = MeleeCommonData::provisional_mole();
    let pass_down_y = -((common.platform_pass_y.max(common.crouch_y) as i16) + 1) as i8;
    let pass_down = [
        PlayerInput::neutral().with_left_stick(0, pass_down_y),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(frame), &pass_down);
    frame += 1;
    assert_eq!(world.players()[0].motion_state, MotionState::Squat);
    assert!(!world.players()[0].platform_pass_pending);

    step_world(&mut world, Frame(frame), &pass_down);

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::Squat);
    assert!(player.grounded);
    assert_eq!(player.position.y, platform.y);
    assert!(player.platform_pass_pending);
    assert_eq!(player.platform_pass_timer, common.platform_drop_delay_ticks);
}

#[test]
fn squat_platform_pass_delay_enters_shared_pass_state_without_shield() {
    let mut world = World::for_two_players();
    let (_stage, mut frame) = land_player_one_on_left_platform(&mut world);
    let common = MeleeCommonData::provisional_mole();
    let pass_down_y = -((common.platform_pass_y.max(common.crouch_y) as i16) + 1) as i8;
    let pass_down = [
        PlayerInput::neutral().with_left_stick(0, pass_down_y),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(frame), &pass_down);
    frame += 1;
    assert_eq!(world.players()[0].motion_state, MotionState::Squat);

    step_world(&mut world, Frame(frame), &pass_down);
    frame += 1;
    assert!(world.players()[0].platform_pass_pending);

    for _ in 0..common.platform_drop_delay_ticks {
        step_world(&mut world, Frame(frame), &pass_down);
        frame += 1;
    }

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::Pass);
    assert!(!player.grounded);
    assert!(!player.platform_pass_pending);
    assert_eq!(player.platform_pass_timer, 0);
    assert_ne!(format!("{:?}", player.motion_state), "ShieldDrop");
    assert_ne!(format!("{:?}", player.motion_state), "AxeDrop");
}

#[test]
fn vanilla_platform_pass_entry_uses_pass_without_custom_drop_state() {
    let mut first = World::for_two_players();
    let mut second = World::for_two_players();

    let pass_frame = enter_pass_from_left_platform(&mut first);
    let second_pass_frame = enter_pass_from_left_platform(&mut second);

    assert_eq!(pass_frame, second_pass_frame);
    assert_eq!(first.checksum(), second.checksum());
    assert_eq!(first.players()[0].motion_state, MotionState::Pass);
    assert_eq!(format!("{:?}", first.players()[0].motion_state), "Pass");
    assert_ne!(
        format!("{:?}", first.players()[0].motion_state),
        "ShieldDrop"
    );
    assert_ne!(format!("{:?}", first.players()[0].motion_state), "AxeDrop");
}

#[test]
fn pass_state_accepts_airborne_action_inputs_in_source_order() {
    let mut special = World::for_two_players();
    let special_frame = enter_pass_from_left_platform(&mut special);
    step_world(
        &mut special,
        Frame(special_frame),
        &[
            PlayerInput::neutral()
                .with_special(true)
                .with_attack(true)
                .with_right_trigger_digital(true)
                .with_jump(true)
                .with_left_stick(80, 0),
            PlayerInput::neutral(),
        ],
    );
    assert_eq!(
        special.players()[0].motion_state,
        MotionState::SpecialAirSStart
    );

    let mut air_dodge = World::for_two_players();
    let air_dodge_frame = enter_pass_from_left_platform(&mut air_dodge);
    step_world(
        &mut air_dodge,
        Frame(air_dodge_frame),
        &[
            PlayerInput::neutral().with_right_trigger_digital(true),
            PlayerInput::neutral(),
        ],
    );
    assert_eq!(air_dodge.players()[0].motion_state, MotionState::EscapeAir);

    let mut aerial = World::for_two_players();
    let aerial_frame = enter_pass_from_left_platform(&mut aerial);
    step_world(
        &mut aerial,
        Frame(aerial_frame),
        &[
            PlayerInput::neutral().with_attack(true),
            PlayerInput::neutral(),
        ],
    );
    assert_eq!(aerial.players()[0].motion_state, MotionState::AttackAirN);

    let mut air_jump = World::for_two_players();
    let air_jump_frame = enter_pass_from_left_platform(&mut air_jump);
    let jumps_before = air_jump.players()[0].jumps_remaining;
    step_world(
        &mut air_jump,
        Frame(air_jump_frame),
        &[
            PlayerInput::neutral().with_jump(true),
            PlayerInput::neutral(),
        ],
    );
    assert_eq!(air_jump.players()[0].motion_state, MotionState::JumpAerialF);
    assert_eq!(air_jump.players()[0].jumps_remaining, jumps_before - 1);
}

#[test]
fn pass_state_applies_normal_air_drift_without_action() {
    let mut world = World::for_two_players();
    let frame = enter_pass_from_left_platform(&mut world);
    let velocity_before = world.players()[0].velocity.x;

    step_world(
        &mut world,
        Frame(frame),
        &[
            PlayerInput::neutral().with_left_stick(80, 0),
            PlayerInput::neutral(),
        ],
    );

    assert_eq!(world.players()[0].motion_state, MotionState::Pass);
    assert!(world.players()[0].velocity.x > velocity_before);
}

#[test]
fn pass_air_drift_advances_from_source_self_velocity_like_ft_80084db0() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    let source_velocity_x = -0.4695_f32;
    let stick_x = -87;
    player.motion_state = MotionState::Pass;
    player.motion_state_alias = Some(MotionState::Pass);
    player.source_action_key = Some(SourceActionKey::new("Pass"));
    player.melee_action_state_id = Some(MeleeActionStateId::new(244));
    player.motion_frame = 2;
    player.grounded = false;
    player.position = Vec2 { x: 0, y: 50_000 };
    player.velocity.x = source_units_to_milli(source_velocity_x);
    player.source_self_velocity_x = source_velocity_x;
    player.velocity.y = 0;
    player.source_self_velocity_y = 0.0;
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(0),
        &[
            PlayerInput::neutral().with_left_stick(stick_x, 0),
            PlayerInput::neutral(),
        ],
    );

    let expected_source_velocity =
        source_air_drift_velocity_f32(source_velocity_x, stick_x as i32, player.profile);
    assert_eq!(
        world.players()[0].source_self_velocity_x.to_bits(),
        expected_source_velocity.to_bits(),
        "ft_80084DB0/ftCommon_8007D28C advances Fighter.self_vel.x from the stored float, not from the rounded render velocity"
    );
    assert_eq!(
        world.players()[0].velocity.x,
        source_units_to_milli(expected_source_velocity)
    );
}

#[test]
fn pass_animation_completion_enters_fall() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.motion_state = MotionState::Pass;
    player.motion_frame = falcon_pass_action_frames() - 1;
    player.position = Vec2 {
        x: 0,
        y: melee_units_f32(80.0),
    };
    player.grounded = false;
    player.velocity = Vec2 { x: 0, y: 0 };
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    assert_eq!(world.players()[0].motion_state, MotionState::Fall);
}

#[test]
fn pass_floor_skip_targets_only_the_platform_that_was_dropped_through() {
    let stage = StageProfile {
        name: "stacked_soft_platforms",
        main_floor: StageSurface {
            name: "main_floor",
            kind: StageSurfaceKind::Solid,
            left_x: -20_000,
            right_x: 20_000,
            y: 0,
            friction_multiplier: 1.0,
        },
        soft_platforms: [
            StageSurface {
                name: "upper_soft",
                kind: StageSurfaceKind::Soft,
                left_x: -20_000,
                right_x: 20_000,
                y: 20_000,
                friction_multiplier: 1.0,
            },
            StageSurface {
                name: "lower_soft",
                kind: StageSurfaceKind::Soft,
                left_x: -20_000,
                right_x: 20_000,
                y: 10_000,
                friction_multiplier: 1.0,
            },
            StageSurface {
                name: "side_soft",
                kind: StageSurfaceKind::Soft,
                left_x: 30_000,
                right_x: 40_000,
                y: 15_000,
                friction_multiplier: 1.0,
            },
        ],
        ledges: &[],
        blast_zones: World::for_two_players().stage().blast_zones,
        spawn_points: World::for_two_players().stage().spawn_points,
        respawn_platforms: World::for_two_players().stage().respawn_platforms,
    };

    let contact = landing_contact_for_bottom_with_floor_skip(
        stage,
        Vec2 { x: 0, y: 21_000 },
        Vec2 { x: 0, y: 9_000 },
        Some(1),
        false,
    )
    .expect("floor skip should still allow landing on a different soft platform");

    assert_eq!(contact.surface.name, "lower_soft");
}

#[test]
fn pass_entry_records_and_preserves_source_floor_skip() {
    let mut world = World::for_two_players();
    let frame = enter_pass_from_left_platform(&mut world);

    assert_eq!(world.players()[0].floor_skip_surface, Some(1));

    step_world(
        &mut world,
        Frame(frame),
        &[
            PlayerInput::neutral().with_right_trigger_digital(true),
            PlayerInput::neutral(),
        ],
    );

    assert_eq!(world.players()[0].motion_state, MotionState::EscapeAir);
    assert_eq!(world.players()[0].floor_skip_surface, Some(1));
}

#[test]
fn escape_air_from_pass_preserves_locked_floor_probe() {
    let mut world = World::for_two_players();
    let frame = enter_pass_from_left_platform(&mut world);

    assert_eq!(world.players()[0].motion_state, MotionState::Pass);
    assert_eq!(world.players()[0].ecb_bottom_offset_y, 0);

    step_world(
        &mut world,
        Frame(frame),
        &[
            PlayerInput::neutral()
                .with_right_trigger_digital(true)
                .with_left_stick(84, -76),
            PlayerInput::neutral(),
        ],
    );

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::EscapeAir);
    assert_eq!(player.ecb_bottom_offset_y, 0);
}

#[test]
fn aerial_jump_from_pass_reloads_source_jobj_ecb_bottom_probe() {
    let mut world = World::for_two_players();
    let frame = enter_pass_from_left_platform(&mut world);

    assert_eq!(world.players()[0].motion_state, MotionState::Pass);
    assert_eq!(world.players()[0].ecb_bottom_offset_y, 0);

    step_world(
        &mut world,
        Frame(frame),
        &[
            PlayerInput::neutral()
                .with_jump(true)
                .with_left_stick(80, 20),
            PlayerInput::neutral(),
        ],
    );

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::JumpAerialF);
    assert_eq!(player.ecb_bottom_offset_y, 2_790);
}

#[test]
fn slippi_escape_air_jobj_probe_lands_on_top_platform_on_oracle_frame_131() {
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let top_platform = world.stage().soft_platforms[2];
    let mut player = world.players()[1];
    player.motion_state = MotionState::EscapeAir;
    player.motion_frame = 3;
    player.position = Vec2 {
        x: melee_units_f32(9.317),
        y: 53_124,
    };
    player.velocity = Vec2 { x: 0, y: -1_684 };
    player.grounded = false;
    assert!(world.set_player_state_for_diagnostic(1, player));

    step_world(
        &mut world,
        Frame(131),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    assert_eq!(
        world.players()[1].motion_state,
        MotionState::LandingFallSpecial
    );
    assert_eq!(world.players()[1].position.y, top_platform.y);
    assert_eq!(world.players()[1].velocity.y, -1_516);
    assert!(world.players()[1].grounded);
}

#[test]
#[ignore = "mid-state diagnostic cannot seed private source CollData history; runtime source-collision sequence owns this parity assertion"]
fn escape_air_visible_counter_four_lands_on_right_platform_under_decomp_floor_sweep() {
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let right_platform = world.stage().soft_platforms[1];
    let mut player = world.players()[1];
    player.motion_state = MotionState::EscapeAir;
    player.set_motion_state_alias(MotionState::EscapeAir);
    player.melee_action_state_id = Some(MeleeActionStateId::new(236));
    player.source_action_key = Some(SourceActionKey::new("EscapeAir"));
    player.source_action_total_frames = 50;
    player.motion_frame = 2;
    player.set_source_motion_anim_frame(2.0);
    player.position = Vec2 {
        x: melee_units_f32(26.692_904),
        y: melee_units_f32(26.407_015),
    };
    player.velocity = Vec2 {
        x: melee_units_f32(-2.026_994),
        y: melee_units_f32(-0.999_222),
    };
    player.grounded = false;
    assert!(world.set_player_state_for_diagnostic(1, player));

    step_world(
        &mut world,
        Frame(212),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    let player = world.players()[1];
    assert_eq!(player.motion_state, MotionState::LandingFallSpecial);
    assert!(player.grounded);
    assert_eq!(player.position.y, right_platform.y);
    assert_eq!(
        player.landing_lag_ticks,
        world.common_data().escapeair_landing_lag_ticks
    );
}

#[test]
fn fall_iasa_escape_air_entry_does_not_land_on_same_frame_platform_crossing() {
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let mut player = world.players()[1];
    player.set_motion_state_alias(MotionState::Fall);
    player.motion_frame = 15;
    player.set_source_motion_anim_frame(15.0);
    player.grounded = false;
    player.facing = 1;
    player.position = Vec2 {
        x: melee_units_f32(34.608_62),
        y: melee_units_f32(25.220_1),
    };
    player.source_position = SourceVec2 {
        x: 34.608_62,
        y: 25.220_1,
    };
    player.source_self_velocity_x = 0.823_5;
    player.source_self_velocity_y = -3.5;
    player.velocity.x = source_units_to_milli(player.source_self_velocity_x);
    player.velocity.y = source_units_to_milli(player.source_self_velocity_y);
    player.set_source_floor_for_diagnostic(Some(3), Some(3));
    assert!(world.set_player_state_for_diagnostic(1, player));
    world.set_input_history_for_diagnostic(
        [
            PlayerInput::neutral(),
            PlayerInput::neutral()
                .with_left_stick(-83, -95)
                .with_left_trigger_analog(200),
        ],
        [MeleeInputTimers::expired(); 2],
    );

    step_world(
        &mut world,
        Frame(142),
        &[
            PlayerInput::neutral(),
            PlayerInput::neutral()
                .with_left_stick(-83, -95)
                .with_left_trigger_analog(255)
                .with_left_trigger_digital(true),
        ],
    );

    let player = world.players()[1];
    assert_eq!(
        player.motion_state,
        MotionState::EscapeAir,
        "ftCo_Fall_IASA enters EscapeAir before Fall_Coll; the newly-entered air dodge should not be converted into LandingFallSpecial on that same frame"
    );
    assert!(!player.grounded);
    assert!(
        player.position.y < world.stage().soft_platforms[1].y,
        "the frame should translate by EscapeAir self_vel rather than snap to the right platform"
    );
}

#[test]
#[ignore = "legacy coarse-contact fallback; source-runtime replay trace is authoritative for this parity target"]
fn escape_air_visible_counter_five_lands_on_right_platform() {
    let mut world = World::for_two_players();
    let right_platform = world.stage().soft_platforms[1];
    let mut player = world.players()[1];
    player.motion_state = MotionState::EscapeAir;
    player.motion_frame = 3;
    player.position = Vec2 {
        x: melee_units_f32(24.868_61),
        y: melee_units_f32(25.507_715),
    };
    player.velocity = Vec2 {
        x: melee_units_f32(-1.824_294),
        y: melee_units_f32(-0.899_3),
    };
    player.grounded = false;
    assert!(world.set_player_state_for_diagnostic(1, player));

    step_world(
        &mut world,
        Frame(213),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    assert_eq!(
        world.players()[1].motion_state,
        MotionState::LandingFallSpecial
    );
    assert_eq!(
        world.players()[1].landing_lag_ticks,
        world.common_data().escapeair_landing_lag_ticks
    );
    assert_eq!(world.players()[1].position.y, right_platform.y);
    assert!(world.players()[1].grounded);
}

#[test]
fn landing_fall_special_shield_entry_preserves_source_ground_slide() {
    let common = MeleeCommonData::provisional_mole();
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.motion_state = MotionState::LandingFallSpecial;
    player.motion_frame = common.escapeair_landing_lag_ticks - 1;
    player.position = Vec2 { x: 0, y: 0 };
    player.grounded = true;
    player.ground_velocity_x = milli_to_source_units(-565);
    player.velocity.x = -565;
    player.velocity.y = 0;
    assert!(world.set_player_state_for_diagnostic(0, player));

    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(shield_analog()),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &shield);

    assert_eq!(world.players()[0].motion_state, MotionState::GuardOn);
    assert_eq!(
        world.players()[0].velocity.x,
        source_general_grounded_friction_velocity(-565, player.profile, common)
    );
    assert_eq!(
        world.players()[0].ground_velocity_x,
        milli_to_source_units(source_general_grounded_friction_velocity(
            -565,
            player.profile,
            common
        ))
    );
}

#[test]
fn landing_fall_special_clings_to_current_floor_edge_until_hard_outward_stick() {
    let mut world = World::for_two_players();
    let right_platform = world.stage().soft_platforms[1];
    let mut player = world.players()[1];
    player.motion_state = MotionState::LandingFallSpecial;
    player.motion_frame = 4;
    player.position = Vec2 {
        x: right_platform.left_x + 533,
        y: right_platform.y,
    };
    player.facing = -1;
    player.grounded = true;
    player.ground_velocity_x = milli_to_source_units(-698);
    player.velocity.x = -698;
    player.velocity.y = 0;
    assert!(world.set_player_state_for_diagnostic(1, player));

    let edge_cling = [
        PlayerInput::neutral(),
        PlayerInput::neutral().with_left_stick(-94, -84),
    ];
    step_world(&mut world, Frame(355), &edge_cling);

    assert_eq!(
        world.players()[1].motion_state,
        MotionState::LandingFallSpecial
    );
    assert_eq!(world.players()[1].position.x, right_platform.left_x - 85);
    assert_eq!(world.players()[1].position.y, right_platform.y);
    assert!(world.players()[1].grounded);

    let hard_outward = [
        PlayerInput::neutral(),
        PlayerInput::neutral().with_left_stick(-97, -81),
    ];
    step_world(&mut world, Frame(356), &hard_outward);

    assert_eq!(world.players()[1].motion_state, MotionState::Fall);
    assert_eq!(world.players()[1].position.y, right_platform.y);
    assert_eq!(world.players()[1].velocity.y, 0);
    assert!(!world.players()[1].grounded);
}

#[test]
fn landing_fall_special_raw_stick_95_exits_floor_edge_threshold() {
    let mut world = World::for_two_players();
    let top_platform = world.stage().soft_platforms[2];
    let mut player = world.players()[1];
    player.motion_state = MotionState::LandingFallSpecial;
    player.motion_frame = 4;
    player.position = Vec2 {
        x: top_platform.right_x - 329,
        y: top_platform.y,
    };
    player.facing = 1;
    player.grounded = true;
    player.ground_velocity_x = milli_to_source_units(1_298);
    player.velocity.x = 1_298;
    player.velocity.y = 0;
    assert!(world.set_player_state_for_diagnostic(1, player));

    let edge_exit = [
        PlayerInput::neutral(),
        PlayerInput::neutral().with_left_stick(95, 0),
    ];
    step_world(&mut world, Frame(335), &edge_exit);

    assert_eq!(world.players()[1].motion_state, MotionState::Fall);
    assert_eq!(world.players()[1].position.y, top_platform.y);
    assert_eq!(world.players()[1].velocity.y, 0);
    assert!(!world.players()[1].grounded);
}

#[test]
fn escape_air_landing_uses_animated_jobj_bottom_probe_between_frames() {
    let mut world = World::for_two_players();
    let right_platform = world.stage().soft_platforms[1];
    let mut player = world.players()[1];
    player.motion_state = MotionState::EscapeAir;
    player.motion_frame = 4;
    player.position = Vec2 {
        x: melee_units_f32(24.868_61),
        y: melee_units_f32(25.507_715),
    };
    player.velocity = Vec2 {
        x: melee_units_f32(-1.824_294),
        y: melee_units_f32(-0.899_3),
    };
    player.grounded = false;
    assert!(world.set_player_state_for_diagnostic(1, player));

    step_world(
        &mut world,
        Frame(213),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    assert_eq!(
        world.players()[1].motion_state,
        MotionState::LandingFallSpecial
    );
    assert_eq!(world.players()[1].position.y, right_platform.y);
    assert!(world.players()[1].grounded);
}

#[test]
fn slippi_air_dodge_oracle_frame_92_lands_on_right_platform() {
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let right_platform = world.stage().soft_platforms[1];
    let mut player = world.players()[1];
    player.motion_state = MotionState::JumpAerialF;
    player.motion_frame = 10;
    player.position = Vec2 {
        x: melee_units_f32(32.916_435),
        y: melee_units_f32(24.010_094),
    };
    player.velocity = Vec2 {
        x: melee_units_f32(0.660),
        y: melee_units_f32(1.360),
    };
    player.facing = 1;
    player.grounded = false;
    assert!(world.set_player_state_for_diagnostic(1, player));

    let air_dodge_down_back = [
        PlayerInput::neutral(),
        PlayerInput::neutral()
            .with_left_stick(-98, -78)
            .with_left_trigger_analog(255)
            .with_left_trigger_digital(true),
    ];

    step_world(&mut world, Frame(215), &air_dodge_down_back);

    assert_eq!(
        world.players()[1].motion_state,
        MotionState::LandingFallSpecial
    );
    assert_eq!(world.players()[1].position.y, right_platform.y);
    assert!(world.players()[1].grounded);
}

#[test]
fn render_snapshot_exposes_active_gameplay_ecb_from_sampled_action_pose() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.motion_state = MotionState::EscapeAir;
    player.motion_frame = 2;
    player.position = Vec2 { x: 1_000, y: 2_000 };
    player.ecb_bottom_offset_y = 2_790;
    player.grounded = false;
    assert!(world.set_player_state_for_diagnostic(0, player));

    let snapshot = world.snapshot();
    let ecb = snapshot.players[0].active_ecb;

    assert_eq!(ecb.bottom, Vec2 { x: 1_000, y: 3_998 });
    assert_eq!(
        ecb.top,
        Vec2 {
            x: 1_000,
            y: 12_865
        }
    );
    assert_eq!(ecb.right, Vec2 { x: 4_913, y: 8_432 });
    assert_eq!(
        ecb.left,
        Vec2 {
            x: -2_913,
            y: 8_432
        }
    );
    assert_ne!(ecb.bottom.y, player.position.y + player.ecb_bottom_offset_y);
}

#[test]
fn render_snapshot_exposes_generated_dash_ecb_from_action_pose_table() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.motion_state = MotionState::Dash;
    player.motion_frame = 2;
    player.position = Vec2 { x: 1_000, y: 2_000 };
    assert!(world.set_player_state_for_diagnostic(0, player));

    let snapshot = world.snapshot();
    let ecb = snapshot.players[0].active_ecb;

    assert_eq!(ecb.bottom, Vec2 { x: 1_000, y: 7_988 });
    assert_eq!(
        ecb.top,
        Vec2 {
            x: 1_000,
            y: 17_769
        }
    );
    assert_eq!(
        ecb.right,
        Vec2 {
            x: 4_955,
            y: 12_879
        }
    );
    assert_eq!(
        ecb.left,
        Vec2 {
            x: -2_955,
            y: 12_879
        }
    );
}

#[test]
fn render_snapshot_exposes_generated_fall_aerial_ecb_from_action_pose_table() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.motion_state = MotionState::FallAerial;
    player.motion_frame = 0;
    player.position = Vec2 { x: 1_000, y: 2_000 };
    assert!(world.set_player_state_for_diagnostic(0, player));

    let snapshot = world.snapshot();
    let ecb = snapshot.players[0].active_ecb;

    assert_eq!(ecb.bottom, Vec2 { x: 1_000, y: 4_730 });
    assert_eq!(
        ecb.top,
        Vec2 {
            x: 1_000,
            y: 13_227
        }
    );
    assert_eq!(ecb.right, Vec2 { x: 7_303, y: 8_979 });
    assert_eq!(
        ecb.left,
        Vec2 {
            x: -3_779,
            y: 8_979
        }
    );
}

#[test]
fn render_snapshot_uses_fall_directional_submotion_ecb_without_changing_state() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.motion_state = MotionState::Fall;
    player.motion_frame = 0;
    player.position = Vec2 { x: 1_000, y: 2_000 };
    player.velocity.x = source_units_to_milli(player.profile.air_drift_max);
    player.grounded = false;
    assert!(world.set_player_state_for_diagnostic(0, player));

    let snapshot = world.snapshot();
    let player_snapshot = snapshot.players[0];
    let ecb = player_snapshot.active_ecb;

    assert_eq!(player_snapshot.motion_state, MotionState::Fall);
    assert_eq!(ecb.bottom, Vec2 { x: 1_000, y: 3_344 });
    assert_eq!(
        ecb.top,
        Vec2 {
            x: 1_000,
            y: 12_374
        }
    );
    assert_eq!(ecb.right, Vec2 { x: 5_571, y: 7_859 });
    assert_eq!(
        ecb.left,
        Vec2 {
            x: -3_571,
            y: 7_859
        }
    );
}

#[test]
fn render_snapshot_uses_fall_aerial_directional_submotion_ecb_without_changing_state() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.motion_state = MotionState::FallAerial;
    player.motion_frame = 0;
    player.position = Vec2 { x: 1_000, y: 2_000 };
    player.velocity.x = -source_units_to_milli(player.profile.air_drift_max);
    player.grounded = false;
    assert!(world.set_player_state_for_diagnostic(0, player));

    let snapshot = world.snapshot();
    let player_snapshot = snapshot.players[0];
    let ecb = player_snapshot.active_ecb;

    assert_eq!(player_snapshot.motion_state, MotionState::FallAerial);
    assert_eq!(ecb.bottom, Vec2 { x: 1_000, y: 4_851 });
    assert_eq!(
        ecb.top,
        Vec2 {
            x: 1_000,
            y: 16_359
        }
    );
    assert_eq!(
        ecb.right,
        Vec2 {
            x: 8_020,
            y: 10_605
        }
    );
    assert_eq!(
        ecb.left,
        Vec2 {
            x: -5_922,
            y: 10_605
        }
    );
}

#[test]
fn render_snapshot_uses_fall_special_directional_submotion_ecb_without_changing_state() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.motion_state = MotionState::FallSpecial;
    player.motion_frame = 0;
    player.position = Vec2 { x: 1_000, y: 2_000 };
    player.velocity.x = source_units_to_milli(player.profile.air_drift_max);
    player.grounded = false;
    assert!(world.set_player_state_for_diagnostic(0, player));

    let snapshot = world.snapshot();
    let player_snapshot = snapshot.players[0];
    let ecb = player_snapshot.active_ecb;

    assert_eq!(player_snapshot.motion_state, MotionState::FallSpecial);
    assert_eq!(ecb.bottom, Vec2 { x: 1_000, y: 2_779 });
    assert_eq!(
        ecb.top,
        Vec2 {
            x: 1_000,
            y: 14_873
        }
    );
    assert_eq!(ecb.right, Vec2 { x: 4_134, y: 8_826 });
    assert_eq!(
        ecb.left,
        Vec2 {
            x: -2_134,
            y: 8_826
        }
    );
}

#[test]
fn render_snapshot_exposes_generated_fall_special_forward_ecb_from_action_pose_table() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.motion_state = MotionState::FallSpecialF;
    player.motion_frame = 0;
    player.position = Vec2 { x: 1_000, y: 2_000 };
    assert!(world.set_player_state_for_diagnostic(0, player));

    let snapshot = world.snapshot();
    let ecb = snapshot.players[0].active_ecb;

    assert_eq!(ecb.bottom, Vec2 { x: 1_000, y: 2_779 });
    assert_eq!(
        ecb.top,
        Vec2 {
            x: 1_000,
            y: 14_873
        }
    );
    assert_eq!(ecb.right, Vec2 { x: 4_134, y: 8_826 });
    assert_eq!(
        ecb.left,
        Vec2 {
            x: -2_134,
            y: 8_826
        }
    );
}

#[test]
fn render_snapshot_exposes_generated_landing_fall_special_ecb_from_exact_submotion() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.motion_state = MotionState::LandingFallSpecial;
    player.motion_frame = 0;
    player.position = Vec2 { x: 1_000, y: 2_000 };
    assert!(world.set_player_state_for_diagnostic(0, player));

    let snapshot = world.snapshot();
    let ecb = snapshot.players[0].active_ecb;

    assert_eq!(ecb.bottom, Vec2 { x: 1_000, y: 2_000 });
    assert_eq!(
        ecb.top,
        Vec2 {
            x: 1_000,
            y: 11_347
        }
    );
    assert_eq!(ecb.right, Vec2 { x: 5_512, y: 6_674 });
    assert_eq!(
        ecb.left,
        Vec2 {
            x: -3_512,
            y: 6_674
        }
    );
}

#[test]
fn landing_fall_special_render_pose_uses_source_scaled_landing_animation_rate() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.motion_state = MotionState::LandingFallSpecial;
    player.motion_frame = 5;
    player.motion_anim_frame_milli = 15_050;
    player.position = Vec2 { x: 1_000, y: 2_000 };
    assert!(world.set_player_state_for_diagnostic(0, player));

    let snapshot = world.snapshot();
    let ecb = snapshot.players[0].active_ecb;

    // ftCo_LandingFallSpecial_Enter sets anim_speed to
    // (0.1 + Landing figatree frames) / x344. Falcon's LandingFallSpecial
    // uses the 30-frame Landing figatree, so lag tick 5 samples action frame 15.
    assert_eq!(ecb.bottom, Vec2 { x: 1_000, y: 2_156 });
    assert_eq!(
        ecb.top,
        Vec2 {
            x: 1_000,
            y: 10_106
        }
    );
    assert_eq!(ecb.right, Vec2 { x: 5_276, y: 6_131 });
    assert_eq!(
        ecb.left,
        Vec2 {
            x: -3_276,
            y: 6_131
        }
    );
}

#[test]
fn landing_air_render_pose_uses_source_scaled_landing_animation_rate() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.motion_state = MotionState::LandingAirF;
    player.motion_frame = 5;
    player.landing_lag_ticks = player.profile.landing_air_f_lag_ticks;
    player.position = Vec2 { x: 1_000, y: 2_000 };
    assert!(world.set_player_state_for_diagnostic(0, player));

    let snapshot = world.snapshot();
    let ecb = snapshot.players[0].active_ecb;

    // ftCo_LandingAir_EnterWithMsidLag sets anim_speed to
    // (0.1 + figatree frames) / lag. Falcon LandingAirF has 30 source
    // samples and 19 landing-lag ticks, so lag tick 5 samples action frame 7.
    assert_eq!(ecb.bottom, Vec2 { x: 1_000, y: 3_983 });
    assert_eq!(
        ecb.top,
        Vec2 {
            x: 1_000,
            y: 15_442
        }
    );
    assert_eq!(ecb.right, Vec2 { x: 5_025, y: 9_713 });
    assert_eq!(
        ecb.left,
        Vec2 {
            x: -3_025,
            y: 9_713
        }
    );
}

#[test]
fn ground_jump_escape_air_uses_locked_floor_probe_for_same_frame_landing() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.motion_state = MotionState::JumpF;
    player.motion_frame = 0;
    player.position = Vec2 {
        x: melee_units_f32(35.357_224),
        y: melee_units_f32(1.900_1),
    };
    player.velocity = Vec2 {
        x: melee_units_f32(-1.577_344),
        y: melee_units_f32(1.9),
    };
    player.source_self_velocity_x = -1.577_344;
    player.source_self_velocity_y = 1.9;
    player.ecb_bottom_offset_y = 0;
    player.ecb_bottom_lock_timer = 9;
    player.grounded = false;
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(178),
        &[
            PlayerInput::neutral()
                .with_left_stick(-90, -87)
                .with_left_trigger_digital(true),
            PlayerInput::neutral(),
        ],
    );

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::LandingFallSpecial);
    assert!(player.grounded);
    assert_eq!(player.position.y, world.stage().main_floor.y);
}

#[test]
fn pass_landing_uses_source_bottom_floor_probe_after_skipping_platform() {
    let mut world = World::for_two_players();
    let frame = enter_pass_from_left_platform(&mut world);

    let mut player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::Pass);
    assert_eq!(player.ecb_bottom_offset_y, 0);
    player.motion_frame = 25;
    player.ecb_bottom_lock_timer = 0;
    player.position.x = melee_units_f32(-20.413_055);
    player.position.y = melee_units_f32(-0.029_897);
    player.velocity.x = melee_units_f32(-0.842_192);
    player.velocity.y = melee_units_f32(-2.9);
    player.source_self_velocity_x = -0.842_192;
    player.source_self_velocity_y = -2.9;
    player.grounded = false;
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(frame + 1),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    assert_eq!(world.players()[0].motion_state, MotionState::Landing);
    assert_eq!(world.players()[0].position.y, world.stage().main_floor.y);
}

#[test]
fn pass_crossing_main_floor_remains_pass_until_collision_callback_frame() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.motion_state = MotionState::Pass;
    player.motion_state_alias = Some(MotionState::Pass);
    player.melee_action_state_id = Some(MeleeActionStateId::new(244));
    player.source_action_key = Some(SourceActionKey::new("Pass"));
    player.motion_frame = 24;
    player.grounded = false;
    player.floor_skip_surface = Some(1);
    player.ecb_bottom_offset_y = 0;
    player.ecb_bottom_lock_timer = 0;
    player.position = Vec2 {
        x: -19_571,
        y: 2_870,
    };
    player.velocity = Vec2 { x: -852, y: -2_900 };
    player.source_self_velocity_x = milli_to_source_units(-852);
    player.source_self_velocity_y = milli_to_source_units(-2_900);
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(217),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    assert_eq!(world.players()[0].motion_state, MotionState::Pass);
    assert_eq!(world.players()[0].position.y, -30);
    assert_eq!(world.players()[0].velocity.y, -2_900);
}

#[test]
fn held_shield_enters_guard_on_after_source_normal_landing_lag_gate() {
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

    for expected_motion_frame in 1..FighterProfile::falcon_like().normal_landing_lag_ticks {
        step_world(&mut world, Frame(frame), &shield);
        frame += 1;
        assert_eq!(world.players()[0].motion_state, MotionState::Landing);
        assert_eq!(world.players()[0].motion_frame, expected_motion_frame);
    }

    step_world(&mut world, Frame(frame), &shield);

    assert_eq!(
        world.players()[0].motion_state,
        MotionState::GuardOn,
        "ftCo_Landing_IASA returns while cur_anim_frame < normal_landing_lag, then may enter GuardOn through ftCo_80091A4C once the source lag gate is reached"
    );
}

#[test]
fn ordinary_landing_lag_gates_iasa_before_animation_completion() {
    let profile = FighterProfile {
        normal_landing_lag_ticks: 2,
        ..FighterProfile::falcon_like()
    };
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let platform = world.stage().soft_platforms[0];
    let mut player = world.players()[0];
    player.position = Vec2 {
        x: (platform.left_x + platform.right_x) / 2,
        y: platform.y + 2_000,
    };
    player.velocity = Vec2 { x: 0, y: -1_000 };
    player.motion_state = MotionState::Fall;
    player.motion_frame = 0;
    player.grounded = false;
    world.set_player_state_for_diagnostic(0, player);
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let walk = [
        PlayerInput::neutral().with_left_stick(64, 0),
        PlayerInput::neutral(),
    ];

    let mut frame = 0;
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
    frame += 1;

    assert_eq!(world.players()[0].motion_state, MotionState::Landing);
    assert_eq!(world.players()[0].motion_frame, 2);

    step_world(&mut world, Frame(frame), &neutral);
    frame += 1;

    assert_eq!(world.players()[0].motion_state, MotionState::Landing);
    assert_eq!(world.players()[0].motion_frame, 3);

    step_world(&mut world, Frame(frame), &walk);

    assert_eq!(world.players()[0].motion_state, MotionState::WalkSlow);
}

#[test]
fn ordinary_landing_neutral_exits_to_wait_on_animation_completion() {
    let profile = FighterProfile {
        normal_landing_lag_ticks: 2,
        ..FighterProfile::falcon_like()
    };
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let platform = world.stage().soft_platforms[0];
    let mut player = world.players()[0];
    player.position = Vec2 {
        x: (platform.left_x + platform.right_x) / 2,
        y: platform.y + 2_000,
    };
    player.velocity = Vec2 { x: 0, y: -1_000 };
    player.motion_state = MotionState::Fall;
    player.motion_frame = 0;
    player.grounded = false;
    world.set_player_state_for_diagnostic(0, player);
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    let mut frame = 0;
    while !world.players()[0].grounded && frame < 120 {
        step_world(&mut world, Frame(frame), &neutral);
        frame += 1;
    }

    assert!(world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::Landing);
    assert_eq!(world.players()[0].motion_frame, 0);

    for _ in 0..40 {
        step_world(&mut world, Frame(frame), &neutral);
        frame += 1;
        if world.players()[0].motion_state == MotionState::Wait {
            break;
        }
    }

    assert_eq!(world.players()[0].motion_state, MotionState::Wait);
    assert_eq!(world.players()[0].motion_frame, 0);
}

#[test]
fn ordinary_landing_walk_iasa_keeps_same_tick_walk_velocity() {
    let mut world = World::for_two_players();
    let floor = world.stage().main_floor;
    let mut player = world.players()[0];
    player.grounded = true;
    player.set_motion_state_alias(MotionState::Landing);
    player.motion_frame = player.profile.normal_landing_lag_ticks;
    player.motion_anim_frame_milli = i32::from(player.motion_frame) * 1_000;
    player.position = Vec2 { x: 0, y: floor.y };
    player.velocity = Vec2 { x: 0, y: 0 };
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(0),
        &[
            PlayerInput::neutral().with_left_stick(100, 0),
            PlayerInput::neutral(),
        ],
    );

    let walked = world.players()[0];
    assert_eq!(walked.motion_state, MotionState::WalkSlow);
    assert!(
        walked.velocity.x > 0,
        "ftCo_Landing_IASA -> ftCo_Walk_CheckInput should preserve same-tick Walk_Phys velocity"
    );
    assert!(
        walked.position.x > 0,
        "same-tick walk velocity should translate on the Landing IASA frame"
    );
}

#[test]
fn landing_iasa_walk_check_precedes_analog_shield_guard_entry() {
    let mut world = World::for_two_players();
    let floor = world.stage().main_floor;
    let mut player = world.players()[0];
    player.grounded = true;
    player.facing = -1;
    player.set_motion_state_alias(MotionState::Landing);
    player.motion_frame = player.profile.normal_landing_lag_ticks;
    player.motion_anim_frame_milli = i32::from(player.motion_frame) * 1_000;
    player.position = Vec2 { x: 0, y: floor.y };
    player.ground_velocity_x = milli_to_source_units(-364);
    player.source_self_velocity_x = milli_to_source_units(-364);
    player.velocity = Vec2 { x: -364, y: 0 };
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(0),
        &[
            PlayerInput::neutral()
                .with_left_stick(-95, 0)
                .with_left_trigger_analog(35),
            PlayerInput::neutral(),
        ],
    );

    let player = world.players()[0];
    assert_eq!(
        player.motion_state,
        MotionState::WalkMiddle,
        "ftCo_Landing_IASA has no guard check; after jump/dash/squat/turn it reaches ftCo_Walk_CheckInput before shield can enter GuardOn"
    );
    assert!(
        player.ground_velocity_x < milli_to_source_units(-364),
        "ftCo_Walk_CheckInput should apply walk acceleration instead of GuardOn traction on the landing IASA frame"
    );
}

#[test]
fn landing_iasa_full_analog_trigger_enters_guard_on_before_jump_dash_or_walk() {
    let mut world = World::for_two_players();
    let floor = world.stage().main_floor;
    let mut player = world.players()[0];
    player.grounded = true;
    player.facing = -1;
    player.set_motion_state_alias(MotionState::Landing);
    player.motion_frame = player.profile.normal_landing_lag_ticks;
    player.motion_anim_frame_milli = i32::from(player.motion_frame) * 1_000;
    player.position = Vec2 { x: 0, y: floor.y };
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(0),
        &[
            PlayerInput::neutral()
                .with_left_stick(-37, -105)
                .with_left_trigger_analog(135),
            PlayerInput::neutral(),
        ],
    );

    assert_eq!(
        world.players()[0].motion_state,
        MotionState::GuardOn,
        "ftCo_Landing_IASA calls ftCo_80091A4C before jump/dash/squat/turn/walk; a trigger past the analog-held threshold enters GuardOn"
    );
}

#[test]
fn walk_iasa_full_analog_trigger_enters_guard_on_and_runs_guard_physics() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.grounded = true;
    player.set_motion_state_alias(MotionState::WalkMiddle);
    player.position = Vec2 { x: 0, y: 0 };
    player.ground_velocity_x = milli_to_source_units(-410);
    player.source_self_velocity_x = milli_to_source_units(-410);
    player.velocity = Vec2 { x: -410, y: 0 };
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(0),
        &[
            PlayerInput::neutral()
                .with_left_stick(-89, 0)
                .with_left_trigger_analog(195),
            PlayerInput::neutral(),
        ],
    );

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::GuardOn);
    assert_eq!(
        player.velocity.x, -330,
        "ftCo_Walk_IASA can enter GuardOn through ftCo_80091A4C; the frame then runs GuardOn_Phys/ft_80084F3C"
    );
    assert_eq!(
        player.position.x, -330,
        "same-frame GuardOn physics should drive this tick's ground translation"
    );
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
    let escape_air_end = 8 + falcon_escape_air_action_frames();
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
fn fall_special_after_air_dodge_applies_source_air_drift() {
    let profile = FighterProfile {
        jump_horizontal_initial_velocity: 0.0,
        air_drift_stick_multiplier: 0.04,
        aerial_drift_base: 0.02,
        air_drift_max: 1.12,
        aerial_friction: 0.01,
        air_max_horizontal_velocity: 1.12,
        ..FighterProfile::falcon_like()
    };
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let up_air_dodge = [
        PlayerInput::neutral()
            .with_right_trigger_digital(true)
            .with_left_stick(0, 127),
        PlayerInput::neutral(),
    ];
    let drift_right = [
        PlayerInput::neutral().with_left_stick(127, 0),
        PlayerInput::neutral(),
    ];

    for frame in 0..4 {
        step_world(&mut world, Frame(frame), &jump);
    }
    for frame in 4..8 {
        step_world(&mut world, Frame(frame), &neutral);
    }
    step_world(&mut world, Frame(8), &up_air_dodge);
    let mut player = world.players()[0];
    player.position.y += melee_units_f32(200.0);
    assert!(world.set_player_state_for_diagnostic(0, player));

    let escape_air_end = 8 + falcon_escape_air_action_frames();
    for frame in 9..=escape_air_end {
        step_world(&mut world, Frame(frame), &neutral);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::FallSpecial);
    assert_eq!(world.players()[0].velocity.x, 0);

    step_world(&mut world, Frame(escape_air_end + 1), &drift_right);

    assert_eq!(world.players()[0].motion_state, MotionState::FallSpecial);
    assert_eq!(world.players()[0].velocity.x, 60);
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
fn aerial_jump_first_tick_applies_falcon_gravity_before_translation() {
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
    let expected_air_jump_velocity = source_units_to_milli(
        profile.jump_vertical_initial_velocity * profile.air_jump_vertical_multiplier
            - profile.gravity,
    );
    assert_eq!(
        world.players()[0].position.y - before_y,
        expected_air_jump_velocity
    );
    assert_eq!(world.players()[0].velocity.y, expected_air_jump_velocity);
}

#[test]
fn aerial_jump_animation_completion_enters_fall_aerial_not_base_fall() {
    let mut world = World::for_two_players();
    let mut player = PlayerState::new(0, 30_000, 1);
    player.grounded = false;
    player.motion_state = MotionState::JumpAerialF;
    player.motion_frame = 49;
    player.velocity.y = -100;
    player.jumps_remaining = 0;
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    assert_eq!(world.players()[0].motion_state, MotionState::FallAerial);
    assert_eq!(world.players()[0].motion_frame, 0);
    assert!(!world.players()[0].grounded);
}

#[test]
fn ground_jump_animation_completion_enters_base_fall() {
    for (motion_state, final_animation_frame) in
        [(MotionState::JumpF, 34), (MotionState::JumpB, 49)]
    {
        let mut world = World::for_two_players();
        let mut player = PlayerState::new(0, 30_000, 1);
        player.grounded = false;
        player.motion_state = motion_state;
        player.motion_frame = final_animation_frame;
        player.velocity.y = -100;
        assert!(world.set_player_state_for_diagnostic(0, player));

        step_world(
            &mut world,
            Frame(0),
            &[PlayerInput::neutral(), PlayerInput::neutral()],
        );

        assert_eq!(world.players()[0].motion_state, MotionState::Fall);
        assert_eq!(world.players()[0].motion_frame, 0);
        assert!(!world.players()[0].grounded);
    }
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
fn world_common_data_drives_aerial_jump_backward_threshold() {
    let common = MeleeCommonData {
        air_jump_backward_x: 100,
        ..MeleeCommonData::provisional_mole()
    };
    let mut world = World::for_two_players_with_common_data(common);
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let soft_back_double_jump = [
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
    step_world(&mut world, Frame(5), &soft_back_double_jump);

    assert_eq!(world.players()[0].motion_state, MotionState::JumpAerialF);
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
            .with_left_stick(30, -127),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &jump);
    step_world(&mut world, Frame(1), &jump);
    step_world(&mut world, Frame(2), &jump);
    step_world(&mut world, Frame(3), &jump);
    step_world(&mut world, Frame(4), &jump);
    step_world(&mut world, Frame(5), &down_air_dodge);
    for frame in 6..80 {
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
    assert_eq!(
        player.velocity.x, 0,
        "stick x 30 is inside Fighter_Spaghetti input cleanup, so this is a straight down air dodge"
    );
    assert_eq!(player.velocity.y, -2_511);
}

#[test]
fn escape_air_landing_carries_source_self_velocity_into_ground_velocity_like_ftcommon_8007d6a4() {
    let mut world = World::for_two_players();
    let floor_y = world.stage().main_floor.y;
    let common = world.common_data();
    let mut player = world.players()[0];
    let source_velocity_x = -2.049_657_106_f32;

    player.motion_state = MotionState::EscapeAir;
    player.set_motion_state_alias(MotionState::EscapeAir);
    player.motion_frame = 18;
    player.motion_cmd_var0 = 0;
    player.position = Vec2 {
        x: 0,
        y: floor_y + 500,
    };
    player.velocity = Vec2 {
        x: source_units_to_milli(source_velocity_x),
        y: -1_200,
    };
    player.source_self_velocity_x = source_velocity_x;
    player.source_self_velocity_y = -1.2;
    player.grounded = false;
    player.ecb_bottom_offset_y = 0;
    player.ecb_bottom_lock_timer = 2;
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(179),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    let landed = world.players()[0];
    let expected_ground_velocity = source_velocity_x * common.escapeair_decay;
    assert!(landed.grounded);
    assert_eq!(landed.motion_state, MotionState::LandingFallSpecial);
    assert_eq!(
        landed.ground_velocity_x.to_bits(),
        expected_ground_velocity.to_bits(),
        "ftCommon_8007D6A4 assigns gr_vel from self_vel.x without round-tripping through display milli-units"
    );
    assert_eq!(
        landed.source_self_velocity_x.to_bits(),
        expected_ground_velocity.to_bits()
    );
    assert_eq!(
        landed.velocity.x,
        source_units_to_milli(expected_ground_velocity)
    );
}

#[test]
fn guard_reflect_jump_squat_entry_applies_source_ground_friction_before_movement() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    let source_velocity_x = 0.051_875_f32;
    let start_position = Vec2 { x: 42_173, y: 0 };

    player.motion_state = MotionState::GuardReflect;
    player.set_motion_state_alias(MotionState::GuardReflect);
    player.motion_frame = 5;
    player.grounded = true;
    player.position = start_position;
    player.velocity.x = source_units_to_milli(source_velocity_x);
    player.ground_velocity_x = source_velocity_x;
    player.source_self_velocity_x = source_velocity_x;
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(1132),
        &[
            PlayerInput::neutral().with_shield(true).with_jump(true),
            PlayerInput::neutral(),
        ],
    );

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::KneeBend);
    assert_eq!(
        player.position, start_position,
        "ftCo_KneeBend_Phys calls ft_80084F3C, so tiny carried gr_vel is frictioned to zero before ftCommon_ApplyGroundMovement"
    );
    assert_eq!(player.ground_velocity_x.to_bits(), 0.0_f32.to_bits());
    assert_eq!(player.source_self_velocity_x.to_bits(), 0.0_f32.to_bits());
}

#[test]
fn fall_special_with_stick_above_x25c_lands_on_soft_platform() {
    let mut world = World::for_two_players();
    let stage = world.stage();
    let platform = stage.soft_platforms[0];
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let up_air_dodge = [
        PlayerInput::neutral()
            .with_right_trigger_digital(true)
            .with_left_stick(0, 127),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    for frame in 0..=4 {
        step_world(&mut world, Frame(frame), &jump);
    }
    let mut frame = 5;
    while world.players()[0].position.y <= platform.y && frame < 60 {
        step_world(&mut world, Frame(frame), &neutral);
        frame += 1;
    }
    assert!(world.players()[0].position.y > platform.y);
    step_world(&mut world, Frame(frame), &up_air_dodge);
    frame += 1;

    while world.players()[0].motion_state != MotionState::FallSpecial && frame < 80 {
        step_world(&mut world, Frame(frame), &neutral);
        frame += 1;
    }

    assert_eq!(world.players()[0].motion_state, MotionState::FallSpecial);

    while !world.players()[0].grounded && frame < 180 {
        step_world(&mut world, Frame(frame), &neutral);
        frame += 1;
    }

    assert!(world.players()[0].grounded);
    assert_eq!(
        world.players()[0].motion_state,
        MotionState::LandingFallSpecial
    );
    assert_eq!(world.players()[0].position.y, platform.y);
}

#[test]
fn fall_special_holding_down_skips_soft_platform_until_main_floor() {
    let mut world = World::for_two_players();
    let stage = world.stage();
    let platform = stage.soft_platforms[0];
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let up_air_dodge = [
        PlayerInput::neutral()
            .with_right_trigger_digital(true)
            .with_left_stick(0, 127),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let down = [
        PlayerInput::neutral().with_left_stick(
            0,
            MeleeCommonData::provisional_mole().fallspecial_platform_landing_y,
        ),
        PlayerInput::neutral(),
    ];

    for frame in 0..=4 {
        step_world(&mut world, Frame(frame), &jump);
    }
    let mut frame = 5;
    while world.players()[0].position.y <= platform.y && frame < 60 {
        step_world(&mut world, Frame(frame), &neutral);
        frame += 1;
    }
    assert!(world.players()[0].position.y > platform.y);
    step_world(&mut world, Frame(frame), &up_air_dodge);
    frame += 1;

    while world.players()[0].motion_state != MotionState::FallSpecial && frame < 80 {
        step_world(&mut world, Frame(frame), &neutral);
        frame += 1;
    }

    assert_eq!(world.players()[0].motion_state, MotionState::FallSpecial);

    while !world.players()[0].grounded && frame < 220 {
        step_world(&mut world, Frame(frame), &down);
        frame += 1;
        assert_ne!(world.players()[0].position.y, platform.y);
    }

    assert!(world.players()[0].grounded);
    assert_eq!(
        world.players()[0].motion_state,
        MotionState::LandingFallSpecial
    );
    assert_eq!(world.players()[0].position.y, stage.main_floor.y);
}

#[test]
fn fall_special_platform_gate_is_deterministic_from_input_snapshots() {
    let mut left = World::for_two_players();
    let mut right = World::for_two_players();
    let stage = left.stage();
    let platform = stage.soft_platforms[0];
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let up_air_dodge = [
        PlayerInput::neutral()
            .with_right_trigger_digital(true)
            .with_left_stick(0, 127),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let down = [
        PlayerInput::neutral().with_left_stick(
            0,
            MeleeCommonData::provisional_mole().fallspecial_platform_landing_y,
        ),
        PlayerInput::neutral(),
    ];

    for frame in 0..=4 {
        step_world(&mut left, Frame(frame), &jump);
        step_world(&mut right, Frame(frame), &jump);
        assert_eq!(left.checksum(), right.checksum());
    }

    let mut frame = 5;
    while left.players()[0].position.y <= platform.y && frame < 60 {
        step_world(&mut left, Frame(frame), &neutral);
        step_world(&mut right, Frame(frame), &neutral);
        assert_eq!(left.checksum(), right.checksum());
        frame += 1;
    }

    assert!(left.players()[0].position.y > platform.y);
    step_world(&mut left, Frame(frame), &up_air_dodge);
    step_world(&mut right, Frame(frame), &up_air_dodge);
    assert_eq!(left.checksum(), right.checksum());
    frame += 1;

    while left.players()[0].motion_state != MotionState::FallSpecial && frame < 80 {
        step_world(&mut left, Frame(frame), &neutral);
        step_world(&mut right, Frame(frame), &neutral);
        assert_eq!(left.checksum(), right.checksum());
        frame += 1;
    }

    while !left.players()[0].grounded && frame < 220 {
        step_world(&mut left, Frame(frame), &down);
        step_world(&mut right, Frame(frame), &down);
        assert_eq!(left.checksum(), right.checksum());
        frame += 1;
    }

    assert!(left.players()[0].grounded);
    assert_eq!(left.players()[0].position.y, stage.main_floor.y);
    assert_eq!(left.snapshot(), right.snapshot());
}

#[test]
fn air_dodge_landing_uses_existing_melee_states_not_wavedash_state() {
    let mut world = World::for_two_players();
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let diagonal_air_dodge = [
        PlayerInput::neutral()
            .with_right_trigger_digital(true)
            .with_left_stick(80, -127),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    for frame in 0..=4 {
        step_world(&mut world, Frame(frame), &jump);
    }
    step_world(&mut world, Frame(5), &diagonal_air_dodge);

    let mut frame = 6;
    while !world.players()[0].grounded && frame < 80 {
        step_world(&mut world, Frame(frame), &neutral);
        assert_ne!(format!("{:?}", world.players()[0].motion_state), "Wavedash");
        frame += 1;
    }

    assert!(world.players()[0].grounded);
    assert_eq!(
        world.players()[0].motion_state,
        MotionState::LandingFallSpecial
    );
    assert_eq!(world.players()[0].position.y, world.stage().main_floor.y);
    assert_ne!(format!("{:?}", world.players()[0].motion_state), "Wavedash");

    assert_eq!(
        world.players()[0].motion_state,
        MotionState::LandingFallSpecial
    );
    assert_ne!(format!("{:?}", world.players()[0].motion_state), "Wavedash");
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
    step_world(&mut world, Frame(4), &jump);
    step_world(&mut world, Frame(5), &down_air_dodge);
    let mut frame = 6;
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
fn landing_fall_special_completion_runs_wait_followup_on_same_frame() {
    let mut world = World::for_two_players();
    let top_platform = world.stage().soft_platforms[2];
    let mut player = world.players()[0];
    player.motion_state = MotionState::LandingFallSpecial;
    player.motion_frame = world.common_data().escapeair_landing_lag_ticks - 1;
    player.position = Vec2 {
        x: melee_units_f32(16.909),
        y: top_platform.y,
    };
    player.velocity = Vec2 {
        x: melee_units_f32(0.319),
        y: 0,
    };
    player.grounded = true;
    player.facing = -1;
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(141),
        &[
            PlayerInput::neutral().with_left_stick(119, 41),
            PlayerInput::neutral(),
        ],
    );

    assert_eq!(world.players()[0].motion_state, MotionState::Turn);
    assert_eq!(world.players()[0].position.y, top_platform.y);
}

#[test]
fn landing_fall_special_completion_uses_stored_source_landing_lag() {
    let mut world = World::for_two_players();
    let common = world.common_data();
    let mut player = world.players()[0];
    player.motion_state = MotionState::LandingFallSpecial;
    player.motion_frame = common.escapeair_landing_lag_ticks - 1;
    player.landing_lag_ticks = common.escapeair_landing_lag_ticks + 2;
    player.grounded = true;
    player.velocity.y = 0;
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    assert_eq!(
        world.players()[0].motion_state,
        MotionState::LandingFallSpecial
    );

    let mut player = world.players()[0];
    player.motion_frame = player.landing_lag_ticks - 1;
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(1),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    assert_eq!(world.players()[0].motion_state, MotionState::Wait);
}

#[test]
fn landing_fall_special_slide_uses_falcon_ground_friction_as_fixed_deceleration() {
    let mut world = World::for_two_players();
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let down_forward_air_dodge = [
        PlayerInput::neutral()
            .with_right_trigger_digital(true)
            .with_left_stick(80, -127),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    for frame in 0..=4 {
        step_world(&mut world, Frame(frame), &jump);
    }
    step_world(&mut world, Frame(5), &down_forward_air_dodge);

    let mut frame = 6;
    while !world.players()[0].grounded && frame < 80 {
        step_world(&mut world, Frame(frame), &neutral);
        frame += 1;
    }

    let player = world.players()[0];
    assert!(player.grounded);
    assert_eq!(player.motion_state, MotionState::LandingFallSpecial);
    assert!(player.velocity.x > source_units_to_milli(player.profile.ground_friction));
    assert!(player.velocity.x > source_units_to_milli(player.profile.walk_max_velocity));

    let landing_velocity = player.velocity.x;
    let common = world.common_data();
    let expected_friction = source_units_to_milli(
        player.profile.ground_friction * common.high_speed_ground_friction_multiplier,
    );
    let expected_after_one_slide_tick = landing_velocity - expected_friction;
    step_world(&mut world, Frame(frame), &neutral);

    assert_eq!(
        world.players()[0].motion_state,
        MotionState::LandingFallSpecial
    );
    assert_eq!(world.players()[0].velocity.x, expected_after_one_slide_tick);
}

#[test]
fn landing_fall_special_sliding_at_floor_edge_keeps_source_support() {
    let mut stage = StageProfile::battlefield_test();
    stage.main_floor = StageSurface {
        name: "narrow_main_floor",
        kind: StageSurfaceKind::Solid,
        left_x: melee_units_f32(-20.5),
        right_x: melee_units_f32(-18.0),
        y: 0,
        friction_multiplier: 1.0,
    };
    let mut world = World::for_two_players_on_stage(stage);
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    let mut player = world.players()[0];
    player.position.x = world.stage().main_floor.right_x - melee_units_f32(0.1);
    player.position.y = world.stage().main_floor.y;
    player.facing = -1;
    player.motion_state = MotionState::LandingFallSpecial;
    player.motion_frame = 0;
    player.grounded = true;
    player.velocity.x = melee_units_f32(1.6);
    player.ground_velocity_x = milli_to_source_units(player.velocity.x);
    player.velocity.y = 0;
    assert!(world.set_player_state_for_diagnostic(0, player));

    assert_eq!(
        world.players()[0].motion_state,
        MotionState::LandingFallSpecial
    );

    let mut frame = 0;
    while world.players()[0].motion_state == MotionState::LandingFallSpecial && frame < 20 {
        step_world(&mut world, Frame(frame), &neutral);
        frame += 1;
    }

    assert!(world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::Wait);
}

#[test]
fn grounded_dash_moving_past_floor_edge_keeps_source_floor_for_the_tick() {
    let mut stage = StageProfile::battlefield_test();
    stage.main_floor = StageSurface {
        name: "narrow_main_floor",
        kind: StageSurfaceKind::Solid,
        left_x: melee_units_f32(-20.5),
        right_x: melee_units_f32(-19.0),
        y: 0,
        friction_multiplier: 1.0,
    };
    let mut world = World::for_two_players_on_stage(stage);
    let dash_right = [
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);

    assert!(world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::Dash);

    step_world(&mut world, Frame(1), &dash_right);

    assert!(world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    assert!(world.players()[0].position.x > stage.main_floor.right_x);
}

#[test]
fn turn_run_floor_edge_collision_zeros_ground_velocity_like_source() {
    let stage = StageProfile::battlefield();
    let mut world = World::for_two_players_on_stage(stage);
    let mut player = world.players()[0];
    player.motion_state = MotionState::TurnRun;
    player.motion_frame = 4;
    player.motion_anim_frame_milli = 4_000;
    player.position.x = stage.main_floor.right_x - 100;
    player.position.y = stage.main_floor.y;
    player.facing = 1;
    player.turn_run_accel_mul = 1;
    player.grounded = true;
    player.ground_velocity_x = 1.0;
    player.source_position = SourceVec2::from_milli(player.position);
    player.source_self_velocity_x = player.ground_velocity_x;
    player.velocity.x = source_units_to_milli(1.0);
    player.set_source_floor_for_diagnostic(
        Some(0),
        Some(source_floor_line_for_surface_at_x(
            stage,
            stage.main_floor,
            player.source_position.x,
        )),
    );
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(1),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    assert!(world.players()[0].grounded);
    assert_ne!(
        world.snapshot().players[0].source_coll_env_flags & (0x100000 | 0x200000),
        0,
        "ft_800827A0 owns the source edge flag before ftCommon_8007E2FC clears velocity"
    );
    assert_eq!(
        world.players()[0].ground_velocity_x,
        0.0,
        "ftCo_TurnRun_Coll calls ftCommon_8007E2FC when Collide_RightEdge is set"
    );
    assert_eq!(world.players()[0].velocity.x, 0);
}

#[test]
fn turn_run_collision_uses_source_floor_callback_before_generic_ledge_slip_gate() {
    let mut world = World::for_two_players();
    let stage = world.stage();
    let right_platform = stage.soft_platforms[1];
    let mut player = world.players()[1];
    player.facing = -1;
    player.set_motion_state_alias(MotionState::TurnRun);
    player.motion_frame = 9;
    player.motion_anim_frame_milli = 9_000;
    player.grounded = true;
    player.position = Vec2 {
        x: right_platform.left_x + 100,
        y: right_platform.y,
    };
    player.source_position = SourceVec2::from_milli(player.position);
    player.set_source_floor_for_diagnostic(
        Some(2),
        Some(source_floor_line_for_surface_at_x(
            stage,
            right_platform,
            player.source_position.x,
        )),
    );
    player.ground_velocity_x = -0.35;
    player.source_self_velocity_x = player.ground_velocity_x;
    player.velocity.x = source_units_to_milli(player.ground_velocity_x);
    player.velocity.y = 0;
    player.turn_run_accel_mul = -1;
    assert!(world.set_player_state_for_diagnostic(1, player));

    step_world(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    let player = world.players()[1];
    assert!(player.grounded);
    assert_eq!(player.motion_state, MotionState::TurnRun);
    assert_ne!(
        world.snapshot().players[1].source_coll_env_flags & (0x100000 | 0x200000),
        0,
        "ftCo_TurnRun_Coll routes through ft_800827A0 before any generic ledge-slip stick gate"
    );
    assert_eq!(
        player.ground_velocity_x, 0.0,
        "ftCommon_8007E2FC clears TurnRun ground velocity after the source edge flag is set"
    );
    assert_eq!(player.velocity.x, 0);
}

#[test]
fn escape_roll_floor_edge_collision_enters_fall_when_live_ecb_loses_support() {
    let mut world = World::for_two_players();
    let right_platform = world.stage().soft_platforms[1];
    let mut player = world.players()[1];
    player.facing = -1;
    player.set_motion_state_alias(MotionState::EscapeF);
    player.motion_frame = 22;
    player.facing = 1;
    player.source_motion_entry_facing = -1;
    player.motion_state_alias = None;
    player.grounded = true;
    player.position = Vec2 {
        x: right_platform.left_x + 100,
        y: right_platform.y,
    };
    player.source_position.x = milli_to_source_units(player.position.x);
    player.source_position.y = milli_to_source_units(player.position.y);
    player.ground_velocity_x = -0.35;
    player.velocity.x = source_units_to_milli(player.ground_velocity_x);
    player.velocity.y = 0;
    assert!(world.set_player_state_for_diagnostic(1, player));

    step_world(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    let player = world.players()[1];
    assert!(!player.grounded);
    assert_eq!(player.motion_state, MotionState::Fall);
    assert_eq!(
        player.position.x, 19_812,
        "ftCo_Escape_Coll routes through the source ground-to-air callback once the live JObj ECB no longer has floor support"
    );
    assert_eq!(
        player.position.y, right_platform.y,
        "the ground-to-air handoff preserves the last supported floor height on the transition frame"
    );
    assert!(
        player.velocity.x < 0,
        "the ground-to-air handoff preserves EscapeF TransN velocity for Fall"
    );
}

#[test]
fn escape_roll_floor_edge_uses_source_floor_projection_tolerance_before_endpoint_clamp() {
    let mut world = World::for_two_players();
    let right_platform = world.stage().soft_platforms[1];
    let mut player = world.players()[1];
    player.facing = 1;
    player.set_motion_state_alias(MotionState::EscapeF);
    player.source_motion_entry_facing = -1;
    player.motion_state_alias = None;
    player.motion_frame = 26;
    player.grounded = true;
    player.position = Vec2 {
        x: right_platform.left_x,
        y: right_platform.y,
    };
    player.source_position.x = milli_to_source_units(player.position.x);
    player.source_position.y = milli_to_source_units(player.position.y);
    player.ground_velocity_x = -0.13724499940872192;
    player.source_self_velocity_x = player.ground_velocity_x;
    player.velocity.x = source_units_to_milli(player.ground_velocity_x);
    player.velocity.y = 0;
    assert!(world.set_player_state_for_diagnostic(1, player));

    step_world(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    let player = world.players()[1];
    assert!(player.grounded);
    assert_eq!(player.motion_state, MotionState::EscapeF);
    assert_eq!(
        player.position.x, 19_901,
        "ft_800827A0 calls mpColl_8004B2DC; mpColl_800488F4/mpLib_8004DD90_Floor keeps X unsnapped while bottom.x is within 0.1 source units of the floor endpoint"
    );
    assert_eq!(player.position.y, right_platform.y);
}

#[test]
fn escape_roll_source_floor_slip_preserves_root_past_unconnected_platform_endpoint() {
    let mut world = World::for_two_players();
    let right_platform = world.stage().soft_platforms[1];
    let mut player = world.players()[1];
    player.facing = 1;
    player.set_motion_state_alias(MotionState::EscapeF);
    player.source_motion_entry_facing = -1;
    player.motion_state_alias = None;
    player.motion_frame = 26;
    player.motion_anim_frame_milli = 26_000;
    player.grounded = true;
    player.position = Vec2 {
        x: right_platform.left_x,
        y: right_platform.y,
    };
    player.source_position.x = milli_to_source_units(player.position.x);
    player.source_position.y = milli_to_source_units(player.position.y);
    player.ground_velocity_x = -0.099055;
    player.source_self_velocity_x = player.ground_velocity_x;
    player.velocity.x = source_units_to_milli(player.ground_velocity_x);
    player.velocity.y = 0;
    assert!(world.set_player_state_for_diagnostic(1, player));

    step_world(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    let player = world.players()[1];
    assert!(player.grounded);
    assert_eq!(player.motion_state, MotionState::EscapeF);
    assert_eq!(
        player.position.x, 19_901,
        "mpColl_8004B2DC should keep EscapeF grounded through the source floor/ledge-slip path without forcing the platform endpoint clamp"
    );
    assert_eq!(player.position.y, right_platform.y);
}

#[test]
fn shield_jump_digital_trigger_on_takeoff_frame_can_airdodge_and_land() {
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
    step_world(&mut world, Frame(4), &shield_jump);
    step_world(&mut world, Frame(5), &shield_jump_with_other_digital);

    assert!(world.players()[0].grounded);
    assert_eq!(
        world.players()[0].motion_state,
        MotionState::LandingFallSpecial
    );
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
            .with_left_stick(80, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &shield);
    step_world(&mut world, Frame(1), &shield_jump);
    step_world(&mut world, Frame(2), &shield_jump);
    step_world(&mut world, Frame(3), &shield_jump);
    step_world(&mut world, Frame(4), &shield_jump);
    step_world(&mut world, Frame(5), &shield_jump);
    step_world(&mut world, Frame(6), &air_dodge);

    assert!(!world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::EscapeAir);
    assert!(world.players()[0].velocity.x > 0);
    assert_eq!(world.players()[0].velocity.y, 0);
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
            .with_left_stick(80, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &shield);
    step_world(&mut world, Frame(1), &shield_jump);
    step_world(&mut world, Frame(2), &shield_jump);
    step_world(&mut world, Frame(3), &shield_jump);
    step_world(&mut world, Frame(4), &shield_jump);
    step_world(&mut world, Frame(5), &shield_jump);
    step_world(&mut world, Frame(6), &bottomed_left_air_dodge);

    assert!(!world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::EscapeAir);
    assert!(world.players()[0].velocity.x > 0);
    assert_eq!(world.players()[0].velocity.y, 0);
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
            .with_left_stick(80, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &shield);
    step_world(&mut world, Frame(1), &shield_jump);
    step_world(&mut world, Frame(2), &shield_jump);
    step_world(&mut world, Frame(3), &shield_jump);
    step_world(&mut world, Frame(4), &shield_jump);
    step_world(&mut world, Frame(5), &shield_jump);
    step_world(&mut world, Frame(6), &attack_air_dodge);

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
    step_world(&mut world, Frame(5), &shield_jump);
    step_world(&mut world, Frame(6), &side_b_air_dodge);

    assert!(!world.players()[0].grounded);
    assert_eq!(
        world.players()[0].motion_state,
        MotionState::SpecialAirSStart
    );
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
            .with_left_stick(-dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];
    let neutral_special = [
        PlayerInput::neutral().with_special(true),
        PlayerInput::neutral(),
    ];
    let diagonal_up_special = [
        PlayerInput::neutral()
            .with_special(true)
            .with_left_stick(-dash_stick_x(), 90),
        PlayerInput::neutral(),
    ];
    let diagonal_down_special = [
        PlayerInput::neutral()
            .with_special(true)
            .with_left_stick(-dash_stick_x(), -90),
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
    assert_eq!(
        side.players()[0].motion_state,
        MotionState::SpecialAirSStart
    );
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
fn falcon_air_up_special_entry_runs_source_specialhi_init_callback() {
    let mut world = World::for_two_players();
    let up_special = [
        PlayerInput::neutral()
            .with_special(true)
            .with_left_stick(0, 90),
        PlayerInput::neutral(),
    ];

    advance_to_air(&mut world);

    step_world(&mut world, Frame(4), &up_special);

    let player = &world.players()[0];
    assert_eq!(player.motion_state, MotionState::SpecialAirHi);
    assert_eq!(player.jumps_remaining, 0);
    assert_eq!(player.motion_cmd_var0, 0);
    assert_eq!(
        player.motion_cmd_var1,
        player.profile.captain_special_attrs.specialhi_unk2 as u32
    );
    assert_eq!(
        player.captain_special_hi_x0,
        player.profile.captain_special_attrs.specialhi_air_var as u16
    );
    assert_eq!(player.captain_special_hi_vel_x.to_bits(), 0.0_f32.to_bits());
    assert_eq!(player.captain_special_hi_vel_y.to_bits(), 0.0_f32.to_bits());
    assert!(!player.captain_special_hi_x2_b0);
    assert!(!player.captain_special_hi_x2_b1);
}

#[test]
fn falcon_air_up_special_physics_uses_source_specialhi_velocity_and_transn() {
    let mut world = World::for_two_players();
    let up_special = [
        PlayerInput::neutral()
            .with_special(true)
            .with_left_stick(0, 90),
        PlayerInput::neutral(),
    ];
    let drift_right = [
        PlayerInput::neutral().with_left_stick(64, 0),
        PlayerInput::neutral(),
    ];

    advance_to_air(&mut world);
    step_world(&mut world, Frame(4), &up_special);
    step_world(&mut world, Frame(5), &drift_right);

    let player = &world.players()[0];
    let common = world.common_data();
    let attrs = player.profile.captain_special_attrs;
    let stick = fighter_stick_axis_to_f32(64);
    let accel =
        stick * player.profile.air_drift_stick_multiplier * attrs.specialhi_air_friction_mul;
    let target_vel = stick * player.profile.air_drift_max * attrs.specialhi_horz_vel;
    let expected_specialhi_vel_x = accel.min(target_vel);
    let source_frame = player.motion_frame.saturating_add(2);
    let (root_x, root_y) = source_root_motion_delta(player.motion_state, source_frame)
        .map(|delta| {
            (
                delta.z * player.profile.model_scaling * f32::from(player.facing),
                delta.y * player.profile.model_scaling,
            )
        })
        .unwrap_or((0.0, 0.0));

    assert_eq!(player.motion_state, MotionState::SpecialAirHi);
    assert!(stick.abs() >= common.special_air_drift_stick_threshold);
    assert_eq!(
        player.captain_special_hi_vel_x.to_bits(),
        expected_specialhi_vel_x.to_bits()
    );
    assert_eq!(player.captain_special_hi_vel_y.to_bits(), 0.0_f32.to_bits());
    assert_eq!(
        player.source_self_velocity_x.to_bits(),
        (root_x + expected_specialhi_vel_x).to_bits()
    );
    assert_eq!(player.source_self_velocity_y.to_bits(), root_y.to_bits());
}

#[test]
fn falcon_air_up_special_entry_from_damage_fall_uses_new_specialhi_physics_same_tick() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    let start_position = Vec2 {
        x: 82_130,
        y: -56_733,
    };
    player.grounded = false;
    player.motion_state_alias = None;
    player.motion_state = MotionState::DamageFall;
    player.melee_action_state_id = Some(MeleeActionStateId::new(38));
    player.source_action_key = Some(SourceActionKey::new("DamageFall"));
    player.source_action_total_frames = 0;
    player.motion_frame = 23;
    player.set_source_motion_anim_frame(23.0);
    player.position = start_position;
    player.source_position = SourceVec2::from_milli(start_position);
    player.source_self_velocity_x = -0.896_000_1;
    player.source_self_velocity_y = -2.9;
    player.velocity = Vec2 {
        x: source_units_to_milli(player.source_self_velocity_x),
        y: source_units_to_milli(player.source_self_velocity_y),
    };
    assert!(world.set_player_state_for_diagnostic(0, player));

    let up_special = [
        PlayerInput::neutral()
            .with_special(true)
            .with_left_stick(65, 108),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &up_special);

    let player = world.players()[0];
    let attrs = player.profile.captain_special_attrs;
    let stick = fighter_stick_axis_to_f32(65);
    let expected_specialhi_vel_x =
        stick * player.profile.air_drift_stick_multiplier * attrs.specialhi_air_friction_mul;
    let first_root = source_root_motion_delta(MotionState::SpecialAirHi, 2)
        .expect("SpecialAirHi frame 2 TransN delta should be extracted");
    let first_root_x = first_root.z * player.profile.model_scaling * f32::from(player.facing);
    let first_root_y = first_root.y * player.profile.model_scaling;
    let expected_self_x = first_root_x + expected_specialhi_vel_x;

    assert_eq!(player.motion_state, MotionState::SpecialAirHi);
    assert_eq!(player.motion_frame, 0);
    assert_eq!(
        player.source_self_velocity_x.to_bits(),
        expected_self_x.to_bits()
    );
    assert_eq!(
        player.source_self_velocity_y.to_bits(),
        first_root_y.to_bits()
    );
    assert_eq!(
        player.source_position.x.to_bits(),
        (SourceVec2::from_milli(start_position).x + expected_self_x).to_bits(),
        "ftCa_SpecialAirHi_Enter installs the SpecialAirHi callbacks before Fighter_procUpdate composes position; stale DamageFall self_vel.x must not translate this row"
    );
    assert_eq!(
        player.source_position.y.to_bits(),
        (SourceVec2::from_milli(start_position).y + first_root_y).to_bits(),
        "the entry tick should use SpecialHi TransN/self velocity, not the old DamageFall fall speed"
    );
}

#[test]
fn falcon_air_up_special_source_cmd_var_turns_before_same_tick_physics() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.grounded = false;
    player.motion_state_alias = None;
    player.motion_state = MotionState::SpecialAirHi;
    player.melee_action_state_id = Some(MeleeActionStateId::new(354));
    player.source_action_key = Some(SourceActionKey::new("SpecialAirHi"));
    player.source_action_total_frames = 65;
    player.motion_frame = 11;
    player.set_source_motion_anim_frame(11.0);
    player.facing = -1;
    player.source_motion_entry_facing = -1;
    player.captain_special_hi_vel_x = 0.342_423_47;
    player.captain_special_hi_vel_y = 0.0;
    player.captain_special_hi_x2_b1 = false;
    assert!(world.set_player_state_for_diagnostic(0, player));

    let inputs = [
        PlayerInput::neutral().with_left_stick(75, 102),
        PlayerInput::neutral(),
    ];
    let source_pose_metadata = |query: &PlayerState| {
        if query.source_action_key == Some(SourceActionKey::new("SpecialAirHi"))
            && query.source_motion_anim_frame.to_bits() == 13.0_f32.to_bits()
        {
            Some(SourceActionPoseMetadata {
                script_events: SourceActionScriptEvents::single(
                    SourceActionScriptEvent::SetCmdVar {
                        cmd_var: 0,
                        value: 1,
                    },
                ),
                ..SourceActionPoseMetadata::default()
            })
        } else {
            Some(SourceActionPoseMetadata::default())
        }
    };

    step_world_with_source_runtime_data(
        &mut world,
        Frame(2799),
        &inputs,
        source_pose_metadata,
        |action_state_id| match action_state_id.get() {
            354 => Some(65),
            _ => None,
        },
    );

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::SpecialAirHi);
    assert_eq!(player.motion_frame, 12);
    assert_eq!(player.facing, 1);
    assert_eq!(player.motion_cmd_var0, 0);
    assert!(player.captain_special_hi_x2_b1);
    let source_frame = player.motion_frame.saturating_add(2);
    let root = source_root_motion_delta(player.motion_state, source_frame)
        .expect("SpecialAirHi TransN root delta should be extracted");
    let expected_root_x = root.z * player.profile.model_scaling * f32::from(player.facing);
    assert!(expected_root_x > 0.0);
    assert_eq!(
        player.source_self_velocity_x.to_bits(),
        (expected_root_x + player.captain_special_hi_vel_x).to_bits()
    );
}

#[test]
fn falcon_air_up_special_anim_end_enters_fallspecial_before_same_row_physics() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    let start_position = SourceVec2 {
        x: 56.753,
        y: -72.205,
    };
    player.grounded = false;
    player.motion_state_alias = None;
    player.motion_state = MotionState::SpecialAirHi;
    player.melee_action_state_id = Some(MeleeActionStateId::new(354));
    player.source_action_key = Some(SourceActionKey::new("SpecialAirHi"));
    player.source_action_total_frames = 65;
    player.motion_frame = 63;
    player.set_source_motion_anim_frame(63.0);
    player.source_position = start_position;
    player.position = start_position.to_milli();
    player.source_self_velocity_x = -0.385;
    player.source_self_velocity_y = -1.395;
    player.velocity = Vec2 {
        x: source_units_to_milli(player.source_self_velocity_x),
        y: source_units_to_milli(player.source_self_velocity_y),
    };
    player.captain_special_hi_vel_x = player.source_self_velocity_x;
    player.captain_special_hi_vel_y = 0.0;
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(2936),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    let player = world.players()[0];
    let expected_self_x = -0.375;
    let expected_self_y = -1.525;
    assert_eq!(
        player.motion_state,
        MotionState::FallSpecial,
        "ftCa_SpecialAirHi_Anim calls ftCo_80096900 as soon as ftAnim_IsFramesRemaining returns false"
    );
    assert_eq!(player.motion_frame, 0);
    assert_eq!(
        player.landing_lag_ticks,
        player.profile.captain_special_attrs.specialhi_landing_lag as u8
    );
    assert!((player.source_self_velocity_x - expected_self_x).abs() <= 0.000_001);
    assert!((player.source_self_velocity_y - expected_self_y).abs() <= 0.000_001);
    let expected_position_x = start_position.x + expected_self_x;
    let expected_position_y = start_position.y + expected_self_y;
    assert!(
        (player.source_position.x - expected_position_x).abs() <= 0.000_01,
        "actual x {:.9}, expected {:.9}",
        player.source_position.x,
        expected_position_x
    );
    assert!(
        (player.source_position.y - expected_position_y).abs() <= 0.000_01,
        "actual y {:.9}, expected {:.9}",
        player.source_position.y,
        expected_position_y
    );
}

#[test]
fn falcon_air_up_special_x2_b1_can_cliff_catch_battlefield_right_ledge() {
    let stage = StageProfile::battlefield();
    let mut world = World::for_two_players_on_stage(stage);
    let right_ledge = stage.ledges[1];
    assert_eq!(right_ledge.side, StageLedgeSide::Right);
    let mut player = world.players()[0];
    player.grounded = false;
    player.motion_state_alias = None;
    player.motion_state = MotionState::SpecialAirHi;
    player.melee_action_state_id = Some(MeleeActionStateId::new(354));
    player.source_action_key = Some(SourceActionKey::new("SpecialAirHi"));
    player.source_action_total_frames = 65;
    player.motion_frame = 43;
    player.set_source_motion_anim_frame(43.0);
    player.facing = 1;
    player.source_motion_entry_facing = -1;
    player.position = Vec2 {
        x: 78_880,
        y: -19_104,
    };
    player.source_position = SourceVec2 {
        x: 78.880_47,
        y: -19.104_21,
    };
    player.source_self_velocity_x = -0.536_614_8;
    player.source_self_velocity_y = 0.018_083_153;
    player.velocity = Vec2 {
        x: source_units_to_milli(player.source_self_velocity_x),
        y: source_units_to_milli(player.source_self_velocity_y),
    };
    player.captain_special_hi_x2_b1 = true;
    player.captain_special_hi_vel_x = -0.519_375_f32;
    player.captain_special_hi_vel_y = 0.0;
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(2831),
        &[
            PlayerInput::neutral().with_left_stick(-125, 0),
            PlayerInput::neutral(),
        ],
    );

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::CliffCatch);
    assert_eq!(
        player.melee_action_state_id,
        Some(MeleeActionStateId::new(252))
    );
    assert_eq!(
        player.source_action_key,
        Some(SourceActionKey::new("CliffCatch"))
    );
    assert_eq!(player.facing, -1);
    assert_eq!(player.source_cliff_ledge_id, Some(right_ledge.index));
    assert_eq!(
        player.position,
        source_scaled_cliff_position_from_transn(
            right_ledge.x_milli,
            right_ledge.y_milli,
            player.facing,
            2,
            player.profile
        )
    );
}

#[test]
fn falcon_ground_up_special_uses_source_specialhi_physics_without_spending_gr_vel() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    let source_ground_velocity = 0.5559999942779541_f32;
    let start_position = Vec2 {
        x: -56_738,
        y: world.stage().main_floor.y,
    };
    player.set_motion_state_alias(MotionState::Wait);
    player.motion_frame = 2;
    player.grounded = true;
    player.position = start_position;
    player.source_position = mole_core::SourceVec2::from_milli(start_position);
    player.ground_velocity_x = source_ground_velocity;
    player.source_self_velocity_x = source_ground_velocity;
    player.velocity.x = source_units_to_milli(source_ground_velocity);
    player.velocity.y = 0;
    assert!(world.set_player_state_for_diagnostic(0, player));

    let up_special = [
        PlayerInput::neutral()
            .with_special(true)
            .with_jump_secondary(true)
            .with_left_stick(0, 124),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &up_special);

    let player = world.players()[0];
    let first_root = source_root_motion_delta(MotionState::SpecialHi, 2)
        .expect("SpecialHi frame 2 TransN delta should be extracted");
    let first_root_x = first_root.z * player.profile.model_scaling;
    let first_root_y = first_root.y * player.profile.model_scaling;
    assert_eq!(player.motion_state, MotionState::SpecialHi);
    assert_eq!(player.motion_frame, 0);
    assert_eq!(
        player.ground_velocity_x.to_bits(),
        source_ground_velocity.to_bits(),
        "ftCa_SpecialHi_Enter initializes move state but does not clear fp->gr_vel"
    );
    assert_eq!(
        player.source_self_velocity_x.to_bits(),
        first_root_x.to_bits()
    );
    assert_eq!(
        player.source_self_velocity_y.to_bits(),
        first_root_y.to_bits()
    );
    assert_eq!(
        player.source_position.x.to_bits(),
        (milli_to_source_units(start_position.x) + first_root_x).to_bits(),
        "grounded SpecialHi movement is driven by ftCa_SpecialHi_Phys/self_vel, not stale gr_vel"
    );
    assert_eq!(
        player.source_position.y.to_bits(),
        (milli_to_source_units(start_position.y) + first_root_y).to_bits(),
        "ftCa_SpecialHi_Phys drives grounded SpecialHi vertical TransN displacement through ft_80085134"
    );

    step_world(&mut world, Frame(1), &up_special);

    let player = world.players()[0];
    let second_root = source_root_motion_delta(MotionState::SpecialHi, 3)
        .expect("SpecialHi frame 3 TransN delta should be extracted");
    let second_root_x = second_root.z * player.profile.model_scaling;
    let second_root_y = second_root.y * player.profile.model_scaling;
    assert_eq!(player.motion_state, MotionState::SpecialHi);
    assert_eq!(player.motion_frame, 1);
    assert_eq!(
        player.ground_velocity_x.to_bits(),
        source_ground_velocity.to_bits()
    );
    assert_eq!(
        player.source_self_velocity_x.to_bits(),
        second_root_x.to_bits()
    );
    assert_eq!(
        player.source_self_velocity_y.to_bits(),
        second_root_y.to_bits()
    );
    assert_eq!(
        player.source_position.x.to_bits(),
        (milli_to_source_units(start_position.x) + first_root_x + second_root_x).to_bits()
    );
    assert_eq!(
        player.source_position.y.to_bits(),
        (milli_to_source_units(start_position.y) + first_root_y + second_root_y).to_bits()
    );
}

#[test]
fn falcon_ground_up_special_frame13_falls_through_source_ground_to_air_callback() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    let source_ground_velocity = 0.5559999942779541_f32;
    let start_position = Vec2 {
        x: -55_201,
        y: world.stage().main_floor.y,
    };
    player.set_motion_state_alias(MotionState::SpecialHi);
    player.motion_frame = 12;
    player.grounded = true;
    player.position = start_position;
    player.source_position = mole_core::SourceVec2::from_milli(start_position);
    player.ground_velocity_x = source_ground_velocity;
    player.source_self_velocity_x = source_ground_velocity;
    player.velocity.x = source_units_to_milli(source_ground_velocity);
    player.velocity.y = 0;
    player.captain_special_hi_vel_x = 0.0;
    player.captain_special_hi_vel_y = 0.0;
    assert!(world.set_player_state_for_diagnostic(0, player));

    let up_special = [
        PlayerInput::neutral()
            .with_special(true)
            .with_jump_secondary(true)
            .with_left_stick(76, 100),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &up_special);

    let player = world.players()[0];
    let root = source_root_motion_delta(MotionState::SpecialHi, 15)
        .expect("SpecialHi frame 15 TransN delta should be extracted");
    let expected_root_x = root.z * player.profile.model_scaling;
    let expected_root_y = root.y * player.profile.model_scaling;
    let attrs = player.profile.captain_special_attrs;
    let stick = fighter_stick_axis_to_f32(76);
    let expected_specialhi_vel_x =
        stick * player.profile.air_drift_stick_multiplier * attrs.specialhi_air_friction_mul;

    assert_eq!(player.motion_state, MotionState::SpecialHi);
    assert_eq!(player.motion_frame, 13);
    assert!(!player.grounded);
    assert_eq!(player.ground_velocity_x.to_bits(), 0.0_f32.to_bits());
    assert_eq!(
        player.source_self_velocity_x.to_bits(),
        (expected_root_x + expected_specialhi_vel_x).to_bits()
    );
    assert_eq!(
        player.source_self_velocity_y.to_bits(),
        expected_root_y.to_bits()
    );
    assert_eq!(
        player.source_position.y.to_bits(),
        (milli_to_source_units(start_position.y) + expected_root_y).to_bits(),
        "Fighter_procUpdate applies SpecialHi self_vel.y before ftCa_SpecialHi_Coll can run ftCommon_8007D5D4"
    );
}

#[test]
fn battlefield_wait_keeps_legacy_ground_support_until_source_wait_map_callback_exists() {
    let stage = StageProfile::battlefield();
    let mut world = World::for_two_players_on_stage(stage);
    let mut player = world.players()[0];

    player.set_motion_state_alias(MotionState::Wait);
    player.motion_frame = 0;
    player.grounded = true;
    player.position = Vec2 {
        x: -55_201,
        y: stage.main_floor.y,
    };
    player.source_position = mole_core::SourceVec2::from_milli(player.position);
    player.set_source_floor_for_diagnostic(None, None);
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::Wait);
    assert!(player.grounded);
    assert_eq!(player.position.y, stage.main_floor.y);
}

#[test]
fn falcon_ground_up_special_frame13_battlefield_ignores_slippi_last_ground_as_live_floor_line() {
    let stage = StageProfile::battlefield();
    let mut world = World::for_two_players_on_stage(stage);
    let mut player = world.players()[0];
    let source_ground_velocity = 0.5559999942779541_f32;
    let start_position = Vec2 {
        x: -55_201,
        y: stage.main_floor.y,
    };
    let source_start = mole_core::SourceVec2::from_milli(start_position);
    let slippi_last_ground_id =
        source_floor_line_for_surface_at_x(stage, stage.main_floor, source_start.x);

    player.set_motion_state_alias(MotionState::SpecialHi);
    player.motion_frame = 12;
    player.grounded = true;
    player.position = start_position;
    player.source_position = source_start;
    player.set_source_floor_for_diagnostic(Some(0), None);
    player.ground_velocity_x = source_ground_velocity;
    player.source_self_velocity_x = source_ground_velocity;
    player.velocity.x = source_units_to_milli(source_ground_velocity);
    player.velocity.y = 0;
    player.captain_special_hi_vel_x = 0.0;
    player.captain_special_hi_vel_y = 0.0;
    assert!(world.set_player_state_for_diagnostic(0, player));

    let up_special = [
        PlayerInput::neutral()
            .with_special(true)
            .with_jump_secondary(true)
            .with_left_stick(76, 100),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(1860), &up_special);

    let player = world.players()[0];
    let root = source_root_motion_delta(MotionState::SpecialHi, 15)
        .expect("SpecialHi frame 15 TransN delta should be extracted");
    let expected_root_y = root.y * player.profile.model_scaling;

    assert_eq!(slippi_last_ground_id, 1);
    assert_eq!(player.motion_state, MotionState::SpecialHi);
    assert_eq!(player.motion_frame, 13);
    assert!(!player.grounded);
    assert_eq!(player.ground_velocity_x.to_bits(), 0.0_f32.to_bits());
    assert_eq!(
        player.source_position.y.to_bits(),
        (source_start.y + expected_root_y).to_bits(),
        "Slippi frame 1860 is airborne with last ground id 1; the source map-collision path must not convert that persistent line into a floor snap"
    );
}

#[test]
fn falcon_specialhi_frame55_stays_airborne_before_battlefield_left_platform_source_contact() {
    let stage = StageProfile::battlefield();
    let mut world = World::for_two_players_on_stage(stage);
    let mut player = world.players()[0];
    let start_position = Vec2 {
        x: -52_815,
        y: 27_951,
    };

    player.set_motion_state_alias(MotionState::SpecialHi);
    player.motion_frame = 54;
    player.motion_anim_frame_milli = 54_000;
    player.grounded = false;
    player.position = start_position;
    player.source_position = mole_core::SourceVec2 {
        x: -52.815_02,
        y: 27.951_029,
    };
    player.captain_special_hi_x2_b1 = true;
    player.captain_special_hi_vel_x = -0.519_375_f32;
    player.captain_special_hi_vel_y = 0.0;
    player.source_self_velocity_x = -0.699_503_f32;
    player.source_self_velocity_y = -1.701_828_f32;
    player.velocity.x = source_units_to_milli(player.source_self_velocity_x);
    player.velocity.y = source_units_to_milli(player.source_self_velocity_y);
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(1902),
        &[
            PlayerInput::neutral().with_left_stick(-125, 0),
            PlayerInput::neutral(),
        ],
    );

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::SpecialHi);
    assert_eq!(player.motion_frame, 55);
    assert_eq!(player.motion_anim_frame_milli, 55_000);
    assert!(!player.grounded);
    assert_eq!(player.source_floor_for_diagnostic(), (None, None));
}

#[test]
fn falcon_specialhi_replay_sequence_stays_airborne_until_battlefield_platform_contact() {
    let stage = StageProfile::battlefield();
    let mut world = World::for_two_players_on_stage(stage);
    let mut player = world.players()[0];

    player.set_motion_state_alias(MotionState::SpecialHi);
    player.motion_frame = 54;
    player.motion_anim_frame_milli = 54_000;
    player.grounded = false;
    player.position = Vec2 {
        x: -52_815,
        y: 27_951,
    };
    player.source_position = mole_core::SourceVec2 {
        x: -52.815_02,
        y: 27.951_029,
    };
    player.captain_special_hi_x2_b1 = true;
    player.captain_special_hi_vel_x = -0.519_375_f32;
    player.captain_special_hi_vel_y = 0.0;
    player.source_self_velocity_x = -0.699_503_f32;
    player.source_self_velocity_y = -1.701_828_f32;
    player.velocity.x = source_units_to_milli(player.source_self_velocity_x);
    player.velocity.y = source_units_to_milli(player.source_self_velocity_y);
    assert!(world.set_player_state_for_diagnostic(0, player));

    for frame in [Frame(1902), Frame(1903), Frame(1904)] {
        step_world(
            &mut world,
            frame,
            &[
                PlayerInput::neutral().with_left_stick(-125, 0),
                PlayerInput::neutral(),
            ],
        );
    }

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::SpecialHi);
    assert_eq!(player.motion_frame, 57);
    assert!(!player.grounded);
    assert_eq!(player.source_floor_for_diagnostic(), (None, None));
    assert_eq!(player.position.x, -53_912);
    assert_eq!(player.position.y, 22_925);
}

#[test]
fn falcon_specialhi_frame60_lands_on_battlefield_left_platform_from_live_source_ecb() {
    let stage = StageProfile::battlefield();
    let mut world = World::for_two_players_on_stage(stage);
    let stage = world.stage();
    let left_platform = stage.soft_platforms[0];
    let mut player = world.players()[0];

    player.set_motion_state_alias(MotionState::SpecialHi);
    player.motion_frame = 54;
    player.motion_anim_frame_milli = 54_000;
    player.grounded = false;
    player.position = Vec2 {
        x: -52_815,
        y: 27_951,
    };
    player.source_position = mole_core::SourceVec2 {
        x: -52.815_02,
        y: 27.951_029,
    };
    player.captain_special_hi_x2_b1 = true;
    player.captain_special_hi_vel_x = -0.519_375_f32;
    player.captain_special_hi_vel_y = 0.0;
    player.source_self_velocity_x = -0.699_503_f32;
    player.source_self_velocity_y = -1.701_828_f32;
    player.velocity.x = source_units_to_milli(player.source_self_velocity_x);
    player.velocity.y = source_units_to_milli(player.source_self_velocity_y);
    assert!(world.set_player_state_for_diagnostic(0, player));

    for frame in [
        Frame(1902),
        Frame(1903),
        Frame(1904),
        Frame(1905),
        Frame(1906),
        Frame(1907),
    ] {
        step_world(
            &mut world,
            frame,
            &[
                PlayerInput::neutral().with_left_stick(-125, 0),
                PlayerInput::neutral(),
            ],
        );
    }

    let landed = world.players()[0];
    assert_eq!(landed.motion_state, MotionState::LandingFallSpecial);
    assert_eq!(landed.motion_frame, 0);
    assert!(landed.grounded);
    assert_eq!(landed.position.y, left_platform.y);
    assert_eq!(
        landed.landing_lag_ticks,
        landed.profile.captain_special_attrs.specialhi_landing_lag as u8
    );
}

#[test]
fn falcon_specialhi_x2_b1_lands_on_soft_platform_via_source_air_collision() {
    let mut world = World::for_two_players();
    let stage = world.stage();
    let left_platform = stage.soft_platforms[0];
    let mut player = world.players()[0];
    let start_position = Vec2 {
        x: -56_313,
        y: 19_749,
    };

    player.set_motion_state_alias(MotionState::SpecialHi);
    player.motion_frame = 59;
    player.motion_anim_frame_milli = 59_000;
    player.grounded = false;
    player.position = start_position;
    player.source_position = mole_core::SourceVec2::from_milli(start_position);
    player.captain_special_hi_x2_b1 = true;
    player.captain_special_hi_vel_x = -0.519_375_f32;
    player.captain_special_hi_vel_y = 0.0;
    player.source_self_velocity_x = -0.699_502_47_f32;
    player.source_self_velocity_y = -1.562_893_f32;
    player.velocity.x = source_units_to_milli(player.source_self_velocity_x);
    player.velocity.y = source_units_to_milli(player.source_self_velocity_y);
    player.set_source_ecb_bottom_lock_for_diagnostic(
        2,
        0.0,
        milli_to_source_units(left_platform.y) - player.source_position.y,
    );
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(1907),
        &[
            PlayerInput::neutral().with_left_stick(-125, 0),
            PlayerInput::neutral(),
        ],
    );

    let landed = world.players()[0];
    assert_eq!(landed.motion_state, MotionState::LandingFallSpecial);
    assert!(landed.grounded);
    assert_eq!(landed.position.y, left_platform.y);
    assert_eq!(
        landed.landing_lag_ticks,
        landed.profile.captain_special_attrs.specialhi_landing_lag as u8
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
            .with_left_stick(-dash_stick_x(), 0),
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
        MotionState::SpecialAirSStart
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
            .with_left_stick(dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];
    let back_air = [
        PlayerInput::neutral()
            .with_attack(true)
            .with_left_stick(-dash_stick_x(), 0),
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
            .with_left_stick(dash_stick_x(), 0)
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
            .with_left_stick(dash_stick_x(), 0),
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
fn aerial_attack_iasa_runs_attack_air_phys_on_entry_tick() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.motion_state = MotionState::JumpF;
    player.motion_state_alias = Some(MotionState::JumpF);
    player.motion_frame = 3;
    player.grounded = false;
    player.position = Vec2 {
        x: melee_units_f32(19.49),
        y: melee_units_f32(6.82),
    };
    player.source_position.x = milli_to_source_units(player.position.x);
    player.source_position.y = milli_to_source_units(player.position.y);
    player.velocity.x = source_units_to_milli(1.5);
    player.velocity.y = source_units_to_milli(1.51);
    player.source_self_velocity_x = 1.5;
    player.source_self_velocity_y = 1.51;
    assert!(world.set_player_state_for_diagnostic(0, player));

    let attack = [
        PlayerInput::neutral().with_attack(true),
        PlayerInput::neutral(),
    ];
    step_world(&mut world, Frame(4), &attack);

    let expected_velocity_x = source_air_drift_velocity(1_500, 0, world.players()[0].profile);
    assert_eq!(world.players()[0].motion_state, MotionState::AttackAirN);
    assert_eq!(
        world.players()[0].velocity.x,
        expected_velocity_x,
        "ftCo_AttackAir_Phys runs after JumpF IASA installs AttackAir callbacks"
    );
    assert_eq!(
        world.players()[0].position.x,
        melee_units_f32(19.49) + expected_velocity_x
    );
}

#[test]
fn aerial_attack_iasa_advances_new_action_anim_on_entry_tick() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.motion_state = MotionState::JumpF;
    player.motion_state_alias = Some(MotionState::JumpF);
    player.motion_frame = 6;
    player.set_source_motion_anim_frame(6.0);
    player.grounded = false;
    assert!(world.set_player_state_for_diagnostic(0, player));

    let attack = [
        PlayerInput::neutral().with_attack(true),
        PlayerInput::neutral(),
    ];
    step_world(&mut world, Frame(4), &attack);

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::AttackAirN);
    assert_eq!(
        player.motion_frame, 1,
        "ftCo_AttackAir_EnterFromMsid calls ftAnim_8006EBA4, advancing the new action's command timeline on the entry tick"
    );
    assert_eq!(
        player.motion_anim_frame_milli, 1_000,
        "ftAction_ChangeAction sets cur_anim_frame to anim_start - frame_speed, then invokes ftAnim_8006E9B4 for the new action during the same IASA tick"
    );
}

#[test]
fn aerial_attack_iasa_processes_frame_zero_cmd_var_on_entry_tick() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.motion_state = MotionState::JumpF;
    player.motion_state_alias = Some(MotionState::JumpF);
    player.motion_frame = 6;
    player.set_source_motion_anim_frame(6.0);
    player.grounded = false;
    assert!(world.set_player_state_for_diagnostic(0, player));

    let attack_hi = [
        PlayerInput::neutral()
            .with_attack(true)
            .with_left_stick(0, 127),
        PlayerInput::neutral(),
    ];
    step_world(&mut world, Frame(4), &attack_hi);

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::AttackAirHi);
    assert_eq!(
        player.motion_cmd_var0, 1,
        "ftCo_AttackAir_EnterFromMsid calls ftAnim_8006EBA4 on entry, so frame-0 action-script cmd vars are live before aerial landing collision"
    );
}

#[test]
fn fresh_forward_dash_tap_from_wait_enters_dash_and_consumes_x_tap() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
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
        PlayerInput::neutral().with_left_stick(-dash_stick_x(), 0),
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
        PlayerInput::neutral().with_left_stick(-dash_stick_x(), -90),
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
fn wait_enters_walk_slow_from_standstill_even_with_mid_forward_stick() {
    let mut world = World::for_two_players();
    let walk = [
        PlayerInput::neutral().with_left_stick(64, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &walk);

    assert_eq!(world.players()[0].motion_state, MotionState::WalkSlow);
    assert_eq!(world.players()[0].facing, 1);
    assert_eq!(world.players()[0].motion_frame, 0);
    assert!(world.players()[0].velocity.x > 0);
    assert!(world.players()[0].velocity.x < 64 * 6);
}

#[test]
fn walk_type_promotes_from_ground_velocity_not_stick_bucket() {
    let mut world = World::for_two_players();
    let hard_walk = [
        PlayerInput::neutral().with_left_stick(100, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &hard_walk);
    assert_eq!(world.players()[0].motion_state, MotionState::WalkSlow);

    for frame in 1..=20 {
        step_world(&mut world, Frame(frame), &hard_walk);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::WalkMiddle);
    assert!(
        world.players()[0].ground_velocity_x
            >= world.players()[0].profile.walk_max_velocity
                * MeleeCommonData::provisional_mole().walk_middle_velocity_ratio
    );
}

#[test]
fn rollback_owned_input_snapshots_deterministically_select_walk_bands() {
    for (stick_x, expected_state, frames_to_hold) in [
        (30, MotionState::WalkSlow, 0),
        (64, MotionState::WalkMiddle, 20),
        (127, MotionState::WalkFast, 20),
    ] {
        let mut a = World::for_two_players();
        let mut b = World::for_two_players();
        let walk = [
            PlayerInput::neutral().with_left_stick(stick_x, 0),
            PlayerInput::neutral(),
        ];

        a.set_input_history_for_diagnostic(walk, *a.input_timers());
        b.set_input_history_for_diagnostic(walk, *b.input_timers());
        step_world(&mut a, Frame(0), &walk);
        step_world(&mut b, Frame(0), &walk);
        for frame in 1..=frames_to_hold {
            step_world(&mut a, Frame(frame), &walk);
            step_world(&mut b, Frame(frame), &walk);
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
    assert!(
        first_walk_velocity
            < source_stick_scaled_velocity(40, FighterProfile::falcon_like().walk_max_velocity)
    );

    step_world(&mut world, Frame(1), &walk_right);

    assert_eq!(world.players()[0].motion_state, MotionState::WalkSlow);
    assert!(world.players()[0].velocity.x > first_walk_velocity);
    assert!(
        world.players()[0].velocity.x
            <= source_stick_scaled_velocity(40, FighterProfile::falcon_like().walk_max_velocity)
    );
}

#[test]
fn walk_velocity_uses_source_walk_formula_and_profile_traction() {
    let steady_profile = FighterProfile {
        walk_initial_velocity: 0.15,
        walk_accel: 0.1,
        walk_max_velocity: 0.85,
        ground_friction: 0.08,
        ..FighterProfile::falcon_like()
    };
    let quick_profile = FighterProfile {
        walk_initial_velocity: 0.3,
        walk_accel: 0.2,
        walk_max_velocity: 1.7,
        ground_friction: 0.16,
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
    assert_eq!(steady.players()[0].velocity.x, 147);
    assert_eq!(quick.players()[0].velocity.x, 294);
    assert_ne!(steady.checksum(), quick.checksum());
    let steady_source_velocity = steady.players()[0].ground_velocity_x;
    let quick_source_velocity = quick.players()[0].ground_velocity_x;

    step_world(&mut steady, Frame(1), &walk_right);
    step_world(&mut quick, Frame(1), &walk_right);

    assert_eq!(
        steady.players()[0].velocity.x,
        source_walk_phys_render_velocity_from_source(
            steady_source_velocity,
            40,
            steady_profile,
            steady.common_data()
        )
    );
    assert_eq!(
        quick.players()[0].velocity.x,
        source_walk_phys_render_velocity_from_source(
            quick_source_velocity,
            40,
            quick_profile,
            quick.common_data()
        )
    );
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

    assert_eq!(side.players()[0].motion_state, MotionState::SpecialSStart);
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
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
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
    let common = MeleeCommonData::provisional_mole();
    let soft_walk = [
        PlayerInput::neutral().with_left_stick(40, 0),
        PlayerInput::neutral(),
    ];
    let full_left = [
        PlayerInput::neutral().with_left_stick(-dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &soft_walk);
    let carried_walk_velocity = world.players()[0].velocity.x;
    step_world(&mut world, Frame(1), &full_left);

    assert_eq!(world.players()[0].motion_state, MotionState::Turn);
    assert_eq!(world.players()[0].facing, 1);
    assert_eq!(
        world.players()[0].velocity.x,
        source_general_grounded_friction_velocity(
            carried_walk_velocity,
            world.players()[0].profile,
            common
        )
    );
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
    assert!(world.players()[0].velocity.x > 0);
}

#[test]
fn walk_rollout_frame_routes_opposite_dash_through_turn_physics_before_dash_entry() {
    let mut world = World::for_two_players();
    let common = MeleeCommonData::provisional_mole();
    let soft_right = [
        PlayerInput::neutral().with_left_stick(40, 0),
        PlayerInput::neutral(),
    ];
    let full_walk_right = [
        PlayerInput::neutral().with_left_stick(127, 0),
        PlayerInput::neutral(),
    ];
    let below_walk = [PlayerInput::neutral(), PlayerInput::neutral()];
    let dash_left = [
        PlayerInput::neutral().with_left_stick(-dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];
    let mid_right = [
        PlayerInput::neutral().with_left_stick(60, 0),
        PlayerInput::neutral(),
    ];
    let near_right = [
        PlayerInput::neutral().with_left_stick(70, 0),
        PlayerInput::neutral(),
    ];
    let full_right = [
        PlayerInput::neutral().with_left_stick(127, 0),
        PlayerInput::neutral(),
    ];

    for frame in 0..=2 {
        step_world(&mut world, Frame(frame), &soft_right);
    }
    for frame in 3..=40 {
        step_world(&mut world, Frame(frame), &full_walk_right);
    }

    assert!(matches!(
        world.players()[0].motion_state,
        MotionState::WalkSlow | MotionState::WalkMiddle | MotionState::WalkFast
    ));
    let full_walk_velocity = world.players()[0].velocity.x;
    assert!(close_to(
        full_walk_velocity,
        source_units_to_milli(world.players()[0].profile.walk_max_velocity),
        10
    ));

    step_world(&mut world, Frame(41), &below_walk);

    assert_eq!(world.players()[0].motion_state, MotionState::Wait);
    assert_eq!(world.players()[0].facing, 1);
    let wait_frame_velocity = source_general_grounded_friction_velocity(
        full_walk_velocity,
        world.players()[0].profile,
        common,
    );
    assert_eq!(world.players()[0].velocity.x, wait_frame_velocity);

    step_world(&mut world, Frame(42), &dash_left);

    assert_eq!(world.players()[0].motion_state, MotionState::Turn);
    let turn_frame_velocity = source_general_grounded_friction_velocity(
        wait_frame_velocity,
        world.players()[0].profile,
        common,
    );
    assert_eq!(world.players()[0].velocity.x, turn_frame_velocity);

    step_world(&mut world, Frame(43), &dash_left);

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    assert_eq!(world.players()[0].facing, -1);
    assert_eq!(
        world.players()[0].velocity.x,
        turn_frame_velocity
            - source_units_to_milli(world.players()[0].profile.dash_initial_velocity)
    );
    assert!(
        world.players()[0].velocity.x
            > -source_units_to_milli(world.players()[0].profile.dash_initial_velocity)
    );
    let dash_entry_velocity = world.players()[0].velocity.x;
    let dash_entry_source_velocity = world.players()[0].ground_velocity_x;

    step_world(&mut world, Frame(44), &soft_right);

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    assert_eq!(
        world.players()[0].velocity.x,
        source_dash_phys_render_velocity_from_source(
            dash_entry_source_velocity,
            40,
            world.players()[0].profile,
            common
        ),
        "Dash_Phys live-stick acceleration runs on the next input frame after same-frame xE8 staging"
    );

    step_world(&mut world, Frame(45), &mid_right);
    step_world(&mut world, Frame(46), &near_right);
    step_world(&mut world, Frame(47), &full_right);

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    assert_eq!(world.players()[0].facing, -1);
    assert!(world.players()[0].velocity.x > dash_entry_velocity);
}

#[test]
fn dash_entry_delta_updates_ground_velocity_after_translation_only() {
    let mut world = World::for_two_players();
    let full_dash_right = [
        PlayerInput::neutral().with_left_stick(127, 0),
        PlayerInput::neutral(),
    ];

    let start_x = world.players()[0].position.x;

    step_world(&mut world, Frame(0), &full_dash_right);

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    assert_eq!(
        world.players()[0].velocity.x,
        source_units_to_milli(world.players()[0].profile.dash_initial_velocity)
    );
    assert_eq!(world.players()[0].position.x, start_x);
}

#[test]
fn dash_entry_uses_source_xe8_staging_before_next_dash_phys() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(127, 0),
        PlayerInput::neutral(),
    ];

    let start_x = world.players()[0].position.x;

    step_world(&mut world, Frame(0), &dash_right);

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    assert_eq!(world.players()[0].position.x, start_x);
    assert_eq!(
        world.players()[0].velocity.x,
        source_units_to_milli(world.players()[0].profile.dash_initial_velocity)
    );
    assert_eq!(
        world.players()[0].dash_entry_velocity_delta,
        0.0,
        "ftCo_Dash_Phys consumes mv.co.dash.x0 during the same engine frame"
    );

    step_world(&mut world, Frame(1), &dash_right);

    assert!(
        world.players()[0].position.x > start_x,
        "the next frame moves using committed gr_vel"
    );
    assert!(
        world.players()[0].velocity.x
            > source_units_to_milli(world.players()[0].profile.dash_initial_velocity),
        "ordinary Dash_Phys live-stick acceleration runs on the next input frame after same-frame xE8 staging"
    );
}

#[test]
fn dash_to_turn_applies_source_iasa_decay_then_same_frame_turn_physics() {
    let mut world = World::for_two_players();
    let common = MeleeCommonData::provisional_mole();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];
    let dash_left = [
        PlayerInput::neutral().with_left_stick(-dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    for frame in 1..=common.dash_early_action_window {
        step_world(&mut world, Frame(frame as u32), &dash_right);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    let carried_dash_velocity = world.players()[0].velocity.x;
    let expected_velocity = source_dash_to_turn_frame_velocity(
        carried_dash_velocity,
        world.players()[0].profile,
        common,
    );
    let start_x = world.players()[0].position.x;

    step_world(
        &mut world,
        Frame((common.dash_early_action_window + 1) as u32),
        &dash_left,
    );

    assert_eq!(world.players()[0].motion_state, MotionState::Turn);
    assert_eq!(
        world.players()[0].velocity.x,
        expected_velocity,
        "Dash->Turn runs ftCo_Dash_IASA x54, then Fighter_procUpdate uses Turn_Phys/ft_80084F3C in the same frame"
    );
    assert_eq!(
        world.players()[0].position.x - start_x,
        expected_velocity,
        "same-frame Turn_Phys contributes xE4 to both position and committed gr_vel"
    );
}

#[test]
fn walk_state_slow_rise_to_full_stick_keeps_walking() {
    let mut world = World::for_two_players();
    let soft_walk = [
        PlayerInput::neutral().with_left_stick(40, 0),
        PlayerInput::neutral(),
    ];
    let full_stick = [
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &soft_walk);
    step_world(&mut world, Frame(1), &soft_walk);
    step_world(&mut world, Frame(2), &soft_walk);
    step_world(&mut world, Frame(3), &full_stick);

    assert!(matches!(
        world.players()[0].motion_state,
        MotionState::WalkSlow | MotionState::WalkMiddle | MotionState::WalkFast
    ));
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
    let profile = FighterProfile {
        standing_turn_direction_change_frames: 2,
        ..FighterProfile::falcon_like()
    };
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let soft_left = [
        PlayerInput::neutral().with_left_stick(-40, 0),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    step_world(&mut world, Frame(0), &soft_left);

    assert_eq!(world.players()[0].motion_state, MotionState::Turn);
    assert_eq!(world.players()[0].facing, 1);

    step_world(&mut world, Frame(1), &neutral);

    assert_eq!(world.players()[0].facing, 1);

    step_world(&mut world, Frame(2), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::Turn);
    assert_eq!(world.players()[0].facing, -1);
}

#[test]
fn standing_turn_fresh_outward_dash_tap_dashes_after_the_turn_frame() {
    let mut world = World::for_two_players();
    let soft_left = [
        PlayerInput::neutral().with_left_stick(-40, 0),
        PlayerInput::neutral(),
    ];
    let full_left = [
        PlayerInput::neutral().with_left_stick(-dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &soft_left);
    step_world(&mut world, Frame(1), &full_left);

    assert_eq!(world.players()[0].motion_state, MotionState::Turn);
    assert_eq!(world.players()[0].facing, 1);

    for frame in 2..=6 {
        step_world(&mut world, Frame(frame), &full_left);
        assert_eq!(world.players()[0].motion_state, MotionState::Turn);
    }

    step_world(&mut world, Frame(7), &full_left);

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    assert_eq!(world.players()[0].facing, -1);
    assert_eq!(world.input_timers()[0].x_tap, 0xfe);
}

#[test]
fn ucf_dashback_amendment_applies_source_turn_hook_without_changing_vanilla_turn() {
    let soft_left = [
        PlayerInput::neutral().with_left_stick(-40, 0),
        PlayerInput::neutral(),
    ];
    let full_left = [
        PlayerInput::neutral().with_left_stick(-dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];
    let ucf_full_left = [
        full_left[0].with_ucf_dashback_amendment(true),
        PlayerInput::neutral(),
    ];

    let mut vanilla = World::for_two_players();
    step_world(&mut vanilla, Frame(0), &soft_left);
    step_world(&mut vanilla, Frame(1), &full_left);

    assert_eq!(vanilla.players()[0].motion_state, MotionState::Turn);
    assert_eq!(vanilla.players()[0].facing, 1);

    let mut ucf = World::for_two_players();
    step_world(&mut ucf, Frame(0), &soft_left);
    step_world(&mut ucf, Frame(1), &ucf_full_left);

    assert_eq!(ucf.players()[0].motion_state, MotionState::Dash);
    assert_eq!(ucf.players()[0].facing, -1);
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

    assert_eq!(world.players()[0].motion_state, MotionState::SpecialSStart);
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
fn turn_pre_flip_old_forward_tilt_uses_temporary_source_facing_for_attack_checks() {
    let mut world = World::for_two_players();
    let soft_left = [
        PlayerInput::neutral().with_left_stick(-40, 0),
        PlayerInput::neutral(),
    ];
    let old_forward_attack = [
        PlayerInput::neutral()
            .with_left_stick(40, 0)
            .with_attack(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &soft_left);
    assert_eq!(world.players()[0].motion_state, MotionState::Turn);
    assert_eq!(world.players()[0].facing, 1);

    step_world(&mut world, Frame(1), &old_forward_attack);

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

    for frame in 2..6 {
        step_world(&mut world, Frame(frame), &neutral);
        assert_eq!(world.players()[0].motion_state, MotionState::Turn);
    }

    step_world(&mut world, Frame(6), &soft_left);

    assert_eq!(world.players()[0].motion_state, MotionState::SpecialSStart);
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
fn completed_turn_checks_wait_inputs_on_the_same_frame() {
    let mut world = World::for_two_players();
    let soft_left = [
        PlayerInput::neutral().with_left_stick(-40, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &soft_left);
    assert_eq!(world.players()[0].motion_state, MotionState::Turn);

    for frame in 1..11 {
        step_world(&mut world, Frame(frame), &soft_left);
        assert_eq!(world.players()[0].motion_state, MotionState::Turn);
    }

    step_world(&mut world, Frame(11), &soft_left);

    assert_eq!(world.players()[0].motion_state, MotionState::WalkSlow);
    assert_eq!(world.players()[0].facing, -1);
    assert!(world.players()[0].velocity.x < 0);
}

#[test]
fn standing_turn_total_duration_uses_profile_action_frames() {
    let profile = FighterProfile {
        standing_turn_total_frames: 7,
        ..FighterProfile::falcon_like()
    };
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let soft_left = [
        PlayerInput::neutral().with_left_stick(-40, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &soft_left);

    assert_current_action_returns_to_wait_after_frames(&mut world, 0, MotionState::Turn, 7);
}

#[test]
fn walk_state_dash_strength_on_last_valid_tap_frame_enters_dash() {
    let mut world = World::for_two_players();
    let soft_walk = [
        PlayerInput::neutral().with_left_stick(40, 0),
        PlayerInput::neutral(),
    ];
    let full_walk = [
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &soft_walk);
    step_world(&mut world, Frame(1), &full_walk);

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    assert_eq!(world.players()[0].facing, 1);
    assert_eq!(world.input_timers()[0].x_tap, 0xfe);
}

#[test]
fn fresh_opposite_dash_tap_during_dash_enters_turn() {
    let mut world = World::for_two_players();
    let common = MeleeCommonData::provisional_mole();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let dash_left = [
        PlayerInput::neutral().with_left_stick(-dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    for frame in 1..=4 {
        step_world(&mut world, Frame(frame), &neutral);
    }
    let carried_dash_velocity = world.players()[0].velocity.x;
    let dash_to_turn_velocity = source_dash_to_turn_frame_velocity(
        carried_dash_velocity,
        world.players()[0].profile,
        common,
    );
    step_world(&mut world, Frame(5), &dash_left);

    assert_eq!(world.players()[0].motion_state, MotionState::Turn);
    assert_eq!(world.players()[0].facing, 1);
    assert_eq!(world.players()[0].velocity.x, dash_to_turn_velocity);
    assert_eq!(world.input_timers()[0].x_tap, 0);

    step_world(&mut world, Frame(6), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::Turn);
    assert_eq!(world.players()[0].facing, -1);
    assert!(world.players()[0].velocity.x > 0);
    assert!(world.players()[0].velocity.x < dash_to_turn_velocity);
}

#[test]
fn dash_out_of_smash_turn_uses_source_initial_dash_delta_from_carried_ground_velocity() {
    let mut world = World::for_two_players();
    let common = MeleeCommonData::provisional_mole();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let dash_left = [
        PlayerInput::neutral().with_left_stick(-dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    for frame in 1..=4 {
        step_world(&mut world, Frame(frame), &neutral);
    }
    let carried_dash_velocity = world.players()[0].velocity.x;
    let dash_to_turn_velocity = source_dash_to_turn_frame_velocity(
        carried_dash_velocity,
        world.players()[0].profile,
        common,
    );
    step_world(&mut world, Frame(5), &dash_left);

    assert_eq!(world.players()[0].motion_state, MotionState::Turn);
    assert_eq!(world.players()[0].velocity.x, dash_to_turn_velocity);

    step_world(&mut world, Frame(6), &dash_left);

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    assert_eq!(world.players()[0].facing, -1);
    assert_eq!(
        world.players()[0].velocity.x,
        dash_to_turn_velocity
            - source_units_to_milli(world.players()[0].profile.dash_initial_velocity)
    );
    assert!(
        world.players()[0].velocity.x
            > -source_units_to_milli(world.players()[0].profile.dash_initial_velocity)
    );
}

#[test]
fn smash_turn_can_dash_out_on_the_turn_frame_if_stick_is_still_outward() {
    let mut world = World::for_two_players();
    let dash_left = [
        PlayerInput::neutral().with_left_stick(-dash_stick_x(), 0),
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
        PlayerInput::neutral().with_left_stick(-dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];
    let below_dash_left = [
        PlayerInput::neutral().with_left_stick(-run_stick_x(), 0),
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
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
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
        PlayerInput::neutral().with_left_stick(-dash_stick_x(), 0),
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
    assert!(world.players()[0].velocity.x < 2_000);
}

#[test]
fn tap_start_dash_blocks_fresh_opposite_dashback_during_early_branch() {
    let mut world = World::for_two_players();
    let full_dash_right = [
        PlayerInput::neutral().with_left_stick(127, 0),
        PlayerInput::neutral(),
    ];
    let full_left = [
        PlayerInput::neutral().with_left_stick(-128, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &full_dash_right);
    let dash_entry_velocity = world.players()[0].velocity.x;

    step_world(&mut world, Frame(1), &full_left);

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    assert_eq!(world.players()[0].facing, 1);
    assert_eq!(world.players()[0].motion_frame, 1);
    assert_eq!(world.input_timers()[0].x_tap, 0);
    assert_eq!(
        world.players()[0].velocity.x,
        source_dash_phys_velocity(
            dash_entry_velocity,
            -128,
            world.players()[0].profile,
            world.common_data(),
        ),
        "tap-start Dash blocks opposite dashback, but live-stick Dash_Phys acceleration still runs"
    );
}

#[test]
fn tap_start_dash_held_opposite_after_tap_window_stays_dash() {
    let mut world = World::for_two_players();
    let mut player = PlayerState::new(0, 0, -1);
    player.set_motion_state_alias(MotionState::Dash);
    player.motion_frame = 20;
    player.grounded = true;
    player.facing = -1;
    player.dash_started_from_tap = true;
    assert!(world.set_player_state_for_diagnostic(0, player));
    let held_opposite = PlayerInput::neutral().with_left_stick(100, 0);
    let mut timers = [MeleeInputTimers::expired(); 2];
    timers[0].x_tap = world.common_data().dash_tap_window;
    world.set_input_history_for_diagnostic([held_opposite, PlayerInput::neutral()], timers);

    step_world(
        &mut world,
        Frame(398),
        &[held_opposite, PlayerInput::neutral()],
    );

    assert_eq!(
        world.players()[0].motion_state,
        MotionState::Dash,
        "ftCo_Dash_IASA calls ftCo_Dash_CheckInput for late opposite dashback, so held opposite stick must be rejected once x670_timer_lstick_tilt_x ages past p_ftCommonData->x40"
    );
    assert!(
        world.input_timers()[0].x_tap >= world.common_data().dash_tap_window,
        "test setup must age the x tap timer out of the decomp dash tap window"
    );
}

#[test]
fn moonwalk_payload_bottom_gate_trace_stays_dash_and_applies_opposite_influence() {
    let mut world = World::for_two_players();
    let full_dash_right = [
        PlayerInput::neutral().with_left_stick(127, 0),
        PlayerInput::neutral(),
    ];
    let opposite_bottom_gate = [
        PlayerInput::neutral().with_left_stick(-101, -45),
        PlayerInput::neutral(),
    ];
    let full_left = [
        PlayerInput::neutral().with_left_stick(-128, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &full_dash_right);
    let dash_entry_velocity = world.players()[0].velocity.x;

    step_world(&mut world, Frame(1), &opposite_bottom_gate);

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    assert_eq!(world.players()[0].facing, 1);
    assert_eq!(world.players()[0].motion_frame, 1);
    assert_eq!(
        world.players()[0].velocity.x,
        source_dash_phys_velocity(
            dash_entry_velocity,
            -101,
            world.players()[0].profile,
            world.common_data(),
        ),
        "Dash_Phys applies moonwalk-like opposite stick influence on the next input frame"
    );
    assert_eq!(world.input_timers()[0].x_tap, 0);

    step_world(&mut world, Frame(2), &opposite_bottom_gate);

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    assert_eq!(world.players()[0].facing, 1);
    assert_eq!(world.players()[0].motion_frame, 2);
    assert_eq!(world.input_timers()[0].x_tap, 1);
    assert!(world.players()[0].velocity.x > 0);
    assert!(world.players()[0].velocity.x < dash_entry_velocity);
    let bottom_gate_velocity = world.players()[0].velocity.x;

    step_world(&mut world, Frame(3), &full_left);

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    assert_eq!(world.players()[0].facing, 1);
    assert_eq!(world.players()[0].motion_frame, 3);
    assert_eq!(world.input_timers()[0].x_tap, 2);
    assert!(world.players()[0].velocity.x > 0);
    assert!(world.players()[0].velocity.x < bottom_gate_velocity);

    let first_full_left_velocity = world.players()[0].velocity.x;
    step_world(&mut world, Frame(4), &full_left);

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    assert_eq!(world.players()[0].facing, 1);
    assert_eq!(world.input_timers()[0].x_tap, 3);
    assert!(world.players()[0].velocity.x < first_full_left_velocity);
}

#[test]
fn dash_can_relay_moonwalk_like_stick_rolls_before_initial_dash_resolves() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];
    let inputs = [
        -40,
        -60,
        -70,
        -dash_stick_x(),
        -dash_stick_x(),
        40,
        60,
        70,
        dash_stick_x(),
        dash_stick_x(),
        -40,
        -60,
        -70,
        -dash_stick_x(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    let initial_dash_velocity = world.players()[0].velocity.x;
    let mut velocity_after_first_left_chain = 0;
    let mut velocity_after_right_chain = 0;

    for (index, stick_x) in inputs.into_iter().enumerate() {
        let frame = Frame((index + 1) as u32);
        let input = [
            PlayerInput::neutral().with_left_stick(stick_x, 0),
            PlayerInput::neutral(),
        ];
        step_world(&mut world, frame, &input);

        if frame == Frame(5) {
            velocity_after_first_left_chain = world.players()[0].velocity.x;
        } else if frame == Frame(10) {
            velocity_after_right_chain = world.players()[0].velocity.x;
        }
    }

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    assert_eq!(world.players()[0].facing, 1);
    assert!(world.players()[0].motion_frame < world.players()[0].profile.dash_frames);
    assert!(velocity_after_first_left_chain < initial_dash_velocity);
    assert!(velocity_after_right_chain > velocity_after_first_left_chain);
    assert!(world.players()[0].velocity.x < velocity_after_right_chain);
}

#[test]
fn dash_neutral_stick_applies_dash_friction_before_dash_ends() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    step_world(&mut world, Frame(0), &dash_right);
    let dash_velocity = world.players()[0].velocity.x;
    step_world(&mut world, Frame(1), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    assert_eq!(
        world.players()[0].velocity.x,
        source_dash_phys_velocity(
            dash_velocity,
            0,
            world.players()[0].profile,
            world.common_data(),
        ),
        "neutral Dash_Phys applies x60 run friction on the next input frame after same-frame xE8 staging"
    );

    step_world(&mut world, Frame(2), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    assert!(world.players()[0].velocity.x > 0);
    assert!(world.players()[0].velocity.x < dash_velocity);
}

#[test]
fn dash_physics_uses_melee_cleaned_main_stick_deadzone() {
    let profile = FighterProfile::falcon_like();
    let common = MeleeCommonData::provisional_mole();
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let mut player = PlayerState::new_with_profile(0, 0, -1, profile);
    player.motion_state = MotionState::Dash;
    player.motion_frame = 4;
    player.grounded = true;
    player.ground_velocity_x = milli_to_source_units(-2_220);
    player.velocity.x = -2_220;
    world.set_player_state_for_diagnostic(0, player);

    let inside_deadzone = [
        PlayerInput::neutral().with_left_stick(5, -10),
        PlayerInput::neutral(),
    ];
    step_world(&mut world, Frame(0), &inside_deadzone);

    assert_eq!(
        world.players()[0].velocity.x,
        source_dash_phys_velocity(-2_220, 0, profile, common),
        "Fighter input cleanup zeroes lstick.x when ABS(lstick.x) <= p_ftCommonData->x0 before Dash_Phys chooses neutral friction"
    );
}

#[test]
fn dash_entry_x0_is_consumed_before_next_normal_dash_physics_tick() {
    let mut world = World::for_two_players();
    let full_right = [
        PlayerInput::neutral().with_left_stick(127, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &full_right);
    let dash_entry_velocity = world.players()[0].velocity.x;

    step_world(&mut world, Frame(1), &full_right);

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    assert_eq!(world.players()[0].motion_frame, 1);
    assert_eq!(
        world.players()[0].velocity.x,
        source_dash_phys_velocity(
            dash_entry_velocity,
            127,
            world.players()[0].profile,
            world.common_data(),
        ),
        "same-frame Dash_Phys consumes mv.co.dash.x0, so the next input frame applies live-stick dash/run acceleration"
    );
}

#[test]
fn dash_acceleration_uses_profile_source_accel_and_stick_scaled_target() {
    let profile = FighterProfile {
        dash_initial_velocity: 0.0,
        dash_run_acceleration_a: 0.02,
        dash_run_acceleration_b: 0.05,
        dash_run_terminal_velocity: 1.0,
        ground_max_horizontal_velocity: 1.2,
        ground_friction: 0.03,
        ..FighterProfile::falcon_like()
    };
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let dash_right = [
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    step_world(&mut world, Frame(1), &dash_right);

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    assert_eq!(world.players()[0].velocity.x, 66);
}

#[test]
fn dash_ground_velocity_accumulates_source_float_sub_milli_accel() {
    let profile = FighterProfile {
        dash_initial_velocity: 0.0,
        dash_run_acceleration_a: 0.00049,
        dash_run_acceleration_b: 0.0,
        dash_run_terminal_velocity: 2.0,
        ground_max_horizontal_velocity: 3.0,
        ground_friction: 0.0,
        ..FighterProfile::falcon_like()
    };
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let mut player = PlayerState::new_with_profile(0, 0, 1, profile);
    player.motion_state = MotionState::Dash;
    player.motion_frame = 1;
    player.grounded = true;
    world.set_player_state_for_diagnostic(0, player);
    let hold_right = [
        PlayerInput::neutral().with_left_stick(127, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &hold_right);
    step_world(&mut world, Frame(1), &hold_right);

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    assert_eq!(
        world.players()[0].velocity.x,
        1,
        "source gr_vel is f32, so two 0.00049-unit Dash_Phys accelerations should survive instead of rounding away per frame"
    );
}

#[test]
fn dash_acceleration_treats_hsd_clamped_full_left_as_melee_negative_one() {
    let profile = FighterProfile {
        dash_initial_velocity: 0.0,
        dash_run_acceleration_a: 0.15000000596046448,
        dash_run_acceleration_b: 0.009999999776482582,
        dash_run_terminal_velocity: 2.299999952316284,
        ground_max_horizontal_velocity: 3.0,
        ground_friction: 0.08,
        ..FighterProfile::falcon_like()
    };
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let mut player = PlayerState::new_with_profile(0, 0, -1, profile);
    player.motion_state = MotionState::Dash;
    player.motion_frame = 1;
    player.grounded = true;
    world.set_player_state_for_diagnostic(0, player);
    let dash_left = [
        PlayerInput::neutral().with_left_stick(-127, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_left);

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    assert_eq!(
        world.players()[0].velocity.x,
        -160,
        "Melee fighter input sees HSD-clamped -127 as -1.0f, so getAccelAndTarget is -0.15 - 0.01"
    );
}

#[test]
fn dash_target_velocity_uses_hsd_normalized_stick_float_not_negative_128_scale() {
    let profile = FighterProfile {
        dash_initial_velocity: 0.0,
        dash_run_acceleration_a: 0.0,
        dash_run_acceleration_b: 0.1,
        dash_run_terminal_velocity: 2.299999952316284,
        ground_max_horizontal_velocity: 3.0,
        ground_friction: 0.08,
        ..FighterProfile::falcon_like()
    };
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let mut player = PlayerState::new_with_profile(0, 0, -1, profile);
    player.motion_state = MotionState::Dash;
    player.motion_frame = 1;
    player.grounded = true;
    player.ground_velocity_x = milli_to_source_units(-2_260);
    player.velocity.x = -2_260;
    world.set_player_state_for_diagnostic(0, player);

    let slippi_left = [
        PlayerInput::neutral().with_left_stick(-125, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &slippi_left);

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    assert_eq!(
        world.players()[0].velocity.x,
        -2_271,
        "Melee fighter math uses the HSD-normalized stick value represented in the replay, so -125 exported from -0.9875 targets round(-0.9875 * 2.3 * 1000)"
    );
}

#[test]
fn dash_iasa_side_special_entry_uses_ft_80084fa8_transn_ground_physics() {
    let profile = FighterProfile {
        dash_initial_velocity: 2.0,
        dash_run_acceleration_a: 0.0,
        dash_run_acceleration_b: 0.0,
        dash_run_terminal_velocity: 2.0,
        ground_friction: 0.08,
        ..FighterProfile::falcon_like()
    };
    let common = MeleeCommonData {
        dash_velocity_decay: 0.75,
        ..MeleeCommonData::provisional_mole()
    };
    let mut world = World::for_two_players_on_stage_with_profiles_and_common_data(
        StageProfile::battlefield_test(),
        [profile; 2],
        common,
    );
    let dash_right = [
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];
    let side_special = [
        PlayerInput::neutral()
            .with_left_stick(dash_stick_x(), 0)
            .with_special(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    assert_eq!(world.players()[0].velocity.x, 2_000);

    step_world(&mut world, Frame(1), &side_special);

    assert_eq!(world.players()[0].motion_state, MotionState::SpecialSStart);
    assert_eq!(
        world.players()[0].velocity.x,
        source_action_root_motion_delta_milli_for_profile(
            SourceActionKey::new("SpecialSStart"),
            2,
            profile
        )
    );
}

#[test]
fn dash_iasa_side_special_entry_resets_dash_carry_before_transn_physics() {
    let profile = FighterProfile {
        dash_initial_velocity: 2.0,
        dash_run_acceleration_a: 0.15000000596046448,
        dash_run_acceleration_b: 0.009999999776482582,
        dash_run_terminal_velocity: 2.299999952316284,
        ground_friction: 0.08,
        ..FighterProfile::falcon_like()
    };
    let common = MeleeCommonData {
        dash_velocity_decay: 0.75,
        ..MeleeCommonData::provisional_mole()
    };
    let mut world = World::for_two_players_on_stage_with_profiles_and_common_data(
        StageProfile::battlefield_test(),
        [profile; 2],
        common,
    );
    let dash_right = [
        PlayerInput::neutral().with_left_stick(127, 0),
        PlayerInput::neutral(),
    ];
    let opposite_side_special = [
        PlayerInput::neutral()
            .with_left_stick(-127, 0)
            .with_special(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);

    step_world(&mut world, Frame(1), &opposite_side_special);

    assert_eq!(world.players()[0].motion_state, MotionState::SpecialSStart);
    assert_eq!(
        world.players()[0].velocity.x,
        -source_action_root_motion_delta_milli_for_profile(
            SourceActionKey::new("SpecialSStart"),
            2,
            profile
        )
    );
}

#[test]
fn dash_state_accepts_side_special_before_shield_or_jump() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
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

    assert_eq!(world.players()[0].motion_state, MotionState::SpecialSStart);
}

#[test]
fn dash_state_dash_grab_beats_dash_attack_and_guard() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
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
fn dash_state_early_defensive_window_held_digital_lr_enters_escape_forward() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];
    let held_digital_shield = [
        PlayerInput::neutral()
            .with_left_stick(80, 0)
            .with_left_trigger_digital(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    world.set_input_history_for_diagnostic(
        [
            PlayerInput::neutral().with_left_trigger_digital(true),
            PlayerInput::neutral(),
        ],
        [MeleeInputTimers::expired(), MeleeInputTimers::expired()],
    );
    step_world(&mut world, Frame(1), &held_digital_shield);

    assert_eq!(world.players()[0].motion_state, MotionState::EscapeF);
}

#[test]
fn dash_state_early_defensive_window_analog_trigger_enters_guard_on_not_escape_forward() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];
    let analog_shield = [
        PlayerInput::neutral()
            .with_left_stick(-127, 0)
            .with_left_trigger_analog(98),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    step_world(&mut world, Frame(1), &analog_shield);

    assert_eq!(world.players()[0].motion_state, MotionState::GuardOn);
}

#[test]
fn dash_fresh_digital_lr_in_tap_dash_window_enters_escape_forward() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];
    let fresh_digital_r = [
        PlayerInput::neutral()
            .with_left_stick(dash_stick_x(), 0)
            .with_right_trigger_digital(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);

    step_world(&mut world, Frame(1), &fresh_digital_r);

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::EscapeF);
}

#[test]
fn dash_state_after_defensive_window_shield_enters_guard_on() {
    let mut world = World::for_two_players();
    let common = MeleeCommonData::provisional_mole();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];
    let shield = [
        PlayerInput::neutral()
            .with_left_stick(80, 0)
            .with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    for frame in 1..=3 {
        step_world(&mut world, Frame(frame), &dash_right);
    }
    let carried_dash_velocity = world.players()[0].velocity.x;
    let expected_velocity = source_general_grounded_friction_velocity(
        source_dash_iasa_decay(carried_dash_velocity, common),
        world.players()[0].profile,
        common,
    );
    step_world(&mut world, Frame(4), &shield);

    assert_eq!(world.players()[0].motion_state, MotionState::GuardOn);
    assert_eq!(
        world.players()[0].velocity.x, expected_velocity,
        "late Dash shield entry follows ftCo_Dash_IASA: enter GuardOn, apply x54 dash decay, then GuardOn_Phys/ft_80084F3C"
    );
}

#[test]
fn guard_on_applies_ground_traction_while_shield_is_held() {
    let common = MeleeCommonData::provisional_mole();
    let profile = FighterProfile::falcon_like();
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let mut player = PlayerState::new_with_profile(0, 0, 1, profile);
    player.motion_state = MotionState::GuardOn;
    player.grounded = true;
    player.ground_velocity_x = milli_to_source_units(408);
    player.velocity.x = 408;
    world.set_player_state_for_diagnostic(0, player);
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(255),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &shield);

    assert_eq!(world.players()[0].motion_state, MotionState::GuardOn);
    assert_eq!(
        world.players()[0].velocity.x,
        source_general_grounded_friction_velocity(408, profile, common),
        "ftCo_GuardOn_Phys calls ft_80084F3C while the shield startup state is still active"
    );
}

#[test]
fn guard_applies_ground_traction_while_shield_is_held() {
    let common = MeleeCommonData::provisional_mole();
    let profile = FighterProfile::falcon_like();
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let mut player = PlayerState::new_with_profile(0, 0, 1, profile);
    player.motion_state = MotionState::Guard;
    player.grounded = true;
    player.ground_velocity_x = milli_to_source_units(408);
    player.velocity.x = 408;
    world.set_player_state_for_diagnostic(0, player);
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(255),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &shield);

    assert_eq!(world.players()[0].motion_state, MotionState::Guard);
    assert_eq!(
        world.players()[0].velocity.x,
        source_general_grounded_friction_velocity(408, profile, common),
        "ftCo_Guard_Phys calls ft_80084F3C while steady shield is held"
    );
}

#[test]
fn dash_fresh_digital_lr_enters_guard_reflect_with_source_x54_decay() {
    let mut world = World::for_two_players();
    let common = MeleeCommonData::provisional_mole();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];
    let fresh_digital_r = [
        PlayerInput::neutral()
            .with_left_stick(121, -37)
            .with_right_trigger_digital(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    for frame in 1..=common.dash_early_action_window {
        step_world(&mut world, Frame(frame as u32), &dash_right);
    }

    let carried_dash_velocity = world.players()[0].velocity.x;
    let expected_velocity = source_general_grounded_friction_velocity(
        source_dash_iasa_decay(carried_dash_velocity, common),
        FighterProfile::falcon_like(),
        common,
    );

    step_world(
        &mut world,
        Frame((common.dash_early_action_window + 1) as u32),
        &fresh_digital_r,
    );

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::GuardReflect);
    assert_eq!(
        player.velocity.x, expected_velocity,
        "Dash IASA applies x54 decay, then the same source frame runs GuardReflect_Phys ground friction"
    );
}

#[test]
fn guard_on_startup_fresh_digital_lr_enters_guard_reflect_like_ftco_80093694() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.motion_state = MotionState::GuardOn;
    player.motion_state_alias = Some(MotionState::GuardOn);
    player.melee_action_state_id = Some(MeleeActionStateId::new(178));
    player.motion_frame = 1;
    player.grounded = true;
    player.shield_health = MeleeCommonData::provisional_mole().shield_start_health;
    assert!(world.set_player_state_for_diagnostic(0, player));

    world.set_input_history_for_diagnostic(
        [PlayerInput::neutral(), PlayerInput::neutral()],
        [MeleeInputTimers::expired(), MeleeInputTimers::expired()],
    );
    let fresh_digital_l = [
        PlayerInput::neutral()
            .with_left_trigger_analog(255)
            .with_left_trigger_digital(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &fresh_digital_l);

    assert_eq!(world.players()[0].motion_state, MotionState::GuardReflect);
    assert_eq!(
        world.players()[0].melee_action_state_id,
        Some(MeleeActionStateId::new(182))
    );
}

#[test]
fn guard_on_reflect_preserves_post_anim_frame_like_ftco_8009388c() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.grounded = true;
    player.motion_state = MotionState::GuardOn;
    player.motion_state_alias = Some(MotionState::GuardOn);
    player.melee_action_state_id = Some(MeleeActionStateId::new(178));
    player.source_action_total_frames = 8;
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.shield_health = MeleeCommonData::provisional_mole().shield_start_health;
    assert!(world.set_player_state_for_diagnostic(0, player));
    world.set_input_history_for_diagnostic(
        [PlayerInput::neutral(), PlayerInput::neutral()],
        [MeleeInputTimers::expired(), MeleeInputTimers::expired()],
    );

    step_world(
        &mut world,
        Frame(0),
        &[
            PlayerInput::neutral()
                .with_left_trigger_analog(255)
                .with_left_trigger_digital(true),
            PlayerInput::neutral(),
        ],
    );

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::GuardReflect);
    assert_eq!(
        player.motion_frame, 1,
        "ftCo_GuardOn_Anim advances the GuardOn animation before ftCo_GuardOn_IASA reaches ftCo_80093694, and ftCo_8009388C preserves fp->cur_anim_frame"
    );

    for frame in 1..=7 {
        step_world(
            &mut world,
            Frame(frame),
            &[
                PlayerInput::neutral().with_left_trigger_digital(true),
                PlayerInput::neutral(),
            ],
        );
    }

    assert_eq!(
        world.players()[0].motion_state,
        MotionState::Guard,
        "the preserved GuardOn frame should finish through ftCo_GuardReflect_Anim -> ftCo_GuardOn_Anim on the next tick"
    );
}

#[test]
fn turn_fresh_digital_lr_enters_guard_reflect_like_ftco_80091a4c() {
    let mut world = World::for_two_players();
    let common = MeleeCommonData::provisional_mole();
    let mut player = world.players()[0];
    player.motion_state = MotionState::Turn;
    player.motion_state_alias = Some(MotionState::Turn);
    player.melee_action_state_id = Some(MeleeActionStateId::new(18));
    player.motion_frame = 0;
    player.grounded = true;
    player.facing = 1;
    player.turn_facing_after = -1;
    player.turn_has_turned = false;
    player.turn_just_turned = false;
    player.ground_velocity_x = milli_to_source_units(468);
    player.velocity.x = 468;
    assert!(world.set_player_state_for_diagnostic(0, player));
    world.set_input_history_for_diagnostic(
        [PlayerInput::neutral(), PlayerInput::neutral()],
        [MeleeInputTimers::expired(), MeleeInputTimers::expired()],
    );
    let fresh_digital_l = [
        PlayerInput::neutral()
            .with_left_stick(-127, 0)
            .with_left_trigger_digital(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &fresh_digital_l);

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::GuardReflect);
    assert_eq!(
        player.velocity.x,
        source_general_grounded_friction_velocity(468, FighterProfile::falcon_like(), common),
        "ftCo_Turn_IASA calls ftCo_80091A4C before normal guard; fresh digital LR enters ftCo_800939B4/GuardReflect and then GuardReflect_Phys applies ground traction"
    );
}

#[test]
fn fresh_guard_reflect_resolves_to_guard_through_source_guard_on_animation_while_shield_held() {
    let profile = FighterProfile {
        action_frames: FighterActionFrames {
            guard_on_total_frames: 2,
            ..FighterActionFrames::falcon_like()
        },
        ..FighterProfile::falcon_like()
    };
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let common = MeleeCommonData::provisional_mole();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];
    let fresh_digital_r = [
        PlayerInput::neutral()
            .with_left_stick(121, -37)
            .with_right_trigger_digital(true),
        PlayerInput::neutral(),
    ];
    let held_digital_r = [
        PlayerInput::neutral().with_right_trigger_digital(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    for frame in 1..=common.dash_early_action_window {
        step_world(&mut world, Frame(frame as u32), &dash_right);
    }
    step_world(
        &mut world,
        Frame((common.dash_early_action_window + 1) as u32),
        &fresh_digital_r,
    );
    assert_eq!(world.players()[0].motion_state, MotionState::GuardReflect);

    step_world(
        &mut world,
        Frame((common.dash_early_action_window + 2) as u32),
        &held_digital_r,
    );
    assert_eq!(world.players()[0].motion_state, MotionState::GuardReflect);

    for frame in (common.dash_early_action_window + 3)..=(common.dash_early_action_window + 9) {
        step_world(&mut world, Frame(frame as u32), &held_digital_r);
    }

    assert_eq!(
        world.players()[0].motion_state,
        MotionState::Guard,
        "source GuardOn has eight frames, so fresh GuardReflect resolves through that source action length rather than an invented profile fallback"
    );
}

#[test]
fn fresh_guard_reflect_stays_active_on_last_guard_on_source_frame() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.grounded = true;
    player.motion_state = MotionState::GuardReflect;
    player.motion_state_alias = Some(MotionState::GuardReflect);
    player.melee_action_state_id = Some(MeleeActionStateId::new(182));
    player.source_action_total_frames = 8;
    player.motion_frame = 6;
    player.set_source_motion_anim_frame(6.0);
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(0),
        &[
            PlayerInput::neutral().with_left_trigger_digital(true),
            PlayerInput::neutral(),
        ],
    );

    assert_eq!(
        world.players()[0].motion_state,
        MotionState::GuardReflect,
        "ftCo_80093A50 fresh-starts GuardReflect at frame zero, so frame 7 is still represented as GuardReflect; it resolves only after the next GuardReflect_Anim/GuardOn_Anim advance reaches the action end"
    );
}

#[test]
fn guard_reflect_roll_applies_escape_root_motion_on_transition_tick() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.motion_state = MotionState::GuardReflect;
    player.motion_state_alias = Some(MotionState::GuardReflect);
    player.melee_action_state_id = Some(MeleeActionStateId::new(182));
    player.motion_frame = 0;
    player.grounded = true;
    player.facing = -1;
    player.ground_velocity_x = milli_to_source_units(388);
    player.velocity.x = 388;
    assert!(world.set_player_state_for_diagnostic(0, player));
    world.set_input_history_for_diagnostic(
        [
            PlayerInput::neutral().with_right_trigger_digital(true),
            PlayerInput::neutral(),
        ],
        [MeleeInputTimers::expired(), MeleeInputTimers::expired()],
    );
    let shield_roll = [
        PlayerInput::neutral()
            .with_left_stick(-127, 0)
            .with_right_trigger_digital(true),
        PlayerInput::neutral(),
    ];
    let expected_velocity = -source_root_motion_delta_milli_for_profile(
        MotionState::EscapeF,
        2,
        FighterProfile::falcon_like(),
    );

    step_world(&mut world, Frame(0), &shield_roll);

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::EscapeF);
    assert_eq!(
        player.velocity.x, expected_velocity,
        "GuardReflect_IASA enters EscapeF through ftCo_8009917C, then the same source tick runs Escape_Phys/root motion"
    );
}

#[test]
fn guard_reflect_roll_uses_ftanim_model_scaled_transn_offset() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.motion_state = MotionState::GuardReflect;
    player.motion_state_alias = Some(MotionState::GuardReflect);
    player.melee_action_state_id = Some(MeleeActionStateId::new(182));
    player.motion_frame = 0;
    player.grounded = true;
    player.facing = -1;
    player.ground_velocity_x = milli_to_source_units(388);
    player.velocity.x = 388;
    assert!(world.set_player_state_for_diagnostic(0, player));
    world.set_input_history_for_diagnostic(
        [
            PlayerInput::neutral().with_right_trigger_digital(true),
            PlayerInput::neutral(),
        ],
        [MeleeInputTimers::expired(), MeleeInputTimers::expired()],
    );
    let shield_roll = [
        PlayerInput::neutral()
            .with_left_stick(-127, 0)
            .with_right_trigger_digital(true),
        PlayerInput::neutral(),
    ];
    let raw_delta = source_root_motion_delta(MotionState::EscapeF, 2)
        .expect("EscapeF frame 2 TransN delta should be extracted");
    let falcon_model_scaling = 0.9700000286102295_f32;
    let expected_velocity = -source_units_to_milli(raw_delta.z * falcon_model_scaling);

    step_world(&mut world, Frame(0), &shield_roll);

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::EscapeF);
    assert_eq!(
        player.velocity.x, expected_velocity,
        "ftAnim_8006E054 multiplies x68C_transNPos by ftCommon_GetModelScale before ft_80085030 consumes x6A4_transNOffset"
    );
}

#[test]
fn guard_reflect_with_held_lr_can_pass_through_soft_platform() {
    let mut world = World::for_two_players();
    let stage = world.stage();
    let platform = stage.soft_platforms[0];
    let common = MeleeCommonData::provisional_mole();
    let mut player = world.players()[0];
    player.position = Vec2 {
        x: platform.left_x + (platform.right_x - platform.left_x) / 2,
        y: platform.y,
    };
    player.source_position = mole_core::SourceVec2::from_milli(player.position);
    player.velocity = Vec2 { x: 495, y: 0 };
    player.ground_velocity_x = milli_to_source_units(495);
    player.set_motion_state_alias(MotionState::GuardReflect);
    player.melee_action_state_id = Some(MeleeActionStateId::new(182));
    player.source_action_total_frames = 8;
    player.motion_frame = 0;
    player.grounded = true;
    player.set_source_floor_for_diagnostic(
        Some(1),
        Some(source_floor_line_for_surface_at_x(
            stage,
            platform,
            player.source_position.x,
        )),
    );
    assert!(world.set_player_state_for_diagnostic(0, player));
    world.set_input_history_for_diagnostic(
        [
            PlayerInput::neutral()
                .with_right_trigger_digital(true)
                .with_left_stick(89, 0),
            PlayerInput::neutral(),
        ],
        *world.input_timers(),
    );
    let held_lr_down = [
        PlayerInput::neutral()
            .with_right_trigger_digital(true)
            .with_left_stick(89, -common.platform_pass_y),
        PlayerInput::neutral(),
    ];
    let snapshot = world
        .melee_input_snapshot(0, held_lr_down[0])
        .expect("diagnostic input snapshot should exist");
    let facts = snapshot.facts(common.input_thresholds());
    assert!(facts.source_held.lr());
    assert!(facts.shield_held);
    assert_eq!(snapshot.y_tap_timer, 0);
    assert_eq!(world.players()[0].motion_state, MotionState::GuardReflect);
    assert_eq!(world.players()[0].position.y, platform.y);
    let expected_velocity_x = source_air_drift_velocity(player.velocity.x, 89, player.profile);
    let expected_position_x = player.position.x + expected_velocity_x;

    step_world(&mut world, Frame(0), &held_lr_down);

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::Pass);
    assert!(!player.grounded);
    assert_eq!(
        player.velocity.x, expected_velocity_x,
        "ftCo_8009A228 enters Pass during GuardReflect_IASA, then the same source frame runs Pass_Phys air drift"
    );
    assert_eq!(player.position.x, expected_position_x);
    assert_eq!(
        player.velocity.y,
        source_units_to_milli(common.pass_initial_y_velocity - player.profile.gravity)
    );
    assert_eq!(
        player.position.y,
        platform.y + source_units_to_milli(common.pass_initial_y_velocity - player.profile.gravity)
    );
}

#[test]
fn dash_state_late_window_opposite_dash_tap_beats_guard() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let shield_dash_left = [
        PlayerInput::neutral()
            .with_left_stick(-dash_stick_x(), 0)
            .with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    for frame in 1..=4 {
        step_world(&mut world, Frame(frame), &neutral);
    }
    step_world(&mut world, Frame(5), &shield_dash_left);

    assert_eq!(world.players()[0].motion_state, MotionState::Turn);
    assert_eq!(world.players()[0].facing, 1);
    assert_eq!(world.input_timers()[0].x_tap, 0);
}

#[test]
fn dash_state_z_grab_enters_catch_dash() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
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
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
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
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];
    let forward_attack = [
        PlayerInput::neutral()
            .with_left_stick(dash_stick_x(), 0)
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
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
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
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];
    let dash_attack = [
        PlayerInput::neutral().with_attack(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    for frame in 1..=4 {
        step_world(&mut world, Frame(frame), &dash_right);
    }
    step_world(&mut world, Frame(5), &dash_attack);

    assert_eq!(world.players()[0].motion_state, MotionState::AttackDash);
}

#[test]
fn dash_holding_forward_exits_to_run_after_falcon_dash_cmd_var_gate() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
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
fn dash_holding_forward_waits_for_source_cmd_var0_before_run_gate() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    for frame in 1..=14 {
        step_world(&mut world, Frame(frame), &dash_right);
        assert_eq!(
            world.players()[0].motion_state,
            MotionState::Dash,
            "Falcon Dash should not enter Run before the zero-based frame-15 cmd_var[0] script gate"
        );
        assert_eq!(world.players()[0].motion_cmd_var0, 0);
    }

    step_world(&mut world, Frame(15), &dash_right);

    assert_eq!(
        world.players()[0].motion_state,
        MotionState::Run,
        "ftCo_Dash_IASA reaches fn_800CA5F0 only after Dash cmd_var[0] is set"
    );
}

#[test]
fn dash_neutral_falls_back_on_animation_completion_not_profile_dash_frames() {
    let profile = FighterProfile {
        ground_friction: 0.01,
        ..FighterProfile::falcon_like()
    };
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let dash_right = [
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    step_world(&mut world, Frame(0), &dash_right);
    for frame in 1..=27 {
        step_world(&mut world, Frame(frame), &neutral);
        assert_eq!(
            world.players()[0].motion_state,
            MotionState::Dash,
            "the entry's explicit ftAnim_8006EBA4 evaluation consumes the first AObj advance, so the remaining 27 global p1 evaluations keep Falcon's 29-frame Dash active"
        );
    }

    step_world(&mut world, Frame(28), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::Wait);
    assert!(
        world.players()[0].velocity.x > 0,
        "ft_8008A2BC falls back to Wait without clearing carried gr_vel"
    );
}

#[test]
fn completed_dash_runs_wait_physics_on_the_same_frame() {
    let profile = FighterProfile::falcon_like();
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let mut player = PlayerState::new_with_profile(0, 0, -1, profile);
    player.motion_state = MotionState::Dash;
    player.motion_frame = profile.action_frames.dash_total_frames - 1;
    player.grounded = true;
    player.ground_velocity_x = milli_to_source_units(1_993);
    player.velocity.x = 1_993;
    world.set_player_state_for_diagnostic(0, player);

    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    step_world(&mut world, Frame(0), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::Wait);
    assert_eq!(
        world.players()[0].velocity.x, 1_833,
        "ftCo_Dash_Anim falls through ft_8008A2BC to Wait, then Wait_Phys/ft_80084F3C applies high-speed ground traction in the same source frame"
    );
    assert_eq!(
        world.players()[0].position.x,
        1_833,
        "the frame's ground translation uses the Wait-traction-adjusted gr_vel"
    );
}

#[test]
fn dash_anim_completion_is_checked_after_source_frame_advance() {
    let profile = FighterProfile::falcon_like();
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let mut player = PlayerState::new_with_profile(0, 0, -1, profile);
    player.motion_state = MotionState::Dash;
    player.motion_frame = profile.action_frames.dash_total_frames - 2;
    player.grounded = true;
    player.ground_velocity_x = milli_to_source_units(1_993);
    player.velocity.x = 1_993;
    world.set_player_state_for_diagnostic(0, player);

    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    step_world(&mut world, Frame(0), &neutral);

    assert_eq!(
        world.players()[0].motion_state,
        MotionState::Wait,
        "Fighter_procUpdate advances ftAnim_8006EBA4 before ftCo_Dash_Anim, so Falcon Dash frame 27 reaches animation completion and falls back to Wait in the same source tick"
    );
    assert_eq!(
        world.players()[0].velocity.x,
        1_833,
        "Wait physics runs after the Dash anim callback fallback in the same source tick"
    );
}

#[test]
fn released_walk_runs_wait_physics_on_the_same_frame() {
    let profile = FighterProfile::falcon_like();
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let mut player = PlayerState::new_with_profile(0, 0, -1, profile);
    player.motion_state = MotionState::WalkMiddle;
    player.grounded = true;
    player.ground_velocity_x = milli_to_source_units(609);
    player.velocity.x = 609;
    world.set_player_state_for_diagnostic(0, player);

    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    step_world(&mut world, Frame(0), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::Wait);
    assert_eq!(
        world.players()[0].velocity.x, 529,
        "ftCo_Walk_IASA can fall through to Wait, then Wait_Phys/ft_80084F3C applies traction in the same source frame"
    );
    assert_eq!(
        world.players()[0].position.x,
        529,
        "the frame's ground translation uses the Wait-traction-adjusted gr_vel"
    );
}

#[test]
fn landing_turn_iasa_runs_turn_physics_on_the_same_frame() {
    let profile = FighterProfile::falcon_like();
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let mut player = PlayerState::new_with_profile(0, 0, 1, profile);
    player.motion_state = MotionState::Landing;
    player.motion_frame = profile.normal_landing_lag_ticks;
    player.grounded = true;
    player.ground_velocity_x = milli_to_source_units(390);
    player.velocity.x = 390;
    world.set_player_state_for_diagnostic(0, player);

    let turn_left = [
        PlayerInput::neutral().with_left_stick(-44, -25),
        PlayerInput::neutral(),
    ];
    step_world(&mut world, Frame(0), &turn_left);

    assert_eq!(world.players()[0].motion_state, MotionState::Turn);
    assert_eq!(
        world.players()[0].velocity.x, 310,
        "ftCo_Landing_IASA can enter Turn, then Turn_Phys/ft_80084F3C applies traction in the same source frame"
    );
    assert_eq!(
        world.players()[0].position.x,
        310,
        "the frame's ground translation uses the Turn-traction-adjusted gr_vel"
    );
}

#[test]
fn completed_landing_fall_special_dash_runs_wait_input_and_dash_physics_on_the_same_frame() {
    let profile = FighterProfile::falcon_like();
    let common = MeleeCommonData::provisional_mole();
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let mut player = PlayerState::new_with_profile(0, 0, -1, profile);
    player.motion_state = MotionState::LandingFallSpecial;
    player.motion_frame = common.escapeair_landing_lag_ticks - 1;
    player.grounded = true;
    player.ground_velocity_x = milli_to_source_units(-648);
    player.velocity.x = -648;
    world.set_player_state_for_diagnostic(0, player);

    let dash_left = [
        PlayerInput::neutral().with_left_stick(-127, 0),
        PlayerInput::neutral(),
    ];
    step_world(&mut world, Frame(0), &dash_left);

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    assert_eq!(
        world.players()[0].position.x, -648,
        "ftCo_Landing_Anim can fall through ft_8008A2BC to Wait before input/phys; the entry frame translates with the carried gr_vel, not LandingFallSpecial traction"
    );
    assert_eq!(
        world.players()[0].velocity.x, -2_000,
        "Wait_IASA enters Dash, then Dash_Phys consumes mv.co.dash.x0 without adding the stale LandingFallSpecial traction"
    );
}

#[test]
fn completed_landing_air_dash_runs_wait_input_on_completion_frame() {
    let profile = FighterProfile::falcon_like();
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let mut player = PlayerState::new_with_profile(0, 0, -1, profile);
    player.motion_state = MotionState::LandingAirN;
    player.motion_frame = profile.landing_air_n_lag_ticks - 1;
    player.landing_lag_ticks = profile.landing_air_n_lag_ticks;
    player.grounded = true;
    player.ground_velocity_x = milli_to_source_units(-304);
    player.velocity.x = -304;
    world.set_player_state_for_diagnostic(0, player);

    let dash_left = [
        PlayerInput::neutral()
            .with_left_stick(-127, 0)
            .with_ucf_dashback_amendment(true),
        PlayerInput::neutral(),
    ];
    step_world(&mut world, Frame(0), &dash_left);

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    assert_eq!(
        world.players()[0].position.x, -304,
        "ftCo_LandingAir_Anim calls ftCo_Landing_Anim, so completion must fall through ft_8008A2BC to Wait before same-frame input/phys"
    );
    assert_eq!(
        world.players()[0].velocity.x, -2_000,
        "Wait_IASA should enter Dash on the LandingAir completion frame instead of spending a stale Wait frame"
    );
}

#[test]
fn completed_landing_air_analog_trigger_below_deadzone_stays_wait() {
    let profile = FighterProfile::falcon_like();
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let mut player = PlayerState::new_with_profile(0, 0, 1, profile);
    player.motion_state = MotionState::LandingAirN;
    player.motion_frame = profile.landing_air_n_lag_ticks - 1;
    player.landing_lag_ticks = profile.landing_air_n_lag_ticks;
    player.grounded = true;
    player.ground_velocity_x = milli_to_source_units(194);
    player.velocity.x = 194;
    world.set_player_state_for_diagnostic(0, player);

    let light_analog_trigger = [
        PlayerInput::neutral().with_left_trigger_analog(51),
        PlayerInput::neutral(),
    ];
    step_world(&mut world, Frame(0), &light_analog_trigger);

    assert_eq!(
        world.players()[0].motion_state,
        MotionState::Wait,
        "Fighter_Spaghetti_8006AD10 clears x650 at or below p_ftCommonData->x10 before held_inputs can receive HSD_PAD_LR"
    );
    assert_eq!(
        world.players()[0].velocity.x,
        114,
        "Wait_Phys should apply source ground friction instead of spending the frame in GuardOn"
    );
}

#[test]
fn wait_iasa_full_analog_trigger_enters_guard_on_and_runs_guard_physics() {
    let common = MeleeCommonData::provisional_mole();
    let profile = FighterProfile::falcon_like();
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let mut player = PlayerState::new_with_profile(0, 0, 1, profile);
    player.motion_state = MotionState::Wait;
    player.grounded = true;
    player.ground_velocity_x = milli_to_source_units(357);
    player.source_self_velocity_x = milli_to_source_units(357);
    player.velocity.x = 357;
    world.set_player_state_for_diagnostic(0, player);

    step_world(
        &mut world,
        Frame(0),
        &[
            PlayerInput::neutral().with_left_trigger_analog(255),
            PlayerInput::neutral(),
        ],
    );

    let expected_velocity = source_general_grounded_friction_velocity(357, profile, common);
    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::GuardOn);
    assert_eq!(
        player.velocity.x, expected_velocity,
        "ftCo_Wait_IASA can enter GuardOn through ftCo_80091A4C; the frame then runs GuardOn_Phys/ft_80084F3C"
    );
    assert_eq!(
        player.position.x, expected_velocity,
        "same-frame GuardOn physics should drive this tick's ground translation from Wait"
    );
}

#[test]
fn walk_iasa_analog_trigger_above_deadzone_below_z_shield_enters_guard_on() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.grounded = true;
    player.set_motion_state_alias(MotionState::WalkMiddle);
    player.position = Vec2 { x: 0, y: 0 };
    player.ground_velocity_x = milli_to_source_units(520);
    player.source_self_velocity_x = milli_to_source_units(520);
    player.velocity = Vec2 { x: 520, y: 0 };
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(0),
        &[
            PlayerInput::neutral()
                .with_left_stick(40, 0)
                .with_left_trigger_analog(84),
            PlayerInput::neutral(),
        ],
    );

    let player = world.players()[0];
    assert_eq!(
        player.motion_state,
        MotionState::GuardOn,
        "ftCo_80091A4C tests fp->input.held_inputs & HSD_PAD_LR; Fighter_Spaghetti_8006AD10 sets HSD_PAD_LR for any post-deadzone x650, not only x650 >= p_ftCommonData->x14"
    );
    assert_eq!(
        player.velocity.x, 440,
        "the same tick should run GuardOn_Phys/ft_80084F3C instead of Walk_Phys acceleration"
    );
    assert_eq!(player.position.x, 440);
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

    assert_eq!(side.players()[0].motion_state, MotionState::SpecialSStart);
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
fn run_direct_same_direction_run_input_handoffs_to_run_preserving_anim_frame() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.motion_state = MotionState::RunDirect;
    player.motion_frame = 7;
    player.facing = 1;
    player.ground_velocity_x = player.profile.dash_run_terminal_velocity;
    player.velocity.x = source_units_to_milli(player.profile.dash_run_terminal_velocity);
    assert!(world.set_player_state_for_diagnostic(0, player));
    let run_right = [
        PlayerInput::neutral().with_left_stick(run_stick_x(), 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &run_right);

    assert_eq!(
        world.players()[0].motion_state,
        MotionState::Run,
        "ftCo_RunDirect_IASA reaches fn_800CA698 when same-facing run input is held"
    );
    assert_eq!(
        world.players()[0].motion_frame,
        7,
        "fn_800CA698 enters Run with the current animation frame"
    );
}

#[test]
fn run_direct_release_exits_to_wait_through_ft_8008a244() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.motion_state = MotionState::RunDirect;
    player.motion_frame = 3;
    player.facing = 1;
    player.ground_velocity_x = player.profile.dash_run_terminal_velocity;
    player.velocity.x = source_units_to_milli(player.profile.dash_run_terminal_velocity);
    assert!(world.set_player_state_for_diagnostic(0, player));
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    step_world(&mut world, Frame(0), &neutral);

    assert_eq!(
        world.players()[0].motion_state,
        MotionState::Wait,
        "ftCo_RunDirect_IASA falls through to ft_8008A244 when the stick leaves the source run gate"
    );
    assert!(
        world.players()[0].velocity.x > 0,
        "ft_8008A244 changes to Wait without clearing carried gr_vel"
    );
}

#[test]
fn dash_releasing_to_neutral_exits_to_wait_after_falcon_dash_animation_completion() {
    let profile = FighterProfile {
        ground_friction: 0.01,
        ..FighterProfile::falcon_like()
    };
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let dash_right = [
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    step_world(&mut world, Frame(0), &dash_right);
    for frame in 1..=29 {
        step_world(&mut world, Frame(frame), &neutral);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::Wait);
    assert!(world.players()[0].velocity.x > 0);

    let wait_entry_velocity = world.players()[0].velocity.x;
    step_world(&mut world, Frame(30), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::Wait);
    assert!(world.players()[0].velocity.x > 0);
    assert!(world.players()[0].velocity.x < wait_entry_velocity);
}

#[test]
fn holding_aged_opposite_after_moonwalk_exits_dash_to_wait_not_walk() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
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
        PlayerInput::neutral().with_left_stick(-dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_right);
    step_world(&mut world, Frame(1), &soft_left);
    step_world(&mut world, Frame(2), &mid_left);
    step_world(&mut world, Frame(3), &near_left);
    for frame in 4..=27 {
        step_world(&mut world, Frame(frame), &full_left);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    step_world(&mut world, Frame(28), &full_left);

    assert_eq!(world.players()[0].motion_state, MotionState::Turn);
    assert_eq!(world.players()[0].facing, 1);
    assert_ne!(world.players()[0].velocity.x, 0);
}

#[test]
fn wait_after_moonwalk_carry_slides_under_ground_friction_instead_of_snapping_to_walk() {
    let mut world = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
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
        PlayerInput::neutral().with_left_stick(-dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    step_world(&mut world, Frame(0), &dash_right);
    step_world(&mut world, Frame(1), &soft_left);
    step_world(&mut world, Frame(2), &mid_left);
    step_world(&mut world, Frame(3), &near_left);
    for frame in 4..=27 {
        step_world(&mut world, Frame(frame), &full_left);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
    step_world(&mut world, Frame(28), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::Wait);
    let carried_velocity = world.players()[0].velocity.x;

    step_world(&mut world, Frame(29), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::Wait);
    assert!(world.players()[0].velocity.x.abs() < carried_velocity.abs());
    assert_ne!(world.players()[0].velocity.x, 0);
}

#[test]
fn run_neutral_stick_enters_run_brake_without_zeroing_velocity() {
    let mut world = World::for_two_players();
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    advance_player_to_run(&mut world);
    let run_velocity = world.players()[0].velocity.x;

    step_world(&mut world, Frame(17), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::RunBrake);
    assert_eq!(world.players()[0].facing, 1);
    assert!(world.players()[0].velocity.x > 0);
    assert!(world.players()[0].velocity.x < run_velocity);
}

#[test]
fn dash_neutral_and_run_brake_use_x60_ground_friction_not_generic_high_speed_traction() {
    let dash_profile = FighterProfile {
        action_frames: FighterActionFrames {
            dash_total_frames: 4,
            dash_cmd_var0_set_frame: 3,
            ..FighterActionFrames::falcon_like()
        },
        dash_frames: 2,
        walk_max_velocity: 0.1,
        ground_friction: 0.08,
        ..FighterProfile::falcon_like()
    };
    let run_profile = FighterProfile {
        action_frames: FighterActionFrames {
            dash_total_frames: 3,
            dash_cmd_var0_set_frame: 1,
            ..FighterActionFrames::falcon_like()
        },
        dash_frames: 1,
        walk_max_velocity: 0.1,
        ground_friction: 0.08,
        ..FighterProfile::falcon_like()
    };
    let common = MeleeCommonData {
        run_ground_friction_multiplier: 1.25,
        high_speed_ground_friction_multiplier: 3.0,
        ..MeleeCommonData::provisional_mole()
    };
    let mut dash_release = World::for_two_players_on_stage_with_profiles_and_common_data(
        StageProfile::battlefield_test(),
        [dash_profile; 2],
        common,
    );
    let mut run_release = World::for_two_players_on_stage_with_profiles_and_common_data(
        StageProfile::battlefield_test(),
        [run_profile; 2],
        common,
    );
    let dash_right = [
        PlayerInput::neutral().with_left_stick(127, 0),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    step_world(&mut dash_release, Frame(0), &dash_right);
    let dash_entry_velocity = dash_release.players()[0].velocity.x;
    step_world(&mut dash_release, Frame(1), &neutral);

    assert_eq!(
        dash_release.players()[0].velocity.x,
        source_dash_phys_velocity(
            dash_entry_velocity,
            0,
            dash_release.players()[0].profile,
            common,
        )
    );
    step_world(&mut dash_release, Frame(2), &neutral);

    assert_eq!(
        dash_release.players()[0].velocity.x,
        dash_entry_velocity - 200
    );

    step_world(&mut run_release, Frame(0), &dash_right);
    step_world(&mut run_release, Frame(1), &dash_right);
    assert_eq!(run_release.players()[0].motion_state, MotionState::Run);
    let run_velocity = run_release.players()[0].velocity.x;
    step_world(&mut run_release, Frame(2), &neutral);

    assert_eq!(run_release.players()[0].motion_state, MotionState::RunBrake);
    assert_eq!(run_release.players()[0].velocity.x, run_velocity - 100);
}

#[test]
fn run_acceleration_uses_source_x5c_remaining_velocity_taper() {
    let profile = FighterProfile {
        action_frames: FighterActionFrames {
            dash_total_frames: 3,
            dash_cmd_var0_set_frame: 1,
            ..FighterActionFrames::falcon_like()
        },
        dash_frames: 1,
        dash_initial_velocity: 0.5,
        dash_run_acceleration_a: 0.01,
        dash_run_acceleration_b: 0.0,
        dash_run_terminal_velocity: 1.27,
        ground_friction: 0.05,
        ground_max_horizontal_velocity: 3.0,
        ..FighterProfile::falcon_like()
    };
    let common = MeleeCommonData {
        run_accel_taper: 0.4,
        run_ground_friction_multiplier: 1.0,
        ..MeleeCommonData::provisional_mole()
    };
    let mut world = World::for_two_players_on_stage_with_profiles_and_common_data(
        StageProfile::battlefield_test(),
        [profile; 2],
        common,
    );
    let run_right = [
        PlayerInput::neutral().with_left_stick(127, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &run_right);
    step_world(&mut world, Frame(1), &run_right);
    assert_eq!(world.players()[0].motion_state, MotionState::Run);
    assert_eq!(world.players()[0].velocity.x, 502);
    let source_velocity = world.players()[0].ground_velocity_x;

    step_world(&mut world, Frame(2), &run_right);

    assert_eq!(world.players()[0].motion_state, MotionState::Run);
    assert_eq!(
        world.players()[0].velocity.x,
        source_run_phys_render_velocity_from_source(source_velocity, 127, profile, common)
    );
}

#[test]
fn run_opposite_stick_enters_turn_run_with_source_turnrun_phys_on_entry_tick() {
    let mut world = World::for_two_players();
    let dash_left = [
        PlayerInput::neutral().with_left_stick(-dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];

    advance_player_to_run(&mut world);
    let run_velocity = world.players()[0].velocity.x;

    step_world(&mut world, Frame(17), &dash_left);
    let expected_velocity = source_turn_run_phys_velocity(
        run_velocity,
        -dash_stick_x() as i32,
        1,
        world.players()[0].profile,
        world.common_data(),
    );

    assert_eq!(world.players()[0].motion_state, MotionState::TurnRun);
    assert_eq!(world.players()[0].facing, 1);
    assert!(world.players()[0].velocity.x > 0);
    assert_eq!(world.players()[0].velocity.x, expected_velocity);
}

#[test]
fn run_turnaround_uses_source_x38_turn_run_threshold_not_x58_run_threshold() {
    let mut world = World::for_two_players();
    let turn_run_left = [
        PlayerInput::neutral().with_left_stick(-turn_run_stick_x(), 0),
        PlayerInput::neutral(),
    ];

    advance_player_to_run(&mut world);

    step_world(&mut world, Frame(17), &turn_run_left);

    assert_eq!(world.players()[0].motion_state, MotionState::TurnRun);
    assert_eq!(world.players()[0].facing, 1);
    assert!(world.players()[0].velocity.x > 0);
}

#[test]
fn turn_run_keeps_old_facing_until_velocity_crosses_zero() {
    let mut world = World::for_two_players();
    let dash_left = [
        PlayerInput::neutral().with_left_stick(-dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];

    advance_player_to_run(&mut world);
    step_world(&mut world, Frame(17), &dash_left);
    step_world(&mut world, Frame(18), &dash_left);

    assert_eq!(world.players()[0].motion_state, MotionState::TurnRun);
    assert_eq!(world.players()[0].facing, 1);
    assert!(world.players()[0].velocity.x > 0);
}

#[test]
fn turn_run_overshoot_keeps_accel_minus_run_friction_like_ftco_turnrun_phys() {
    let mut world = World::for_two_players();
    let profile = world.players()[0].profile;
    let common = world.common_data();
    let stick_x = 125;
    let current_velocity =
        source_stick_scaled_velocity(stick_x, profile.dash_run_terminal_velocity);
    let mut player = world.players()[0];
    player.motion_state = MotionState::TurnRun;
    player.motion_state_alias = Some(MotionState::TurnRun);
    player.source_action_key = Some(SourceActionKey::new("TurnRun"));
    player.melee_action_state_id = Some(MeleeActionStateId::new(19));
    player.motion_frame = profile.action_frames.turn_run_cmd_var1_frame + 5;
    player.motion_anim_frame_milli = i32::from(player.motion_frame) * 1_000;
    player.grounded = true;
    player.facing = 1;
    player.turn_facing_after = 1;
    player.turn_has_turned = true;
    player.turn_run_x14 = true;
    player.turn_run_accel_mul = -1;
    player.ground_velocity_x = milli_to_source_units(current_velocity);
    player.velocity.x = current_velocity;
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(0),
        &[
            PlayerInput::neutral().with_left_stick(stick_x as i8, 0),
            PlayerInput::neutral(),
        ],
    );

    let expected_velocity =
        source_turn_run_phys_velocity(current_velocity, stick_x, -1, profile, common);
    assert!(
        expected_velocity > current_velocity,
        "the decomp TurnRun overshoot branch continues past target velocity by accel minus x60 friction"
    );
    assert_eq!(world.players()[0].motion_state, MotionState::TurnRun);
    assert_eq!(world.players()[0].velocity.x, expected_velocity);
}

#[test]
fn turn_run_resume_from_velocity_crossing_holds_script_frame_one_more_tick() {
    let mut world = World::for_two_players();
    let dash_left = [
        PlayerInput::neutral().with_left_stick(-dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];

    advance_player_to_run(&mut world);
    step_world(&mut world, Frame(17), &dash_left);

    let mut frame = 18;
    while world.players()[0].velocity.x > 0 {
        step_world(&mut world, Frame(frame), &dash_left);
        frame += 1;
    }

    let paused_frame = world.players()[0].motion_frame;
    assert_eq!(world.players()[0].motion_state, MotionState::TurnRun);
    assert_eq!(
        paused_frame,
        world.players()[0]
            .profile
            .action_frames
            .turn_run_cmd_var1_frame
    );
    assert_eq!(world.players()[0].facing, 1);

    step_world(&mut world, Frame(frame), &dash_left);

    assert_eq!(world.players()[0].motion_state, MotionState::TurnRun);
    assert_eq!(world.players()[0].motion_frame, paused_frame);
    assert_eq!(world.players()[0].facing, -1);

    step_world(&mut world, Frame(frame + 1), &dash_left);

    assert_eq!(world.players()[0].motion_state, MotionState::TurnRun);
    assert_eq!(world.players()[0].motion_frame, paused_frame + 1);
}

#[test]
fn turn_run_shield_input_does_not_cancel_before_source_jump_iasa() {
    let mut world = World::for_two_players();
    let dash_left = [
        PlayerInput::neutral().with_left_stick(-dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];
    let shield_left = [
        PlayerInput::neutral()
            .with_left_stick(-dash_stick_x(), 0)
            .with_left_trigger_analog(70),
        PlayerInput::neutral(),
    ];

    advance_player_to_run(&mut world);
    step_world(&mut world, Frame(17), &dash_left);
    step_world(&mut world, Frame(18), &shield_left);

    assert_eq!(world.players()[0].motion_state, MotionState::TurnRun);
    assert_eq!(world.players()[0].facing, 1);
}

#[test]
fn run_entered_from_turn_run_keeps_source_no_interrupt_window() {
    let mut world = World::for_two_players();
    let dash_left = [
        PlayerInput::neutral().with_left_stick(-dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    advance_player_to_run(&mut world);
    step_world(&mut world, Frame(17), &dash_left);

    let mut run_entry_frame = None;
    for frame in 18..=120 {
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
fn run_x430_decrements_before_iasa_allows_turnrun_on_boundary_frame() {
    let profile = quick_turn_run_profile();
    let common = MeleeCommonData {
        run_turn_run_no_interrupt_frames: 1,
        ..MeleeCommonData::provisional_mole()
    };
    let mut world = World::for_two_players_on_stage_with_profiles_and_common_data(
        StageProfile::battlefield_test(),
        [profile; 2],
        common,
    );
    let right = [
        PlayerInput::neutral().with_left_stick(127, 0),
        PlayerInput::neutral(),
    ];
    let left = [
        PlayerInput::neutral().with_left_stick(-127, 0),
        PlayerInput::neutral(),
    ];

    advance_quick_profile_to_run(&mut world, 127);
    step_world(&mut world, Frame(2), &left);
    assert_eq!(world.players()[0].motion_state, MotionState::TurnRun);

    let mut frame = 3;
    let completion_limit = 4 + profile.action_frames.turn_run_total_frames as u32;
    while frame <= completion_limit && world.players()[0].motion_state != MotionState::Run {
        step_world(&mut world, Frame(frame), &left);
        frame += 1;
    }

    assert_eq!(world.players()[0].motion_state, MotionState::Run);
    assert_eq!(world.players()[0].run_no_interrupt_frames, 1);

    step_world(&mut world, Frame(frame), &right);

    assert_eq!(world.players()[0].motion_state, MotionState::TurnRun);
}

#[test]
fn turn_run_does_not_enter_run_before_source_animation_completion() {
    let profile = quick_turn_run_profile();
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let left = [
        PlayerInput::neutral().with_left_stick(-127, 0),
        PlayerInput::neutral(),
    ];

    advance_quick_profile_to_run(&mut world, 127);
    step_world(&mut world, Frame(2), &left);
    assert_eq!(world.players()[0].motion_state, MotionState::TurnRun);

    for frame in 3..=(2 + profile.action_frames.turn_run_total_frames as u32) {
        step_world(&mut world, Frame(frame), &left);
        assert_eq!(world.players()[0].motion_state, MotionState::TurnRun);
    }
}

#[test]
fn turn_run_completion_enters_run_through_source_x58_gate() {
    let profile = quick_turn_run_profile();
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let left = [
        PlayerInput::neutral().with_left_stick(-127, 0),
        PlayerInput::neutral(),
    ];

    advance_quick_profile_to_run(&mut world, 127);
    step_world(&mut world, Frame(2), &left);
    assert_eq!(world.players()[0].motion_state, MotionState::TurnRun);

    let mut run_entry_frame = None;
    for frame in 3..=(4 + profile.action_frames.turn_run_total_frames as u32) {
        step_world(&mut world, Frame(frame), &left);
        if world.players()[0].motion_state == MotionState::Run {
            run_entry_frame = Some(frame);
            break;
        }
    }
    assert!(run_entry_frame.is_some());

    assert_eq!(world.players()[0].motion_state, MotionState::Run);
    assert_eq!(world.players()[0].facing, -1);
    assert_eq!(
        world.players()[0].run_no_interrupt_frames,
        world.common_data().run_turn_run_no_interrupt_frames
    );
}

#[test]
fn turn_run_completion_uses_previous_frame_run_gate_before_current_input() {
    let profile = quick_turn_run_profile();
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let left = [
        PlayerInput::neutral().with_left_stick(-127, 0),
        PlayerInput::neutral(),
    ];
    let right = [
        PlayerInput::neutral().with_left_stick(127, 0),
        PlayerInput::neutral(),
    ];

    advance_quick_profile_to_run(&mut world, 127);
    step_world(&mut world, Frame(2), &left);
    assert_eq!(world.players()[0].motion_state, MotionState::TurnRun);

    let completion_gate_frame = {
        let mut frame = 3;
        loop {
            step_world(&mut world, Frame(frame), &left);
            assert_eq!(world.players()[0].motion_state, MotionState::TurnRun);
            if world.players()[0].motion_frame
                == profile
                    .action_frames
                    .turn_run_total_frames
                    .saturating_sub(1)
            {
                break frame;
            }
            frame += 1;
        }
    };

    assert_eq!(world.players()[0].facing, -1);
    step_world(&mut world, Frame(completion_gate_frame + 1), &right);

    assert_eq!(world.players()[0].motion_state, MotionState::Run);
    assert_eq!(world.players()[0].facing, -1);
    assert_eq!(
        world.players()[0].run_no_interrupt_frames,
        world.common_data().run_turn_run_no_interrupt_frames
    );
}

#[test]
fn turn_run_completion_into_run_uses_source_run_iasa_and_phys_handoff_tick() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.motion_state = MotionState::TurnRun;
    player.motion_state_alias = Some(MotionState::TurnRun);
    player.source_action_key = Some(SourceActionKey::new("TurnRun"));
    player.melee_action_state_id = Some(MeleeActionStateId::new(19));
    player.grounded = true;
    player.facing = 1;
    player.turn_facing_after = 1;
    player.turn_has_turned = true;
    player.turn_run_accel_mul = -1;
    player.turn_run_completion_pending = true;
    player.turn_run_completion_enters_run = true;
    player.ground_velocity_x = 2.788125;
    player.source_self_velocity_x = 2.788125;
    player.velocity.x = source_units_to_milli(2.788125);
    assert!(world.set_player_state_for_diagnostic(0, player));

    let opposite_stick = [
        PlayerInput::neutral().with_left_stick(-121, -38),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &opposite_stick);

    let player = world.players()[0];
    println!(
        "handoff debug: motion_state={:?} motion_frame={} motion_anim_frame_milli={} run_no_interrupt_frames={} ground_velocity_x={} ground_accel_x={} ground_accel_x2={} source_self_velocity_x={} velocity_x={}",
        player.motion_state,
        player.motion_frame,
        player.motion_anim_frame_milli,
        player.run_no_interrupt_frames,
        player.ground_velocity_x,
        player.ground_accel_x,
        player.ground_accel_x2,
        player.source_self_velocity_x,
        player.velocity.x,
    );

    assert_eq!(
        world.players()[0].motion_state,
        MotionState::Run,
        "ftCo_TurnRun_Anim enters Run before the same frame's Run_IASA/Run_Phys callbacks"
    );
    assert_eq!(
        world.players()[0].velocity.x,
        source_units_to_milli(2.1475),
        "the TurnRun->Run completion tick first applies Fighter_ChangeMotionState's source action-flag ground velocity clamp, then runs Run_Phys"
    );
}

#[test]
fn turn_run_completion_run_iasa_jump_wins_on_handoff_tick() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.motion_state = MotionState::TurnRun;
    player.motion_state_alias = Some(MotionState::TurnRun);
    player.source_action_key = Some(SourceActionKey::new("TurnRun"));
    player.melee_action_state_id = Some(MeleeActionStateId::new(19));
    player.grounded = true;
    player.facing = 1;
    player.turn_facing_after = 1;
    player.turn_has_turned = true;
    player.turn_run_accel_mul = -1;
    player.turn_run_completion_pending = true;
    player.turn_run_completion_enters_run = true;
    player.ground_velocity_x = 2.58375;
    player.source_self_velocity_x = 2.58375;
    player.velocity.x = source_units_to_milli(2.58375);
    assert!(world.set_player_state_for_diagnostic(0, player));

    let jump = [
        PlayerInput::neutral()
            .with_left_stick(125, -11)
            .with_jump_secondary(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &jump);

    assert_eq!(
        world.players()[0].motion_state,
        MotionState::KneeBend,
        "TurnRun_Anim installs Run, then the same frame's Run_IASA sees fresh Y"
    );
    assert_eq!(world.players()[0].motion_frame, 0);
    assert_eq!(world.players()[0].velocity.x, source_units_to_milli(2.14));
}

#[test]
fn turn_run_resume_advances_when_pause_started_after_velocity_crossing() {
    let profile = quick_turn_run_profile();
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let right = [
        PlayerInput::neutral().with_left_stick(127, 0),
        PlayerInput::neutral(),
    ];
    let mut player = world.players()[0];
    player.set_motion_state_alias(MotionState::TurnRun);
    player.motion_frame = profile.action_frames.turn_run_cmd_var1_frame;
    player.motion_anim_frame_milli = i32::from(player.motion_frame) * 1_000;
    player.motion_cmd_var1 = 0;
    player.turn_run_x14 = false;
    player.turn_run_resume_advances = false;
    player.turn_run_accel_mul = -1;
    player.turn_facing_after = 1;
    player.turn_has_turned = false;
    player.ground_velocity_x = 1.0;
    player.velocity.x = 1_000;
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(&mut world, Frame(0), &right);

    let paused = world.players()[0];
    assert_eq!(paused.motion_state, MotionState::TurnRun);
    assert_eq!(
        paused.motion_frame,
        profile.action_frames.turn_run_cmd_var1_frame
    );
    assert!(paused.turn_run_x14);
    assert!(paused.turn_run_resume_advances);

    step_world(&mut world, Frame(1), &right);

    let resumed = world.players()[0];
    assert_eq!(resumed.motion_state, MotionState::TurnRun);
    assert_eq!(
        resumed.motion_frame,
        profile.action_frames.turn_run_cmd_var1_frame + 1
    );
    assert_eq!(resumed.motion_cmd_var1, 0);
    assert!(
        !resumed.turn_run_resume_advances,
        "resume flag should be consumed once the paused script frame advances"
    );
    assert_eq!(
        resumed.facing, 1,
        "ftCo_TurnRun_Anim flips facing when the pause resumes"
    );
}

#[test]
fn full_run_turnaround_followthrough_can_chain_back_into_run_states() {
    let mut world = World::for_two_players();
    let dash_left = [
        PlayerInput::neutral().with_left_stick(-dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];
    let dash_right = [
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];

    advance_player_to_run(&mut world);

    let mut frame = 16;
    let mut saw_left_turn_run = false;
    while frame <= 60 {
        step_world(&mut world, Frame(frame), &dash_left);
        assert!(!matches!(
            world.players()[0].motion_state,
            MotionState::WalkSlow | MotionState::WalkMiddle | MotionState::WalkFast
        ));
        saw_left_turn_run |= world.players()[0].motion_state == MotionState::TurnRun;
        if saw_left_turn_run
            && world.players()[0].motion_state == MotionState::Run
            && world.players()[0].facing == -1
        {
            break;
        }
        frame += 1;
    }

    assert!(saw_left_turn_run);
    assert_eq!(world.players()[0].motion_state, MotionState::Run);
    assert_eq!(world.players()[0].facing, -1);

    frame += 1;
    let mut saw_right_turn_run = false;
    while frame <= 110 {
        step_world(&mut world, Frame(frame), &dash_right);
        assert!(!matches!(
            world.players()[0].motion_state,
            MotionState::WalkSlow | MotionState::WalkMiddle | MotionState::WalkFast
        ));
        saw_right_turn_run |= world.players()[0].motion_state == MotionState::TurnRun;
        if saw_right_turn_run
            && world.players()[0].motion_state == MotionState::Run
            && world.players()[0].facing == 1
        {
            break;
        }
        frame += 1;
    }

    assert!(saw_right_turn_run);
    assert_eq!(world.players()[0].motion_state, MotionState::Run);
    assert_eq!(world.players()[0].facing, 1);
}

#[test]
fn run_brake_forward_or_soft_stick_keeps_braking_until_source_exit() {
    let mut forward = World::for_two_players();
    let mut soft = World::for_two_players();
    let dash_right = [
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let soft_right = [
        PlayerInput::neutral().with_left_stick(40, 0),
        PlayerInput::neutral(),
    ];

    advance_player_to_run(&mut forward);
    advance_player_to_run(&mut soft);
    step_world(&mut forward, Frame(17), &neutral);
    step_world(&mut soft, Frame(17), &neutral);

    assert_eq!(forward.players()[0].motion_state, MotionState::RunBrake);
    assert_eq!(soft.players()[0].motion_state, MotionState::RunBrake);

    step_world(&mut forward, Frame(18), &dash_right);
    step_world(&mut soft, Frame(18), &soft_right);

    assert_eq!(forward.players()[0].motion_state, MotionState::RunBrake);
    assert_eq!(soft.players()[0].motion_state, MotionState::RunBrake);
}

#[test]
fn run_brake_cmd_var0_window_can_branch_to_turnrun_before_wait_or_walk() {
    let profile = quick_turn_run_profile();
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let left = [
        PlayerInput::neutral().with_left_stick(-127, 0),
        PlayerInput::neutral(),
    ];

    advance_quick_profile_to_run(&mut world, 127);
    step_world(&mut world, Frame(2), &neutral);
    assert_eq!(world.players()[0].motion_state, MotionState::RunBrake);

    let mut saw_turn_run = false;
    for frame in 3..=17 {
        step_world(&mut world, Frame(frame), &left);
        assert!(!matches!(
            world.players()[0].motion_state,
            MotionState::WalkSlow | MotionState::WalkMiddle | MotionState::WalkFast
        ));
        saw_turn_run |= world.players()[0].motion_state == MotionState::TurnRun;
        if saw_turn_run {
            break;
        }
    }

    assert!(saw_turn_run);
}

#[test]
fn run_brake_uses_extracted_profile_max_frames_when_available() {
    let profile = FighterProfile {
        max_run_brake_frames: Some(2),
        ground_friction: 0.001,
        ..FighterProfile::falcon_like()
    };
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    advance_player_to_run(&mut world);
    step_world(&mut world, Frame(17), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::RunBrake);
    assert!(world.players()[0].velocity.x > 0);

    step_world(&mut world, Frame(18), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::RunBrake);
    assert_eq!(world.players()[0].motion_frame, 1);
    assert!(world.players()[0].velocity.x > 0);

    step_world(&mut world, Frame(19), &neutral);

    assert_eq!(world.players()[0].motion_state, MotionState::Wait);
    assert_eq!(world.players()[0].motion_frame, 0);
    assert!(world.players()[0].velocity.x > 0);
}

#[test]
fn down_stick_from_wait_enters_squat() {
    let mut world = World::for_two_players();
    let down = [
        PlayerInput::neutral().with_left_stick(0, crouch_stick_y()),
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
        PlayerInput::neutral().with_left_stick(40, crouch_stick_y()),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &down_forward);

    assert_eq!(world.players()[0].motion_state, MotionState::Squat);
}

#[test]
fn dash_threshold_input_keeps_priority_over_crouch_from_wait() {
    let mut world = World::for_two_players();
    let dash_down_forward = [
        PlayerInput::neutral().with_left_stick(dash_stick_x(), crouch_stick_y()),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &dash_down_forward);

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
}

#[test]
fn crouch_has_priority_over_turn_from_wait() {
    let mut world = World::for_two_players();
    let down_back = [
        PlayerInput::neutral().with_left_stick(-40, crouch_stick_y()),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &down_back);

    assert_eq!(world.players()[0].motion_state, MotionState::Squat);
}

#[test]
fn squat_startup_does_not_release_before_squat_wait_but_jump_or_shield_take_priority() {
    let profile = FighterProfile {
        action_frames: FighterActionFrames {
            squat_total_frames: 2,
            ..FighterActionFrames::falcon_like()
        },
        ..FighterProfile::falcon_like()
    };
    let mut release = World::for_two_players_with_profiles([profile; 2]);
    let mut jump = World::for_two_players_with_profiles([profile; 2]);
    let mut shield = World::for_two_players_with_profiles([profile; 2]);
    let down = [
        PlayerInput::neutral().with_left_stick(0, crouch_stick_y()),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let down_jump = [
        PlayerInput::neutral()
            .with_left_stick(0, crouch_stick_y())
            .with_jump(true),
        PlayerInput::neutral(),
    ];
    let down_shield = [
        PlayerInput::neutral()
            .with_left_stick(0, crouch_stick_y())
            .with_left_trigger_analog(shield_analog()),
        PlayerInput::neutral(),
    ];

    step_world(&mut release, Frame(0), &down);
    step_world(&mut jump, Frame(0), &down);
    step_world(&mut shield, Frame(0), &down);

    step_world(&mut release, Frame(1), &neutral);
    step_world(&mut jump, Frame(1), &down_jump);
    step_world(&mut shield, Frame(1), &down_shield);

    assert_eq!(release.players()[0].motion_state, MotionState::Squat);
    assert_eq!(jump.players()[0].motion_state, MotionState::KneeBend);
    assert_eq!(shield.players()[0].motion_state, MotionState::GuardOn);
}

#[test]
fn squat_enters_squat_wait_after_profile_startup_frames() {
    let mut world = World::for_two_players();
    let down = [
        PlayerInput::neutral().with_left_stick(0, crouch_stick_y()),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &down);
    for frame in 1..=4 {
        step_world(&mut world, Frame(frame), &down);
    }

    assert_eq!(
        world.players()[0].motion_state,
        MotionState::Squat,
        "ftCo_Squat_Anim waits for the extracted Squat animation to finish; Falcon's source action has 8 sampled frames, not the old profile fallback of 4"
    );

    for frame in 5..=7 {
        step_world(&mut world, Frame(frame), &down);
    }

    assert_eq!(world.players()[0].motion_state, MotionState::SquatWait);
}

#[test]
fn squat_entry_preserves_carried_walk_velocity_for_same_frame_squat_physics() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.grounded = true;
    player.facing = -1;
    player.set_motion_state_alias(MotionState::WalkSlow);
    player.position = Vec2 { x: 0, y: 0 };
    player.source_position.x = 0.0;
    player.source_position.y = 0.0;
    player.ground_velocity_x = milli_to_source_units(-154);
    player.source_self_velocity_x = milli_to_source_units(-154);
    player.velocity = Vec2 { x: -154, y: 0 };
    assert!(world.set_player_state_for_diagnostic(0, player));
    let mut other = world.players()[1];
    other.position.x = 100_000;
    other.source_position.x = 100.0;
    assert!(world.set_player_state_for_diagnostic(1, other));

    step_world(
        &mut world,
        Frame(0),
        &[
            PlayerInput::neutral().with_left_stick(-57, -89),
            PlayerInput::neutral(),
        ],
    );

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::Squat);
    assert_eq!(
        player.velocity.x, -74,
        "ftCo_Squat_Enter does not clear gr_vel; Squat_Phys/ft_80084F3C applies normal traction to the carried WalkSlow velocity"
    );
    assert_eq!(
        player.position.x, -74,
        "the Squat entry tick should translate with the post-traction ground velocity"
    );
}

#[test]
fn squat_animation_completion_can_enter_squat_rv_same_tick_when_stick_released() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.grounded = true;
    player.set_motion_state_alias(MotionState::Squat);
    player.source_action_total_frames = 8;
    player.motion_frame = 6;
    player.set_source_motion_anim_frame(6.0);
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(
        &mut world,
        Frame(0),
        &[PlayerInput::neutral(), PlayerInput::neutral()],
    );

    assert_eq!(
        world.players()[0].motion_state,
        MotionState::SquatRv,
        "ftCo_Squat_Anim enters SquatWait when the Squat animation ends; the same tick can then run SquatWait_IASA and ftCo_SquatRv_CheckInput when lstick.y is above -x94"
    );
}

#[test]
fn squat_wait_release_uses_common_data_x94_hysteresis() {
    let profile = FighterProfile {
        action_frames: FighterActionFrames {
            squat_rv_total_frames: 2,
            ..FighterActionFrames::falcon_like()
        },
        ..FighterProfile::falcon_like()
    };
    let common = MeleeCommonData {
        crouch_y: 60,
        crouch_release_y: 20,
        ..MeleeCommonData::provisional_mole()
    };
    let mut world = World::for_two_players_on_stage_with_profiles_and_common_data(
        StageProfile::battlefield_test(),
        [profile; 2],
        common,
    );
    let mut player = world.players()[0];
    player.grounded = true;
    player.set_motion_state_alias(MotionState::SquatWait);
    assert!(world.set_player_state_for_diagnostic(0, player));
    let between_crouch_and_release = [
        PlayerInput::neutral().with_left_stick(0, -30),
        PlayerInput::neutral(),
    ];
    let released = [PlayerInput::neutral(), PlayerInput::neutral()];

    step_world(&mut world, Frame(0), &between_crouch_and_release);

    assert_eq!(world.players()[0].motion_state, MotionState::SquatWait);

    step_world(&mut world, Frame(1), &released);

    assert_current_action_returns_to_wait_after_frames(&mut world, 1, MotionState::SquatRv, 2);
}

#[test]
fn falcon_squat_rv_uses_extracted_action_animation_length() {
    let mut world = World::for_two_players();
    let down = [
        PlayerInput::neutral().with_left_stick(0, crouch_stick_y()),
        PlayerInput::neutral(),
    ];
    let released = [PlayerInput::neutral(), PlayerInput::neutral()];

    step_world(&mut world, Frame(0), &down);
    step_world(&mut world, Frame(1), &down);
    step_world(&mut world, Frame(2), &down);
    step_world(&mut world, Frame(3), &down);
    step_world(&mut world, Frame(4), &down);
    assert_eq!(world.players()[0].motion_state, MotionState::SquatWait);

    step_world(&mut world, Frame(5), &released);

    assert_current_action_returns_to_wait_after_frames(&mut world, 5, MotionState::SquatRv, 10);
}

#[test]
fn squat_rv_backward_walk_strength_input_does_not_enter_walk() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.set_motion_state_alias(MotionState::SquatRv);
    player.motion_frame = 5;
    player.grounded = true;
    player.facing = 1;
    assert!(world.set_player_state_for_diagnostic(0, player));
    let back_walk = [
        PlayerInput::neutral()
            .with_left_stick(-(MeleeCommonData::provisional_mole().walk_x + 10), 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &back_walk);

    let player = world.players()[0];
    assert_eq!(
        player.motion_state,
        MotionState::SquatRv,
        "ftCo_Walk_CheckInput from SquatRv requires lstick.x * facing_dir >= p_ftCommonData->x24"
    );
    assert_eq!(player.motion_frame, 6);
}

#[test]
fn completed_squat_rv_runs_wait_input_callback_on_same_frame() {
    let profile = FighterProfile::falcon_like();
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let mut player = PlayerState::new_with_profile(0, 0, 1, profile);
    player.set_motion_state_alias(MotionState::SquatRv);
    player.motion_frame = profile.action_frames.squat_rv_total_frames - 1;
    player.grounded = true;
    player.facing = 1;
    player.ground_velocity_x = milli_to_source_units(390);
    player.velocity.x = 390;
    assert!(world.set_player_state_for_diagnostic(0, player));
    let back_turn = [
        PlayerInput::neutral().with_left_stick(-127, 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &back_turn);

    let player = world.players()[0];
    assert_eq!(
        player.motion_state,
        MotionState::Turn,
        "ftCo_SquatRv_Anim calls ft_8008A2BC before the same frame's input callback, so Wait_IASA can enter Turn immediately"
    );
    assert_eq!(player.motion_frame, 0);
    assert_eq!(player.facing, 1);
    assert_eq!(player.turn_facing_after, -1);
    assert_eq!(
        player.velocity.x, 310,
        "Turn_Phys/ft_80084F3C should run after the same-frame Wait_IASA handoff"
    );
    assert_eq!(
        player.position.x, 310,
        "same-frame post-handoff physics should drive this tick's ground translation"
    );
}

#[test]
fn squat_rv_physics_uses_source_ground_traction() {
    let profile = FighterProfile::falcon_like();
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let mut player = PlayerState::new_with_profile(0, 0, 1, profile);
    player.set_motion_state_alias(MotionState::SquatRv);
    player.motion_frame = 5;
    player.grounded = true;
    player.ground_velocity_x = milli_to_source_units(390);
    player.velocity.x = 390;
    assert!(world.set_player_state_for_diagnostic(0, player));
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    step_world(&mut world, Frame(0), &neutral);

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::SquatRv);
    assert_eq!(
        player.velocity.x, 310,
        "ftCo_SquatRv_Phys calls ft_80084F3C, not an immediate horizontal velocity clear"
    );
    assert_eq!(player.position.x, 310);
}

#[test]
fn squat_wait_dash_check_runs_before_squat_release() {
    let profile = FighterProfile {
        action_frames: FighterActionFrames {
            squat_total_frames: 1,
            ..FighterActionFrames::falcon_like()
        },
        ..FighterProfile::falcon_like()
    };
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let down = [
        PlayerInput::neutral().with_left_stick(0, -80),
        PlayerInput::neutral(),
    ];
    let forward_dash_release = [
        PlayerInput::neutral().with_left_stick(dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &down);
    step_world(&mut world, Frame(1), &down);
    step_world(&mut world, Frame(2), &forward_dash_release);

    assert_eq!(world.players()[0].motion_state, MotionState::Dash);
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
            .with_left_stick(-dash_stick_x(), 0),
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
            .with_left_stick(-dash_stick_x(), 90),
        PlayerInput::neutral(),
    ];

    step_world(&mut side, Frame(0), &side_special);
    step_world(&mut up, Frame(0), &up_special);
    step_world(&mut down, Frame(0), &down_special);
    step_world(&mut down_boundary, Frame(0), &down_boundary_special);
    step_world(&mut diagonal_side_first, Frame(0), &diagonal_special);

    assert_eq!(side.players()[0].motion_state, MotionState::SpecialSStart);
    assert_eq!(side.players()[0].facing, -1);
    assert_eq!(up.players()[0].motion_state, MotionState::SpecialHi);
    assert_eq!(down.players()[0].motion_state, MotionState::SpecialLw);
    assert_eq!(down_boundary.players()[0].motion_state, MotionState::Wait);
    assert_eq!(
        diagonal_side_first.players()[0].motion_state,
        MotionState::SpecialSStart
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
            .with_left_stick(dash_stick_x(), 0)
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
            .with_left_stick(dash_stick_x(), 0),
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
            .with_left_stick(dash_stick_x(), 0)
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

    for frame in 0..=4 {
        step_world(world, Frame(frame), &jump);
    }

    assert!(!world.players()[0].grounded);
    assert_eq!(world.players()[0].motion_state, MotionState::JumpF);
}

fn advance_guard_on_release_to_guard_off(world: &mut World, start_frame: u32) -> u32 {
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let mut frame = start_frame;

    while world.players()[0].motion_state == MotionState::GuardOn && frame < start_frame + 60 {
        step_world(world, Frame(frame), &neutral);
        frame += 1;
    }

    assert_eq!(world.players()[0].motion_state, MotionState::GuardOff);
    frame - 1
}

fn land_player_one_on_left_platform(world: &mut World) -> (StageProfile, u32) {
    let stage = world.stage();
    let platform = stage.soft_platforms[0];
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    for frame in 0..=4 {
        step_world(world, Frame(frame), &jump);
    }

    let mut frame = 5;
    while !world.players()[0].grounded && frame < 120 {
        step_world(world, Frame(frame), &neutral);
        frame += 1;
    }

    assert!(world.players()[0].grounded);
    assert_eq!(world.players()[0].position.y, platform.y);

    let mut player = world.players()[0];
    player.motion_state = MotionState::Wait;
    player.motion_frame = 0;
    player.velocity.y = 0;
    player.grounded = true;
    assert!(world.set_player_state_for_diagnostic(0, player));
    assert_eq!(world.players()[0].motion_state, MotionState::Wait);
    (stage, frame)
}

fn enter_pass_from_left_platform(world: &mut World) -> u32 {
    let (_stage, mut frame) = land_player_one_on_left_platform(world);
    let shield = [
        PlayerInput::neutral().with_left_trigger_digital(true),
        PlayerInput::neutral(),
    ];
    let shield_down = [
        PlayerInput::neutral()
            .with_left_trigger_digital(true)
            .with_left_stick(0, -MeleeCommonData::provisional_mole().platform_pass_y),
        PlayerInput::neutral(),
    ];

    step_world(world, Frame(frame), &shield);
    frame += 1;
    while world.players()[0].motion_state != MotionState::Guard && frame < 180 {
        step_world(world, Frame(frame), &shield);
        frame += 1;
    }
    assert_eq!(world.players()[0].motion_state, MotionState::Guard);

    step_world(world, Frame(frame), &shield_down);
    frame += 1;
    assert_eq!(world.players()[0].motion_state, MotionState::Pass);

    frame
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
            .with_left_stick(dash_stick_x(), 0),
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
        100,
    );
}

#[test]
fn attack1_total_duration_uses_profile_action_frames() {
    let profile = FighterProfile {
        action_frames: FighterActionFrames {
            attack1_total_frames: 9,
            ..FighterActionFrames::falcon_like()
        },
        ..FighterProfile::falcon_like()
    };
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let jab = [
        PlayerInput::neutral().with_attack(true),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &jab);

    assert_current_action_returns_to_wait_after_frames(&mut world, 0, MotionState::Attack1, 9);
}

#[test]
fn attack1_iasa_uses_profile_action_frames() {
    let profile = FighterProfile {
        action_frames: FighterActionFrames {
            attack1_iasa_frame: 3,
            ..FighterActionFrames::falcon_like()
        },
        ..FighterProfile::falcon_like()
    };
    let mut world = World::for_two_players_with_profiles([profile; 2]);
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
    step_world(&mut world, Frame(1), &neutral);
    step_world(&mut world, Frame(2), &neutral);
    step_world(&mut world, Frame(3), &jump);

    assert_eq!(world.players()[0].motion_state, MotionState::KneeBend);
}

#[test]
fn attack_dash_total_duration_uses_profile_action_frames() {
    let profile = FighterProfile {
        action_frames: FighterActionFrames {
            attack_dash_total_frames: 12,
            ..FighterActionFrames::falcon_like()
        },
        ..FighterProfile::falcon_like()
    };
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let dash_attack = [
        PlayerInput::neutral()
            .with_left_stick(80, 0)
            .with_attack(true),
        PlayerInput::neutral(),
    ];

    advance_player_to_run(&mut world);
    step_world(&mut world, Frame(16), &dash_attack);

    assert_current_action_returns_to_wait_after_frames(&mut world, 16, MotionState::AttackDash, 12);
}

#[test]
fn attack_dash_iasa_uses_profile_action_frames() {
    let profile = FighterProfile {
        action_frames: FighterActionFrames {
            attack_dash_iasa_frame: 3,
            ..FighterActionFrames::falcon_like()
        },
        ..FighterProfile::falcon_like()
    };
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let dash_attack = [
        PlayerInput::neutral()
            .with_left_stick(80, 0)
            .with_attack(true),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];

    advance_player_to_run(&mut world);
    step_world(&mut world, Frame(17), &dash_attack);
    step_world(&mut world, Frame(18), &neutral);
    step_world(&mut world, Frame(19), &neutral);
    step_world(&mut world, Frame(20), &jump);

    assert_eq!(world.players()[0].motion_state, MotionState::KneeBend);
}

#[test]
fn guard_on_duration_uses_profile_action_frames() {
    let profile = FighterProfile {
        action_frames: FighterActionFrames {
            guard_on_total_frames: 2,
            ..FighterActionFrames::falcon_like()
        },
        ..FighterProfile::falcon_like()
    };
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &shield);
    assert_eq!(world.players()[0].motion_state, MotionState::GuardOn);

    step_world(&mut world, Frame(1), &shield);
    assert_eq!(world.players()[0].motion_state, MotionState::GuardOn);

    step_world(&mut world, Frame(2), &shield);
    assert_eq!(world.players()[0].motion_state, MotionState::Guard);
}

#[test]
fn falcon_guard_on_stays_startup_for_eight_visible_frames() {
    let mut world = World::for_two_players();
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];

    for frame in 0..=7 {
        step_world(&mut world, Frame(frame), &shield);
        assert_eq!(world.players()[0].motion_state, MotionState::GuardOn);
    }

    step_world(&mut world, Frame(8), &shield);
    assert_eq!(world.players()[0].motion_state, MotionState::Guard);
}

#[test]
fn guard_off_duration_uses_profile_action_frames() {
    let profile = FighterProfile {
        action_frames: FighterActionFrames {
            guard_off_total_frames: 3,
            ..FighterActionFrames::falcon_like()
        },
        ..FighterProfile::falcon_like()
    };
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &shield);
    let guard_off_entry_frame = advance_guard_on_release_to_guard_off(&mut world, 1);

    assert_current_action_returns_to_wait_after_frames(
        &mut world,
        guard_off_entry_frame,
        MotionState::GuardOff,
        3,
    );
}

#[test]
fn spotdodge_duration_uses_profile_action_frames() {
    let profile = FighterProfile {
        action_frames: FighterActionFrames {
            escape_n_total_frames: 7,
            ..FighterActionFrames::falcon_like()
        },
        ..FighterProfile::falcon_like()
    };
    let mut world = World::for_two_players_with_profiles([profile; 2]);
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];
    let shield_down = [
        PlayerInput::neutral()
            .with_left_trigger_analog(80)
            .with_left_stick(0, -90),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &shield);
    step_world(&mut world, Frame(1), &shield_down);

    assert_current_action_returns_to_wait_after_frames(&mut world, 1, MotionState::EscapeN, 7);
}

#[test]
fn roll_durations_use_profile_action_frames() {
    let profile = FighterProfile {
        action_frames: FighterActionFrames {
            escape_f_total_frames: 8,
            escape_b_total_frames: 9,
            escape_f_throw_flags_b3_frame: 20,
            escape_b_throw_flags_b3_frame: 20,
            ..FighterActionFrames::falcon_like()
        },
        ..FighterProfile::falcon_like()
    };
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];
    let shield_forward = [
        PlayerInput::neutral()
            .with_left_trigger_analog(80)
            .with_left_stick(dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];
    let shield_back = [
        PlayerInput::neutral()
            .with_left_trigger_analog(80)
            .with_left_stick(-dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];
    let mut forward = World::for_two_players_with_profiles([profile; 2]);
    let mut back = World::for_two_players_with_profiles([profile; 2]);

    step_world(&mut forward, Frame(0), &shield);
    step_world(&mut forward, Frame(1), &shield_forward);
    assert_current_action_returns_to_wait_after_frames(&mut forward, 1, MotionState::EscapeF, 8);

    step_world(&mut back, Frame(0), &shield);
    step_world(&mut back, Frame(1), &shield_back);
    assert_current_action_returns_to_wait_after_frames(&mut back, 1, MotionState::EscapeB, 9);
}

#[test]
fn forward_roll_authoritative_position_consumes_source_transn_delta() {
    let mut world = World::for_two_players();
    let shield = [
        PlayerInput::neutral().with_left_trigger_analog(80),
        PlayerInput::neutral(),
    ];
    let shield_forward = [
        PlayerInput::neutral()
            .with_left_trigger_analog(80)
            .with_left_stick(dash_stick_x(), 0),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &shield);
    step_world(&mut world, Frame(1), &shield_forward);
    let before_x = world.players()[0].position.x;
    step_world(&mut world, Frame(2), &shield_forward);

    assert_eq!(world.players()[0].motion_state, MotionState::EscapeF);
    assert_eq!(
        world.players()[0].position.x - before_x,
        source_root_motion_delta_milli_for_profile(
            MotionState::EscapeF,
            3,
            FighterProfile::falcon_like(),
        )
    );
}

#[test]
fn escape_roll_b3_facing_flip_keeps_source_root_motion_entry_direction() {
    let mut world = World::for_two_players();
    let mut player = world.players()[0];
    player.facing = -1;
    player.set_motion_state_alias(MotionState::EscapeF);
    player.motion_frame = player.profile.action_frames.escape_f_throw_flags_b3_frame - 1;
    player.grounded = true;
    assert!(world.set_player_state_for_diagnostic(0, player));
    let expected_velocity = -source_root_motion_delta_milli_for_profile(
        MotionState::EscapeF,
        player.profile.action_frames.escape_f_throw_flags_b3_frame + 1,
        player.profile,
    );

    step_world(&mut world, Frame(0), &[PlayerInput::neutral(); 2]);

    let player = world.players()[0];
    assert_eq!(
        player.facing, 1,
        "EscapeF script throw flag B3 flips ft facing_dir on the source event frame"
    );
    assert_eq!(
        player.velocity.x, expected_velocity,
        "ftCo_Escape_Phys drives TransN ground motion from Fighter.facing_dir1 latched at Fighter_ChangeMotionState entry, not the later script-flipped facing_dir"
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
fn attack11_repeated_a_enters_canonical_attack12_like_ftco_attack1() {
    let mut world = World::for_two_players();
    let jab = [
        PlayerInput::neutral().with_attack(true),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    step_world_with_falcon_jab_source_data(&mut world, 0, &jab);
    for frame in 1..=8 {
        step_world_with_falcon_jab_source_data(&mut world, frame, &neutral);
    }
    step_world_with_falcon_jab_source_data(&mut world, 9, &jab);

    let player = &world.players()[0];
    assert_eq!(
        player.melee_action_state_id,
        Some(MeleeActionStateId::new(45))
    );
    assert_eq!(
        player.source_action_key,
        Some(SourceActionKey::new("Attack12"))
    );
    assert_eq!(player.motion_state_alias, None);
}

#[test]
fn attack12_repeated_a_enters_canonical_attack13_like_ftco_attack1() {
    let mut world = World::for_two_players();
    let jab = [
        PlayerInput::neutral().with_attack(true),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    step_world_with_falcon_jab_source_data(&mut world, 0, &jab);
    for frame in 1..=8 {
        step_world_with_falcon_jab_source_data(&mut world, frame, &neutral);
    }
    step_world_with_falcon_jab_source_data(&mut world, 9, &jab);
    for frame in 10..=16 {
        step_world_with_falcon_jab_source_data(&mut world, frame, &neutral);
    }
    step_world_with_falcon_jab_source_data(&mut world, 17, &jab);

    let player = &world.players()[0];
    assert_eq!(
        player.melee_action_state_id,
        Some(MeleeActionStateId::new(46))
    );
    assert_eq!(
        player.source_action_key,
        Some(SourceActionKey::new("Attack13"))
    );
    assert_eq!(player.motion_state_alias, None);
}

fn falcon_jab_source_metadata(player: &PlayerState) -> Option<SourceActionPoseMetadata> {
    let action_state_id = player.melee_action_state_id?.get();
    let source_frame = if player.source_motion_anim_frame.is_finite() {
        (player
            .source_motion_anim_frame
            .floor()
            .clamp(0.0, f32::from(u8::MAX)) as u8)
            .saturating_add(1)
    } else {
        1
    };
    let event = if player.source_action_key == Some(SourceActionKey::new("Attack100Loop"))
        && matches!(source_frame, 6 | 14 | 22 | 30 | 37)
    {
        SourceActionScriptEvent::SetThrowFlag {
            hit_idx: 0,
            flag_bit: Some(3),
        }
    } else {
        match (action_state_id, source_frame) {
            (44, 5) => SourceActionScriptEvent::SetJabCombo { disabled: true },
            (44, 9) => SourceActionScriptEvent::SetJabCombo { disabled: false },
            (45, 4) => SourceActionScriptEvent::SetJabCombo { disabled: true },
            (45, 8) => SourceActionScriptEvent::SetJabCombo { disabled: false },
            (46, 10) => SourceActionScriptEvent::SetJabRapid { state: true },
            _ => SourceActionScriptEvent::None,
        }
    };

    Some(SourceActionPoseMetadata {
        script_events: if event == SourceActionScriptEvent::None {
            SourceActionScriptEvents::empty()
        } else {
            SourceActionScriptEvents::single(event)
        },
        ..SourceActionPoseMetadata::default()
    })
}

fn falcon_jab_source_action_total_frames(action_state_id: MeleeActionStateId) -> Option<u8> {
    match action_state_id.get() {
        45 => Some(20),
        46 => Some(32),
        47 => Some(6),
        48 => Some(40),
        49 => Some(9),
        _ => None,
    }
}

fn step_world_with_falcon_jab_source_data(
    world: &mut World,
    frame: u32,
    inputs: &[PlayerInput; 2],
) {
    step_world_with_source_runtime_data(
        world,
        Frame(frame),
        inputs,
        falcon_jab_source_metadata,
        falcon_jab_source_action_total_frames,
    );
}

#[test]
fn falcon_jab_attack100_loop_fixture_matches_source_throw_flag_rows() {
    for source_frame in 1u8..=40 {
        let mut player = PlayerState::new(0, 0, 1);
        player.melee_action_state_id = Some(MeleeActionStateId::new(48));
        player.source_action_key = Some(SourceActionKey::new("Attack100Loop"));
        player.set_source_motion_anim_frame(f32::from(source_frame - 1));
        let expected = if matches!(source_frame, 6 | 14 | 22 | 30 | 37) {
            SourceActionScriptEvents::single(SourceActionScriptEvent::SetThrowFlag {
                hit_idx: 0,
                flag_bit: Some(3),
            })
        } else {
            SourceActionScriptEvents::empty()
        };
        assert_eq!(
            falcon_jab_source_metadata(&player)
                .expect("fixture should return metadata")
                .script_events,
            expected,
            "fixture must match the decoded source event at frame {source_frame}"
        );
    }
}

#[test]
fn attack13_rapid_jab_script_enters_attack100_start_like_ftco_attack100() {
    let mut world = World::for_two_players();
    let jab = [
        PlayerInput::neutral().with_attack(true),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    step_world_with_falcon_jab_source_data(&mut world, 0, &jab);
    for frame in 1..=8 {
        step_world_with_falcon_jab_source_data(&mut world, frame, &neutral);
    }
    step_world_with_falcon_jab_source_data(&mut world, 9, &jab);
    for frame in 10..=16 {
        step_world_with_falcon_jab_source_data(&mut world, frame, &neutral);
    }
    step_world_with_falcon_jab_source_data(&mut world, 17, &jab);
    for frame in 18..=26 {
        step_world_with_falcon_jab_source_data(&mut world, frame, &neutral);
    }

    let player = &world.players()[0];
    assert_eq!(
        player.melee_action_state_id,
        Some(MeleeActionStateId::new(47)),
        "ftCo_Attack_800D6A50 should route Falcon Attack13 into Attack100Start once x1A54 reaches rapid_jab_window and x2218_b2 is set by the decoded Attack13 script"
    );
    assert_eq!(
        player.source_action_key,
        Some(SourceActionKey::new("Attack100Start"))
    );
    assert_eq!(player.source_action_total_frames, 6);
    assert_eq!(player.motion_state_alias, None);
    assert_eq!(player.motion_frame, 0);
    assert_eq!(player.cur_anim_frame(), 1.0);
}

#[test]
fn attack100_start_loop_and_end_follow_decomp_action_chain() {
    let mut world = World::for_two_players();
    let jab = [
        PlayerInput::neutral().with_attack(true),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];

    step_world_with_falcon_jab_source_data(&mut world, 0, &jab);
    for frame in 1..=8 {
        step_world_with_falcon_jab_source_data(&mut world, frame, &neutral);
    }
    step_world_with_falcon_jab_source_data(&mut world, 9, &jab);
    for frame in 10..=16 {
        step_world_with_falcon_jab_source_data(&mut world, frame, &neutral);
    }
    step_world_with_falcon_jab_source_data(&mut world, 17, &jab);
    for frame in 18..=26 {
        step_world_with_falcon_jab_source_data(&mut world, frame, &neutral);
    }
    assert_eq!(world.players()[0].cur_anim_frame(), 1.0);

    for frame in 27..=30 {
        step_world_with_falcon_jab_source_data(&mut world, frame, &neutral);
    }
    assert_eq!(
        world.players()[0].melee_action_state_id,
        Some(MeleeActionStateId::new(47))
    );

    step_world_with_falcon_jab_source_data(&mut world, 31, &neutral);
    assert_eq!(
        world.players()[0].melee_action_state_id,
        Some(MeleeActionStateId::new(48))
    );
    assert_eq!(
        world.players()[0].source_action_key,
        Some(SourceActionKey::new("Attack100Loop"))
    );
    assert_eq!(world.players()[0].motion_frame, 0);
    assert_eq!(world.players()[0].cur_anim_frame(), 0.0);
    assert!(!world.players()[0].source_attack100_loop_has_started);

    step_world_with_falcon_jab_source_data(&mut world, 32, &neutral);
    assert!(world.players()[0].source_attack100_loop_has_started);

    for frame in 33..=35 {
        step_world_with_falcon_jab_source_data(&mut world, frame, &neutral);
    }
    assert_eq!(
        world.players()[0].melee_action_state_id,
        Some(MeleeActionStateId::new(48)),
        "Attack100Loop should stay active before the first decoded throw-flag row"
    );

    step_world_with_falcon_jab_source_data(&mut world, 36, &neutral);
    assert_eq!(
        world.players()[0].melee_action_state_id,
        Some(MeleeActionStateId::new(49))
    );
    assert_eq!(
        world.players()[0].source_action_key,
        Some(SourceActionKey::new("Attack100End"))
    );
    assert_eq!(world.players()[0].motion_frame, 0);
    assert_eq!(world.players()[0].cur_anim_frame(), 0.0);

    for frame in 37..=44 {
        step_world_with_falcon_jab_source_data(&mut world, frame, &neutral);
    }
    assert_eq!(world.players()[0].motion_state, MotionState::Attack1);

    step_world_with_falcon_jab_source_data(&mut world, 45, &neutral);
    assert_eq!(world.players()[0].motion_state, MotionState::Wait);
}

#[test]
fn fsmash_does_not_interrupt_before_falcon_iasa_but_can_after() {
    let mut early = World::for_two_players();
    let mut iasa = World::for_two_players();
    let fsmash = [
        PlayerInput::neutral()
            .with_attack(true)
            .with_left_stick(dash_stick_x(), 0),
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
fn source_state_sequence_records_landing_and_knee_bend_callback_order() {
    let landing = mole_core::source_state_sequence_for_motion_state(MotionState::Landing)
        .expect("Landing sequence");
    assert_eq!(
        landing
            .callbacks
            .iter()
            .map(|callback| callback.phase)
            .collect::<Vec<_>>(),
        ["Anim", "IASA", "Phys", "Coll"]
    );
    assert_eq!(
        landing
            .transitions
            .iter()
            .map(|transition| (transition.to, transition.function))
            .collect::<Vec<_>>(),
        [
            (MotionState::Wait, "ftCo_Landing_Anim / ft_8008A2BC"),
            (
                MotionState::KneeBend,
                "ftCo_Landing_IASA / ftCo_Jump_CheckInput"
            ),
            (MotionState::Fall, "ftCo_Landing_Coll / ft_80084280"),
        ]
    );

    let knee_bend = mole_core::source_state_sequence_for_motion_state(MotionState::KneeBend)
        .expect("KneeBend sequence");
    assert_eq!(
        knee_bend
            .callbacks
            .iter()
            .map(|callback| callback.phase)
            .collect::<Vec<_>>(),
        ["Anim", "IASA", "Phys", "Coll"]
    );
    assert_eq!(
        knee_bend
            .transitions
            .iter()
            .map(|transition| (transition.to, transition.function))
            .collect::<Vec<_>>(),
        [
            (MotionState::JumpF, "ftCo_KneeBend_Anim / ftCo_Jump_Enter"),
            (MotionState::JumpB, "ftCo_KneeBend_Anim / ftCo_Jump_Enter"),
        ]
    );
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
        -source_units_to_milli(profile.fast_fall_velocity)
    );
    assert_eq!(
        fast_fall.players()[0].position.y - position_before_tap,
        -source_units_to_milli(profile.fast_fall_velocity)
    );

    frame += 1;
    let position_before_held_down = fast_fall.players()[0].position.y;
    step_world(&mut fast_fall, Frame(frame), &down_input);

    assert_eq!(
        fast_fall.players()[0].velocity.y,
        -source_units_to_milli(profile.fast_fall_velocity)
    );
    assert_eq!(
        fast_fall.players()[0].position.y - position_before_held_down,
        -source_units_to_milli(profile.fast_fall_velocity)
    );
}

#[test]
fn world_common_data_drives_fast_fall_gate() {
    let common = MeleeCommonData {
        fast_fall_y: 40,
        fast_fall_window: 3,
        ..MeleeCommonData::provisional_mole()
    };
    let mut world = World::for_two_players_with_common_data(common);
    let profile = FighterProfile::falcon_like();
    let jump = [
        PlayerInput::neutral().with_jump(true),
        PlayerInput::neutral(),
    ];
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    let soft_down = [
        PlayerInput::neutral().with_left_stick(0, -50),
        PlayerInput::neutral(),
    ];

    step_world(&mut world, Frame(0), &jump);

    let mut frame = 1;
    while world.players()[0].velocity.y >= 0 {
        step_world(&mut world, Frame(frame), &neutral);
        frame += 1;
        assert!(frame < 120);
    }

    step_world(&mut world, Frame(frame), &soft_down);

    assert!(world.players()[0].fast_falling);
    assert_eq!(
        world.players()[0].velocity.y,
        -source_units_to_milli(profile.fast_fall_velocity)
    );
}

#[test]
fn runtime_motion_state_lookup_handles_source_action_aliases() {
    assert_eq!(
        motion_state_for_runtime_variant("AttackLw3"),
        Some(MotionState::AttackLw3)
    );
    assert_eq!(
        runtime_motion_state_for_source_key("AttackS3S"),
        Some("AttackS3")
    );
    assert_eq!(
        runtime_motion_state_for_source_key("Attack11"),
        Some("Attack1")
    );
    assert_eq!(motion_state_for_runtime_variant("AttackS3S"), None);
}

#[test]
fn slippi_match_start_uses_source_stock_and_player_state_shape() {
    let world = World::for_slippi_battlefield_singles_match_start();

    assert_eq!(world.players()[0].player_state, PLAYER_STATE_IN_GAME);
    assert_eq!(world.players()[1].player_state, PLAYER_STATE_IN_GAME);
    assert_eq!(world.players()[0].stocks, DEFAULT_STOCK_COUNT);
    assert_eq!(world.players()[1].stocks, DEFAULT_STOCK_COUNT);
}

#[test]
fn match_phase_is_rollback_owned_for_playtest_match_flow() {
    let mut world = World::for_slippi_battlefield_singles_match_start();
    assert_eq!(world.match_phase(), MatchPhase::Ready);
    assert!(world.match_phase_timer() > 0);

    let before = world.rollback_snapshot();
    let before_checksum = world.checksum();
    let neutral = [PlayerInput::neutral(), PlayerInput::neutral()];
    step_world(&mut world, Frame(0), &neutral);

    assert_ne!(world.checksum(), before_checksum);

    world.restore_rollback_snapshot(&before);
    assert_eq!(world.match_phase(), MatchPhase::Ready);
    assert_eq!(world.checksum(), before_checksum);
}

#[test]
fn hsd_randi_matches_baselib_random_sequence_and_rolls_back() {
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let before = world.rollback_snapshot();
    let before_checksum = world.checksum();

    assert_eq!(world.hsd_randi_for_diagnostic(100), 0);
    assert_eq!(world.hsd_rng_seed(), 2_745_024);
    assert_ne!(world.checksum(), before_checksum);

    world.restore_rollback_snapshot(&before);
    assert_eq!(world.hsd_rng_seed(), 1);
    assert_eq!(world.checksum(), before_checksum);
}

#[test]
fn battlefield_stage_promotes_source_rebirth_platform_points_from_map_head_x280() {
    let stage = StageProfile::battlefield();
    let melee_stage = stage
        .melee_stage_profile()
        .expect("Battlefield profile should retain source stage metadata");

    let p2_point = melee_stage
        .map_head
        .point_mappings
        .iter()
        .find(|point| point.stage_info_index == 5)
        .expect("Battlefield x280[5] should be decoded from map_head.unk0");

    assert_eq!(p2_point.tree_index, 19);
    assert_eq!(p2_point.joint_index, Some(19));
    assert_eq!(p2_point.source_position.x.to_bits(), (-50.0_f32).to_bits());
    assert_eq!(p2_point.source_position.y.to_bits(), 100.0_f32.to_bits());
    assert_eq!(p2_point.scaled_x, -40_000);
    assert_eq!(p2_point.scaled_y, 80_000);

    let platform = stage.respawn_platforms[1];
    assert_eq!(platform.platform_index, 1);
    assert_eq!(platform.stage_point_index, 5);
    assert_eq!(platform.final_x, -40_000);
    assert_eq!(platform.final_y, 80_000);
    assert_eq!(platform.top_y, 136_000);
    assert_eq!(platform.facing, 1);
}

#[test]
fn dead_down_rebirth_uses_source_platform_top_and_first_velocity_step() {
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let common_data = world.common_data();
    let mut player = world.players()[1];
    player.enter_source_dead_motion_state(MotionState::DeadDown);
    player.source_common_timer = 1;
    assert!(world.set_player_state_for_diagnostic(1, player));

    step_world(&mut world, Frame(0), &[PlayerInput::neutral(); 2]);

    let platform = world.stage().respawn_platforms[1];
    let player = world.players()[1];
    assert_eq!(player.motion_state, MotionState::Rebirth);
    assert_eq!(player.facing, 1);
    assert_eq!(player.source_common_timer, common_data.rebirth_ticks - 1);
    assert_eq!(player.position.x, platform.final_x);
    assert_eq!(player.position.y, 135_067);
    assert_eq!(source_units_to_milli(player.source_self_velocity_x), 0);
    assert_eq!(source_units_to_milli(player.source_self_velocity_y), -933);
}

#[test]
fn stage_blast_zone_ko_loses_stock_and_enters_source_dead_state_before_rebirth() {
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let stage = world.stage();
    let mut player = world.players()[0];
    player.position.x = stage.blast_zones.right_x + 1_000;
    player.source_position = mole_core::SourceVec2::from_milli(player.position);
    player.damage_percent = 137.0;
    player.velocity = Vec2 {
        x: 42_000,
        y: 12_000,
    };
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(&mut world, Frame(0), &[PlayerInput::neutral(); 2]);

    let player = world.players()[0];
    assert_eq!(player.stocks, DEFAULT_STOCK_COUNT - 1);
    assert_eq!(player.player_state, PLAYER_STATE_IN_GAME);
    assert_eq!(player.motion_state, MotionState::DeadRight);
    assert_eq!(
        player.melee_action_state_id,
        Some(MeleeActionStateId::new(2))
    );
    assert_eq!(player.damage_percent, 137.0);
    assert_eq!(player.velocity, Vec2 { x: 0, y: 0 });

    let mut player = world.players()[0];
    player.source_common_timer = 1;
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(&mut world, Frame(1), &[PlayerInput::neutral(); 2]);
    let player = world.players()[0];
    let platform = stage.respawn_platforms[0];
    assert_eq!(player.motion_state, MotionState::Rebirth);
    assert_eq!(
        player.melee_action_state_id,
        Some(MeleeActionStateId::new(12))
    );
    assert_eq!(
        player.position,
        Vec2 {
            x: platform.final_x,
            y: 135_067
        }
    );
    assert_eq!(player.facing, platform.facing);
    assert_eq!(player.damage_percent, 0.0);
    assert_eq!(player.velocity, Vec2 { x: 0, y: -933 });
    assert!(!player.grounded);
}

#[test]
fn upward_blast_zone_ko_uses_source_rng_and_x520_fall_chance() {
    let mut fall = World::for_slippi_battlefield_singles_match_start();
    let mut player = fall.players()[0];
    player.position.y = fall.stage().blast_zones.top_y + 1_000;
    player.source_position = mole_core::SourceVec2::from_milli(player.position);
    assert!(fall.set_player_state_for_diagnostic(0, player));

    step_world(&mut fall, Frame(0), &[PlayerInput::neutral(); 2]);

    assert_eq!(fall.players()[0].motion_state, MotionState::DeadUpFall);
    assert_eq!(fall.players()[0].stocks, DEFAULT_STOCK_COUNT - 1);
    assert_eq!(fall.hsd_rng_seed(), 2_745_024);

    let mut star = World::for_slippi_battlefield_singles_match_start();
    star.set_hsd_rng_seed_for_diagnostic(12_030);
    let mut player = star.players()[0];
    player.position.y = star.stage().blast_zones.top_y + 1_000;
    player.source_position = mole_core::SourceVec2::from_milli(player.position);
    assert!(star.set_player_state_for_diagnostic(0, player));

    step_world(&mut star, Frame(0), &[PlayerInput::neutral(); 2]);

    assert_eq!(star.players()[0].motion_state, MotionState::DeadUpStar);
    assert_eq!(star.players()[0].stocks, DEFAULT_STOCK_COUNT - 1);
    assert_eq!(star.hsd_rng_seed(), 2_577_107_401);
}

#[test]
fn dead_up_fall_preserves_source_phase_timers_before_rebirth() {
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let stage = world.stage();
    let mut player = world.players()[0];
    player.position.y = stage.blast_zones.top_y + 1_000;
    player.source_position = mole_core::SourceVec2::from_milli(player.position);
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(&mut world, Frame(0), &[PlayerInput::neutral(); 2]);
    assert_eq!(world.players()[0].motion_state, MotionState::DeadUpFall);

    step_world(&mut world, Frame(1), &[PlayerInput::neutral(); 2]);
    assert_eq!(world.players()[0].motion_state, MotionState::DeadUpFall);

    for frame in 2..=129 {
        step_world(&mut world, Frame(frame), &[PlayerInput::neutral(); 2]);
    }

    let player = world.players()[0];
    let platform = stage.respawn_platforms[0];
    assert_eq!(player.motion_state, MotionState::Rebirth);
    assert_eq!(
        player.position,
        Vec2 {
            x: platform.final_x,
            y: 135_067
        }
    );
    assert_eq!(player.damage_percent, 0.0);
}

#[test]
fn dead_up_star_preserves_source_phase_timers_before_rebirth() {
    let mut world = World::for_slippi_battlefield_singles_match_start();
    world.set_hsd_rng_seed_for_diagnostic(12_030);
    let stage = world.stage();
    let mut player = world.players()[0];
    player.position.y = stage.blast_zones.top_y + 1_000;
    player.source_position = mole_core::SourceVec2::from_milli(player.position);
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(&mut world, Frame(0), &[PlayerInput::neutral(); 2]);
    assert_eq!(world.players()[0].motion_state, MotionState::DeadUpStar);

    step_world(&mut world, Frame(1), &[PlayerInput::neutral(); 2]);
    assert_eq!(world.players()[0].motion_state, MotionState::DeadUpStar);

    for frame in 2..=176 {
        step_world(&mut world, Frame(frame), &[PlayerInput::neutral(); 2]);
    }

    let player = world.players()[0];
    let platform = stage.respawn_platforms[0];
    assert_eq!(player.motion_state, MotionState::Rebirth);
    assert_eq!(
        player.position,
        Vec2 {
            x: platform.final_x,
            y: 135_067
        }
    );
    assert_eq!(player.damage_percent, 0.0);
}

#[test]
fn rebirth_wait_exit_installs_source_hurt_intangibility_from_x5d8_before_fall() {
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let stage = world.stage();
    let mut player = world.players()[0];
    player.position.x = stage.blast_zones.right_x + 1_000;
    player.source_position = mole_core::SourceVec2::from_milli(player.position);
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(&mut world, Frame(0), &[PlayerInput::neutral(); 2]);
    assert_eq!(world.players()[0].motion_state, MotionState::DeadRight);
    let mut player = world.players()[0];
    player.source_common_timer = 1;
    assert!(world.set_player_state_for_diagnostic(0, player));
    step_world(&mut world, Frame(1), &[PlayerInput::neutral(); 2]);
    assert_eq!(world.players()[0].motion_state, MotionState::Rebirth);

    for frame in 2..=61 {
        step_world(&mut world, Frame(frame), &[PlayerInput::neutral(); 2]);
    }
    assert_eq!(world.players()[0].motion_state, MotionState::RebirthWait);

    for frame in 62..=302 {
        step_world(&mut world, Frame(frame), &[PlayerInput::neutral(); 2]);
    }

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::Fall);
    assert_eq!(
        player.source_hurt_intangible_timer,
        world.common_data().rebirth_hurt_intangible_ticks
    );
    assert_eq!(
        player.source_collision_state,
        SOURCE_COLLISION_STATE_HURT_INTANGIBLE
    );
}

#[test]
fn rebirth_wait_down_input_exits_spawn_platform_into_fall_with_source_intangibility() {
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let stage = world.stage();
    let mut player = world.players()[0];
    player.position.x = stage.blast_zones.right_x + 1_000;
    player.source_position = mole_core::SourceVec2::from_milli(player.position);
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(&mut world, Frame(0), &[PlayerInput::neutral(); 2]);
    let mut player = world.players()[0];
    player.source_common_timer = 1;
    assert!(world.set_player_state_for_diagnostic(0, player));
    step_world(&mut world, Frame(1), &[PlayerInput::neutral(); 2]);
    for frame in 2..=61 {
        step_world(&mut world, Frame(frame), &[PlayerInput::neutral(); 2]);
    }
    assert_eq!(world.players()[0].motion_state, MotionState::RebirthWait);
    let timer_before = world.players()[0].source_common_timer;
    let platform_y_before = world.players()[0].position.y;

    let down = PlayerInput::neutral().with_left_stick(0, -127);
    step_world(&mut world, Frame(62), &[down, PlayerInput::neutral()]);

    let player = world.players()[0];
    assert!(timer_before > 1);
    assert_eq!(player.motion_state, MotionState::Fall);
    assert_eq!(
        player.source_hurt_intangible_timer,
        world.common_data().rebirth_hurt_intangible_ticks
    );
    assert_eq!(
        player.source_collision_state,
        SOURCE_COLLISION_STATE_HURT_INTANGIBLE
    );
    assert_eq!(
        player.motion_frame, 0,
        "RebirthWait IASA installs Fall after RebirthWait anim, so Fall anim has not advanced yet"
    );
    assert_eq!(
        player.velocity.y,
        source_units_to_milli(-player.profile.gravity),
        "Fall physics must run on the same tick RebirthWait IASA enters Fall"
    );
    assert_eq!(
        player.position.y,
        platform_y_before + player.velocity.y,
        "Fall position integration must run on the RebirthWait release tick"
    );
}

#[test]
fn rebirth_handoff_runs_rebirth_wait_iasa_before_same_tick_fall_physics() {
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let common = world.common_data();
    let platform = world.stage().respawn_platforms[0];
    let platform_position = Vec2 {
        x: platform.final_x,
        y: platform.final_y + platform.offset_y,
    };
    let mut player = world.players()[0];
    player.enter_source_rebirth_state(platform, common.shield_start_health, common.rebirth_ticks);
    player.position = platform_position;
    player.source_position = SourceVec2::from_milli(platform_position);
    player.source_common_timer = 0;
    player.motion_frame = common.rebirth_ticks.saturating_sub(1);
    player.velocity = Vec2 { x: 0, y: 0 };
    player.source_self_velocity_x = 0.0;
    player.source_self_velocity_y = 0.0;
    assert!(world.set_player_state_for_diagnostic(0, player));

    let down = PlayerInput::neutral().with_left_stick(0, -127);
    step_world(&mut world, Frame(0), &[down, PlayerInput::neutral()]);

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::Fall);
    assert_eq!(
        player.source_hurt_intangible_timer,
        common.rebirth_hurt_intangible_ticks
    );
    assert_eq!(
        player.source_collision_state,
        SOURCE_COLLISION_STATE_HURT_INTANGIBLE
    );
    assert_eq!(
        player.motion_frame, 0,
        "Rebirth_Anim installs RebirthWait before later procs; Fall anim has not advanced"
    );
    assert_eq!(
        player.velocity.y,
        source_units_to_milli(-player.profile.gravity)
    );
    assert_eq!(
        player.position.y,
        platform_position.y + player.velocity.y,
        "Fall physics must run after RebirthWait IASA enters Fall on the handoff tick"
    );
}

#[test]
fn rebirth_wait_timer_expiry_enters_fall_before_same_tick_fall_physics() {
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let common = world.common_data();
    let platform = world.stage().respawn_platforms[0];
    let platform_position = Vec2 {
        x: platform.final_x,
        y: platform.final_y + platform.offset_y,
    };
    let mut player = world.players()[0];
    player.enter_source_rebirth_state(platform, common.shield_start_health, common.rebirth_ticks);
    player.position = platform_position;
    player.source_position = SourceVec2::from_milli(platform_position);
    player.enter_source_rebirth_wait_state(1);
    assert!(world.set_player_state_for_diagnostic(0, player));

    step_world(&mut world, Frame(0), &[PlayerInput::neutral(); 2]);

    let player = world.players()[0];
    assert_eq!(player.motion_state, MotionState::Fall);
    assert_eq!(
        player.source_hurt_intangible_timer,
        common.rebirth_hurt_intangible_ticks
    );
    assert_eq!(
        player.source_collision_state,
        SOURCE_COLLISION_STATE_HURT_INTANGIBLE
    );
    assert_eq!(
        player.motion_frame, 0,
        "RebirthWait_Anim enters Fall before Fall anim advances"
    );
    assert_eq!(
        player.velocity.y,
        source_units_to_milli(-player.profile.gravity)
    );
    assert_eq!(
        player.position.y,
        platform_position.y + player.velocity.y,
        "Fall physics must run after RebirthWait_Anim expires into Fall"
    );
}

#[test]
fn four_stock_battlefield_match_flow_smoke_reaches_player_eliminated() {
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let mut frame = 0;

    for expected_remaining_stocks in (0..DEFAULT_STOCK_COUNT).rev() {
        let stage = world.stage();
        let mut victim = world.players()[1];
        victim.position.x = stage.blast_zones.right_x + 1_000;
        victim.source_position = mole_core::SourceVec2::from_milli(victim.position);
        assert!(world.set_player_state_for_diagnostic(1, victim));

        step_world(&mut world, Frame(frame), &[PlayerInput::neutral(); 2]);
        frame += 1;

        let victim = world.players()[1];
        assert_eq!(victim.stocks, expected_remaining_stocks);

        if expected_remaining_stocks == 0 {
            assert_eq!(victim.player_state, PLAYER_STATE_NONE);
            break;
        }

        assert_eq!(victim.player_state, PLAYER_STATE_IN_GAME);
        assert_eq!(victim.motion_state, MotionState::DeadRight);
        let mut victim = world.players()[1];
        victim.source_common_timer = 1;
        assert!(world.set_player_state_for_diagnostic(1, victim));

        step_world(&mut world, Frame(frame), &[PlayerInput::neutral(); 2]);
        frame += 1;
        assert_eq!(world.players()[1].motion_state, MotionState::Rebirth);

        for _ in 0..60 {
            step_world(&mut world, Frame(frame), &[PlayerInput::neutral(); 2]);
            frame += 1;
        }
        assert_eq!(world.players()[1].motion_state, MotionState::RebirthWait);

        for _ in 0..241 {
            step_world(&mut world, Frame(frame), &[PlayerInput::neutral(); 2]);
            frame += 1;
        }
        assert_eq!(world.players()[1].motion_state, MotionState::Fall);
    }

    assert_eq!(world.players()[0].player_state, PLAYER_STATE_IN_GAME);
    assert_eq!(world.players()[0].stocks, DEFAULT_STOCK_COUNT);
    assert_eq!(world.players()[1].player_state, PLAYER_STATE_NONE);
    assert_eq!(world.players()[1].stocks, 0);
}

#[test]
fn final_blast_zone_ko_clears_player_state_and_restores_through_rollback() {
    let mut world = World::for_slippi_battlefield_singles_match_start();
    let mut player = world.players()[0];
    player.stocks = 1;
    assert!(world.set_player_state_for_diagnostic(0, player));
    let before = world.rollback_snapshot();
    let before_checksum = world.checksum();

    player.position.y = world.stage().blast_zones.bottom_y - 1_000;
    player.source_position = mole_core::SourceVec2::from_milli(player.position);
    assert!(world.set_player_state_for_diagnostic(0, player));
    step_world(&mut world, Frame(0), &[PlayerInput::neutral(); 2]);

    assert_eq!(world.players()[0].stocks, 0);
    assert_eq!(world.players()[0].player_state, PLAYER_STATE_NONE);
    assert_ne!(world.checksum(), before_checksum);

    world.restore_rollback_snapshot(&before);
    assert_eq!(world.players()[0].stocks, 1);
    assert_eq!(world.players()[0].player_state, PLAYER_STATE_IN_GAME);
    assert_eq!(world.checksum(), before_checksum);
}
