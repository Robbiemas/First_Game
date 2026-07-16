use crate::input::NO_GROUNDED_SPECIAL_DIRECTION;
use crate::{
    collision::{
        floor_friction_multiplier_for_bottom, floor_surface_for_bottom,
        floor_surface_index_for_bottom, source_ledge_grab_contact_for_facing,
        SourceHitboxAttributes, SourceInstalledThrowHitbox, SourceThrowHitboxAttributes,
    },
    fighter_stick_axis_to_f32,
    state::{
        action_sample_frame_count_for_motion_state, active_ecb_bottom_offset_y,
        active_ecb_for_player, enter_source_shield_break_fly, is_source_damage_action_state_id,
        is_source_dead_motion_state, is_source_rebirth_motion_state,
        landing_fall_special_lag_ticks, live_source_local_ecb_for_player_pose_frame_with_flags,
        melee_action_state_id_for_motion_state, player_model_facing,
        source_binding_for_motion_state, source_motion_change_clamps_ground_velocity,
        source_root_motion_delta, source_root_motion_delta_for_action_key,
        source_root_motion_frame_count, source_root_motion_position,
        source_special_action_binding_for_motion_state, SourceFighterEcb, EXPIRED_INPUT_TIMER,
        SOURCE_COLLISION_STATE_HIT_AND_HURT_INTANGIBLE, SOURCE_COLLISION_STATE_HURT_INTANGIBLE,
        SOURCE_COLLISION_STATE_NORMAL, SOURCE_COLL_ECB_ZERO, SOURCE_JOBJ_ECB_BOTTOM_OFFSET_Y,
    },
    units::{milli_to_source_units, source_units_to_milli},
    FighterProfile, Frame, MeleeActionStateId, MeleeCommonData, MeleeInputFacts,
    MeleeInputSnapshot, MeleeInputTimers, MeleeJumpInput, MotionState, PlayerInput, PlayerState,
    SourceActionKey, SourceActionPoseMetadata, SourceActionScriptEvent, SourceActionScriptEvents,
    SourceCapturePose, SourceVec2, StageCollisionLineKind, StageCollisionProfile, StageLedge,
    StageLedgeSide, StageProfile, StageSurfaceKind, Vec2, World, PLAYER_COUNT,
    PLAYER_STATE_IN_GAME,
};

const ATTACK_ACTIVE_TICKS: u8 = 12;
const SHIELD_TURN_FRAMES: u8 = 5;
const TURN_LATCH_ATTACK: u8 = 0x01;
const TURN_LATCH_SPECIAL: u8 = 0x02;
const FALCON_ATTACK_S3_FRAMES: u8 = 29;
const FALCON_ATTACK_HI3_FRAMES: u8 = 39;
const FALCON_ATTACK_LW3_FRAMES: u8 = 35;
const FALCON_ATTACK_S4_FRAMES: u8 = 64;
const FALCON_ATTACK_HI4_FRAMES: u8 = 54;
const FALCON_ATTACK_LW4_FRAMES: u8 = 49;
const FALCON_CATCH_FRAMES: u8 = 30;
const FALCON_CATCH_DASH_FRAMES: u8 = 40;
const FALCON_ATTACK_HI3_IASA: u8 = 38;
const FALCON_ATTACK_LW3_IASA: u8 = 35;
const FALCON_ATTACK_S4_IASA: u8 = 60;
const FALCON_ATTACK_HI4_IASA: u8 = 40;
const FALCON_ATTACK_LW4_IASA: u8 = 45;
const FALCON_SPECIAL_N_IASA: u8 = 65;
const GROUND_TO_AIR_ECB_LOCK_FRAMES: u8 = 10;
const AIRBORNE_STATE_TWO_ECB_LOCK_FRAMES: u8 = 5;
const UCF_DASHBACK_SOURCE_TURN_FRAME: u8 = 1;
const GROUND_EDGE_EXPORTED_STICK_THRESHOLD: i32 = 95;
const SOURCE_DOWN_BOUND_U_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(183);
const SOURCE_DOWN_BOUND_D_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(191);
const SOURCE_DOWN_BOUND_U_ACTION_KEY: SourceActionKey = SourceActionKey::new("DownBoundU");
const SOURCE_DOWN_BOUND_D_ACTION_KEY: SourceActionKey = SourceActionKey::new("DownBoundD");
const SOURCE_DOWN_WAIT_U_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(184);
const SOURCE_DOWN_WAIT_D_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(192);
const SOURCE_DOWN_WAIT_U_ACTION_KEY: SourceActionKey = SourceActionKey::new("DownWaitU");
const SOURCE_DOWN_WAIT_D_ACTION_KEY: SourceActionKey = SourceActionKey::new("DownWaitD");
const SOURCE_DOWN_STAND_U_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(186);
const SOURCE_DOWN_STAND_D_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(194);
const SOURCE_DOWN_STAND_U_ACTION_KEY: SourceActionKey = SourceActionKey::new("DownStandU");
const SOURCE_DOWN_STAND_D_ACTION_KEY: SourceActionKey = SourceActionKey::new("DownStandD");
const SOURCE_DOWN_ATTACK_U_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(187);
const SOURCE_DOWN_ATTACK_D_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(195);
const SOURCE_DOWN_ATTACK_U_ACTION_KEY: SourceActionKey = SourceActionKey::new("DownAttackU");
const SOURCE_DOWN_ATTACK_D_ACTION_KEY: SourceActionKey = SourceActionKey::new("DownAttackD");
const SOURCE_DOWN_FORWARD_U_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(188);
const SOURCE_DOWN_BACK_U_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(189);
const SOURCE_DOWN_SPOT_U_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(190);
const SOURCE_DOWN_FORWARD_D_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(196);
const SOURCE_DOWN_BACK_D_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(197);
const SOURCE_DOWN_SPOT_D_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(198);
const SOURCE_DOWN_FORWARD_U_ACTION_KEY: SourceActionKey = SourceActionKey::new("DownFowardU");
const SOURCE_DOWN_BACK_U_ACTION_KEY: SourceActionKey = SourceActionKey::new("DownBackU");
const SOURCE_DOWN_FORWARD_D_ACTION_KEY: SourceActionKey = SourceActionKey::new("DownFowardD");
const SOURCE_DOWN_BACK_D_ACTION_KEY: SourceActionKey = SourceActionKey::new("DownBackD");
const SOURCE_PASSIVE_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(199);
const SOURCE_PASSIVE_STAND_F_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(200);
const SOURCE_PASSIVE_STAND_B_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(201);
const SOURCE_PASSIVE_ACTION_KEY: SourceActionKey = SourceActionKey::new("Passive");
const SOURCE_PASSIVE_STAND_F_ACTION_KEY: SourceActionKey = SourceActionKey::new("PassiveStandF");
const SOURCE_PASSIVE_STAND_B_ACTION_KEY: SourceActionKey = SourceActionKey::new("PassiveStandB");
const SOURCE_DAMAGE_FALL_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(38);
const SOURCE_DAMAGE_FALL_ACTION_KEY: SourceActionKey = SourceActionKey::new("DamageFall");
const SOURCE_ATTACK12_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(45);
const SOURCE_ATTACK13_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(46);
const SOURCE_ATTACK100_START_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(47);
const SOURCE_ATTACK100_LOOP_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(48);
const SOURCE_ATTACK100_END_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(49);
const SOURCE_ATTACK12_ACTION_KEY: SourceActionKey = SourceActionKey::new("Attack12");
const SOURCE_ATTACK13_ACTION_KEY: SourceActionKey = SourceActionKey::new("Attack13");
const SOURCE_ATTACK100_START_ACTION_KEY: SourceActionKey = SourceActionKey::new("Attack100Start");
const SOURCE_ATTACK100_LOOP_ACTION_KEY: SourceActionKey = SourceActionKey::new("Attack100Loop");
const SOURCE_ATTACK100_END_ACTION_KEY: SourceActionKey = SourceActionKey::new("Attack100End");
const SOURCE_CATCH_PULL_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(213);
const SOURCE_CATCH_DASH_PULL_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(215);
const SOURCE_CATCH_WAIT_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(216);
const SOURCE_CATCH_WAIT_ACTION_KEY: SourceActionKey = SourceActionKey::new("CatchWait");
const SOURCE_CATCH_ATTACK_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(217);
const SOURCE_CATCH_ATTACK_ACTION_KEY: SourceActionKey = SourceActionKey::new("CatchAttack");
const SOURCE_CATCH_CUT_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(218);
const SOURCE_CATCH_CUT_ACTION_KEY: SourceActionKey = SourceActionKey::new("CatchCut");
const SOURCE_THROW_F_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(219);
const SOURCE_THROW_B_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(220);
const SOURCE_THROW_HI_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(221);
const SOURCE_THROW_LW_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(222);
const SOURCE_THROW_F_ACTION_KEY: SourceActionKey = SourceActionKey::new("ThrowF");
const SOURCE_THROW_B_ACTION_KEY: SourceActionKey = SourceActionKey::new("ThrowB");
const SOURCE_THROW_HI_ACTION_KEY: SourceActionKey = SourceActionKey::new("ThrowHi");
const SOURCE_THROW_LW_ACTION_KEY: SourceActionKey = SourceActionKey::new("ThrowLw");
const SOURCE_CAPTURE_PULLED_HI_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(223);
const SOURCE_CAPTURE_PULLED_HI_ACTION_KEY: SourceActionKey =
    SourceActionKey::new("CapturePulledHi");
const SOURCE_CAPTURE_WAIT_HI_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(224);
const SOURCE_CAPTURE_WAIT_HI_ACTION_KEY: SourceActionKey = SourceActionKey::new("CaptureWaitHi");
const SOURCE_CAPTURE_PULLED_LW_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(226);
const SOURCE_CAPTURE_WAIT_LW_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(227);
const SOURCE_CAPTURE_WAIT_LW_ACTION_KEY: SourceActionKey = SourceActionKey::new("CaptureWaitLw");
const SOURCE_CAPTURE_JUMP_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(228);
const SOURCE_CAPTURE_JUMP_ACTION_KEY: SourceActionKey = SourceActionKey::new("CaptureJump");
const SOURCE_CAPTURE_CUT_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(229);
const SOURCE_CAPTURE_CUT_ACTION_KEY: SourceActionKey = SourceActionKey::new("CaptureCut");
const SOURCE_THROWN_F_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(239);
const SOURCE_THROWN_B_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(240);
const SOURCE_THROWN_HI_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(241);
const SOURCE_THROWN_LW_ACTION_STATE_ID: MeleeActionStateId = MeleeActionStateId::new(242);
const SOURCE_THROWN_F_ACTION_KEY: SourceActionKey = SourceActionKey::new("TCaptainThrowF");
const SOURCE_THROWN_B_ACTION_KEY: SourceActionKey = SourceActionKey::new("TCaptainThrowB");
const SOURCE_THROWN_HI_ACTION_KEY: SourceActionKey = SourceActionKey::new("TCaptainThrowHi");
const SOURCE_THROWN_LW_ACTION_KEY: SourceActionKey = SourceActionKey::new("TCaptainThrowLw");

#[derive(Clone, Copy, Debug, PartialEq)]
struct PendingSourceThrowTransition {
    thrower_index: usize,
    victim_index: usize,
    victim_action_state_id: MeleeActionStateId,
    victim_source_action_key: SourceActionKey,
    facing: i8,
    anim_speed: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PendingSourceCaptureWaitTransition {
    grabber_index: usize,
    victim_index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PendingSourceCatchCutTransition {
    grabber_index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PendingSourceThrowRelease {
    thrower_index: usize,
    victim_index: usize,
    hitbox: SourceInstalledThrowHitbox,
    source_release_transn2_position: Option<SourceVec2>,
    source_release_last_pos: Option<SourceVec2>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PendingSourceThrowAnimFreeze {
    victim_index: usize,
    anim_timer: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct SourcePoseMetadataSnapshot {
    source_position: SourceVec2,
    model_facing: i8,
    capture_pose: Option<SourceCapturePose>,
    script_events: SourceActionScriptEvents,
    primary_hitbox: Option<SourceHitboxAttributes>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GroundEdgeCollisionPolicy {
    LedgeSlipWithStickGate,
    ClampToFloorEndpoint,
}

pub fn step_world(world: &mut World, frame: Frame, inputs: &[PlayerInput; 2]) {
    step_world_with_source_runtime_data(world, frame, inputs, |_| None, |_| None);
}

pub fn step_world_with_source_runtime_data(
    world: &mut World,
    frame: Frame,
    inputs: &[PlayerInput; 2],
    mut source_pose_metadata: impl FnMut(&PlayerState) -> Option<SourceActionPoseMetadata>,
    mut source_action_total_frames: impl FnMut(MeleeActionStateId) -> Option<u8>,
) {
    world.set_frame(frame);
    let previous_inputs = *world.previous_inputs();
    let mut input_timers = *world.input_timers();
    let mut last_input_facts = [MeleeInputFacts::default(); 2];
    let stage = world.stage();
    let common_data = world.common_data();
    let engine_features = world.engine_features();
    let player_profiles = world.players().map(|player| player.profile);
    let previous_action_state_ids = world.players().map(|player| player.melee_action_state_id);
    let source_hitlag_frames_before_tick = world.players().map(|player| player.hitlag_frames);
    compute_player_nudge_velocities_after_source_anim_callbacks(
        world.players_mut(),
        stage,
        common_data,
    );
    for player in world.players_mut() {
        player.source_previous_position = player.source_position;
        player.source_previous_position_z = player.source_position_z;
        player.source_collision_lightshield_amount = player.lightshield_amount;
    }
    let mut source_ledge_owners = source_ledge_owners(world.players());
    let mut pending_source_throw_transitions: [Option<PendingSourceThrowTransition>; PLAYER_COUNT] =
        [None; PLAYER_COUNT];
    let mut pending_source_capture_wait_transitions: [Option<PendingSourceCaptureWaitTransition>;
        PLAYER_COUNT] = [None; PLAYER_COUNT];
    let mut pending_source_catch_cut_transitions: [Option<PendingSourceCatchCutTransition>;
        PLAYER_COUNT] = [None; PLAYER_COUNT];
    let mut pending_source_throw_releases: [Option<PendingSourceThrowRelease>; PLAYER_COUNT] =
        [None; PLAYER_COUNT];
    let mut pending_source_throw_anim_freezes: [Option<PendingSourceThrowAnimFreeze>;
        PLAYER_COUNT] = [None; PLAYER_COUNT];
    let mut source_capture_released_this_tick = [false; PLAYER_COUNT];
    let mut source_capture_wait_anim_advanced_this_tick = [false; PLAYER_COUNT];
    let mut skip_source_player_tick = [false; PLAYER_COUNT];
    let mut source_pose_metadata_snapshots = world.players().map(|player| {
        let metadata = source_pose_metadata(&player);
        SourcePoseMetadataSnapshot {
            source_position: player.source_position,
            model_facing: player_model_facing(&player),
            capture_pose: metadata.and_then(|metadata| metadata.capture_pose),
            script_events: metadata
                .map(|metadata| metadata.script_events)
                .unwrap_or_default(),
            primary_hitbox: metadata.and_then(|metadata| metadata.primary_hitbox),
        }
    });

    for player_index in 0..PLAYER_COUNT {
        let previous_input = previous_inputs[player_index];
        let input_snapshot = inputs[player_index].melee_snapshot_with_config(
            previous_input,
            input_timers[player_index],
            common_data.input_config(),
        );
        let input_facts = input_snapshot.facts(common_data.input_thresholds());
        let player = &mut world.players_mut()[player_index];
        if player
            .melee_action_state_id
            .is_some_and(source_capture_wait_state)
        {
            if let Some(grabber_index) = advance_source_capture_wait_state(
                player,
                player_index,
                input_facts,
                input_snapshot,
                common_data,
                &mut source_action_total_frames,
                &mut source_capture_released_this_tick,
            ) {
                pending_source_catch_cut_transitions[player_index] =
                    Some(PendingSourceCatchCutTransition { grabber_index });
            }
            source_capture_wait_anim_advanced_this_tick[player_index] = true;
        }
    }

    for player_index in 0..PLAYER_COUNT {
        apply_pending_source_throw_releases_for_victim(
            world,
            &mut pending_source_throw_releases,
            player_index,
            previous_inputs[player_index],
            &mut input_timers,
            &mut source_action_total_frames,
        );
        let input = inputs[player_index];
        let previous_input = previous_inputs[player_index];
        let previous_input_timers = input_timers[player_index];
        let player = &mut world.players_mut()[player_index];
        if player.player_state != PLAYER_STATE_IN_GAME {
            continue;
        }
        if skip_source_player_tick[player_index] {
            continue;
        }
        let input_snapshot = input.melee_snapshot_with_config(
            previous_input,
            input_timers[player_index],
            common_data.input_config(),
        );
        let stick_x = input_snapshot.lstick.0 as i32;
        let stick_y = input_snapshot.lstick.1;
        let pre_input_stick_x = input_snapshot.prev_lstick.0 as i32;
        let input_facts = input_snapshot.facts(common_data.input_thresholds());
        last_input_facts[player_index] = input_facts;
        input_timers[player_index].x_tap = input_snapshot.x_tap_timer;
        input_timers[player_index].y_tap = input_snapshot.y_tap_timer;
        input_timers[player_index].trigger = input_snapshot.trigger_timer;
        let x_tap_timer = input_timers[player_index].x_tap;
        let y_tap_timer = input_timers[player_index].y_tap;
        let trigger_timer = input_timers[player_index].trigger;
        advance_source_lr_digital_press_timers(player, input_facts);
        tick_source_collision_lifecycle(player);
        tick_source_ledge_cooldown(player);
        if tick_guard_shield_lifecycle(player, input_facts, common_data) {
            continue;
        }
        source_update_guard_shield_visual(player, input_facts, common_data);
        if player.hitlag_frames > 0 {
            begin_ground_velocity_tick(player);
            let has_source_damage_hitlag_callbacks = player.source_allow_sdi;
            let final_hitlag_tick = player.hitlag_frames == 1;
            let hitlag_input = if final_hitlag_tick {
                previous_input
            } else {
                input
            };
            let mut source_hitlag_input_timers = if final_hitlag_tick {
                previous_input_timers
            } else {
                input_timers[player_index]
            };
            if has_source_damage_hitlag_callbacks {
                apply_source_damage_on_every_hitlag(
                    player,
                    hitlag_input,
                    &mut source_hitlag_input_timers,
                    common_data,
                );
            }
            player.hitlag_frames = player.hitlag_frames.saturating_sub(1);
            if player.hitlag_frames == 0 && has_source_damage_hitlag_callbacks {
                apply_source_damage_on_exit_hitlag(player, previous_input, common_data);
            }
            if player.hitlag_frames == 0 {
                player.source_x2219_b5 = false;
            }
            if player.hitlag_frames > 0 {
                if has_source_damage_hitlag_callbacks {
                    input_timers[player_index] = source_hitlag_input_timers;
                }
                continue;
            }
        }
        tick_source_hurt_collision_lockout(player);
        if player.source_x2219_b5 {
            let linked_timer_active = player
                .source_x1a5c_index
                .map(usize::from)
                .and_then(|linked_index| source_hitlag_frames_before_tick.get(linked_index))
                .is_some_and(|frames| *frames > 1);
            if linked_timer_active {
                continue;
            }
            player.source_x2219_b5 = false;
        }
        let previous_position = player.position;
        let previous_source_position = player.source_position;
        let previous_ecb_bottom = ecb_bottom_world_position_for_motion_frame(
            player,
            previous_position,
            collision_ecb_motion_frame(player),
            common_data,
        );
        let mut skip_position_update_this_tick = false;
        let mut skip_airborne_vertical_physics_this_tick = false;
        let mut skip_airborne_collision_this_tick = false;
        let mut enter_fall_special_after_position_update = false;
        let mut fall_entered_from_rebirth_wait_this_tick = false;
        begin_ground_velocity_tick(player);
        let motion_state_before_tick = player.motion_state;
        let mut skip_action_dispatch_this_tick = false;
        if is_source_damage_player(player) {
            skip_action_dispatch_this_tick = advance_source_damage_state(
                player,
                stage,
                common_data,
                previous_ecb_bottom,
                input_facts,
                stick_x,
                stick_y,
                &mut source_pose_metadata,
                &mut source_action_total_frames,
            );
            if !skip_action_dispatch_this_tick {
                continue;
            }
        }
        if is_source_passive_player(player) {
            if source_action_completed_before_frame_input(player) {
                enter_source_common_action_end(player);
            } else {
                advance_source_passive_state(player, stage, common_data, stick_x);
                continue;
            }
        }
        if is_source_down_bound_player(player) {
            advance_source_down_bound_state(
                player,
                stage,
                common_data,
                previous_ecb_bottom,
                stick_x,
                stick_y as i32,
                &mut source_pose_metadata,
                &mut source_action_total_frames,
            );
            continue;
        }
        if is_source_down_wait_player(player) {
            advance_source_down_wait_state(
                player,
                stage,
                common_data,
                input_facts,
                stick_x,
                stick_y as i32,
                &mut source_action_total_frames,
            );
            continue;
        }
        if is_source_down_stand_player(player) {
            advance_source_down_stand_state(player, stage, common_data, stick_x);
            continue;
        }
        if is_source_down_attack_player(player) {
            advance_source_down_attack_state(player, stage, common_data, stick_x);
            continue;
        }
        if is_source_down_roll_player(player) {
            advance_source_down_roll_state(player, stage, common_data, stick_x);
            continue;
        }
        if is_source_grab_capture_player(player) {
            let (
                throw_transition,
                capture_wait_transition,
                catch_cut_transition,
                throw_release,
                throw_anim_freeze,
            ) = advance_source_grab_capture_state(
                player,
                player_index,
                input_facts,
                input_snapshot,
                stage,
                common_data,
                &mut source_pose_metadata_snapshots,
                &mut source_pose_metadata,
                &mut source_action_total_frames,
                &mut source_capture_released_this_tick,
                source_capture_wait_anim_advanced_this_tick[player_index],
                player_profiles,
            );
            if player.motion_state == MotionState::Wait {
                // Fighter_procUpdate runs the old Throw animation callback first,
                // then dispatches the newly entered Wait IASA callback in the
                // same fighter tick. Physics has not run yet.
                apply_wait_state_inputs(
                    player,
                    input_facts,
                    stick_x,
                    stage,
                    common_data,
                    &mut input_timers[player_index].x_tap,
                );
                // Dash entry stages its initial speed in x80/x84. The same
                // tick's physics phase commits it without moving the fighter;
                // position integration uses the pre-entry ground velocity.
                commit_ground_velocity(player, stage);
            }
            if let Some(transition) = throw_transition {
                skip_source_player_tick[transition.victim_index] = true;
            }
            if let Some(freeze) = throw_anim_freeze {
                skip_source_player_tick[freeze.victim_index] = true;
            }
            pending_source_throw_transitions[player_index] = throw_transition;
            pending_source_capture_wait_transitions[player_index] = capture_wait_transition;
            if catch_cut_transition.is_some() {
                pending_source_catch_cut_transitions[player_index] = catch_cut_transition;
            }
            pending_source_throw_releases[player_index] = throw_release;
            pending_source_throw_anim_freezes[player_index] = throw_anim_freeze;
            if let Some(release) = throw_release {
                if release.victim_index > player_index {
                    apply_pending_source_throw_releases_for_victim(
                        world,
                        &mut pending_source_throw_releases,
                        release.victim_index,
                        previous_inputs[release.victim_index],
                        &mut input_timers,
                        &mut source_action_total_frames,
                    );
                }
            }
            continue;
        }
        if is_source_dead_motion_state(player.motion_state) {
            advance_source_dead_state(player, player_index, stage, common_data);
            continue;
        }
        if is_source_rebirth_motion_state(player.motion_state) {
            if advance_source_rebirth_state(player, input_facts, common_data) {
                fall_entered_from_rebirth_wait_this_tick = player.motion_state == MotionState::Fall;
            } else {
                continue;
            }
        }

        if !skip_action_dispatch_this_tick {
            match player.motion_state {
                MotionState::Entry => {
                    advance_entry(player, common_data);
                    skip_position_update_this_tick = true;
                }
                MotionState::EntryStart => {
                    advance_entry_start(player, stage, common_data);
                    skip_position_update_this_tick = true;
                }
                MotionState::EntryEnd => {
                    skip_position_update_this_tick = advance_entry_end(player, stage, common_data);
                    skip_airborne_collision_this_tick =
                        !skip_position_update_this_tick && player.motion_state == MotionState::Fall;
                }
                MotionState::Wait => {
                    wait_anim_tick(player);
                    apply_wait_state_inputs(
                        player,
                        input_facts,
                        stick_x,
                        stage,
                        common_data,
                        &mut input_timers[player_index].x_tap,
                    );
                    if player.motion_state == MotionState::GuardOn && player.motion_frame == 0 {
                        // Wait's IASA installs GuardOn after this fighter's current
                        // collision pass. GuardOn_Phys registers it on the next tick.
                        player.source_shield_collision_active = false;
                    }
                }
                MotionState::WalkSlow | MotionState::WalkMiddle | MotionState::WalkFast => {
                    walk_anim_tick(player);
                    let walk_forward_dash = fresh_walk_forward_dash_direction(
                        input_facts,
                        x_tap_timer,
                        player.facing,
                        common_data,
                    );
                    let walk_smash_turn = fresh_walk_smash_turn_direction(
                        input_facts,
                        x_tap_timer,
                        player.facing,
                        common_data,
                    );

                    if !player.grounded {
                        enter_fall(player);
                    } else if let Some(action_state) = walk_action_state(input_facts) {
                        enter_action_state(player, action_state, stick_x, stage, common_data);
                    } else if input_facts.digital_shield_pressed
                        && trigger_timer < common_data.guard_reflect_input_window
                    {
                        enter_guard_reflect(player, common_data);
                        apply_ground_traction(player, stage, common_data);
                    } else if decomp_guard_held_input(input_facts, common_data) {
                        enter_guard(player, input_facts, common_data);
                        source_update_guard_shield_visual(player, input_facts, common_data);
                        apply_ground_traction(player, stage, common_data);
                    } else if input_facts.normal_jump_pressed {
                        enter_knee_bend_from_ground(
                            player,
                            input_facts.normal_jump_input,
                            stage,
                            common_data,
                        );
                    } else if walk_forward_dash != 0 {
                        enter_dash(player, walk_forward_dash, true);
                        input_timers[player_index].x_tap = EXPIRED_INPUT_TIMER;
                    } else if walk_smash_turn != 0 {
                        enter_smash_turn(player, walk_smash_turn);
                        apply_ground_traction(player, stage, common_data);
                    } else if input_facts.crouch {
                        enter_squat(player);
                        apply_ground_traction(player, stage, common_data);
                    } else if input_facts.walk_direction == player.facing {
                        let next_walk_state = walk_motion_state(player, common_data);
                        if next_walk_state != player.motion_state {
                            player.set_motion_state_alias(next_walk_state);
                            player.set_source_motion_anim_rate_milli(1_000);
                            player.walk_anim_velocity_x = player.ground_velocity_x;
                        }
                        apply_walk_velocity(player, stick_x, common_data);
                    } else {
                        enter_wait_from_walk(player);
                        apply_ground_traction(player, stage, common_data);
                    }
                }
                MotionState::Dash => {
                    dash_anim_tick(player);
                    if !player.grounded {
                        enter_fall(player);
                    } else if player.motion_state != MotionState::Dash {
                        if player.motion_state == MotionState::Wait {
                            apply_wait_state_inputs(
                                player,
                                input_facts,
                                stick_x,
                                stage,
                                common_data,
                                &mut input_timers[player_index].x_tap,
                            );
                        }
                    } else if let Some(action_state) = dash_action_state(
                        input_facts,
                        player.motion_frame,
                        player.dash_started_from_tap,
                        player.facing,
                        trigger_timer,
                        common_data,
                    ) {
                        enter_dash_iasa_action_state(
                            player,
                            action_state,
                            stick_x,
                            stage,
                            common_data,
                        );
                    } else {
                        let smash_turn_direction = if is_fresh_walk_dash_tap(
                            input_timers[player_index].x_tap,
                            common_data,
                        ) {
                            input_facts.smash_turn_direction(player.facing)
                        } else {
                            0
                        };
                        if dash_allows_opposite_dashback(player, common_data)
                            && smash_turn_direction != 0
                        {
                            let ground_velocity = staged_ground_velocity_x(player);
                            enter_smash_turn(player, smash_turn_direction);
                            set_ground_velocity_x(
                                player,
                                dash_iasa_decayed_ground_velocity(ground_velocity, common_data),
                            );
                            apply_ground_traction(player, stage, common_data);
                        } else if input_facts.shield_held {
                            let ground_velocity = staged_ground_velocity_x(player);
                            enter_guard_from_run(player, input_facts, common_data);
                            source_update_guard_shield_visual(player, input_facts, common_data);
                            set_ground_velocity_x(
                                player,
                                dash_iasa_decayed_ground_velocity(ground_velocity, common_data),
                            );
                            apply_ground_traction(player, stage, common_data);
                        } else if input_facts.normal_jump_pressed {
                            enter_knee_bend_from_ground(
                                player,
                                input_facts.normal_jump_input,
                                stage,
                                common_data,
                            );
                        } else if player.motion_cmd_var0 != 0
                            && is_same_direction_run(stick_x, player.facing, common_data)
                        {
                            enter_run(player);
                            apply_run_velocity(player, stick_x, stage, common_data);
                        } else {
                            apply_dash_physics(player, stick_x, stage, common_data);
                        }
                    }
                }
                MotionState::Run => {
                    run_anim_tick(player);
                    apply_run_state_inputs(player, input_facts, stick_x, stage, common_data);
                }
                MotionState::RunDirect => {
                    if !player.grounded {
                        enter_fall(player);
                    } else if let Some(action_state) = run_action_state(input_facts) {
                        enter_action_state(player, action_state, stick_x, stage, common_data);
                    } else if input_facts.shield_held {
                        enter_guard_from_run(player, input_facts, common_data);
                        source_update_guard_shield_visual(player, input_facts, common_data);
                    } else if input_facts.normal_jump_pressed {
                        enter_knee_bend_from_ground(
                            player,
                            input_facts.normal_jump_input,
                            stage,
                            common_data,
                        );
                    } else if is_same_direction_run(stick_x, player.facing, common_data) {
                        enter_run_from_run_direct(player);
                        apply_run_velocity(player, stick_x, stage, common_data);
                    } else if run_direct_releases_to_wait(stick_x, player.facing, common_data) {
                        enter_wait_from_walk(player);
                        apply_ground_traction(player, stage, common_data);
                    } else {
                        advance_source_bound_motion_frame(player);
                        if stick_x == 0 {
                            apply_run_ground_traction(player, stage, common_data);
                        } else {
                            apply_run_velocity(player, stick_x, stage, common_data);
                        }
                    }
                }
                MotionState::RunBrake => {
                    run_brake_anim_tick(player, common_data);
                    if !player.grounded {
                        enter_fall(player);
                    } else if player.motion_state != MotionState::RunBrake {
                        // RunBrake_Anim can fall back before IASA/Phys.
                    } else if input_facts.normal_jump_pressed {
                        enter_knee_bend_from_ground(
                            player,
                            input_facts.normal_jump_input,
                            stage,
                            common_data,
                        );
                    } else if player.motion_cmd_var0 != 0
                        && is_opposite_run_turn(stick_x, player.facing, common_data)
                    {
                        enter_turn_run(player, stick_x, stage, common_data, player.motion_frame);
                    } else if input_facts.crouch {
                        enter_squat(player);
                    } else {
                        apply_run_ground_traction(player, stage, common_data);
                    }
                }
                MotionState::TurnRun => {
                    let turn_run_anim_outcome = turn_run_anim_tick(player, stick_x, common_data);
                    if !player.grounded {
                        clear_turn_state(player);
                        enter_fall(player);
                    } else if turn_run_anim_outcome == TurnRunAnimOutcome::EnteredRun {
                        // ftCo_TurnRun_Anim installs Run callbacks before the
                        // frame's input/physics pass, so fresh Run_IASA branches
                        // such as jump can still win on the handoff tick.
                        apply_run_state_inputs(player, input_facts, stick_x, stage, common_data);
                    } else if player.motion_state != MotionState::TurnRun {
                        // TurnRun_Anim can complete into Run or fall back before IASA/Phys.
                    } else if input_facts.normal_jump_pressed {
                        enter_knee_bend_from_ground(
                            player,
                            input_facts.normal_jump_input,
                            stage,
                            common_data,
                        );
                    } else {
                        apply_turn_run_velocity(player, stick_x, stage, common_data);
                    }
                }
                MotionState::Turn => {
                    if !player.grounded {
                        clear_turn_state(player);
                        enter_fall(player);
                    } else {
                        advance_turn_anim(player);
                        apply_ucf_dashback_turn_hook(
                            player,
                            input.ucf_dashback_amendment(),
                            stick_x,
                            common_data,
                        );

                        if player.motion_frame >= player.profile.standing_turn_total_frames {
                            player.set_motion_state_alias(MotionState::Wait);
                            player.motion_frame = 0;
                            clear_turn_state(player);
                            apply_wait_state_inputs(
                                player,
                                input_facts,
                                stick_x,
                                stage,
                                common_data,
                                &mut input_timers[player_index].x_tap,
                            );
                        } else {
                            let turn_facts = turn_effective_input_facts(player, input_facts);
                            if let Some(action_state) = turn_action_state(turn_facts) {
                                if player.facing != player.turn_facing_after {
                                    player.facing = player.turn_facing_after;
                                    player.source_model_facing = player.facing;
                                }
                                enter_action_state(
                                    player,
                                    action_state,
                                    stick_x,
                                    stage,
                                    common_data,
                                );
                            } else if turn_facts.digital_shield_pressed
                                && input_timers[player_index].trigger
                                    < common_data.guard_reflect_input_window
                            {
                                if player.facing != player.turn_facing_after {
                                    player.facing = player.turn_facing_after;
                                    player.source_model_facing = player.facing;
                                }
                                enter_guard_reflect(player, common_data);
                                apply_ground_traction(player, stage, common_data);
                            } else if input_facts.shield_held {
                                enter_guard(player, input_facts, common_data);
                                source_update_guard_shield_visual(player, input_facts, common_data);
                            } else if input_facts.normal_jump_pressed {
                                enter_knee_bend_from_ground(
                                    player,
                                    input_facts.normal_jump_input,
                                    stage,
                                    common_data,
                                );
                            } else {
                                arm_turn_dash_after_if_fresh(
                                    player,
                                    stick_x,
                                    x_tap_timer,
                                    common_data,
                                );
                                if player.turn_just_turned
                                    && player.turn_dash_after_direction != 0
                                    && stick_x * player.turn_facing_after as i32
                                        >= common_data.dash_x as i32
                                {
                                    enter_dash(player, player.turn_facing_after, false);
                                    input_timers[player_index].x_tap = EXPIRED_INPUT_TIMER;
                                } else {
                                    latch_turn_buttons(player, input_facts);
                                    if player.turn_just_turned {
                                        player.turn_just_turned = false;
                                    }
                                }
                            }

                            if player.motion_state == MotionState::Turn {
                                apply_ground_traction(player, stage, common_data);
                            }
                        }
                    }
                }
                MotionState::Squat => {
                    if !player.grounded {
                        enter_fall(player);
                    } else if let Some(action_state) = wait_action_state(input_facts) {
                        enter_action_state(player, action_state, stick_x, stage, common_data);
                    } else if input_facts.digital_shield_pressed
                        && trigger_timer < common_data.guard_reflect_input_window
                    {
                        enter_guard_reflect(player, common_data);
                        apply_ground_traction(player, stage, common_data);
                    } else if input_facts.shield_held {
                        enter_guard(player, input_facts, common_data);
                        source_update_guard_shield_visual(player, input_facts, common_data);
                    } else if input_facts.normal_jump_pressed {
                        enter_knee_bend_from_ground(
                            player,
                            input_facts.normal_jump_input,
                            stage,
                            common_data,
                        );
                    } else if arm_squat_platform_pass(
                        player,
                        stage,
                        stick_y,
                        y_tap_timer,
                        common_data,
                    ) {
                        advance_squat_frame(player, stage, common_data, stick_y);
                    } else if player.platform_pass_pending
                        && advance_squat_platform_pass(player, stage, common_data, stick_x)
                    {
                        input_timers[player_index].y_tap = EXPIRED_INPUT_TIMER;
                    } else {
                        advance_squat_frame(player, stage, common_data, stick_y);
                    }
                }
                MotionState::SquatWait => {
                    if !player.grounded {
                        enter_fall(player);
                    } else if let Some(action_state) = wait_action_state(input_facts) {
                        enter_action_state(player, action_state, stick_x, stage, common_data);
                    } else if input_facts.digital_shield_pressed
                        && trigger_timer < common_data.guard_reflect_input_window
                    {
                        enter_guard_reflect(player, common_data);
                        apply_ground_traction(player, stage, common_data);
                    } else if input_facts.shield_held {
                        enter_guard(player, input_facts, common_data);
                        source_update_guard_shield_visual(player, input_facts, common_data);
                    } else if input_facts.normal_jump_pressed {
                        enter_knee_bend_from_ground(
                            player,
                            input_facts.normal_jump_input,
                            stage,
                            common_data,
                        );
                    } else if input_facts.forward_dash_direction(player.facing) != 0 {
                        enter_dash(player, player.facing, true);
                        input_timers[player_index].x_tap = EXPIRED_INPUT_TIMER;
                    } else if crouch_released(stick_y, common_data) {
                        enter_squat_rv(player);
                    } else {
                        advance_source_bound_motion_frame(player);
                        clear_ground_horizontal_velocity(player);
                        player.velocity.y = 0;
                    }
                }
                MotionState::SquatRv => {
                    if !player.grounded {
                        enter_fall(player);
                    } else {
                        let next_motion_frame = player.motion_frame.saturating_add(1);
                        if next_motion_frame >= player.profile.action_frames.squat_rv_total_frames {
                            player.set_motion_state_alias(MotionState::Wait);
                            player.motion_frame = 0;
                            player.set_source_motion_anim_frame(0.0);
                            apply_wait_state_inputs(
                                player,
                                input_facts,
                                stick_x,
                                stage,
                                common_data,
                                &mut input_timers[player_index].x_tap,
                            );
                        } else if let Some(action_state) = wait_action_state(input_facts) {
                            enter_action_state(player, action_state, stick_x, stage, common_data);
                        } else if input_facts.shield_held {
                            enter_guard(player, input_facts, common_data);
                            source_update_guard_shield_visual(player, input_facts, common_data);
                        } else if input_facts.normal_jump_pressed {
                            enter_knee_bend_from_ground(
                                player,
                                input_facts.normal_jump_input,
                                stage,
                                common_data,
                            );
                        } else if input_facts.walk_direction == player.facing {
                            enter_walk(
                                player,
                                walk_motion_state(player, common_data),
                                stick_x,
                                common_data,
                            );
                        } else {
                            player.motion_frame = next_motion_frame;
                            apply_ground_traction(player, stage, common_data);
                        }
                    }
                }
                MotionState::Catch | MotionState::CatchDash => {
                    advance_source_motion_frame(player);
                    apply_catch_ground_physics(player, stage, common_data);
                    if let Some(next_state) =
                        grounded_action_iasa_state(player, input_facts, common_data)
                    {
                        enter_iasa_state(
                            player,
                            next_state,
                            input_facts,
                            input_facts.normal_jump_input,
                            stick_x,
                            stage,
                            common_data,
                        );
                    } else if player.motion_frame >= grounded_action_total_frames(player) {
                        player.set_motion_state_alias(MotionState::Wait);
                        player.motion_frame = 0;
                        clear_motion_script_state(player);
                        if input_facts.shield_held {
                            enter_guard(player, input_facts, common_data);
                            source_update_guard_shield_visual(player, input_facts, common_data);
                        }
                    }
                }
                MotionState::SpecialN
                | MotionState::SpecialSStart
                | MotionState::SpecialS
                | MotionState::SpecialLw
                | MotionState::Attack1
                | MotionState::AttackDash
                | MotionState::AttackS3
                | MotionState::AttackHi3
                | MotionState::AttackLw3
                | MotionState::AttackS4
                | MotionState::AttackHi4
                | MotionState::AttackLw4
                | MotionState::EscapeN => {
                    advance_source_motion_frame(player);
                    let script_events =
                        source_current_anim_script_events(player, &mut source_pose_metadata);
                    apply_source_script_events(player, script_events, common_data);
                    if player.motion_frame >= grounded_action_total_frames(player) {
                        // The animation callback changes the live action-state
                        // callback table before the later input and physics procs.
                        player.set_motion_state_alias(MotionState::Wait);
                        player.motion_frame = 0;
                        clear_motion_script_state(player);
                        apply_wait_state_inputs(
                            player,
                            input_facts,
                            stick_x,
                            stage,
                            common_data,
                            &mut input_timers[player_index].x_tap,
                        );
                    } else {
                        if matches!(
                            player.motion_state,
                            MotionState::SpecialSStart | MotionState::SpecialS
                        ) {
                            apply_source_ft_80084fa8_ground_physics(player, stage, common_data);
                        } else if player.motion_state == MotionState::AttackDash {
                            apply_source_root_ground_motion(player);
                        } else if player.motion_state == MotionState::EscapeN {
                            apply_ground_traction(player, stage, common_data);
                        } else {
                            clear_ground_horizontal_velocity(player);
                        }
                        if player.motion_state == MotionState::Attack1 {
                            apply_attack100_loop_input(player, input_facts);
                        }
                        if player.motion_state == MotionState::Attack1
                            && apply_rapid_jab_input(
                                player,
                                input_facts,
                                &mut source_action_total_frames,
                            )
                        {
                        } else if player.motion_state == MotionState::Attack1
                            && apply_jab_followup_input(player, input_facts)
                        {
                        } else if player.motion_state == MotionState::Attack1
                            && advance_attack100_state_if_needed(
                                player,
                                &mut source_action_total_frames,
                            )
                        {
                        } else if let Some(next_state) =
                            grounded_action_iasa_state(player, input_facts, common_data)
                        {
                            enter_iasa_state(
                                player,
                                next_state,
                                input_facts,
                                input_facts.normal_jump_input,
                                stick_x,
                                stage,
                                common_data,
                            );
                            if matches!(
                                player.motion_state,
                                MotionState::Squat | MotionState::SquatWait | MotionState::SquatRv
                            ) {
                                apply_ground_traction(player, stage, common_data);
                            }
                        }
                    }
                }
                MotionState::SpecialHi => {
                    advance_source_motion_frame(player);
                    let script_events =
                        source_anim_command_script_events(player, &mut source_pose_metadata);
                    apply_source_script_events(player, script_events, common_data);
                    if source_action_completed_before_frame_input(player) {
                        enter_falcon_special_hi_fall_special(player);
                        apply_air_drift(player, stick_x, common_data);
                    } else {
                        apply_falcon_special_hi_iasa(player, stick_x);
                        apply_falcon_special_hi_physics(player, stick_x, common_data);
                        skip_airborne_vertical_physics_this_tick = true;
                    }
                }
                MotionState::EscapeF | MotionState::EscapeB => {
                    advance_source_bound_motion_frame(player);
                    apply_escape_script_events(player);
                    apply_escape_anim_events(player);
                    apply_source_root_ground_motion(player);
                    if let Some(next_state) =
                        grounded_action_iasa_state(player, input_facts, common_data)
                    {
                        enter_iasa_state(
                            player,
                            next_state,
                            input_facts,
                            input_facts.normal_jump_input,
                            stick_x,
                            stage,
                            common_data,
                        );
                    } else if player.motion_frame > grounded_action_total_frames(player) {
                        clear_ground_horizontal_velocity(player);
                        player.set_motion_state_alias(MotionState::Wait);
                        player.motion_frame = 0;
                        clear_motion_script_state(player);
                    }
                }
                MotionState::SpecialAirN
                | MotionState::SpecialAirSStart
                | MotionState::SpecialAirS
                | MotionState::SpecialAirHi
                | MotionState::SpecialAirLw => {
                    advance_source_motion_frame(player);
                    if player.motion_state == MotionState::SpecialAirHi {
                        let script_events =
                            source_anim_command_script_events(player, &mut source_pose_metadata);
                        apply_source_script_events(player, script_events, common_data);
                        if source_action_completed_before_frame_input(player) {
                            enter_falcon_special_hi_fall_special(player);
                            apply_air_drift(player, stick_x, common_data);
                        } else {
                            apply_falcon_special_hi_iasa(player, stick_x);
                            apply_falcon_special_hi_physics(player, stick_x, common_data);
                            skip_airborne_vertical_physics_this_tick = true;
                        }
                    }
                }
                MotionState::AttackAirN
                | MotionState::AttackAirF
                | MotionState::AttackAirB
                | MotionState::AttackAirHi
                | MotionState::AttackAirLw => {
                    apply_attack_air_script_events(player);
                    advance_source_motion_frame(player);
                    let script_events =
                        source_current_anim_script_events(player, &mut source_pose_metadata);
                    apply_source_script_events(player, script_events, common_data);
                    apply_attack_air_script_events(player);
                    if source_action_has_no_frames_remaining(player) {
                        enter_fall(player);
                    }
                    if matches!(
                        player.motion_state,
                        MotionState::Fall
                            | MotionState::FallF
                            | MotionState::FallB
                            | MotionState::FallAerial
                            | MotionState::FallAerialF
                            | MotionState::FallAerialB
                    ) {
                        if apply_airborne_iasa_or_drift(
                            player,
                            input_facts,
                            stick_x,
                            stick_y,
                            common_data,
                        ) && apply_entered_airborne_action_physics(player, stick_x, common_data)
                        {
                            skip_airborne_vertical_physics_this_tick = true;
                        }
                    } else if apply_attack_air_iasa_actions(
                        player,
                        input_facts,
                        stick_x,
                        common_data,
                    ) {
                        if apply_entered_airborne_action_physics(player, stick_x, common_data) {
                            skip_airborne_vertical_physics_this_tick = true;
                        }
                    } else {
                        apply_air_drift(player, stick_x, common_data);
                    }
                }
                MotionState::GuardOn => {
                    if !player.grounded {
                        clear_guard_state(player);
                        enter_fall(player);
                    } else {
                        latch_guard_release_if_needed(player, input_facts);
                        if guard_on_release_exits_this_tick(player) {
                            advance_source_bound_motion_frame(player);
                            if engine_features.shield_turnaround_during_guard {
                                update_shield_turn(player, input_facts);
                            }
                            apply_ground_traction(player, stage, common_data);
                            source_guard_phys_updates_shield_hit(player);
                            player.velocity.y = 0;
                            enter_guard_off(player);
                        } else if player.motion_frame < common_data.guard_reflect_input_window
                            && input_facts.digital_shield_pressed
                            && trigger_timer < common_data.guard_reflect_input_window
                        {
                            enter_guard_reflect_from_guard_on(player, common_data);
                            apply_ground_traction(player, stage, common_data);
                        } else if let Some(action_state) = guard_on_action_state(
                            player,
                            input_facts,
                            GuardInputContext {
                                facing: player.facing,
                                stage,
                                y_tap_timer,
                                stick_y,
                                ucf_shield_drop_amendment: input.ucf_shield_drop_amendment(),
                                common_data,
                            },
                        ) {
                            if action_state == MotionState::Pass {
                                enter_pass_with_same_frame_horizontal_physics(
                                    player,
                                    stage,
                                    common_data,
                                    stick_x,
                                );
                                input_timers[player_index].y_tap = EXPIRED_INPUT_TIMER;
                            } else {
                                enter_iasa_state(
                                    player,
                                    action_state,
                                    input_facts,
                                    input_facts.jump_input,
                                    stick_x,
                                    stage,
                                    common_data,
                                );
                            }
                        } else {
                            advance_source_bound_motion_frame(player);
                            if engine_features.shield_turnaround_during_guard {
                                update_shield_turn(player, input_facts);
                            }
                            apply_ground_traction(player, stage, common_data);
                            source_guard_phys_updates_shield_hit(player);
                            player.velocity.y = 0;
                            if player.motion_frame
                                >= player.profile.action_frames.guard_on_total_frames
                            {
                                if input_facts.shield_held {
                                    enter_guard_steady(player);
                                } else {
                                    enter_guard_off(player);
                                }
                            }
                        }
                    }
                }
                MotionState::Guard | MotionState::GuardReflect => {
                    if !player.grounded {
                        clear_guard_state(player);
                        enter_fall(player);
                    } else {
                        if player.motion_state == MotionState::GuardReflect {
                            tick_source_guard_reflect_timers(player);
                        }
                        latch_guard_release_if_needed(player, input_facts);
                        if player.guard_release_latched {
                            enter_guard_off(player);
                        } else if let Some(action_state) = guard_action_state(
                            player,
                            input_facts,
                            GuardInputContext {
                                facing: player.facing,
                                stage,
                                y_tap_timer,
                                stick_y,
                                ucf_shield_drop_amendment: input.ucf_shield_drop_amendment(),
                                common_data,
                            },
                        ) {
                            if action_state == MotionState::Pass {
                                enter_pass_with_same_frame_horizontal_physics(
                                    player,
                                    stage,
                                    common_data,
                                    stick_x,
                                );
                                input_timers[player_index].y_tap = EXPIRED_INPUT_TIMER;
                            } else {
                                enter_iasa_state(
                                    player,
                                    action_state,
                                    input_facts,
                                    input_facts.jump_input,
                                    stick_x,
                                    stage,
                                    common_data,
                                );
                            }
                        } else if input_facts.shield_held {
                            advance_source_bound_motion_frame(player);
                            if engine_features.shield_turnaround_during_guard {
                                update_shield_turn(player, input_facts);
                            }
                            apply_ground_traction(player, stage, common_data);
                            if player.motion_state != MotionState::GuardReflect {
                                source_guard_phys_updates_shield_hit(player);
                            }
                            player.velocity.y = 0;
                            if player.motion_state == MotionState::GuardReflect
                                && player.motion_frame >= guard_startup_total_frames(player)
                            {
                                enter_guard_steady(player);
                            }
                        }
                    }
                }
                MotionState::GuardOff => {
                    if !player.grounded {
                        enter_fall(player);
                    } else {
                        advance_source_bound_motion_frame(player);
                        apply_ground_traction(player, stage, common_data);
                        player.velocity.y = 0;
                        if let Some(action_state) = guard_off_action_state(input_facts) {
                            enter_iasa_state(
                                player,
                                action_state,
                                input_facts,
                                input_facts.jump_input,
                                stick_x,
                                stage,
                                common_data,
                            );
                        } else if player.motion_frame
                            >= player.profile.action_frames.guard_off_total_frames
                        {
                            player.set_motion_state_alias(MotionState::Wait);
                            player.motion_frame = 0;
                        }
                    }
                }
                MotionState::GuardSetOff => {
                    if !player.grounded {
                        clear_guard_state(player);
                        enter_fall(player);
                    } else {
                        latch_guard_release_if_needed(player, input_facts);
                        tick_source_guard_reflect_timers(player);
                        advance_source_bound_motion_frame(player);
                        player.source_common_timer = player.source_common_timer.saturating_sub(1);
                        apply_ground_traction(player, stage, common_data);
                        source_guard_phys_updates_shield_hit(player);
                        player.velocity.y = 0;
                        if player.source_common_timer == 0 {
                            if player.guard_release_latched || !input_facts.shield_held {
                                enter_guard_off(player);
                            } else if let Some(action_state) = guard_action_state(
                                player,
                                input_facts,
                                GuardInputContext {
                                    facing: player.facing,
                                    stage,
                                    y_tap_timer,
                                    stick_y,
                                    ucf_shield_drop_amendment: input.ucf_shield_drop_amendment(),
                                    common_data,
                                },
                            ) {
                                if action_state == MotionState::Pass {
                                    enter_pass_with_same_frame_horizontal_physics(
                                        player,
                                        stage,
                                        common_data,
                                        stick_x,
                                    );
                                    input_timers[player_index].y_tap = EXPIRED_INPUT_TIMER;
                                } else {
                                    enter_iasa_state(
                                        player,
                                        action_state,
                                        input_facts,
                                        input_facts.jump_input,
                                        stick_x,
                                        stage,
                                        common_data,
                                    );
                                }
                            } else {
                                enter_guard_steady(player);
                            }
                        }
                    }
                }
                MotionState::KneeBend => {
                    advance_source_bound_motion_frame(player);
                    if player.motion_frame >= player.profile.jumpsquat_frames {
                        apply_jump_takeoff_velocity(player, pre_input_stick_x);
                        set_source_self_velocity_y(player, ground_jump_vertical_velocity(player));
                        player.grounded = false;
                        player.set_motion_state_alias(ground_jump_motion_state(
                            player,
                            pre_input_stick_x,
                            common_data,
                        ));
                        player.motion_frame = 0;
                        player.set_source_motion_anim_frame(0.0);
                        player.set_source_motion_anim_rate_milli(1_000);
                        player.fast_falling = false;
                        player.jumps_remaining = player.profile.reusable_air_jumps();
                        lock_ground_to_air_ecb_bottom(player);
                        let entered_airborne_action = apply_airborne_iasa_actions(
                            player,
                            input_facts,
                            stick_x,
                            stick_y,
                            common_data,
                        );
                        if entered_airborne_action {
                            skip_airborne_vertical_physics_this_tick =
                                apply_entered_airborne_action_physics(player, stick_x, common_data);
                        } else {
                            // ftCo_Jump_Phys_Inner returns before ordinary
                            // airborne gravity on the first Jump tick.
                            skip_airborne_vertical_physics_this_tick = true;
                        }
                    } else if let Some(action_state) =
                        knee_bend_action_state(input_facts, stick_y, common_data)
                    {
                        enter_action_state(player, action_state, stick_x, stage, common_data);
                    } else {
                        if input_facts.short_hop_released_for(player.jump_input) {
                            player.short_hop = true;
                        }
                        apply_ground_traction(player, stage, common_data);
                    }
                }
                MotionState::JumpAerialF | MotionState::JumpAerialB => {
                    advance_source_motion_frame(player);
                    if action_sample_frame_count_for_motion_state(player.motion_state)
                        .is_some_and(|frames| player.motion_frame >= frames)
                    {
                        enter_fall_aerial(player);
                    }
                    if apply_airborne_iasa_or_drift(
                        player,
                        input_facts,
                        stick_x,
                        stick_y,
                        common_data,
                    ) && apply_entered_airborne_action_physics(player, stick_x, common_data)
                    {
                        skip_airborne_vertical_physics_this_tick = true;
                    }
                }
                MotionState::JumpF | MotionState::JumpB => {
                    advance_source_motion_frame(player);
                    if action_sample_frame_count_for_motion_state(player.motion_state)
                        .is_some_and(|frames| player.motion_frame >= frames)
                    {
                        enter_fall(player);
                    }
                    if apply_airborne_iasa_or_drift(
                        player,
                        input_facts,
                        stick_x,
                        stick_y,
                        common_data,
                    ) && apply_entered_airborne_action_physics(player, stick_x, common_data)
                    {
                        skip_airborne_vertical_physics_this_tick = true;
                    }
                }
                MotionState::Fall
                | MotionState::FallF
                | MotionState::FallB
                | MotionState::FallAerial
                | MotionState::FallAerialF
                | MotionState::FallAerialB
                | MotionState::DamageFall => {
                    if !fall_entered_from_rebirth_wait_this_tick {
                        advance_source_bound_motion_frame(player);
                    }
                    update_source_fall_anim_inner(player, common_data);
                    if apply_airborne_iasa_or_drift(
                        player,
                        input_facts,
                        stick_x,
                        stick_y,
                        common_data,
                    ) && apply_entered_airborne_action_physics(player, stick_x, common_data)
                    {
                        skip_airborne_vertical_physics_this_tick = true;
                    }
                }
                MotionState::Pass => {
                    advance_source_bound_motion_frame(player);
                    if action_sample_frame_count_for_motion_state(player.motion_state)
                        .is_some_and(|frames| player.motion_frame >= frames)
                    {
                        enter_fall(player);
                    }
                    if apply_airborne_iasa_or_drift(
                        player,
                        input_facts,
                        stick_x,
                        stick_y,
                        common_data,
                    ) && apply_entered_airborne_action_physics(player, stick_x, common_data)
                    {
                        skip_airborne_vertical_physics_this_tick = true;
                    }
                }
                MotionState::CliffCatch => {
                    if player.motion_frame == 0 {
                        let script_events =
                            source_current_anim_script_events(player, &mut source_pose_metadata);
                        apply_source_script_events(player, script_events, common_data);
                    }
                    advance_source_bound_motion_frame(player);
                    if action_sample_frame_count_for_motion_state(player.motion_state)
                        .is_some_and(|frames| player.motion_frame >= frames.saturating_sub(1))
                    {
                        enter_source_cliff_wait(player, common_data);
                    }
                    if !apply_source_cliff_physics(player, stage, common_data) {
                        enter_fall(player);
                    } else {
                        source_fighter_proc_map_begin(player);
                    }
                    skip_position_update_this_tick = true;
                }
                MotionState::CliffWait => {
                    advance_source_bound_motion_frame(player);
                    let remained_on_cliff = if apply_source_cliff_wait_iasa(
                        player,
                        input_facts,
                        input_snapshot,
                        stick_x,
                        stick_y,
                        common_data,
                    ) {
                        if matches!(player.motion_state, MotionState::CliffWait) {
                            true
                        } else if source_cliff_option_anchors_on_entry(player.motion_state) {
                            if !apply_source_cliff_physics(player, stage, common_data) {
                                enter_fall(player);
                                false
                            } else {
                                source_fighter_proc_map_begin(player);
                                !player.grounded
                            }
                        } else {
                            if player.motion_state == MotionState::Fall {
                                apply_air_drift(player, stick_x, common_data);
                            }
                            false
                        }
                    } else if !apply_source_cliff_physics(player, stage, common_data) {
                        enter_fall(player);
                        false
                    } else {
                        source_fighter_proc_map_begin(player);
                        tick_source_cliff_wait_timer(player);
                        true
                    };
                    skip_position_update_this_tick = remained_on_cliff;
                }
                MotionState::CliffClimbSlow
                | MotionState::CliffClimbQuick
                | MotionState::CliffAttackSlow
                | MotionState::CliffAttackQuick
                | MotionState::CliffEscapeSlow
                | MotionState::CliffEscapeQuick => {
                    advance_source_bound_motion_frame(player);
                    if source_action_has_no_frames_remaining(player) {
                        if player.grounded {
                            source_ft_8008a2bc_entry_grounded_handoff(player);
                            apply_wait_state_inputs(
                                player,
                                input_facts,
                                stick_x,
                                stage,
                                common_data,
                                &mut input_timers[player_index].x_tap,
                            );
                        } else {
                            enter_fall(player);
                        }
                        player.source_cliff_ledge_id = None;
                        player.source_cliff_stick_gate = false;
                        player.source_cliff_wait_timer = 0;
                    } else if !apply_source_cliff_physics(player, stage, common_data) {
                        enter_fall(player);
                    } else {
                        source_fighter_proc_map_begin(player);
                        skip_position_update_this_tick = !player.grounded;
                    }
                }
                MotionState::CliffJumpSlow1 | MotionState::CliffJumpQuick1 => {
                    advance_source_bound_motion_frame(player);
                    if source_action_has_no_frames_remaining(player) {
                        enter_source_cliff_jump_2(player);
                        skip_airborne_vertical_physics_this_tick = true;
                    } else if !apply_source_cliff_physics(player, stage, common_data) {
                        enter_fall(player);
                    } else {
                        source_ft_800821dc_cliff_air_collision(stage, player, common_data);
                        skip_position_update_this_tick = true;
                    }
                }
                MotionState::CliffJumpSlow2 | MotionState::CliffJumpQuick2 => {
                    advance_source_bound_motion_frame(player);
                    if source_action_has_no_frames_remaining(player) {
                        enter_fall(player);
                    }
                    apply_air_drift(player, stick_x, common_data);
                }
                MotionState::ShieldBreakFly => {
                    advance_source_bound_motion_frame(player);
                    if source_shield_break_action_done(player) {
                        enter_source_shield_break_fall(player);
                        skip_airborne_vertical_physics_this_tick = true;
                    }
                }
                MotionState::ShieldBreakFall => {
                    advance_source_bound_motion_frame(player);
                    if source_shield_break_action_done(player) {
                        player.motion_frame = player.source_action_total_frames;
                    }
                }
                MotionState::ShieldBreakDownU | MotionState::ShieldBreakDownD => {
                    advance_source_bound_motion_frame(player);
                    if source_shield_break_action_done(player) {
                        enter_source_shield_break_stand(player);
                    } else {
                        apply_ground_traction(player, stage, common_data);
                        player.velocity.y = 0;
                    }
                }
                MotionState::ShieldBreakStandU | MotionState::ShieldBreakStandD => {
                    advance_source_bound_motion_frame(player);
                    if source_shield_break_action_done(player) {
                        enter_source_furafura(player, common_data);
                    } else {
                        apply_ground_traction(player, stage, common_data);
                        player.velocity.y = 0;
                    }
                }
                MotionState::Furafura => {
                    player.shield_health = common_data.shield_break_reset_health;
                    player.source_grab_timer -= common_data.shield_break_furafura_timer_decrement;
                    source_ftcommon_grab_mash_with_decrement(
                        player,
                        input_snapshot,
                        common_data,
                        common_data.shield_break_furafura_mash_decrement,
                    );
                    if player.source_grab_timer <= 0.0 {
                        enter_source_common_action_end(player);
                    } else {
                        advance_source_bound_motion_frame(player);
                        apply_ground_traction(player, stage, common_data);
                        player.velocity.y = 0;
                    }
                }
                MotionState::EscapeAir => {
                    advance_source_bound_motion_frame(player);
                    if player.motion_frame.saturating_add(1)
                        >= player.profile.action_frames.escape_air_skip_decay_frame
                    {
                        player.motion_cmd_var0 = 1;
                    }
                    player.escape_air_iasa_timer = player.escape_air_iasa_timer.saturating_sub(1);
                    if action_sample_frame_count_for_motion_state(player.motion_state)
                        .is_some_and(|frames| player.motion_frame.saturating_add(1) >= frames)
                    {
                        enter_fall_special_after_position_update = true;
                    }
                }
                MotionState::FallSpecial
                | MotionState::FallSpecialF
                | MotionState::FallSpecialB => {
                    if input_facts.normal_jump_pressed && player.jumps_remaining > 0 {
                        enter_air_jump(player, stick_x, common_data);
                    } else {
                        advance_source_bound_motion_frame(player);
                        apply_air_drift(player, stick_x, common_data);
                    }
                }
                MotionState::LandingFallSpecial => {
                    advance_source_motion_frame(player);
                    let landing_lag_ticks = landing_fall_special_lag_ticks(player, common_data);
                    if player.motion_frame >= landing_lag_ticks {
                        player.set_motion_state_alias(MotionState::Wait);
                        player.motion_frame = 0;
                        player.set_source_motion_anim_frame(0.0);
                        player.set_source_motion_anim_rate_milli(1_000);
                        apply_wait_state_inputs(
                            player,
                            input_facts,
                            stick_x,
                            stage,
                            common_data,
                            &mut input_timers[player_index].x_tap,
                        );
                        if matches!(player.motion_state, MotionState::GuardOn) {
                            apply_ground_traction(player, stage, common_data);
                        }
                    } else {
                        apply_ground_traction(player, stage, common_data);
                    }
                    player.velocity.y = 0;
                }
                MotionState::Landing => {
                    landing_anim_tick(player, common_data);
                    if player.motion_state == MotionState::Landing
                        && player.motion_frame >= player.profile.normal_landing_lag_ticks
                        && apply_landing_iasa(
                            player,
                            input_facts,
                            stick_x,
                            stage,
                            common_data,
                            trigger_timer,
                        )
                    {
                        if player.motion_state == MotionState::Dash {
                            input_timers[player_index].x_tap = EXPIRED_INPUT_TIMER;
                        }
                    }
                    if player.motion_state == MotionState::Landing {
                        apply_ground_traction(player, stage, common_data);
                    } else if matches!(
                        player.motion_state,
                        MotionState::Turn | MotionState::SquatWait
                    ) {
                        apply_ground_traction(player, stage, common_data);
                    }
                    player.velocity.y = 0;
                }
                MotionState::LandingAirN
                | MotionState::LandingAirF
                | MotionState::LandingAirB
                | MotionState::LandingAirHi
                | MotionState::LandingAirLw => {
                    let landing_air_completed = landing_anim_tick(player, common_data);
                    if landing_air_completed {
                        player.set_source_motion_anim_rate_milli(1_000);
                        apply_wait_state_inputs(
                            player,
                            input_facts,
                            stick_x,
                            stage,
                            common_data,
                            &mut input_timers[player_index].x_tap,
                        );
                        if matches!(player.motion_state, MotionState::GuardOn) {
                            apply_ground_traction(player, stage, common_data);
                        }
                    } else {
                        apply_ground_traction(player, stage, common_data);
                    }
                    player.velocity.y = 0;
                }
                MotionState::DeadDown
                | MotionState::DeadLeft
                | MotionState::DeadRight
                | MotionState::DeadUp
                | MotionState::DeadUpStar
                | MotionState::DeadUpStarIce
                | MotionState::DeadUpFall
                | MotionState::DeadUpFallHitCamera
                | MotionState::DeadUpFallHitCameraFlat
                | MotionState::DeadUpFallIce
                | MotionState::DeadUpFallHitCameraIce
                | MotionState::Sleep
                | MotionState::Rebirth
                | MotionState::RebirthWait => {}
            }
        }

        if motion_state_before_tick != MotionState::SpecialHi
            && player.motion_state == MotionState::SpecialHi
        {
            apply_falcon_special_hi_physics(player, stick_x, common_data);
        }

        if input.attack() {
            player.attack_frame = ATTACK_ACTIVE_TICKS;
        } else {
            player.attack_frame = player.attack_frame.saturating_sub(1);
        }

        if player.motion_state != MotionState::Run {
            player.run_no_interrupt_frames = 0;
        }
        if !matches!(
            player.motion_state,
            MotionState::Dash
                | MotionState::RunBrake
                | MotionState::TurnRun
                | MotionState::WalkSlow
                | MotionState::WalkMiddle
                | MotionState::WalkFast
                | MotionState::Run
                | MotionState::EscapeAir
                | MotionState::AttackAirN
                | MotionState::AttackAirF
                | MotionState::AttackAirB
                | MotionState::AttackAirHi
                | MotionState::AttackAirLw
        ) && source_special_action_binding_for_motion_state(player.motion_state).is_none()
        {
            player.motion_cmd_var0 = 0;
            player.motion_cmd_var1 = 0;
            player.run_brake_x0 = false;
            player.run_brake_frames_remaining = 0;
            player.turn_run_x14 = false;
            player.turn_run_resume_advances = false;
            if !motion_preserves_entry_frame_speed_mul(player.motion_state) {
                player.set_source_motion_anim_rate_milli(1_000);
            }
        }
        if !matches!(
            player.motion_state,
            MotionState::WalkSlow | MotionState::WalkMiddle | MotionState::WalkFast
        ) {
            player.walk_anim_velocity_x = 0.0;
            player.walk_accel_mul_milli = 1_000;
        }
        if player.motion_state != MotionState::Dash {
            player.dash_entry_velocity_delta = 0.0;
            player.dash_started_from_tap = false;
        }
        if skip_position_update_this_tick {
            clear_ground_accels(player);
            continue;
        }

        let floor_surface_before_ground_move =
            source_floor_surface_before_ground_move(stage, player);

        add_source_position_x(player, player.player_nudge_x);
        add_source_position_z(player, player.player_nudge_z);

        if player.grounded {
            apply_common_source_knockback_physics(player, stage, common_data);
            apply_source_attacker_shield_knockback_physics(player, stage, common_data);
            add_source_position_x(player, grounded_position_delta_x(player));
            add_source_position_x(player, player.source_attacker_shield_velocity_x);
            add_source_position_x(player, player.source_knockback_velocity_x);
            if player.motion_state == MotionState::SpecialHi {
                add_source_position_y(player, player.source_self_velocity_y);
            }
            add_source_position_y(player, player.source_knockback_velocity_y);
            commit_ground_velocity(player, stage);
        } else if player.motion_state == MotionState::EscapeAir {
            if player.motion_cmd_var0 == 0 {
                apply_escape_air_decay(player, common_data);
            } else {
                apply_air_drift(player, stick_x, common_data);
            }
            apply_common_source_knockback_physics(player, stage, common_data);
            add_source_position_x(
                player,
                player.source_self_velocity_x + player.source_knockback_velocity_x,
            );
            clear_ground_accels(player);
        } else {
            seed_source_self_velocity_from_projected_velocity(player);
            apply_common_source_knockback_physics(player, stage, common_data);
            add_source_position_x(
                player,
                player.source_self_velocity_x + player.source_knockback_velocity_x,
            );
            clear_ground_accels(player);
        }

        if player.grounded
            && !resolve_ground_support_after_move(
                stage,
                player,
                ground_support_motion_state(motion_state_before_tick, player.motion_state),
                stick_x,
                floor_surface_before_ground_move,
                common_data,
                previous_source_position,
            )
        {
            if player.motion_state == MotionState::SpecialHi {
                source_ft_common_8007d5d4_ground_to_air(player);
            } else {
                enter_fall(player);
            }
            continue;
        }

        if !player.grounded {
            if player.motion_state == MotionState::EscapeAir && player.motion_cmd_var0 == 0 {
                add_source_position_y(
                    player,
                    player.source_self_velocity_y + player.source_knockback_velocity_y,
                );
                if skip_airborne_collision_this_tick {
                    continue;
                }
                let source_air_touched_floor = apply_source_air_map_wall_collision(
                    stage,
                    player,
                    previous_position,
                    previous_source_position,
                    player.motion_state,
                    stick_y,
                    common_data,
                );

                if source_air_touched_floor {
                    apply_source_air_floor_collision_callback(
                        player,
                        player.motion_state,
                        trigger_timer,
                        common_data,
                    );
                    install_source_floor_from_grounded_position(stage, player);
                    continue;
                }

                if try_source_cliff_catch(
                    player,
                    stage,
                    previous_position,
                    common_data,
                    input_facts,
                    player_index,
                    &source_ledge_owners,
                ) {
                    source_ledge_owners[player_index] = player.source_cliff_ledge_id;
                    continue;
                }
            } else {
                if !skip_airborne_vertical_physics_this_tick {
                    let fast_fall_tap = stick_y <= -common_data.fast_fall_y
                        && input_timers[player_index].y_tap < common_data.fast_fall_window;
                    let starts_fast_fall =
                        !player.fast_falling && player.velocity.y < 0 && fast_fall_tap;
                    if starts_fast_fall {
                        player.fast_falling = true;
                        input_timers[player_index].y_tap = EXPIRED_INPUT_TIMER;
                    }
                    if player.fast_falling {
                        set_source_self_velocity_y(player, -player.profile.fast_fall_velocity);
                    } else {
                        set_source_self_velocity_y(
                            player,
                            (player.source_self_velocity_y - player.profile.gravity)
                                .max(-player.profile.terminal_velocity),
                        );
                    }
                }
                add_source_position_y(
                    player,
                    player.source_self_velocity_y + player.source_knockback_velocity_y,
                );
                let source_air_touched_floor = apply_source_air_map_wall_collision(
                    stage,
                    player,
                    previous_position,
                    previous_source_position,
                    player.motion_state,
                    stick_y,
                    common_data,
                );

                if source_air_touched_floor {
                    apply_source_air_floor_collision_callback(
                        player,
                        player.motion_state,
                        trigger_timer,
                        common_data,
                    );
                    install_source_floor_from_grounded_position(stage, player);
                    continue;
                }

                if try_source_cliff_catch(
                    player,
                    stage,
                    previous_position,
                    common_data,
                    input_facts,
                    player_index,
                    &source_ledge_owners,
                ) {
                    source_ledge_owners[player_index] = player.source_cliff_ledge_id;
                    continue;
                }

                if stage.melee_stage_profile().is_none() {
                    let floor_skip_surface = (player.motion_state == MotionState::Pass)
                        .then_some(player.floor_skip_surface)
                        .flatten();
                    let drop_through_soft_platforms =
                        airborne_platform_landing_callback_rejects_soft_platform(
                            player.motion_state,
                            stick_y,
                            common_data,
                        );
                    if let Some(contact) = airborne_landing_contact(
                        stage,
                        player,
                        previous_ecb_bottom,
                        floor_skip_surface,
                        drop_through_soft_platforms,
                        common_data,
                    ) {
                        let landing_state = player.motion_state;
                        let impact_velocity_y = player.source_self_velocity_y;
                        snap_player_to_floor_contact(player, contact.y);
                        install_source_floor_contact(stage, player, contact);
                        if matches!(
                            landing_state,
                            MotionState::EscapeAir
                                | MotionState::FallSpecial
                                | MotionState::FallSpecialF
                                | MotionState::FallSpecialB
                        ) {
                            enter_landing_fall_special(
                                player,
                                common_data.escapeair_landing_lag_ticks,
                            );
                        } else {
                            enter_landing_from_airborne(
                                player,
                                landing_state,
                                impact_velocity_y,
                                trigger_timer,
                                common_data,
                            );
                        }
                    }
                }
            }
        }

        if enter_fall_special_after_position_update
            && player.motion_state == MotionState::EscapeAir
            && !player.grounded
        {
            enter_fall_special(player);
        }
    }

    apply_pending_source_throw_transitions(
        world.players_mut(),
        pending_source_throw_transitions,
        &mut source_action_total_frames,
        &mut source_pose_metadata,
    );
    apply_pending_source_capture_wait_transitions(
        world.players_mut(),
        pending_source_capture_wait_transitions,
        common_data,
        &mut source_action_total_frames,
    );
    apply_pending_source_catch_cut_transitions(
        world.players_mut(),
        pending_source_catch_cut_transitions,
        common_data,
        &mut source_action_total_frames,
    );
    apply_pending_source_throw_anim_freezes(world.players_mut(), pending_source_throw_anim_freezes);
    apply_pending_source_throw_releases(
        world,
        pending_source_throw_releases,
        inputs,
        &mut input_timers,
        &mut source_action_total_frames,
    );

    if world.match_flow_enabled() {
        for player_index in 0..PLAYER_COUNT {
            world.resolve_stage_blast_zone_ko_for_player(player_index);
        }
        world.sync_match_phase_from_players();
    }

    world.update_source_attack_ids_from_action_state_changes(previous_action_state_ids);
    world.set_input_timers(input_timers);
    world.set_last_input_facts(last_input_facts);
    world.set_previous_inputs(*inputs);
    world.set_frame(frame.next());
}

fn apply_wait_state_inputs(
    player: &mut PlayerState,
    input_facts: MeleeInputFacts,
    stick_x: i32,
    stage: StageProfile,
    common_data: MeleeCommonData,
    x_tap_timer: &mut u8,
) {
    if let Some(action_state) = wait_action_state(input_facts) {
        enter_action_state(player, action_state, stick_x, stage, common_data);
    } else if player.grounded && input_facts.shield_held {
        enter_guard(player, input_facts, common_data);
        source_update_guard_shield_visual(player, input_facts, common_data);
        apply_ground_traction(player, stage, common_data);
    } else if player.grounded && input_facts.normal_jump_pressed {
        enter_knee_bend_from_ground(player, input_facts.normal_jump_input, stage, common_data);
    } else if !player.grounded {
        enter_fall(player);
    } else {
        let forward_dash = input_facts.forward_dash_direction(player.facing);
        let smash_turn = input_facts.smash_turn_direction(player.facing);
        let standing_turn = input_facts.standing_turn_direction(player.facing);
        if forward_dash != 0 {
            enter_dash(player, forward_dash, true);
            *x_tap_timer = EXPIRED_INPUT_TIMER;
        } else if smash_turn != 0 {
            enter_smash_turn(player, smash_turn);
            apply_ground_traction(player, stage, common_data);
        } else if input_facts.crouch {
            enter_squat(player);
            apply_ground_traction(player, stage, common_data);
        } else if standing_turn != 0 {
            enter_standing_turn(player, standing_turn);
            apply_ground_traction(player, stage, common_data);
        } else if input_facts.walk_direction != 0 {
            enter_walk(
                player,
                walk_motion_state(player, common_data),
                stick_x,
                common_data,
            );
        } else {
            apply_ground_traction(player, stage, common_data);
        }
    }
}

fn begin_ground_velocity_tick(player: &mut PlayerState) {
    clear_ground_accels(player);
}

fn compute_player_nudge_velocities_after_source_anim_callbacks(
    players: &mut [PlayerState; 2],
    stage: StageProfile,
    common_data: MeleeCommonData,
) {
    let mut source_anim_view = *players;
    for player_index in 0..players.len() {
        apply_source_anim_callback_nudge_view(&mut source_anim_view[player_index]);
        let (nudge_x, nudge_z) =
            player_nudge_velocity_for_index(player_index, source_anim_view, stage, common_data);
        players[player_index].player_nudge_x = nudge_x;
        players[player_index].player_nudge_z = nudge_z;
    }
}

fn apply_source_anim_callback_nudge_view(player: &mut PlayerState) {
    if player.player_state != PLAYER_STATE_IN_GAME || player.hitlag_frames > 0 {
        return;
    }

    if player.motion_state == MotionState::KneeBend
        && player.motion_frame.saturating_add(1) >= player.profile.jumpsquat_frames
    {
        player.grounded = false;
    }
}

fn player_nudge_velocity_for_index(
    player_index: usize,
    players: [PlayerState; 2],
    stage: StageProfile,
    common_data: MeleeCommonData,
) -> (f32, f32) {
    let player = players[player_index];
    if player.hitlag_frames > 0 || !player.grounded {
        return (0.0, 0.0);
    }

    let mut nudge_x = 0.0_f32;
    let mut nudge_z = 0.0_f32;
    let mut reached_self_or_teammate = false;
    let player_position_x = milli_to_source_units(player.position.x);
    let source_collision = stage
        .melee_stage_profile()
        .map(|melee_stage| melee_stage.collision);
    let player_floor_line = player_nudge_floor_line(stage, source_collision, player);
    let player_floor_surface = player.source_coll_floor_surface_index.or_else(|| {
        floor_surface_index_for_bottom(stage, player.position)
            .map(|(surface_index, _)| surface_index)
    });

    for (other_index, other) in players.iter().copied().enumerate() {
        if other_index == player_index {
            reached_self_or_teammate = true;
            continue;
        }
        if !other.grounded {
            continue;
        }
        let other_floor_line = player_nudge_floor_line(stage, source_collision, other);
        let other_floor_surface = other.source_coll_floor_surface_index.or_else(|| {
            floor_surface_index_for_bottom(stage, other.position)
                .map(|(surface_index, _)| surface_index)
        });
        if !player_nudge_floor_matches(
            source_collision,
            player_floor_line,
            other_floor_line,
            player_floor_surface,
            other_floor_surface,
        ) {
            continue;
        }

        let other_position_x = milli_to_source_units(other.position.x);
        let player_projected_x = player.profile.player_nudge_body_center_x
            * f32::from(player.facing)
            + player_position_x;
        let other_projected_x =
            other.profile.player_nudge_body_center_x * f32::from(other.facing) + other_position_x;
        let delta_x = player_projected_x - other_projected_x;
        let combined_half_width = player.profile.player_nudge_body_half_width
            + other.profile.player_nudge_body_half_width;
        if delta_x.abs() >= combined_half_width {
            continue;
        }

        if delta_x < 0.0 {
            nudge_x -= common_data.player_nudge_x;
            nudge_z += player_nudge_depth_delta(player, other, common_data, -1.0);
        } else if delta_x > 0.0 {
            nudge_x += common_data.player_nudge_x;
            nudge_z += player_nudge_depth_delta(player, other, common_data, 1.0);
        } else if reached_self_or_teammate {
            nudge_x -= common_data.player_nudge_x;
            nudge_z += player_nudge_depth_delta(player, other, common_data, -1.0);
        } else {
            nudge_x += common_data.player_nudge_x;
            nudge_z += player_nudge_depth_delta(player, other, common_data, 1.0);
        }
    }

    if nudge_z == 0.0 && player.source_position_z != 0.0 {
        nudge_z = if player.source_position_z < 0.0 {
            common_data.player_nudge_z
        } else {
            -common_data.player_nudge_z
        };
    }
    if (nudge_z > 0.0
        && player.source_position_z < 0.0
        && player.source_position_z + nudge_z >= 0.0)
        || (nudge_z < 0.0
            && player.source_position_z > 0.0
            && player.source_position_z + nudge_z <= 0.0)
    {
        nudge_z = -player.source_position_z;
    }
    if player.source_position_z + nudge_z > common_data.player_nudge_z_clamp {
        nudge_z = common_data.player_nudge_z_clamp - player.source_position_z;
    } else if player.source_position_z + nudge_z < -common_data.player_nudge_z_clamp {
        nudge_z = -common_data.player_nudge_z_clamp - player.source_position_z;
    }
    (nudge_x, nudge_z)
}

fn player_nudge_depth_delta(
    player: PlayerState,
    other: PlayerState,
    common_data: MeleeCommonData,
    fallback_sign: f32,
) -> f32 {
    let delta_z = player.source_position_z - other.source_position_z;
    if delta_z < 0.0 {
        -common_data.player_nudge_z
    } else if delta_z > 0.0 {
        common_data.player_nudge_z
    } else {
        common_data.player_nudge_z * fallback_sign
    }
}

fn player_nudge_floor_line(
    stage: StageProfile,
    collision: Option<StageCollisionProfile>,
    player: PlayerState,
) -> Option<usize> {
    if let Some(line_id) = player.source_coll_floor_line_index.map(usize::from) {
        return Some(line_id);
    }
    collision
        .and_then(|collision| source_floor_line_for_grounded_position(stage, collision, &player))
}

fn player_nudge_floor_matches(
    collision: Option<StageCollisionProfile>,
    player_floor_line: Option<usize>,
    other_floor_line: Option<usize>,
    player_floor_surface: Option<u8>,
    other_floor_surface: Option<u8>,
) -> bool {
    if let (Some(player), Some(other)) = (player_floor_line, other_floor_line) {
        return player == other
            || collision.is_some_and(|collision| {
                source_line_get_next(collision, player) == Some(other)
                    || source_line_get_prev(collision, player) == Some(other)
            });
    }

    matches!(
        (player_floor_surface, other_floor_surface),
        (Some(player), Some(other)) if player == other
    )
}

fn advance_source_lr_digital_press_timers(player: &mut PlayerState, input_facts: MeleeInputFacts) {
    if input_facts.source_pressed.lr() {
        player.source_previous_lr_digital_press_timer = player.source_lr_digital_press_timer;
        player.source_lr_digital_press_timer = 0;
    } else if player.source_lr_digital_press_timer < 0xff {
        player.source_lr_digital_press_timer =
            player.source_lr_digital_press_timer.saturating_add(1);
    }

    if input_facts.source_pressed.lr() {
        player.source_lcancel_timer = 0;
    } else if !(player.hitlag_frames > 0 && player.source_lcancel_timer == 0)
        && player.source_lcancel_timer < 0xff
    {
        player.source_lcancel_timer = player.source_lcancel_timer.saturating_add(1);
    }
}

fn is_source_damage_player(player: &PlayerState) -> bool {
    player.motion_state_alias.is_none()
        && player
            .melee_action_state_id
            .is_some_and(is_source_damage_action_state_id)
}

fn is_source_down_bound_player(player: &PlayerState) -> bool {
    player.motion_state_alias.is_none()
        && player.melee_action_state_id.is_some_and(|action_state_id| {
            matches!(
                action_state_id,
                SOURCE_DOWN_BOUND_U_ACTION_STATE_ID | SOURCE_DOWN_BOUND_D_ACTION_STATE_ID
            )
        })
}

fn is_source_down_wait_player(player: &PlayerState) -> bool {
    player.motion_state_alias.is_none()
        && player.melee_action_state_id.is_some_and(|action_state_id| {
            matches!(
                action_state_id,
                SOURCE_DOWN_WAIT_U_ACTION_STATE_ID | SOURCE_DOWN_WAIT_D_ACTION_STATE_ID
            )
        })
}

fn is_source_down_stand_player(player: &PlayerState) -> bool {
    player.motion_state_alias.is_none()
        && player.melee_action_state_id.is_some_and(|action_state_id| {
            matches!(
                action_state_id,
                SOURCE_DOWN_STAND_U_ACTION_STATE_ID | SOURCE_DOWN_STAND_D_ACTION_STATE_ID
            )
        })
}

fn is_source_down_attack_player(player: &PlayerState) -> bool {
    player.motion_state_alias.is_none()
        && player.melee_action_state_id.is_some_and(|action_state_id| {
            matches!(
                action_state_id,
                SOURCE_DOWN_ATTACK_U_ACTION_STATE_ID | SOURCE_DOWN_ATTACK_D_ACTION_STATE_ID
            )
        })
}

fn is_source_down_roll_player(player: &PlayerState) -> bool {
    player.motion_state_alias.is_none()
        && player.melee_action_state_id.is_some_and(|action_state_id| {
            matches!(
                action_state_id,
                SOURCE_DOWN_FORWARD_U_ACTION_STATE_ID
                    | SOURCE_DOWN_BACK_U_ACTION_STATE_ID
                    | SOURCE_DOWN_SPOT_U_ACTION_STATE_ID
                    | SOURCE_DOWN_FORWARD_D_ACTION_STATE_ID
                    | SOURCE_DOWN_BACK_D_ACTION_STATE_ID
                    | SOURCE_DOWN_SPOT_D_ACTION_STATE_ID
            )
        })
}

fn is_source_passive_player(player: &PlayerState) -> bool {
    player.motion_state_alias.is_none()
        && player.melee_action_state_id.is_some_and(|action_state_id| {
            matches!(
                action_state_id,
                SOURCE_PASSIVE_ACTION_STATE_ID
                    | SOURCE_PASSIVE_STAND_F_ACTION_STATE_ID
                    | SOURCE_PASSIVE_STAND_B_ACTION_STATE_ID
            )
        })
}

fn is_source_grab_capture_player(player: &PlayerState) -> bool {
    player.motion_state_alias.is_none()
        && player.melee_action_state_id.is_some_and(|action_state_id| {
            matches!(
                action_state_id,
                SOURCE_CATCH_PULL_ACTION_STATE_ID
                    | SOURCE_CATCH_DASH_PULL_ACTION_STATE_ID
                    | SOURCE_CATCH_WAIT_ACTION_STATE_ID
                    | SOURCE_CATCH_ATTACK_ACTION_STATE_ID
                    | SOURCE_CATCH_CUT_ACTION_STATE_ID
                    | SOURCE_THROW_F_ACTION_STATE_ID
                    | SOURCE_THROW_B_ACTION_STATE_ID
                    | SOURCE_THROW_HI_ACTION_STATE_ID
                    | SOURCE_THROW_LW_ACTION_STATE_ID
                    | SOURCE_THROWN_F_ACTION_STATE_ID
                    | SOURCE_THROWN_B_ACTION_STATE_ID
                    | SOURCE_THROWN_HI_ACTION_STATE_ID
                    | SOURCE_THROWN_LW_ACTION_STATE_ID
                    | SOURCE_CAPTURE_PULLED_HI_ACTION_STATE_ID
                    | SOURCE_CAPTURE_WAIT_HI_ACTION_STATE_ID
                    | SOURCE_CAPTURE_JUMP_ACTION_STATE_ID
                    | SOURCE_CAPTURE_CUT_ACTION_STATE_ID
                    | SOURCE_CAPTURE_PULLED_LW_ACTION_STATE_ID
                    | SOURCE_CAPTURE_WAIT_LW_ACTION_STATE_ID
            )
        })
}

fn advance_source_grab_capture_state(
    player: &mut PlayerState,
    player_index: usize,
    input_facts: MeleeInputFacts,
    input_snapshot: MeleeInputSnapshot,
    stage: StageProfile,
    common_data: MeleeCommonData,
    source_pose_metadata_snapshots: &mut [SourcePoseMetadataSnapshot; PLAYER_COUNT],
    source_pose_metadata: &mut impl FnMut(&PlayerState) -> Option<SourceActionPoseMetadata>,
    source_action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
    source_capture_released_this_tick: &mut [bool; PLAYER_COUNT],
    source_capture_wait_anim_already_advanced: bool,
    player_profiles: [FighterProfile; PLAYER_COUNT],
) -> (
    Option<PendingSourceThrowTransition>,
    Option<PendingSourceCaptureWaitTransition>,
    Option<PendingSourceCatchCutTransition>,
    Option<PendingSourceThrowRelease>,
    Option<PendingSourceThrowAnimFreeze>,
) {
    let Some(mut action_state_id) = player.melee_action_state_id else {
        return (None, None, None, None, None);
    };
    if player.motion_frame == 0 {
        apply_source_script_events(
            player,
            source_pose_metadata_snapshots[player_index].script_events,
            common_data,
        );
    }
    advance_source_motion_frame(player);
    if source_thrown_state_uses_ftco_800de508(action_state_id) {
        apply_source_thrown_anim_timer(player);
    }
    source_pose_metadata_snapshots[player_index] =
        source_pose_metadata_snapshot_for_player(player, source_pose_metadata);
    apply_source_script_events(
        player,
        source_pose_metadata_snapshots[player_index].script_events,
        common_data,
    );

    if source_capture_state_uses_joint_delta(action_state_id) {
        apply_source_capture_joint_delta(player, player_index, source_pose_metadata_snapshots);
    }
    if source_thrown_state_uses_ftco_800de508(action_state_id) {
        apply_source_thrown_accessory_position(
            player,
            player_index,
            source_pose_metadata_snapshots,
        );
    }
    if source_capture_low_state_uses_ft_8008403c(action_state_id)
        && !player.source_x2226_b2
        && !apply_source_capture_low_collision(player, stage, common_data)
    {
        enter_source_capture_pulled_hi_from_low(player, source_action_total_frames);
        action_state_id = SOURCE_CAPTURE_PULLED_HI_ACTION_STATE_ID;
        source_pose_metadata_snapshots[player_index] =
            source_pose_metadata_snapshot_for_player(player, source_pose_metadata);
        apply_source_capture_joint_delta(player, player_index, source_pose_metadata_snapshots);
    }

    if source_throw_state_uses_ftco_800dd724(action_state_id) {
        let mut pending_release = None;
        let mut pending_anim_freeze = None;
        if player.motion_throw_flags & SOURCE_THROW_FLAGS_B4 != 0 {
            player.motion_throw_flags &= !SOURCE_THROW_FLAGS_B4;
            player.facing = -player.facing;
        }
        if player.motion_throw_flags & SOURCE_THROW_FLAGS_B3 != 0 {
            player.motion_throw_flags &= !SOURCE_THROW_FLAGS_B3;
            if let (Some(victim_index), Some(hitbox)) = (
                player.source_victim_index.map(usize::from),
                player.source_throw_hitboxes[0],
            ) {
                pending_release = Some(PendingSourceThrowRelease {
                    thrower_index: player_index,
                    victim_index,
                    hitbox,
                    source_release_transn2_position: source_throw_release_transn2_position(
                        source_pose_metadata_snapshots[player_index],
                    ),
                    source_release_last_pos: Some(source_throw_release_last_pos(
                        source_pose_metadata_snapshots[player_index],
                        player.source_coll_ecb,
                    )),
                });
            }
        }
        if player.motion_cmd_var0 != 0 && !player.source_throw_x4 {
            player.source_throw_x4 = true;
            player.motion_cmd_var0 = 0;
            player.set_source_motion_anim_rate_milli(0);
            if let Some(victim_index) = player.source_victim_index.map(usize::from) {
                pending_anim_freeze = Some(PendingSourceThrowAnimFreeze {
                    victim_index,
                    anim_timer: player.source_motion_anim_frame,
                });
            }
        }
        if pending_release.is_some() || pending_anim_freeze.is_some() {
            return (None, None, None, pending_release, pending_anim_freeze);
        }
        if source_action_animation_done(player) {
            enter_source_common_action_end(player);
            source_pose_metadata_snapshots[player_index] =
                source_pose_metadata_snapshot_for_player(player, source_pose_metadata);
            return (None, None, None, None, None);
        }
    }

    if matches!(
        action_state_id,
        SOURCE_CATCH_CUT_ACTION_STATE_ID
            | SOURCE_CAPTURE_CUT_ACTION_STATE_ID
            | SOURCE_CAPTURE_JUMP_ACTION_STATE_ID
    ) && source_action_animation_done(player)
    {
        enter_source_common_action_end(player);
        source_pose_metadata_snapshots[player_index] =
            source_pose_metadata_snapshot_for_player(player, source_pose_metadata);
        return (None, None, None, None, None);
    }

    if action_state_id == SOURCE_CATCH_ATTACK_ACTION_STATE_ID
        && source_action_animation_done(player)
    {
        enter_source_catch_wait_after_attack(player, source_action_total_frames);
        source_pose_metadata_snapshots[player_index] =
            source_pose_metadata_snapshot_for_player(player, source_pose_metadata);
        let released_victim_with_held_cstick_down = player
            .source_victim_index
            .map(usize::from)
            .is_some_and(|victim_index| source_capture_released_this_tick[victim_index])
            && source_cstick_down_throw_held(
                input_snapshot.prev_cstick.1,
                input_snapshot.cstick.1,
                common_data.throw_down_y,
            );
        if !released_victim_with_held_cstick_down {
            return (None, None, None, None, None);
        }
        action_state_id = SOURCE_CATCH_WAIT_ACTION_STATE_ID;
    }

    if matches!(
        action_state_id,
        SOURCE_CATCH_PULL_ACTION_STATE_ID | SOURCE_CATCH_DASH_PULL_ACTION_STATE_ID
    ) && source_catch_pull_should_enter_wait(player)
    {
        let Some(victim_index) = player.source_victim_index.map(usize::from) else {
            return (None, None, None, None, None);
        };
        enter_source_catch_wait_from_pull(player, source_action_total_frames);
        source_pose_metadata_snapshots[player_index] =
            source_pose_metadata_snapshot_for_player(player, source_pose_metadata);
        return (
            None,
            Some(PendingSourceCaptureWaitTransition {
                grabber_index: player_index,
                victim_index,
            }),
            None,
            None,
            None,
        );
    }

    if source_capture_wait_state(action_state_id) && !source_capture_wait_anim_already_advanced {
        if let Some(grabber_index) = advance_source_capture_wait_state(
            player,
            player_index,
            input_facts,
            input_snapshot,
            common_data,
            source_action_total_frames,
            source_capture_released_this_tick,
        ) {
            source_pose_metadata_snapshots[player_index] =
                source_pose_metadata_snapshot_for_player(player, source_pose_metadata);
            return (
                None,
                None,
                Some(PendingSourceCatchCutTransition { grabber_index }),
                None,
                None,
            );
        }
    }

    if action_state_id == SOURCE_CATCH_WAIT_ACTION_STATE_ID {
        if input_facts.source_pressed.a() {
            enter_source_catch_attack(player, source_action_total_frames);
            source_pose_metadata_snapshots[player_index] =
                source_pose_metadata_snapshot_for_player(player, source_pose_metadata);
            return (None, None, None, None, None);
        }
        let victim_released_this_tick = player
            .source_victim_index
            .map(usize::from)
            .is_some_and(|victim_index| source_capture_released_this_tick[victim_index]);
        let held_cstick_down_throw = source_cstick_down_throw_held(
            input_snapshot.prev_cstick.1,
            input_snapshot.cstick.1,
            common_data.throw_down_y,
        );
        if victim_released_this_tick && !held_cstick_down_throw {
            return (None, None, None, None, None);
        }
        let throw_action_state_id = if victim_released_this_tick {
            Some(SOURCE_THROW_LW_ACTION_STATE_ID)
        } else {
            source_catch_wait_throw_action_state(player, input_snapshot, common_data)
        };
        if let Some(throw_action_state_id) = throw_action_state_id {
            let Some(victim_index) = player.source_victim_index.map(usize::from) else {
                return (None, None, None, None, None);
            };
            let Some((victim_action_state_id, victim_source_action_key)) =
                source_victim_thrown_action_for_throw(throw_action_state_id)
            else {
                return (None, None, None, None, None);
            };
            let anim_speed = source_throw_animation_speed(
                player.profile,
                player_profiles[victim_index],
                throw_action_state_id,
                common_data,
            );
            enter_source_throw_action(
                player,
                throw_action_state_id,
                source_action_total_frames,
                common_data,
                anim_speed,
            );
            return (
                Some(PendingSourceThrowTransition {
                    thrower_index: player_index,
                    victim_index,
                    victim_action_state_id,
                    victim_source_action_key,
                    facing: player.facing,
                    anim_speed,
                }),
                None,
                None,
                None,
                None,
            );
        }
    }

    if matches!(
        action_state_id,
        SOURCE_CATCH_PULL_ACTION_STATE_ID
            | SOURCE_CATCH_DASH_PULL_ACTION_STATE_ID
            | SOURCE_CATCH_WAIT_ACTION_STATE_ID
            | SOURCE_CATCH_ATTACK_ACTION_STATE_ID
    ) && player.grounded
    {
        apply_catch_ground_physics(player, stage, common_data);
    }

    (None, None, None, None, None)
}

fn apply_source_script_events(
    player: &mut PlayerState,
    events: SourceActionScriptEvents,
    common_data: MeleeCommonData,
) {
    for event in events.iter() {
        match event {
            SourceActionScriptEvent::None => {}
            SourceActionScriptEvent::SetCmdVar { cmd_var, value } => {
                apply_source_set_cmd_var_event(player, cmd_var, value);
            }
            SourceActionScriptEvent::AllowInterrupt => {
                player.source_allow_interrupt = true;
            }
            SourceActionScriptEvent::SetAirborneState { state } => {
                apply_source_set_airborne_state_event(player, state);
            }
            SourceActionScriptEvent::SetCollisionState { state } => {
                player.source_collision_state = state;
            }
            SourceActionScriptEvent::SetJabCombo { disabled } => {
                player.source_jab_combo_enabled = !disabled;
            }
            SourceActionScriptEvent::SetJabRapid { state } => {
                player.source_jab_rapid_enabled = state;
            }
            SourceActionScriptEvent::SetThrowFlag { hit_idx, flag_bit } => {
                if let Some(bit) = source_throw_flag_bit(hit_idx, flag_bit) {
                    player.motion_throw_flags |= 1u8 << bit;
                }
            }
            SourceActionScriptEvent::SetThrowHitbox(hitbox) => {
                let installed =
                    source_installed_throw_hitbox_from_script(player, common_data, hitbox);
                if let Some(slot) = player
                    .source_throw_hitboxes
                    .get_mut(usize::from(hitbox.hitbox_idx))
                {
                    *slot = Some(installed);
                }
            }
        }
    }
}

fn source_anim_command_script_events(
    player: &PlayerState,
    source_pose_metadata: &mut impl FnMut(&PlayerState) -> Option<SourceActionPoseMetadata>,
) -> SourceActionScriptEvents {
    let mut command_player = *player;
    let command_frame = if player.source_motion_anim_frame.is_finite() {
        player.source_motion_anim_frame.floor() + 1.0
    } else {
        1.0
    };
    command_player.set_source_motion_anim_frame(command_frame);
    command_player.motion_frame = command_frame.clamp(0.0, f32::from(u8::MAX)) as u8;
    source_pose_metadata(&command_player)
        .map(|metadata| metadata.script_events)
        .unwrap_or_default()
}

fn source_current_anim_script_events(
    player: &PlayerState,
    source_pose_metadata: &mut impl FnMut(&PlayerState) -> Option<SourceActionPoseMetadata>,
) -> SourceActionScriptEvents {
    let mut command_player = *player;
    let command_frame = if player.source_motion_anim_frame.is_finite() {
        player.source_motion_anim_frame.floor().max(0.0)
    } else {
        0.0
    };
    command_player.set_source_motion_anim_frame(command_frame);
    command_player.motion_frame = command_frame.clamp(0.0, f32::from(u8::MAX)) as u8;
    source_pose_metadata(&command_player)
        .map(|metadata| metadata.script_events)
        .unwrap_or_default()
}

fn source_installed_throw_hitbox_from_script(
    player: &PlayerState,
    common_data: MeleeCommonData,
    hitbox: SourceThrowHitboxAttributes,
) -> SourceInstalledThrowHitbox {
    let scaled_damage = hitbox.damage as f32;
    let multiplier = player.source_stale_move_table.damage_multiplier(
        common_data.stale_move_damage_reductions,
        player.source_attack_id,
    );
    SourceInstalledThrowHitbox {
        hitbox,
        damage: scaled_damage * multiplier,
        unk_count: scaled_damage as u16,
    }
}

fn apply_source_set_cmd_var_event(player: &mut PlayerState, cmd_var: u8, value: u32) {
    match cmd_var {
        0 => player.motion_cmd_var0 = value,
        1 => player.motion_cmd_var1 = value,
        _ => {}
    }
}

fn apply_source_set_airborne_state_event(player: &mut PlayerState, state: u8) {
    match state {
        0 => {
            if !player.grounded {
                player.grounded = true;
                player.ground_velocity_x = player.source_self_velocity_x;
                player.jumps_remaining = player.profile.reusable_air_jumps();
                player.ecb_bottom_lock_timer = 0;
                player.source_coll_x130_locked = false;
                clear_ground_accels(player);
            }
        }
        1 => {
            source_ft_common_8007d5d4_ground_to_air(player);
        }
        2 => {
            player.grounded = false;
            player.ground_velocity_x = 0.0;
            player.jumps_remaining = 0;
            lock_ecb_bottom_for_frames(player, AIRBORNE_STATE_TWO_ECB_LOCK_FRAMES);
            clear_ground_accels(player);
        }
        _ => {}
    }
}

fn source_throw_flag_bit(hit_idx: u32, flag_bit: Option<u8>) -> Option<u8> {
    match flag_bit {
        Some(bit) if bit < 8 => Some(bit),
        Some(_) => None,
        None => match hit_idx {
            0 => Some(3),
            1 => Some(4),
            _ => None,
        },
    }
}

fn source_capture_state_uses_joint_delta(action_state_id: MeleeActionStateId) -> bool {
    matches!(
        action_state_id,
        SOURCE_CAPTURE_PULLED_HI_ACTION_STATE_ID
            | SOURCE_CAPTURE_WAIT_HI_ACTION_STATE_ID
            | SOURCE_CAPTURE_PULLED_LW_ACTION_STATE_ID
            | SOURCE_CAPTURE_WAIT_LW_ACTION_STATE_ID
    )
}

fn source_capture_wait_state(action_state_id: MeleeActionStateId) -> bool {
    matches!(
        action_state_id,
        SOURCE_CAPTURE_WAIT_HI_ACTION_STATE_ID | SOURCE_CAPTURE_WAIT_LW_ACTION_STATE_ID
    )
}

fn advance_source_capture_wait_state(
    player: &mut PlayerState,
    player_index: usize,
    input_facts: MeleeInputFacts,
    input_snapshot: MeleeInputSnapshot,
    common_data: MeleeCommonData,
    source_action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
    source_capture_released_this_tick: &mut [bool; PLAYER_COUNT],
) -> Option<usize> {
    player.source_capture_wait_timer += 1.0;
    player.source_grab_timer -= common_data.grab_timer_decrement;
    player.source_capture_wait_mashed =
        source_ftcommon_grab_mash(player, input_snapshot, common_data);
    if player.source_capture_wait_timer < common_data.capture_wait_jump_input_window
        && (input_facts.source_pressed.x() || input_facts.source_pressed.y())
    {
        player.source_capture_wait_jump_queued = true;
    }
    if player.source_grab_timer <= 0.0 {
        source_capture_released_this_tick[player_index] = true;
        let grabber_index = player.source_victim_index.map(usize::from)?;
        enter_source_capture_wait_release(
            player,
            input_snapshot,
            common_data,
            source_action_total_frames,
        );
        return Some(grabber_index);
    }
    if player.source_capture_wait_anim_timer != 0.0 {
        player.source_capture_wait_anim_timer -= 1.0;
        if player.source_capture_wait_anim_timer <= 0.0 && !player.source_capture_wait_mashed {
            player.set_source_motion_anim_rate_milli(1_000);
            player.source_capture_wait_anim_timer = 0.0;
        }
    }
    if player.source_capture_wait_anim_timer <= 0.0 && player.source_capture_wait_mashed {
        player.source_capture_wait_anim_timer = common_data.capture_wait_mash_anim_timer;
        player.set_source_motion_anim_rate_milli(
            (common_data.capture_wait_mash_anim_rate * 1000.0).round() as i32,
        );
    }
    None
}

fn source_ftcommon_grab_mash(
    player: &mut PlayerState,
    input_snapshot: MeleeInputSnapshot,
    common_data: MeleeCommonData,
) -> bool {
    source_ftcommon_grab_mash_with_decrement(
        player,
        input_snapshot,
        common_data,
        common_data.grab_mash_timer_decrement,
    )
}

fn source_ftcommon_grab_mash_with_decrement(
    player: &mut PlayerState,
    input_snapshot: MeleeInputSnapshot,
    common_data: MeleeCommonData,
    timer_decrement: f32,
) -> bool {
    let mut result = false;
    if input_snapshot.pressed.a()
        || input_snapshot.pressed.b()
        || input_snapshot.pressed.x()
        || input_snapshot.pressed.y()
        || input_snapshot.pressed.l()
        || input_snapshot.pressed.r()
    {
        player.source_grab_timer -= timer_decrement;
        result = true;
    }
    let previous_x = player.source_grab_mash_x;
    let previous_y = player.source_grab_mash_y;
    let threshold = common_data.grab_mash_stick_threshold.abs();
    if input_snapshot.lstick.0 < -threshold {
        player.source_grab_mash_x = -1;
    }
    if input_snapshot.lstick.0 > threshold {
        player.source_grab_mash_x = 1;
    }
    if input_snapshot.lstick.1 < -threshold {
        player.source_grab_mash_y = -1;
    }
    if input_snapshot.lstick.1 > threshold {
        player.source_grab_mash_y = 1;
    }
    if previous_x != player.source_grab_mash_x || previous_y != player.source_grab_mash_y {
        player.source_grab_timer -= timer_decrement;
        result = true;
    }
    result
}

fn enter_source_capture_wait_release(
    player: &mut PlayerState,
    input_snapshot: MeleeInputSnapshot,
    common_data: MeleeCommonData,
    source_action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
) {
    let jump_release =
        player.source_capture_wait_jump_queued || input_snapshot.lstick.1 >= common_data.tap_jump_y;
    if jump_release {
        enter_source_capture_jump(player, common_data, source_action_total_frames);
    } else {
        enter_source_capture_cut(player, common_data, source_action_total_frames);
    }
}

fn enter_source_catch_cut(
    player: &mut PlayerState,
    common_data: MeleeCommonData,
    source_action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
) {
    if player.grounded {
        player.ground_velocity_x =
            -f32::from(player.facing) * common_data.catch_cut_ground_velocity;
    } else {
        player.source_self_velocity_x =
            -f32::from(player.facing) * common_data.capture_jump_velocity_x;
        player.source_self_velocity_y = common_data.capture_jump_velocity_y;
        player.velocity.x = source_units_to_milli(player.source_self_velocity_x);
        player.velocity.y = source_units_to_milli(player.source_self_velocity_y);
    }
    source_clear_capture_links(player);
    enter_source_grab_related_action(
        player,
        SOURCE_CATCH_CUT_ACTION_STATE_ID,
        SOURCE_CATCH_CUT_ACTION_KEY,
        source_action_total_frames,
    );
}

fn enter_source_capture_cut(
    player: &mut PlayerState,
    common_data: MeleeCommonData,
    source_action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
) {
    source_clear_capture_links(player);
    if player.grounded {
        player.ground_velocity_x =
            -f32::from(player.facing) * common_data.catch_cut_ground_velocity;
    } else {
        player.source_self_velocity_x =
            -f32::from(player.facing) * common_data.catch_cut_ground_velocity;
        player.velocity.x = source_units_to_milli(player.source_self_velocity_x);
    }
    enter_source_grab_related_action(
        player,
        SOURCE_CAPTURE_CUT_ACTION_STATE_ID,
        SOURCE_CAPTURE_CUT_ACTION_KEY,
        source_action_total_frames,
    );
}

fn enter_source_capture_jump(
    player: &mut PlayerState,
    common_data: MeleeCommonData,
    source_action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
) {
    source_clear_capture_links(player);
    player.grounded = false;
    player.source_self_velocity_x = -f32::from(player.facing) * common_data.capture_jump_velocity_x;
    player.source_self_velocity_y = common_data.capture_jump_velocity_y;
    player.velocity.x = source_units_to_milli(player.source_self_velocity_x);
    player.velocity.y = source_units_to_milli(player.source_self_velocity_y);
    enter_source_grab_related_action(
        player,
        SOURCE_CAPTURE_JUMP_ACTION_STATE_ID,
        SOURCE_CAPTURE_JUMP_ACTION_KEY,
        source_action_total_frames,
    );
}

fn source_clear_capture_links(player: &mut PlayerState) {
    player.source_victim_index = None;
    player.source_x1a5c_index = None;
    player.source_x221b_b5 = false;
    player.source_x2226_b2 = false;
}

fn source_capture_low_state_uses_ft_8008403c(action_state_id: MeleeActionStateId) -> bool {
    matches!(
        action_state_id,
        SOURCE_CAPTURE_PULLED_LW_ACTION_STATE_ID | SOURCE_CAPTURE_WAIT_LW_ACTION_STATE_ID
    )
}

fn source_thrown_state_uses_ftco_800de508(action_state_id: MeleeActionStateId) -> bool {
    matches!(
        action_state_id,
        SOURCE_THROWN_F_ACTION_STATE_ID
            | SOURCE_THROWN_B_ACTION_STATE_ID
            | SOURCE_THROWN_HI_ACTION_STATE_ID
            | SOURCE_THROWN_LW_ACTION_STATE_ID
    )
}

fn source_throw_state_uses_ftco_800dd724(action_state_id: MeleeActionStateId) -> bool {
    matches!(
        action_state_id,
        SOURCE_THROW_F_ACTION_STATE_ID
            | SOURCE_THROW_B_ACTION_STATE_ID
            | SOURCE_THROW_HI_ACTION_STATE_ID
            | SOURCE_THROW_LW_ACTION_STATE_ID
    )
}

fn apply_source_capture_low_collision(
    player: &mut PlayerState,
    stage: StageProfile,
    common_data: MeleeCommonData,
) -> bool {
    source_fighter_proc_map_begin(player);
    let Some(melee_stage) = stage.melee_stage_profile() else {
        return false;
    };
    source_ft_80082708_allow_ground_to_air(stage, melee_stage.collision, player, common_data)
}

fn apply_source_capture_joint_delta(
    player: &mut PlayerState,
    player_index: usize,
    source_pose_metadata_snapshots: &[SourcePoseMetadataSnapshot; PLAYER_COUNT],
) -> Option<f32> {
    let grabber_index = player.source_victim_index.map(usize::from)?;
    if grabber_index >= PLAYER_COUNT || player_index >= PLAYER_COUNT {
        return None;
    }
    let victim_pose = source_pose_metadata_snapshots[player_index].capture_pose?;
    let grabber_snapshot = source_pose_metadata_snapshots[grabber_index];
    let grabber_pose = grabber_snapshot.capture_pose?;

    let grabber_anchor_x = grabber_snapshot.source_position.x
        + source_faced_pose_x(grabber_snapshot.model_facing, grabber_pose.capture_anchor.x);
    let grabber_anchor_y = grabber_snapshot.source_position.y + grabber_pose.capture_anchor.y;
    let victim_snapshot = source_pose_metadata_snapshots[player_index];
    let victim_xrotn_x = player.source_position.x
        + source_faced_pose_x(victim_snapshot.model_facing, victim_pose.xrotn.x);
    let victim_xrotn_y = player.source_position.y + victim_pose.xrotn.y;
    let delta_x = grabber_anchor_x - victim_xrotn_x;
    let delta_y = grabber_anchor_y - victim_xrotn_y;
    add_source_position_x(player, delta_x);
    add_source_position_y(player, delta_y);
    Some(delta_y)
}

fn enter_source_capture_pulled_hi_from_low(
    player: &mut PlayerState,
    source_action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
) {
    player.grounded = false;
    player.ground_velocity_x = 0.0;
    player.ground_accel_x = 0.0;
    player.ground_accel_x2 = 0.0;
    player.jumps_remaining = player.profile.reusable_air_jumps();
    player.ecb_bottom_lock_timer = 0;
    player.source_coll_x130_locked = false;
    player.ecb_bottom_offset_y = SOURCE_JOBJ_ECB_BOTTOM_OFFSET_Y;
    player.melee_action_state_id = Some(SOURCE_CAPTURE_PULLED_HI_ACTION_STATE_ID);
    player.source_action_key = Some(SOURCE_CAPTURE_PULLED_HI_ACTION_KEY);
    player.source_action_total_frames =
        source_action_total_frames(SOURCE_CAPTURE_PULLED_HI_ACTION_STATE_ID).unwrap_or(0);
}

fn apply_source_thrown_accessory_position(
    player: &mut PlayerState,
    player_index: usize,
    source_pose_metadata_snapshots: &[SourcePoseMetadataSnapshot; PLAYER_COUNT],
) {
    if !player.source_x2226_b2 {
        return;
    }
    let Some(thrower_index) = player.source_victim_index.map(usize::from) else {
        return;
    };
    if thrower_index >= PLAYER_COUNT || player_index >= PLAYER_COUNT {
        return;
    }
    let thrower_snapshot = source_pose_metadata_snapshots[thrower_index];
    let Some(thrower_pose) = thrower_snapshot.capture_pose else {
        return;
    };
    let facing = if player.facing < 0 { -1.0 } else { 1.0 };
    let scale_y = player.source_x34_scale_y;
    set_source_position_x_source(
        player,
        thrower_snapshot.source_position.x
            + source_faced_pose_x(thrower_snapshot.model_facing, thrower_pose.transn2.x)
            + facing * player.source_x1a70.z * scale_y,
    );
    set_source_position_y_source(
        player,
        thrower_snapshot.source_position.y
            + thrower_pose.transn2.y
            + player.source_x1a70.y * scale_y,
    );
}

fn source_throw_release_transn2_position(
    thrower_snapshot: SourcePoseMetadataSnapshot,
) -> Option<SourceVec2> {
    let thrower_pose = thrower_snapshot.capture_pose?;
    Some(SourceVec2 {
        x: thrower_snapshot.source_position.x
            + source_faced_pose_x(thrower_snapshot.model_facing, thrower_pose.transn2.x),
        y: thrower_snapshot.source_position.y + thrower_pose.transn2.y,
    })
}

fn source_faced_pose_x(facing: i8, x: f32) -> f32 {
    if facing < 0 {
        -x
    } else {
        x
    }
}

fn source_throw_release_last_pos(
    thrower_snapshot: SourcePoseMetadataSnapshot,
    thrower_ecb: SourceFighterEcb,
) -> SourceVec2 {
    SourceVec2 {
        x: thrower_snapshot.source_position.x,
        y: thrower_snapshot.source_position.y + 0.5 * (thrower_ecb.top.y + thrower_ecb.bottom.y),
    }
}

fn apply_source_thrown_anim_timer(player: &mut PlayerState) {
    if !player.source_thrown_unk_bool || player.source_thrown_anim_timer == 0.0 {
        return;
    }
    let anim_timer_milli = (player.source_thrown_anim_timer * 1000.0).round() as i32;
    if player.motion_anim_frame_milli == anim_timer_milli {
        player.set_source_motion_anim_rate_milli(0);
        player.source_thrown_anim_timer = 0.0;
    }
}

fn source_pose_metadata_snapshot_for_player(
    player: &PlayerState,
    source_pose_metadata: &mut impl FnMut(&PlayerState) -> Option<SourceActionPoseMetadata>,
) -> SourcePoseMetadataSnapshot {
    let metadata = source_pose_metadata(player);
    SourcePoseMetadataSnapshot {
        source_position: player.source_position,
        model_facing: player_model_facing(player),
        capture_pose: metadata.and_then(|metadata| metadata.capture_pose),
        script_events: metadata
            .map(|metadata| metadata.script_events)
            .unwrap_or_default(),
        primary_hitbox: metadata.and_then(|metadata| metadata.primary_hitbox),
    }
}

fn source_catch_pull_should_enter_wait(player: &mut PlayerState) -> bool {
    let frames_remaining = player.source_action_total_frames == 0
        || player.motion_frame < player.source_action_total_frames;
    if frames_remaining {
        if player.motion_throw_flags & SOURCE_THROW_FLAGS_B3 != 0 {
            player.motion_throw_flags &= !SOURCE_THROW_FLAGS_B3;
            return true;
        }
        return false;
    }
    true
}

fn enter_source_catch_wait_from_pull(
    player: &mut PlayerState,
    source_action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
) {
    player.ground_velocity_x = 0.0;
    player.ground_accel_x = 0.0;
    player.ground_accel_x2 = 0.0;
    player.source_self_velocity_x = 0.0;
    player.velocity.x = 0;
    enter_source_grab_related_action(
        player,
        SOURCE_CATCH_WAIT_ACTION_STATE_ID,
        SOURCE_CATCH_WAIT_ACTION_KEY,
        source_action_total_frames,
    );
}

fn enter_source_catch_wait_after_attack(
    player: &mut PlayerState,
    source_action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
) {
    enter_source_grab_related_action(
        player,
        SOURCE_CATCH_WAIT_ACTION_STATE_ID,
        SOURCE_CATCH_WAIT_ACTION_KEY,
        source_action_total_frames,
    );
}

fn enter_source_catch_attack(
    player: &mut PlayerState,
    source_action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
) {
    player.ground_velocity_x = 0.0;
    player.ground_accel_x = 0.0;
    player.ground_accel_x2 = 0.0;
    player.source_self_velocity_x = 0.0;
    player.velocity.x = 0;
    enter_source_grab_related_action(
        player,
        SOURCE_CATCH_ATTACK_ACTION_STATE_ID,
        SOURCE_CATCH_ATTACK_ACTION_KEY,
        source_action_total_frames,
    );
}

fn source_catch_wait_throw_action_state(
    player: &PlayerState,
    input_snapshot: MeleeInputSnapshot,
    common_data: MeleeCommonData,
) -> Option<MeleeActionStateId> {
    if source_axis_crossed_pos_or_neg(
        input_snapshot.prev_lstick.0,
        input_snapshot.lstick.0,
        common_data.tilt_x,
    ) {
        return Some(source_side_throw_action_state(
            input_snapshot.lstick.0,
            player.facing,
        ));
    }
    if source_axis_crossed_pos_or_neg(
        input_snapshot.prev_cstick.0,
        input_snapshot.cstick.0,
        common_data.tilt_x,
    ) {
        return Some(source_side_throw_action_state(
            input_snapshot.cstick.0,
            player.facing,
        ));
    }
    if source_axis_crossed_positive(
        input_snapshot.prev_lstick.1,
        input_snapshot.lstick.1,
        common_data.tilt_y,
    ) || source_axis_crossed_positive(
        input_snapshot.prev_cstick.1,
        input_snapshot.cstick.1,
        common_data.tilt_y,
    ) {
        return Some(SOURCE_THROW_HI_ACTION_STATE_ID);
    }
    if source_axis_crossed_negative(
        input_snapshot.prev_lstick.1,
        input_snapshot.lstick.1,
        common_data.throw_down_y,
    ) || source_cstick_down_throw_held(
        input_snapshot.prev_cstick.1,
        input_snapshot.cstick.1,
        common_data.throw_down_y,
    ) {
        return Some(SOURCE_THROW_LW_ACTION_STATE_ID);
    }
    None
}

fn source_axis_crossed_pos_or_neg(previous: i8, current: i8, threshold: i8) -> bool {
    let threshold = i16::from(threshold.abs());
    (i16::from(previous) < threshold && i16::from(current) >= threshold)
        || (i16::from(previous) > -threshold && i16::from(current) <= -threshold)
}

fn source_axis_crossed_positive(previous: i8, current: i8, threshold: i8) -> bool {
    let threshold = i16::from(threshold.abs());
    i16::from(previous) < threshold && i16::from(current) >= threshold
}

fn source_axis_crossed_negative(previous: i8, current: i8, threshold: i8) -> bool {
    let threshold = if threshold < 0 {
        i16::from(threshold)
    } else {
        -i16::from(threshold)
    };
    i16::from(previous) > threshold && i16::from(current) <= threshold
}

fn source_cstick_down_throw_held(previous: i8, current: i8, threshold: i8) -> bool {
    let threshold = if threshold < 0 {
        i16::from(threshold)
    } else {
        -i16::from(threshold)
    };
    i16::from(previous) <= threshold && i16::from(current) <= threshold
}

fn source_side_throw_action_state(stick_x: i8, facing: i8) -> MeleeActionStateId {
    if i16::from(stick_x) * i16::from(facing) > 0 {
        SOURCE_THROW_F_ACTION_STATE_ID
    } else {
        SOURCE_THROW_B_ACTION_STATE_ID
    }
}

fn source_throw_action_key(action_state_id: MeleeActionStateId) -> Option<SourceActionKey> {
    match action_state_id {
        SOURCE_THROW_F_ACTION_STATE_ID => Some(SOURCE_THROW_F_ACTION_KEY),
        SOURCE_THROW_B_ACTION_STATE_ID => Some(SOURCE_THROW_B_ACTION_KEY),
        SOURCE_THROW_HI_ACTION_STATE_ID => Some(SOURCE_THROW_HI_ACTION_KEY),
        SOURCE_THROW_LW_ACTION_STATE_ID => Some(SOURCE_THROW_LW_ACTION_KEY),
        _ => None,
    }
}

fn source_victim_thrown_action_for_throw(
    throw_action_state_id: MeleeActionStateId,
) -> Option<(MeleeActionStateId, SourceActionKey)> {
    match throw_action_state_id {
        SOURCE_THROW_F_ACTION_STATE_ID => {
            Some((SOURCE_THROWN_F_ACTION_STATE_ID, SOURCE_THROWN_F_ACTION_KEY))
        }
        SOURCE_THROW_B_ACTION_STATE_ID => {
            Some((SOURCE_THROWN_B_ACTION_STATE_ID, SOURCE_THROWN_B_ACTION_KEY))
        }
        SOURCE_THROW_HI_ACTION_STATE_ID => Some((
            SOURCE_THROWN_HI_ACTION_STATE_ID,
            SOURCE_THROWN_HI_ACTION_KEY,
        )),
        SOURCE_THROW_LW_ACTION_STATE_ID => Some((
            SOURCE_THROWN_LW_ACTION_STATE_ID,
            SOURCE_THROWN_LW_ACTION_KEY,
        )),
        _ => None,
    }
}

fn enter_source_throw_action(
    player: &mut PlayerState,
    action_state_id: MeleeActionStateId,
    source_action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
    common_data: MeleeCommonData,
    anim_speed: f32,
) {
    let Some(source_action_key) = source_throw_action_key(action_state_id) else {
        return;
    };
    player.motion_cmd_var0 = 0;
    player.motion_throw_flags = 0;
    player.source_throw_x4 = false;
    enter_source_grab_related_action(
        player,
        action_state_id,
        source_action_key,
        source_action_total_frames,
    );
    player.set_source_motion_anim_rate(anim_speed);
    sample_source_motion_frame_for_action_entry(player);
    player.apply_source_hurt_collision_lockout_timer(common_data.throw_collision_lockout_ticks);
}

fn source_throw_animation_speed(
    thrower_profile: FighterProfile,
    victim_profile: FighterProfile,
    action_state_id: MeleeActionStateId,
    common_data: MeleeCommonData,
) -> f32 {
    let throw_index = action_state_id
        .get()
        .saturating_sub(SOURCE_THROW_F_ACTION_STATE_ID.get());
    if throw_index < u8::BITS as u16
        && thrower_profile.weight_independent_throws_mask & (1u8 << throw_index) != 0
    {
        return 1.0;
    }
    let denominator = victim_profile.weight * common_data.throw_weight_animation_scale;
    if denominator > 0.0 {
        1.0 / denominator
    } else {
        1.0
    }
}

fn enter_source_grab_related_action(
    player: &mut PlayerState,
    action_state_id: MeleeActionStateId,
    source_action_key: SourceActionKey,
    source_action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
) {
    player.melee_action_state_id = Some(action_state_id);
    player.source_action_key = Some(source_action_key);
    player.source_action_total_frames = source_action_total_frames(action_state_id).unwrap_or(0);
    player.source_down_bound_pose = None;
    player.source_down_wait_timer = 0.0;
    player.motion_state_alias = None;
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.set_source_motion_anim_rate_milli(1_000);
    player.source_thrown_hitbox_owner_index = None;
    player.source_thrown_hitbox_team_unk = 0;
    player.source_thrown_hitbox_grabber_player_id = None;
    player.source_throw_x4 = false;
    player.source_thrown_unk_bool = false;
    player.source_thrown_anim_timer = 0.0;
    player.hitlag_frames = 0;
    player.source_allow_sdi = false;
    player.source_x2219_b5 = false;
    player.damage_hitstun_frames = 0;
    player.source_x2226_b2 = false;
}

fn apply_pending_source_throw_transitions(
    players: &mut [PlayerState; PLAYER_COUNT],
    pending: [Option<PendingSourceThrowTransition>; PLAYER_COUNT],
    source_action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
    source_pose_metadata: &mut impl FnMut(&PlayerState) -> Option<SourceActionPoseMetadata>,
) {
    for transition in pending.into_iter().flatten() {
        let Some((thrower, victim)) =
            source_players_two_mut(players, transition.thrower_index, transition.victim_index)
        else {
            continue;
        };
        victim.facing = transition.facing;
        victim.source_model_facing = victim.facing;
        victim.source_victim_index = Some(transition.thrower_index as u8);
        victim.source_x1a5c_index = Some(transition.thrower_index as u8);
        victim.source_x221b_b5 = false;
        victim.grounded = false;
        enter_source_grab_related_action(
            victim,
            transition.victim_action_state_id,
            transition.victim_source_action_key,
            source_action_total_frames,
        );
        victim.set_source_motion_anim_rate(transition.anim_speed);
        if let Ok(thrower_u8) = u8::try_from(transition.thrower_index) {
            victim.source_thrown_hitbox_owner_index = Some(thrower_u8);
            victim.source_thrown_hitbox_team_unk = 0;
            victim.source_thrown_hitbox_grabber_player_id = Some(thrower_u8);
        }
        victim.source_x2226_b2 = true;
        sample_source_motion_frame_for_action_entry(victim);
        thrower.source_victim_index = Some(transition.victim_index as u8);

        let mut source_pose_metadata_snapshots =
            [SourcePoseMetadataSnapshot::default(); PLAYER_COUNT];
        source_pose_metadata_snapshots[transition.thrower_index] =
            source_pose_metadata_snapshot_for_player(
                &players[transition.thrower_index],
                source_pose_metadata,
            );
        source_pose_metadata_snapshots[transition.victim_index] =
            source_pose_metadata_snapshot_for_player(
                &players[transition.victim_index],
                source_pose_metadata,
            );
        apply_source_thrown_accessory_position(
            &mut players[transition.victim_index],
            transition.victim_index,
            &source_pose_metadata_snapshots,
        );
    }
}

fn apply_pending_source_capture_wait_transitions(
    players: &mut [PlayerState; PLAYER_COUNT],
    pending: [Option<PendingSourceCaptureWaitTransition>; PLAYER_COUNT],
    common_data: MeleeCommonData,
    source_action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
) {
    for transition in pending.into_iter().flatten() {
        let Some((grabber, victim)) =
            source_players_two_mut(players, transition.grabber_index, transition.victim_index)
        else {
            continue;
        };
        let (victim_action_state_id, victim_source_action_key) =
            if victim.melee_action_state_id == Some(SOURCE_CAPTURE_PULLED_HI_ACTION_STATE_ID) {
                (
                    SOURCE_CAPTURE_WAIT_HI_ACTION_STATE_ID,
                    SOURCE_CAPTURE_WAIT_HI_ACTION_KEY,
                )
            } else {
                (
                    SOURCE_CAPTURE_WAIT_LW_ACTION_STATE_ID,
                    SOURCE_CAPTURE_WAIT_LW_ACTION_KEY,
                )
            };
        victim.source_victim_index = Some(transition.grabber_index as u8);
        victim.source_x1a5c_index = Some(transition.grabber_index as u8);
        victim.source_x221b_b5 = false;
        enter_source_grab_related_action(
            victim,
            victim_action_state_id,
            victim_source_action_key,
            source_action_total_frames,
        );
        victim.source_grab_timer = source_grab_timer_for_capture(victim, common_data);
        victim.source_grab_mash_x = 0;
        victim.source_grab_mash_y = 0;
        victim.source_capture_wait_timer = 0.0;
        victim.source_capture_wait_anim_timer = 0.0;
        victim.source_capture_wait_mashed = false;
        victim.source_capture_wait_jump_queued = false;
        grabber.source_victim_index = Some(transition.victim_index as u8);
        grabber.source_x1a5c_index = Some(transition.victim_index as u8);
    }
}

fn apply_pending_source_catch_cut_transitions(
    players: &mut [PlayerState; PLAYER_COUNT],
    pending: [Option<PendingSourceCatchCutTransition>; PLAYER_COUNT],
    common_data: MeleeCommonData,
    source_action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
) {
    for transition in pending.into_iter().flatten() {
        let Some(grabber) = players.get_mut(transition.grabber_index) else {
            continue;
        };
        if !matches!(
            grabber.melee_action_state_id,
            Some(
                SOURCE_CATCH_PULL_ACTION_STATE_ID
                    | SOURCE_CATCH_DASH_PULL_ACTION_STATE_ID
                    | SOURCE_CATCH_WAIT_ACTION_STATE_ID
                    | SOURCE_CATCH_ATTACK_ACTION_STATE_ID
            )
        ) {
            continue;
        }
        enter_source_catch_cut(grabber, common_data, source_action_total_frames);
    }
}

fn source_grab_timer_for_capture(victim: &PlayerState, common_data: MeleeCommonData) -> f32 {
    common_data.grab_timer_base
        + common_data.grab_timer_handicap_scale * common_data.grab_timer_handicap_offset
        + common_data.grab_timer_rank_scale * (common_data.grab_timer_rank_offset - 1.0)
        + victim.damage_percent * common_data.grab_timer_percent_scale
}

fn apply_pending_source_throw_anim_freezes(
    players: &mut [PlayerState; PLAYER_COUNT],
    pending: [Option<PendingSourceThrowAnimFreeze>; PLAYER_COUNT],
) {
    for freeze in pending.into_iter().flatten() {
        let Some(victim) = players.get_mut(freeze.victim_index) else {
            continue;
        };
        victim.source_thrown_unk_bool = true;
        if victim.motion_anim_frame_milli == (freeze.anim_timer * 1000.0).round() as i32 {
            victim.set_source_motion_anim_rate_milli(0);
            victim.source_thrown_anim_timer = 0.0;
        } else {
            victim.source_thrown_anim_timer = freeze.anim_timer;
        }
    }
}

fn apply_pending_source_throw_releases(
    world: &mut World,
    mut pending: [Option<PendingSourceThrowRelease>; PLAYER_COUNT],
    inputs: &[PlayerInput; PLAYER_COUNT],
    input_timers: &mut [MeleeInputTimers; PLAYER_COUNT],
    source_action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
) {
    for release in pending.iter_mut().filter_map(Option::take) {
        if world.apply_source_throw_release_damage_with_action_total_frames(
            release.thrower_index,
            release.victim_index,
            release.hitbox,
            release.source_release_transn2_position,
            release.source_release_last_pos,
            inputs[release.victim_index],
            &mut *source_action_total_frames,
        ) {
            reset_source_damage_entry_tilt_timers(input_timers, release.victim_index);
        }
    }
}

fn apply_pending_source_throw_releases_for_victim(
    world: &mut World,
    pending: &mut [Option<PendingSourceThrowRelease>; PLAYER_COUNT],
    victim_index: usize,
    victim_input: PlayerInput,
    input_timers: &mut [MeleeInputTimers; PLAYER_COUNT],
    source_action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
) -> bool {
    let mut applied_any = false;
    for release_slot in pending.iter_mut() {
        let Some(release) = *release_slot else {
            continue;
        };
        if release.victim_index != victim_index {
            continue;
        }
        *release_slot = None;
        if world.apply_source_throw_release_damage_with_action_total_frames(
            release.thrower_index,
            release.victim_index,
            release.hitbox,
            release.source_release_transn2_position,
            release.source_release_last_pos,
            victim_input,
            &mut *source_action_total_frames,
        ) {
            reset_source_damage_entry_tilt_timers(input_timers, release.victim_index);
            applied_any = true;
        }
    }
    applied_any
}

fn reset_source_damage_entry_tilt_timers(
    input_timers: &mut [MeleeInputTimers; PLAYER_COUNT],
    player_index: usize,
) {
    if let Some(timers) = input_timers.get_mut(player_index) {
        timers.x_tap = EXPIRED_INPUT_TIMER;
        timers.y_tap = EXPIRED_INPUT_TIMER;
    }
}

fn source_players_two_mut(
    players: &mut [PlayerState; PLAYER_COUNT],
    first: usize,
    second: usize,
) -> Option<(&mut PlayerState, &mut PlayerState)> {
    if first == second || first >= PLAYER_COUNT || second >= PLAYER_COUNT {
        return None;
    }
    if first < second {
        let (left, right) = players.split_at_mut(second);
        Some((&mut left[first], &mut right[0]))
    } else {
        let (left, right) = players.split_at_mut(first);
        Some((&mut right[0], &mut left[second]))
    }
}

fn advance_source_damage_state(
    player: &mut PlayerState,
    stage: StageProfile,
    common_data: MeleeCommonData,
    _previous_ecb_bottom: Vec2,
    input_facts: MeleeInputFacts,
    stick_x: i32,
    stick_y: i8,
    source_pose_metadata: &mut impl FnMut(&PlayerState) -> Option<SourceActionPoseMetadata>,
    source_action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
) -> bool {
    let previous_position = player.position;
    let floor_surface_before_ground_move = source_floor_surface_before_ground_move(stage, player);

    player.motion_frame = player.motion_frame.saturating_add(1);
    player.set_source_motion_anim_frame(f32::from(player.motion_frame));
    if let Some(metadata) = source_pose_metadata(player) {
        player.source_down_bound_pose = metadata.down_bound_pose;
    }
    if player.damage_hitstun_frames > 0 {
        player.damage_hitstun_frames -= 1;
    }

    if player.damage_hitstun_frames == 0
        && !player.grounded
        && apply_airborne_iasa_actions(player, input_facts, stick_x, stick_y, common_data)
    {
        // DamageFly_Anim clears x221C_b6 before DamageFly_IASA runs.
        // The newly entered aerial action owns this tick's physics callback.
        return true;
    }

    if player.grounded {
        apply_ground_traction(player, stage, common_data);
        player.velocity.y = 0;
        player.source_self_velocity_y = 0.0;
        add_source_position_x(player, player.player_nudge_x);
        add_source_position_z(player, player.player_nudge_z);
        add_source_position_x(player, ground_position_delta_x(player));
        commit_ground_velocity(player, stage);
        apply_source_ground_knockback_physics(player, stage, common_data);
        add_source_position_x(player, player.source_knockback_velocity_x);
        add_source_position_y(player, player.source_knockback_velocity_y);
        if !resolve_ground_support_after_move(
            stage,
            player,
            player.motion_state,
            stick_x,
            floor_surface_before_ground_move,
            common_data,
            source_vec2_from_milli_position(previous_position),
        ) {
            player.grounded = false;
        }
    } else {
        apply_source_damage_air_physics(player, stick_x, common_data);
        add_source_position_x(
            player,
            player.source_self_velocity_x + player.source_knockback_velocity_x,
        );
        clear_ground_accels(player);
        add_source_position_y(
            player,
            player.source_self_velocity_y + player.source_knockback_velocity_y,
        );
        if source_ft_80081dd4_damage_air_collision(stage, player, common_data) {
            apply_source_damage_floor_collision(
                player,
                stage,
                common_data,
                stick_x,
                source_action_total_frames,
            );
        }
    }

    if player.damage_hitstun_frames == 0 && source_damage_animation_has_no_frames_remaining(player)
    {
        if player.grounded {
            player.set_motion_state_alias(MotionState::Wait);
            player.motion_frame = 0;
            player.set_source_motion_anim_frame(0.0);
            player.velocity.y = 0;
        } else if player
            .melee_action_state_id
            .is_some_and(is_source_damage_fly_action_state_id)
            || player.source_force_damage_down_bound
        {
            enter_source_damage_fall(player, source_action_total_frames);
        } else {
            enter_fall(player);
        }
    }
    false
}

fn source_ft_80081dd4_damage_air_collision(
    stage: StageProfile,
    player: &mut PlayerState,
    common_data: MeleeCommonData,
) -> bool {
    let Some(melee_stage) = stage.melee_stage_profile() else {
        return false;
    };
    let collision = melee_stage.collision;
    source_mp_coll_air_wrapper_begin(player);
    let touched_floor = if player.source_allow_sdi {
        source_mp_coll_800477e0(stage, collision, player, common_data)
    } else if player.source_ledge_cooldown_timer != 0 || player.source_force_damage_down_bound {
        source_mp_coll_800471f8(stage, collision, player, common_data)
    } else {
        source_mp_coll_800473cc(stage, collision, player, common_data)
    };
    set_source_position_x_source(player, player.source_coll_cur_pos.x);
    set_source_position_y_source(player, player.source_coll_cur_pos.y);
    touched_floor
}

fn source_action_animation_done(player: &PlayerState) -> bool {
    player.source_action_total_frames == 0
        || player.source_motion_anim_frame >= f32::from(player.source_action_total_frames)
}

fn source_damage_animation_has_no_frames_remaining(player: &PlayerState) -> bool {
    player.source_action_total_frames == 0
        || player.source_motion_anim_frame
            >= f32::from(player.source_action_total_frames.saturating_sub(1))
}

fn source_shield_break_action_done(player: &PlayerState) -> bool {
    player.source_action_total_frames > 0
        && player.source_motion_anim_frame >= f32::from(player.source_action_total_frames)
}

fn source_action_has_no_frames_remaining(player: &PlayerState) -> bool {
    player.source_action_total_frames > 0
        && player.source_motion_anim_frame >= f32::from(player.source_action_total_frames)
}

fn source_action_completed_before_frame_input(player: &PlayerState) -> bool {
    player.source_action_total_frames > 0
        && player.motion_frame >= player.source_action_total_frames.saturating_sub(1)
}

fn enter_source_shield_break_fall(player: &mut PlayerState) {
    player.set_motion_state_alias(MotionState::ShieldBreakFall);
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.set_source_motion_anim_rate_milli(1_000);
    player.grounded = false;
    player.fast_falling = false;
}

fn enter_source_shield_break_down(player: &mut PlayerState) {
    let Some(face_up) = source_down_bound_face_up(player) else {
        return;
    };
    let motion_state = if face_up {
        MotionState::ShieldBreakDownU
    } else {
        MotionState::ShieldBreakDownD
    };
    player.set_motion_state_alias(motion_state);
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.set_source_motion_anim_rate_milli(1_000);
    player.grounded = true;
    player.fast_falling = false;
    player.velocity.y = 0;
    player.source_self_velocity_y = 0.0;
}

fn enter_source_shield_break_stand(player: &mut PlayerState) {
    let motion_state = if player.motion_state == MotionState::ShieldBreakDownU {
        MotionState::ShieldBreakStandU
    } else {
        MotionState::ShieldBreakStandD
    };
    player.set_motion_state_alias(motion_state);
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.set_source_motion_anim_rate_milli(1_000);
}

fn enter_source_furafura(player: &mut PlayerState, common_data: MeleeCommonData) {
    player.set_motion_state_alias(MotionState::Furafura);
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.set_source_motion_anim_rate_milli(1_000);
    player.shield_health = common_data.shield_break_reset_health;
    let percent_component =
        (common_data.shield_break_furafura_percent_base - player.damage_percent).max(0.0);
    player.source_grab_timer = percent_component + common_data.shield_break_furafura_timer_base;
    player.source_grab_mash_x = 0;
    player.source_grab_mash_y = 0;
}

fn enter_source_common_action_end(player: &mut PlayerState) {
    if player.grounded {
        player.set_motion_state_alias(MotionState::Wait);
        player.motion_frame = 0;
        player.set_source_motion_anim_frame(0.0);
    } else {
        enter_fall(player);
    }
    clear_motion_script_state(player);
    player.source_victim_index = None;
    player.source_x1a5c_index = None;
    player.source_x221b_b5 = false;
    player.source_x2226_b2 = false;
    player.source_throw_x4 = false;
    player.source_thrown_unk_bool = false;
    player.source_thrown_anim_timer = 0.0;
}

fn enter_source_damage_fall(
    player: &mut PlayerState,
    source_action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
) {
    if player.grounded {
        source_ft_common_8007d5d4_ground_to_air(player);
    }
    source_clear_guard_shield(player);
    clear_motion_script_state(player);
    player.motion_state = MotionState::DamageFall;
    player.motion_state_alias = None;
    player.source_motion_entry_facing = player.facing;
    player.melee_action_state_id = Some(SOURCE_DAMAGE_FALL_ACTION_STATE_ID);
    player.source_action_key = Some(SOURCE_DAMAGE_FALL_ACTION_KEY);
    player.source_action_total_frames =
        source_action_total_frames(SOURCE_DAMAGE_FALL_ACTION_STATE_ID).unwrap_or(0);
    player.source_down_bound_pose = None;
    player.source_down_wait_timer = 0.0;
    player.damage_hitstun_frames = 0;
    player.source_allow_sdi = false;
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.set_source_motion_anim_rate_milli(1_000);
    player.grounded = false;
    player.fast_falling = false;
    player.source_ground_knockback_velocity = 0.0;
    set_source_self_velocity_x(
        player,
        player
            .source_self_velocity_x
            .clamp(-player.profile.air_drift_max, player.profile.air_drift_max),
    );
    set_source_self_velocity_y(player, player.source_self_velocity_y);
    clear_ground_accels(player);
}

fn apply_source_damage_floor_collision(
    player: &mut PlayerState,
    stage: StageProfile,
    common_data: MeleeCommonData,
    stick_x: i32,
    source_action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
) {
    let Some(action_state_id) = player.melee_action_state_id else {
        return;
    };

    if is_source_standard_damage_action_state_id(action_state_id) {
        let magnitude = source_damage_knockback_velocity_magnitude(player);
        if player.source_force_damage_down_bound
            || magnitude >= common_data.damage_landing_down_bound_knockback_threshold
        {
            enter_source_damage_down_bound(player, stage, common_data, source_action_total_frames);
            return;
        }
        if magnitude >= common_data.damage_landing_basic_knockback_threshold {
            enter_landing_as(player, MotionState::Landing, 0);
        }
    } else if is_source_damage_fly_action_state_id(action_state_id) {
        apply_source_damage_fly_floor_collision(
            player,
            stage,
            common_data,
            stick_x,
            source_action_total_frames,
        );
    }
}

fn apply_source_damage_fly_floor_collision(
    player: &mut PlayerState,
    stage: StageProfile,
    common_data: MeleeCommonData,
    stick_x: i32,
    source_action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
) {
    // ftCo_DamageFly_Coll / ftCo_DamageFlyRoll_Coll try passive first, then
    // fall through to ftCo_80097D40.
    if try_enter_source_damage_fly_passive(
        player,
        stage,
        common_data,
        stick_x,
        source_action_total_frames,
    ) {
        return;
    }
    enter_source_damage_down_bound(player, stage, common_data, source_action_total_frames);
}

fn try_enter_source_damage_fly_passive(
    player: &mut PlayerState,
    stage: StageProfile,
    common_data: MeleeCommonData,
    stick_x: i32,
    source_action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
) -> bool {
    if !source_passive_window_open(player, common_data) {
        return false;
    }

    let stick = fighter_stick_axis_to_f32(stick_x.clamp(-127, 127) as i8);
    if stick.abs() >= common_data.passive_stand_stick_x {
        let (action_state_id, source_action_key) = if stick * f32::from(player.facing) >= 0.0 {
            (
                SOURCE_PASSIVE_STAND_F_ACTION_STATE_ID,
                SOURCE_PASSIVE_STAND_F_ACTION_KEY,
            )
        } else {
            (
                SOURCE_PASSIVE_STAND_B_ACTION_STATE_ID,
                SOURCE_PASSIVE_STAND_B_ACTION_KEY,
            )
        };
        enter_source_passive_action(
            player,
            stage,
            common_data,
            action_state_id,
            source_action_key,
            source_action_total_frames,
        );
        return true;
    }

    enter_source_passive_action(
        player,
        stage,
        common_data,
        SOURCE_PASSIVE_ACTION_STATE_ID,
        SOURCE_PASSIVE_ACTION_KEY,
        source_action_total_frames,
    );
    true
}

fn source_passive_window_open(player: &PlayerState, common_data: MeleeCommonData) -> bool {
    // ftCo_800986B0 also rejects the Hammer item branch via ftCo_800C5240.
    // Held-item state is not represented in this core slice, so this is the
    // normal no-Hammer path through the decomp predicate.
    // Rust advances input timers before fighter collision; x680 is read by the
    // decomp collision callback before that tick's timer increment.
    f32::from(player.source_lr_digital_press_timer.saturating_sub(1))
        < common_data.passive_window_max
        && player.source_previous_lr_digital_press_timer >= common_data.passive_input_age_threshold
}

fn enter_source_passive_action(
    player: &mut PlayerState,
    stage: StageProfile,
    common_data: MeleeCommonData,
    action_state_id: MeleeActionStateId,
    source_action_key: SourceActionKey,
    source_action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
) {
    clear_shield_turn(player);
    clear_turn_state(player);
    player.melee_action_state_id = Some(action_state_id);
    player.source_action_key = Some(source_action_key);
    player.source_action_total_frames = source_action_total_frames(action_state_id).unwrap_or(0);
    player.source_down_bound_pose = None;
    player.source_down_wait_timer = 0.0;
    player.motion_state_alias = None;
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.set_source_motion_anim_rate_milli(1_000);
    player.grounded = true;
    player.fast_falling = false;
    player.velocity.y = 0;
    set_ground_velocity_x(player, 0.0);
    source_ftcommon_8007cce8_project_ground_knockback(player, stage, common_data);
}

fn enter_source_damage_down_bound(
    player: &mut PlayerState,
    stage: StageProfile,
    common_data: MeleeCommonData,
    source_action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
) {
    let Some(face_up) = source_down_bound_face_up(player) else {
        return;
    };
    let (action_state_id, source_action_key) = if face_up {
        (
            SOURCE_DOWN_BOUND_U_ACTION_STATE_ID,
            SOURCE_DOWN_BOUND_U_ACTION_KEY,
        )
    } else {
        (
            SOURCE_DOWN_BOUND_D_ACTION_STATE_ID,
            SOURCE_DOWN_BOUND_D_ACTION_KEY,
        )
    };

    clear_shield_turn(player);
    clear_turn_state(player);
    player.melee_action_state_id = Some(action_state_id);
    player.source_action_key = Some(source_action_key);
    player.source_action_total_frames = source_action_total_frames(action_state_id).unwrap_or(0);
    player.source_down_wait_timer = 0.0;
    player.motion_state_alias = None;
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.set_source_motion_anim_rate_milli(1_000);
    player.grounded = true;
    player.fast_falling = false;
    player.velocity.y = 0;
    set_ground_velocity_x(player, 0.0);
    source_ftcommon_8007cce8_project_ground_knockback(player, stage, common_data);
}

fn source_ftcommon_8007cce8_project_ground_knockback(
    player: &mut PlayerState,
    stage: StageProfile,
    common_data: MeleeCommonData,
) {
    if !player.grounded || player.source_ground_knockback_velocity != 0.0 {
        return;
    }
    let clamp = common_data.damage_ground_knockback_init_clamp;
    player.source_ground_knockback_velocity =
        player.source_knockback_velocity_x.clamp(-clamp, clamp);
    let ground_normal = source_ground_normal_for_player(stage, player);
    player.source_knockback_velocity_x = ground_normal.y * player.source_ground_knockback_velocity;
    player.source_knockback_velocity_y = -ground_normal.x * player.source_ground_knockback_velocity;
    sync_damage_velocity_projection(player);
}

fn source_down_bound_face_up(player: &PlayerState) -> Option<bool> {
    let pose = player.source_down_bound_pose?;
    let axis = if player.source_down_bound_use_z_axis {
        pose.hip_mtx_1_2
    } else {
        pose.hip_mtx_1_1
    };
    let face_up = axis > 0.0;
    Some(if player.source_down_bound_reverse_face_up {
        !face_up
    } else {
        face_up
    })
}

fn is_source_standard_damage_action_state_id(action_state_id: MeleeActionStateId) -> bool {
    let id = action_state_id.get();
    id >= 75 && id <= 86
}

fn is_source_damage_fly_action_state_id(action_state_id: MeleeActionStateId) -> bool {
    let id = action_state_id.get();
    id >= 87 && id <= 91
}

fn advance_source_passive_state(
    player: &mut PlayerState,
    stage: StageProfile,
    common_data: MeleeCommonData,
    stick_x: i32,
) {
    let passive_stand = source_passive_state_uses_ft_80084fa8(player);
    player.motion_frame = player.motion_frame.saturating_add(1);
    player.set_source_motion_anim_frame(f32::from(player.motion_frame));
    if source_action_animation_done(player) {
        player.set_motion_state_alias(MotionState::Wait);
        player.motion_frame = 0;
        player.set_source_motion_anim_frame(0.0);
        player.velocity.y = 0;
        return;
    }

    if player.grounded {
        let kept_grounded = if passive_stand {
            advance_source_passive_stand_ground_physics(player, stage, common_data, stick_x)
        } else {
            advance_source_down_ground_physics(player, stage, common_data, stick_x)
        };
        if !kept_grounded {
            enter_fall(player);
        }
    } else {
        enter_fall(player);
    }
}

fn source_passive_state_uses_ft_80084fa8(player: &PlayerState) -> bool {
    player.melee_action_state_id.is_some_and(|action_state_id| {
        matches!(
            action_state_id,
            SOURCE_PASSIVE_STAND_F_ACTION_STATE_ID | SOURCE_PASSIVE_STAND_B_ACTION_STATE_ID
        )
    })
}

fn advance_source_passive_stand_ground_physics(
    player: &mut PlayerState,
    stage: StageProfile,
    common_data: MeleeCommonData,
    stick_x: i32,
) -> bool {
    let previous_position = player.position;
    let floor_surface_before_ground_move = source_floor_surface_before_ground_move(stage, player);
    apply_source_ft_80084fa8_ground_physics(player, stage, common_data);
    player.velocity.y = 0;
    player.source_self_velocity_y = 0.0;
    add_source_position_x(player, player.player_nudge_x);
    add_source_position_z(player, player.player_nudge_z);
    add_source_position_x(player, ground_position_delta_x(player));
    commit_ground_velocity(player, stage);
    apply_common_source_knockback_physics(player, stage, common_data);
    add_source_position_x(player, player.source_knockback_velocity_x);
    add_source_position_y(player, player.source_knockback_velocity_y);
    resolve_ground_support_after_move(
        stage,
        player,
        player.motion_state,
        stick_x,
        floor_surface_before_ground_move,
        common_data,
        source_vec2_from_milli_position(previous_position),
    )
}

fn advance_source_down_bound_state(
    player: &mut PlayerState,
    stage: StageProfile,
    common_data: MeleeCommonData,
    _previous_ecb_bottom: Vec2,
    stick_x: i32,
    stick_y: i32,
    source_pose_metadata: &mut impl FnMut(&PlayerState) -> Option<SourceActionPoseMetadata>,
    source_action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
) {
    player.motion_frame = player.motion_frame.saturating_add(1);
    player.set_source_motion_anim_frame(f32::from(player.motion_frame));
    if let Some(metadata) = source_pose_metadata(player) {
        player.source_down_bound_pose = metadata.down_bound_pose;
        apply_source_script_events(player, metadata.script_events, common_data);
    }
    if player.source_action_total_frames > 0
        && player.motion_frame >= player.source_action_total_frames
    {
        if source_down_wait_roll_input(stick_x, stick_y, common_data) {
            enter_source_down_roll(
                player,
                stick_x,
                stage,
                common_data,
                source_action_total_frames,
            );
            return;
        }
        enter_source_down_wait(player, common_data, source_action_total_frames);
        return;
    }

    if player.grounded {
        if !advance_source_down_ground_physics(player, stage, common_data, stick_x) {
            player.grounded = false;
        }
    } else {
        advance_source_down_bound_air_lane_physics(player, stage, common_data);
        apply_source_down_bound_collision(player, stage, common_data);
    }
}

fn apply_source_down_bound_collision(
    player: &mut PlayerState,
    stage: StageProfile,
    common_data: MeleeCommonData,
) {
    source_fighter_proc_map_begin(player);
    let Some(melee_stage) = stage.melee_stage_profile() else {
        return;
    };
    ensure_source_floor_line_index(stage, melee_stage.collision, player);
    let ft_80082708_returned_ga_air =
        source_ft_80082708_allow_ground_to_air(stage, melee_stage.collision, player, common_data);
    if !ft_80082708_returned_ga_air {
        enter_fall(player);
    }
}

fn enter_source_down_wait(
    player: &mut PlayerState,
    common_data: MeleeCommonData,
    source_action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
) {
    let face_up = matches!(
        player.melee_action_state_id,
        Some(SOURCE_DOWN_BOUND_U_ACTION_STATE_ID)
    );
    let (action_state_id, source_action_key) = if face_up {
        (
            SOURCE_DOWN_WAIT_U_ACTION_STATE_ID,
            SOURCE_DOWN_WAIT_U_ACTION_KEY,
        )
    } else {
        (
            SOURCE_DOWN_WAIT_D_ACTION_STATE_ID,
            SOURCE_DOWN_WAIT_D_ACTION_KEY,
        )
    };

    player.melee_action_state_id = Some(action_state_id);
    player.source_action_key = Some(source_action_key);
    player.source_action_total_frames = source_action_total_frames(action_state_id).unwrap_or(0);
    player.source_down_bound_pose = None;
    player.source_down_wait_timer = common_data.down_wait_timer;
    player.motion_state_alias = None;
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.set_source_motion_anim_rate_milli(1_000);
    player.grounded = true;
    player.fast_falling = false;
    player.velocity.y = 0;
}

fn advance_source_down_wait_state(
    player: &mut PlayerState,
    stage: StageProfile,
    common_data: MeleeCommonData,
    input_facts: MeleeInputFacts,
    stick_x: i32,
    stick_y: i32,
    source_action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
) {
    player.motion_frame = player.motion_frame.saturating_add(1);
    player.set_source_motion_anim_frame(f32::from(player.motion_frame));
    if player.source_action_total_frames > 0
        && player.motion_frame >= player.source_action_total_frames
    {
        let last_frame = player.source_action_total_frames.saturating_sub(1);
        player.motion_frame = last_frame;
        player.set_source_motion_anim_frame(f32::from(last_frame));
    }
    if !player.source_force_damage_down_bound {
        player.source_down_wait_timer -= 1.0;
    }
    if player.source_down_wait_timer <= 0.0 {
        enter_source_down_stand(player, source_action_total_frames);
        return;
    }
    if input_facts.source_pressed.a() || input_facts.source_pressed.b() {
        enter_source_down_attack(player, source_action_total_frames);
        return;
    }
    if source_down_wait_roll_input(stick_x, stick_y, common_data) {
        enter_source_down_roll(
            player,
            stick_x,
            stage,
            common_data,
            source_action_total_frames,
        );
        return;
    }
    if source_down_wait_stand_input(input_facts, stick_x, stick_y, common_data) {
        enter_source_down_stand(player, source_action_total_frames);
        return;
    }

    if player.grounded {
        if !advance_source_down_ground_physics(player, stage, common_data, stick_x) {
            enter_fall(player);
        }
    } else {
        enter_fall(player);
    }
}

fn source_down_wait_roll_input(stick_x: i32, stick_y: i32, common_data: MeleeCommonData) -> bool {
    let abs_x = stick_x.abs();
    abs_x >= i32::from(common_data.down_roll_stick_x)
        && (stick_y <= 0 || stick_y * 1000 < abs_x * common_data.aerial_vertical_angle_tan_milli)
}

fn enter_source_down_roll(
    player: &mut PlayerState,
    stick_x: i32,
    stage: StageProfile,
    common_data: MeleeCommonData,
    source_action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
) {
    let face_up = matches!(
        player.melee_action_state_id,
        Some(SOURCE_DOWN_WAIT_U_ACTION_STATE_ID)
    );
    let forward = stick_x * i32::from(player.facing) >= 0;
    let (action_state_id, source_action_key) = match (face_up, forward) {
        (true, true) => (
            SOURCE_DOWN_FORWARD_U_ACTION_STATE_ID,
            SOURCE_DOWN_FORWARD_U_ACTION_KEY,
        ),
        (true, false) => (
            SOURCE_DOWN_BACK_U_ACTION_STATE_ID,
            SOURCE_DOWN_BACK_U_ACTION_KEY,
        ),
        (false, true) => (
            SOURCE_DOWN_FORWARD_D_ACTION_STATE_ID,
            SOURCE_DOWN_FORWARD_D_ACTION_KEY,
        ),
        (false, false) => (
            SOURCE_DOWN_BACK_D_ACTION_STATE_ID,
            SOURCE_DOWN_BACK_D_ACTION_KEY,
        ),
    };

    player.melee_action_state_id = Some(action_state_id);
    player.source_action_key = Some(source_action_key);
    player.source_action_total_frames = source_action_total_frames(action_state_id).unwrap_or(0);
    player.source_down_wait_timer = 0.0;
    player.motion_state_alias = None;
    player.motion_frame = 1;
    player.set_source_motion_anim_frame(1.0);
    player.set_source_motion_anim_rate_milli(1_000);
    player.grounded = true;
    player.fast_falling = false;
    player.velocity.y = 0;
    source_ftcommon_8007cce8_project_ground_knockback(player, stage, common_data);
    if !advance_source_passive_stand_ground_physics(player, stage, common_data, stick_x) {
        enter_fall(player);
    }
}

fn source_down_wait_stand_input(
    input_facts: MeleeInputFacts,
    stick_x: i32,
    stick_y: i32,
    common_data: MeleeCommonData,
) -> bool {
    input_facts.source_pressed.lr()
        || source_down_wait_up_stick_stand_input(stick_x, stick_y, common_data)
}

fn source_down_wait_up_stick_stand_input(
    stick_x: i32,
    stick_y: i32,
    common_data: MeleeCommonData,
) -> bool {
    if stick_y < common_data.down_stand_stick_y as i32 {
        return false;
    }
    if stick_y <= 0 {
        return false;
    }
    let abs_x = stick_x.abs();
    if abs_x == 0 {
        return true;
    }
    stick_y.abs() * 1000 >= abs_x * common_data.aerial_vertical_angle_tan_milli
}

fn enter_source_down_stand(
    player: &mut PlayerState,
    source_action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
) {
    let face_up = matches!(
        player.melee_action_state_id,
        Some(SOURCE_DOWN_WAIT_U_ACTION_STATE_ID)
    );
    let (action_state_id, source_action_key) = if face_up {
        (
            SOURCE_DOWN_STAND_U_ACTION_STATE_ID,
            SOURCE_DOWN_STAND_U_ACTION_KEY,
        )
    } else {
        (
            SOURCE_DOWN_STAND_D_ACTION_STATE_ID,
            SOURCE_DOWN_STAND_D_ACTION_KEY,
        )
    };

    player.melee_action_state_id = Some(action_state_id);
    player.source_action_key = Some(source_action_key);
    player.source_action_total_frames = source_action_total_frames(action_state_id).unwrap_or(0);
    player.source_down_wait_timer = 0.0;
    player.motion_state_alias = None;
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.set_source_motion_anim_rate_milli(1_000);
    player.grounded = true;
    player.fast_falling = false;
    player.velocity.y = 0;
}

fn advance_source_down_stand_state(
    player: &mut PlayerState,
    stage: StageProfile,
    common_data: MeleeCommonData,
    stick_x: i32,
) {
    player.motion_frame = player.motion_frame.saturating_add(1);
    player.set_source_motion_anim_frame(f32::from(player.motion_frame));
    if source_action_animation_done(player) {
        player.set_motion_state_alias(MotionState::Wait);
        player.motion_frame = 0;
        player.set_source_motion_anim_frame(0.0);
        player.velocity.y = 0;
        return;
    }

    if player.grounded {
        if !advance_source_down_ground_physics(player, stage, common_data, stick_x) {
            enter_fall(player);
        }
    } else {
        enter_fall(player);
    }
}

fn enter_source_down_attack(
    player: &mut PlayerState,
    source_action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
) {
    let face_up = matches!(
        player.melee_action_state_id,
        Some(SOURCE_DOWN_WAIT_U_ACTION_STATE_ID)
    );
    let (action_state_id, source_action_key) = if face_up {
        (
            SOURCE_DOWN_ATTACK_U_ACTION_STATE_ID,
            SOURCE_DOWN_ATTACK_U_ACTION_KEY,
        )
    } else {
        (
            SOURCE_DOWN_ATTACK_D_ACTION_STATE_ID,
            SOURCE_DOWN_ATTACK_D_ACTION_KEY,
        )
    };

    player.melee_action_state_id = Some(action_state_id);
    player.source_action_key = Some(source_action_key);
    player.source_action_total_frames = source_action_total_frames(action_state_id).unwrap_or(0);
    player.motion_state_alias = None;
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.set_source_motion_anim_rate_milli(1_000);
    player.grounded = true;
    player.fast_falling = false;
    player.velocity.y = 0;
}

fn advance_source_down_attack_state(
    player: &mut PlayerState,
    stage: StageProfile,
    common_data: MeleeCommonData,
    stick_x: i32,
) {
    player.motion_frame = player.motion_frame.saturating_add(1);
    player.set_source_motion_anim_frame(f32::from(player.motion_frame));
    if source_action_animation_done(player) {
        player.set_motion_state_alias(MotionState::Wait);
        player.motion_frame = 0;
        player.set_source_motion_anim_frame(0.0);
        player.velocity.y = 0;
        return;
    }

    if player.grounded {
        if !advance_source_down_ground_physics(player, stage, common_data, stick_x) {
            enter_fall(player);
        }
    } else {
        enter_fall(player);
    }
}

fn advance_source_down_roll_state(
    player: &mut PlayerState,
    stage: StageProfile,
    common_data: MeleeCommonData,
    stick_x: i32,
) {
    player.motion_frame = player.motion_frame.saturating_add(1);
    player.set_source_motion_anim_frame(f32::from(player.motion_frame));
    if source_action_animation_done(player) {
        player.set_motion_state_alias(MotionState::Wait);
        player.motion_frame = 0;
        player.set_source_motion_anim_frame(0.0);
        player.velocity.y = 0;
        return;
    }

    if player.grounded {
        if !advance_source_passive_stand_ground_physics(player, stage, common_data, stick_x) {
            enter_fall(player);
        }
    } else {
        enter_fall(player);
    }
}

fn advance_source_down_ground_physics(
    player: &mut PlayerState,
    stage: StageProfile,
    common_data: MeleeCommonData,
    stick_x: i32,
) -> bool {
    let previous_position = player.position;
    let floor_surface_before_ground_move = source_floor_surface_before_ground_move(stage, player);
    apply_ground_traction(player, stage, common_data);
    player.velocity.y = 0;
    player.source_self_velocity_y = 0.0;
    add_source_position_x(player, player.player_nudge_x);
    add_source_position_z(player, player.player_nudge_z);
    add_source_position_x(player, ground_position_delta_x(player));
    commit_ground_velocity(player, stage);
    apply_source_ground_knockback_physics(player, stage, common_data);
    add_source_position_x(player, player.source_knockback_velocity_x);
    add_source_position_y(player, player.source_knockback_velocity_y);
    resolve_ground_support_after_move(
        stage,
        player,
        player.motion_state,
        stick_x,
        floor_surface_before_ground_move,
        common_data,
        source_vec2_from_milli_position(previous_position),
    )
}

fn advance_source_down_bound_air_lane_physics(
    player: &mut PlayerState,
    stage: StageProfile,
    common_data: MeleeCommonData,
) {
    apply_ground_traction(player, stage, common_data);
    player.velocity.y = 0;
    player.source_self_velocity_y = 0.0;
    add_source_position_x(player, player.player_nudge_x);
    add_source_position_z(player, player.player_nudge_z);
    add_source_position_x(player, ground_position_delta_x(player));
    commit_ground_velocity(player, stage);
    apply_common_source_knockback_physics(player, stage, common_data);
    sync_damage_velocity_projection(player);
    add_source_position_x(player, player.source_knockback_velocity_x);
    add_source_position_y(player, player.source_knockback_velocity_y);
}

fn source_damage_knockback_velocity_magnitude(player: &PlayerState) -> f32 {
    let x = player.source_knockback_velocity_x;
    let y = player.source_knockback_velocity_y;
    (x * x + y * y).sqrt()
}

fn apply_common_source_knockback_physics(
    player: &mut PlayerState,
    stage: StageProfile,
    common_data: MeleeCommonData,
) {
    if player.source_knockback_velocity_x == 0.0 && player.source_knockback_velocity_y == 0.0 {
        return;
    }
    if player.grounded {
        apply_source_ground_knockback_physics(player, stage, common_data);
    } else {
        player.source_ground_knockback_velocity = 0.0;
        apply_source_knockback_frame_decay(player, common_data);
    }
}

fn apply_source_damage_air_physics(
    player: &mut PlayerState,
    stick_x: i32,
    common_data: MeleeCommonData,
) {
    player.source_ground_knockback_velocity = 0.0;
    if player.damage_hitstun_frames > 0 {
        apply_source_damage_locked_air_physics(player);
    } else {
        apply_air_drift(player, stick_x, common_data);
        apply_source_self_gravity(player);
    }
    apply_source_knockback_frame_decay(player, common_data);
    sync_damage_velocity_projection(player);
}

fn apply_source_damage_locked_air_physics(player: &mut PlayerState) {
    apply_source_self_gravity(player);
    player.source_self_velocity_x = apply_friction_to_zero(
        player.source_self_velocity_x,
        player.profile.aerial_friction,
    );
}

fn apply_source_self_gravity(player: &mut PlayerState) {
    let profile = player.profile;
    player.source_self_velocity_y =
        (player.source_self_velocity_y - profile.gravity).max(-profile.terminal_velocity);
}

fn apply_source_knockback_frame_decay(player: &mut PlayerState, common_data: MeleeCommonData) {
    let kb_x = player.source_knockback_velocity_x;
    let kb_y = player.source_knockback_velocity_y;
    if kb_x == 0.0 && kb_y == 0.0 {
        return;
    }
    if !player.grounded {
        let kb_angle = kb_y.atan2(kb_x);
        if source_damage_knockback_velocity_magnitude(player)
            < common_data.damage_knockback_frame_decay
        {
            player.source_knockback_velocity_x = 0.0;
            player.source_knockback_velocity_y = 0.0;
        } else {
            player.source_knockback_velocity_x -=
                common_data.damage_knockback_frame_decay * kb_angle.cos();
            player.source_knockback_velocity_y -=
                common_data.damage_knockback_frame_decay * kb_angle.sin();
        }
    }
}

fn apply_source_ground_knockback_physics(
    player: &mut PlayerState,
    stage: StageProfile,
    common_data: MeleeCommonData,
) {
    if player.source_knockback_velocity_x == 0.0 && player.source_knockback_velocity_y == 0.0 {
        return;
    }
    if player.source_ground_knockback_velocity == 0.0 {
        player.source_ground_knockback_velocity = player.source_knockback_velocity_x;
    }
    let friction = floor_friction_multiplier_for_bottom(stage, player.position)
        * player.profile.ground_friction
        * common_data.damage_ground_knockback_friction_multiplier;
    player.source_ground_knockback_velocity =
        apply_friction_to_zero(player.source_ground_knockback_velocity, friction);
    let ground_normal = source_ground_normal_for_player(stage, player);
    player.source_knockback_velocity_x = ground_normal.y * player.source_ground_knockback_velocity;
    player.source_knockback_velocity_y = -ground_normal.x * player.source_ground_knockback_velocity;
    sync_damage_velocity_projection(player);
}

fn apply_source_attacker_shield_knockback_physics(
    player: &mut PlayerState,
    stage: StageProfile,
    common_data: MeleeCommonData,
) {
    if player.source_attacker_shield_velocity_x == 0.0 {
        return;
    }
    let friction = floor_friction_multiplier_for_bottom(stage, player.position)
        * player.profile.ground_friction
        * common_data.attacker_shield_ground_friction_multiplier;
    player.source_attacker_shield_velocity_x =
        apply_friction_to_zero(player.source_attacker_shield_velocity_x, friction);
}

fn sync_damage_velocity_projection(player: &mut PlayerState) {
    player.velocity.x =
        source_units_to_milli(player.source_self_velocity_x + player.source_knockback_velocity_x);
    player.velocity.y =
        source_units_to_milli(player.source_self_velocity_y + player.source_knockback_velocity_y);
}

fn advance_source_dead_state(
    player: &mut PlayerState,
    player_index: usize,
    stage: StageProfile,
    common_data: MeleeCommonData,
) {
    match player.motion_state {
        MotionState::DeadDown
        | MotionState::DeadLeft
        | MotionState::DeadRight
        | MotionState::DeadUp => {
            if advance_source_dead_timer(player) {
                return;
            }
        }
        MotionState::DeadUpStar | MotionState::DeadUpStarIce => {
            if advance_source_dead_timer(player) {
                return;
            }
            match player.source_dead_phase {
                0 => {
                    player.source_common_timer = common_data.dead_up_star_rise_ticks;
                    player.source_dead_phase = 1;
                    return;
                }
                1 => {
                    player.source_common_timer = common_data.dead_up_star_exit_ticks;
                    player.source_dead_phase = 2;
                    return;
                }
                _ => {}
            }
        }
        MotionState::DeadUpFall
        | MotionState::DeadUpFallHitCamera
        | MotionState::DeadUpFallHitCameraFlat
        | MotionState::DeadUpFallIce
        | MotionState::DeadUpFallHitCameraIce => {
            if advance_source_dead_timer(player) {
                return;
            }
            match player.source_dead_phase {
                0 => {
                    player.source_common_timer = common_data.dead_up_fall_anim_ticks;
                    player.source_dead_phase = 1;
                    return;
                }
                1 => {
                    player.source_common_timer = common_data.dead_up_fall_hit_camera_ticks;
                    player.source_dead_phase = 2;
                    return;
                }
                2 => {
                    player.source_common_timer = common_data.dead_up_fall_drift_ticks;
                    player.source_dead_phase = 3;
                    return;
                }
                3 => {
                    player.source_common_timer = common_data.dead_up_fall_exit_ticks;
                    player.source_dead_phase = 4;
                    return;
                }
                _ => {}
            }
        }
        _ => {}
    }

    let Some(platform) = stage.respawn_platforms.get(player_index).copied() else {
        return;
    };
    player.enter_source_rebirth_state(
        platform,
        common_data.shield_start_health,
        common_data.rebirth_ticks,
    );
    if player.source_common_timer > 0 {
        apply_source_rebirth_physics_step(player);
        player.source_common_timer = player.source_common_timer.saturating_sub(1);
    }
}

fn advance_source_dead_timer(player: &mut PlayerState) -> bool {
    if player.source_common_timer > 0 {
        player.source_common_timer = player.source_common_timer.saturating_sub(1);
        player.motion_frame = player.motion_frame.saturating_add(1);
        player.source_common_timer > 0
    } else {
        false
    }
}

fn advance_source_rebirth_state(
    player: &mut PlayerState,
    input_facts: MeleeInputFacts,
    common_data: MeleeCommonData,
) -> bool {
    let mut entered_rebirth_wait_from_rebirth = false;
    match player.motion_state {
        MotionState::Rebirth => {
            if player.source_common_timer > 0 {
                apply_source_rebirth_physics_step(player);
                player.source_common_timer = player.source_common_timer.saturating_sub(1);
                player.motion_frame = player.motion_frame.saturating_add(1);
                return false;
            }
            player.enter_source_rebirth_wait_state(common_data.rebirth_wait_ticks);
            entered_rebirth_wait_from_rebirth = true;
        }
        MotionState::RebirthWait => {}
        _ => return false,
    }

    if player.motion_state == MotionState::RebirthWait {
        if source_rebirth_wait_fall_input(input_facts) {
            player.apply_source_hurt_intangible_timer(common_data.rebirth_hurt_intangible_ticks);
            enter_fall(player);
            return true;
        }
        if entered_rebirth_wait_from_rebirth {
            return false;
        }
        if player.source_common_timer > 0 {
            player.source_common_timer = player.source_common_timer.saturating_sub(1);
            player.motion_frame = player.motion_frame.saturating_add(1);
            return false;
        }
        player.apply_source_hurt_intangible_timer(common_data.rebirth_hurt_intangible_ticks);
        enter_fall(player);
        return true;
    }
    false
}

fn apply_source_rebirth_physics_step(player: &mut PlayerState) {
    let timer = f32::from(player.source_common_timer.max(1));
    player.source_self_velocity_x =
        (player.source_rebirth_target_x - player.source_position.x) / timer;
    player.source_self_velocity_y =
        (player.source_rebirth_target_y - player.source_position.y) / timer;
    player.source_position.x += player.source_self_velocity_x;
    player.source_position.y += player.source_self_velocity_y;
    player.position = player.source_position.to_milli();
    player.velocity.x = source_units_to_milli(player.source_self_velocity_x);
    player.velocity.y = source_units_to_milli(player.source_self_velocity_y);
}

fn source_rebirth_wait_fall_input(input_facts: MeleeInputFacts) -> bool {
    input_facts.crouch
}

fn tick_source_collision_lifecycle(player: &mut PlayerState) {
    if player.source_hit_intangible_timer > 0 {
        player.source_hit_intangible_timer = player.source_hit_intangible_timer.saturating_sub(1);
        if player.source_hit_intangible_timer == 0 {
            player.source_collision_state = if player.source_hurt_intangible_timer != 0 {
                SOURCE_COLLISION_STATE_HURT_INTANGIBLE
            } else {
                SOURCE_COLLISION_STATE_NORMAL
            };
        }
    }

    if player.source_hurt_intangible_timer > 0 {
        player.source_hurt_intangible_timer = player.source_hurt_intangible_timer.saturating_sub(1);
        if player.source_hurt_intangible_timer == 0 {
            player.source_collision_state = if player.source_hit_intangible_timer != 0 {
                SOURCE_COLLISION_STATE_HIT_AND_HURT_INTANGIBLE
            } else {
                SOURCE_COLLISION_STATE_NORMAL
            };
        }
    }
}

fn tick_source_hurt_collision_lockout(player: &mut PlayerState) {
    if player.source_hurt_collision_lockout_timer == 0 {
        return;
    }
    player.source_hurt_collision_lockout_timer =
        player.source_hurt_collision_lockout_timer.saturating_sub(1);
    if player.source_hurt_collision_lockout_timer == 0 {
        player.source_hurt_collision_state = 0;
    }
}

fn apply_source_damage_on_every_hitlag(
    player: &mut PlayerState,
    input: PlayerInput,
    input_timers: &mut MeleeInputTimers,
    common_data: MeleeCommonData,
) {
    if !player.source_allow_sdi
        || !source_stick_mag_meets_sdi_min(input.stick_x(), input.stick_y(), common_data)
    {
        return;
    }
    if input_timers.x_tap >= common_data.sdi_stick_window
        && input_timers.y_tap >= common_data.sdi_stick_window
    {
        return;
    }

    add_source_position_x(
        player,
        fighter_stick_axis_to_f32(input.stick_x()) * common_data.sdi_pos_scale,
    );
    add_source_position_y(
        player,
        fighter_stick_axis_to_f32(input.stick_y()) * common_data.sdi_pos_scale,
    );
    input_timers.x_tap = EXPIRED_INPUT_TIMER;
    input_timers.y_tap = EXPIRED_INPUT_TIMER;
}

fn apply_source_damage_on_exit_hitlag(
    player: &mut PlayerState,
    input: PlayerInput,
    common_data: MeleeCommonData,
) {
    if player.motion_state == MotionState::GuardSetOff {
        player.source_allow_sdi = false;
        return;
    }
    apply_source_asdi_on_exit_hitlag(player, input, common_data);
    apply_source_di_on_exit_hitlag(player, input, common_data);
    player.source_allow_sdi = false;
}

fn apply_source_asdi_on_exit_hitlag(
    player: &mut PlayerState,
    input: PlayerInput,
    common_data: MeleeCommonData,
) {
    let main_stick = (input.stick_x(), input.stick_y());
    let c_stick = (input.c_stick_x(), input.c_stick_y());
    let stick = if source_stick_mag_meets_sdi_min(c_stick.0, c_stick.1, common_data) {
        c_stick
    } else if source_stick_mag_meets_sdi_min(main_stick.0, main_stick.1, common_data) {
        main_stick
    } else {
        return;
    };

    add_source_position_x(
        player,
        fighter_stick_axis_to_f32(stick.0) * common_data.asdi_pos_scale,
    );
    add_source_position_y(
        player,
        fighter_stick_axis_to_f32(stick.1) * common_data.asdi_pos_scale,
    );
}

fn apply_source_di_on_exit_hitlag(
    player: &mut PlayerState,
    input: PlayerInput,
    common_data: MeleeCommonData,
) {
    let stick_x = fighter_stick_axis_to_f32(input.stick_x());
    let stick_y = fighter_stick_axis_to_f32(input.stick_y());
    if stick_x == 0.0 && stick_y == 0.0 {
        apply_source_trigger_di_knockback_scale(player, input, common_data);
        return;
    }

    let kb_x = player.source_knockback_velocity_x;
    let kb_y = player.source_knockback_velocity_y;
    let kb_vel_x_neg = -kb_x;
    let kb_mag_sq = kb_vel_x_neg * kb_vel_x_neg + kb_y * kb_y;
    if kb_mag_sq >= 0.00001 {
        let f3 = kb_y * stick_x + kb_vel_x_neg * stick_y;
        let mut f30 = f3 * f3 / kb_mag_sq;
        let cross_z = kb_x * stick_y - kb_y * stick_x;
        if cross_z < 0.0 {
            f30 = -f30;
        }
        let kb_mag = (kb_x * kb_x + kb_y * kb_y).sqrt();
        let angle = kb_y.atan2(kb_x) + common_data.di_angle_degrees.to_radians() * f30;
        player.source_knockback_velocity_x = kb_mag * angle.cos();
        player.source_knockback_velocity_y = kb_mag * angle.sin();
    }

    apply_source_trigger_di_knockback_scale(player, input, common_data);
    sync_damage_velocity_projection(player);
}

fn apply_source_trigger_di_knockback_scale(
    player: &mut PlayerState,
    input: PlayerInput,
    common_data: MeleeCommonData,
) {
    if !(input.left_trigger_digital() || input.right_trigger_digital()) {
        return;
    }
    let kb_x = player.source_knockback_velocity_x;
    let kb_y = player.source_knockback_velocity_y;
    if kb_x == 0.0 && kb_y == 0.0 {
        return;
    }
    let kb_angle = kb_y.atan2(kb_x);
    let scaled_kb_mag =
        (kb_x * kb_x + kb_y * kb_y).sqrt() * common_data.trigger_di_knockback_multiplier;
    player.source_knockback_velocity_x = scaled_kb_mag * kb_angle.cos();
    player.source_knockback_velocity_y = scaled_kb_mag * kb_angle.sin();
    sync_damage_velocity_projection(player);
}

fn source_stick_mag_meets_sdi_min(stick_x: i8, stick_y: i8, common_data: MeleeCommonData) -> bool {
    let x = fighter_stick_axis_to_f32(stick_x);
    let y = fighter_stick_axis_to_f32(stick_y);
    x * x + y * y >= common_data.sdi_min_stick_mag * common_data.sdi_min_stick_mag
}

fn tick_source_ledge_cooldown(player: &mut PlayerState) {
    player.source_ledge_cooldown_timer = player.source_ledge_cooldown_timer.saturating_sub(1);
}

fn advance_entry(player: &mut PlayerState, common_data: MeleeCommonData) {
    clear_ground_accels(player);
    player.velocity = Vec2 { x: 0, y: 0 };
    player.grounded = false;
    if player.entry_timer == 0 {
        enter_entry_start(player, common_data);
    } else {
        player.entry_timer -= 1;
    }
}

fn enter_entry_start(player: &mut PlayerState, common_data: MeleeCommonData) {
    player.set_motion_state_alias(MotionState::EntryStart);
    player.motion_frame = 0;
    player.entry_timer = common_data.entry_start_ticks.saturating_sub(1);
    player.entry_base_y = player.position.y;
    player.entry_platform_offset_y = player.profile.entry_platform_offset_y;
    set_source_position_y_milli(
        player,
        entry_position_y(
            player.entry_base_y,
            player.entry_platform_offset_y,
            1,
            common_data.entry_start_ticks,
        ),
    );
    player.velocity = Vec2 { x: 0, y: 0 };
    set_source_self_velocity(player, 0.0, 0.0);
}

fn advance_entry_start(
    player: &mut PlayerState,
    stage: StageProfile,
    common_data: MeleeCommonData,
) {
    clear_ground_accels(player);
    player.velocity = Vec2 { x: 0, y: 0 };
    if player.entry_timer > 0 {
        player.entry_timer -= 1;
    }
    if player.entry_timer == 0 {
        enter_entry_end(player, common_data);
    } else {
        let progress = common_data.entry_start_ticks - player.entry_timer;
        set_source_position_y_milli(
            player,
            entry_position_y(
                player.entry_base_y,
                player.entry_platform_offset_y,
                progress,
                common_data.entry_start_ticks,
            ),
        );
    }
    apply_source_entry_platform_collision(stage, player, common_data);
}

fn enter_entry_end(player: &mut PlayerState, common_data: MeleeCommonData) {
    player.set_motion_state_alias(MotionState::EntryEnd);
    player.motion_frame = 0;
    player.entry_timer = common_data.entry_end_ticks;
    set_source_position_y_milli(player, player.entry_base_y + player.entry_platform_offset_y);
    player.velocity = Vec2 { x: 0, y: 0 };
    set_source_self_velocity(player, 0.0, 0.0);
}

fn advance_entry_end(
    player: &mut PlayerState,
    stage: StageProfile,
    common_data: MeleeCommonData,
) -> bool {
    clear_ground_accels(player);
    player.velocity = Vec2 { x: 0, y: 0 };
    if player.entry_timer > 0 {
        player.entry_timer -= 1;
    }
    if player.entry_timer == 0 {
        source_ft_common_8007d92c_entry_end_handoff(player);
        return player.motion_state != MotionState::Fall;
    } else {
        set_source_position_y_milli(
            player,
            entry_position_y(
                player.entry_base_y,
                player.entry_platform_offset_y,
                player.entry_timer,
                common_data.entry_start_ticks,
            ),
        );
    }
    apply_source_entry_platform_collision(stage, player, common_data);
    true
}

fn source_ft_common_8007d92c_entry_end_handoff(player: &mut PlayerState) {
    if player.grounded {
        source_ft_8008a2bc_entry_grounded_handoff(player);
    } else {
        enter_fall(player);
    }
}

fn source_ft_8008a2bc_entry_grounded_handoff(player: &mut PlayerState) {
    enter_wait_from_airborne_contact(player);
}

fn apply_source_entry_platform_collision(
    stage: StageProfile,
    player: &mut PlayerState,
    common_data: MeleeCommonData,
) {
    let Some(melee_stage) = stage.melee_stage_profile() else {
        return;
    };
    let collision = melee_stage.collision;
    if player.grounded {
        if !source_ft_800846b0_entry_platform_ground_collision(
            stage,
            collision,
            player,
            common_data,
        ) {
            source_ft_common_8007d5d4_ground_to_air(player);
        }
    } else if source_ft_80083e64_entry_platform_air_collision(stage, collision, player, common_data)
    {
        finish_source_floor_contact(player);
    }
}

fn entry_position_y(base_y: i32, offset_y: i32, numerator: u8, denominator: u8) -> i32 {
    if denominator == 0 {
        return base_y;
    }
    let scaled = offset_y as i64 * numerator as i64;
    let denominator = denominator as i64;
    let rounded = if scaled >= 0 {
        (scaled + denominator / 2) / denominator
    } else {
        (scaled - denominator / 2) / denominator
    };
    base_y + rounded as i32
}

fn clear_ground_accels(player: &mut PlayerState) {
    player.ground_accel_x = 0.0;
    player.ground_accel_x2 = 0.0;
}

fn ground_position_delta_x(player: &PlayerState) -> f32 {
    player.ground_velocity_x + player.ground_accel_x
}

fn grounded_position_delta_x(player: &PlayerState) -> f32 {
    match player.motion_state {
        MotionState::SpecialHi => player.source_self_velocity_x,
        _ => ground_position_delta_x(player),
    }
}

fn add_source_position_x(player: &mut PlayerState, delta_x: f32) {
    player.source_position.x += delta_x;
    player.position.x = source_units_to_milli(player.source_position.x);
}

fn add_source_position_y(player: &mut PlayerState, delta_y: f32) {
    player.source_position.y += delta_y;
    player.position.y = source_units_to_milli(player.source_position.y);
}

fn add_source_position_z(player: &mut PlayerState, delta_z: f32) {
    player.source_position_z += delta_z;
}

fn set_source_position_x_milli(player: &mut PlayerState, position_x: i32) {
    player.position.x = position_x;
    player.source_position.x = milli_to_source_units(position_x);
}

fn set_source_position_x_source(player: &mut PlayerState, position_x: f32) {
    player.source_position.x = position_x;
    player.position.x = source_units_to_milli(position_x);
}

fn set_source_position_y_source(player: &mut PlayerState, position_y: f32) {
    player.source_position.y = position_y;
    player.position.y = source_units_to_milli(position_y);
}

fn set_source_position_y_milli(player: &mut PlayerState, position_y: i32) {
    player.position.y = position_y;
    player.source_position.y = milli_to_source_units(position_y);
}

fn source_vec2_from_milli_position(position: Vec2) -> SourceVec2 {
    SourceVec2 {
        x: milli_to_source_units(position.x),
        y: milli_to_source_units(position.y),
    }
}

fn source_ledge_owners(players: &[PlayerState; PLAYER_COUNT]) -> [Option<u16>; PLAYER_COUNT] {
    let mut owners = [None; PLAYER_COUNT];
    for (index, player) in players.iter().enumerate() {
        if player.player_state == PLAYER_STATE_IN_GAME
            && matches!(
                player.motion_state,
                MotionState::CliffCatch | MotionState::CliffWait
            )
        {
            owners[index] = player.source_cliff_ledge_id;
        }
    }
    owners
}

fn try_source_cliff_catch(
    player: &mut PlayerState,
    stage: StageProfile,
    previous_position: Vec2,
    common_data: MeleeCommonData,
    input_facts: MeleeInputFacts,
    player_index: usize,
    source_ledge_owners: &[Option<u16>; PLAYER_COUNT],
) -> bool {
    if player.grounded || input_facts.crouch {
        return false;
    }
    if player.source_ledge_cooldown_timer != 0 {
        return false;
    }
    if player.source_self_velocity_y > 0.0 && player.velocity.y > 0 {
        return false;
    }

    if player.source_coll_env_flags & SOURCE_COLLIDE_LEDGE_GRAB_MASK != 0 {
        let Some(ledge) = source_cliff_collision_ledge_candidate(player, stage) else {
            return false;
        };
        if source_ledge_is_occupied(ledge.index, player_index, source_ledge_owners) {
            return false;
        }
        if enter_source_cliff_catch(player, ledge) {
            return true;
        }
        enter_fall(player);
        return false;
    }

    let Some(contact) = source_ledge_grab_contact_for_facing(
        stage,
        previous_position,
        player.position,
        active_ecb_for_player(player, common_data),
        player.profile.ledge_snap_x_milli,
        player.profile.ledge_snap_y_milli,
        player.profile.ledge_snap_height_milli,
        player.facing,
    )
    .filter(|contact| {
        !source_ledge_is_occupied(contact.ledge.index, player_index, source_ledge_owners)
    }) else {
        return false;
    };

    if enter_source_cliff_catch(player, contact.ledge) {
        true
    } else {
        enter_fall(player);
        false
    }
}

fn source_cliff_collision_ledge_candidate(
    player: &PlayerState,
    stage: StageProfile,
) -> Option<StageLedge> {
    if player.source_coll_env_flags & SOURCE_COLLIDE_LEFT_LEDGE_GRAB != 0 {
        let ledge_id = player.source_coll_ledge_id_left?;
        return stage
            .ledges
            .iter()
            .copied()
            .find(|ledge| ledge.line_index == ledge_id);
    }
    if player.source_coll_env_flags & SOURCE_COLLIDE_RIGHT_LEDGE_GRAB != 0 {
        let ledge_id = player.source_coll_ledge_id_right?;
        return stage
            .ledges
            .iter()
            .copied()
            .find(|ledge| ledge.line_index == ledge_id);
    }
    None
}

fn source_ledge_is_occupied(
    ledge_id: u16,
    player_index: usize,
    source_ledge_owners: &[Option<u16>; PLAYER_COUNT],
) -> bool {
    source_ledge_owners
        .iter()
        .enumerate()
        .any(|(owner_index, owner_ledge)| {
            owner_index != player_index && *owner_ledge == Some(ledge_id)
        })
}

fn ledge_facing(side: StageLedgeSide) -> i8 {
    match side {
        StageLedgeSide::Left => 1,
        StageLedgeSide::Right => -1,
    }
}

fn enter_source_cliff_catch(player: &mut PlayerState, ledge: StageLedge) -> bool {
    let facing = ledge_facing(ledge.side);
    player.set_motion_state_alias(MotionState::CliffCatch);
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.facing = facing;
    player.grounded = false;
    player.fast_falling = false;
    player.velocity = Vec2 { x: 0, y: 0 };
    set_source_self_velocity(player, 0.0, 0.0);
    player.jumps_remaining = player.profile.reusable_air_jumps();
    player.ecb_bottom_lock_timer = 10;
    player.source_coll_x130_locked = true;
    player.source_cliff_ledge_id = Some(ledge.index);
    player.source_cliff_stick_gate = false;
    player.source_cliff_wait_timer = 0;
    let Some((_, transn_position)) = source_cliff_current_transn_position(player) else {
        return false;
    };
    apply_source_cliff_ledge_position(player, ledge, transn_position);
    true
}

fn enter_source_cliff_wait(player: &mut PlayerState, common_data: MeleeCommonData) {
    player.set_motion_state_alias(MotionState::CliffWait);
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.source_cliff_stick_gate = false;
    player.source_cliff_wait_timer =
        if player.damage_percent < common_data.cliff_quick_percent_threshold as f32 {
            common_data.cliff_wait_low_percent_ticks
        } else {
            common_data.cliff_wait_high_percent_ticks
        };
    player.source_hurt_intangible_timer = common_data.cliff_wait_hurt_intangible_ticks;
    player.grounded = false;
    player.fast_falling = false;
    player.velocity = Vec2 { x: 0, y: 0 };
    set_source_self_velocity(player, 0.0, 0.0);
}

fn apply_source_cliff_wait_iasa(
    player: &mut PlayerState,
    input_facts: MeleeInputFacts,
    input_snapshot: MeleeInputSnapshot,
    stick_x: i32,
    stick_y: i8,
    common_data: MeleeCommonData,
) -> bool {
    if source_cliff_attack_input(input_facts, input_snapshot, common_data) {
        enter_source_cliff_option(
            player,
            source_cliff_percent_state(
                player,
                common_data,
                MotionState::CliffAttackQuick,
                MotionState::CliffAttackSlow,
            ),
        );
        return true;
    }

    if input_facts.source_pressed.lr()
        || source_cstick_toward_crossed(input_snapshot, player.facing, common_data)
    {
        enter_source_cliff_option(
            player,
            source_cliff_percent_state(
                player,
                common_data,
                MotionState::CliffEscapeQuick,
                MotionState::CliffEscapeSlow,
            ),
        );
        return true;
    }

    if input_facts.jump_pressed {
        enter_source_cliff_option(
            player,
            source_cliff_percent_state(
                player,
                common_data,
                MotionState::CliffJumpQuick1,
                MotionState::CliffJumpSlow1,
            ),
        );
        return true;
    }

    let stick_y = i32::from(stick_y);
    if source_cliff_stick_exceeds_option_threshold(stick_x, stick_y, common_data) {
        if source_cliff_stick_is_away_or_down(stick_x, stick_y, player.facing, common_data) {
            if player.source_cliff_stick_gate {
                player.source_ledge_cooldown_timer = common_data.ledge_cooldown_ticks;
                player.source_cliff_stick_gate = false;
                player.source_cliff_wait_timer = 0;
                enter_fall(player);
                return true;
            }
            return false;
        }
        if player.source_cliff_stick_gate {
            enter_source_cliff_option(
                player,
                source_cliff_percent_state(
                    player,
                    common_data,
                    MotionState::CliffClimbQuick,
                    MotionState::CliffClimbSlow,
                ),
            );
            return true;
        }
        return false;
    }

    let cstick_x = i32::from(input_snapshot.cstick.0);
    let cstick_y = i32::from(input_snapshot.cstick.1);
    if source_cliff_stick_exceeds_option_threshold(cstick_x, cstick_y, common_data)
        && source_cliff_stick_is_away_or_down(cstick_x, cstick_y, player.facing, common_data)
        && player.source_cliff_stick_gate
    {
        player.source_ledge_cooldown_timer = common_data.ledge_cooldown_ticks;
        player.source_cliff_stick_gate = false;
        player.source_cliff_wait_timer = 0;
        enter_fall(player);
        return true;
    }

    player.source_cliff_stick_gate = true;
    if player.source_cliff_wait_timer == 0 {
        player.source_ledge_cooldown_timer = common_data.ledge_cooldown_ticks;
        player.source_cliff_stick_gate = false;
        enter_fall(player);
        return true;
    }
    false
}

fn source_cliff_attack_input(
    input_facts: MeleeInputFacts,
    input_snapshot: MeleeInputSnapshot,
    common_data: MeleeCommonData,
) -> bool {
    input_facts.source_pressed.a()
        || input_facts.source_pressed.b()
        || source_axis_crossed_positive(
            input_snapshot.prev_cstick.1,
            input_snapshot.cstick.1,
            common_data.c_stick,
        )
}

fn source_cstick_toward_crossed(
    input_snapshot: MeleeInputSnapshot,
    facing: i8,
    common_data: MeleeCommonData,
) -> bool {
    let previous = i16::from(input_snapshot.prev_cstick.0) * i16::from(facing);
    let current = i16::from(input_snapshot.cstick.0) * i16::from(facing);
    let threshold = i16::from(common_data.c_stick.abs());
    previous < threshold && current >= threshold
}

fn source_cliff_percent_state(
    player: &PlayerState,
    common_data: MeleeCommonData,
    quick_state: MotionState,
    slow_state: MotionState,
) -> MotionState {
    if player.damage_percent < common_data.cliff_quick_percent_threshold as f32 {
        quick_state
    } else {
        slow_state
    }
}

fn source_cliff_option_anchors_on_entry(motion_state: MotionState) -> bool {
    matches!(
        motion_state,
        MotionState::CliffClimbSlow
            | MotionState::CliffClimbQuick
            | MotionState::CliffAttackSlow
            | MotionState::CliffAttackQuick
            | MotionState::CliffEscapeSlow
            | MotionState::CliffEscapeQuick
            | MotionState::CliffJumpSlow1
            | MotionState::CliffJumpQuick1
    )
}

fn enter_source_cliff_option(player: &mut PlayerState, motion_state: MotionState) {
    player.set_motion_state_alias(motion_state);
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    sample_source_motion_frame_for_action_entry(player);
    player.source_cliff_stick_gate = false;
    player.source_cliff_wait_timer = 0;
    player.source_hurt_intangible_timer = 32;
    player.grounded = false;
    player.fast_falling = false;
    player.velocity = Vec2 { x: 0, y: 0 };
    set_source_self_velocity(player, 0.0, 0.0);
}

fn enter_source_cliff_jump_2(player: &mut PlayerState) {
    let next_state = if player.motion_state == MotionState::CliffJumpQuick1 {
        MotionState::CliffJumpQuick2
    } else {
        MotionState::CliffJumpSlow2
    };
    player.set_motion_state_alias(next_state);
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    sample_source_motion_frame_for_action_entry(player);
    set_source_self_velocity_x(
        player,
        player.source_self_velocity_x
            + f32::from(player.facing) * player.profile.ledge_jump_horizontal_velocity,
    );
    set_source_self_velocity_y(player, player.profile.ledge_jump_vertical_velocity);
    player.source_cliff_ledge_id = None;
    player.source_cliff_stick_gate = false;
    player.source_cliff_wait_timer = 0;
    player.grounded = false;
}

fn tick_source_cliff_wait_timer(player: &mut PlayerState) {
    player.source_cliff_wait_timer = player.source_cliff_wait_timer.saturating_sub(1);
}

fn source_cliff_stick_exceeds_option_threshold(
    stick_x: i32,
    stick_y: i32,
    common_data: MeleeCommonData,
) -> bool {
    let threshold = i32::from(common_data.cliff_option_stick_threshold);
    stick_x.abs() >= threshold || stick_y.abs() >= threshold
}

fn source_cliff_stick_is_away_or_down(
    stick_x: i32,
    stick_y: i32,
    facing: i8,
    common_data: MeleeCommonData,
) -> bool {
    let angle = (stick_y as f32).atan2(stick_x as f32);
    let threshold = (common_data.aerial_vertical_angle_tan_milli as f32 / 1000.0).atan();
    angle <= -threshold || (angle <= threshold && stick_x * i32::from(facing) < 0)
}

fn apply_source_cliff_physics(
    player: &mut PlayerState,
    stage: StageProfile,
    common_data: MeleeCommonData,
) -> bool {
    if player.grounded {
        apply_source_ft_80084fa8_ground_physics(player, stage, common_data);
        return true;
    }

    let Some(ledge_id) = player.source_cliff_ledge_id else {
        return false;
    };
    let Some(ledge) = stage
        .ledges
        .iter()
        .copied()
        .find(|ledge| ledge.index == ledge_id)
    else {
        return false;
    };

    let Some((source_frame, transn_position)) = source_cliff_current_transn_position(player) else {
        return false;
    };
    apply_source_cliff_ledge_position(player, ledge, transn_position);
    if source_cliff_climb_physics_commits_ground(player.motion_state, transn_position) {
        apply_source_cliff_ground_commit(player, stage, ledge, source_frame);
    } else {
        player.velocity = Vec2 { x: 0, y: 0 };
        set_source_self_velocity(player, 0.0, 0.0);
    }
    true
}

fn source_cliff_current_transn_position(
    player: &PlayerState,
) -> Option<(u8, crate::collision::Vec3)> {
    let Some(frame_count) = source_root_motion_frame_count(player.motion_state) else {
        return None;
    };
    let source_frame = source_cliff_physics_pose_frame(player).min(frame_count);
    let transn_position = source_root_motion_position(player.motion_state, source_frame)?;
    Some((source_frame, transn_position))
}

fn apply_source_cliff_ledge_position(
    player: &mut PlayerState,
    ledge: StageLedge,
    transn_position: crate::collision::Vec3,
) {
    let facing = ledge_facing(ledge.side);
    player.facing = facing;
    set_source_position_x_source(
        player,
        milli_to_source_units(ledge.x_milli)
            + transn_position.z * source_root_motion_model_scale(player) * f32::from(facing),
    );
    set_source_position_y_source(
        player,
        milli_to_source_units(ledge.y_milli)
            + transn_position.y * source_root_motion_model_scale(player),
    );
}

fn source_cliff_climb_physics_commits_ground(
    motion_state: MotionState,
    transn_position: crate::collision::Vec3,
) -> bool {
    matches!(
        motion_state,
        MotionState::CliffClimbSlow
            | MotionState::CliffClimbQuick
            | MotionState::CliffAttackSlow
            | MotionState::CliffAttackQuick
            | MotionState::CliffEscapeSlow
            | MotionState::CliffEscapeQuick
    ) && transn_position.z >= 0.0
        && transn_position.y >= 0.0
}

fn apply_source_cliff_ground_commit(
    player: &mut PlayerState,
    stage: StageProfile,
    ledge: StageLedge,
    source_frame: u8,
) {
    if let Some(melee_stage) = stage.melee_stage_profile() {
        let line_id = usize::from(ledge.line_index);
        player.source_coll_floor_line_index = Some(ledge.line_index);
        player.source_coll_floor_surface_index =
            source_floor_surface_index_for_line(stage, melee_stage.collision, line_id);
    }
    finish_source_floor_contact(player);
    if let Some(delta) = source_root_motion_delta(player.motion_state, source_frame) {
        let velocity_x =
            delta.z * source_root_motion_model_scale(player) * f32::from(player.facing);
        set_source_self_velocity_x(player, velocity_x);
    }
    player.ground_velocity_x = player.source_self_velocity_x;
    clear_ground_accels(player);
}

fn source_cliff_physics_pose_frame(player: &PlayerState) -> u8 {
    match player.motion_state {
        MotionState::CliffCatch => player.motion_frame.saturating_add(2),
        _ => source_root_motion_physics_frame(player),
    }
}

fn commit_ground_velocity(player: &mut PlayerState, stage: StageProfile) {
    if player.motion_state == MotionState::SpecialHi {
        clear_ground_accels(player);
        player.dash_entry_velocity_delta = 0.0;
        player.dash_x0 = 0.0;
        return;
    }

    let committed = player.ground_velocity_x + player.ground_accel_x + player.ground_accel_x2;
    let committed = if player_has_source_root_motion_delta(player) {
        committed
    } else {
        committed.clamp(
            -player.profile.ground_max_horizontal_velocity,
            player.profile.ground_max_horizontal_velocity,
        )
    };
    player.ground_velocity_x = committed;
    let ground_normal = source_ground_normal_for_player(stage, player);
    player.source_self_velocity_x = ground_normal.y * committed;
    player.source_self_velocity_y = -ground_normal.x * committed;
    player.velocity.x = source_units_to_milli(player.source_self_velocity_x);
    player.velocity.y = source_units_to_milli(player.source_self_velocity_y);
    clear_ground_accels(player);
    player.dash_entry_velocity_delta = 0.0;
    player.dash_x0 = 0.0;
}

fn player_has_source_root_motion_delta(player: &PlayerState) -> bool {
    let source_frame = player.motion_frame.saturating_add(1);
    source_root_motion_delta(player.motion_state, source_frame).is_some()
        || player
            .source_action_key
            .and_then(|source_action_key| {
                source_root_motion_delta_for_action_key(source_action_key, source_frame)
            })
            .is_some()
}

fn source_ground_normal_for_player(stage: StageProfile, player: &PlayerState) -> SourcePoint {
    if let Some(melee_stage) = stage.melee_stage_profile() {
        if let Some(line_id) = player.source_coll_floor_line_index.map(usize::from) {
            if matches!(
                source_line_kind(melee_stage.collision, line_id),
                Some(StageCollisionLineKind::Floor | StageCollisionLineKind::SoftFloor)
            ) {
                if let Some(normal) = source_line_normal(melee_stage.collision, line_id) {
                    return source_floor_normal_oriented_up(normal);
                }
            }
        }
    }
    SourcePoint { x: 0.0, y: 1.0 }
}

fn source_floor_normal_oriented_up(normal: SourcePoint) -> SourcePoint {
    if normal.y < 0.0 {
        SourcePoint {
            x: -normal.x,
            y: -normal.y,
        }
    } else {
        normal
    }
}

fn resolve_ground_support_after_move(
    stage: StageProfile,
    player: &mut PlayerState,
    motion_state: MotionState,
    stick_x: i32,
    previous_floor_surface_index: Option<u8>,
    common_data: MeleeCommonData,
    _previous_source_position: SourceVec2,
) -> bool {
    source_fighter_proc_map_begin(player);
    if source_motion_uses_ft_80084280(motion_state) {
        if let Some(melee_stage) = stage.melee_stage_profile() {
            ensure_source_floor_line_index(stage, melee_stage.collision, player);
            return source_ft_80084280_wait_collision(
                stage,
                melee_stage.collision,
                player,
                common_data,
            );
        }
    }

    if source_player_uses_ft_800827a0(player) {
        if let Some(melee_stage) = stage.melee_stage_profile() {
            ensure_source_floor_line_index_if_missing(stage, melee_stage.collision, player);
            let grounded =
                source_ft_800827a0_skip_fall(stage, melee_stage.collision, player, common_data);
            if grounded {
                apply_ft_800827a0_grounded_followups(player);
            }
            return grounded;
        }
    }

    if source_player_uses_ft_80082708(player, motion_state) {
        if let Some(melee_stage) = stage.melee_stage_profile() {
            ensure_source_floor_line_index(stage, melee_stage.collision, player);
            return source_ft_80082708_allow_ground_to_air(
                stage,
                melee_stage.collision,
                player,
                common_data,
            );
        }
    }

    if motion_state == MotionState::Turn
        && player.source_coll_prev_env_flags & SOURCE_COLLIDE_FLOOR_PUSH != 0
    {
        if let Some(melee_stage) = stage.melee_stage_profile() {
            ensure_source_floor_line_index(stage, melee_stage.collision, player);
            return source_ft_80082708_allow_ground_to_air(
                stage,
                melee_stage.collision,
                player,
                common_data,
            );
        }
    }

    if let Some((index, _surface)) = floor_surface_index_for_bottom(stage, player.position) {
        player.source_coll_floor_surface_index = Some(index);
        return true;
    }

    let Some(previous_floor_surface_index) = previous_floor_surface_index else {
        return false;
    };
    let surfaces = stage.collision_surfaces();
    let Some(surface) = surfaces
        .get(usize::from(previous_floor_surface_index))
        .copied()
    else {
        return false;
    };
    if player.position.y != surface.y {
        return false;
    }

    let edge_policy = ground_edge_collision_policy(motion_state);
    if player.position.x <= surface.left_x {
        let keep_grounded = match edge_policy {
            GroundEdgeCollisionPolicy::ClampToFloorEndpoint => true,
            GroundEdgeCollisionPolicy::LedgeSlipWithStickGate => {
                player.facing == -1 && stick_x > -GROUND_EDGE_EXPORTED_STICK_THRESHOLD
            }
        };
        if keep_grounded {
            let bottom_offset = active_ecb_bottom_offset_for_player(player);
            set_source_position_x_milli(player, surface.left_x - bottom_offset.x);
            set_source_position_y_milli(player, surface.y - bottom_offset.y);
            player.source_coll_floor_surface_index = Some(previous_floor_surface_index);
            apply_ground_edge_velocity_stop(player);
            return true;
        }
    }
    if player.position.x >= surface.right_x {
        let keep_grounded = match edge_policy {
            GroundEdgeCollisionPolicy::ClampToFloorEndpoint => true,
            GroundEdgeCollisionPolicy::LedgeSlipWithStickGate => {
                player.facing == 1 && stick_x < GROUND_EDGE_EXPORTED_STICK_THRESHOLD
            }
        };
        if keep_grounded {
            let bottom_offset = active_ecb_bottom_offset_for_player(player);
            set_source_position_x_milli(player, surface.right_x - bottom_offset.x);
            set_source_position_y_milli(player, surface.y - bottom_offset.y);
            player.source_coll_floor_surface_index = Some(previous_floor_surface_index);
            apply_ground_edge_velocity_stop(player);
            return true;
        }
    }

    player.source_coll_floor_surface_index = None;
    player.source_coll_floor_line_index = None;
    false
}

fn ground_support_motion_state(_previous: MotionState, current: MotionState) -> MotionState {
    current
}

fn source_motion_uses_ft_800827a0(motion_state: MotionState) -> bool {
    matches!(
        motion_state,
        MotionState::EscapeF
            | MotionState::EscapeB
            | MotionState::EscapeN
            | MotionState::TurnRun
            | MotionState::CliffClimbSlow
            | MotionState::CliffClimbQuick
            | MotionState::CliffAttackSlow
            | MotionState::CliffAttackQuick
            | MotionState::CliffEscapeSlow
            | MotionState::CliffEscapeQuick
    )
}

fn source_player_uses_ft_800827a0(player: &PlayerState) -> bool {
    source_motion_uses_ft_800827a0(player.motion_state)
        || (player.motion_state == MotionState::GuardSetOff && player.source_allow_sdi)
        || source_passive_state_uses_ft_80084fa8(player)
        || is_source_down_roll_player(player)
}

fn apply_ft_800827a0_grounded_followups(player: &mut PlayerState) {
    if player.motion_state == MotionState::TurnRun
        && player.source_coll_env_flags & (SOURCE_COLLIDE_LEFT_EDGE | SOURCE_COLLIDE_RIGHT_EDGE)
            != 0
    {
        apply_ground_edge_velocity_stop(player);
    }
}

fn source_motion_uses_ft_80084280(motion_state: MotionState) -> bool {
    matches!(
        motion_state,
        MotionState::Wait
            | MotionState::Landing
            | MotionState::LandingAirN
            | MotionState::LandingAirF
            | MotionState::LandingAirB
            | MotionState::LandingAirHi
            | MotionState::LandingAirLw
            | MotionState::LandingFallSpecial
    )
}

fn source_motion_uses_ft_80082708(motion_state: MotionState) -> bool {
    matches!(
        motion_state,
        MotionState::Dash
            | MotionState::KneeBend
            | MotionState::Squat
            | MotionState::SquatWait
            | MotionState::SquatRv
            | MotionState::SpecialHi
            | MotionState::SpecialSStart
            | MotionState::SpecialS
            | MotionState::SpecialLw
    )
}

fn source_player_uses_ft_80082708(player: &PlayerState, motion_state: MotionState) -> bool {
    source_motion_uses_ft_80082708(motion_state)
        || (motion_state == MotionState::GuardSetOff && !player.source_allow_sdi)
        || is_source_down_bound_player(player)
}

fn source_floor_surface_before_ground_move(
    stage: StageProfile,
    player: &PlayerState,
) -> Option<u8> {
    if !player.grounded {
        return None;
    }
    player
        .source_coll_floor_surface_index
        .or_else(|| floor_surface_index_for_bottom(stage, player.position).map(|(index, _)| index))
}

fn source_mp_lib_floor_project(
    collision: StageCollisionProfile,
    mut line_id: usize,
    point: SourcePoint,
) -> Option<(usize, f32)> {
    let mut x = point.x;
    let y = point.y;
    let mut direction = 0_i8;
    loop {
        let line = collision.scaled_line(line_id)?;
        if !source_line_kind_is_floor(line.kind) {
            return None;
        }
        if x < line.x0 {
            if direction != 1 {
                if let Some(prev_id) = source_line_get_prev(collision, line_id) {
                    if source_line_is_floor(collision, prev_id) {
                        line_id = prev_id;
                        direction = -1;
                        continue;
                    }
                }
                if x - line.x0 < -SOURCE_COLL_LINE_TOLERANCE {
                    return None;
                }
                x = line.x0;
                break;
            }
            x = line.x0;
            break;
        }
        if x > line.x1 {
            if direction != -1 {
                if let Some(next_id) = source_line_get_next(collision, line_id) {
                    if source_line_is_floor(collision, next_id) {
                        line_id = next_id;
                        direction = 1;
                        continue;
                    }
                }
                if x - line.x1 > SOURCE_COLL_LINE_TOLERANCE {
                    return None;
                }
                x = line.x1;
                break;
            }
            x = line.x1;
            break;
        }
        break;
    }

    let line = collision.scaled_line(line_id)?;
    let dx = line.x1 - line.x0;
    if dx.abs() <= f32::EPSILON {
        return None;
    }
    let projected_y = (line.y1 - line.y0) * (x - line.x0) / dx + line.y0;
    Some((line_id, projected_y - y + 0.0001))
}

fn ensure_source_floor_line_index(
    stage: StageProfile,
    collision: StageCollisionProfile,
    player: &mut PlayerState,
) {
    if player.source_coll_floor_line_index.is_some_and(|line_id| {
        let line_id = usize::from(line_id);
        source_line_is_floor(collision, line_id)
            && source_floor_line_matches_grounded_position(stage, collision, player, line_id)
    }) {
        return;
    }

    player.source_coll_floor_surface_index = None;
    let Some(line_id) = source_floor_line_for_grounded_position(stage, collision, player) else {
        player.source_coll_floor_line_index = None;
        return;
    };
    player.source_coll_floor_line_index = Some(line_id as u16);
    if player.source_coll_floor_surface_index.is_none() {
        player.source_coll_floor_surface_index =
            source_floor_surface_index_for_line(stage, collision, line_id);
    }
}

fn ensure_source_floor_line_index_if_missing(
    stage: StageProfile,
    collision: StageCollisionProfile,
    player: &mut PlayerState,
) {
    if player
        .source_coll_floor_line_index
        .is_some_and(|line_id| source_line_is_floor(collision, usize::from(line_id)))
    {
        return;
    }

    player.source_coll_floor_surface_index = None;
    let Some(line_id) = source_floor_line_for_grounded_position(stage, collision, player) else {
        player.source_coll_floor_line_index = None;
        return;
    };
    player.source_coll_floor_line_index = Some(line_id as u16);
    if player.source_coll_floor_surface_index.is_none() {
        player.source_coll_floor_surface_index =
            source_floor_surface_index_for_line(stage, collision, line_id);
    }
}

fn source_floor_line_matches_grounded_position(
    stage: StageProfile,
    collision: StageCollisionProfile,
    player: &PlayerState,
    line_id: usize,
) -> bool {
    let Some(line) = collision.scaled_line(line_id) else {
        return false;
    };
    if let Some(surface_index) = player.source_coll_floor_surface_index {
        let Some(target_y) = source_floor_surface_y(stage, surface_index) else {
            return false;
        };
        if !source_position_matches_floor_y(player, target_y) {
            return false;
        }
        return source_floor_surface_index_for_line(stage, collision, line_id)
            == Some(surface_index);
    }

    let target_y = player.source_position.y;
    let left = line.x0.min(line.x1) - SOURCE_COLL_LINE_TOLERANCE;
    let right = line.x0.max(line.x1) + SOURCE_COLL_LINE_TOLERANCE;
    if player.source_position.x < left || player.source_position.x > right {
        return false;
    }
    let dx = line.x1 - line.x0;
    if dx.abs() <= f32::EPSILON {
        return false;
    }
    let projected_y = (line.y1 - line.y0) * (player.source_position.x - line.x0) / dx + line.y0;
    (projected_y - target_y).abs() <= SOURCE_COLL_LINE_TOLERANCE
}

fn source_floor_line_for_grounded_position(
    stage: StageProfile,
    collision: StageCollisionProfile,
    player: &PlayerState,
) -> Option<usize> {
    let target_y = if let Some(surface_index) = player.source_coll_floor_surface_index {
        let target_y = source_floor_surface_y(stage, surface_index)?;
        if !source_position_matches_floor_y(player, target_y) {
            return None;
        }
        target_y
    } else {
        player.source_position.y
    };
    let point = SourcePoint {
        x: player.source_position.x,
        y: target_y,
    };

    let mut best = None;
    source_for_each_floor_line(collision, |line_id, _line| {
        let Some(line) = collision.scaled_line(line_id) else {
            return;
        };
        let left = line.x0.min(line.x1) - SOURCE_COLL_LINE_TOLERANCE;
        let right = line.x0.max(line.x1) + SOURCE_COLL_LINE_TOLERANCE;
        if point.x < left || point.x > right {
            return;
        }
        let dx = line.x1 - line.x0;
        if dx.abs() <= f32::EPSILON {
            return;
        }
        let projected_y = (line.y1 - line.y0) * (point.x - line.x0) / dx + line.y0;
        let distance = (projected_y - point.y).abs();
        if distance > SOURCE_COLL_LINE_TOLERANCE {
            return;
        }
        if best
            .map(|(_, best_distance)| distance < best_distance)
            .unwrap_or(true)
        {
            best = Some((line_id, distance));
        }
    });

    best.map(|(line_id, _)| line_id)
}

fn source_floor_surface_y(stage: StageProfile, surface_index: u8) -> Option<f32> {
    stage
        .collision_surfaces()
        .get(usize::from(surface_index))
        .copied()
        .map(|surface| milli_to_source_units(surface.y))
}

fn source_position_matches_floor_y(player: &PlayerState, floor_y: f32) -> bool {
    (player.source_position.y - floor_y).abs() <= SOURCE_COLL_LINE_TOLERANCE
}

fn source_floor_surface_index_for_line(
    stage: StageProfile,
    collision: StageCollisionProfile,
    line_id: usize,
) -> Option<u8> {
    let line = collision.scaled_line(line_id)?;
    let left = source_units_to_milli(line.x0.min(line.x1));
    let right = source_units_to_milli(line.x0.max(line.x1));
    let y = source_units_to_milli(line.y0);
    stage
        .collision_surfaces()
        .into_iter()
        .enumerate()
        .find(|(_, surface)| surface.y == y && left >= surface.left_x && right <= surface.right_x)
        .map(|(index, _)| index as u8)
}

fn source_line_is_floor(collision: StageCollisionProfile, line_id: usize) -> bool {
    collision
        .lines
        .get(line_id)
        .is_some_and(|line| source_line_kind_is_floor(line.kind))
}

fn source_line_kind_is_floor(kind: StageCollisionLineKind) -> bool {
    matches!(
        kind,
        StageCollisionLineKind::Floor | StageCollisionLineKind::SoftFloor
    )
}

fn active_ecb_bottom_offset_for_player(player: &PlayerState) -> Vec2 {
    Vec2 {
        x: source_units_to_milli(player.source_coll_ecb.bottom.x),
        // mpColl_LoadECB_inline(coll, 5) reaches mpColl_LoadECB_JObj with flags & 1,
        // which forces desired_ecb.bottom.y to 0 before floor-edge clamping.
        y: 0,
    }
}

fn ground_edge_collision_policy(motion_state: MotionState) -> GroundEdgeCollisionPolicy {
    match motion_state {
        MotionState::EscapeF | MotionState::EscapeB | MotionState::EscapeN => {
            GroundEdgeCollisionPolicy::ClampToFloorEndpoint
        }
        _ => GroundEdgeCollisionPolicy::LedgeSlipWithStickGate,
    }
}

fn apply_ground_edge_velocity_stop(player: &mut PlayerState) {
    if player.motion_state == MotionState::TurnRun {
        clear_ground_horizontal_velocity(player);
    }
}

fn clear_ground_horizontal_velocity(player: &mut PlayerState) {
    player.ground_velocity_x = 0.0;
    player.source_self_velocity_x = 0.0;
    player.velocity.x = 0;
    clear_ground_accels(player);
    player.dash_entry_velocity_delta = 0.0;
}

fn set_ground_velocity_x(player: &mut PlayerState, velocity_x: f32) {
    let velocity_x = velocity_x.clamp(
        -player.profile.ground_max_horizontal_velocity,
        player.profile.ground_max_horizontal_velocity,
    );
    player.ground_velocity_x = velocity_x;
    player.source_self_velocity_x = velocity_x;
    player.velocity.x = source_units_to_milli(velocity_x);
    clear_ground_accels(player);
    player.dash_entry_velocity_delta = 0.0;
}

fn stage_ground_velocity_x(player: &mut PlayerState, next_velocity_x: f32) {
    let next_velocity_x = next_velocity_x.clamp(
        -player.profile.ground_max_horizontal_velocity,
        player.profile.ground_max_horizontal_velocity,
    );
    player.ground_accel_x = next_velocity_x - player.ground_velocity_x;
    player.velocity.x = source_units_to_milli(next_velocity_x);
}

fn apply_source_ground_movement(player: &mut PlayerState, stage: StageProfile) {
    let ground_normal = source_ground_normal_for_player(stage, player);
    player.source_self_velocity_x = ground_normal.y * player.ground_velocity_x;
    player.source_self_velocity_y = -ground_normal.x * player.ground_velocity_x;
    player.velocity.y = source_units_to_milli(player.source_self_velocity_y);
}

fn apply_source_root_ground_motion(player: &mut PlayerState) {
    let source_frame = player.motion_frame.saturating_add(1);
    let Some(delta) = source_root_motion_delta(player.motion_state, source_frame) else {
        clear_ground_horizontal_velocity(player);
        return;
    };
    let next_velocity_x = delta.z
        * source_root_motion_model_scale(player)
        * f32::from(player.source_motion_entry_facing);
    player.ground_accel_x = next_velocity_x - player.ground_velocity_x;
    player.velocity.x = source_units_to_milli(next_velocity_x);
}

fn apply_source_action_root_ground_motion(player: &mut PlayerState, facing: i8) -> bool {
    let Some(source_action_key) = player.source_action_key else {
        return false;
    };
    let source_frame = source_root_motion_physics_frame(player);
    let Some(delta) = source_root_motion_delta_for_action_key(source_action_key, source_frame)
    else {
        return false;
    };
    let next_velocity_x = delta.z * source_root_motion_model_scale(player) * f32::from(facing);
    player.ground_accel_x = next_velocity_x - player.ground_velocity_x;
    player.velocity.x = source_units_to_milli(next_velocity_x);
    true
}

fn source_root_motion_physics_frame(player: &PlayerState) -> u8 {
    let sampled_anim_frame = player.cur_anim_frame();
    if sampled_anim_frame > f32::from(player.motion_frame) {
        return (sampled_anim_frame.round() as u8).saturating_add(1);
    }
    player.motion_frame.saturating_add(1)
}

fn apply_source_ft_80084fa8_ground_physics(
    player: &mut PlayerState,
    stage: StageProfile,
    common_data: MeleeCommonData,
) {
    let mut friction = player.profile.ground_friction;
    if player.ground_velocity_x.abs() > player.profile.walk_max_velocity {
        friction *= common_data.high_speed_ground_friction_multiplier;
    }
    if !apply_source_action_root_ground_motion(player, player.facing) {
        apply_source_ground_friction(player, stage, friction);
    }
}

fn source_root_motion_model_scale(player: &PlayerState) -> f32 {
    player.profile.model_scaling
}

fn stage_dash_entry_ground_accel2(player: &mut PlayerState, delta: f32) {
    player.ground_accel_x2 = delta;
    player.dash_entry_velocity_delta = delta;
    let velocity_x = (player.ground_velocity_x + player.ground_accel_x + delta).clamp(
        -player.profile.ground_max_horizontal_velocity,
        player.profile.ground_max_horizontal_velocity,
    );
    player.velocity.x = source_units_to_milli(velocity_x);
}

fn staged_ground_velocity_x(player: &PlayerState) -> f32 {
    (player.ground_velocity_x + player.ground_accel_x + player.ground_accel_x2).clamp(
        -player.profile.ground_max_horizontal_velocity,
        player.profile.ground_max_horizontal_velocity,
    )
}

fn enter_knee_bend(player: &mut PlayerState, jump_input: MeleeJumpInput) {
    clear_shield_turn(player);
    clear_turn_state(player);
    clear_platform_pass_pending(player);
    fighter_change_motion_state(player, MotionState::KneeBend, 0, 0.0, 1.0, 0.0);
    player.jump_input = jump_input;
    player.short_hop = false;
    player.velocity.y = 0;
}

fn enter_knee_bend_from_ground(
    player: &mut PlayerState,
    jump_input: MeleeJumpInput,
    stage: StageProfile,
    common_data: MeleeCommonData,
) {
    enter_knee_bend(player, jump_input);
    apply_ground_traction(player, stage, common_data);
}

fn apply_jump_takeoff_velocity(player: &mut PlayerState, stick_x: i32) {
    let profile = player.profile;
    let carried_velocity =
        staged_ground_velocity_x(player) * profile.ground_to_air_jump_momentum_multiplier;
    let stick_velocity =
        stick_scaled_velocity_source(stick_x, profile.jump_horizontal_initial_velocity);
    let max_jump_velocity = profile.jump_horizontal_max_velocity;
    set_source_self_velocity_x(
        player,
        (carried_velocity + stick_velocity).clamp(-max_jump_velocity, max_jump_velocity),
    );
    clear_ground_accels(player);
}

fn ground_jump_vertical_velocity(player: &PlayerState) -> f32 {
    if player.short_hop {
        player.profile.hop_vertical_initial_velocity
    } else {
        player.profile.jump_vertical_initial_velocity
    }
}

fn enter_walk(
    player: &mut PlayerState,
    motion_state: MotionState,
    stick_x: i32,
    common_data: MeleeCommonData,
) {
    clear_shield_turn(player);
    clear_turn_state(player);
    clear_platform_pass_pending(player);
    player.set_motion_state_alias(motion_state);
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.walk_accel_mul_milli = 1_000;
    player.walk_anim_velocity_x = player.ground_velocity_x;
    player.set_source_motion_anim_rate_milli(1_000);
    apply_walk_velocity(player, stick_x, common_data);
}

fn walk_motion_state(player: &PlayerState, common_data: MeleeCommonData) -> MotionState {
    let walk_velocity = player.ground_velocity_x.abs();
    let walk_speed = player.profile.walk_max_velocity;
    let middle_velocity =
        walk_speed * common_data.walk_middle_velocity_ratio * player.walk_accel_mul_milli as f32
            / 1000.0;
    let fast_velocity =
        walk_speed * common_data.walk_fast_velocity_ratio * player.walk_accel_mul_milli as f32
            / 1000.0;

    if walk_velocity >= fast_velocity {
        MotionState::WalkFast
    } else if walk_velocity >= middle_velocity {
        MotionState::WalkMiddle
    } else {
        MotionState::WalkSlow
    }
}

fn enter_dash(player: &mut PlayerState, direction: i8, started_from_tap: bool) {
    clear_shield_turn(player);
    clear_turn_state(player);
    clear_platform_pass_pending(player);
    clear_motion_script_state(player);
    player.set_motion_state_alias(MotionState::Dash);
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.facing = direction;
    player.source_model_facing = direction;
    player.dash_started_from_tap = started_from_tap;
    let dash_entry_delta =
        source_dash_entry_velocity_delta(player.ground_velocity_x, direction, player.profile);
    player.dash_x0 = dash_entry_delta;
    stage_dash_entry_ground_accel2(player, dash_entry_delta);
}

fn source_dash_entry_velocity_delta(
    current_velocity: f32,
    direction: i8,
    profile: crate::FighterProfile,
) -> f32 {
    let initial_velocity = direction as f32 * profile.dash_initial_velocity;
    if current_velocity * (direction as f32) < 0.0 {
        initial_velocity
    } else {
        initial_velocity - current_velocity
    }
}

fn clear_motion_script_state(player: &mut PlayerState) {
    player.source_allow_interrupt = false;
    player.motion_cmd_var0 = 0;
    player.motion_cmd_var1 = 0;
    player.motion_throw_flags = 0;
    player.source_throw_hitboxes = [None; 2];
    player.source_jab_followup_timer = 0;
    player.source_jab_followup_queued = false;
    player.source_jab_combo_enabled = false;
    player.source_jab_rapid_enabled = false;
    player.source_rapid_jab_input_count = 0;
    player.source_attack100_loop_continue_input = false;
    player.landing_lag_ticks = 0;
    player.dash_x0 = 0.0;
    player.walk_anim_velocity_x = 0.0;
    player.walk_accel_mul_milli = 1_000;
    player.run_brake_x0 = false;
    player.run_brake_frames_remaining = 0;
    player.turn_run_x14 = false;
    player.turn_run_resume_advances = false;
    player.turn_run_completion_pending = false;
    player.turn_run_completion_enters_run = false;
    player.set_source_motion_anim_rate_milli(1_000);
}

fn advance_source_motion_frame(player: &mut PlayerState) {
    if player.frame_speed_mul_milli() > 0 {
        let next_anim_frame = player.cur_anim_frame() + player.frame_speed_mul();
        player.set_source_motion_anim_frame(next_anim_frame);
        player.motion_frame = player.motion_frame.saturating_add(1);
    }
}

fn advance_source_bound_motion_frame(player: &mut PlayerState) {
    if source_binding_for_motion_state(player.motion_state).is_some()
        || player.source_action_key.is_some()
    {
        advance_source_motion_frame(player);
    } else {
        player.motion_frame = player.motion_frame.saturating_add(1);
    }
}

fn sample_source_motion_frame_for_action_entry(player: &mut PlayerState) {
    // ftAction_ChangeAction seeds cur_anim_frame to anim_start - frame_speed,
    // then immediately samples the new action through ftAnim_8006E9B4. Keep
    // motion_frame on the command-script timeline; root motion and frame-0
    // script events are not the same counter as the sampled JObj pose.
    if player.frame_speed_mul_milli() > 0 {
        let next_anim_frame = player.cur_anim_frame() + player.frame_speed_mul();
        player.set_source_motion_anim_frame(next_anim_frame);
    }
}

fn walk_anim_tick(player: &mut PlayerState) {
    advance_source_motion_frame(player);
    update_walk_anim_rate(player);
}

fn wait_anim_tick(player: &mut PlayerState) {
    if player.motion_anim_rate_milli <= 0 {
        player.set_source_motion_anim_rate_milli(1_000);
    }
    advance_source_motion_frame(player);
    let total_frames = player
        .source_action_total_frames
        .max(action_sample_frame_count_for_motion_state(MotionState::Wait).unwrap_or(0));
    if total_frames > 0 && player.motion_frame >= total_frames {
        player.motion_frame = 0;
        player.set_source_motion_anim_frame(0.0);
    }
}

fn update_walk_anim_rate(player: &mut PlayerState) {
    if player.ground_velocity_x * player.facing as f32 <= 0.0 {
        player.set_source_motion_anim_rate_milli(0);
        return;
    }

    let denominator = match player.motion_state {
        MotionState::WalkSlow => player.profile.slow_walk_max_velocity,
        MotionState::WalkMiddle => player.profile.mid_walk_point,
        MotionState::WalkFast => player.profile.fast_walk_min,
        _ => 0.0,
    };
    player.set_source_motion_anim_rate_milli(if denominator > 0.0 {
        (player.ground_velocity_x.abs() * 1000.0 / denominator).round() as i32
    } else {
        0
    });
}

fn dash_anim_tick(player: &mut PlayerState) {
    advance_source_motion_frame(player);
    apply_dash_script_events(player);
    if player.motion_frame
        >= player
            .profile
            .action_frames
            .dash_total_frames
            .saturating_sub(1)
    {
        enter_wait_from_walk(player);
        clear_motion_script_state(player);
        return;
    }
}

fn apply_dash_script_events(player: &mut PlayerState) {
    let frames = player.profile.action_frames;
    if dash_script_event_frame_matches(player.motion_frame, frames.dash_cmd_var0_clear_frame) {
        player.motion_cmd_var0 = 0;
    }
    if dash_script_event_frame_matches(player.motion_frame, frames.dash_cmd_var0_set_frame) {
        player.motion_cmd_var0 = 1;
    }
}

fn apply_attack_air_script_events(player: &mut PlayerState) {
    let Some((set_frame, clear_frame)) =
        attack_air_landing_lag_cmd_var0_frames(player.motion_state, player.profile.action_frames)
    else {
        return;
    };
    if dash_script_event_frame_matches(player.motion_frame, set_frame) {
        player.motion_cmd_var0 = 1;
    }
    if dash_script_event_frame_matches(player.motion_frame, clear_frame) {
        player.motion_cmd_var0 = 0;
    }
}

const SOURCE_THROW_FLAGS_B3: u8 = 1 << 3;
const SOURCE_THROW_FLAGS_B4: u8 = 1 << 4;

fn apply_escape_script_events(player: &mut PlayerState) {
    let frames = player.profile.action_frames;
    let throw_flags_b3_frame = match player.motion_state {
        MotionState::EscapeF => frames.escape_f_throw_flags_b3_frame,
        MotionState::EscapeB => frames.escape_b_throw_flags_b3_frame,
        _ => return,
    };

    if player.motion_frame == throw_flags_b3_frame {
        player.motion_throw_flags |= SOURCE_THROW_FLAGS_B3;
    }
}

fn apply_escape_anim_events(player: &mut PlayerState) {
    if player.motion_throw_flags & SOURCE_THROW_FLAGS_B3 != 0 {
        player.motion_throw_flags &= !SOURCE_THROW_FLAGS_B3;
        player.facing = -player.facing;
        player.source_model_facing = player.facing;
    }
}

fn source_attack100_action_state(action_state_id: MeleeActionStateId) -> bool {
    matches!(
        action_state_id,
        SOURCE_ATTACK100_START_ACTION_STATE_ID
            | SOURCE_ATTACK100_LOOP_ACTION_STATE_ID
            | SOURCE_ATTACK100_END_ACTION_STATE_ID
    )
}

fn source_attack1_family_action_state(action_state_id: MeleeActionStateId) -> bool {
    action_state_id == melee_action_state_id_for_motion_state(MotionState::Attack1)
        || action_state_id == SOURCE_ATTACK12_ACTION_STATE_ID
        || action_state_id == SOURCE_ATTACK13_ACTION_STATE_ID
        || source_attack100_action_state(action_state_id)
}

fn apply_rapid_jab_input(
    player: &mut PlayerState,
    input_facts: MeleeInputFacts,
    source_action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
) -> bool {
    let Some(action_state_id) = player.melee_action_state_id else {
        return false;
    };
    if !source_attack1_family_action_state(action_state_id)
        || source_attack100_action_state(action_state_id)
    {
        return false;
    }

    if input_facts.source_pressed.a() || input_facts.source_released.a() {
        player.source_rapid_jab_input_count = player.source_rapid_jab_input_count.saturating_add(1);
    }

    if player.source_rapid_jab_input_count >= player.profile.action_frames.rapid_jab_window
        && player.source_jab_rapid_enabled
    {
        enter_source_attack100_action(
            player,
            SOURCE_ATTACK100_START_ACTION_STATE_ID,
            SOURCE_ATTACK100_START_ACTION_KEY,
            source_action_total_frames,
        );
        true
    } else {
        false
    }
}

fn apply_attack100_loop_input(player: &mut PlayerState, input_facts: MeleeInputFacts) {
    if player
        .melee_action_state_id
        .is_some_and(|action_state_id| action_state_id == SOURCE_ATTACK100_LOOP_ACTION_STATE_ID)
        && (input_facts.source_pressed.a() || input_facts.source_released.a())
    {
        player.source_attack100_loop_continue_input = true;
    }
}

fn apply_jab_followup_input(player: &mut PlayerState, input_facts: MeleeInputFacts) -> bool {
    let Some(action_state_id) = player.melee_action_state_id else {
        return false;
    };
    let (next_action_state_id, next_action_key, next_total_frames, next_followup_timer) =
        if action_state_id == melee_action_state_id_for_motion_state(MotionState::Attack1) {
            (
                SOURCE_ATTACK12_ACTION_STATE_ID,
                SOURCE_ATTACK12_ACTION_KEY,
                player.profile.action_frames.attack12_total_frames,
                player.profile.action_frames.jab_3_input_window,
            )
        } else if action_state_id == SOURCE_ATTACK12_ACTION_STATE_ID {
            (
                SOURCE_ATTACK13_ACTION_STATE_ID,
                SOURCE_ATTACK13_ACTION_KEY,
                player.profile.action_frames.attack13_total_frames,
                0,
            )
        } else {
            return false;
        };

    if player.source_jab_followup_timer > 0 {
        player.source_jab_followup_timer -= 1;
        if input_facts.source_pressed.a() {
            player.source_jab_followup_queued = true;
        }
    }

    if player.source_jab_followup_queued && player.source_jab_combo_enabled {
        enter_source_jab_followup_action(
            player,
            next_action_state_id,
            next_action_key,
            next_total_frames,
            next_followup_timer,
        );
        true
    } else {
        false
    }
}

fn enter_attack11_jab_state(player: &mut PlayerState) {
    player.source_jab_followup_timer = player.profile.action_frames.jab_2_input_window;
    player.source_jab_followup_queued = false;
    player.source_jab_combo_enabled = false;
    player.source_jab_rapid_enabled = false;
    player.source_rapid_jab_input_count = 0;
    player.source_attack100_loop_continue_input = false;
}

fn enter_source_jab_followup_action(
    player: &mut PlayerState,
    action_state_id: MeleeActionStateId,
    source_action_key: SourceActionKey,
    source_action_total_frames: u8,
    next_followup_timer: u8,
) {
    clear_guard_state(player);
    clear_turn_state(player);
    clear_platform_pass_pending(player);
    player.motion_state = MotionState::Attack1;
    player.motion_state_alias = None;
    player.melee_action_state_id = Some(action_state_id);
    player.source_action_key = Some(source_action_key);
    player.source_action_total_frames = source_action_total_frames;
    player.source_down_bound_pose = None;
    player.source_down_wait_timer = 0.0;
    player.damage_hitstun_frames = 0;
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.set_source_motion_anim_rate_milli(1_000);
    player.source_jab_followup_timer = next_followup_timer;
    player.source_jab_followup_queued = false;
    player.source_jab_combo_enabled = false;
    player.grounded = true;
    player.velocity.y = 0;
    clear_ground_horizontal_velocity(player);
}

fn enter_source_attack100_action(
    player: &mut PlayerState,
    action_state_id: MeleeActionStateId,
    source_action_key: SourceActionKey,
    source_action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
) {
    clear_guard_state(player);
    clear_turn_state(player);
    clear_platform_pass_pending(player);
    player.motion_state = MotionState::Attack1;
    player.motion_state_alias = None;
    player.melee_action_state_id = Some(action_state_id);
    player.source_action_key = Some(source_action_key);
    player.source_action_total_frames = source_action_total_frames(action_state_id).unwrap_or(0);
    player.source_down_bound_pose = None;
    player.source_down_wait_timer = 0.0;
    player.damage_hitstun_frames = 0;
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.set_source_motion_anim_rate_milli(1_000);
    player.motion_throw_flags = 0;
    player.source_jab_followup_timer = 0;
    player.source_jab_followup_queued = false;
    player.source_jab_combo_enabled = false;
    player.source_attack100_loop_continue_input = false;
    player.grounded = true;
    player.velocity.y = 0;
    clear_ground_horizontal_velocity(player);
}

fn advance_attack100_state_if_needed(
    player: &mut PlayerState,
    source_action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
) -> bool {
    let Some(action_state_id) = player.melee_action_state_id else {
        return false;
    };
    match action_state_id {
        SOURCE_ATTACK100_START_ACTION_STATE_ID => {
            if source_action_animation_done(player) {
                enter_source_attack100_action(
                    player,
                    SOURCE_ATTACK100_LOOP_ACTION_STATE_ID,
                    SOURCE_ATTACK100_LOOP_ACTION_KEY,
                    source_action_total_frames,
                );
            }
            true
        }
        SOURCE_ATTACK100_LOOP_ACTION_STATE_ID => {
            if player.source_action_total_frames > 0
                && player.motion_frame >= player.source_action_total_frames
            {
                if player.source_attack100_loop_continue_input {
                    player.motion_frame = 0;
                    player.set_source_motion_anim_frame(0.0);
                    player.source_attack100_loop_continue_input = false;
                } else {
                    enter_source_attack100_action(
                        player,
                        SOURCE_ATTACK100_END_ACTION_STATE_ID,
                        SOURCE_ATTACK100_END_ACTION_KEY,
                        source_action_total_frames,
                    );
                }
            }
            true
        }
        SOURCE_ATTACK100_END_ACTION_STATE_ID => {
            if source_action_animation_done(player) {
                enter_source_common_action_end(player);
            }
            true
        }
        _ => false,
    }
}

fn attack_air_landing_lag_cmd_var0_frames(
    motion_state: MotionState,
    frames: crate::FighterActionFrames,
) -> Option<(u8, u8)> {
    match motion_state {
        MotionState::AttackAirN => Some((
            frames.attack_air_n_landing_lag_set_frame,
            frames.attack_air_n_landing_lag_clear_frame,
        )),
        MotionState::AttackAirF => Some((
            frames.attack_air_f_landing_lag_set_frame,
            frames.attack_air_f_landing_lag_clear_frame,
        )),
        MotionState::AttackAirB => Some((
            frames.attack_air_b_landing_lag_set_frame,
            frames.attack_air_b_landing_lag_clear_frame,
        )),
        MotionState::AttackAirHi => Some((
            frames.attack_air_hi_landing_lag_set_frame,
            frames.attack_air_hi_landing_lag_clear_frame,
        )),
        MotionState::AttackAirLw => Some((
            frames.attack_air_lw_landing_lag_set_frame,
            frames.attack_air_lw_landing_lag_clear_frame,
        )),
        _ => None,
    }
}

fn dash_script_event_frame_matches(motion_frame: u8, event_frame: u8) -> bool {
    if event_frame == 0 {
        motion_frame == 0
    } else {
        motion_frame.saturating_add(1) == event_frame
    }
}

fn landing_anim_tick(player: &mut PlayerState, common_data: MeleeCommonData) -> bool {
    advance_source_motion_frame(player);
    if landing_animation_complete(player, common_data) {
        player.set_motion_state_alias(MotionState::Wait);
        player.motion_frame = 0;
        player.set_source_motion_anim_frame(0.0);
        true
    } else {
        false
    }
}

fn motion_preserves_entry_frame_speed_mul(motion_state: MotionState) -> bool {
    matches!(
        motion_state,
        MotionState::LandingAirN
            | MotionState::LandingAirF
            | MotionState::LandingAirB
            | MotionState::LandingAirHi
            | MotionState::LandingAirLw
            | MotionState::LandingFallSpecial
    )
}

fn landing_animation_complete(player: &PlayerState, common_data: MeleeCommonData) -> bool {
    match player.motion_state {
        MotionState::Landing => action_sample_frame_count_for_motion_state(MotionState::Landing)
            .is_some_and(|frames| player.motion_frame >= frames),
        MotionState::LandingAirN
        | MotionState::LandingAirF
        | MotionState::LandingAirB
        | MotionState::LandingAirHi
        | MotionState::LandingAirLw => {
            let lag_ticks = if player.landing_lag_ticks == 0 {
                landing_air_base_lag_ticks(player.motion_state, player.profile)
            } else {
                player.landing_lag_ticks
            };
            player.motion_frame >= lag_ticks
        }
        MotionState::LandingFallSpecial => {
            player.motion_frame >= landing_fall_special_lag_ticks(player, common_data)
        }
        _ => false,
    }
}

fn run_anim_tick(player: &mut PlayerState) {
    update_run_anim_rate(player);
    advance_source_motion_frame(player);
    if player.run_no_interrupt_frames > 0 {
        player.run_no_interrupt_frames -= 1;
    }
}

fn update_run_anim_rate(player: &mut PlayerState) {
    if player.ground_velocity_x * player.facing as f32 <= 0.0 {
        player.set_source_motion_anim_rate_milli(0);
        return;
    }

    player.set_source_motion_anim_rate_milli(if player.profile.run_animation_scaling > 0.0 {
        (player.ground_velocity_x.abs() * 1000.0 / player.profile.run_animation_scaling).round()
            as i32
    } else {
        0
    });
}

fn apply_run_state_inputs(
    player: &mut PlayerState,
    input_facts: MeleeInputFacts,
    stick_x: i32,
    stage: StageProfile,
    common_data: MeleeCommonData,
) {
    if !player.grounded {
        enter_fall(player);
    } else if let Some(action_state) = run_action_state(input_facts) {
        enter_action_state(player, action_state, stick_x, stage, common_data);
    } else if input_facts.shield_held {
        enter_guard_from_run(player, input_facts, common_data);
        source_update_guard_shield_visual(player, input_facts, common_data);
    } else if input_facts.normal_jump_pressed {
        enter_knee_bend_from_ground(player, input_facts.normal_jump_input, stage, common_data);
    } else if player.run_no_interrupt_frames > 0 {
        if stick_x == 0 {
            apply_run_ground_traction(player, stage, common_data);
        } else {
            apply_run_velocity(player, stick_x, stage, common_data);
        }
    } else if is_same_direction_run(stick_x, player.facing, common_data) {
        apply_run_velocity(player, stick_x, stage, common_data);
    } else if is_opposite_run_turn(stick_x, player.facing, common_data) {
        enter_turn_run(player, stick_x, stage, common_data, 0);
    } else {
        enter_run_brake(player, stage, common_data);
    }
}

fn run_brake_anim_tick(player: &mut PlayerState, common_data: MeleeCommonData) {
    apply_run_brake_script_events(player);
    advance_source_motion_frame(player);
    apply_run_brake_script_events(player);

    if player.motion_cmd_var1 != 0 {
        let gate = common_data.run_brake_animation_pause_velocity;
        if !player.run_brake_x0 {
            if player.ground_velocity_x.abs() >= gate {
                player.set_source_motion_anim_rate_milli(0);
                player.run_brake_x0 = true;
            }
        } else if player.ground_velocity_x.abs() <= gate {
            player.set_source_motion_anim_rate_milli(1_000);
            player.motion_cmd_var1 = 0;
        }
    }

    if player.run_brake_frames_remaining > 0 {
        player.run_brake_frames_remaining -= 1;
    }

    if player.motion_frame >= player.profile.action_frames.run_brake_total_frames
        || player.run_brake_frames_remaining == 0
    {
        enter_wait_from_walk(player);
        clear_motion_script_state(player);
    }
}

fn apply_run_brake_script_events(player: &mut PlayerState) {
    let frames = player.profile.action_frames;
    if player.motion_frame == frames.run_brake_cmd_var0_set_frame {
        player.motion_cmd_var0 = 1;
    }
    if player.motion_frame == frames.run_brake_cmd_var0_clear_frame {
        player.motion_cmd_var0 = 0;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TurnRunAnimOutcome {
    StillTurnRun,
    EnteredRun,
    EnteredWait,
}

fn turn_run_anim_tick(
    player: &mut PlayerState,
    stick_x: i32,
    common_data: MeleeCommonData,
) -> TurnRunAnimOutcome {
    player.turn_just_turned = false;

    if player.turn_run_completion_pending {
        let enters_run = player.turn_run_completion_enters_run;
        player.turn_run_completion_pending = false;
        player.turn_run_completion_enters_run = false;
        if enters_run {
            enter_run_from_turn_run(player, common_data);
            return TurnRunAnimOutcome::EnteredRun;
        }

        player.set_motion_state_alias(MotionState::Wait);
        player.motion_frame = 0;
        player.set_source_motion_anim_frame(0.0);
        clear_turn_state(player);
        clear_motion_script_state(player);
        return TurnRunAnimOutcome::EnteredWait;
    }

    apply_turn_run_script_events(player);
    let resumed_from_pause = update_turn_run_command_pause(player);
    if !resumed_from_pause {
        advance_source_motion_frame(player);
        apply_turn_run_script_events(player);
    }

    if player.motion_state != MotionState::TurnRun {
        return TurnRunAnimOutcome::StillTurnRun;
    }

    if player.motion_frame
        >= player
            .profile
            .action_frames
            .turn_run_total_frames
            .saturating_sub(1)
    {
        player.turn_run_completion_pending = true;
        player.turn_run_completion_enters_run =
            is_same_direction_run(stick_x, player.facing, common_data);
    }

    TurnRunAnimOutcome::StillTurnRun
}

fn apply_turn_run_script_events(player: &mut PlayerState) {
    if player.motion_frame == player.profile.action_frames.turn_run_cmd_var1_frame
        && !player.turn_run_x14
    {
        player.motion_cmd_var1 = 1;
    }
}

fn update_turn_run_command_pause(player: &mut PlayerState) -> bool {
    if player.motion_cmd_var1 == 0 {
        return false;
    }

    if !player.turn_run_x14 {
        player.set_source_motion_anim_rate_milli(0);
        player.turn_run_x14 = true;
        player.turn_run_resume_advances =
            player.ground_velocity_x * player.turn_run_accel_mul as f32 <= 0.01;
        return false;
    }

    if player.ground_velocity_x * player.turn_run_accel_mul as f32 <= 0.01 {
        let advance_on_resume = player.turn_run_resume_advances;
        player.turn_run_resume_advances = false;
        player.set_source_motion_anim_rate_milli(1_000);
        player.motion_cmd_var1 = 0;
        if !player.turn_has_turned {
            player.facing = player.turn_facing_after;
            player.turn_has_turned = true;
            player.turn_just_turned = true;
        }
        return !advance_on_resume;
    }

    false
}

fn enter_run(player: &mut PlayerState) {
    clear_shield_turn(player);
    clear_turn_state(player);
    clear_motion_script_state(player);
    clear_platform_pass_pending(player);
    change_motion_state_alias(player, MotionState::Run);
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.source_model_facing = player.facing;
    player.run_no_interrupt_frames = 0;
}

fn change_motion_state_alias(player: &mut PlayerState, next_motion_state: MotionState) {
    let previous_motion_state = player.motion_state;
    player.set_motion_state_alias(next_motion_state);
    apply_source_motion_change_ground_velocity_bridge(
        player,
        previous_motion_state,
        next_motion_state,
    );
}

fn apply_source_motion_change_ground_velocity_bridge(
    player: &mut PlayerState,
    previous_motion_state: MotionState,
    next_motion_state: MotionState,
) {
    if !source_motion_change_clamps_ground_velocity(previous_motion_state, next_motion_state) {
        return;
    }

    set_ground_velocity_x(
        player,
        player.ground_velocity_x.clamp(
            -player.profile.dash_run_terminal_velocity,
            player.profile.dash_run_terminal_velocity,
        ),
    );
}

fn enter_run_from_turn_run(player: &mut PlayerState, common_data: MeleeCommonData) {
    enter_run(player);
    player.run_no_interrupt_frames = common_data.run_turn_run_no_interrupt_frames;
}

fn enter_run_from_run_direct(player: &mut PlayerState) {
    clear_shield_turn(player);
    clear_turn_state(player);
    clear_motion_script_state(player);
    clear_platform_pass_pending(player);
    player.set_motion_state_alias(MotionState::Run);
    player.set_source_motion_anim_frame(f32::from(player.motion_frame));
    player.run_no_interrupt_frames = 0;
}

fn enter_run_brake(player: &mut PlayerState, stage: StageProfile, common_data: MeleeCommonData) {
    clear_shield_turn(player);
    clear_turn_state(player);
    clear_motion_script_state(player);
    clear_platform_pass_pending(player);
    player.set_motion_state_alias(MotionState::RunBrake);
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.run_brake_frames_remaining = player
        .profile
        .max_run_brake_frames
        .unwrap_or(player.profile.action_frames.run_brake_total_frames);
    apply_run_ground_traction(player, stage, common_data);
}

fn enter_turn_run(
    player: &mut PlayerState,
    stick_x: i32,
    stage: StageProfile,
    common_data: MeleeCommonData,
    anim_start: u8,
) {
    clear_shield_turn(player);
    let accel_mul = player.facing;
    clear_turn_state(player);
    clear_motion_script_state(player);
    clear_platform_pass_pending(player);
    player.set_motion_state_alias(MotionState::TurnRun);
    player.motion_frame = anim_start;
    player.set_source_motion_anim_frame(f32::from(anim_start));
    player.turn_facing_after = -accel_mul;
    player.turn_run_accel_mul = accel_mul;
    player.turn_has_turned = false;
    player.turn_just_turned = false;
    apply_turn_run_velocity(player, stick_x, stage, common_data);
}

fn enter_smash_turn(player: &mut PlayerState, direction: i8) {
    clear_shield_turn(player);
    clear_platform_pass_pending(player);
    let dash_after_direction = player.facing;
    player.set_motion_state_alias(MotionState::Turn);
    player.motion_frame = 0;
    player.turn_facing_after = direction;
    player.turn_has_turned = false;
    player.turn_just_turned = false;
    player.turn_frames_to_turn = 0;
    player.turn_dash_after_direction = dash_after_direction;
    player.turn_latched_buttons = 0;
}

fn enter_standing_turn(player: &mut PlayerState, direction: i8) {
    clear_shield_turn(player);
    clear_platform_pass_pending(player);
    player.set_motion_state_alias(MotionState::Turn);
    player.motion_frame = 0;
    player.turn_facing_after = direction;
    player.turn_has_turned = false;
    player.turn_just_turned = false;
    player.turn_frames_to_turn = player.profile.standing_turn_direction_change_frames;
    player.turn_dash_after_direction = 0;
    player.turn_latched_buttons = 0;
}

fn advance_turn_anim(player: &mut PlayerState) {
    player.motion_frame = player.motion_frame.saturating_add(1);

    if player.turn_frames_to_turn > 0 {
        player.turn_frames_to_turn -= 1;
        return;
    }

    if !player.turn_has_turned {
        player.turn_has_turned = true;
        player.turn_just_turned = true;
        player.facing = player.turn_facing_after;
        player.source_model_facing = player.facing;
    }
}

fn apply_ucf_dashback_turn_hook(
    player: &mut PlayerState,
    dashback_amendment: bool,
    stick_x: i32,
    common_data: MeleeCommonData,
) {
    if !dashback_amendment
        || player.turn_has_turned
        || player.motion_frame != UCF_DASHBACK_SOURCE_TURN_FRAME
        || stick_x * (player.turn_facing_after as i32) < common_data.dash_x as i32
    {
        return;
    }

    player.turn_has_turned = true;
    player.turn_just_turned = true;
    player.turn_frames_to_turn = 0;
    player.facing = player.turn_facing_after;
    player.source_model_facing = player.facing;
}

fn turn_effective_input_facts(
    player: &PlayerState,
    input_facts: MeleeInputFacts,
) -> MeleeInputFacts {
    let mut facts = input_facts;
    if player.turn_just_turned && player.turn_latched_buttons & TURN_LATCH_ATTACK != 0 {
        facts.attack_pressed = true;
        if facts.cstick_smash_direction != (0, 0) {
            facts.smash_attack_direction = facts.cstick_smash_direction;
        } else if facts.horizontal_smash_direction != 0 {
            facts.smash_attack_direction = (facts.horizontal_smash_direction, 0);
            facts.tilt_attack_direction = (0, 0);
            facts.neutral_attack_pressed = false;
        } else if facts.tilt_direction != (0, 0) {
            facts.tilt_attack_direction = facts.tilt_direction;
            facts.neutral_attack_pressed = false;
        } else {
            facts.neutral_attack_pressed = true;
        }
    }

    if player.turn_just_turned && player.turn_latched_buttons & TURN_LATCH_SPECIAL != 0 {
        facts.special_pressed = true;
        if facts.special_direction == (0, 0) {
            facts.special_direction = special_direction_from_turn_facts(facts);
        }
    }

    apply_turn_temporary_facing_to_attack_facts(&mut facts, turn_action_check_facing(player));
    facts
}

fn turn_action_check_facing(player: &PlayerState) -> i8 {
    if player.turn_has_turned {
        player.facing
    } else {
        player.turn_facing_after
    }
}

fn apply_turn_temporary_facing_to_attack_facts(facts: &mut MeleeInputFacts, facing: i8) {
    if !facts.attack_pressed || facts.tilt_attack_direction.0 == 0 {
        return;
    }

    if facts.tilt_attack_direction.0 == facing {
        return;
    }

    facts.tilt_attack_direction = if facts.tilt_direction.1 != 0 {
        (0, facts.tilt_direction.1)
    } else {
        (0, 0)
    };
    facts.neutral_attack_pressed = facts.smash_attack_direction == (0, 0)
        && facts.cstick_smash_direction == (0, 0)
        && facts.tilt_attack_direction == (0, 0);
}

fn special_direction_from_turn_facts(facts: MeleeInputFacts) -> (i8, i8) {
    if facts.tilt_direction.0 != 0 {
        (facts.tilt_direction.0, 0)
    } else if facts.tilt_direction.1 != 0 {
        (0, facts.tilt_direction.1)
    } else {
        (0, 0)
    }
}

fn arm_turn_dash_after_if_fresh(
    player: &mut PlayerState,
    stick_x: i32,
    x_tap_timer: u8,
    common_data: MeleeCommonData,
) {
    if stick_x * player.turn_facing_after as i32 >= common_data.dash_x as i32
        && x_tap_timer < common_data.dash_tap_window
    {
        player.turn_dash_after_direction = player.turn_facing_after;
    }
}

fn apply_turn_run_velocity(
    player: &mut PlayerState,
    stick_x: i32,
    stage: StageProfile,
    common_data: MeleeCommonData,
) {
    let (mut accel, target_velocity) = dash_run_accel_and_target(player.profile, stick_x);
    if target_velocity == 0.0 {
        apply_run_ground_traction(player, stage, common_data);
    } else if player.turn_run_accel_mul as f32 * accel < 0.0 {
        if accel > 0.0 {
            if player.ground_velocity_x + accel > target_velocity {
                accel -= run_ground_friction(player, stage, common_data);
                if player.ground_velocity_x + accel < target_velocity {
                    accel = target_velocity - player.ground_velocity_x;
                }
            }
        } else if player.ground_velocity_x + accel < target_velocity {
            accel += run_ground_friction(player, stage, common_data);
            if player.ground_velocity_x + accel > target_velocity {
                accel = target_velocity - player.ground_velocity_x;
            }
        }
        stage_ground_velocity_x(player, player.ground_velocity_x + accel);
    } else {
        apply_run_ground_traction(player, stage, common_data);
    }
}

fn latch_turn_buttons(player: &mut PlayerState, input_facts: MeleeInputFacts) {
    if input_facts.attack_pressed {
        player.turn_latched_buttons |= TURN_LATCH_ATTACK;
    }
    if input_facts.special_pressed {
        player.turn_latched_buttons |= TURN_LATCH_SPECIAL;
    }
}

fn clear_turn_state(player: &mut PlayerState) {
    player.turn_facing_after = player.facing;
    player.turn_has_turned = false;
    player.turn_just_turned = false;
    player.turn_frames_to_turn = 0;
    player.turn_dash_after_direction = 0;
    player.turn_latched_buttons = 0;
    player.turn_run_accel_mul = player.facing;
    player.turn_run_resume_advances = false;
    player.turn_run_completion_pending = false;
    player.turn_run_completion_enters_run = false;
}

fn clear_platform_pass_pending(player: &mut PlayerState) {
    player.platform_pass_pending = false;
    player.platform_pass_timer = 0;
}

fn advance_squat_frame(
    player: &mut PlayerState,
    stage: StageProfile,
    common_data: MeleeCommonData,
    stick_y: i8,
) {
    player.motion_frame = player.motion_frame.saturating_add(1);
    if player.motion_frame >= squat_completion_frame(player) {
        if crouch_released(stick_y, common_data) {
            enter_squat_rv(player);
        } else {
            enter_squat_wait(player);
        }
    } else {
        apply_ground_traction(player, stage, common_data);
    }
}

fn squat_completion_frame(player: &PlayerState) -> u8 {
    squat_total_frames(player).saturating_sub(1)
}

fn squat_total_frames(player: &PlayerState) -> u8 {
    if player.source_action_total_frames != 0 {
        player.source_action_total_frames
    } else {
        player.profile.action_frames.squat_total_frames
    }
}

fn arm_squat_platform_pass(
    player: &mut PlayerState,
    stage: StageProfile,
    stick_y: i8,
    y_tap_timer: u8,
    common_data: MeleeCommonData,
) -> bool {
    if player.platform_pass_pending
        || !platform_pass_gate(player, stage, stick_y, y_tap_timer, common_data)
    {
        return false;
    }

    player.platform_pass_pending = true;
    player.platform_pass_timer = common_data.platform_drop_delay_ticks;
    true
}

fn advance_squat_platform_pass(
    player: &mut PlayerState,
    stage: StageProfile,
    common_data: MeleeCommonData,
    stick_x: i32,
) -> bool {
    player.platform_pass_timer = player.platform_pass_timer.saturating_sub(1);
    if player.platform_pass_timer == 0 && grounded_on_soft_platform(player, stage) {
        enter_pass_with_same_frame_horizontal_physics(player, stage, common_data, stick_x);
        return true;
    }
    false
}

fn enter_squat(player: &mut PlayerState) {
    clear_shield_turn(player);
    clear_turn_state(player);
    clear_platform_pass_pending(player);
    player.set_motion_state_alias(MotionState::Squat);
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.velocity.y = 0;
}

fn enter_squat_wait(player: &mut PlayerState) {
    clear_shield_turn(player);
    clear_turn_state(player);
    clear_platform_pass_pending(player);
    player.set_motion_state_alias(MotionState::SquatWait);
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.velocity.y = 0;
}

fn enter_squat_rv(player: &mut PlayerState) {
    clear_shield_turn(player);
    clear_turn_state(player);
    clear_platform_pass_pending(player);
    player.set_motion_state_alias(MotionState::SquatRv);
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    clear_ground_horizontal_velocity(player);
    player.velocity.y = 0;
}

fn crouch_released(stick_y: i8, common_data: MeleeCommonData) -> bool {
    stick_y > -common_data.crouch_release_y
}

fn enter_guard(
    player: &mut PlayerState,
    input_facts: MeleeInputFacts,
    common_data: MeleeCommonData,
) {
    enter_guard_on(player, 0, input_facts, common_data);
}

fn enter_guard_from_run(
    player: &mut PlayerState,
    input_facts: MeleeInputFacts,
    common_data: MeleeCommonData,
) {
    enter_guard_on(
        player,
        common_data.guard_on_catch_dash_window,
        input_facts,
        common_data,
    );
}

fn enter_guard_on(
    player: &mut PlayerState,
    catch_dash_window: u8,
    input_facts: MeleeInputFacts,
    common_data: MeleeCommonData,
) {
    clear_turn_state(player);
    clear_platform_pass_pending(player);
    player.set_motion_state_alias(MotionState::GuardOn);
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.velocity.y = 0;
    player.guard_catch_dash_window = catch_dash_window;
    player.guard_release_latched = false;
    source_install_guard_shield(player);
    player.lightshield_amount =
        source_initial_lightshield_amount(input_facts.analog_shield, common_data);
    // GuardOn's animation callback runs on the transition tick and normalizes
    // x650 through ftCo_800925A4 before collision processing.
    player.lightshield_amount = source_lightshield_amount(
        input_facts.analog_shield,
        player.lightshield_amount,
        common_data,
    );
    player.source_shield_aim_angle_degrees = 10.0;
    player.source_shield_aim_magnitude = 0.0;
    player.source_x221c_b3 = false;
    player.source_x221c_b1 = false;
    player.source_x221c_b2 = false;
    player.source_guard_reflect_timer = 0.0;
    player.source_guard_reflect_damage_skip_timer = 0.0;
    source_guard_phys_updates_shield_hit(player);
    clear_shield_turn(player);
}

fn enter_guard_steady(player: &mut PlayerState) {
    clear_turn_state(player);
    clear_platform_pass_pending(player);
    player.set_motion_state_alias(MotionState::Guard);
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.velocity.y = 0;
    player.guard_catch_dash_window = 0;
    player.guard_release_latched = false;
    source_install_guard_shield(player);
    source_guard_phys_updates_shield_hit(player);
}

fn enter_guard_reflect(player: &mut PlayerState, common_data: MeleeCommonData) {
    clear_turn_state(player);
    clear_platform_pass_pending(player);
    player.set_motion_state_alias(MotionState::GuardReflect);
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.velocity.y = 0;
    player.guard_catch_dash_window = 0;
    player.guard_release_latched = false;
    source_clear_guard_shield(player);
    player.source_x221c_b3 = true;
    player.source_x221c_b1 = true;
    player.source_x221c_b2 = true;
    player.source_guard_reflect_timer = common_data.guard_reflect_timer;
    player.source_guard_reflect_damage_skip_timer = common_data.guard_reflect_damage_skip_timer;
    clear_shield_turn(player);
}

fn enter_guard_reflect_from_guard_on(player: &mut PlayerState, common_data: MeleeCommonData) {
    let preserved_anim_frame = player.cur_anim_frame() + player.frame_speed_mul();
    let preserved_motion_frame = player.motion_frame.saturating_add(1);
    enter_guard_reflect(player, common_data);
    player.motion_frame = preserved_motion_frame;
    player.set_source_motion_anim_frame(preserved_anim_frame);
}

fn tick_source_guard_reflect_timers(player: &mut PlayerState) {
    if player.source_x221c_b3 {
        player.source_x221c_b3 = false;
    }
    if player.source_x221c_b1 {
        player.source_guard_reflect_timer -= 1.0;
        if player.source_guard_reflect_timer < 0.0 {
            player.source_x221c_b1 = false;
            source_install_guard_shield(player);
        }
    }
    if player.source_x221c_b2 {
        player.source_guard_reflect_damage_skip_timer -= 1.0;
        if player.source_guard_reflect_damage_skip_timer < 0.0 {
            player.source_x221c_b2 = false;
        }
    }
}

fn guard_startup_total_frames(player: &PlayerState) -> u8 {
    if player.source_action_total_frames != 0 {
        player.source_action_total_frames
    } else {
        player.profile.action_frames.guard_on_total_frames
    }
}

fn clear_guard_state(player: &mut PlayerState) {
    clear_shield_turn(player);
    player.guard_catch_dash_window = 0;
    player.guard_release_latched = false;
}

fn tick_guard_shield_lifecycle(
    player: &mut PlayerState,
    input_facts: MeleeInputFacts,
    common_data: MeleeCommonData,
) -> bool {
    if player.shield_release_lockout_frames > 0 {
        player.shield_release_lockout_frames -= 1;
    }

    if player.source_shield_hit_active {
        if player.motion_frame == 0 {
            player.shield_release_lockout_frames = player
                .shield_release_lockout_frames
                .max(common_data.shield_release_lockout_frames);
        }
        player.lightshield_amount = source_lightshield_amount(
            input_facts.analog_shield,
            player.lightshield_amount,
            common_data,
        );
        let scale = player.lightshield_amount
            * (common_data.shield_hold_lightshield_max - common_data.shield_hold_lightshield_min)
            + common_data.shield_hold_lightshield_min;
        player.shield_health =
            (player.shield_health - common_data.shield_hold_drain * scale).max(0.0);
        if player.shield_health <= 0.0 {
            enter_source_shield_break_fly(player, common_data);
            return true;
        }
    } else if !player.source_shield_collision_active
        && player.shield_health < common_data.shield_start_health
    {
        player.shield_health =
            (player.shield_health + common_data.shield_regen).min(common_data.shield_start_health);
    }
    false
}

fn source_lightshield_amount(
    analog_shield: u8,
    previous: f32,
    common_data: MeleeCommonData,
) -> f32 {
    let input = f32::from(analog_shield) / f32::from(u8::MAX);
    let deadzone = f32::from(common_data.trigger_deadzone) / f32::from(u8::MAX);
    let amount = (input - deadzone) / (1.0 - deadzone);
    if amount < 0.0 {
        previous
    } else {
        amount
    }
}

fn source_initial_lightshield_amount(analog_shield: u8, common_data: MeleeCommonData) -> f32 {
    let input = f32::from(analog_shield) / f32::from(u8::MAX);
    let deadzone = f32::from(common_data.trigger_deadzone) / f32::from(u8::MAX);
    let denom = 1.0 - deadzone;
    if denom <= 0.0 {
        0.0
    } else {
        input / denom
    }
}

fn source_update_guard_shield_visual(
    player: &mut PlayerState,
    input_facts: MeleeInputFacts,
    common_data: MeleeCommonData,
) {
    if !player.source_shield_hit_active {
        return;
    }

    let stick_x = fighter_stick_axis_to_f32(input_facts.lstick.0) * player.facing as f32;
    let stick_y = fighter_stick_axis_to_f32(input_facts.lstick.1);
    let mut stick_radians = stick_y.atan2(stick_x);
    if stick_radians < 0.0 {
        stick_radians += core::f32::consts::TAU;
    }

    let mut stick_degrees = stick_radians.to_degrees();
    if stick_degrees < 0.0 {
        stick_degrees = 0.0;
    }
    if stick_degrees > 359.0 {
        stick_degrees = 359.0;
    }

    let offset = player.source_shield_aim_angle_degrees - 10.0;
    let delta = normalize_source_guard_angle_180(stick_degrees - offset);
    player.source_shield_aim_angle_degrees =
        10.0 + normalize_source_guard_angle_0(delta * common_data.shield_aim_smoothing + offset);

    let raw_stick_x = fighter_stick_axis_to_f32(input_facts.lstick.0);
    let stick_magnitude = (raw_stick_x * raw_stick_x + stick_y * stick_y)
        .sqrt()
        .min(1.0);
    player.source_shield_aim_magnitude = common_data.shield_aim_smoothing
        * (stick_magnitude - player.source_shield_aim_magnitude)
        + player.source_shield_aim_magnitude;
}

fn normalize_source_guard_angle_180(mut degrees: f32) -> f32 {
    if degrees > 180.0 {
        degrees -= 360.0;
    } else if degrees < -180.0 {
        degrees += 360.0;
    }
    degrees
}

fn normalize_source_guard_angle_0(mut degrees: f32) -> f32 {
    if degrees > 360.0 {
        degrees -= 360.0;
    } else if degrees < 0.0 {
        degrees += 360.0;
    }
    degrees
}

fn source_install_guard_shield(player: &mut PlayerState) {
    player.source_shield_hit_active = true;
    player.source_shield_collision_active = true;
    player.source_shield_hit_update_pos = true;
}

fn source_clear_guard_shield(player: &mut PlayerState) {
    player.clear_source_guard_shield_object();
}

fn source_guard_phys_updates_shield_hit(player: &mut PlayerState) {
    if player.source_shield_hit_active {
        player.source_shield_collision_active = true;
        player.source_shield_hit_update_pos = true;
    }
}

fn update_shield_turn(player: &mut PlayerState, input_facts: MeleeInputFacts) {
    let turn_direction = input_facts.turn_direction;
    if turn_direction == 0 || turn_direction == player.facing {
        clear_shield_turn(player);
        return;
    }

    if player.shield_turn_facing_after != turn_direction {
        player.shield_turn_facing_after = turn_direction;
        player.shield_turn_frame = 0;
    }

    player.shield_turn_frame = player.shield_turn_frame.saturating_add(1);
    if player.shield_turn_frame >= SHIELD_TURN_FRAMES {
        player.facing = player.shield_turn_facing_after;
        player.source_model_facing = player.facing;
        clear_shield_turn(player);
    }
}

fn clear_shield_turn(player: &mut PlayerState) {
    player.shield_turn_facing_after = player.facing;
    player.shield_turn_frame = 0;
}

fn enter_guard_off(player: &mut PlayerState) {
    clear_guard_state(player);
    clear_turn_state(player);
    clear_platform_pass_pending(player);
    player.set_motion_state_alias(MotionState::GuardOff);
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.velocity.y = 0;
    source_clear_guard_shield(player);
}

fn enter_pass(player: &mut PlayerState, stage: StageProfile, common_data: MeleeCommonData) {
    clear_guard_state(player);
    clear_turn_state(player);
    clear_platform_pass_pending(player);
    let source_velocity_x = staged_ground_velocity_x(player)
        .clamp(-player.profile.air_drift_max, player.profile.air_drift_max);
    source_ft_common_8007d5d4_ground_to_air(player);
    set_source_self_velocity_x(player, source_velocity_x);
    player.fast_falling = false;
    set_source_self_velocity_y(player, common_data.pass_initial_y_velocity);
    fighter_change_motion_state(player, MotionState::Pass, 0, 0.0, 1.0, 0.0);
    player.source_coll_floor_skip_line_index = player.source_coll_floor_line_index;
    player.floor_skip_surface =
        floor_surface_index_for_bottom(stage, player.position).map(|(index, _)| index);
}

fn enter_pass_with_same_frame_horizontal_physics(
    player: &mut PlayerState,
    stage: StageProfile,
    common_data: MeleeCommonData,
    stick_x: i32,
) {
    enter_pass(player, stage, common_data);
    apply_air_drift(player, stick_x, common_data);
}

fn enter_action_state(
    player: &mut PlayerState,
    motion_state: MotionState,
    stick_x: i32,
    stage: StageProfile,
    common_data: MeleeCommonData,
) {
    clear_guard_state(player);
    clear_turn_state(player);
    clear_platform_pass_pending(player);
    fighter_change_motion_state(player, motion_state, 0, 0.0, 1.0, 0.0);
    if motion_state == MotionState::Attack1 {
        enter_attack11_jab_state(player);
    }
    if motion_state == MotionState::SpecialHi {
        init_falcon_special_hi_source_vars(player);
    }
    if matches!(
        motion_state,
        MotionState::SpecialSStart | MotionState::SpecialS
    ) && stick_x != 0
    {
        player.facing = stick_x.signum() as i8;
        player.source_model_facing = player.facing;
    }
    if player.grounded && motion_state != MotionState::SpecialHi {
        if matches!(motion_state, MotionState::Catch | MotionState::CatchDash) {
            apply_catch_ground_physics(player, stage, common_data);
        } else if matches!(
            motion_state,
            MotionState::SpecialSStart | MotionState::SpecialS
        ) {
            clear_ground_horizontal_velocity(player);
            sample_source_motion_frame_for_action_entry(player);
            apply_source_ft_80084fa8_ground_physics(player, stage, common_data);
        } else {
            clear_ground_horizontal_velocity(player);
        }
    } else if !player.grounded {
        player.velocity.x = 0;
        clear_ground_accels(player);
    }
    player.velocity.y = 0;
    if player.grounded && motion_state == MotionState::AttackDash {
        // doEnter calls ftAnim_8006EBA4 before the same-tick physics proc.
        player.motion_frame = 1;
        player.set_source_motion_anim_frame(1.0);
        apply_source_root_ground_motion(player);
    }
}

fn fighter_change_motion_state(
    player: &mut PlayerState,
    motion_state: MotionState,
    _flags: u32,
    anim_start: f32,
    anim_speed: f32,
    _anim_blend: f32,
) {
    source_clear_guard_shield(player);
    player.set_motion_state_alias(motion_state);
    player.source_coll_floor_skip_line_index = None;
    let anim_start_milli = (anim_start * 1_000.0).round() as i32;
    player.motion_frame = (anim_start_milli / 1_000).clamp(0, u8::MAX as i32) as u8;
    player.set_source_motion_anim_frame_milli(anim_start_milli);
    player.set_source_motion_anim_rate_milli((anim_speed * 1_000.0).round() as i32);
}

fn enter_dash_iasa_action_state(
    player: &mut PlayerState,
    motion_state: MotionState,
    stick_x: i32,
    stage: StageProfile,
    common_data: MeleeCommonData,
) {
    let ground_velocity = staged_ground_velocity_x(player);
    if motion_state == MotionState::GuardReflect {
        enter_guard_reflect(player, common_data);
    } else if matches!(motion_state, MotionState::EscapeF | MotionState::EscapeB) {
        enter_ground_escape(player, motion_state);
        player.motion_frame = 1;
        player.set_source_motion_anim_frame(1.0);
    } else {
        enter_action_state(player, motion_state, stick_x, stage, common_data);
    }
    if dash_iasa_action_applies_x54_decay(motion_state) {
        set_ground_velocity_x(
            player,
            dash_iasa_decayed_ground_velocity(ground_velocity, common_data),
        );
    }
    if motion_state == MotionState::GuardReflect {
        apply_ground_traction(player, stage, common_data);
    } else if matches!(motion_state, MotionState::EscapeF | MotionState::EscapeB) {
        // Dash IASA applies x54 after changing state; the later Escape physics
        // callback then replaces that velocity with frame-1 root movement.
        apply_source_root_ground_motion(player);
    }
}

fn dash_iasa_action_applies_x54_decay(motion_state: MotionState) -> bool {
    matches!(
        motion_state,
        MotionState::AttackS4 | MotionState::EscapeF | MotionState::GuardReflect
    )
}

fn grounded_action_total_frames(player: &PlayerState) -> u8 {
    if let Some(binding) = source_special_action_binding_for_motion_state(player.motion_state) {
        return binding.total_frames;
    }

    match player.motion_state {
        MotionState::EscapeN => player.profile.action_frames.escape_n_total_frames,
        MotionState::EscapeF => player.profile.action_frames.escape_f_total_frames,
        MotionState::EscapeB => player.profile.action_frames.escape_b_total_frames,
        MotionState::Catch => FALCON_CATCH_FRAMES,
        MotionState::CatchDash => FALCON_CATCH_DASH_FRAMES,
        MotionState::Attack1 => {
            if player
                .melee_action_state_id
                .is_some_and(source_attack100_action_state)
            {
                player.source_action_total_frames
            } else if player
                .melee_action_state_id
                .is_some_and(|action_state_id| action_state_id == SOURCE_ATTACK12_ACTION_STATE_ID)
            {
                player.profile.action_frames.attack12_total_frames
            } else if player
                .melee_action_state_id
                .is_some_and(|action_state_id| action_state_id == SOURCE_ATTACK13_ACTION_STATE_ID)
            {
                player.profile.action_frames.attack13_total_frames
            } else {
                player.profile.action_frames.attack1_total_frames
            }
        }
        MotionState::AttackDash => player.profile.action_frames.attack_dash_total_frames,
        MotionState::AttackS3 => FALCON_ATTACK_S3_FRAMES,
        MotionState::AttackHi3 => FALCON_ATTACK_HI3_FRAMES,
        MotionState::AttackLw3 => FALCON_ATTACK_LW3_FRAMES,
        MotionState::AttackS4 => FALCON_ATTACK_S4_FRAMES,
        MotionState::AttackHi4 => FALCON_ATTACK_HI4_FRAMES,
        MotionState::AttackLw4 => FALCON_ATTACK_LW4_FRAMES,
        _ => 0,
    }
}

fn grounded_action_iasa_state(
    player: &PlayerState,
    input_facts: MeleeInputFacts,
    common_data: MeleeCommonData,
) -> Option<MotionState> {
    if player.motion_frame < grounded_action_iasa_frame(player)? {
        return None;
    }

    wait_action_state(input_facts)
        .or_else(|| input_facts.shield_held.then_some(MotionState::Guard))
        .or_else(|| {
            input_facts
                .normal_jump_pressed
                .then_some(MotionState::KneeBend)
        })
        .or_else(|| {
            (input_facts.forward_dash_direction(player.facing) != 0).then_some(MotionState::Dash)
        })
        .or_else(|| input_facts.crouch.then_some(MotionState::Squat))
        .or_else(|| {
            (input_facts.smash_turn_direction(player.facing) != 0).then_some(MotionState::Turn)
        })
        .or_else(|| {
            (input_facts.standing_turn_direction(player.facing) != 0).then_some(MotionState::Turn)
        })
        .or_else(|| {
            (input_facts.walk_direction != 0).then_some(walk_motion_state(player, common_data))
        })
}

fn grounded_action_iasa_frame(player: &PlayerState) -> Option<u8> {
    match player.motion_state {
        MotionState::SpecialN => Some(FALCON_SPECIAL_N_IASA),
        MotionState::Attack1 => {
            if player
                .melee_action_state_id
                .is_some_and(source_attack100_action_state)
            {
                None
            } else if player
                .melee_action_state_id
                .is_some_and(|action_state_id| action_state_id == SOURCE_ATTACK12_ACTION_STATE_ID)
            {
                Some(player.profile.action_frames.attack12_iasa_frame)
            } else if player
                .melee_action_state_id
                .is_some_and(|action_state_id| action_state_id == SOURCE_ATTACK13_ACTION_STATE_ID)
            {
                Some(player.profile.action_frames.attack13_iasa_frame)
            } else {
                Some(player.profile.action_frames.attack1_iasa_frame)
            }
        }
        MotionState::AttackDash => Some(player.profile.action_frames.attack_dash_iasa_frame),
        MotionState::AttackHi3 => Some(FALCON_ATTACK_HI3_IASA),
        MotionState::AttackLw3 => Some(FALCON_ATTACK_LW3_IASA),
        MotionState::AttackS4 => Some(FALCON_ATTACK_S4_IASA),
        MotionState::AttackHi4 => Some(FALCON_ATTACK_HI4_IASA),
        MotionState::AttackLw4 => Some(FALCON_ATTACK_LW4_IASA),
        _ => None,
    }
}

fn enter_iasa_state(
    player: &mut PlayerState,
    motion_state: MotionState,
    input_facts: MeleeInputFacts,
    jump_input: MeleeJumpInput,
    stick_x: i32,
    stage: StageProfile,
    common_data: MeleeCommonData,
) {
    match motion_state {
        MotionState::Guard => {
            enter_guard(player, input_facts, common_data);
        }
        MotionState::GuardOff => enter_guard_off(player),
        MotionState::Pass => {
            enter_pass_with_same_frame_horizontal_physics(player, stage, common_data, stick_x)
        }
        MotionState::GuardReflect => {
            enter_guard_reflect(player, common_data);
            apply_ground_traction(player, stage, common_data);
        }
        MotionState::KneeBend => {
            enter_knee_bend_from_ground(player, jump_input, stage, common_data);
        }
        MotionState::Dash => enter_dash(player, player.facing, true),
        MotionState::Squat => enter_squat(player),
        MotionState::Turn => enter_smash_turn(player, -player.facing),
        MotionState::WalkSlow | MotionState::WalkMiddle | MotionState::WalkFast => {
            enter_walk(player, motion_state, stick_x, common_data);
        }
        MotionState::EscapeN => {
            enter_ground_escape(player, motion_state);
        }
        MotionState::EscapeF | MotionState::EscapeB => {
            enter_ground_escape(player, motion_state);
            player.motion_frame = 1;
            player.set_source_motion_anim_frame(1.0);
            apply_source_root_ground_motion(player);
        }
        _ => enter_action_state(player, motion_state, stick_x, stage, common_data),
    }
}

fn enter_escape_air(
    player: &mut PlayerState,
    stick_x: i32,
    stick_y: i8,
    common_data: MeleeCommonData,
) {
    if player.ecb_bottom_lock_timer == 0 {
        player.ecb_bottom_offset_y = SOURCE_JOBJ_ECB_BOTTOM_OFFSET_Y;
    }
    clear_motion_script_state(player);
    fighter_change_motion_state(player, MotionState::EscapeAir, 0, 0.0, 1.0, 0.0);
    sample_source_motion_frame_for_action_entry(player);
    player.escape_air_iasa_timer = common_data.escapeair_iasa_timer_ticks;
    player.fast_falling = false;
    let cleaned_stick_x =
        fighter_input_cleanup_stick_axis(stick_x, common_data.main_stick_deadzone_x);
    let cleaned_stick_y =
        fighter_input_cleanup_stick_axis(stick_y as i32, common_data.main_stick_deadzone_y);
    let (velocity_x, velocity_y) =
        escape_air_velocity(cleaned_stick_x, cleaned_stick_y, common_data);
    set_source_self_velocity(player, velocity_x, velocity_y);
    clear_ground_accels(player);
}

fn airborne_platform_landing_callback_rejects_soft_platform(
    motion_state: MotionState,
    stick_y: i8,
    common_data: MeleeCommonData,
) -> bool {
    matches!(
        motion_state,
        MotionState::Fall
            | MotionState::FallF
            | MotionState::FallB
            | MotionState::FallAerial
            | MotionState::FallAerialF
            | MotionState::FallAerialB
            | MotionState::DamageFall
            | MotionState::JumpF
            | MotionState::JumpB
            | MotionState::JumpAerialF
            | MotionState::JumpAerialB
            | MotionState::FallSpecial
            | MotionState::FallSpecialF
            | MotionState::FallSpecialB
            | MotionState::CliffJumpSlow2
            | MotionState::CliffJumpQuick2
    ) && stick_y <= common_data.fallspecial_platform_landing_y
}

fn enter_fall_special(player: &mut PlayerState) {
    player.set_motion_state_alias(MotionState::FallSpecial);
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    reset_source_fall_anim(player, MotionState::FallSpecial);
    player.escape_air_iasa_timer = 0;
}

fn enter_falcon_special_hi_fall_special(player: &mut PlayerState) {
    let was_grounded = player.grounded;
    enter_fall_special(player);
    player.landing_lag_ticks = player.profile.captain_special_attrs.specialhi_landing_lag as u8;
    player.jumps_remaining = 0;
    if was_grounded {
        player.grounded = false;
        player.ground_velocity_x = 0.0;
        lock_ecb_bottom_for_frames(player, AIRBORNE_STATE_TWO_ECB_LOCK_FRAMES);
        clear_ground_accels(player);
    }
}

fn enter_fall(player: &mut PlayerState) {
    let was_grounded = player.grounded;
    player.set_motion_state_alias(MotionState::Fall);
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.set_source_motion_anim_rate_milli(1_000);
    reset_source_fall_anim(player, MotionState::Fall);
    player.grounded = false;
    if was_grounded {
        lock_ground_to_air_ecb_bottom(player);
        set_source_self_velocity_x(
            player,
            staged_ground_velocity_x(player)
                .clamp(-player.profile.air_drift_max, player.profile.air_drift_max),
        );
    } else {
        set_source_self_velocity_x(
            player,
            player
                .source_self_velocity_x
                .clamp(-player.profile.air_drift_max, player.profile.air_drift_max),
        );
    }
    set_source_self_velocity_y(player, player.source_self_velocity_y);
    player.ground_velocity_x = 0.0;
    player.escape_air_iasa_timer = 0;
    player.source_cliff_ledge_id = None;
    player.source_cliff_stick_gate = false;
    player.source_cliff_wait_timer = 0;
    clear_ground_accels(player);
}

fn source_ft_common_8007d5d4_ground_to_air(player: &mut PlayerState) {
    player.grounded = false;
    player.ground_velocity_x = 0.0;
    player.jumps_remaining = player.profile.max_jumps.saturating_sub(1);
    lock_ground_to_air_ecb_bottom(player);
    clear_ground_accels(player);
}

fn enter_fall_aerial(player: &mut PlayerState) {
    player.set_motion_state_alias(MotionState::FallAerial);
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    reset_source_fall_anim(player, MotionState::FallAerial);
    player.grounded = false;
    player.escape_air_iasa_timer = 0;
    player.fast_falling = false;
}

fn reset_source_fall_anim(player: &mut PlayerState, neutral_pose: MotionState) {
    player.source_fall_anim_pose = neutral_pose;
    player.source_fall_anim_blend = 0.0;
}

fn update_source_fall_anim_inner(player: &mut PlayerState, common_data: MeleeCommonData) {
    let Some((neutral, forward, backward)) = source_fall_anim_pose_set(player.motion_state) else {
        return;
    };
    let air_drift_max = player.profile.air_drift_max;
    if air_drift_max == 0.0 {
        player.source_fall_anim_pose = neutral;
        player.source_fall_anim_blend = 0.0;
        return;
    }

    let mut air_drift_frac = player.source_self_velocity_x / air_drift_max;
    air_drift_frac = air_drift_frac.clamp(-1.0, 1.0);
    let air_drift_frac_abs = air_drift_frac.abs();
    let (target_pose, target_blend) =
        if air_drift_frac_abs > common_data.fall_animation_drift_threshold {
            let pose = if air_drift_frac * f32::from(player.facing) > 0.0 {
                forward
            } else {
                backward
            };
            let denom = 1.0 - common_data.fall_animation_drift_threshold;
            let blend = if denom.abs() <= f32::EPSILON {
                1.0
            } else {
                (air_drift_frac_abs - common_data.fall_animation_drift_threshold) / denom
            };
            (pose, blend)
        } else {
            (neutral, 0.0)
        };

    player.source_fall_anim_blend +=
        common_data.fall_animation_blend * (target_blend - player.source_fall_anim_blend);
    if player.source_fall_anim_blend != 0.0 && target_pose != player.source_fall_anim_pose {
        player.source_fall_anim_pose = target_pose;
    }
}

fn source_fall_anim_pose_set(
    motion_state: MotionState,
) -> Option<(MotionState, MotionState, MotionState)> {
    match motion_state {
        MotionState::Fall | MotionState::FallF | MotionState::FallB => {
            Some((MotionState::Fall, MotionState::FallF, MotionState::FallB))
        }
        MotionState::FallAerial | MotionState::FallAerialF | MotionState::FallAerialB => Some((
            MotionState::FallAerial,
            MotionState::FallAerialF,
            MotionState::FallAerialB,
        )),
        MotionState::FallSpecial | MotionState::FallSpecialF | MotionState::FallSpecialB => Some((
            MotionState::FallSpecial,
            MotionState::FallSpecialF,
            MotionState::FallSpecialB,
        )),
        _ => None,
    }
}

fn enter_landing_fall_special(player: &mut PlayerState, landing_lag_ticks: u8) {
    let landing_velocity_x = source_ft_common_8007d6a4_ground_velocity_x(player);
    clear_shield_turn(player);
    clear_turn_state(player);
    player.set_motion_state_alias(MotionState::LandingFallSpecial);
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.escape_air_iasa_timer = 0;
    player.motion_cmd_var0 = 0;
    player.motion_cmd_var1 = 0;
    player.landing_lag_ticks = landing_lag_ticks;
    player.set_source_motion_anim_rate_milli(landing_animation_rate_milli(
        MotionState::LandingFallSpecial,
        landing_lag_ticks,
    ));
    player.grounded = true;
    player.fast_falling = false;
    set_ground_velocity_x(player, landing_velocity_x);
    sync_source_ground_collision_root(player);
}

fn source_ft_common_8007d6a4_ground_velocity_x(player: &PlayerState) -> f32 {
    let transn_velocity_x = match player.motion_state {
        MotionState::SpecialHi | MotionState::SpecialAirHi => {
            Some(source_special_hi_transn_self_velocity(player).0)
        }
        _ => {
            let source_frame = player.motion_frame.saturating_add(1);
            source_root_motion_delta(player.motion_state, source_frame).map(|delta| {
                delta.z
                    * source_root_motion_model_scale(player)
                    * f32::from(player.source_motion_entry_facing)
            })
        }
    };

    transn_velocity_x.unwrap_or(player.source_self_velocity_x)
}

fn landing_animation_rate_milli(motion_state: MotionState, landing_lag_ticks: u8) -> i32 {
    let Some(sample_count) = action_sample_frame_count_for_motion_state(motion_state) else {
        return 1_000;
    };
    if landing_lag_ticks == 0 {
        return 1_000;
    }
    let action_frames_tenths = i32::from(sample_count).saturating_mul(10).saturating_add(1);
    (action_frames_tenths.saturating_mul(100) / i32::from(landing_lag_ticks)).max(1)
}

fn airborne_landing_contact(
    stage: StageProfile,
    player: &PlayerState,
    previous_bottom: Vec2,
    floor_skip_surface: Option<u8>,
    drop_through_soft_platforms: bool,
    common_data: MeleeCommonData,
) -> Option<crate::StageLandingContact> {
    let current_bottom = ecb_bottom_world_position_for_motion_frame(
        player,
        player.position,
        collision_ecb_motion_frame(player),
        common_data,
    );
    if current_bottom.y >= previous_bottom.y {
        return None;
    }
    let tolerance = source_units_to_milli(SOURCE_COLL_LINE_TOLERANCE);
    let mut best = None;
    for (surface_index, surface) in stage.collision_surfaces().into_iter().enumerate() {
        if floor_skip_surface == Some(surface_index as u8) {
            continue;
        }
        if drop_through_soft_platforms && surface.kind == StageSurfaceKind::Soft {
            continue;
        }
        if current_bottom.x < surface.left_x || current_bottom.x > surface.right_x {
            continue;
        }
        if previous_bottom.y >= surface.y.saturating_sub(tolerance)
            && current_bottom.y <= surface.y.saturating_sub(tolerance)
        {
            let contact = crate::StageLandingContact {
                surface,
                y: surface.y,
            };
            if best
                .map(|existing: crate::StageLandingContact| contact.y > existing.y)
                .unwrap_or(true)
            {
                best = Some(contact);
            }
        }
    }
    best
}

fn apply_source_air_map_wall_collision(
    stage: StageProfile,
    player: &mut PlayerState,
    _previous_position: Vec2,
    _previous_source_position: SourceVec2,
    collision_motion_state: MotionState,
    stick_y: i8,
    common_data: MeleeCommonData,
) -> bool {
    let Some(melee_stage) = stage.melee_stage_profile() else {
        return false;
    };
    let collision = melee_stage.collision;
    source_mp_coll_air_wrapper_begin(player);
    let flags = source_air_map_collision_flags(player, collision_motion_state);
    player.source_coll_facing_dir = if flags & SOURCE_COLLISION_FLAG_AIR_CAN_GRAB_LEDGE != 0 {
        source_air_map_collision_facing_dir(player, collision_motion_state)
    } else {
        0
    };
    source_mp_coll_prev(player);
    source_mp_coll_load_ecb_inline(player, common_data, 6);
    let platform_pass_rejects_soft = flags & SOURCE_COLLISION_FLAG_AIR_PLATFORM_PASS_CALLBACK != 0
        && airborne_platform_landing_callback_rejects_soft_platform(
            collision_motion_state,
            stick_y,
            common_data,
        );
    let touched_floor =
        source_mp_coll_air_inline1(stage, collision, player, flags, platform_pass_rejects_soft);
    set_source_position_x_source(player, player.source_coll_cur_pos.x);
    set_source_position_y_source(player, player.source_coll_cur_pos.y);
    touched_floor
}

fn source_air_map_collision_flags(player: &PlayerState, motion_state: MotionState) -> u32 {
    if matches!(
        motion_state,
        MotionState::CliffJumpSlow2 | MotionState::CliffJumpQuick2
    ) {
        if player.source_ledge_cooldown_timer != 0 {
            SOURCE_COLLISION_FLAG_AIR_PLATFORM_PASS_CALLBACK
        } else {
            SOURCE_COLLISION_FLAG_AIR_PLATFORM_PASS_CALLBACK
                | SOURCE_COLLISION_FLAG_AIR_CAN_GRAB_LEDGE
        }
    } else if source_air_collision_uses_fall_wrapper(motion_state) {
        if player.source_ledge_cooldown_timer != 0 {
            SOURCE_COLLISION_FLAG_AIR_PLATFORM_PASS_CALLBACK
        } else {
            SOURCE_COLLISION_FLAG_AIR_PLATFORM_PASS_CALLBACK
                | SOURCE_COLLISION_FLAG_AIR_CAN_GRAB_LEDGE
        }
    } else if source_air_collision_uses_special_hi_ledge_wrapper(player, motion_state) {
        SOURCE_COLLISION_FLAG_AIR_CAN_GRAB_LEDGE
    } else {
        0
    }
}

fn source_air_map_collision_facing_dir(player: &PlayerState, motion_state: MotionState) -> i8 {
    if source_air_collision_uses_special_hi_ledge_wrapper(player, motion_state) {
        0
    } else if player.facing < 0 {
        -1
    } else {
        1
    }
}

fn source_air_collision_uses_special_hi_ledge_wrapper(
    player: &PlayerState,
    motion_state: MotionState,
) -> bool {
    matches!(
        motion_state,
        MotionState::SpecialHi | MotionState::SpecialAirHi
    ) && player.captain_special_hi_x2_b1
        && player.source_ledge_cooldown_timer == 0
}

fn apply_captain_special_hi_air_floor_collision(player: &mut PlayerState) {
    finish_source_floor_contact(player);
    if player.captain_special_hi_x2_b1 {
        let landing_lag = player.profile.captain_special_attrs.specialhi_landing_lag as u8;
        enter_landing_fall_special(player, landing_lag);
    } else {
        enter_wait_from_airborne_contact(player);
    }
}

fn apply_source_air_floor_collision_callback(
    player: &mut PlayerState,
    collision_motion_state: MotionState,
    trigger_timer: u8,
    common_data: MeleeCommonData,
) {
    if matches!(
        collision_motion_state,
        MotionState::ShieldBreakFly | MotionState::ShieldBreakFall
    ) {
        finish_source_floor_contact(player);
        enter_source_shield_break_down(player);
        return;
    }

    if matches!(
        collision_motion_state,
        MotionState::SpecialHi | MotionState::SpecialAirHi
    ) {
        apply_captain_special_hi_air_floor_collision(player);
        return;
    }

    finish_source_floor_contact(player);
    if matches!(
        collision_motion_state,
        MotionState::EscapeAir
            | MotionState::FallSpecial
            | MotionState::FallSpecialF
            | MotionState::FallSpecialB
    ) {
        enter_landing_fall_special(player, common_data.escapeair_landing_lag_ticks);
    } else {
        let impact_velocity_y = player.source_self_velocity_y;
        enter_landing_from_airborne(
            player,
            collision_motion_state,
            impact_velocity_y,
            trigger_timer,
            common_data,
        );
    }
}

fn finish_source_floor_contact(player: &mut PlayerState) {
    player.ecb_bottom_lock_timer = 0;
    player.source_coll_x130_locked = false;
    player.grounded = true;
    player.fast_falling = false;
    player.jumps_remaining = player.profile.reusable_air_jumps();
    sync_source_ground_collision_root(player);
}

fn sync_source_ground_collision_root(player: &mut PlayerState) {
    player.source_coll_last_pos = player.source_position;
    player.source_coll_prev_pos = player.source_position;
    player.source_coll_cur_pos = player.source_position;
}

fn install_source_floor_contact(
    stage: StageProfile,
    player: &mut PlayerState,
    contact: crate::StageLandingContact,
) {
    finish_source_floor_contact(player);
    install_source_floor_from_grounded_position(stage, player);
    if player.source_coll_floor_line_index.is_some() {
        return;
    }
    player.source_coll_floor_surface_index = stage
        .collision_surfaces()
        .into_iter()
        .position(|surface| surface == contact.surface)
        .map(|index| index as u8);
}

fn install_source_floor_from_grounded_position(stage: StageProfile, player: &mut PlayerState) {
    player.source_coll_floor_surface_index = None;
    player.source_coll_floor_line_index = None;
    if let Some(melee_stage) = stage.melee_stage_profile() {
        if let Some(line_id) =
            source_floor_line_for_grounded_position(stage, melee_stage.collision, player)
        {
            player.source_coll_floor_line_index = Some(line_id as u16);
            player.source_coll_floor_surface_index =
                source_floor_surface_index_for_line(stage, melee_stage.collision, line_id);
            return;
        }
    }
    player.source_coll_floor_surface_index =
        floor_surface_index_for_bottom(stage, player.position).map(|(index, _)| index);
}

fn source_mp_coll_live_jobj_ecb(
    player: &PlayerState,
    common_data: MeleeCommonData,
    flags: u32,
) -> SourceFighterEcb {
    let pose_frame = source_mp_coll_jobj_pose_frame(player);
    let ecb = live_source_local_ecb_for_player_pose_frame_with_flags(
        player,
        pose_frame,
        common_data,
        flags,
    );
    source_ecb_for_facing(ecb, player.source_model_facing)
}

fn source_mp_coll_jobj_pose_frame(player: &PlayerState) -> f32 {
    if source_mp_coll_jobj_samples_post_anim_pose(player) {
        player.cur_anim_frame() + player.frame_speed_mul()
    } else {
        player.cur_anim_frame()
    }
}

fn source_mp_coll_jobj_samples_post_anim_pose(player: &PlayerState) -> bool {
    matches!(
        player.motion_state,
        MotionState::SpecialHi | MotionState::SpecialAirHi
    ) || player
        .melee_action_state_id
        .is_some_and(is_source_damage_action_state_id)
}

fn source_ecb_for_facing(ecb: SourceFighterEcb, _facing: i8) -> SourceFighterEcb {
    // The live directional pose sampler has already evaluated TopN with the
    // fighter's model orientation. Applying facing here mirrors the ECB twice.
    ecb
}

fn source_mp_coll_load_jobj_ecb(
    player: &mut PlayerState,
    common_data: MeleeCommonData,
    flags: u32,
) {
    if player.source_coll_x130_clear {
        player.source_coll_ecb = SOURCE_COLL_ECB_ZERO;
        player.source_coll_x130_clear = false;
    }
    player.source_coll_xe4_ecb = player.source_coll_ecb;
    let ecb = source_mp_coll_live_jobj_ecb(player, common_data, flags);
    player.source_coll_desired_ecb = ecb;
}

fn source_mp_coll_load_ecb_inline(
    player: &mut PlayerState,
    common_data: MeleeCommonData,
    flags: u32,
) {
    let saved_bottom = if player.source_coll_x130_locked {
        Some(player.source_coll_desired_ecb.bottom)
    } else {
        None
    };
    source_mp_coll_load_jobj_ecb(player, common_data, flags);
    if let Some(saved_bottom) = saved_bottom {
        player.source_coll_desired_ecb.bottom = saved_bottom;
    }
    source_mp_coll_sanitize_desired_ecb(player);
}

fn source_mp_coll_load_entry_platform_ecb_inline(
    player: &mut PlayerState,
    common_data: MeleeCommonData,
    flags: u32,
) {
    source_mp_coll_load_jobj_ecb(player, common_data, flags);
    player.source_coll_desired_ecb.bottom = SourceVec2 {
        x: 0.0,
        y: -milli_to_source_units(player.position.y - player.entry_base_y),
    };
    source_mp_coll_sanitize_desired_ecb(player);
}

fn source_mp_coll_sanitize_desired_ecb(player: &mut PlayerState) {
    let ecb = &mut player.source_coll_desired_ecb;
    if (ecb.top.y - ecb.bottom.y).abs() < 1.0 {
        ecb.top.y += 1.0;
        let mid = 0.5 * (ecb.top.y + ecb.bottom.y);
        ecb.left.y = mid;
        ecb.right.y = mid;
    }
    if ecb.top.y < 1.0 {
        ecb.top.y = 1.0;
    }
    if ecb.left.x > -1.0 {
        ecb.left.x = -1.0;
    }
    if ecb.right.x < 1.0 {
        ecb.right.x = 1.0;
    }
    if ecb.top.y < ecb.bottom.y {
        ecb.top.y = ecb.bottom.y + 1.0;
    }
    if ecb.right.y > ecb.top.y || ecb.right.y < ecb.bottom.y {
        let mid = 0.5 * (ecb.top.y + ecb.bottom.y);
        ecb.left.y = mid;
        ecb.right.y = mid;
    }
    if ecb.top.y - ecb.right.y < 0.001 || ecb.right.y - ecb.bottom.y < 0.001 {
        ecb.right.y = 0.5 * (ecb.top.y + ecb.bottom.y);
    }
    if ecb.top.y - ecb.left.y < 0.001 || ecb.left.y - ecb.bottom.y < 0.001 {
        ecb.left.y = 0.5 * (ecb.top.y + ecb.bottom.y);
    }
}

fn source_mp_coll_air_wrapper_begin(player: &mut PlayerState) {
    source_fighter_proc_map_begin(player);
    player.source_coll_last_pos = player.source_coll_cur_pos;
    player.source_coll_cur_pos = player.source_position;
}

fn source_ft_80083e64_entry_platform_air_collision(
    stage: StageProfile,
    collision: StageCollisionProfile,
    player: &mut PlayerState,
    common_data: MeleeCommonData,
) -> bool {
    source_mp_coll_air_wrapper_begin(player);
    player.source_coll_facing_dir = 0;
    source_mp_coll_prev(player);
    source_mp_coll_load_entry_platform_ecb_inline(player, common_data, 6);
    let touched_floor = source_mp_coll_air_inline1(stage, collision, player, 0, false);
    set_source_position_x_source(player, player.source_coll_cur_pos.x);
    set_source_position_y_source(player, player.source_coll_cur_pos.y);
    touched_floor
}

fn source_ft_800821dc_cliff_air_collision(
    stage: StageProfile,
    player: &mut PlayerState,
    common_data: MeleeCommonData,
) -> bool {
    let Some(melee_stage) = stage.melee_stage_profile() else {
        source_fighter_proc_map_begin(player);
        return false;
    };
    source_mp_coll_air_wrapper_begin(player);
    source_mp_coll_prev(player);
    source_mp_coll_load_ecb_inline(player, common_data, 0xA);
    let touched_floor = source_mp_coll_air_inline1(stage, melee_stage.collision, player, 0, false);
    set_source_position_x_source(player, player.source_coll_cur_pos.x);
    set_source_position_y_source(player, player.source_coll_cur_pos.y);
    touched_floor
}

fn source_ft_800846b0_entry_platform_ground_collision(
    stage: StageProfile,
    collision: StageCollisionProfile,
    player: &mut PlayerState,
    common_data: MeleeCommonData,
) -> bool {
    source_fighter_proc_map_begin(player);
    player.source_coll_last_pos = player.source_coll_cur_pos;
    player.source_coll_cur_pos = player.source_position;
    source_mp_coll_prev(player);
    source_mp_coll_load_entry_platform_ecb_inline(player, common_data, 5);
    let grounded = source_mp_coll_ground_inline2(stage, collision, player, 0);
    set_source_position_x_source(player, player.source_coll_cur_pos.x);
    set_source_position_y_source(player, player.source_coll_cur_pos.y);
    grounded
}

fn source_ft_80082708_allow_ground_to_air(
    stage: StageProfile,
    collision: StageCollisionProfile,
    player: &mut PlayerState,
    common_data: MeleeCommonData,
) -> bool {
    let moved_source_y = player.source_position.y;
    player.source_coll_last_pos = player.source_coll_cur_pos;
    player.source_coll_cur_pos = player.source_position;
    let mp_coll_returned_air = source_mp_coll_8004b108(stage, collision, player, common_data);
    set_source_position_x_source(player, player.source_coll_cur_pos.x);
    if mp_coll_returned_air && player.motion_state == MotionState::SpecialHi {
        set_source_position_y_source(player, moved_source_y);
    } else {
        set_source_position_y_source(player, player.source_coll_cur_pos.y);
    }
    player.source_coll_cur_pos = player.source_position;
    mp_coll_returned_air
}

fn source_ft_800827a0_skip_fall(
    stage: StageProfile,
    collision: StageCollisionProfile,
    player: &mut PlayerState,
    common_data: MeleeCommonData,
) -> bool {
    player.source_coll_last_pos = player.source_coll_cur_pos;
    player.source_coll_cur_pos = player.source_position;
    let grounded = source_mp_coll_8004b2dc(stage, collision, player, common_data);
    set_source_position_x_source(player, player.source_coll_cur_pos.x);
    set_source_position_y_source(player, player.source_coll_cur_pos.y);
    player.source_coll_cur_pos = player.source_position;
    grounded
}

fn source_ft_80084280_wait_collision(
    stage: StageProfile,
    collision: StageCollisionProfile,
    player: &mut PlayerState,
    common_data: MeleeCommonData,
) -> bool {
    player.source_coll_last_pos = player.source_coll_cur_pos;
    player.source_coll_cur_pos = player.source_position;
    let grounded =
        if player.player_nudge_x != 0.0 && player.player_nudge_x * f32::from(player.facing) < 0.0 {
            source_mp_coll_8004b2dc(stage, collision, player, common_data)
        } else {
            source_mp_coll_8004b4b0(stage, collision, player, common_data)
        };
    set_source_position_x_source(player, player.source_coll_cur_pos.x);
    set_source_position_y_source(player, player.source_coll_cur_pos.y);
    player.source_coll_cur_pos = player.source_position;
    grounded
}

fn source_mp_coll_8004b108(
    stage: StageProfile,
    collision: StageCollisionProfile,
    player: &mut PlayerState,
    common_data: MeleeCommonData,
) -> bool {
    source_mp_coll_prev(player);
    source_mp_coll_load_ecb_inline(player, common_data, 5);
    source_mp_coll_ground_inline2(stage, collision, player, 0)
}

fn source_mp_coll_8004b2dc(
    stage: StageProfile,
    collision: StageCollisionProfile,
    player: &mut PlayerState,
    common_data: MeleeCommonData,
) -> bool {
    source_mp_coll_prev(player);
    source_mp_coll_load_ecb_inline(player, common_data, 5);
    source_mp_coll_ground_inline2(stage, collision, player, 2)
}

fn source_mp_coll_8004b4b0(
    stage: StageProfile,
    collision: StageCollisionProfile,
    player: &mut PlayerState,
    common_data: MeleeCommonData,
) -> bool {
    source_mp_coll_prev(player);
    source_mp_coll_load_ecb_inline(player, common_data, 5);
    source_mp_coll_ground_inline2(stage, collision, player, 1)
}

fn source_mp_coll_800471f8(
    stage: StageProfile,
    collision: StageCollisionProfile,
    player: &mut PlayerState,
    common_data: MeleeCommonData,
) -> bool {
    source_mp_coll_prev(player);
    source_mp_coll_load_ecb_inline(player, common_data, 6);
    source_mp_coll_air_inline1(stage, collision, player, 0, false)
}

fn source_mp_coll_800473cc(
    stage: StageProfile,
    collision: StageCollisionProfile,
    player: &mut PlayerState,
    common_data: MeleeCommonData,
) -> bool {
    source_mp_coll_prev(player);
    source_mp_coll_load_ecb_inline(player, common_data, 6);
    source_mp_coll_air_inline1(
        stage,
        collision,
        player,
        SOURCE_COLLISION_FLAG_AIR_CAN_GRAB_LEDGE,
        false,
    )
}

fn source_mp_coll_800477e0(
    stage: StageProfile,
    collision: StageCollisionProfile,
    player: &mut PlayerState,
    common_data: MeleeCommonData,
) -> bool {
    source_mp_coll_prev(player);
    source_mp_coll_load_ecb_inline(player, common_data, 6);
    source_mp_coll_air_inline1(stage, collision, player, 1, false)
}

fn source_mp_coll_prev(player: &mut PlayerState) {
    player.source_coll_x28_vec = player.source_coll_cur_pos;
}

fn source_mp_coll_ground_inline2(
    stage: StageProfile,
    collision: StageCollisionProfile,
    player: &mut PlayerState,
    flags: u32,
) -> bool {
    player.source_coll_prev_env_flags = player.source_coll_env_flags;
    player.source_coll_env_flags = 0;
    let is_ecb_tiny = source_coll_is_ecb_tiny(player.source_coll_ecb);
    let mut touching_floor = source_mp_coll_80043754(
        stage,
        collision,
        player,
        flags,
        is_ecb_tiny,
        SourceMapCollisionCallback::GroundAllowGroundToAir,
        false,
    );
    if source_mp_coll_ground_floor_transition(stage, collision, player) {
        player.source_coll_x34_b5 = false;
        touching_floor = true;
    }
    if touching_floor {
        player.source_coll_env_flags |= SOURCE_COLLIDE_FLOOR_PUSH;
    }
    touching_floor
}

fn source_mp_coll_air_inline1(
    stage: StageProfile,
    collision: StageCollisionProfile,
    player: &mut PlayerState,
    flags: u32,
    platform_pass_rejects_soft: bool,
) -> bool {
    player.source_coll_prev_env_flags = player.source_coll_env_flags;
    player.source_coll_env_flags = 0;
    let is_ecb_tiny = source_coll_is_ecb_tiny(player.source_coll_ecb);
    source_mp_coll_80043754(
        stage,
        collision,
        player,
        flags,
        is_ecb_tiny,
        SourceMapCollisionCallback::AirWallAndFloor,
        platform_pass_rejects_soft,
    )
}

fn source_mp_coll_interpolate_ecb(player: &mut PlayerState, time: f32) {
    player.source_coll_prev_ecb = player.source_coll_ecb;
    if player.source_coll_x34_b6 {
        player.source_coll_ecb = player.source_coll_x64_ecb;
        player.source_coll_x34_b6 = false;
    }
    let current = source_ecb_from_fighter_ecb(player.source_coll_ecb);
    let desired = source_ecb_from_fighter_ecb(player.source_coll_desired_ecb);
    player.source_coll_ecb =
        source_fighter_ecb_from_source_ecb(source_interpolate_ecb(current, desired, time));
}

fn source_mp_coll_80043754(
    stage: StageProfile,
    collision: StageCollisionProfile,
    player: &mut PlayerState,
    flags: u32,
    is_ecb_tiny: bool,
    callback: SourceMapCollisionCallback,
    platform_pass_rejects_soft: bool,
) -> bool {
    let velocity = SourcePoint {
        x: player.source_coll_cur_pos.x - player.source_coll_last_pos.x,
        y: player.source_coll_cur_pos.y - player.source_coll_last_pos.y,
    };
    let previous_ecb = source_ecb_from_fighter_ecb(player.source_coll_ecb);
    let desired_ecb = source_ecb_from_fighter_ecb(player.source_coll_desired_ecb);
    let steps = source_air_map_collision_steps(velocity, previous_ecb, desired_ecb);
    let step_velocity = SourcePoint {
        x: velocity.x / steps as f32,
        y: velocity.y / steps as f32,
    };
    player.source_coll_cur_pos = player.source_coll_last_pos;
    player.source_coll_x34_b5 = false;
    let mut ret = false;
    for step in 0..steps {
        if player.source_coll_x34_b5 {
            break;
        }
        let remaining_steps = (steps - step) as f32;
        source_mp_coll_interpolate_ecb(player, 1.0 / remaining_steps);
        player.source_coll_prev_pos = player.source_coll_cur_pos;
        player.source_coll_cur_pos.x += step_velocity.x;
        player.source_coll_cur_pos.y += step_velocity.y;

        let previous_root = source_point_from_vec(player.source_coll_prev_pos);
        let mut current_root = source_point_from_vec(player.source_coll_cur_pos);
        let previous_ecb = source_ecb_from_fighter_ecb(player.source_coll_prev_ecb);
        let current_ecb = source_ecb_from_fighter_ecb(player.source_coll_ecb);
        ret = match callback {
            SourceMapCollisionCallback::AirWallAndFloor => source_air_map_wall_callback(
                stage,
                collision,
                player,
                &mut current_root,
                previous_root,
                previous_ecb,
                current_ecb,
                flags,
                is_ecb_tiny,
                platform_pass_rejects_soft,
            ),
            SourceMapCollisionCallback::GroundAllowGroundToAir => {
                source_ground_map_collision_callback(
                    stage,
                    collision,
                    player,
                    &mut current_root,
                    previous_root,
                    previous_ecb,
                    current_ecb,
                    flags,
                    is_ecb_tiny,
                )
            }
        };
        player.source_coll_cur_pos = source_vec2_from_point(current_root);
    }
    ret
}

const SOURCE_COLL_SUBSTEP_SIZE: f32 = 6.0;
const SOURCE_COLL_LINE_TOLERANCE: f32 = 0.1;
const SOURCE_COLL_INTERSECTION_TOLERANCE: f64 = 0.1;
const SOURCE_COLL_AREA_TOLERANCE: f64 = 0.0001;
const SOURCE_COLL_WALL_ID_MAX: usize = 8;
const SOURCE_COLLISION_FLAG_AIR_PLATFORM_PASS_CALLBACK: u32 = 0x2;
const SOURCE_COLLISION_FLAG_AIR_CAN_GRAB_LEDGE: u32 = 0x4;
const SOURCE_COLLIDE_LEFT_WALL_PUSH: u32 = 0x1;
const SOURCE_COLLIDE_LEFT_WALL_HUG: u32 = 0x20;
const SOURCE_COLLIDE_RIGHT_WALL_PUSH: u32 = 0x40;
const SOURCE_COLLIDE_RIGHT_WALL_HUG: u32 = 0x800;
const SOURCE_COLLIDE_FLOOR_PUSH: u32 = 0x8000;
const SOURCE_COLLIDE_FLOOR_HUG: u32 = 0x10000;
const SOURCE_COLLIDE_LEFT_EDGE: u32 = 0x100000;
const SOURCE_COLLIDE_RIGHT_EDGE: u32 = 0x200000;
const SOURCE_COLLIDE_LEFT_LEDGE_GRAB: u32 = 0x1000000;
const SOURCE_COLLIDE_RIGHT_LEDGE_GRAB: u32 = 0x2000000;
const SOURCE_COLLIDE_LEDGE_GRAB_MASK: u32 =
    SOURCE_COLLIDE_LEFT_LEDGE_GRAB | SOURCE_COLLIDE_RIGHT_LEDGE_GRAB;
const SOURCE_COLLIDE_LEFT_LEDGE_SLIP: u32 = 0x1000_0000;
const SOURCE_COLLIDE_RIGHT_LEDGE_SLIP: u32 = 0x2000_0000;

#[derive(Clone, Copy, Debug)]
enum SourceMapCollisionCallback {
    AirWallAndFloor,
    GroundAllowGroundToAir,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SourcePoint {
    x: f32,
    y: f32,
}

#[derive(Debug, Clone, Copy)]
struct SourceEcb {
    top: SourcePoint,
    right: SourcePoint,
    bottom: SourcePoint,
    left: SourcePoint,
}

#[derive(Debug, Clone, Copy)]
struct SourceWallLine {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
}

#[derive(Debug, Clone, Copy)]
struct SourceWallHits {
    ids: [usize; SOURCE_COLL_WALL_ID_MAX],
    len: usize,
}

#[derive(Debug, Default, Clone, Copy)]
struct SourceWallExclusions {
    ids: [Option<usize>; 2],
}

impl SourceWallExclusions {
    fn contains(self, line_id: usize) -> bool {
        self.ids
            .into_iter()
            .flatten()
            .any(|excluded| excluded == line_id)
    }
}

impl Default for SourceWallHits {
    fn default() -> Self {
        Self {
            ids: [usize::MAX; SOURCE_COLL_WALL_ID_MAX],
            len: 0,
        }
    }
}

impl SourceWallHits {
    fn is_empty(self) -> bool {
        self.len == 0
    }

    fn iter(self) -> impl Iterator<Item = usize> {
        self.ids.into_iter().take(self.len)
    }
}

#[derive(Debug, Clone, Copy)]
struct SourceSweptWallHit {
    line_id: usize,
}

fn source_point_from_vec(point: SourceVec2) -> SourcePoint {
    SourcePoint {
        x: point.x,
        y: point.y,
    }
}

fn source_ecb_from_fighter_ecb(ecb: SourceFighterEcb) -> SourceEcb {
    SourceEcb {
        top: source_point_from_vec(ecb.top),
        right: source_point_from_vec(ecb.right),
        bottom: source_point_from_vec(ecb.bottom),
        left: source_point_from_vec(ecb.left),
    }
}

fn source_fighter_ecb_from_source_ecb(ecb: SourceEcb) -> SourceFighterEcb {
    SourceFighterEcb {
        top: source_vec2_from_point(ecb.top),
        right: source_vec2_from_point(ecb.right),
        bottom: source_vec2_from_point(ecb.bottom),
        left: source_vec2_from_point(ecb.left),
    }
}

fn source_vec2_from_point(point: SourcePoint) -> SourceVec2 {
    SourceVec2 {
        x: point.x,
        y: point.y,
    }
}

fn source_air_map_collision_steps(
    velocity: SourcePoint,
    previous_ecb: SourceEcb,
    desired_ecb: SourceEcb,
) -> usize {
    let max_step = velocity
        .x
        .abs()
        .max(velocity.y.abs())
        .max((desired_ecb.left.x - previous_ecb.left.x).abs())
        .max((desired_ecb.right.x - previous_ecb.right.x).abs())
        .max((desired_ecb.top.y - previous_ecb.top.y).abs())
        .max((desired_ecb.right.y - previous_ecb.right.y).abs());
    if max_step > SOURCE_COLL_SUBSTEP_SIZE {
        (max_step / SOURCE_COLL_SUBSTEP_SIZE) as usize + 1
    } else {
        1
    }
}

fn source_coll_is_ecb_tiny(ecb: SourceFighterEcb) -> bool {
    ecb.top.y - ecb.bottom.y < 6.0 && ecb.right.y - ecb.left.y < 6.0
}

fn source_interpolate_ecb(current: SourceEcb, desired: SourceEcb, time: f32) -> SourceEcb {
    SourceEcb {
        top: source_interpolate_point(current.top, desired.top, time),
        right: source_interpolate_point(current.right, desired.right, time),
        bottom: source_interpolate_point(current.bottom, desired.bottom, time),
        left: source_interpolate_point(current.left, desired.left, time),
    }
}

fn source_interpolate_point(current: SourcePoint, desired: SourcePoint, time: f32) -> SourcePoint {
    SourcePoint {
        x: current.x + (desired.x - current.x) * time,
        y: current.y + (desired.y - current.y) * time,
    }
}

fn source_air_map_wall_callback(
    stage: StageProfile,
    collision: StageCollisionProfile,
    player: &mut PlayerState,
    current_root: &mut SourcePoint,
    previous_root: SourcePoint,
    previous_ecb: SourceEcb,
    current_ecb: SourceEcb,
    flags: u32,
    is_ecb_tiny: bool,
    platform_pass_rejects_soft: bool,
) -> bool {
    let platform_pass = flags & SOURCE_COLLISION_FLAG_AIR_PLATFORM_PASS_CALLBACK != 0;
    let stay_airborne = flags & 0x1 != 0;
    let mut touched_floor = false;
    let mut squeeze_flags_all = 0_u8;
    let mut squeeze_flags = 0_u8;
    let mut left_right_flags = 0_u8;
    let mut current_ecb = current_ecb;
    // mpColl_80044E10_RightWall and mpColl_80045B74_LeftWall do not
    // exclude walls connected to the fighter's last floor line.
    let floor_id = None;

    loop {
        let old_squeeze_flags = squeeze_flags;
        let prev_b6 = player.source_coll_x34_b6;
        let mut x_after_collide_left = 0.0_f32;
        let mut x_after_collide_right = 0.0_f32;
        let mut y_after_collide_ceiling = 0.0_f32;
        let mut y_after_collide_floor = 0.0_f32;
        squeeze_flags = 0;

        let mut left_hits = SourceWallHits::default();
        let left_wall_hug = source_collect_left_wall_hits(
            collision,
            &mut left_hits,
            previous_root,
            *current_root,
            previous_ecb,
            current_ecb,
            is_ecb_tiny,
            floor_id,
        );
        if left_wall_hug {
            player.source_coll_env_flags |= SOURCE_COLLIDE_LEFT_WALL_HUG;
        }
        if !left_hits.is_empty() {
            player.source_coll_env_flags |= SOURCE_COLLIDE_LEFT_WALL_PUSH;
            if source_correct_left_wall(collision, current_root, current_ecb, left_hits) {
                left_right_flags |= 1;
                squeeze_flags |= 8;
            }
            x_after_collide_left = current_root.x;
        }

        let mut right_hits = SourceWallHits::default();
        let right_wall_hug = source_collect_right_wall_hits(
            collision,
            &mut right_hits,
            previous_root,
            *current_root,
            previous_ecb,
            current_ecb,
            is_ecb_tiny,
            floor_id,
        );
        if right_wall_hug {
            player.source_coll_env_flags |= SOURCE_COLLIDE_RIGHT_WALL_HUG;
        }
        if !right_hits.is_empty() {
            player.source_coll_env_flags |= SOURCE_COLLIDE_RIGHT_WALL_PUSH;
            if source_correct_right_wall_air(collision, current_root, current_ecb, right_hits) {
                left_right_flags |= 2;
                squeeze_flags |= 4;
            }
            x_after_collide_right = current_root.x;
        }

        let mut left_hits = SourceWallHits::default();
        let left_wall_hug = source_collect_left_wall_hits(
            collision,
            &mut left_hits,
            previous_root,
            *current_root,
            previous_ecb,
            current_ecb,
            is_ecb_tiny,
            floor_id,
        );
        if left_wall_hug {
            player.source_coll_env_flags |= SOURCE_COLLIDE_LEFT_WALL_HUG;
        }
        if !left_hits.is_empty() {
            player.source_coll_env_flags |= SOURCE_COLLIDE_LEFT_WALL_PUSH;
            if source_correct_left_wall(collision, current_root, current_ecb, left_hits) {
                left_right_flags |= 1;
                squeeze_flags |= 8;
            }
            x_after_collide_left = current_root.x;
        }

        let mut right_hits = SourceWallHits::default();
        let right_wall_hug = source_collect_right_wall_hits(
            collision,
            &mut right_hits,
            previous_root,
            *current_root,
            previous_ecb,
            current_ecb,
            is_ecb_tiny,
            floor_id,
        );
        if right_wall_hug {
            player.source_coll_env_flags |= SOURCE_COLLIDE_RIGHT_WALL_HUG;
        }
        if !right_hits.is_empty() {
            player.source_coll_env_flags |= SOURCE_COLLIDE_RIGHT_WALL_PUSH;
            if source_correct_right_wall_air(collision, current_root, current_ecb, right_hits) {
                left_right_flags |= 2;
                squeeze_flags |= 4;
            }
            x_after_collide_right = current_root.x;
        }

        if (squeeze_flags & 0b1100) == 0b1100 {
            source_mp_coll_squeeze_horizontal(
                player,
                current_root,
                &mut current_ecb,
                x_after_collide_right,
                x_after_collide_left,
            );
        }

        if let Some(ceiling_id) = source_mp_coll_ceiling_check_air(
            collision,
            current_root,
            previous_root,
            previous_ecb,
            current_ecb,
            left_right_flags,
        ) {
            source_mp_coll_ceiling_collide_air(collision, current_root, current_ecb, ceiling_id);
            squeeze_flags |= 1;
            y_after_collide_ceiling = current_root.y;
        }

        if source_mp_coll_floor_check_air(
            stage,
            collision,
            player,
            current_root,
            previous_root,
            previous_ecb,
            current_ecb,
            flags,
            platform_pass_rejects_soft,
        ) {
            y_after_collide_floor = current_root.y;
            squeeze_flags |= 2;
            if stay_airborne {
                player.source_coll_x34_b5 = false;
            } else {
                player.source_coll_x34_b5 = true;
                touched_floor = true;
            }

            if let Some(ceiling_id) = source_mp_coll_ceiling_check_air(
                collision,
                current_root,
                previous_root,
                previous_ecb,
                current_ecb,
                left_right_flags,
            ) {
                source_mp_coll_ceiling_collide_air(
                    collision,
                    current_root,
                    current_ecb,
                    ceiling_id,
                );
                squeeze_flags |= 1;
                y_after_collide_ceiling = current_root.y;
            }
        }

        if (squeeze_flags & 0b0011) == 0b0011 {
            source_mp_coll_squeeze_vertical(
                player,
                current_root,
                &mut current_ecb,
                !touched_floor,
                y_after_collide_ceiling,
                y_after_collide_floor,
            );
        }

        squeeze_flags_all |= squeeze_flags;
        if prev_b6 == player.source_coll_x34_b6 && squeeze_flags == old_squeeze_flags {
            break;
        }
        let _ = platform_pass;
    }
    source_air_collision_check_for_ledge_grab(
        stage,
        collision,
        player,
        current_root,
        previous_root,
        current_ecb,
        flags,
        touched_floor,
    );
    if squeeze_flags_all & 0b1000 == 0 {
        player.source_coll_env_flags &=
            !(SOURCE_COLLIDE_LEFT_WALL_PUSH | SOURCE_COLLIDE_LEFT_WALL_HUG);
    }
    if squeeze_flags_all & 0b0100 == 0 {
        player.source_coll_env_flags &=
            !(SOURCE_COLLIDE_RIGHT_WALL_PUSH | SOURCE_COLLIDE_RIGHT_WALL_HUG);
    }
    player.source_coll_ecb = source_fighter_ecb_from_source_ecb(current_ecb);
    touched_floor
}

fn source_air_collision_check_for_ledge_grab(
    stage: StageProfile,
    collision: StageCollisionProfile,
    player: &mut PlayerState,
    current_root: &SourcePoint,
    previous_root: SourcePoint,
    current_ecb: SourceEcb,
    flags: u32,
    touched_floor: bool,
) {
    if touched_floor || flags & SOURCE_COLLISION_FLAG_AIR_CAN_GRAB_LEDGE == 0 {
        return;
    }
    let mut on_edge =
        player.source_coll_env_flags & (SOURCE_COLLIDE_LEFT_EDGE | SOURCE_COLLIDE_RIGHT_EDGE) != 0;
    if on_edge || current_root.y >= previous_root.y {
        return;
    }
    if player.source_coll_facing_dir == 1 || player.source_coll_facing_dir == 0 {
        if let Some(ledge) = source_air_collision_check_left_ledge_grab(
            stage,
            collision,
            player.profile.ledge_snap_x_milli,
            player.profile.ledge_snap_y_milli,
            player.profile.ledge_snap_height_milli,
            previous_root,
            *current_root,
            current_ecb,
        ) {
            on_edge = true;
            player.source_coll_ledge_id_left = Some(ledge.line_index);
            player.source_coll_env_flags |= SOURCE_COLLIDE_LEFT_LEDGE_GRAB;
        } else {
            on_edge = false;
        }
        if on_edge {
            player.source_coll_env_flags |= SOURCE_COLLIDE_LEFT_LEDGE_GRAB;
        }
    }
    if player.source_coll_facing_dir == -1 || player.source_coll_facing_dir == 0 {
        if let Some(ledge) = source_air_collision_check_right_ledge_grab(
            stage,
            collision,
            player.profile.ledge_snap_x_milli,
            player.profile.ledge_snap_y_milli,
            player.profile.ledge_snap_height_milli,
            previous_root,
            *current_root,
            current_ecb,
        ) {
            on_edge = true;
            player.source_coll_ledge_id_right = Some(ledge.line_index);
            player.source_coll_env_flags |= SOURCE_COLLIDE_RIGHT_LEDGE_GRAB;
        } else {
            on_edge = false;
        }
        if on_edge {
            player.source_coll_env_flags |= SOURCE_COLLIDE_RIGHT_LEDGE_GRAB;
        }
    }
}

fn source_air_collision_check_left_ledge_grab(
    stage: StageProfile,
    collision: StageCollisionProfile,
    ledge_snap_x_milli: i32,
    ledge_snap_y_milli: i32,
    ledge_snap_height_milli: i32,
    previous_root: SourcePoint,
    current_root: SourcePoint,
    current_ecb: SourceEcb,
) -> Option<StageLedge> {
    source_air_collision_check_ledge_grab_on_side(
        stage,
        collision,
        ledge_snap_x_milli,
        ledge_snap_y_milli,
        ledge_snap_height_milli,
        previous_root,
        current_root,
        current_ecb,
        StageLedgeSide::Left,
    )
}

fn source_air_collision_check_right_ledge_grab(
    stage: StageProfile,
    collision: StageCollisionProfile,
    ledge_snap_x_milli: i32,
    ledge_snap_y_milli: i32,
    ledge_snap_height_milli: i32,
    previous_root: SourcePoint,
    current_root: SourcePoint,
    current_ecb: SourceEcb,
) -> Option<StageLedge> {
    source_air_collision_check_ledge_grab_on_side(
        stage,
        collision,
        ledge_snap_x_milli,
        ledge_snap_y_milli,
        ledge_snap_height_milli,
        previous_root,
        current_root,
        current_ecb,
        StageLedgeSide::Right,
    )
}

fn source_air_collision_check_ledge_grab_on_side(
    stage: StageProfile,
    collision: StageCollisionProfile,
    ledge_snap_x_milli: i32,
    ledge_snap_y_milli: i32,
    ledge_snap_height_milli: i32,
    previous_root: SourcePoint,
    current_root: SourcePoint,
    current_ecb: SourceEcb,
    side: StageLedgeSide,
) -> Option<StageLedge> {
    let snap_x = milli_to_source_units(ledge_snap_x_milli);
    let snap_y = milli_to_source_units(ledge_snap_y_milli);
    let half_height = 0.5 * milli_to_source_units(ledge_snap_height_milli);
    let (left, right) = match side {
        StageLedgeSide::Left => {
            if previous_root.x < current_root.x {
                (
                    previous_root.x,
                    snap_x + current_root.x + current_ecb.right.x,
                )
            } else {
                (
                    current_root.x,
                    snap_x + previous_root.x + current_ecb.right.x,
                )
            }
        }
        StageLedgeSide::Right => {
            let snap_x = -snap_x;
            if previous_root.x > current_root.x {
                (
                    snap_x + current_root.x + current_ecb.left.x,
                    previous_root.x,
                )
            } else {
                (
                    snap_x + previous_root.x + current_ecb.left.x,
                    current_root.x,
                )
            }
        }
    };
    let (bottom, top) = if previous_root.y < current_root.y {
        (
            previous_root.y + snap_y - half_height,
            current_root.y + snap_y + half_height,
        )
    } else {
        (
            current_root.y + snap_y - half_height,
            previous_root.y + snap_y + half_height,
        )
    };

    let mut best: Option<(StageLedge, SourcePoint)> = None;
    for ledge in stage
        .ledges
        .iter()
        .copied()
        .filter(|ledge| ledge.side == side)
    {
        let line_id = usize::from(ledge.line_index);
        let Some(source_line) = collision.lines.get(line_id) else {
            continue;
        };
        if source_line.kind != StageCollisionLineKind::Floor
            || source_line.passable
            || !source_line.has_ledge_flag()
        {
            continue;
        }
        let Some(line) = collision.scaled_line(line_id) else {
            continue;
        };
        if !source_ledge_line_bounds_overlap_source(
            line.x0, line.y0, line.x1, line.y1, left, bottom, right, top,
        ) {
            continue;
        }
        let edge = match side {
            StageLedgeSide::Left => SourcePoint {
                x: line.x0,
                y: line.y0,
            },
            StageLedgeSide::Right => SourcePoint {
                x: line.x1,
                y: line.y1,
            },
        };
        if !source_air_collision_ledge_reaches_edge(current_root, current_ecb, edge, side) {
            continue;
        }
        let replace = match (side, best) {
            (_, None) => true,
            (StageLedgeSide::Left, Some((_, current_edge))) => edge.x < current_edge.x,
            (StageLedgeSide::Right, Some((_, current_edge))) => edge.x > current_edge.x,
        };
        if replace {
            best = Some((ledge, edge));
        }
    }
    best.map(|(ledge, _)| ledge)
}

fn source_ledge_line_bounds_overlap_source(
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    left: f32,
    bottom: f32,
    right: f32,
    top: f32,
) -> bool {
    let line_left = x0.min(x1);
    let line_right = x0.max(x1);
    let line_bottom = y0.min(y1);
    let line_top = y0.max(y1);
    (line_right + line_left - (right + left)).abs() < (line_right - line_left) + (right - left)
        && (line_top + line_bottom - (top + bottom)).abs()
            < (line_top - line_bottom) + (top - bottom)
}

fn source_air_collision_ledge_reaches_edge(
    current_root: SourcePoint,
    current_ecb: SourceEcb,
    edge: SourcePoint,
    side: StageLedgeSide,
) -> bool {
    let current_bottom = source_ecb_world_point(current_root, current_ecb.bottom);
    match side {
        StageLedgeSide::Left => {
            current_root.x + current_ecb.right.x <= edge.x + 5.0
                && current_bottom.x < edge.x
                && current_bottom.y < edge.y
        }
        StageLedgeSide::Right => {
            current_root.x + current_ecb.left.x >= edge.x - 5.0
                && current_bottom.x > edge.x
                && current_bottom.y < edge.y
        }
    }
}

fn source_ground_map_collision_callback(
    stage: StageProfile,
    collision: StageCollisionProfile,
    player: &mut PlayerState,
    current_root: &mut SourcePoint,
    previous_root: SourcePoint,
    previous_ecb: SourceEcb,
    current_ecb: SourceEcb,
    flags: u32,
    is_ecb_tiny: bool,
) -> bool {
    for _ in 0..2 {
        let floor_id = player.source_coll_floor_line_index.map(usize::from);
        let mut left_hits = SourceWallHits::default();
        let left_wall_hug = source_collect_ground_left_wall_hits(
            collision,
            &mut left_hits,
            previous_root,
            *current_root,
            previous_ecb,
            current_ecb,
            is_ecb_tiny,
            floor_id,
        );
        if left_wall_hug {
            player.source_coll_env_flags |= SOURCE_COLLIDE_LEFT_WALL_HUG;
        }
        if !left_hits.is_empty() {
            player.source_coll_env_flags |= SOURCE_COLLIDE_LEFT_WALL_PUSH;
            if !source_correct_ground_left_wall(collision, current_root, current_ecb, left_hits) {
                player.source_coll_env_flags &=
                    !(SOURCE_COLLIDE_LEFT_WALL_PUSH | SOURCE_COLLIDE_LEFT_WALL_HUG);
            }
            player.source_coll_x34_b5 = true;
        }

        let mut right_hits = SourceWallHits::default();
        let right_wall_hug = source_collect_ground_right_wall_hits(
            collision,
            &mut right_hits,
            previous_root,
            *current_root,
            previous_ecb,
            current_ecb,
            is_ecb_tiny,
            floor_id,
        );
        if right_wall_hug {
            player.source_coll_env_flags |= SOURCE_COLLIDE_RIGHT_WALL_HUG;
        }
        if !right_hits.is_empty() {
            player.source_coll_env_flags |= SOURCE_COLLIDE_RIGHT_WALL_PUSH;
            if !source_correct_ground_right_wall(collision, current_root, current_ecb, right_hits) {
                player.source_coll_env_flags &=
                    !(SOURCE_COLLIDE_RIGHT_WALL_PUSH | SOURCE_COLLIDE_RIGHT_WALL_HUG);
            }
            player.source_coll_x34_b5 = true;
        }
    }

    if source_mp_coll_floor_push_at_root(stage, collision, player, current_root, current_ecb, flags)
    {
        player.source_coll_env_flags |= SOURCE_COLLIDE_FLOOR_PUSH;
        return true;
    }

    if flags & 2 != 0
        && source_mp_coll_floor_edge_clamp_flags2(
            stage,
            collision,
            player,
            current_root,
            current_ecb,
        )
    {
        player.source_coll_env_flags |= SOURCE_COLLIDE_FLOOR_PUSH;
        player.source_coll_x34_b5 = true;
        return true;
    }

    false
}

fn source_air_collision_uses_fall_wrapper(motion_state: MotionState) -> bool {
    matches!(
        motion_state,
        MotionState::Fall
            | MotionState::FallF
            | MotionState::FallB
            | MotionState::FallAerial
            | MotionState::FallAerialF
            | MotionState::FallAerialB
            | MotionState::DamageFall
            | MotionState::JumpF
            | MotionState::JumpB
            | MotionState::JumpAerialF
            | MotionState::JumpAerialB
            | MotionState::FallSpecial
            | MotionState::FallSpecialF
            | MotionState::FallSpecialB
            | MotionState::ShieldBreakFly
            | MotionState::ShieldBreakFall
    )
}

fn source_mp_coll_squeeze_horizontal(
    player: &mut PlayerState,
    current_root: &mut SourcePoint,
    current_ecb: &mut SourceEcb,
    right: f32,
    left: f32,
) {
    let half_width = 0.5 * (left - right + current_ecb.right.x - current_ecb.left.x);
    if !player.source_coll_x34_b6 {
        player.source_coll_x64_ecb = player.source_coll_ecb;
    }
    player.source_coll_x34_b6 = true;
    current_root.x = (left + current_ecb.right.x) - half_width;
    current_ecb.right.x = half_width;
    current_ecb.left.x = -half_width;
    player.source_coll_desired_ecb.right.x = current_ecb.right.x;
    player.source_coll_desired_ecb.left.x = current_ecb.left.x;
    player.source_coll_x34_b5 = false;
}

fn source_mp_coll_squeeze_vertical(
    player: &mut PlayerState,
    current_root: &mut SourcePoint,
    current_ecb: &mut SourceEcb,
    airborne: bool,
    top: f32,
    bottom: f32,
) {
    let height = top - bottom + current_ecb.top.y - current_ecb.bottom.y;
    if !player.source_coll_x34_b6 {
        player.source_coll_x64_ecb = player.source_coll_ecb;
    }
    player.source_coll_x34_b6 = true;
    if height < 3.0 {
        let old_height = current_ecb.top.y - current_ecb.bottom.y;
        let new_height = current_ecb.top.y + top - bottom;
        current_ecb.top.y = old_height.min(new_height);
        current_ecb.bottom.y = 0.0;
        current_root.y = bottom;
    } else if !airborne {
        current_root.y = bottom;
        current_ecb.top.y = height + current_ecb.bottom.y;
    } else {
        current_root.y = 0.5 * (top + bottom);
        current_ecb.top.y = 0.5 * (current_ecb.top.y + current_ecb.bottom.y + height);
        current_ecb.bottom.y = current_ecb.top.y - height;
    }
    let mid_y = 0.5 * (current_ecb.top.y + current_ecb.bottom.y);
    current_ecb.right.y = mid_y;
    current_ecb.left.y = mid_y;
    player.source_coll_desired_ecb.top.y = current_ecb.top.y;
    player.source_coll_desired_ecb.bottom.y = current_ecb.bottom.y;
    player.source_coll_desired_ecb.left.y = current_ecb.left.y;
    player.source_coll_desired_ecb.right.y = current_ecb.right.y;
    player.source_coll_x34_b5 = false;
}

fn source_mp_coll_ceiling_check_air(
    collision: StageCollisionProfile,
    current_root: &SourcePoint,
    previous_root: SourcePoint,
    previous_ecb: SourceEcb,
    current_ecb: SourceEcb,
    _left_right_flags: u8,
) -> Option<usize> {
    let previous_top = source_ecb_world_point(previous_root, previous_ecb.top);
    let current_top = source_ecb_world_point(*current_root, current_ecb.top);
    source_check_ceiling(collision, previous_top, current_top)
}

fn source_mp_coll_ceiling_collide_air(
    collision: StageCollisionProfile,
    current_root: &mut SourcePoint,
    current_ecb: SourceEcb,
    ceiling_id: usize,
) -> usize {
    let top = source_ecb_world_point(*current_root, current_ecb.top);
    if let Some((line_id, y_delta)) = source_mp_lib_ceiling_project(collision, ceiling_id, top) {
        current_root.y += y_delta;
        return line_id;
    }
    ceiling_id
}

fn source_mp_coll_floor_check_air(
    stage: StageProfile,
    collision: StageCollisionProfile,
    player: &mut PlayerState,
    current_root: &mut SourcePoint,
    previous_root: SourcePoint,
    previous_ecb: SourceEcb,
    current_ecb: SourceEcb,
    _flags: u32,
    platform_pass_rejects_soft: bool,
) -> bool {
    let previous_bottom = source_ecb_world_point(previous_root, previous_ecb.bottom);
    let current_bottom = source_ecb_world_point(*current_root, current_ecb.bottom);
    let Some(floor_id) = source_check_floor(collision, previous_bottom, current_bottom) else {
        return false;
    };
    if platform_pass_rejects_soft
        && source_line_kind(collision, floor_id) == Some(StageCollisionLineKind::SoftFloor)
    {
        return false;
    }
    if player
        .source_coll_floor_skip_line_index
        .is_some_and(|skip_line_id| usize::from(skip_line_id) == floor_id)
    {
        return false;
    }
    let snapped_floor_id =
        source_mp_coll_snap_to_floor_no_edge_pass(collision, current_root, current_ecb, floor_id);
    player.source_coll_env_flags |= SOURCE_COLLIDE_FLOOR_PUSH | SOURCE_COLLIDE_FLOOR_HUG;
    player.source_coll_floor_line_index = Some(snapped_floor_id as u16);
    player.source_coll_floor_surface_index =
        source_floor_surface_index_for_line(stage, collision, snapped_floor_id);
    true
}

fn source_check_ceiling(
    collision: StageCollisionProfile,
    previous_top: SourcePoint,
    current_top: SourcePoint,
) -> Option<usize> {
    let mut best = None;
    let mut best_distance = f32::INFINITY;
    source_for_each_ceiling_line(collision, |line_id, _line| {
        let Some(line) = source_wall_line(collision, line_id) else {
            return;
        };
        let ceiling_start = SourcePoint {
            x: line.x0,
            y: line.y0,
        };
        let ceiling_end = SourcePoint {
            x: line.x1,
            y: line.y1,
        };
        let Some(intersection) =
            source_floor_line_intersection(ceiling_start, ceiling_end, previous_top, current_top)
        else {
            return;
        };
        let distance = source_distance_sq(previous_top, intersection);
        if distance < best_distance {
            best_distance = distance;
            best = Some(line_id);
        }
    });
    best
}

fn source_mp_coll_ground_floor_transition(
    stage: StageProfile,
    collision: StageCollisionProfile,
    player: &mut PlayerState,
) -> bool {
    let current_floor = player.source_coll_floor_line_index.map(usize::from);
    let previous_root = source_point_from_vec(player.source_coll_prev_pos);
    let current_root = source_point_from_vec(player.source_coll_cur_pos);
    let previous_ecb = source_ecb_from_fighter_ecb(player.source_coll_prev_ecb);
    let current_ecb = source_ecb_from_fighter_ecb(player.source_coll_ecb);
    let previous_bottom = source_ecb_world_point(previous_root, previous_ecb.bottom);
    let current_bottom = source_ecb_world_point(current_root, current_ecb.bottom);
    let floor_id = source_check_floor_excluding_connected(
        collision,
        previous_bottom,
        current_bottom,
        current_floor,
    )
    .or_else(|| {
        let previous_mid = SourcePoint {
            x: previous_bottom.x,
            y: previous_root.y + 0.5 * (previous_ecb.top.y + previous_ecb.bottom.y),
        };
        source_check_floor_excluding_connected(
            collision,
            previous_mid,
            current_bottom,
            current_floor,
        )
    });
    let Some(floor_id) = floor_id else {
        return false;
    };
    let mut snapped_root = current_root;
    let snapped_floor_id = source_mp_coll_snap_to_floor_with_bottom(
        collision,
        &mut snapped_root,
        current_ecb,
        floor_id,
    );
    player.source_coll_cur_pos = source_vec2_from_point(snapped_root);
    player.source_coll_floor_line_index = Some(snapped_floor_id as u16);
    player.source_coll_floor_surface_index =
        source_floor_surface_index_for_line(stage, collision, snapped_floor_id);
    true
}

fn source_check_floor(
    collision: StageCollisionProfile,
    previous_bottom: SourcePoint,
    current_bottom: SourcePoint,
) -> Option<usize> {
    let mut best = None;
    let mut best_distance = f32::INFINITY;
    source_for_each_floor_line_in_sweep(
        collision,
        previous_bottom,
        current_bottom,
        |line_id, line| {
            let Some(source_line) = source_wall_line(collision, line_id) else {
                return;
            };
            let floor_start = SourcePoint {
                x: source_line.x0,
                y: source_line.y0,
            };
            let floor_end = SourcePoint {
                x: source_line.x1,
                y: source_line.y1,
            };
            let Some(intersection) = source_floor_line_intersection_for_kind(
                line.kind,
                floor_start,
                floor_end,
                previous_bottom,
                current_bottom,
            ) else {
                return;
            };
            let distance = source_distance_sq(previous_bottom, intersection);
            if distance < best_distance {
                best_distance = distance;
                best = Some(line_id);
            }
        },
    );
    best
}

fn source_floor_line_intersection_for_kind(
    _kind: StageCollisionLineKind,
    floor_start: SourcePoint,
    floor_end: SourcePoint,
    sweep_start: SourcePoint,
    sweep_end: SourcePoint,
) -> Option<SourcePoint> {
    source_floor_line_intersection(floor_start, floor_end, sweep_start, sweep_end)
}

fn source_check_floor_excluding_connected(
    collision: StageCollisionProfile,
    previous_bottom: SourcePoint,
    current_bottom: SourcePoint,
    excluded_floor_id: Option<usize>,
) -> Option<usize> {
    let mut best = None;
    let mut best_distance = f32::INFINITY;
    source_for_each_floor_line_in_sweep(
        collision,
        previous_bottom,
        current_bottom,
        |line_id, _line| {
            if let Some(excluded_floor_id) = excluded_floor_id {
                if line_id == excluded_floor_id
                    || source_lines_connected(collision, line_id, excluded_floor_id)
                {
                    return;
                }
            }
            let Some(line) = source_wall_line(collision, line_id) else {
                return;
            };
            let floor_start = SourcePoint {
                x: line.x0,
                y: line.y0,
            };
            let floor_end = SourcePoint {
                x: line.x1,
                y: line.y1,
            };
            let Some(intersection) = source_floor_line_intersection(
                floor_start,
                floor_end,
                previous_bottom,
                current_bottom,
            ) else {
                return;
            };
            let distance = source_distance_sq(previous_bottom, intersection);
            if distance < best_distance {
                best_distance = distance;
                best = Some(line_id);
            }
        },
    );
    best
}

fn source_mp_coll_snap_to_floor_with_bottom(
    collision: StageCollisionProfile,
    current_root: &mut SourcePoint,
    current_ecb: SourceEcb,
    floor_id: usize,
) -> usize {
    let bottom = source_ecb_world_point(*current_root, current_ecb.bottom);
    if let Some((line_id, y_delta)) = source_mp_lib_floor_project(collision, floor_id, bottom) {
        current_root.y += y_delta;
        return line_id;
    }
    floor_id
}

fn source_mp_lib_ceiling_project(
    collision: StageCollisionProfile,
    ceiling_id: usize,
    point: SourcePoint,
) -> Option<(usize, f32)> {
    let line = collision.scaled_line(ceiling_id)?;
    let min_x = line.x0.min(line.x1) - SOURCE_COLL_LINE_TOLERANCE;
    let max_x = line.x0.max(line.x1) + SOURCE_COLL_LINE_TOLERANCE;
    if point.x < min_x || point.x > max_x {
        return None;
    }
    let dx = line.x1 - line.x0;
    if dx.abs() <= f32::EPSILON {
        return None;
    }
    let t = ((point.x - line.x0) / dx).clamp(0.0, 1.0);
    let y = line.y0 + (line.y1 - line.y0) * t;
    Some((ceiling_id, y - point.y))
}

fn source_mp_coll_snap_to_floor_no_edge_pass(
    collision: StageCollisionProfile,
    current_root: &mut SourcePoint,
    current_ecb: SourceEcb,
    floor_id: usize,
) -> usize {
    let ignore_bottom = current_ecb.bottom.y > 0.0;
    let bottom = if ignore_bottom {
        *current_root
    } else {
        source_ecb_world_point(*current_root, current_ecb.bottom)
    };

    if let Some((line_id, y_delta)) = source_mp_lib_floor_project(collision, floor_id, bottom) {
        current_root.y += y_delta;
        return line_id;
    }

    let Some(line) = collision.scaled_line(floor_id) else {
        return floor_id;
    };
    let edge = if line.x0 <= bottom.x {
        SourcePoint {
            x: line.x1,
            y: line.y1,
        }
    } else {
        SourcePoint {
            x: line.x0,
            y: line.y0,
        }
    };
    current_root.x = edge.x - current_ecb.bottom.x;
    current_root.y = edge.y - current_ecb.bottom.y;
    source_mp_lib_floor_project(collision, floor_id, edge)
        .map(|(line_id, _)| line_id)
        .unwrap_or(floor_id)
}

fn source_mp_coll_floor_push_at_root(
    stage: StageProfile,
    collision: StageCollisionProfile,
    player: &mut PlayerState,
    current_root: &mut SourcePoint,
    current_ecb: SourceEcb,
    _flags: u32,
) -> bool {
    let Some(floor_id) = player.source_coll_floor_line_index.map(usize::from) else {
        return false;
    };
    if !source_line_is_floor(collision, floor_id) {
        return false;
    }

    let bottom = source_ecb_world_point(*current_root, current_ecb.bottom);
    if let Some((line_id, y_delta)) = source_mp_lib_floor_project(collision, floor_id, bottom) {
        current_root.y += y_delta;
        player.source_coll_floor_line_index = Some(line_id as u16);
        player.source_coll_floor_surface_index =
            source_floor_surface_index_for_line(stage, collision, line_id);
        return true;
    }

    if let Some(line) = collision.scaled_line(floor_id) {
        if current_root.x < line.x0.min(line.x1) {
            player.source_coll_env_flags |= SOURCE_COLLIDE_LEFT_LEDGE_SLIP;
        } else if current_root.x > line.x0.max(line.x1) {
            player.source_coll_env_flags |= SOURCE_COLLIDE_RIGHT_LEDGE_SLIP;
        }
    }
    false
}

#[derive(Debug, Clone, Copy)]
enum SourceFloorEdgeSide {
    Left,
    Right,
}

fn source_mp_coll_floor_edge_clamp_flags2(
    stage: StageProfile,
    collision: StageCollisionProfile,
    player: &mut PlayerState,
    current_root: &mut SourcePoint,
    current_ecb: SourceEcb,
) -> bool {
    let Some(floor_id) = player.source_coll_floor_line_index.map(usize::from) else {
        return false;
    };
    if !source_line_is_floor(collision, floor_id) {
        return false;
    }
    let Some((edge, side)) = source_floor_edge_for_cur_pos(collision, floor_id, current_root.x)
    else {
        return false;
    };
    let Some((snapped_floor_id, _)) = source_mp_lib_floor_project(collision, floor_id, edge) else {
        return false;
    };
    if source_floor_edge_blocked_by_wall(collision, edge, current_ecb, side) {
        return false;
    }

    current_root.x = edge.x - current_ecb.bottom.x;
    current_root.y = edge.y - current_ecb.bottom.y;
    player.source_coll_floor_line_index = Some(snapped_floor_id as u16);
    player.source_coll_floor_surface_index =
        source_floor_surface_index_for_line(stage, collision, snapped_floor_id);
    player.source_coll_env_flags &=
        !(SOURCE_COLLIDE_LEFT_LEDGE_SLIP | SOURCE_COLLIDE_RIGHT_LEDGE_SLIP);
    player.source_coll_env_flags |= match side {
        SourceFloorEdgeSide::Left => SOURCE_COLLIDE_RIGHT_EDGE,
        SourceFloorEdgeSide::Right => SOURCE_COLLIDE_LEFT_EDGE,
    };
    true
}

fn source_floor_edge_for_cur_pos(
    collision: StageCollisionProfile,
    floor_id: usize,
    cur_pos_x: f32,
) -> Option<(SourcePoint, SourceFloorEdgeSide)> {
    let line = collision.scaled_line(floor_id)?;
    if cur_pos_x <= line.x0 {
        return Some((
            SourcePoint {
                x: line.x0,
                y: line.y0,
            },
            SourceFloorEdgeSide::Left,
        ));
    }
    if cur_pos_x >= line.x1 {
        return Some((
            SourcePoint {
                x: line.x1,
                y: line.y1,
            },
            SourceFloorEdgeSide::Right,
        ));
    }
    None
}

fn source_floor_edge_blocked_by_wall(
    collision: StageCollisionProfile,
    edge: SourcePoint,
    ecb: SourceEcb,
    side: SourceFloorEdgeSide,
) -> bool {
    match side {
        SourceFloorEdgeSide::Left => {
            let start = SourcePoint {
                x: edge.x + 1.0,
                y: edge.y + 1.0,
            };
            let end = SourcePoint {
                x: edge.x + ecb.right.x - ecb.bottom.x,
                y: edge.y + ecb.right.y - ecb.bottom.y,
            };
            source_check_wall(collision, StageCollisionLineKind::LeftWall, start, end).is_some()
        }
        SourceFloorEdgeSide::Right => {
            let start = SourcePoint {
                x: edge.x - 1.0,
                y: edge.y + 1.0,
            };
            let end = SourcePoint {
                x: edge.x + ecb.left.x - ecb.bottom.x,
                y: edge.y + ecb.left.y - ecb.bottom.y,
            };
            source_check_wall(collision, StageCollisionLineKind::RightWall, start, end).is_some()
        }
    }
}

fn source_for_each_ceiling_line(
    collision: StageCollisionProfile,
    mut visit: impl FnMut(usize, &crate::StageCollisionLine),
) {
    let mut visited_any_joint = false;
    for joint in collision.joints {
        source_for_each_line_in_range(
            collision,
            joint.ceiling_start,
            joint.ceiling_count,
            StageCollisionLineKind::Ceiling,
            &mut visit,
        );
        source_for_each_line_in_range(
            collision,
            joint.dynamic_start,
            joint.dynamic_count,
            StageCollisionLineKind::Ceiling,
            &mut visit,
        );
        visited_any_joint = true;
    }
    if !visited_any_joint {
        for (line_id, line) in collision.lines.iter().enumerate() {
            if source_line_is_active_kind(line, StageCollisionLineKind::Ceiling) {
                visit(line_id, line);
            }
        }
    }
}

fn source_for_each_floor_line(
    collision: StageCollisionProfile,
    mut visit: impl FnMut(usize, &crate::StageCollisionLine),
) {
    source_for_each_floor_line_matching_joint(collision, |_| true, &mut visit);
}

fn source_for_each_floor_line_in_sweep(
    collision: StageCollisionProfile,
    previous_bottom: SourcePoint,
    current_bottom: SourcePoint,
    mut visit: impl FnMut(usize, &crate::StageCollisionLine),
) {
    source_for_each_floor_line_matching_joint(
        collision,
        |joint| source_joint_overlaps_sweep(joint, previous_bottom, current_bottom),
        &mut visit,
    );
}

fn source_for_each_floor_line_matching_joint(
    collision: StageCollisionProfile,
    mut joint_matches: impl FnMut(&crate::StageCollisionJoint) -> bool,
    visit: &mut impl FnMut(usize, &crate::StageCollisionLine),
) {
    let mut visited_any_joint = false;
    for joint in collision.joints {
        if !joint_matches(joint) {
            continue;
        }
        source_for_each_line_in_range(
            collision,
            joint.floor_start,
            joint.floor_count,
            StageCollisionLineKind::Floor,
            visit,
        );
        source_for_each_line_in_range(
            collision,
            joint.floor_start,
            joint.floor_count,
            StageCollisionLineKind::SoftFloor,
            visit,
        );
        source_for_each_line_in_range(
            collision,
            joint.dynamic_start,
            joint.dynamic_count,
            StageCollisionLineKind::Floor,
            visit,
        );
        source_for_each_line_in_range(
            collision,
            joint.dynamic_start,
            joint.dynamic_count,
            StageCollisionLineKind::SoftFloor,
            visit,
        );
        visited_any_joint = true;
    }
    if !visited_any_joint {
        for (line_id, line) in collision.lines.iter().enumerate() {
            if source_line_is_active_kind(line, StageCollisionLineKind::Floor)
                || source_line_is_active_kind(line, StageCollisionLineKind::SoftFloor)
            {
                visit(line_id, line);
            }
        }
    }
}

fn source_joint_overlaps_sweep(
    joint: &crate::StageCollisionJoint,
    a: SourcePoint,
    b: SourcePoint,
) -> bool {
    let left = a.x.min(b.x);
    let right = a.x.max(b.x);
    let bottom = a.y.min(b.y);
    let top = a.y.max(b.y);
    let joint_left = milli_to_source_units(joint.left_bound_milli);
    let joint_right = milli_to_source_units(joint.right_bound_milli);
    let joint_bottom = milli_to_source_units(joint.bottom_bound_milli);
    let joint_top = milli_to_source_units(joint.top_bound_milli);

    left <= joint_right && right >= joint_left && bottom <= joint_top && top >= joint_bottom
}

fn source_collect_left_wall_hits(
    collision: StageCollisionProfile,
    hits: &mut SourceWallHits,
    previous_root: SourcePoint,
    current_root: SourcePoint,
    previous_ecb: SourceEcb,
    current_ecb: SourceEcb,
    is_ecb_tiny: bool,
    floor_id: Option<usize>,
) -> bool {
    let trace_left_wall = std::env::var_os("MOLE_TRACE_LEFT_WALL").is_some();
    let exclusions = source_left_wall_floor_exclusions(collision, floor_id);

    let previous_right = source_ecb_world_point(previous_root, previous_ecb.right);
    let current_right = source_ecb_world_point(current_root, current_ecb.right);
    let previous_bottom = source_ecb_world_point(previous_root, previous_ecb.bottom);
    let current_bottom = source_ecb_world_point(current_root, current_ecb.bottom);
    let previous_top = source_ecb_world_point(previous_root, previous_ecb.top);
    let current_top = source_ecb_world_point(current_root, current_ecb.top);

    let wall_hug = source_collect_left_wall_hit(
        collision,
        hits,
        "right_sweep",
        previous_right,
        current_right,
        trace_left_wall,
    );
    source_collect_left_wall_hit_excluding(
        collision,
        hits,
        "bottom_sweep",
        previous_bottom,
        current_bottom,
        trace_left_wall,
        exclusions,
    );
    source_collect_left_wall_hit(
        collision,
        hits,
        "top_sweep",
        previous_top,
        current_top,
        trace_left_wall,
    );
    source_collect_left_wall_hit_excluding(
        collision,
        hits,
        "bottom_right_edge",
        current_bottom,
        current_right,
        trace_left_wall,
        exclusions,
    );
    if !is_ecb_tiny {
        source_collect_left_wall_swept_edge_hit_excluding(
            collision,
            hits,
            "bottom_right_swept_edge",
            previous_right,
            previous_bottom,
            current_right,
            current_bottom,
            trace_left_wall,
            exclusions,
        );
    }
    source_collect_left_wall_hit(
        collision,
        hits,
        "top_right_edge",
        current_top,
        current_right,
        trace_left_wall,
    );
    if !is_ecb_tiny {
        source_collect_left_wall_swept_edge_hit(
            collision,
            hits,
            "top_right_swept_edge",
            previous_top,
            previous_right,
            current_top,
            current_right,
            trace_left_wall,
        );
    }
    wall_hug
}

fn source_collect_right_wall_hits(
    collision: StageCollisionProfile,
    hits: &mut SourceWallHits,
    previous_root: SourcePoint,
    current_root: SourcePoint,
    previous_ecb: SourceEcb,
    current_ecb: SourceEcb,
    is_ecb_tiny: bool,
    floor_id: Option<usize>,
) -> bool {
    let exclusions = source_right_wall_floor_exclusions(collision, floor_id);
    let previous_left = source_ecb_world_point(previous_root, previous_ecb.left);
    let current_left = source_ecb_world_point(current_root, current_ecb.left);
    let previous_bottom = source_ecb_world_point(previous_root, previous_ecb.bottom);
    let current_bottom = source_ecb_world_point(current_root, current_ecb.bottom);
    let previous_top = source_ecb_world_point(previous_root, previous_ecb.top);
    let current_top = source_ecb_world_point(current_root, current_ecb.top);

    let wall_hug = source_collect_right_wall_hit(collision, hits, previous_left, current_left);
    source_collect_right_wall_hit_excluding(
        collision,
        hits,
        previous_bottom,
        current_bottom,
        exclusions,
    );
    source_collect_right_wall_hit(collision, hits, previous_top, current_top);
    source_collect_right_wall_hit_excluding(
        collision,
        hits,
        current_bottom,
        current_left,
        exclusions,
    );
    if !is_ecb_tiny {
        source_collect_right_wall_swept_edge_hit_excluding(
            collision,
            hits,
            previous_bottom,
            previous_left,
            current_bottom,
            current_left,
            exclusions,
        );
    }
    source_collect_right_wall_hit(collision, hits, current_top, current_left);
    if !is_ecb_tiny {
        source_collect_right_wall_swept_edge_hit(
            collision,
            hits,
            previous_left,
            previous_top,
            current_left,
            current_top,
        );
    }
    wall_hug
}

fn source_collect_ground_left_wall_hits(
    collision: StageCollisionProfile,
    hits: &mut SourceWallHits,
    previous_root: SourcePoint,
    current_root: SourcePoint,
    previous_ecb: SourceEcb,
    current_ecb: SourceEcb,
    is_ecb_tiny: bool,
    floor_id: Option<usize>,
) -> bool {
    let trace_left_wall = std::env::var_os("MOLE_TRACE_LEFT_WALL").is_some();
    let exclusions = source_left_wall_floor_exclusions(collision, floor_id);

    let previous_right = source_ecb_world_point(previous_root, previous_ecb.right);
    let current_right = source_ecb_world_point(current_root, current_ecb.right);
    let previous_bottom = source_ecb_world_point(previous_root, previous_ecb.bottom);
    let current_bottom = source_ecb_world_point(current_root, current_ecb.bottom);
    let previous_top = source_ecb_world_point(previous_root, previous_ecb.top);
    let current_top = source_ecb_world_point(current_root, current_ecb.top);

    let wall_hug = source_collect_left_wall_hit(
        collision,
        hits,
        "ground_right_sweep",
        previous_right,
        current_right,
        trace_left_wall,
    );
    source_collect_left_wall_hit_excluding(
        collision,
        hits,
        "ground_bottom_sweep",
        previous_bottom,
        current_bottom,
        trace_left_wall,
        exclusions,
    );
    source_collect_left_wall_hit(
        collision,
        hits,
        "ground_top_sweep",
        previous_top,
        current_top,
        trace_left_wall,
    );
    source_collect_left_wall_hit_excluding(
        collision,
        hits,
        "ground_bottom_right_edge",
        current_bottom,
        current_right,
        trace_left_wall,
        exclusions,
    );
    if !is_ecb_tiny {
        source_collect_left_wall_swept_edge_hit_excluding(
            collision,
            hits,
            "ground_bottom_right_swept_edge",
            previous_right,
            previous_bottom,
            current_right,
            current_bottom,
            trace_left_wall,
            exclusions,
        );
    }
    source_collect_left_wall_hit(
        collision,
        hits,
        "ground_top_right_edge",
        current_top,
        current_right,
        trace_left_wall,
    );
    if !is_ecb_tiny {
        source_collect_left_wall_swept_edge_hit_excluding(
            collision,
            hits,
            "ground_top_right_swept_edge",
            previous_top,
            previous_right,
            current_top,
            current_right,
            trace_left_wall,
            SourceWallExclusions::default(),
        );
    }
    wall_hug
}

fn source_collect_ground_right_wall_hits(
    collision: StageCollisionProfile,
    hits: &mut SourceWallHits,
    previous_root: SourcePoint,
    current_root: SourcePoint,
    previous_ecb: SourceEcb,
    current_ecb: SourceEcb,
    is_ecb_tiny: bool,
    floor_id: Option<usize>,
) -> bool {
    let exclusions = source_right_wall_floor_exclusions(collision, floor_id);
    let previous_left = source_ecb_world_point(previous_root, previous_ecb.left);
    let current_left = source_ecb_world_point(current_root, current_ecb.left);
    let previous_bottom = source_ecb_world_point(previous_root, previous_ecb.bottom);
    let current_bottom = source_ecb_world_point(current_root, current_ecb.bottom);
    let previous_top = source_ecb_world_point(previous_root, previous_ecb.top);
    let current_top = source_ecb_world_point(current_root, current_ecb.top);

    let wall_hug = source_collect_right_wall_hit(collision, hits, previous_left, current_left);
    source_collect_right_wall_hit_excluding(
        collision,
        hits,
        previous_bottom,
        current_bottom,
        exclusions,
    );
    source_collect_right_wall_hit(collision, hits, previous_top, current_top);
    source_collect_right_wall_hit_excluding(
        collision,
        hits,
        current_bottom,
        current_left,
        exclusions,
    );
    if !is_ecb_tiny {
        source_collect_right_wall_swept_edge_hit_excluding(
            collision,
            hits,
            previous_bottom,
            previous_left,
            current_bottom,
            current_left,
            exclusions,
        );
    }
    source_collect_right_wall_hit(collision, hits, current_top, current_left);
    if !is_ecb_tiny {
        source_collect_right_wall_swept_edge_hit(
            collision,
            hits,
            previous_left,
            previous_top,
            current_left,
            current_top,
        );
    }
    wall_hug
}

fn source_collect_left_wall_hit(
    collision: StageCollisionProfile,
    hits: &mut SourceWallHits,
    label: &str,
    start: SourcePoint,
    end: SourcePoint,
    trace_left_wall: bool,
) -> bool {
    let Some(line_id) = source_check_wall(collision, StageCollisionLineKind::LeftWall, start, end)
    else {
        return false;
    };
    if trace_left_wall {
        eprintln!(
            "left_wall hit {label} line_id={line_id} start=({:.6},{:.6}) end=({:.6},{:.6})",
            start.x, start.y, end.x, end.y
        );
    }
    source_wall_hits_add(collision, hits, line_id);
    true
}

fn source_collect_left_wall_hit_excluding(
    collision: StageCollisionProfile,
    hits: &mut SourceWallHits,
    label: &str,
    start: SourcePoint,
    end: SourcePoint,
    trace_left_wall: bool,
    exclusions: SourceWallExclusions,
) -> bool {
    let Some(line_id) = source_check_wall(collision, StageCollisionLineKind::LeftWall, start, end)
    else {
        return false;
    };
    if exclusions.contains(line_id) {
        return false;
    }
    if trace_left_wall {
        eprintln!(
            "left_wall hit {label} line_id={line_id} start=({:.6},{:.6}) end=({:.6},{:.6})",
            start.x, start.y, end.x, end.y
        );
    }
    source_wall_hits_add(collision, hits, line_id);
    true
}

fn source_collect_left_wall_swept_edge_hit(
    collision: StageCollisionProfile,
    hits: &mut SourceWallHits,
    label: &str,
    previous_start: SourcePoint,
    previous_end: SourcePoint,
    current_start: SourcePoint,
    current_end: SourcePoint,
    trace_left_wall: bool,
) {
    let Some(hit) = source_check_swept_wall_edge(
        collision,
        StageCollisionLineKind::LeftWall,
        previous_start,
        previous_end,
        current_start,
        current_end,
    ) else {
        return;
    };
    let line_id = hit.line_id;
    if trace_left_wall {
        eprintln!(
            "left_wall hit {label} line_id={line_id} prev_start=({:.6},{:.6}) prev_end=({:.6},{:.6}) current_start=({:.6},{:.6}) current_end=({:.6},{:.6})",
            previous_start.x,
            previous_start.y,
            previous_end.x,
            previous_end.y,
            current_start.x,
            current_start.y,
            current_end.x,
            current_end.y
        );
    }
    source_wall_hits_add(collision, hits, line_id);
}

fn source_collect_left_wall_swept_edge_hit_excluding(
    collision: StageCollisionProfile,
    hits: &mut SourceWallHits,
    label: &str,
    previous_start: SourcePoint,
    previous_end: SourcePoint,
    current_start: SourcePoint,
    current_end: SourcePoint,
    trace_left_wall: bool,
    exclusions: SourceWallExclusions,
) {
    let Some(hit) = source_check_swept_wall_edge(
        collision,
        StageCollisionLineKind::LeftWall,
        previous_start,
        previous_end,
        current_start,
        current_end,
    ) else {
        return;
    };
    let line_id = hit.line_id;
    if exclusions.contains(line_id) {
        return;
    }
    if trace_left_wall {
        eprintln!(
            "left_wall hit {label} line_id={line_id} prev_start=({:.6},{:.6}) prev_end=({:.6},{:.6}) current_start=({:.6},{:.6}) current_end=({:.6},{:.6})",
            previous_start.x,
            previous_start.y,
            previous_end.x,
            previous_end.y,
            current_start.x,
            current_start.y,
            current_end.x,
            current_end.y
        );
    }
    source_wall_hits_add(collision, hits, line_id);
}

fn source_collect_right_wall_hit(
    collision: StageCollisionProfile,
    hits: &mut SourceWallHits,
    start: SourcePoint,
    end: SourcePoint,
) -> bool {
    if let Some(line_id) =
        source_check_wall(collision, StageCollisionLineKind::RightWall, start, end)
    {
        source_wall_hits_add(collision, hits, line_id);
        return true;
    }
    false
}

fn source_collect_right_wall_hit_excluding(
    collision: StageCollisionProfile,
    hits: &mut SourceWallHits,
    start: SourcePoint,
    end: SourcePoint,
    exclusions: SourceWallExclusions,
) -> bool {
    if let Some(line_id) =
        source_check_wall(collision, StageCollisionLineKind::RightWall, start, end)
    {
        if exclusions.contains(line_id) {
            return false;
        }
        source_wall_hits_add(collision, hits, line_id);
        return true;
    }
    false
}

fn source_collect_right_wall_swept_edge_hit(
    collision: StageCollisionProfile,
    hits: &mut SourceWallHits,
    previous_start: SourcePoint,
    previous_end: SourcePoint,
    current_start: SourcePoint,
    current_end: SourcePoint,
) {
    if let Some(hit) = source_check_swept_wall_edge(
        collision,
        StageCollisionLineKind::RightWall,
        previous_start,
        previous_end,
        current_start,
        current_end,
    ) {
        let line_id = hit.line_id;
        source_wall_hits_add(collision, hits, line_id);
    }
}

fn source_collect_right_wall_swept_edge_hit_excluding(
    collision: StageCollisionProfile,
    hits: &mut SourceWallHits,
    previous_start: SourcePoint,
    previous_end: SourcePoint,
    current_start: SourcePoint,
    current_end: SourcePoint,
    exclusions: SourceWallExclusions,
) {
    if let Some(hit) = source_check_swept_wall_edge(
        collision,
        StageCollisionLineKind::RightWall,
        previous_start,
        previous_end,
        current_start,
        current_end,
    ) {
        let line_id = hit.line_id;
        if !exclusions.contains(line_id) {
            source_wall_hits_add(collision, hits, line_id);
        }
    }
}

fn source_ecb_world_point(root: SourcePoint, local: SourcePoint) -> SourcePoint {
    SourcePoint {
        x: root.x + local.x,
        y: root.y + local.y,
    }
}

fn source_wall_hits_add(
    collision: StageCollisionProfile,
    hits: &mut SourceWallHits,
    line_id: usize,
) {
    for existing in hits.iter() {
        if existing == line_id || source_lines_connected(collision, existing, line_id) {
            return;
        }
    }
    debug_assert!(hits.len < SOURCE_COLL_WALL_ID_MAX);
    if hits.len < SOURCE_COLL_WALL_ID_MAX {
        hits.ids[hits.len] = line_id;
        hits.len += 1;
    }
}

fn source_check_wall(
    collision: StageCollisionProfile,
    kind: StageCollisionLineKind,
    start: SourcePoint,
    end: SourcePoint,
) -> Option<usize> {
    let mut best = None;
    let mut best_distance = f32::INFINITY;
    source_for_each_wall_line(collision, kind, |line_id, _line| {
        let Some(wall) = source_wall_line(collision, line_id) else {
            return;
        };
        let wall_start = SourcePoint {
            x: wall.x0,
            y: wall.y0,
        };
        let wall_end = SourcePoint {
            x: wall.x1,
            y: wall.y1,
        };
        let intersection = if (wall_start.x - wall_end.x).abs() > 0.0001 {
            source_mp_line_intersection(wall_start, wall_end, start, end)
        } else {
            let direction_allowed = match kind {
                StageCollisionLineKind::LeftWall => start.x <= end.x,
                StageCollisionLineKind::RightWall => start.x >= end.x,
                _ => false,
            };
            if direction_allowed {
                source_mp_line_intersection_v(wall_start.x, wall_start.y, wall_end.y, start, end)
            } else {
                None
            }
        };
        let Some(intersection) = intersection else {
            return;
        };
        let distance = source_distance_sq(start, intersection);
        if distance < best_distance {
            best_distance = distance;
            best = Some(line_id);
        }
    });
    best
}

fn source_check_swept_wall_edge(
    collision: StageCollisionProfile,
    kind: StageCollisionLineKind,
    previous_start: SourcePoint,
    previous_end: SourcePoint,
    current_start: SourcePoint,
    current_end: SourcePoint,
) -> Option<SourceSweptWallHit> {
    let mut best = None;
    let mut best_distance = f32::INFINITY;
    source_for_each_wall_line(collision, kind, |line_id, line| {
        for vertex in [line.v0_idx, line.v1_idx] {
            let Some((current_x, current_y)) = collision.scaled_vertex(vertex as usize) else {
                continue;
            };
            let Some((previous_x, previous_y)) = collision.previous_vertex(vertex as usize) else {
                continue;
            };
            let current_vertex = SourcePoint {
                x: current_x,
                y: current_y,
            };
            let previous_vertex = SourcePoint {
                x: previous_x,
                y: previous_y,
            };
            let remapped_vertex = source_remap_2d(
                previous_start,
                previous_end,
                current_start,
                current_end,
                previous_vertex,
            );
            let dx = current_vertex.x - remapped_vertex.x;
            let dy = current_vertex.y - remapped_vertex.y;
            if dx * dx + dy * dy <= 0.001 {
                continue;
            }
            let Some(intersection) = source_mp_line_intersection(
                current_start,
                current_end,
                remapped_vertex,
                current_vertex,
            ) else {
                continue;
            };
            let mut distance = source_distance_sq(previous_vertex, intersection);
            if dx * (intersection.x - previous_vertex.x) + dy * (intersection.y - previous_vertex.y)
                < 0.0
            {
                distance = -distance;
            }
            if distance < best_distance {
                best_distance = distance;
                best = Some(SourceSweptWallHit { line_id });
            }
        }
    });
    best
}

fn source_for_each_wall_line(
    collision: StageCollisionProfile,
    kind: StageCollisionLineKind,
    mut visit: impl FnMut(usize, &crate::StageCollisionLine),
) {
    let mut visited_any_joint = false;
    for joint in collision.joints {
        let (start, count) = match kind {
            StageCollisionLineKind::LeftWall => (joint.left_wall_start, joint.left_wall_count),
            StageCollisionLineKind::RightWall => (joint.right_wall_start, joint.right_wall_count),
            _ => continue,
        };
        source_for_each_line_in_range(collision, start, count, kind, &mut visit);
        source_for_each_line_in_range(
            collision,
            joint.dynamic_start,
            joint.dynamic_count,
            kind,
            &mut visit,
        );
        visited_any_joint = true;
    }
    if !visited_any_joint {
        for (line_id, line) in collision.lines.iter().enumerate() {
            if source_line_is_active_kind(line, kind) {
                visit(line_id, line);
            }
        }
    }
}

fn source_for_each_line_in_range(
    collision: StageCollisionProfile,
    start: i16,
    count: i16,
    kind: StageCollisionLineKind,
    visit: &mut impl FnMut(usize, &crate::StageCollisionLine),
) {
    let Some(start) = source_index_from_i16(start) else {
        return;
    };
    let Ok(count) = usize::try_from(count) else {
        return;
    };
    for line_id in start..start.saturating_add(count) {
        let Some(line) = collision.lines.get(line_id) else {
            continue;
        };
        if source_line_is_active_kind(line, kind) {
            visit(line_id, line);
        }
    }
}

fn source_line_is_active_kind(
    line: &crate::StageCollisionLine,
    kind: StageCollisionLineKind,
) -> bool {
    const LINE_FLAG_EMPTY: u16 = 0x0080;
    line.kind == kind && line.hi_flags & LINE_FLAG_EMPTY == 0
}

fn source_remap_2d(
    source_start: SourcePoint,
    source_end: SourcePoint,
    target_start: SourcePoint,
    target_end: SourcePoint,
    point: SourcePoint,
) -> SourcePoint {
    let dx = (source_end.x - source_start.x) as f64;
    let dy = (source_end.y - source_start.y) as f64;
    let point_dx = point.x - source_start.x;
    let point_dy = point.y - source_start.y;
    let dist_sq = dy * dy + dx * dx;
    if dist_sq.abs() > 0.0001 {
        let t = ((dy * point_dy as f64 + dx * point_dx as f64) / dist_sq).clamp(0.0, 1.0);
        SourcePoint {
            x: point.x
                + ((1.0 - t) as f32) * (target_start.x - source_start.x)
                + (t as f32) * (target_end.x - source_end.x),
            y: point.y
                + ((1.0 - t) as f32) * (target_start.y - source_start.y)
                + (t as f32) * (target_end.y - source_end.y),
        }
    } else {
        SourcePoint {
            x: point.x + (target_start.x - source_start.x) + (target_end.x - source_start.x),
            y: point.y + (target_start.y - source_start.y) + (target_end.y - source_start.y),
        }
    }
}

fn source_correct_left_wall(
    collision: StageCollisionProfile,
    root: &mut SourcePoint,
    ecb: SourceEcb,
    hits: SourceWallHits,
) -> bool {
    source_correct_left_wall_impl(collision, root, ecb, hits, true)
}

fn source_correct_ground_left_wall(
    collision: StageCollisionProfile,
    root: &mut SourcePoint,
    ecb: SourceEcb,
    hits: SourceWallHits,
) -> bool {
    source_correct_left_wall_impl(collision, root, ecb, hits, false)
}

fn source_correct_left_wall_impl(
    collision: StageCollisionProfile,
    root: &mut SourcePoint,
    ecb: SourceEcb,
    hits: SourceWallHits,
    include_ceiling_bridge: bool,
) -> bool {
    let trace_left_wall = std::env::var_os("MOLE_TRACE_LEFT_WALL").is_some();
    let mut best_x = f32::INFINITY;
    for wall_id in hits.iter() {
        if trace_left_wall {
            eprintln!(
                "left_wall start wall_id={wall_id} root=({:.6},{:.6}) ecb top=({:.6},{:.6}) right=({:.6},{:.6}) bottom=({:.6},{:.6})",
                root.x,
                root.y,
                ecb.top.x,
                ecb.top.y,
                ecb.right.x,
                ecb.right.y,
                ecb.bottom.x,
                ecb.bottom.y
            );
        }
        let bottom_world_y = root.y + ecb.bottom.y;
        let top_world_y = root.y + ecb.top.y;

        if let Some(top) = source_left_wall_top(collision, wall_id) {
            if top.y < bottom_world_y {
                if source_project_left_wall(collision, wall_id, top).is_some() {
                    if trace_left_wall {
                        eprintln!("  top_below candidate wall_id={wall_id} x={:.6}", top.x);
                    }
                    best_x = best_x.min(top.x);
                }
                continue;
            }
        }

        if let Some(bottom) = source_left_wall_bottom(collision, wall_id) {
            if bottom.y > top_world_y {
                if source_project_left_wall(collision, wall_id, bottom).is_some() {
                    if trace_left_wall {
                        eprintln!(
                            "  bottom_above candidate wall_id={wall_id} x={:.6}",
                            bottom.x
                        );
                    }
                    best_x = best_x.min(bottom.x);
                }
                continue;
            }
        }

        for local in [ecb.bottom, ecb.right, ecb.top] {
            let point = source_ecb_world_point(*root, local);
            if let Some(delta_x) = source_project_left_wall(collision, wall_id, point) {
                if trace_left_wall {
                    eprintln!(
                        "  point candidate wall_id={wall_id} point=({:.6},{:.6}) delta={:.6} root_x={:.6}",
                        point.x,
                        point.y,
                        delta_x,
                        root.x + delta_x
                    );
                }
                best_x = best_x.min(root.x + delta_x);
            }
        }

        if include_ceiling_bridge {
            let top_point = source_ecb_world_point(*root, ecb.top);
            if let Some(ceiling_id) =
                source_line_next_non_kind(collision, wall_id, StageCollisionLineKind::LeftWall)
            {
                if source_line_kind(collision, ceiling_id) == Some(StageCollisionLineKind::Ceiling)
                {
                    if let Some(top) = source_left_wall_top(collision, wall_id) {
                        if top_point.y > top.y {
                            if let Some(next_wall_id) = source_line_prev_non_kind(
                                collision,
                                ceiling_id,
                                StageCollisionLineKind::Ceiling,
                            ) {
                                if source_line_kind(collision, next_wall_id)
                                    == Some(StageCollisionLineKind::LeftWall)
                                {
                                    if let Some(normal) =
                                        source_line_normal(collision, next_wall_id)
                                    {
                                        if normal.x.abs() > f32::EPSILON {
                                            let delta_x = (top_point.y - top.y) / -normal.x
                                                * normal.y
                                                + top.x
                                                - top_point.x
                                                - 0.5;
                                            if trace_left_wall {
                                                eprintln!(
                                                    "  ceiling candidate wall_id={wall_id} next_wall_id={next_wall_id} delta={:.6} root_x={:.6}",
                                                    delta_x,
                                                    root.x + delta_x
                                                );
                                            }
                                            best_x = best_x.min(root.x + delta_x);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let mut connected = Some(wall_id);
        while let Some(line_id) = connected {
            if source_line_kind(collision, line_id) != Some(StageCollisionLineKind::LeftWall) {
                break;
            }
            let Some(pos) = source_line_v0(collision, line_id) else {
                break;
            };
            if let Some(candidate) = source_right_edge_root_x_at_wall_vertex(*root, ecb, pos) {
                if trace_left_wall {
                    eprintln!(
                        "  prev-chain candidate line_id={line_id} pos=({:.6},{:.6}) root_x={:.6}",
                        pos.x, pos.y, candidate
                    );
                }
                best_x = best_x.min(candidate);
            } else if pos.y < bottom_world_y {
                break;
            }
            connected = source_line_get_prev(collision, line_id);
        }

        let mut connected = Some(wall_id);
        while let Some(line_id) = connected {
            if source_line_kind(collision, line_id) != Some(StageCollisionLineKind::LeftWall) {
                break;
            }
            let Some(pos) = source_line_v1(collision, line_id) else {
                break;
            };
            if let Some(candidate) = source_right_edge_root_x_at_wall_vertex(*root, ecb, pos) {
                if trace_left_wall {
                    eprintln!(
                        "  next-chain candidate line_id={line_id} pos=({:.6},{:.6}) root_x={:.6}",
                        pos.x, pos.y, candidate
                    );
                }
                best_x = best_x.min(candidate);
            } else if pos.y > top_world_y {
                break;
            }
            connected = source_line_get_next(collision, line_id);
        }
    }

    if trace_left_wall {
        eprintln!(
            "left_wall best root_before_x={:.6} best_x={:.6}",
            root.x, best_x
        );
    }
    if root.x > best_x {
        root.x = best_x;
        return true;
    }
    false
}

fn source_correct_right_wall_air(
    collision: StageCollisionProfile,
    root: &mut SourcePoint,
    ecb: SourceEcb,
    hits: SourceWallHits,
) -> bool {
    source_correct_right_wall_impl(collision, root, ecb, hits, true)
}

fn source_correct_ground_right_wall(
    collision: StageCollisionProfile,
    root: &mut SourcePoint,
    ecb: SourceEcb,
    hits: SourceWallHits,
) -> bool {
    source_correct_right_wall_impl(collision, root, ecb, hits, false)
}

fn source_correct_right_wall_impl(
    collision: StageCollisionProfile,
    root: &mut SourcePoint,
    ecb: SourceEcb,
    hits: SourceWallHits,
    include_ceiling_bridge: bool,
) -> bool {
    let mut best_x = -f32::INFINITY;
    for wall_id in hits.iter() {
        let bottom_world_y = root.y + ecb.bottom.y;
        let top_world_y = root.y + ecb.top.y;

        if let Some(top) = source_right_wall_top(collision, wall_id) {
            if top.y < bottom_world_y {
                if source_project_right_wall(collision, wall_id, top).is_some() {
                    best_x = best_x.max(top.x);
                }
                continue;
            }
        }

        if let Some(bottom) = source_right_wall_bottom(collision, wall_id) {
            if bottom.y > top_world_y {
                if source_project_right_wall(collision, wall_id, bottom).is_some() {
                    best_x = best_x.max(bottom.x);
                }
                continue;
            }
        }

        for local in [ecb.bottom, ecb.left, ecb.top] {
            let point = source_ecb_world_point(*root, local);
            if let Some(delta_x) = source_project_right_wall(collision, wall_id, point) {
                let candidate = root.x + delta_x;
                best_x = best_x.max(candidate);
            }
        }

        if include_ceiling_bridge {
            let top_point = source_ecb_world_point(*root, ecb.top);
            if let Some(ceiling_id) =
                source_line_prev_non_kind(collision, wall_id, StageCollisionLineKind::RightWall)
            {
                if source_line_kind(collision, ceiling_id) == Some(StageCollisionLineKind::Ceiling)
                {
                    if let Some(top) = source_right_wall_top(collision, wall_id) {
                        if top_point.y > top.y {
                            if let Some(next_wall_id) = source_line_next_non_kind(
                                collision,
                                ceiling_id,
                                StageCollisionLineKind::Ceiling,
                            ) {
                                if source_line_kind(collision, next_wall_id)
                                    == Some(StageCollisionLineKind::RightWall)
                                {
                                    if let Some(normal) =
                                        source_line_normal(collision, next_wall_id)
                                    {
                                        if normal.x.abs() > f32::EPSILON {
                                            let delta_x = (top_point.y - top.y) / normal.x
                                                * -normal.y
                                                + top.x
                                                - top_point.x
                                                + 0.5;
                                            let candidate = root.x + delta_x;
                                            best_x = best_x.max(candidate);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let mut connected = Some(wall_id);
        while let Some(line_id) = connected {
            if source_line_kind(collision, line_id) != Some(StageCollisionLineKind::RightWall) {
                break;
            }
            let Some(pos) = source_line_v1(collision, line_id) else {
                break;
            };
            if let Some(candidate) = source_left_edge_root_x_at_wall_vertex(*root, ecb, pos) {
                best_x = best_x.max(candidate);
            } else if pos.y < bottom_world_y {
                break;
            }
            connected = source_line_get_next(collision, line_id);
        }

        let mut connected = Some(wall_id);
        while let Some(line_id) = connected {
            if source_line_kind(collision, line_id) != Some(StageCollisionLineKind::RightWall) {
                break;
            }
            let Some(pos) = source_line_v0(collision, line_id) else {
                break;
            };
            if let Some(candidate) = source_left_edge_root_x_at_wall_vertex(*root, ecb, pos) {
                best_x = best_x.max(candidate);
            } else if pos.y > top_world_y {
                break;
            }
            connected = source_line_get_prev(collision, line_id);
        }
    }

    if root.x < best_x {
        root.x = best_x;
        return true;
    }
    false
}

fn source_right_edge_root_x_at_wall_vertex(
    root: SourcePoint,
    ecb: SourceEcb,
    pos: SourcePoint,
) -> Option<f32> {
    let bottom_world_y = root.y + ecb.bottom.y;
    let right_world_y = root.y + ecb.right.y;
    let top_world_y = root.y + ecb.top.y;
    if bottom_world_y <= pos.y && pos.y <= right_world_y {
        let denominator = ecb.right.y - ecb.bottom.y;
        if denominator.abs() <= f32::EPSILON {
            return None;
        }
        let local_x =
            ecb.bottom.x + (ecb.right.x - ecb.bottom.x) * ((pos.y - bottom_world_y) / denominator);
        return Some(pos.x - local_x);
    }
    if right_world_y <= pos.y && pos.y <= top_world_y {
        let denominator = ecb.right.y - ecb.top.y;
        if denominator.abs() <= f32::EPSILON {
            return None;
        }
        let local_x = ecb.top.x + (ecb.right.x - ecb.top.x) * ((pos.y - top_world_y) / denominator);
        return Some(pos.x - local_x);
    }
    None
}

fn source_left_edge_root_x_at_wall_vertex(
    root: SourcePoint,
    ecb: SourceEcb,
    pos: SourcePoint,
) -> Option<f32> {
    let bottom_world_y = root.y + ecb.bottom.y;
    let left_world_y = root.y + ecb.left.y;
    let top_world_y = root.y + ecb.top.y;
    if bottom_world_y <= pos.y && pos.y <= left_world_y {
        let denominator = ecb.left.y - ecb.bottom.y;
        if denominator.abs() <= f32::EPSILON {
            return None;
        }
        let local_x =
            ecb.bottom.x + (ecb.left.x - ecb.bottom.x) * ((pos.y - bottom_world_y) / denominator);
        return Some(pos.x - local_x);
    }
    if left_world_y <= pos.y && pos.y <= top_world_y {
        let denominator = ecb.left.y - ecb.top.y;
        if denominator.abs() <= f32::EPSILON {
            return None;
        }
        let local_x = ecb.top.x + (ecb.left.x - ecb.top.x) * ((pos.y - top_world_y) / denominator);
        return Some(pos.x - local_x);
    }
    None
}

fn source_project_left_wall(
    collision: StageCollisionProfile,
    mut wall_id: usize,
    point: SourcePoint,
) -> Option<f32> {
    let mut y = point.y;
    let mut direction = 0_i8;
    loop {
        let line = source_wall_line(collision, wall_id)?;
        if y < line.y0 {
            if direction != 1 {
                if let Some(prev_id) = source_line_get_prev(collision, wall_id) {
                    if source_line_kind(collision, prev_id)
                        == Some(StageCollisionLineKind::LeftWall)
                    {
                        wall_id = prev_id;
                        direction = -1;
                        continue;
                    }
                }
            }
            if y - line.y0 < -SOURCE_COLL_LINE_TOLERANCE {
                return None;
            }
            y = line.y0;
            return source_wall_delta_x_at_y(line, point, y);
        }
        if y > line.y1 {
            if direction != -1 {
                if let Some(next_id) = source_line_get_next(collision, wall_id) {
                    if source_line_kind(collision, next_id)
                        == Some(StageCollisionLineKind::LeftWall)
                    {
                        wall_id = next_id;
                        direction = 1;
                        continue;
                    }
                }
            }
            if y - line.y1 > SOURCE_COLL_LINE_TOLERANCE {
                return None;
            }
            y = line.y1;
            return source_wall_delta_x_at_y(line, point, y);
        }
        return source_wall_delta_x_at_y(line, point, y);
    }
}

fn source_project_right_wall(
    collision: StageCollisionProfile,
    mut wall_id: usize,
    point: SourcePoint,
) -> Option<f32> {
    let mut y = point.y;
    let mut direction = 0_i8;
    loop {
        let line = source_wall_line(collision, wall_id)?;
        if point.y > line.y0 {
            if direction != -1 {
                if let Some(prev_id) = source_line_get_prev(collision, wall_id) {
                    if source_line_kind(collision, prev_id)
                        == Some(StageCollisionLineKind::RightWall)
                    {
                        wall_id = prev_id;
                        direction = 1;
                        continue;
                    }
                }
                if point.y - line.y0 > SOURCE_COLL_LINE_TOLERANCE {
                    return None;
                }
            }
            y = line.y0;
            return source_wall_delta_x_at_y(line, point, y);
        }
        if point.y < line.y1 {
            if direction != 1 {
                if let Some(next_id) = source_line_get_next(collision, wall_id) {
                    if source_line_kind(collision, next_id)
                        == Some(StageCollisionLineKind::RightWall)
                    {
                        wall_id = next_id;
                        direction = -1;
                        continue;
                    }
                }
                if point.y - line.y1 < -SOURCE_COLL_LINE_TOLERANCE {
                    return None;
                }
            }
            y = line.y1;
            return source_wall_delta_x_at_y(line, point, y);
        }
        return source_wall_delta_x_at_y(line, point, y);
    }
}

fn source_wall_delta_x_at_y(line: SourceWallLine, point: SourcePoint, y: f32) -> Option<f32> {
    if (line.y1 - line.y0).abs() <= f32::EPSILON {
        return None;
    }
    let x = line.x0 + (line.x1 - line.x0) * ((y - line.y0) / (line.y1 - line.y0));
    Some(x - point.x)
}

fn source_lines_connected(
    collision: StageCollisionProfile,
    start_line_id: usize,
    target_line_id: usize,
) -> bool {
    let Some(kind) = source_line_kind(collision, start_line_id) else {
        return false;
    };
    let mut line_id = source_line_get_next(collision, start_line_id);
    while let Some(current_id) = line_id {
        if current_id == target_line_id {
            return true;
        }
        if source_line_kind(collision, current_id) != Some(kind) {
            break;
        }
        line_id = source_line_get_next(collision, current_id);
    }

    let mut line_id = source_line_get_prev(collision, start_line_id);
    while let Some(current_id) = line_id {
        if current_id == target_line_id {
            return true;
        }
        if source_line_kind(collision, current_id) != Some(kind) {
            break;
        }
        line_id = source_line_get_prev(collision, current_id);
    }
    false
}

fn source_line_get_next(collision: StageCollisionProfile, line_id: usize) -> Option<usize> {
    let line = collision.lines.get(line_id)?;
    if let Some(next_id) = source_index_from_i16(line.next_id1) {
        if source_line_endpoint_close(collision, line.v1_idx, collision.lines.get(next_id)?.v0_idx)
        {
            return Some(next_id);
        }
    }
    source_index_from_i16(line.next_id0)
}

fn source_line_get_prev(collision: StageCollisionProfile, line_id: usize) -> Option<usize> {
    let line = collision.lines.get(line_id)?;
    if let Some(prev_id) = source_index_from_i16(line.prev_id1) {
        if source_line_endpoint_close(collision, line.v0_idx, collision.lines.get(prev_id)?.v1_idx)
        {
            return Some(prev_id);
        }
    }
    source_index_from_i16(line.prev_id0)
}

fn source_line_next_non_kind(
    collision: StageCollisionProfile,
    line_id: usize,
    kind: StageCollisionLineKind,
) -> Option<usize> {
    let mut next_id = source_line_get_next(collision, line_id);
    while let Some(current_id) = next_id {
        if current_id == line_id {
            return None;
        }
        if source_line_kind(collision, current_id) != Some(kind) {
            return Some(current_id);
        }
        next_id = source_line_get_next(collision, current_id);
    }
    None
}

fn source_line_prev_non_kind(
    collision: StageCollisionProfile,
    line_id: usize,
    kind: StageCollisionLineKind,
) -> Option<usize> {
    let mut prev_id = source_line_get_prev(collision, line_id);
    while let Some(current_id) = prev_id {
        if current_id == line_id {
            return None;
        }
        if source_line_kind(collision, current_id) != Some(kind) {
            return Some(current_id);
        }
        prev_id = source_line_get_prev(collision, current_id);
    }
    None
}

fn source_right_wall_floor_exclusions(
    collision: StageCollisionProfile,
    floor_id: Option<usize>,
) -> SourceWallExclusions {
    let line_id1 = source_floor_x0_next0_non_floor(collision, floor_id);
    let line_id2 = source_floor_prev_alt_floor(collision, floor_id)
        .and_then(|line_id| source_floor_x0_next0_non_floor(collision, Some(line_id)));
    SourceWallExclusions {
        ids: [line_id1, line_id2],
    }
}

fn source_left_wall_floor_exclusions(
    collision: StageCollisionProfile,
    floor_id: Option<usize>,
) -> SourceWallExclusions {
    let line_id1 = source_floor_x0_prev0_non_floor(collision, floor_id);
    let line_id2 = source_floor_next_alt_floor(collision, floor_id)
        .and_then(|line_id| source_floor_x0_prev0_non_floor(collision, Some(line_id)));
    SourceWallExclusions {
        ids: [line_id1, line_id2],
    }
}

fn source_floor_x0_next0_non_floor(
    collision: StageCollisionProfile,
    floor_id: Option<usize>,
) -> Option<usize> {
    let mut line_id = source_index_from_i16(collision.lines.get(floor_id?)?.next_id0);
    while let Some(current_id) = line_id {
        if !source_line_kind_is_floor(source_line_kind(collision, current_id)?) {
            return Some(current_id);
        }
        line_id = source_index_from_i16(collision.lines.get(current_id)?.next_id0);
    }
    None
}

fn source_floor_x0_prev0_non_floor(
    collision: StageCollisionProfile,
    floor_id: Option<usize>,
) -> Option<usize> {
    let mut line_id = source_index_from_i16(collision.lines.get(floor_id?)?.prev_id0);
    while let Some(current_id) = line_id {
        if !source_line_kind_is_floor(source_line_kind(collision, current_id)?) {
            return Some(current_id);
        }
        line_id = source_index_from_i16(collision.lines.get(current_id)?.prev_id0);
    }
    None
}

fn source_floor_next_alt_floor(
    collision: StageCollisionProfile,
    floor_id: Option<usize>,
) -> Option<usize> {
    let floor_id = floor_id?;
    let alt = source_index_from_i16(collision.lines.get(floor_id)?.next_id1);
    let mut line_id = source_line_get_next(collision, floor_id);
    while let Some(current_id) = line_id {
        if !source_line_kind_is_floor(source_line_kind(collision, current_id)?) {
            return None;
        }
        if Some(current_id) == alt {
            return Some(current_id);
        }
        line_id = source_line_get_next(collision, current_id);
    }
    None
}

fn source_floor_prev_alt_floor(
    collision: StageCollisionProfile,
    floor_id: Option<usize>,
) -> Option<usize> {
    let floor_id = floor_id?;
    let alt = source_index_from_i16(collision.lines.get(floor_id)?.prev_id1);
    let mut line_id = source_line_get_prev(collision, floor_id);
    while let Some(current_id) = line_id {
        if !source_line_kind_is_floor(source_line_kind(collision, current_id)?) {
            return None;
        }
        if Some(current_id) == alt {
            return Some(current_id);
        }
        line_id = source_line_get_prev(collision, current_id);
    }
    None
}

fn source_line_endpoint_close(
    collision: StageCollisionProfile,
    vertex_a: u16,
    vertex_b: u16,
) -> bool {
    let Some((x0, y0)) = collision.scaled_vertex(vertex_a as usize) else {
        return false;
    };
    let Some((x1, y1)) = collision.scaled_vertex(vertex_b as usize) else {
        return false;
    };
    let dx = x0 - x1;
    let dy = y0 - y1;
    dx * dx + dy * dy < 4.0
}

fn source_index_from_i16(index: i16) -> Option<usize> {
    (index >= 0).then_some(index as usize)
}

fn source_line_kind(
    collision: StageCollisionProfile,
    line_id: usize,
) -> Option<StageCollisionLineKind> {
    collision.lines.get(line_id).map(|line| line.kind)
}

fn source_wall_line(collision: StageCollisionProfile, line_id: usize) -> Option<SourceWallLine> {
    let line = collision.scaled_line(line_id)?;
    Some(SourceWallLine {
        x0: line.x0,
        y0: line.y0,
        x1: line.x1,
        y1: line.y1,
    })
}

fn source_line_v0(collision: StageCollisionProfile, line_id: usize) -> Option<SourcePoint> {
    let line = source_wall_line(collision, line_id)?;
    Some(SourcePoint {
        x: line.x0,
        y: line.y0,
    })
}

fn source_line_v1(collision: StageCollisionProfile, line_id: usize) -> Option<SourcePoint> {
    let line = source_wall_line(collision, line_id)?;
    Some(SourcePoint {
        x: line.x1,
        y: line.y1,
    })
}

fn source_line_normal(collision: StageCollisionProfile, line_id: usize) -> Option<SourcePoint> {
    let line = source_wall_line(collision, line_id)?;
    let x = -(line.y1 - line.y0);
    let y = line.x1 - line.x0;
    let len = (x * x + y * y).sqrt();
    (len > f32::EPSILON).then_some(SourcePoint {
        x: x / len,
        y: y / len,
    })
}

fn source_left_wall_top(collision: StageCollisionProfile, line_id: usize) -> Option<SourcePoint> {
    let mut current_id = line_id;
    loop {
        let good_id = current_id;
        match source_line_get_next(collision, current_id) {
            Some(next_id)
                if source_line_kind(collision, next_id)
                    == Some(StageCollisionLineKind::LeftWall) =>
            {
                current_id = next_id;
            }
            _ => return source_line_v1(collision, good_id),
        }
    }
}

fn source_left_wall_bottom(
    collision: StageCollisionProfile,
    line_id: usize,
) -> Option<SourcePoint> {
    let mut current_id = line_id;
    loop {
        let good_id = current_id;
        match source_line_get_prev(collision, current_id) {
            Some(prev_id)
                if source_line_kind(collision, prev_id)
                    == Some(StageCollisionLineKind::LeftWall) =>
            {
                current_id = prev_id;
            }
            _ => return source_line_v0(collision, good_id),
        }
    }
}

fn source_right_wall_top(collision: StageCollisionProfile, line_id: usize) -> Option<SourcePoint> {
    let mut current_id = line_id;
    loop {
        let good_id = current_id;
        match source_line_get_prev(collision, current_id) {
            Some(prev_id)
                if source_line_kind(collision, prev_id)
                    == Some(StageCollisionLineKind::RightWall) =>
            {
                current_id = prev_id;
            }
            _ => return source_line_v0(collision, good_id),
        }
    }
}

fn source_right_wall_bottom(
    collision: StageCollisionProfile,
    line_id: usize,
) -> Option<SourcePoint> {
    let mut current_id = line_id;
    loop {
        let good_id = current_id;
        match source_line_get_next(collision, current_id) {
            Some(next_id)
                if source_line_kind(collision, next_id)
                    == Some(StageCollisionLineKind::RightWall) =>
            {
                current_id = next_id;
            }
            _ => return source_line_v1(collision, good_id),
        }
    }
}

fn source_mp_line_intersection(
    a0: SourcePoint,
    a1: SourcePoint,
    b0: SourcePoint,
    b1: SourcePoint,
) -> Option<SourcePoint> {
    let (a0x, a0y, a1x, a1y) = (a0.x as f64, a0.y as f64, a1.x as f64, a1.y as f64);
    let (b0x, b0y, b1x, b1y) = (b0.x as f64, b0.y as f64, b1.x as f64, b1.y as f64);

    if a0x <= a1x {
        if (b0x < a0x && b1x < a0x) || (a1x < b0x && a1x < b1x) {
            return None;
        }
    } else if (b0x < a1x && b1x < a1x) || (a0x < b0x && a0x < b1x) {
        return None;
    }

    if a0y <= a1y {
        if (b0y < a0y && b1y < a0y) || (a1y < b0y && a1y < b1y) {
            return None;
        }
    } else if (b0y < a1y && b1y < a1y) || (a0y < b0y && a0y < b1y) {
        return None;
    }

    let ah = a1y - a0y;
    let aw = a1x - a0x;
    let d0x = b0x - a0x;
    let d0y = b0y - a0y;
    let hs_b0_a = aw * d0y - ah * d0x;
    let mut b1_below_a = false;
    let mut b2_above_a = false;
    if hs_b0_a < 0.0 {
        if hs_b0_a < -SOURCE_COLL_INTERSECTION_TOLERANCE {
            return None;
        }
        b1_below_a = true;
    }

    let d1x = b1x - a1x;
    let d1y = b1y - a1y;
    let hs_b1_a = aw * d1y - ah * d1x;
    if hs_b1_a > 0.0 {
        if hs_b1_a > SOURCE_COLL_INTERSECTION_TOLERANCE {
            return None;
        }
        b2_above_a = true;
    }

    if hs_b0_a == 0.0 && hs_b1_a == 0.0 {
        return None;
    }

    let det = d0x * d1y - d0y * d1x;
    if det < hs_b0_a {
        if det < hs_b1_a {
            return None;
        }
    } else if det > hs_b0_a && det > hs_b1_a {
        return None;
    }

    let bw = b1x - b0x;
    let bh = b1y - b0y;
    if !((bw == 0.0 && bh == 0.0) || (b1_below_a && b2_above_a) || (hs_b0_a >= 0.0 && b2_above_a)) {
        let area = bw * ah - bh * aw;
        if area.abs() <= SOURCE_COLL_AREA_TOLERANCE {
            return None;
        }
        let t = (bw * d0y - bh * d0x) / area;
        if t > 0.0 {
            if t < 1.0 {
                return Some(SourcePoint {
                    x: (aw * t + a0x) as f32,
                    y: (ah * t + a0y) as f32,
                });
            }
            return Some(a1);
        }
        return Some(a0);
    }

    None
}

fn source_floor_line_intersection(
    floor_start: SourcePoint,
    floor_end: SourcePoint,
    sweep_start: SourcePoint,
    sweep_end: SourcePoint,
) -> Option<SourcePoint> {
    if (floor_start.y - floor_end.y).abs() <= 0.0001 {
        source_mp_line_intersection_h(
            floor_start.x,
            floor_start.y,
            floor_end.x,
            sweep_start,
            sweep_end,
        )
    } else {
        source_mp_line_intersection(floor_start, floor_end, sweep_start, sweep_end)
    }
}

fn source_mp_line_intersection_h(
    a0x: f32,
    a0y: f32,
    a1x: f32,
    b0: SourcePoint,
    b1: SourcePoint,
) -> Option<SourcePoint> {
    let (min_ax, max_ax) = if a0x < a1x {
        if (b0.x < a0x && b1.x < a0x) || (a1x < b0.x && a1x < b1.x) {
            return None;
        }
        if b0.y - a0y < -0.0001 || b1.y - a0y > 0.0001 {
            return None;
        }
        (a0x, a1x)
    } else {
        if (b0.x < a1x && b1.x < a1x) || (a0x < b0.x && a0x < b1.x) {
            return None;
        }
        if b1.y - a0y < -0.0001 || b0.y - a0y > 0.0001 {
            return None;
        }
        (a1x, a0x)
    };

    let dby = (b1.y - b0.y) as f64;
    let dbx = (b1.x - b0.x) as f64;
    if dby.abs() < 0.0001 {
        return None;
    }

    let mut new_x = dbx / dby * f64::from(a0y - b0.y) + f64::from(b0.x);
    let dx_min = new_x - f64::from(min_ax);
    if dx_min < 0.0 {
        if dx_min < -0.1 {
            return None;
        }
        new_x = f64::from(min_ax);
    }
    let dx_max = new_x - f64::from(max_ax);
    if dx_max > 0.0 {
        if dx_max > 0.1 {
            return None;
        }
        new_x = f64::from(max_ax);
    }

    Some(SourcePoint {
        x: new_x as f32,
        y: a0y,
    })
}

fn source_mp_line_intersection_v(
    a0x: f32,
    a0y: f32,
    a1y: f32,
    b0: SourcePoint,
    b1: SourcePoint,
) -> Option<SourcePoint> {
    let a0x = a0x as f64;
    let a0y = a0y as f64;
    let a1y = a1y as f64;
    let b0x = b0.x as f64;
    let b0y = b0.y as f64;
    let b1x = b1.x as f64;
    let b1y = b1.y as f64;

    let (min_ay, max_ay) = if a0y < a1y {
        if (b0y < a0y && b1y < a0y) || (a1y < b0y && a1y < b1y) {
            return None;
        }
        if b1x - a0x < -0.0001 || b0x - a0x > 0.0001 {
            return None;
        }
        (a0y, a1y)
    } else {
        if (b0y < a1y && b1y < a1y) || (a0y < b0y && a0y < b1y) {
            return None;
        }
        if b0x - a0x < -0.0001 || b1x - a0x > 0.0001 {
            return None;
        }
        (a1y, a0y)
    };

    let dby = b1y - b0y;
    let dbx = b1x - b0x;
    if dbx.abs() < 0.0001 {
        return None;
    }

    let mut new_y = (dby / dbx * (a0x - b0x)) + b0y;
    let dy = new_y - min_ay;
    if dy < 0.0 {
        if dy < -0.1 {
            return None;
        }
        new_y = min_ay;
    }
    let dy = new_y - max_ay;
    if dy > 0.0 {
        if dy > 0.1 {
            return None;
        }
        new_y = max_ay;
    }

    Some(SourcePoint {
        x: a0x as f32,
        y: new_y as f32,
    })
}

#[cfg(test)]
mod source_map_collision_tests {
    use super::*;
    use crate::state::{
        live_source_local_ecb_for_player_pose_frame_milli_with_flags, SourceStaleMoveEntry,
        SourceStaleMoveTable,
    };

    #[test]
    fn source_cliff_ground_commit_does_not_project_transn_y_before_collision_callback() {
        let stage = StageProfile::battlefield();
        let ledge = stage.ledges[1];
        let mut player = PlayerState::new(0, 0, ledge_facing(ledge.side));
        player.set_motion_state_alias(MotionState::CliffEscapeQuick);
        player.grounded = false;
        player.source_cliff_ledge_id = Some(ledge.index);
        let source_frame = 18;
        let transn_position = source_root_motion_position(player.motion_state, source_frame)
            .expect("CliffEscapeQuick TransN position should be generated");
        apply_source_cliff_ledge_position(&mut player, ledge, transn_position);
        let pre_commit_y = player.source_position.y;

        apply_source_cliff_ground_commit(&mut player, stage, ledge, source_frame);

        assert!(player.grounded);
        assert!(
            (player.source_position.y - pre_commit_y).abs() < 0.000_001,
            "ftCo_CliffClimb_Phys writes cur_pos.y from ledge vertex plus x68C_transNPos.y; ftCommon_8007D7FC/ftCommon_8007D6A4 convert to ground without projecting that Y"
        );
    }

    #[test]
    fn grounded_cliff_escape_physics_uses_ft_80084fa8_without_reanchoring_to_ledge() {
        let stage = StageProfile::battlefield();
        let ledge = stage.ledges[1];
        let mut player = PlayerState::new(0, 0, ledge_facing(ledge.side));
        player.set_motion_state_alias(MotionState::CliffEscapeQuick);
        player.grounded = true;
        player.source_cliff_ledge_id = Some(ledge.index);
        player.motion_frame = 17;
        player.set_source_motion_anim_frame(18.0);
        let previous_ground_velocity = -3.566_990_6_f32;
        player.ground_velocity_x = previous_ground_velocity;
        player.source_self_velocity_x = previous_ground_velocity;
        player.source_position = SourceVec2 {
            x: 63.206_016,
            y: 0.000_1,
        };
        player.position = Vec2 {
            x: source_units_to_milli(player.source_position.x),
            y: source_units_to_milli(player.source_position.y),
        };
        let previous_source_position = player.source_position;
        let source_frame = source_root_motion_physics_frame(&player);
        let delta = source_root_motion_delta_for_action_key(
            SourceActionKey::new("CliffEscapeQuick"),
            source_frame,
        )
        .expect("CliffEscapeQuick grounded TransN offset should be extracted");
        let expected_next_velocity =
            delta.z * player.profile.model_scaling * f32::from(player.facing);

        assert!(apply_source_cliff_physics(
            &mut player,
            stage,
            MeleeCommonData::PROVISIONAL
        ));

        assert_eq!(
            player.source_position, previous_source_position,
            "ftCo_CliffClimb_Phys only applies the ledge-relative cur_pos formula while GA_Air; grounded cliff actions call ft_80084FA8 instead"
        );
        assert!(
            (player.ground_accel_x - (expected_next_velocity - previous_ground_velocity)).abs()
                < 0.000_001,
            "ft_80085030 sets xE4_ground_accel_1 to x6A4_transNOffset.z*facing - gr_vel"
        );
        assert_eq!(
            player.ground_velocity_x.to_bits(),
            previous_ground_velocity.to_bits()
        );
        assert_eq!(
            player.velocity.x,
            source_units_to_milli(expected_next_velocity)
        );
    }

    #[test]
    fn source_horizontal_floor_intersection_matches_mp_line_intersection_h_directionality() {
        let floor_y = 27.2_f32;
        let downward = source_mp_line_intersection_h(
            -57.6,
            floor_y,
            -20.0,
            SourcePoint { x: -52.8, y: 28.7 },
            SourcePoint { x: -53.5, y: 26.9 },
        );
        assert_eq!(
            downward,
            Some(SourcePoint {
                x: -53.383_33,
                y: floor_y
            })
        );

        let upward = source_mp_line_intersection_h(
            -57.6,
            floor_y,
            -20.0,
            SourcePoint { x: -53.5, y: 26.9 },
            SourcePoint { x: -52.8, y: 28.7 },
        );
        assert_eq!(upward, None);
    }

    #[test]
    fn source_soft_floor_intersection_uses_mp_line_intersection_h_vertical_tolerance() {
        let floor_y = 27.2001_f32;
        let intersection = source_floor_line_intersection_for_kind(
            StageCollisionLineKind::SoftFloor,
            SourcePoint {
                x: 20.0,
                y: floor_y,
            },
            SourcePoint {
                x: 57.6,
                y: floor_y,
            },
            SourcePoint {
                x: 29.444_954,
                y: 29.100_101,
            },
            SourcePoint {
                x: 27.402_969,
                y: 27.198_944,
            },
        );

        assert!(
            intersection.is_some(),
            "mpCheckFloor calls mpLineIntersectionH for horizontal floors; a descending sweep that ends slightly below the floor must count"
        );
    }

    #[test]
    fn fall_reaching_battlefield_right_ledge_populates_right_wall_hit_before_correction() {
        let stage = StageProfile::battlefield();
        let melee_stage = stage
            .melee_stage_profile()
            .expect("Battlefield must expose source collision");
        let collision = melee_stage.collision;
        let previous_root = SourcePoint {
            x: 70.220_909,
            y: -3.629_900,
        };
        let current_root = SourcePoint {
            x: 70.812_067,
            y: -7.129_900,
        };
        let previous_ecb = SourceEcb {
            bottom: SourcePoint { x: 0.0, y: 0.0 },
            left: SourcePoint {
                x: -4.058_132,
                y: 8.770_508,
            },
            top: SourcePoint {
                x: 0.0,
                y: 13.178_587,
            },
            right: SourcePoint {
                x: 4.058_132,
                y: 8.770_508,
            },
        };
        let current_ecb = SourceEcb {
            bottom: SourcePoint { x: 0.0, y: 0.0 },
            left: SourcePoint {
                x: -4.201_041,
                y: 8.953_657,
            },
            top: SourcePoint {
                x: 0.0,
                y: 13.377_256,
            },
            right: SourcePoint {
                x: 4.201_041,
                y: 8.953_657,
            },
        };

        let mut hits = SourceWallHits::default();
        source_collect_right_wall_hits(
            collision,
            &mut hits,
            previous_root,
            current_root,
            previous_ecb,
            current_ecb,
            false,
            None,
        );

        assert!(
            hits.iter().any(|line_id| line_id == 16),
            "mpColl_80044E10_RightWall must include Battlefield right ledge wall line 16 once the Fall ECB edge reaches the ledge wall; got {:?}",
            hits
        );
    }

    #[test]
    fn mp_check_right_wall_uses_source_segment_not_extended_endpoint() {
        let stage = StageProfile::battlefield();
        let melee_stage = stage
            .melee_stage_profile()
            .expect("Battlefield must expose source collision");
        let collision = melee_stage.collision;

        let hit = source_check_wall(
            collision,
            StageCollisionLineKind::RightWall,
            SourcePoint {
                x: 68.736_038,
                y: -0.262_497,
            },
            SourcePoint {
                x: 67.640_038,
                y: 0.901_374,
            },
        );

        assert_eq!(
            hit, None,
            "decomp mpCheckRightWall tests source v0/v1 segment coordinates; plain wall checks must not extend connected endpoints"
        );
    }

    #[test]
    fn mp_check_left_wall_uses_source_segment_not_extended_endpoint() {
        let stage = StageProfile::battlefield();
        let melee_stage = stage
            .melee_stage_profile()
            .expect("Battlefield must expose source collision");
        let collision = melee_stage.collision;

        let hit = source_check_wall(
            collision,
            StageCollisionLineKind::LeftWall,
            SourcePoint {
                x: -68.736_038,
                y: -0.262_497,
            },
            SourcePoint {
                x: -67.640_038,
                y: 0.901_374,
            },
        );

        assert_eq!(
            hit, None,
            "decomp mpCheckLeftWall tests line 17 against its source v0/v1 segment (-64.98,-6.0)->(-68.4,0.0); this sweep only hits our invented extended endpoint"
        );
    }

    #[test]
    fn damage_n3_air_collision_uses_ft_80081dd4_before_landing() {
        let stage = StageProfile::battlefield();
        let common_data = MeleeCommonData::PROVISIONAL;
        let mut player = PlayerState::new(34_764, 156, 1);
        player.grounded = false;
        player.melee_action_state_id = Some(MeleeActionStateId::new(80));
        player.source_action_key = Some(SourceActionKey::new("DamageN3"));
        player.source_action_total_frames = 49;
        player.motion_state = MotionState::Fall;
        player.motion_state_alias = None;
        player.motion_frame = 8;
        player.set_source_motion_anim_frame(8.0);
        player.source_position = SourceVec2 {
            x: 34.764_168,
            y: 0.155_873_9,
        };
        player.position = player.source_position.to_milli();
        player.source_coll_cur_pos = player.source_position;
        player.source_coll_last_pos = SourceVec2 {
            x: 33.495_647,
            y: 0.317_969_92,
        };
        player.source_self_velocity_y = -1.039_999_96;
        player.source_knockback_velocity_x = 1.268_520_4;
        player.source_knockback_velocity_y = 0.877_903_94;
        player.damage_hitstun_frames = 20;
        player.velocity.x = source_units_to_milli(player.source_knockback_velocity_x);
        player.velocity.y = source_units_to_milli(
            player.source_self_velocity_y + player.source_knockback_velocity_y,
        );

        let previous_ecb_bottom = player.position;
        let mut no_pose_metadata = |_player: &PlayerState| None;
        let mut no_action_total_frames = |_action_state_id: MeleeActionStateId| Some(49);

        advance_source_damage_state(
            &mut player,
            stage,
            common_data,
            previous_ecb_bottom,
            MeleeInputFacts::default(),
            0,
            0,
            &mut no_pose_metadata,
            &mut no_action_total_frames,
        );

        assert_eq!(
            player.melee_action_state_id,
            Some(MeleeActionStateId::new(80)),
            "ftCo_Damage_Coll calls ft_80081DD4; source row 2286 stays DamageN3 instead of entering Landing"
        );
        assert_eq!(player.position.y, -165);
        assert!(
            (player.source_coll_cur_pos.x - player.source_position.x).abs() <= 0.0001
                && (player.source_coll_cur_pos.y - player.source_position.y).abs() <= 0.0001,
            "ft_80081DD4 writes fp->cur_pos back from coll->cur_pos every damage-air collision tick; stale CollData causes later floor/ledge checks to use the wrong sweep"
        );
    }

    #[test]
    fn damage_n3_mp_coll_load_ecb_uses_source_action_pose_before_rust_alias() {
        let mut player = PlayerState::new(0, 0, 1);
        player.motion_state = MotionState::Landing;
        player.motion_state_alias = None;
        player.melee_action_state_id = Some(MeleeActionStateId::new(80));
        player.source_action_key = Some(SourceActionKey::new("DamageN3"));
        player.source_action_total_frames = 49;
        player.motion_anim_frame_milli = 0;
        player.motion_frame = 0;
        player.set_source_motion_anim_frame(0.0);

        source_mp_coll_load_ecb_inline(&mut player, MeleeCommonData::PROVISIONAL, 6);

        assert!(
            player.source_coll_desired_ecb.bottom.y > 1.0,
            "source-backed DamageN3 collision must sample the action-table/JObj ECB before any Rust motion-state alias such as Landing; got {:?}",
            player.source_coll_desired_ecb
        );
    }

    #[test]
    fn damage_lw3_mp_coll_load_ecb_jobj_samples_post_anim_pose_seen_by_coll_callback() {
        let mut player = PlayerState::new(0, 0, 1);
        player.melee_action_state_id = Some(MeleeActionStateId::new(83));
        player.source_action_key = Some(SourceActionKey::new("DamageLw3"));
        player.source_action_total_frames = 49;
        player.motion_anim_frame_milli = 18_000;
        player.motion_anim_rate_milli = 1_000;
        player.motion_frame = 18;
        player.set_source_motion_anim_frame(18.0);

        let current = live_source_local_ecb_for_player_pose_frame_milli_with_flags(
            &player,
            18_000,
            MeleeCommonData::PROVISIONAL,
            6,
        );
        let post_anim = live_source_local_ecb_for_player_pose_frame_milli_with_flags(
            &player,
            19_000,
            MeleeCommonData::PROVISIONAL,
            6,
        );
        assert!(
            (current.bottom.y - post_anim.bottom.y).abs() > 0.0001,
            "test must cover a DamageLw3 pose boundary with distinct current/post-ftAnim ECB samples"
        );

        source_mp_coll_load_ecb_inline(&mut player, MeleeCommonData::PROVISIONAL, 6);

        assert!(
            (player.source_coll_desired_ecb.bottom.y - post_anim.bottom.y).abs() <= 0.0001,
            "Fighter_Spaghetti_8006AD10 advances ftAnim before Fighter_procMap calls Damage coll_cb; mpColl_LoadECB_JObj must see DamageLw3 frame-19 bottom.y {:.6}, got {:.6}",
            post_anim.bottom.y,
            player.source_coll_desired_ecb.bottom.y
        );
    }

    #[test]
    fn air_collision_loop_records_decomp_right_ledge_grab_flag_after_descending_sweep() {
        let stage = StageProfile::battlefield();
        let melee_stage = stage
            .melee_stage_profile()
            .expect("Battlefield must expose source collision");
        let collision = melee_stage.collision;
        let previous_root = SourcePoint { x: 70.0, y: -14.0 };
        let mut current_root = SourcePoint { x: 70.0, y: -17.0 };
        let previous_ecb = SourceEcb {
            bottom: SourcePoint { x: 0.0, y: 0.0 },
            left: SourcePoint { x: -4.0, y: 4.0 },
            top: SourcePoint { x: 0.0, y: 8.0 },
            right: SourcePoint { x: 4.0, y: 4.0 },
        };
        let current_ecb = SourceEcb {
            bottom: SourcePoint { x: 0.0, y: 0.0 },
            left: SourcePoint { x: -4.0, y: 4.0 },
            top: SourcePoint { x: 0.0, y: 8.0 },
            right: SourcePoint { x: 4.0, y: 4.0 },
        };
        let mut player = PlayerState::new(0, 0, 1);
        player.grounded = false;
        player.motion_state = MotionState::Fall;
        player.facing = -1;
        player.source_coll_facing_dir = -1;

        let touched_floor = source_air_map_wall_callback(
            stage,
            collision,
            &mut player,
            &mut current_root,
            previous_root,
            previous_ecb,
            current_ecb,
            SOURCE_COLLISION_FLAG_AIR_CAN_GRAB_LEDGE,
            false,
            false,
        );

        assert!(!touched_floor);
        let right_line = collision
            .scaled_line(5)
            .expect("Battlefield right ledge floor line should be scaled");
        assert_eq!(
            player.source_coll_env_flags & 0x0200_0000,
            0x0200_0000,
            "mpColl_80046904 sets Collide_RightLedgeGrab after a descending can-grab-ledge air collision sweep when mpColl_800443C4 finds Battlefield's right floor edge; corrected_root={:?} line5=({:?},{:?})->({:?},{:?}) env={:#x}",
            current_root,
            right_line.x0,
            right_line.y0,
            right_line.x1,
            right_line.y1,
            player.source_coll_env_flags
        );
    }

    #[test]
    fn cliff_catch_facing_write_preserves_topn_model_orientation() {
        let stage = StageProfile::battlefield();
        let ledge = stage.ledges[1];
        let mut player = PlayerState::new(0, 0, 1);

        assert!(enter_source_cliff_catch(&mut player, ledge));

        assert_eq!(player.facing, -1);
        assert_eq!(
            player.source_model_facing, 1,
            "ftCliffCommon_80081370 writes fp->facing_dir directly and does not call ftPartSetRotY"
        );
    }

    #[test]
    fn live_directional_ecb_is_not_mirrored_again_for_facing() {
        let baked = SourceFighterEcb {
            top: SourceVec2 { x: 0.0, y: 10.0 },
            right: SourceVec2 { x: 7.0, y: 5.0 },
            bottom: SourceVec2 { x: 0.0, y: 0.0 },
            left: SourceVec2 { x: -3.0, y: 5.0 },
        };

        assert_eq!(source_ecb_for_facing(baked, 1), baked);
        assert_eq!(source_ecb_for_facing(baked, -1), baked);
    }

    #[test]
    fn set_throw_hitbox_installs_ftcoll_8007abd0_float_damage_state() {
        let common_data = MeleeCommonData::PROVISIONAL;
        let mut player = PlayerState::new(0, 0, 1);
        player.melee_action_state_id = Some(MeleeActionStateId::new(221));
        player.source_attack_id = 56;
        player.source_attack_instance = 7;
        player.source_stale_move_table = SourceStaleMoveTable {
            current_index: 1,
            entries: [
                SourceStaleMoveEntry {
                    move_id: 56,
                    attack_instance: 3,
                },
                SourceStaleMoveEntry::EMPTY,
                SourceStaleMoveEntry::EMPTY,
                SourceStaleMoveEntry::EMPTY,
                SourceStaleMoveEntry::EMPTY,
                SourceStaleMoveEntry::EMPTY,
                SourceStaleMoveEntry::EMPTY,
                SourceStaleMoveEntry::EMPTY,
                SourceStaleMoveEntry::EMPTY,
                SourceStaleMoveEntry::EMPTY,
            ],
        };
        let raw = SourceThrowHitboxAttributes {
            hitbox_idx: 0,
            damage: 10,
            angle: 90,
            hit_x24: 105,
            hit_x28: 0,
            hit_x2c: 70,
            element: 0,
            sfx_severity: 0,
            sfx_kind: 0,
        };

        apply_source_script_events(
            &mut player,
            SourceActionScriptEvents::single(SourceActionScriptEvent::SetThrowHitbox(raw)),
            common_data,
        );

        let installed =
            player.source_throw_hitboxes[0].expect("ftAction_80071E04 should install fp->xDF4[0]");
        let expected_damage = 10.0_f32 * (1.0 - common_data.stale_move_damage_reductions[0]);
        assert_eq!(
            installed.damage.to_bits(),
            expected_damage.to_bits(),
            "ftColl_8007ABD0 stores ft_80089228 stale-scaled float damage in HitCapsule.damage"
        );
        assert_eq!(
            installed.unk_count, 10,
            "ftColl_8007ABD0 stores unk_count from un-staled scaled damage"
        );
    }

    #[test]
    fn special_hi_mp_coll_load_ecb_jobj_samples_post_anim_pose_seen_by_coll_callback() {
        let mut player = PlayerState::new(0, 0, 1);
        player.set_motion_state_alias(MotionState::SpecialHi);
        player.set_source_motion_anim_frame_milli(60_000);
        player.motion_anim_rate_milli = 1_000;

        source_mp_coll_load_ecb_inline(&mut player, MeleeCommonData::PROVISIONAL, 6);

        let expected = live_source_local_ecb_for_player_pose_frame_milli_with_flags(
            &player,
            61_000,
            MeleeCommonData::PROVISIONAL,
            6,
        );
        assert!(
            (player.source_coll_desired_ecb.bottom.y - expected.bottom.y).abs() <= 0.0001,
            "mpColl_LoadECB_JObj must see the post-ftAnim live JObj pose before coll_cb; got bottom.y {:.6}, expected frame 61 bottom.y {:.6}",
            player.source_coll_desired_ecb.bottom.y,
            expected.bottom.y
        );
    }

    #[test]
    fn jump_aerial_f_mp_coll_load_ecb_jobj_samples_current_pose_for_wall_parity() {
        let mut player = PlayerState::new(0, 0, 1);
        player.set_motion_state_alias(MotionState::JumpAerialF);
        player.set_source_motion_anim_frame_milli(9_000);
        player.motion_anim_rate_milli = 1_000;

        source_mp_coll_load_ecb_inline(&mut player, MeleeCommonData::PROVISIONAL, 6);

        let current = live_source_local_ecb_for_player_pose_frame_milli_with_flags(
            &player,
            9_000,
            MeleeCommonData::PROVISIONAL,
            6,
        );
        let next = live_source_local_ecb_for_player_pose_frame_milli_with_flags(
            &player,
            10_000,
            MeleeCommonData::PROVISIONAL,
            6,
        );
        assert!(
            (current.bottom.y - next.bottom.y).abs() > 0.0001,
            "test must cover a JumpAerialF pose boundary with distinct current/next ECB samples"
        );
        assert!(
            (player.source_coll_desired_ecb.bottom.y - current.bottom.y).abs() <= 0.0001,
            "JumpAerialF map collision must keep current-frame JObj ECB sampling; got bottom.y {:.6}, expected current frame bottom.y {:.6}",
            player.source_coll_desired_ecb.bottom.y,
            current.bottom.y
        );
    }

    #[test]
    fn escape_air_mp_coll_load_ecb_jobj_samples_current_pose_for_soft_floor_parity() {
        let mut player = PlayerState::new(0, 0, 1);
        player.set_motion_state_alias(MotionState::EscapeAir);
        player.set_source_motion_anim_frame_milli(2_000);
        player.motion_anim_rate_milli = 1_000;

        source_mp_coll_load_ecb_inline(&mut player, MeleeCommonData::PROVISIONAL, 6);

        let current = live_source_local_ecb_for_player_pose_frame_milli_with_flags(
            &player,
            2_000,
            MeleeCommonData::PROVISIONAL,
            6,
        );
        let next = live_source_local_ecb_for_player_pose_frame_milli_with_flags(
            &player,
            3_000,
            MeleeCommonData::PROVISIONAL,
            6,
        );
        assert!(
            (current.bottom.y - next.bottom.y).abs() > 0.0001,
            "test must cover an EscapeAir pose boundary with distinct current/next ECB samples"
        );
        assert!(
            (player.source_coll_desired_ecb.bottom.y - current.bottom.y).abs() <= 0.0001,
            "EscapeAir mpColl_LoadECB must use the current JObj ECB sample; got bottom.y {:.6}, expected current frame bottom.y {:.6}",
            player.source_coll_desired_ecb.bottom.y,
            current.bottom.y
        );
    }

    #[test]
    fn fall_air_collision_soft_platform_floor_callback_controls_battlefield_side_platform() {
        let stage = StageProfile::battlefield();
        let melee_stage = stage
            .melee_stage_profile()
            .expect("Battlefield must expose source collision");
        let collision = melee_stage.collision;
        let previous_root = SourcePoint {
            x: 34.608_62,
            y: 25.220_1,
        };
        let current_root = SourcePoint {
            x: 32.781_364,
            y: 23.111_729,
        };
        let previous_ecb = SourceEcb {
            bottom: SourcePoint {
                x: 0.0,
                y: 1.997_981,
            },
            left: SourcePoint {
                x: -3.912_228,
                y: 6.388_651,
            },
            top: SourcePoint {
                x: 0.0,
                y: 10.779_321,
            },
            right: SourcePoint {
                x: 3.912_228,
                y: 6.388_651,
            },
        };
        let current_ecb = SourceEcb {
            bottom: SourcePoint {
                x: 0.0,
                y: 1.712_494,
            },
            left: SourcePoint {
                x: -2.191_981,
                y: 6.461_828,
            },
            top: SourcePoint {
                x: 0.0,
                y: 11.211_162,
            },
            right: SourcePoint {
                x: 2.191_981,
                y: 6.461_828,
            },
        };

        let mut accepted = current_root;
        let mut player = PlayerState::new(0, 0, 1);
        player.grounded = false;
        player.set_motion_state_alias(MotionState::Fall);
        let touched_floor = source_air_map_wall_callback(
            stage,
            collision,
            &mut player,
            &mut accepted,
            previous_root,
            previous_ecb,
            current_ecb,
            SOURCE_COLLISION_FLAG_AIR_PLATFORM_PASS_CALLBACK,
            false,
            false,
        );
        assert!(touched_floor);
        assert_eq!(player.source_coll_floor_line_index, Some(4));

        let mut rejected = current_root;
        let mut player = PlayerState::new(0, 0, 1);
        player.grounded = false;
        player.set_motion_state_alias(MotionState::Fall);
        let touched_floor = source_air_map_wall_callback(
            stage,
            collision,
            &mut player,
            &mut rejected,
            previous_root,
            previous_ecb,
            current_ecb,
            SOURCE_COLLISION_FLAG_AIR_PLATFORM_PASS_CALLBACK,
            false,
            true,
        );
        assert!(
            !touched_floor,
            "ftCo_80096CC8 rejects LINE_FLAG_PLATFORM floor contacts while the stick is at or below p_ftCommonData->x25C"
        );
    }

    #[test]
    fn mp_check_floor_skips_too_far_collision_joints_after_bounding_check() {
        static VERTICES: [crate::StageCollisionVertex; 4] = [
            crate::StageCollisionVertex {
                index: 0,
                source_x: -1.0,
                source_y: 0.0,
                pos_x: -1.0,
                pos_y: 0.0,
                x10: -1.0,
                x14: 0.0,
            },
            crate::StageCollisionVertex {
                index: 1,
                source_x: 1.0,
                source_y: 0.0,
                pos_x: 1.0,
                pos_y: 0.0,
                x10: 1.0,
                x14: 0.0,
            },
            crate::StageCollisionVertex {
                index: 2,
                source_x: -1.0,
                source_y: -10.0,
                pos_x: -1.0,
                pos_y: -10.0,
                x10: -1.0,
                x14: -10.0,
            },
            crate::StageCollisionVertex {
                index: 3,
                source_x: 1.0,
                source_y: -10.0,
                pos_x: 1.0,
                pos_y: -10.0,
                x10: 1.0,
                x14: -10.0,
            },
        ];
        static LINES: [crate::StageCollisionLine; 2] = [
            crate::StageCollisionLine {
                index: 0,
                kind: StageCollisionLineKind::Floor,
                passable: false,
                v0_idx: 0,
                v1_idx: 1,
                prev_id0: -1,
                next_id0: -1,
                prev_id1: -1,
                next_id1: -1,
                hi_flags: 1,
                lo_flags: 0,
            },
            crate::StageCollisionLine {
                index: 1,
                kind: StageCollisionLineKind::Floor,
                passable: false,
                v0_idx: 2,
                v1_idx: 3,
                prev_id0: -1,
                next_id0: -1,
                prev_id1: -1,
                next_id1: -1,
                hi_flags: 1,
                lo_flags: 0,
            },
        ];
        static JOINTS: [crate::StageCollisionJoint; 2] = [
            crate::StageCollisionJoint {
                index: 0,
                floor_start: 0,
                floor_count: 1,
                ceiling_start: 0,
                ceiling_count: 0,
                right_wall_start: 0,
                right_wall_count: 0,
                left_wall_start: 0,
                left_wall_count: 0,
                dynamic_start: 0,
                dynamic_count: 0,
                left_bound_milli: 100_000,
                bottom_bound_milli: -1_000,
                right_bound_milli: 110_000,
                top_bound_milli: 1_000,
                vtx_start: 0,
                vtx_count: 2,
            },
            crate::StageCollisionJoint {
                index: 1,
                floor_start: 1,
                floor_count: 1,
                ceiling_start: 0,
                ceiling_count: 0,
                right_wall_start: 0,
                right_wall_count: 0,
                left_wall_start: 0,
                left_wall_count: 0,
                dynamic_start: 0,
                dynamic_count: 0,
                left_bound_milli: -1_000,
                bottom_bound_milli: -11_000,
                right_bound_milli: 1_000,
                top_bound_milli: -9_000,
                vtx_start: 2,
                vtx_count: 2,
            },
        ];
        let collision = StageCollisionProfile {
            scale: 1.0,
            vertices: &VERTICES,
            lines: &LINES,
            joints: &JOINTS,
        };

        let floor = source_check_floor(
            collision,
            SourcePoint { x: 0.0, y: 1.0 },
            SourcePoint { x: 0.0, y: -11.0 },
        );

        assert_eq!(
            floor,
            Some(1),
            "mpCheckFloor calls mpBoundingCheck2 and skips CollJoint_TooFar before line iteration"
        );
    }

    #[test]
    fn mp_check_floor_does_not_treat_ecb_unlock_growth_as_downward_platform_crossing() {
        let stage = StageProfile::battlefield();
        let melee_stage = stage
            .melee_stage_profile()
            .expect("Battlefield must expose source collision");
        let collision = melee_stage.collision;
        let previous_bottom = SourcePoint {
            x: 20.628_222,
            y: 25.250_101,
        };
        let current_bottom = SourcePoint {
            x: 20.985_222,
            y: 24.470_1 + 2.497_491,
        };

        assert_eq!(
            source_check_floor(collision, previous_bottom, current_bottom),
            None,
            "mpColl_80044628_Floor calls mpCheckFloor/Remap with prev_pos+prev_ecb.bottom and cur_pos+ecb.bottom; the source frame 2625 ECB unlock grows the bottom point upward, so horizontal mpLineIntersectionH must not report a downward floor crossing"
        );
    }

    #[test]
    fn escape_air_collision_flags_match_ft_80082c74_mp_coll_800471f8() {
        let mut player = PlayerState::new(0, 0, 1);
        player.grounded = false;
        player.set_motion_state_alias(MotionState::EscapeAir);

        assert_eq!(
            source_air_map_collision_flags(&player, MotionState::EscapeAir),
            0,
            "ftCo_EscapeAir_Coll routes through ft_80082C74 -> ft_80081D0C -> mpColl_800471F8, which calls inline0(coll, 0, true)"
        );
    }

    #[test]
    fn escape_air_collision_accepts_soft_platform_with_ft_80082c74_flags_zero() {
        let stage = StageProfile::battlefield();
        let melee_stage = stage
            .melee_stage_profile()
            .expect("Battlefield must expose source collision");
        let collision = melee_stage.collision;
        let previous_root = SourcePoint {
            x: 34.608_62,
            y: 25.220_1,
        };
        let current_root = SourcePoint {
            x: 32.781_364,
            y: 23.111_729,
        };
        let previous_ecb = SourceEcb {
            bottom: SourcePoint {
                x: 0.0,
                y: 1.997_981,
            },
            left: SourcePoint {
                x: -3.912_228,
                y: 6.388_651,
            },
            top: SourcePoint {
                x: 0.0,
                y: 10.779_321,
            },
            right: SourcePoint {
                x: 3.912_228,
                y: 6.388_651,
            },
        };
        let current_ecb = SourceEcb {
            bottom: SourcePoint {
                x: 0.0,
                y: 1.712_494,
            },
            left: SourcePoint {
                x: -2.191_981,
                y: 6.461_828,
            },
            top: SourcePoint {
                x: 0.0,
                y: 11.211_162,
            },
            right: SourcePoint {
                x: 2.191_981,
                y: 6.461_828,
            },
        };

        let mut rejected = current_root;
        let mut player = PlayerState::new(0, 0, 1);
        player.grounded = false;
        player.set_motion_state_alias(MotionState::EscapeAir);
        let flags = source_air_map_collision_flags(&player, MotionState::EscapeAir);
        let touched_floor = source_air_map_wall_callback(
            stage,
            collision,
            &mut player,
            &mut rejected,
            previous_root,
            previous_ecb,
            current_ecb,
            flags,
            false,
            false,
        );

        assert!(
            touched_floor,
            "ftCo_EscapeAir_Coll routes through ft_80082C74 -> ft_80081D0C -> mpColl_800471F8, whose inline0 flags argument is 0"
        );
    }

    #[test]
    fn air_floor_collision_rejects_decomp_floor_skip_line_id() {
        let stage = StageProfile::battlefield();
        let melee_stage = stage
            .melee_stage_profile()
            .expect("Battlefield must expose source collision");
        let collision = melee_stage.collision;
        let previous_root = SourcePoint {
            x: 34.608_62,
            y: 25.220_1,
        };
        let mut current_root = SourcePoint {
            x: 32.781_364,
            y: 23.111_729,
        };
        let previous_ecb = SourceEcb {
            bottom: SourcePoint {
                x: 0.0,
                y: 1.997_981,
            },
            left: SourcePoint {
                x: -3.912_228,
                y: 6.388_651,
            },
            top: SourcePoint {
                x: 0.0,
                y: 10.779_321,
            },
            right: SourcePoint {
                x: 3.912_228,
                y: 6.388_651,
            },
        };
        let current_ecb = SourceEcb {
            bottom: SourcePoint {
                x: 0.0,
                y: 1.712_494,
            },
            left: SourcePoint {
                x: -2.191_981,
                y: 6.461_828,
            },
            top: SourcePoint {
                x: 0.0,
                y: 11.211_162,
            },
            right: SourcePoint {
                x: 2.191_981,
                y: 6.461_828,
            },
        };

        let mut player = PlayerState::new(0, 0, 1);
        player.grounded = false;
        player.set_motion_state_alias(MotionState::EscapeAir);
        player.source_coll_floor_skip_line_index = Some(4);

        let touched_floor = source_air_map_wall_callback(
            stage,
            collision,
            &mut player,
            &mut current_root,
            previous_root,
            previous_ecb,
            current_ecb,
            0,
            false,
            false,
        );

        assert!(
            !touched_floor,
            "mpColl_80044628_Floor compares coll->floor.index against coll->floor_skip; floor_skip is the decomp line id, so line 4 must be rejected even with EscapeAir flags=0"
        );
    }

    #[test]
    fn escape_air_enter_clears_decomp_floor_skip_through_motion_state_change() {
        let mut player = PlayerState::new(0, 0, 1);
        player.grounded = false;
        player.set_motion_state_alias(MotionState::Fall);
        player.source_coll_floor_skip_line_index = Some(4);

        enter_escape_air(&mut player, -83, -95, MeleeCommonData::PROVISIONAL);

        assert_eq!(
            player.source_coll_floor_skip_line_index, None,
            "ftCo_80099A9C enters EscapeAir through Fighter_ChangeMotionState, whose common transition path calls mpClearFloorSkip"
        );
        assert_eq!(player.motion_state, MotionState::EscapeAir);
    }

    #[test]
    fn source_backed_air_collision_must_not_use_legacy_landing_fallback_after_mp_coll_rejects() {
        let stage = StageProfile::battlefield();
        let melee_stage = stage
            .melee_stage_profile()
            .expect("Battlefield must expose source collision");
        let collision = melee_stage.collision;
        let previous_root = SourcePoint {
            x: 34.608_62,
            y: 25.220_1,
        };
        let mut current_root = SourcePoint {
            x: 32.781_364,
            y: 23.111_729,
        };
        let previous_ecb = SourceEcb {
            bottom: SourcePoint {
                x: 0.0,
                y: 1.997_981,
            },
            left: SourcePoint {
                x: -3.912_228,
                y: 6.388_651,
            },
            top: SourcePoint {
                x: 0.0,
                y: 10.779_321,
            },
            right: SourcePoint {
                x: 3.912_228,
                y: 6.388_651,
            },
        };
        let current_ecb = SourceEcb {
            bottom: SourcePoint {
                x: 0.0,
                y: 1.712_494,
            },
            left: SourcePoint {
                x: -2.191_981,
                y: 6.461_828,
            },
            top: SourcePoint {
                x: 0.0,
                y: 11.211_162,
            },
            right: SourcePoint {
                x: 2.191_981,
                y: 6.461_828,
            },
        };

        let mut player = PlayerState::new(
            source_units_to_milli(current_root.x),
            source_units_to_milli(current_root.y),
            1,
        );
        player.grounded = false;
        player.set_motion_state_alias(MotionState::EscapeAir);
        player.source_coll_floor_skip_line_index = Some(4);
        player.source_coll_prev_ecb = source_fighter_ecb_from_source_ecb(previous_ecb);
        player.source_coll_ecb = source_fighter_ecb_from_source_ecb(current_ecb);
        player.source_coll_desired_ecb = source_fighter_ecb_from_source_ecb(current_ecb);

        let touched_floor = source_air_map_wall_callback(
            stage,
            collision,
            &mut player,
            &mut current_root,
            previous_root,
            previous_ecb,
            current_ecb,
            0,
            false,
            false,
        );

        let legacy_fallback_contact = airborne_landing_contact(
            stage,
            &player,
            Vec2 {
                x: source_units_to_milli(previous_root.x + previous_ecb.bottom.x),
                y: source_units_to_milli(previous_root.y + previous_ecb.bottom.y),
            },
            None,
            false,
            MeleeCommonData::PROVISIONAL,
        );

        assert!(!touched_floor);
        assert!(
            legacy_fallback_contact.is_some(),
            "the old Rust-only fallback would land here even though mpColl_80044628_Floor rejected the decomp line-id floor_skip"
        );
        assert!(
            stage.melee_stage_profile().is_some(),
            "source-backed stages must trust mpColl_800471F8 instead of running the legacy landing fallback"
        );
    }

    #[test]
    fn pass_entry_copies_current_floor_line_into_decomp_floor_skip() {
        let stage = StageProfile::battlefield();
        let mut player = PlayerState::new(0, 0, 1);
        player.grounded = true;
        player.source_coll_floor_line_index = Some(2);
        player.source_coll_floor_surface_index = Some(1);
        set_ground_velocity_x(&mut player, 0.543);

        enter_pass(&mut player, stage, MeleeCommonData::PROVISIONAL);

        assert_eq!(
            player.source_coll_floor_skip_line_index,
            Some(2),
            "ftCo_8009A228 calls ftCommon_8007D5D4 before mpUpdateFloorSkip, and ftCommon_8007D5D4 does not clear coll_data.floor.index"
        );
    }

    #[test]
    fn fall_mp_coll_load_ecb_jobj_uses_ftco_fall_anim_inner_pose_for_collision() {
        let common_data = MeleeCommonData::PROVISIONAL;
        let mut player = PlayerState::new(0, 0, -1);
        player.set_motion_state_alias(MotionState::Fall);
        player.motion_anim_frame_milli = 2_000;
        player.source_self_velocity_x = 0.601_157_9;
        player.velocity.x = source_units_to_milli(player.source_self_velocity_x);

        let mut neutral = player;
        neutral.source_fall_anim_pose = MotionState::Fall;
        neutral.source_fall_anim_blend = 0.0;
        source_mp_coll_load_ecb_inline(&mut neutral, common_data, 6);

        let mut full_directional = player;
        full_directional.source_fall_anim_pose = MotionState::FallB;
        full_directional.source_fall_anim_blend = 1.0;
        source_mp_coll_load_ecb_inline(&mut full_directional, common_data, 6);

        update_source_fall_anim_inner(&mut player, common_data);
        source_mp_coll_load_ecb_inline(&mut player, common_data, 6);

        assert!(
            player.source_fall_anim_blend > 0.0 && player.source_fall_anim_blend < 1.0,
            "ftCo_Fall_Anim_Inner should move mv.co.fall.x4 toward the target blend, got {}",
            player.source_fall_anim_blend
        );
        assert_eq!(
            player.source_fall_anim_pose,
            MotionState::FallB,
            "with facing -1 and positive self_vel.x, ftCo_Fall_Anim_Inner selects the backward Fall pose"
        );

        let point_blend_left_y = neutral.source_coll_desired_ecb.left.y
            + (full_directional.source_coll_desired_ecb.left.y
                - neutral.source_coll_desired_ecb.left.y)
                * player.source_fall_anim_blend;
        assert!(
            (player.source_coll_desired_ecb.left.y - point_blend_left_y).abs() > 0.001,
            "ftAnim_8006FE9C blends/copies JObj SRT buffers before mpColl_LoadECB_JObj derives the ECB; final ECB point blending is not decomp parity. neutral={:?}, directional={:?}, actual={:?}",
            neutral.source_coll_desired_ecb,
            full_directional.source_coll_desired_ecb,
            player.source_coll_desired_ecb
        );
    }

    #[test]
    fn mp_coll_load_ecb_jobj_supports_raw_expansion_flags_0x12_from_decomp() {
        let mut no_expand = PlayerState::new(0, 0, 1);
        no_expand.set_motion_state_alias(MotionState::Fall);
        no_expand.motion_anim_frame_milli = 2_000;
        source_mp_coll_load_ecb_inline(&mut no_expand, MeleeCommonData::PROVISIONAL, 6);

        let mut raw_expand = PlayerState::new(0, 0, 1);
        raw_expand.set_motion_state_alias(MotionState::Fall);
        raw_expand.motion_anim_frame_milli = 2_000;
        source_mp_coll_load_ecb_inline(&mut raw_expand, MeleeCommonData::PROVISIONAL, 0x12);

        let no_expand_width =
            no_expand.source_coll_desired_ecb.right.x - no_expand.source_coll_desired_ecb.left.x;
        let raw_expand_width =
            raw_expand.source_coll_desired_ecb.right.x - raw_expand.source_coll_desired_ecb.left.x;
        assert!(
            raw_expand_width - no_expand_width > 3.9,
            "mpColl_LoadECB_JObj flags without 0b100 must apply +/-2.0F width expansion before clamps; no_expand={:?}, raw_expand={:?}",
            no_expand.source_coll_desired_ecb,
            raw_expand.source_coll_desired_ecb
        );
        assert!(
            (raw_expand.source_coll_desired_ecb.top.y
                - raw_expand.source_coll_desired_ecb.bottom.y
                - 2.0)
                .abs()
                <= 0.0001,
            "mpColl_LoadECB_JObj flags 0x12 must apply the decomp 0b10000 top/bottom clamp"
        );
    }

    #[test]
    fn escape_air_enter_immediately_samples_entry_animation_pose() {
        let mut player = PlayerState::new(0, 0, 1);

        enter_escape_air(&mut player, -113, -56, MeleeCommonData::PROVISIONAL);

        assert_eq!(
            player.motion_frame, 0,
            "ftCo_EscapeAir_Enter keeps the command-frame counter at action entry"
        );
        assert_eq!(
            player.motion_anim_frame_milli, 1_000,
            "ftCo_EscapeAir_Enter calls ftAnim_8006EBA4 immediately after Fighter_ChangeMotionState"
        );
        assert_eq!(player.motion_state, MotionState::EscapeAir);
    }

    #[test]
    fn special_hi_landing_fall_special_uses_source_transn_ground_velocity() {
        let mut player = PlayerState::new(0, 0, 1);
        player.set_motion_state_alias(MotionState::SpecialHi);
        player.motion_frame = 60;
        player.motion_anim_frame_milli = 60_000;
        player.source_self_velocity_x = -0.699_502_9;
        player.captain_special_hi_x2_b1 = true;
        let expected_ground_velocity_x = source_special_hi_transn_self_velocity(&player).0;

        apply_captain_special_hi_air_floor_collision(&mut player);

        assert_eq!(player.motion_state, MotionState::LandingFallSpecial);
        assert!(
            (player.ground_velocity_x - expected_ground_velocity_x).abs() <= 0.0001,
            "ftCo_LandingFallSpecial_Enter must route through ftCommon_8007D6A4/x6A4 TransN velocity; got {:.6}, expected {:.6}",
            player.ground_velocity_x,
            expected_ground_velocity_x
        );
    }

    #[test]
    fn landing_fall_special_entry_preserves_impact_vertical_self_velocity() {
        let mut player = PlayerState::new(0, 0, 1);
        player.set_motion_state_alias(MotionState::SpecialHi);
        player.motion_frame = 60;
        player.motion_anim_frame_milli = 60_000;
        player.source_self_velocity_y = -1.502_846;
        player.velocity.y = source_units_to_milli(player.source_self_velocity_y);
        let landing_lag = player.profile.captain_special_attrs.specialhi_landing_lag as u8;

        finish_source_floor_contact(&mut player);
        enter_landing_fall_special(&mut player, landing_lag);

        assert_eq!(player.motion_state, MotionState::LandingFallSpecial);
        assert_eq!(
            player.source_self_velocity_y.to_bits(),
            (-1.502_846_f32).to_bits(),
            "ftCommon_8007D6A4 does not clear fp->self_vel.y during the map-collision landing entry"
        );
        assert_eq!(
            player.velocity.y, -1_503,
            "Slippi exposes the preserved landing-entry vertical self velocity for this frame"
        );
    }

    #[test]
    fn landing_fall_special_keeps_decomp_frame_speed_mul_after_entry() {
        let stage = StageProfile::battlefield();
        let mut world = World::for_two_players_on_stage(stage);
        world.set_engine_features(crate::EngineFeatureToggles::parity());
        let player = &mut world.players_mut()[1];
        enter_landing_fall_special(
            player,
            MeleeCommonData::PROVISIONAL.escapeair_landing_lag_ticks,
        );

        assert_eq!(
            player.motion_anim_rate_milli, 3_010,
            "ftCo_LandingFallSpecial_Enter sets frame_speed_mul = (0.1F + fp->x2EC) / landing_lag"
        );

        step_world(
            &mut world,
            Frame(0),
            &[PlayerInput::neutral(); PLAYER_COUNT],
        );
        let player = &world.players()[1];
        assert_eq!(player.motion_frame, 1);
        assert_eq!(
            player.motion_anim_rate_milli, 3_010,
            "ftCo_Landing_Anim/Phys/Coll do not reset frame_speed_mul while LandingFallSpecial remains active"
        );
        assert_eq!(player.motion_anim_frame_milli, 3_010);

        step_world(
            &mut world,
            Frame(1),
            &[PlayerInput::neutral(); PLAYER_COUNT],
        );
        let player = &world.players()[1];
        assert_eq!(player.motion_frame, 2);
        assert_eq!(player.motion_anim_rate_milli, 3_010);
        assert_eq!(player.motion_anim_frame_milli, 6_020);
    }

    #[test]
    fn ftco_fall_enter_resets_landing_fall_special_anim_rate_to_one() {
        let mut player = PlayerState::new(0, 0, -1);
        player.set_motion_state_alias(MotionState::LandingFallSpecial);
        player.grounded = true;
        player.motion_anim_rate_milli = 3_010;
        player.motion_anim_frame_milli = 24_080;
        set_ground_velocity_x(&mut player, 0.621_157_9);

        enter_fall(&mut player);

        assert_eq!(player.motion_state, MotionState::Fall);
        assert_eq!(player.motion_anim_frame_milli, 0);
        assert_eq!(
            player.motion_anim_rate_milli, 1_000,
            "ftCo_Fall_Enter calls Fighter_ChangeMotionState(... start=0.0F, anim_rate=1.0F, blend=0.0F), so it must not inherit LandingFallSpecial frame_speed_mul"
        );
    }

    #[test]
    fn ftco_fall_enter_preserves_damage_knockback_vectors() {
        let mut player = PlayerState::new(0, 0, -1);
        player.motion_state_alias = None;
        player.melee_action_state_id = Some(SOURCE_DOWN_BOUND_U_ACTION_STATE_ID);
        player.source_action_key = Some(SOURCE_DOWN_BOUND_U_ACTION_KEY);
        player.grounded = false;
        player.source_self_velocity_x = 0.0;
        player.source_knockback_velocity_x = -0.140_688;
        player.source_knockback_velocity_y = 0.0;
        player.source_ground_knockback_velocity = -0.140_688;
        player.velocity.x = source_units_to_milli(player.source_knockback_velocity_x);

        enter_fall(&mut player);

        assert_eq!(player.motion_state, MotionState::Fall);
        assert!(!player.grounded);
        assert_eq!(
            player.source_self_velocity_x.to_bits(),
            0.0_f32.to_bits(),
            "ftCo_Fall_Enter clamps fp->self_vel.x; it does not seed self_vel.x from exported velocity that includes fp->x8c_kb_vel.x"
        );
        assert_eq!(
            player.velocity.x, 0,
            "Fall's main velocity export stays self_vel.x while damage knockback remains in the separate attack/knockback field"
        );
        assert_eq!(
            player.source_knockback_velocity_x.to_bits(),
            (-0.140_688_f32).to_bits(),
            "ftCo_Fall_Enter/Fighter_ChangeMotionState/ftCommon_8007D5D4 do not clear fp->x8c_kb_vel.x"
        );
        assert_eq!(
            player.source_knockback_velocity_y.to_bits(),
            0.0_f32.to_bits(),
            "ftCo_Fall_Enter must preserve fp->x8c_kb_vel.y"
        );
        assert_eq!(
            player.source_ground_knockback_velocity.to_bits(),
            (-0.140_688_f32).to_bits(),
            "ftCommon_8007D5D4 does not clear fp->xF0_ground_kb_vel; Fighter_procUpdate clears it later only in the airborne knockback branch"
        );
    }

    #[test]
    fn normal_landing_entry_preserves_impact_vertical_self_velocity() {
        let mut player = PlayerState::new(0, 0, 1);
        player.set_motion_state_alias(MotionState::Fall);
        player.motion_frame = 11;
        player.motion_anim_frame_milli = 11_000;
        player.source_self_velocity_y = -1.690_000_1;
        player.velocity.y = source_units_to_milli(player.source_self_velocity_y);
        let impact_velocity_y = player.source_self_velocity_y;

        snap_player_to_floor_contact(&mut player, 27_200);
        enter_landing_from_airborne(
            &mut player,
            MotionState::Fall,
            impact_velocity_y,
            EXPIRED_INPUT_TIMER,
            MeleeCommonData::PROVISIONAL,
        );

        assert_eq!(player.motion_state, MotionState::Landing);
        assert_eq!(
            player.source_self_velocity_y.to_bits(),
            (-1.690_000_1_f32).to_bits(),
            "ftCo_Landing_Enter routes through ftCommon_8007D6A4, which preserves fp->self_vel.y on the landing-entry frame"
        );
        assert_eq!(
            player.velocity.y, -1_690,
            "Slippi exposes the preserved impact vertical self velocity on the first Landing frame"
        );
    }

    #[test]
    fn normal_landing_entry_resets_frozen_damage_frame_speed_mul() {
        let mut player = PlayerState::new(0, 0, -1);
        player.motion_state_alias = None;
        player.melee_action_state_id = Some(MeleeActionStateId::new(80));
        player.source_action_key = Some(SourceActionKey::new("DamageN3"));
        player.source_action_total_frames = 49;
        player.motion_frame = 15;
        player.set_source_motion_anim_frame(15.0);
        player.motion_anim_rate_milli = 0;
        player.source_self_velocity_y = -2.08;
        player.velocity.y = source_units_to_milli(player.source_self_velocity_y);
        let impact_velocity_y = player.source_self_velocity_y;

        snap_player_to_floor_contact(&mut player, 0);
        enter_landing_from_airborne(
            &mut player,
            MotionState::Fall,
            impact_velocity_y,
            EXPIRED_INPUT_TIMER,
            MeleeCommonData::PROVISIONAL,
        );

        assert_eq!(player.motion_state, MotionState::Landing);
        assert_eq!(player.motion_frame, 0);
        assert_eq!(player.motion_anim_frame_milli, 0);
        assert_eq!(
            player.motion_anim_rate_milli, 1_000,
            "ftCo_Landing_Enter_Basic calls Fighter_ChangeMotionState(... anim_speed=1.0F), so Landing must not inherit Damage's Ft_MF_FreezeState frame_speed_mul"
        );
    }

    #[test]
    fn grounded_landing_fall_special_physics_projects_self_velocity_to_floor_normal() {
        let stage = StageProfile::battlefield();
        let mut world = World::for_two_players_on_stage(stage);
        world.set_engine_features(crate::EngineFeatureToggles::parity());

        let player = &mut world.players_mut()[1];
        player.set_motion_state_alias(MotionState::LandingFallSpecial);
        player.motion_frame = 1;
        player.motion_anim_frame_milli = 1_000;
        player.grounded = true;
        set_source_position_x_source(player, -56.770);
        set_source_position_y_source(player, 27.200);
        player.source_self_velocity_y = -1.502_846;
        player.velocity.y = source_units_to_milli(player.source_self_velocity_y);
        player.source_coll_floor_line_index = Some(2);
        player.source_coll_floor_surface_index = Some(1);

        step_world(
            &mut world,
            Frame(1940),
            &[PlayerInput::neutral(); PLAYER_COUNT],
        );

        let player = &world.players()[1];
        assert_eq!(player.motion_state, MotionState::LandingFallSpecial);
        assert_eq!(
            player.source_self_velocity_y.to_bits(),
            0.0_f32.to_bits(),
            "ftCommon_ApplyGroundMovement must overwrite stale landing self_vel.y from the live flat floor normal before a later ground-to-air handoff"
        );
        assert_eq!(player.velocity.y, 0);
    }

    #[test]
    fn catch_pull_consumes_current_frame_throw_flag_before_anim_transition() {
        let stage = StageProfile::battlefield();
        let common_data = MeleeCommonData::PROVISIONAL;
        let input = PlayerInput::neutral();
        let input_snapshot = input.melee_snapshot(input, MeleeInputTimers::default());
        let input_facts = input_snapshot.facts(common_data.input_thresholds());
        let mut player = PlayerState::new(0, 0, 1);
        player.grounded = true;
        player.motion_state_alias = None;
        player.melee_action_state_id = Some(SOURCE_CATCH_PULL_ACTION_STATE_ID);
        player.source_action_key = Some(SourceActionKey::new("Catch"));
        player.source_action_total_frames = 30;
        player.source_victim_index = Some(1);
        player.motion_frame = 7;
        player.set_source_motion_anim_frame(7.0);
        let mut snapshots = [SourcePoseMetadataSnapshot::default(); PLAYER_COUNT];
        let mut source_pose_metadata = |player: &PlayerState| {
            (player.motion_frame == 8).then_some(SourceActionPoseMetadata {
                script_events: SourceActionScriptEvents::single(
                    SourceActionScriptEvent::SetThrowFlag {
                        hit_idx: 0,
                        flag_bit: None,
                    },
                ),
                ..Default::default()
            })
        };
        let mut source_action_total_frames =
            |action_state_id: MeleeActionStateId| match action_state_id {
                SOURCE_CATCH_WAIT_ACTION_STATE_ID => Some(6),
                SOURCE_CATCH_PULL_ACTION_STATE_ID => Some(30),
                _ => None,
            };
        let mut source_capture_released_this_tick = [false; PLAYER_COUNT];

        let (_, pending_capture_wait, _, _, _) = advance_source_grab_capture_state(
            &mut player,
            0,
            input_facts,
            input_snapshot,
            stage,
            common_data,
            &mut snapshots,
            &mut source_pose_metadata,
            &mut source_action_total_frames,
            &mut source_capture_released_this_tick,
            false,
            [FighterProfile::falcon_like(); PLAYER_COUNT],
        );

        assert_eq!(
            player.melee_action_state_id,
            Some(SOURCE_CATCH_WAIT_ACTION_STATE_ID)
        );
        assert_eq!(player.source_action_key, Some(SOURCE_CATCH_WAIT_ACTION_KEY));
        assert_eq!(
            pending_capture_wait,
            Some(PendingSourceCaptureWaitTransition {
                grabber_index: 0,
                victim_index: 1,
            }),
            "ftAction_800718A4 sets throw_flags_b3 on the current command frame and ftCo_CatchPull_Anim consumes it in the same animation callback"
        );
    }

    #[test]
    fn landing_ground_traction_applies_ground_movement_before_outer_commit() {
        let stage = StageProfile::battlefield();
        let common_data = MeleeCommonData::PROVISIONAL;
        let mut player = PlayerState::new(1, 0, 1);
        player.set_motion_state_alias(MotionState::LandingFallSpecial);
        player.grounded = true;
        set_source_position_x_source(&mut player, -56.770);
        set_source_position_y_source(&mut player, 27.200);
        player.source_coll_floor_line_index = Some(2);
        player.source_coll_floor_surface_index = Some(1);
        set_ground_velocity_x(&mut player, -0.137_246_2);
        player.source_self_velocity_y = -1.502_846;
        player.velocity.y = source_units_to_milli(player.source_self_velocity_y);

        apply_ground_traction(&mut player, stage, common_data);

        assert!(
            player.source_self_velocity_y.abs() <= f32::EPSILON,
            "ft_80084F3C calls ftCommon_ApplyGroundMovement during Landing physics, so fp->self_vel.y must be projected from the live floor normal before fighter.c's outer gr_vel/x74 commit"
        );
        assert_eq!(player.velocity.y, 0);
    }

    #[test]
    fn escape_f_uses_source_ground_collision_at_platform_edge() {
        let stage = StageProfile::battlefield();
        let common_data = MeleeCommonData::PROVISIONAL;
        let mut world = World::for_two_players_on_stage(stage);
        world.set_engine_features(crate::EngineFeatureToggles::parity());

        let player = &mut world.players_mut()[1];
        player.set_motion_state_alias(MotionState::EscapeF);
        player.motion_frame = 26;
        player.motion_anim_frame_milli = 26_000;
        player.motion_anim_rate_milli = 1_000;
        player.grounded = true;
        player.facing = 1;
        player.source_motion_entry_facing = -1;
        set_source_position_x_source(player, 20.0);
        set_source_position_y_source(player, 27.200_1);
        set_ground_velocity_x(player, -0.137_246_2);
        player.set_source_floor_for_diagnostic(Some(2), None);
        player.source_coll_cur_pos = player.source_position;
        player.source_coll_last_pos = player.source_position;
        source_mp_coll_load_ecb_inline(player, common_data, 5);
        player.source_coll_ecb = player.source_coll_desired_ecb;
        player.source_coll_prev_ecb = player.source_coll_desired_ecb;

        step_world(
            &mut world,
            Frame(1300),
            &[PlayerInput::neutral(); PLAYER_COUNT],
        );

        let player = &world.players()[1];
        assert_eq!(player.motion_state, MotionState::EscapeF);
        assert!(player.grounded);
        assert_eq!(
            player.position.x, 19_901,
            "ftCo_Escape_Coll must route through ft_800827A0/mpColl_8004B2DC instead of clamping EscapeF to the soft-platform endpoint"
        );
        assert!(
            (player.source_position.x - 19.900_94).abs() <= 0.000_01,
            "got source x {:.6}",
            player.source_position.x
        );
    }

    #[test]
    fn escape_f_flags2_clamps_to_source_floor_edge_when_projection_misses() {
        let stage = StageProfile::battlefield();
        let common_data = MeleeCommonData::PROVISIONAL;
        let melee_stage = stage
            .melee_stage_profile()
            .expect("Battlefield must expose source collision");
        let mut world = World::for_two_players_on_stage(stage);
        world.set_engine_features(crate::EngineFeatureToggles::parity());

        let player = &mut world.players_mut()[1];
        player.set_motion_state_alias(MotionState::EscapeF);
        player.grounded = true;
        player.facing = 1;
        player.source_motion_entry_facing = -1;
        set_source_position_x_source(player, 19.830_664);
        set_source_position_y_source(player, 27.200_1);
        player.source_coll_cur_pos = SourceVec2 {
            x: 20.118_858,
            y: 27.200_1,
        };
        player.source_coll_floor_line_index = Some(4);
        player.source_coll_floor_surface_index = Some(2);
        source_mp_coll_load_ecb_inline(player, common_data, 5);
        player.source_coll_ecb = player.source_coll_desired_ecb;
        player.source_coll_prev_ecb = player.source_coll_desired_ecb;

        let touching_floor =
            source_ft_800827a0_skip_fall(stage, melee_stage.collision, player, common_data);

        assert!(
            touching_floor,
            "mpColl_8004B2DC inline2(flags=2) must keep EscapeF grounded by clamping to the persistent source floor edge"
        );
        assert!(
            (player.source_position.x - 20.0).abs() <= 0.000_01,
            "got source x {:.6}",
            player.source_position.x
        );
        assert!(
            (player.source_position.y - 27.2).abs() <= 0.000_01,
            "got source y {:.6}",
            player.source_position.y
        );
    }

    #[test]
    fn wait_uses_source_ground_collision_tolerance_at_platform_edge() {
        let stage = StageProfile::battlefield();
        let common_data = MeleeCommonData::PROVISIONAL;
        let mut world = World::for_two_players_on_stage(stage);
        world.set_engine_features(crate::EngineFeatureToggles::parity());

        let player = &mut world.players_mut()[1];
        player.set_motion_state_alias(MotionState::Wait);
        player.grounded = true;
        player.facing = 1;
        set_source_position_x_source(player, 19.931_313);
        set_source_position_y_source(player, 27.200_1);
        player.source_coll_cur_pos = player.source_position;
        player.source_coll_floor_line_index = Some(4);
        player.source_coll_floor_surface_index = Some(2);
        source_mp_coll_load_ecb_inline(player, common_data, 5);
        player.source_coll_ecb = player.source_coll_desired_ecb;
        player.source_coll_prev_ecb = player.source_coll_desired_ecb;

        let grounded = resolve_ground_support_after_move(
            stage,
            player,
            MotionState::Wait,
            0,
            Some(2),
            common_data,
            player.source_position,
        );

        assert!(
            grounded,
            "ftCo_Wait_Coll must use source CollData/mpColl_8004B4B0 tolerance instead of the coarse surface edge gate"
        );
        assert_eq!(player.motion_state, MotionState::Wait);
        assert_eq!(player.source_coll_floor_line_index, Some(4));
        assert!(
            (player.source_position.x - 19.931_313).abs() <= 0.000_01,
            "got source x {:.6}",
            player.source_position.x
        );
    }

    #[test]
    fn source_ground_support_replaces_stale_floor_line_when_surface_disagrees() {
        let stage = StageProfile::battlefield();
        let common_data = MeleeCommonData::PROVISIONAL;
        let mut world = World::for_two_players_on_stage(stage);
        world.set_engine_features(crate::EngineFeatureToggles::parity());

        let player = &mut world.players_mut()[1];
        player.set_motion_state_alias(MotionState::Wait);
        player.grounded = true;
        player.facing = 1;
        set_source_position_x_source(player, 43.210_0);
        set_source_position_y_source(player, 27.200_1);
        player.source_coll_cur_pos = player.source_position;
        player.source_coll_floor_line_index = Some(1);
        player.source_coll_floor_surface_index = Some(2);
        source_mp_coll_load_ecb_inline(player, common_data, 5);
        player.source_coll_ecb = player.source_coll_desired_ecb;
        player.source_coll_prev_ecb = player.source_coll_desired_ecb;

        let grounded = resolve_ground_support_after_move(
            stage,
            player,
            MotionState::Wait,
            0,
            Some(2),
            common_data,
            player.source_position,
        );

        assert!(grounded);
        assert_eq!(
            player.source_coll_floor_line_index,
            Some(4),
            "source CollData floor line must be re-derived when stale line state disagrees with the platform surface"
        );
        assert!(
            (player.source_position.y - 27.200_1).abs() <= 0.000_2,
            "stale main-floor projection moved y to {:.6}",
            player.source_position.y
        );
    }

    #[test]
    fn dash_uses_source_ground_collision_tolerance_at_platform_edge() {
        let stage = StageProfile::battlefield();
        let common_data = MeleeCommonData::PROVISIONAL;
        let mut world = World::for_two_players_on_stage(stage);
        world.set_engine_features(crate::EngineFeatureToggles::parity());

        let player = &mut world.players_mut()[1];
        player.set_motion_state_alias(MotionState::Dash);
        player.grounded = true;
        player.facing = 1;
        set_source_position_x_source(player, 19.931_313);
        set_source_position_y_source(player, 27.200_1);
        player.source_coll_cur_pos = player.source_position;
        player.source_coll_floor_line_index = Some(4);
        player.source_coll_floor_surface_index = Some(2);
        source_mp_coll_load_ecb_inline(player, common_data, 5);
        player.source_coll_ecb = player.source_coll_desired_ecb;
        player.source_coll_prev_ecb = player.source_coll_desired_ecb;

        let grounded = resolve_ground_support_after_move(
            stage,
            player,
            MotionState::Dash,
            127,
            Some(2),
            common_data,
            player.source_position,
        );

        assert!(
            grounded,
            "ftCo_Dash_Coll must route through ft_800844EC/ft_80082708 source ground collision before falling"
        );
        assert_eq!(player.motion_state, MotionState::Dash);
        assert_eq!(player.source_coll_floor_line_index, Some(4));
        assert!(
            (player.source_position.x - 19.931_313).abs() <= 0.000_01,
            "got source x {:.6}",
            player.source_position.x
        );
    }

    #[test]
    fn source_down_bound_uses_source_ground_to_air_collision_even_with_stale_alias() {
        let stage = StageProfile::battlefield();
        let common_data = MeleeCommonData::PROVISIONAL;
        let melee_stage = stage
            .melee_stage_profile()
            .expect("Battlefield must expose source collision");
        let mut world = World::for_two_players_on_stage(stage);
        world.set_engine_features(crate::EngineFeatureToggles::parity());

        let player = &mut world.players_mut()[1];
        player.motion_state = MotionState::GuardOff;
        player.motion_state_alias = None;
        player.melee_action_state_id = Some(SOURCE_DOWN_BOUND_U_ACTION_STATE_ID);
        player.source_action_key = Some(SOURCE_DOWN_BOUND_U_ACTION_KEY);
        player.grounded = true;
        player.facing = -1;
        set_source_position_x_source(player, 19.931_313);
        set_source_position_y_source(player, 27.200_1);
        player.source_coll_cur_pos = player.source_position;
        player.source_coll_floor_line_index = Some(4);
        player.source_coll_floor_surface_index = Some(2);
        source_mp_coll_load_ecb_inline(player, common_data, 5);
        player.source_coll_ecb = player.source_coll_desired_ecb;
        player.source_coll_prev_ecb = player.source_coll_desired_ecb;

        let mut direct = player.clone();
        let direct_grounded = source_ft_80082708_allow_ground_to_air(
            stage,
            melee_stage.collision,
            &mut direct,
            common_data,
        );
        let routed_grounded = resolve_ground_support_after_move(
            stage,
            player,
            MotionState::GuardOff,
            0,
            Some(2),
            common_data,
            player.source_position,
        );

        assert_eq!(
            routed_grounded, direct_grounded,
            "ftCo_DownBound_Coll calls ft_80082708 based on the source action state, not the stale Rust motion-state alias"
        );
        assert_eq!(
            player.source_coll_floor_line_index,
            direct.source_coll_floor_line_index
        );
        assert!(
            (player.source_position.x - direct.source_position.x).abs() <= 0.000_01,
            "routed x {:.6} did not match direct ft_80082708 x {:.6}",
            player.source_position.x,
            direct.source_position.x
        );
        assert!(
            (player.source_position.y - direct.source_position.y).abs() <= 0.000_01,
            "routed y {:.6} did not match direct ft_80082708 y {:.6}",
            player.source_position.y,
            direct.source_position.y
        );
    }

    #[test]
    fn ft_80082708_maps_floor_contact_to_ga_air_for_downbound_collision() {
        let stage = StageProfile::battlefield();
        let common_data = MeleeCommonData::PROVISIONAL;
        let melee_stage = stage
            .melee_stage_profile()
            .expect("Battlefield must expose source collision");
        let mut world = World::for_two_players_on_stage(stage);
        world.set_engine_features(crate::EngineFeatureToggles::parity());

        let player = &mut world.players_mut()[1];
        player.motion_state = MotionState::GuardOff;
        player.motion_state_alias = None;
        player.melee_action_state_id = Some(SOURCE_DOWN_BOUND_U_ACTION_STATE_ID);
        player.source_action_key = Some(SOURCE_DOWN_BOUND_U_ACTION_KEY);
        player.grounded = true;
        player.facing = -1;
        set_source_position_x_source(player, 19.931_313);
        set_source_position_y_source(player, 27.200_1);
        player.source_coll_cur_pos = player.source_position;
        player.source_coll_floor_line_index = Some(4);
        player.source_coll_floor_surface_index = Some(2);
        source_mp_coll_load_ecb_inline(player, common_data, 5);
        player.source_coll_ecb = player.source_coll_desired_ecb;
        player.source_coll_prev_ecb = player.source_coll_desired_ecb;

        let mut coll_probe = player.clone();
        let mp_coll_8004b108_returned_true =
            source_mp_coll_8004b108(stage, melee_stage.collision, &mut coll_probe, common_data);
        assert!(
            mp_coll_8004b108_returned_true,
            "this fixture must hit the decomp mpColl_8004B108 touching-floor return path"
        );

        let ft_80082708_returned_ga_air = source_ft_80082708_allow_ground_to_air(
            stage,
            melee_stage.collision,
            player,
            common_data,
        );

        assert!(
            ft_80082708_returned_ga_air,
            "decomp ft_80082708 returns mpColl_8004B108(...) ? GA_Air : GA_Ground; DownBound must not treat a true mpColl_8004B108 result as GA_Ground/Fall"
        );
    }

    #[test]
    fn source_set_airborne_state_one_calls_ftcommon_8007d5d4_without_motion_change() {
        let mut player = PlayerState::new(0, 0, 1);
        player.melee_action_state_id = Some(MeleeActionStateId::new(183));
        player.source_action_key = Some(SourceActionKey::new("DownBoundU"));
        player.motion_state_alias = None;
        player.grounded = true;
        player.ground_velocity_x = -0.25;
        player.jumps_remaining = 0;

        apply_source_script_events(
            &mut player,
            SourceActionScriptEvents::single(SourceActionScriptEvent::SetAirborneState {
                state: 1,
            }),
            MeleeCommonData::PROVISIONAL,
        );

        assert_eq!(
            player.melee_action_state_id,
            Some(MeleeActionStateId::new(183)),
            "ftAction_80071998 state 1 calls ftCommon_8007D5D4 and does not change motion"
        );
        assert_eq!(
            player.source_action_key,
            Some(SourceActionKey::new("DownBoundU"))
        );
        assert_eq!(player.motion_state_alias, None);
        assert!(!player.grounded);
        assert_eq!(player.ground_velocity_x, 0.0);
        assert_eq!(
            player.jumps_remaining,
            player.profile.max_jumps.saturating_sub(1)
        );
        assert_eq!(
            player.ecb_bottom_lock_timer,
            GROUND_TO_AIR_ECB_LOCK_FRAMES,
            "ftAction_80071998 state 1 installs ftCommon_8007D5D4; Fighter_procMap consumes the count later in the same full fighter tick"
        );
        assert!(player.source_coll_x130_locked);
    }

    #[test]
    fn source_collision_state_command_controls_global_hurt_admission() {
        let mut player = PlayerState::new(0, 0, 1);

        apply_source_script_events(
            &mut player,
            SourceActionScriptEvents::single(SourceActionScriptEvent::SetCollisionState {
                state: 2,
            }),
            MeleeCommonData::PROVISIONAL,
        );
        assert_eq!(player.source_collision_state, 2);
        assert!(!player.source_allows_hurt_collision());

        apply_source_script_events(
            &mut player,
            SourceActionScriptEvents::single(SourceActionScriptEvent::SetCollisionState {
                state: 0,
            }),
            MeleeCommonData::PROVISIONAL,
        );
        assert_eq!(player.source_collision_state, 0);
        assert!(player.source_allows_hurt_collision());
    }

    #[test]
    fn source_set_airborne_state_zero_calls_ftcommon_8007d7fc_without_self_velocity_clamp() {
        let mut player = PlayerState::new(0, 0, 1);
        let source_velocity_x = -3.566_990_6_f32;
        player.grounded = false;
        player.source_self_velocity_x = source_velocity_x;
        player.ground_velocity_x = 1.25;
        player.jumps_remaining = 0;
        player.ecb_bottom_lock_timer = 2;
        player.source_coll_x130_locked = true;

        apply_source_script_events(
            &mut player,
            SourceActionScriptEvents::single(SourceActionScriptEvent::SetAirborneState {
                state: 0,
            }),
            MeleeCommonData::PROVISIONAL,
        );

        assert!(player.grounded);
        assert_eq!(
            player.ground_velocity_x.to_bits(),
            source_velocity_x.to_bits(),
            "ftAction_80071998 state 0 calls ftCommon_8007D7FC/8007D6A4; the final gr_vel assignment uses self_vel.x, not the pre-assignment ClampGrVel result"
        );
        assert_eq!(
            player.source_self_velocity_x.to_bits(),
            source_velocity_x.to_bits()
        );
        assert_eq!(player.jumps_remaining, player.profile.reusable_air_jumps());
        assert_eq!(player.ecb_bottom_lock_timer, 0);
        assert!(!player.source_coll_x130_locked);
    }

    #[test]
    fn source_set_airborne_state_two_calls_ftcommon_8007d60c_lock_count() {
        let mut player = PlayerState::new(0, 0, 1);
        player.grounded = true;
        player.ground_velocity_x = 0.5;
        player.jumps_remaining = 1;

        apply_source_script_events(
            &mut player,
            SourceActionScriptEvents::single(SourceActionScriptEvent::SetAirborneState {
                state: 2,
            }),
            MeleeCommonData::PROVISIONAL,
        );

        assert!(!player.grounded);
        assert_eq!(player.ground_velocity_x, 0.0);
        assert_eq!(player.jumps_remaining, 0);
        assert_eq!(
            player.ecb_bottom_lock_timer, AIRBORNE_STATE_TWO_ECB_LOCK_FRAMES,
            "ftCommon_8007D60C sets ecb_lock to 5, distinct from ftCommon_8007D5D4's 10-frame lock"
        );
        assert!(player.source_coll_x130_locked);
    }

    #[test]
    fn source_down_bound_consumes_action_script_airborne_state_event() {
        let stage = StageProfile::battlefield();
        let common_data = MeleeCommonData::PROVISIONAL;
        let mut player = PlayerState::new(0, 0, -1);
        player.melee_action_state_id = Some(SOURCE_DOWN_BOUND_U_ACTION_STATE_ID);
        player.source_action_key = Some(SOURCE_DOWN_BOUND_U_ACTION_KEY);
        player.source_action_total_frames = 26;
        player.motion_state_alias = None;
        player.grounded = true;
        player.motion_frame = 3;
        player.source_ground_knockback_velocity = -0.39568788;
        player.source_knockback_velocity_x = -0.39568788;

        let mut source_pose_metadata = |player: &PlayerState| {
            let script_events = if player.motion_frame == 4 {
                SourceActionScriptEvents::single(SourceActionScriptEvent::SetAirborneState {
                    state: 1,
                })
            } else {
                SourceActionScriptEvents::empty()
            };
            Some(SourceActionPoseMetadata {
                script_events,
                ..Default::default()
            })
        };
        let mut source_action_total_frames = |_| Some(26);

        advance_source_down_bound_state(
            &mut player,
            stage,
            common_data,
            Vec2 { x: 0, y: 0 },
            0,
            0,
            &mut source_pose_metadata,
            &mut source_action_total_frames,
        );

        assert_eq!(player.motion_frame, 4);
        assert!(!player.grounded);
        assert_eq!(player.ground_velocity_x, 0.0);
        assert_eq!(
            player.ecb_bottom_lock_timer,
            GROUND_TO_AIR_ECB_LOCK_FRAMES - 1,
            "Fighter_procMap decrements ecb_lock in the same frame after ftAction_80071998 state 1 installs ftCommon_8007D5D4"
        );
        assert!(player.source_coll_x130_locked);
        assert_eq!(
            player.velocity.x,
            source_units_to_milli(player.source_knockback_velocity_x),
            "after ftAction_80071998 state 1 switches DownBound to air, Fighter_procUpdate still exports the surviving x8c_kb_vel.x"
        );
    }

    #[test]
    fn source_down_bound_entry_projects_ground_knockback_like_ftcommon_8007cce8() {
        let stage = StageProfile::battlefield();
        let common_data = MeleeCommonData::PROVISIONAL;
        let mut world = World::for_two_players_on_stage(stage);
        world.set_engine_features(crate::EngineFeatureToggles::parity());

        let player = &mut world.players_mut()[1];
        player.grounded = true;
        player.motion_state = MotionState::Fall;
        player.motion_state_alias = None;
        player.source_down_bound_pose = Some(crate::state::SourceDownBoundPose {
            hip_mtx_0_1: 0.0,
            hip_mtx_0_2: 0.0,
            hip_mtx_1_1: 1.0,
            hip_mtx_1_2: -1.0,
        });
        player.source_knockback_velocity_x = -0.635_688;
        player.source_knockback_velocity_y = 2.064_893;
        player.source_ground_knockback_velocity = 0.0;
        player.source_coll_floor_line_index = Some(4);
        player.source_coll_floor_surface_index = Some(2);
        set_source_position_x_source(player, 22.504_606);
        set_source_position_y_source(player, 27.200_1);
        player.source_coll_cur_pos = player.source_position;

        enter_source_damage_down_bound(player, stage, common_data, &mut |action_state_id| {
            match action_state_id.get() {
                183 | 191 => Some(26),
                _ => None,
            }
        });

        assert_eq!(
            player.melee_action_state_id,
            Some(SOURCE_DOWN_BOUND_U_ACTION_STATE_ID)
        );
        assert_eq!(
            player.source_ground_knockback_velocity.to_bits(),
            (-0.635_688_f32).to_bits(),
            "ftCommon_8007CCE8 seeds xF0_ground_kb_vel from x8c_kb_vel.x on grounded DownBound entry"
        );
        assert_eq!(
            player.source_knockback_velocity_x.to_bits(),
            (-0.635_688_f32).to_bits()
        );
        assert_eq!(
            player.source_knockback_velocity_y, 0.0,
            "ftCommon_8007CCE8 projects grounded damage knockback onto the floor tangent immediately at DownBound entry"
        );
    }

    #[test]
    fn ordinary_dead_down_waits_source_x500_timer_before_rebirth() {
        let stage = StageProfile::battlefield();
        let common_data = MeleeCommonData::PROVISIONAL;
        let mut player = PlayerState::new(0, 0, 1);
        player.enter_source_dead_motion_state(MotionState::DeadDown);
        player.source_common_timer = 2;

        advance_source_dead_state(&mut player, 0, stage, common_data);

        assert_eq!(player.motion_state, MotionState::DeadDown);
        assert_eq!(player.motion_frame, 1);
        assert_eq!(player.source_common_timer, 1);

        advance_source_dead_state(&mut player, 0, stage, common_data);

        assert_eq!(player.motion_state, MotionState::Rebirth);
        assert_eq!(player.motion_frame, 0);
        assert_eq!(player.source_common_timer, common_data.rebirth_ticks - 1);
        assert_eq!(player.position.x, stage.respawn_platforms[0].final_x);
        assert_eq!(player.position.y, 135_067);
    }
}

fn source_distance_sq(a: SourcePoint, b: SourcePoint) -> f32 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    dx * dx + dy * dy
}

fn collision_ecb_motion_frame(player: &PlayerState) -> u8 {
    match player.motion_state {
        MotionState::EscapeAir => player.motion_frame.saturating_add(1),
        _ => player.motion_frame,
    }
}

fn ecb_bottom_world_position_for_motion_frame(
    player: &PlayerState,
    root_position: Vec2,
    motion_frame: u8,
    common_data: MeleeCommonData,
) -> Vec2 {
    let bottom_offset_y = active_ecb_bottom_offset_y(player, motion_frame, common_data);
    Vec2 {
        x: root_position.x,
        y: root_position.y + bottom_offset_y,
    }
}

fn snap_player_to_floor_contact(player: &mut PlayerState, y: i32) {
    let landing_velocity_x = player.source_self_velocity_x;
    let root_y = if player.ecb_bottom_offset_y > 0 {
        y
    } else {
        y - player.ecb_bottom_offset_y
    };
    set_source_position_y_milli(player, root_y);
    player.ecb_bottom_lock_timer = 0;
    player.source_coll_x130_locked = false;
    player.grounded = true;
    player.fast_falling = false;
    set_ground_velocity_x(player, landing_velocity_x);
    player.jumps_remaining = player.profile.reusable_air_jumps();
    player.jump_input = Default::default();
    player.short_hop = false;
    player.floor_skip_surface = None;
}

fn lock_ground_to_air_ecb_bottom(player: &mut PlayerState) {
    lock_ecb_bottom_for_frames(player, GROUND_TO_AIR_ECB_LOCK_FRAMES);
}

fn lock_ecb_bottom_for_frames(player: &mut PlayerState, frames: u8) {
    player.ecb_bottom_offset_y = 0;
    player.ecb_bottom_lock_timer = frames;
    player.source_coll_x130_locked = true;
}

fn source_fighter_proc_map_begin(player: &mut PlayerState) {
    tick_ecb_bottom_lock(player);
}

fn tick_ecb_bottom_lock(player: &mut PlayerState) {
    if player.ecb_bottom_lock_timer == 0 {
        return;
    }
    player.ecb_bottom_lock_timer -= 1;
    if player.ecb_bottom_lock_timer == 0 {
        player.source_coll_x130_locked = false;
        if !player.grounded {
            player.ecb_bottom_offset_y = SOURCE_JOBJ_ECB_BOTTOM_OFFSET_Y;
        }
    }
}

fn enter_landing_from_airborne(
    player: &mut PlayerState,
    airborne_motion_state: MotionState,
    impact_velocity_y: f32,
    _trigger_timer: u8,
    common_data: MeleeCommonData,
) {
    let Some(landing_motion_state) = attack_air_landing_state(airborne_motion_state) else {
        if source_landing_callback_enters_wait(impact_velocity_y, common_data) {
            enter_wait_from_airborne_contact(player);
        } else {
            enter_landing_as(player, MotionState::Landing, 0);
        }
        return;
    };

    if player.motion_cmd_var0 == 0 {
        enter_landing_as(player, MotionState::Landing, 0);
        return;
    }

    let base_lag = landing_air_base_lag_ticks(landing_motion_state, player.profile);
    let lag_ticks =
        landing_air_lag_with_lcancel(base_lag, player.source_lcancel_timer, common_data);
    enter_landing_as(player, landing_motion_state, lag_ticks);
}

fn source_landing_callback_enters_wait(
    impact_velocity_y: f32,
    common_data: MeleeCommonData,
) -> bool {
    impact_velocity_y > -common_data.landing_wait_y_velocity_threshold
}

fn enter_wait_from_airborne_contact(player: &mut PlayerState) {
    let landing_velocity_x = player.source_self_velocity_x;
    clear_shield_turn(player);
    clear_turn_state(player);
    player.set_motion_state_alias(MotionState::Wait);
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.motion_cmd_var0 = 0;
    player.motion_cmd_var1 = 0;
    player.landing_lag_ticks = 0;
    player.grounded = true;
    player.fast_falling = false;
    set_ground_velocity_x(player, landing_velocity_x);
}

fn enter_landing_as(player: &mut PlayerState, motion_state: MotionState, landing_lag_ticks: u8) {
    let landing_velocity_x = player.source_self_velocity_x;
    clear_shield_turn(player);
    clear_turn_state(player);
    player.set_motion_state_alias(motion_state);
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.motion_cmd_var0 = 0;
    player.motion_cmd_var1 = 0;
    player.landing_lag_ticks = landing_lag_ticks;
    if motion_state == MotionState::Landing {
        player.set_source_motion_anim_rate_milli(1_000);
    }
    player.grounded = true;
    player.fast_falling = false;
    set_ground_velocity_x(player, landing_velocity_x);
}

fn enter_air_special(player: &mut PlayerState, motion_state: MotionState, stick_x: i32) {
    player.set_motion_state_alias(motion_state);
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.fast_falling = false;
    if motion_state == MotionState::SpecialAirHi {
        init_falcon_special_hi_source_vars(player);
    }
    if matches!(
        motion_state,
        MotionState::SpecialAirSStart | MotionState::SpecialAirS
    ) && stick_x != 0
    {
        player.facing = stick_x.signum() as i8;
    }
}

fn init_falcon_special_hi_source_vars(player: &mut PlayerState) {
    let attrs = player.profile.captain_special_attrs;
    player.jumps_remaining = 0;
    player.captain_special_hi_x0 = attrs.specialhi_air_var as u16;
    player.motion_cmd_var0 = 0;
    player.motion_cmd_var1 = attrs.specialhi_unk2 as u32;
    player.captain_special_hi_vel_x = 0.0;
    player.captain_special_hi_vel_y = 0.0;
    player.captain_special_hi_x2_b0 = false;
    player.captain_special_hi_x2_b1 = false;
}

fn ground_jump_motion_state(
    player: &PlayerState,
    stick_x: i32,
    common_data: MeleeCommonData,
) -> MotionState {
    if stick_x * player.facing as i32 > -(common_data.air_jump_backward_x as i32) {
        MotionState::JumpF
    } else {
        MotionState::JumpB
    }
}

fn enter_air_jump(player: &mut PlayerState, stick_x: i32, common_data: MeleeCommonData) {
    // ftCo_JumpAerial_Enter_Basic calls ftCommon_8007D5D4 before changing state.
    lock_ground_to_air_ecb_bottom(player);
    player.set_motion_state_alias(
        if stick_x * player.facing as i32 > -(common_data.air_jump_backward_x as i32) {
            MotionState::JumpAerialF
        } else {
            MotionState::JumpAerialB
        },
    );
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.set_source_motion_anim_rate_milli(1_000);
    set_source_self_velocity_x(
        player,
        stick_scaled_velocity_source(stick_x, player.profile.air_jump_horizontal_multiplier),
    );
    set_source_self_velocity_y(
        player,
        player.profile.jump_vertical_initial_velocity * player.profile.air_jump_vertical_multiplier,
    );
    player.fast_falling = false;
    player.jumps_remaining -= 1;
    apply_air_drift(player, stick_x, common_data);
}

fn enter_air_attack(player: &mut PlayerState, motion_state: MotionState) {
    player.set_motion_state_alias(motion_state);
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.source_allow_interrupt = false;
    player.motion_cmd_var0 = 0;
    player.motion_cmd_var1 = 0;
    player.motion_throw_flags = 0;
    player.landing_lag_ticks = 0;
    apply_attack_air_script_events(player);
    sample_source_motion_frame_for_action_entry(player);
}

fn apply_airborne_iasa_or_drift(
    player: &mut PlayerState,
    input_facts: MeleeInputFacts,
    stick_x: i32,
    stick_y: i8,
    common_data: MeleeCommonData,
) -> bool {
    if apply_airborne_iasa_actions(player, input_facts, stick_x, stick_y, common_data) {
        true
    } else {
        apply_air_drift(player, stick_x, common_data);
        false
    }
}

fn apply_airborne_iasa_actions(
    player: &mut PlayerState,
    input_facts: MeleeInputFacts,
    stick_x: i32,
    stick_y: i8,
    common_data: MeleeCommonData,
) -> bool {
    if input_facts.special_pressed {
        enter_air_special(
            player,
            air_special_state_from_direction(input_facts.air_special_direction),
            stick_x,
        );
        true
    } else if input_facts.air_dodge_pressed {
        enter_escape_air(player, stick_x, stick_y, common_data);
        true
    } else if input_facts.air_attack_pressed {
        enter_air_attack(
            player,
            air_attack_state_from_direction(input_facts.air_attack_direction, player.facing),
        );
        apply_air_drift(player, stick_x, common_data);
        true
    } else if input_facts.normal_jump_pressed && player.jumps_remaining > 0 {
        enter_air_jump(player, stick_x, common_data);
        true
    } else {
        false
    }
}

fn apply_attack_air_iasa_actions(
    player: &mut PlayerState,
    input_facts: MeleeInputFacts,
    stick_x: i32,
    common_data: MeleeCommonData,
) -> bool {
    if !player.source_allow_interrupt {
        return false;
    }
    if input_facts.normal_jump_pressed && player.jumps_remaining > 0 {
        enter_air_jump(player, stick_x, common_data);
        true
    } else {
        false
    }
}

fn apply_entered_airborne_action_physics(
    player: &mut PlayerState,
    stick_x: i32,
    common_data: MeleeCommonData,
) -> bool {
    if player.motion_state == MotionState::SpecialAirHi {
        apply_falcon_special_hi_physics(player, stick_x, common_data);
        true
    } else {
        false
    }
}

fn enter_ground_escape(player: &mut PlayerState, motion_state: MotionState) {
    clear_guard_state(player);
    clear_turn_state(player);
    clear_platform_pass_pending(player);
    clear_motion_script_state(player);
    fighter_change_motion_state(player, motion_state, 0, 0.0, 1.0, 0.0);
    clear_ground_horizontal_velocity(player);
    player.velocity.y = 0;
}

fn apply_walk_velocity(player: &mut PlayerState, stick_x: i32, common_data: MeleeCommonData) {
    if stick_x > 0 {
        player.facing = 1;
        player.source_model_facing = 1;
    } else if stick_x < 0 {
        player.facing = -1;
        player.source_model_facing = -1;
    }

    let profile = player.profile;
    let accel_mul = player.walk_accel_mul_milli as f32 / 1000.0;
    let target_velocity =
        stick_scaled_velocity_source(stick_x, profile.walk_max_velocity) * accel_mul;
    let mut accel =
        stick_scaled_velocity_source(stick_x, profile.walk_initial_velocity) * accel_mul;
    if stick_x > 0 {
        accel += profile.walk_accel * accel_mul;
    } else if stick_x < 0 {
        accel -= profile.walk_accel * accel_mul;
    }
    accel = apply_remaining_velocity_taper(
        player.ground_velocity_x,
        accel,
        target_velocity,
        common_data.walk_accel_taper,
    );

    let next_velocity = apply_ground_accel_toward_target(
        player.ground_velocity_x,
        accel,
        target_velocity,
        profile.ground_friction,
        profile.ground_max_horizontal_velocity,
    );
    player.walk_anim_velocity_x = target_velocity * common_data.animation_velocity_scale;
    stage_ground_velocity_x(player, next_velocity);
}

fn apply_air_drift(player: &mut PlayerState, stick_x: i32, common_data: MeleeCommonData) {
    let profile = player.profile;
    let stick_x = fighter_input_cleanup_stick_axis(stick_x, common_data.main_stick_deadzone_x);
    let current_velocity = player.source_self_velocity_x;
    let target_velocity = stick_scaled_velocity_source(stick_x, profile.air_drift_max);
    if target_velocity == 0.0 {
        set_source_self_velocity_x(
            player,
            apply_friction_to_zero(current_velocity, profile.aerial_friction),
        );
        return;
    }

    let stick_accel = stick_scaled_velocity_source(stick_x, profile.air_drift_stick_multiplier);
    let base_accel = stick_x.signum() as f32 * profile.aerial_drift_base;
    let accel = air_accel_for_velocity_source(
        current_velocity,
        stick_accel + base_accel,
        target_velocity,
        profile.aerial_friction,
        profile.air_max_horizontal_velocity,
    );
    set_source_self_velocity_x(player, current_velocity + accel);
}

fn apply_falcon_special_hi_iasa(player: &mut PlayerState, stick_x: i32) {
    if player.motion_cmd_var0 == 0 {
        return;
    }

    player.motion_cmd_var0 = 0;
    player.captain_special_hi_x2_b1 = true;
    let stick = fighter_stick_axis_to_f32(stick_x.clamp(-127, 127) as i8);
    if stick.abs() > player.profile.captain_special_attrs.specialhi_input_var && stick_x != 0 {
        player.facing = stick_x.signum() as i8;
        player.source_model_facing = player.facing;
    }
}

fn apply_falcon_special_hi_physics(
    player: &mut PlayerState,
    stick_x: i32,
    common_data: MeleeCommonData,
) {
    let attrs = player.profile.captain_special_attrs;
    let self_vel_x = player.captain_special_hi_vel_x;
    let self_vel_y = player.captain_special_hi_vel_y;
    let max_vel = attrs.specialhi_horz_vel * player.profile.air_drift_max;
    let (hit_speed_clamp, mut anim_vel_x) =
        source_ft_common_8007d050_anim_vel(self_vel_x, max_vel, player.profile, common_data);

    if !hit_speed_clamp {
        let stick_x = fighter_input_cleanup_stick_axis(stick_x, common_data.main_stick_deadzone_x);
        anim_vel_x = source_ft_common_8007d3a8_anim_vel(
            self_vel_x,
            stick_x,
            common_data.special_air_drift_stick_threshold,
            player.profile.air_drift_stick_multiplier * attrs.specialhi_air_friction_mul,
            player.profile.air_drift_max * attrs.specialhi_horz_vel,
        );
    }

    player.captain_special_hi_vel_x = anim_vel_x + self_vel_x;
    player.captain_special_hi_vel_y = self_vel_y;
    let (root_x, root_y) = source_special_hi_transn_self_velocity(player);
    set_source_self_velocity(
        player,
        root_x + player.captain_special_hi_vel_x,
        root_y + player.captain_special_hi_vel_y,
    );
}

fn source_ft_common_8007d050_anim_vel(
    self_vel_x: f32,
    max_vel: f32,
    profile: crate::FighterProfile,
    common_data: MeleeCommonData,
) -> (bool, f32) {
    if self_vel_x.abs() > max_vel {
        (
            true,
            source_apply_friction_air_anim_vel(self_vel_x, common_data.air_speed_clamp_friction),
        )
    } else {
        (
            false,
            source_apply_friction_air_anim_vel(self_vel_x, profile.aerial_friction),
        )
    }
}

fn source_apply_friction_air_anim_vel(self_vel_x: f32, friction: f32) -> f32 {
    if friction.abs() >= self_vel_x.abs() {
        -self_vel_x
    } else if self_vel_x > 0.0 {
        -friction
    } else {
        friction
    }
}

fn source_ft_common_8007d3a8_anim_vel(
    self_vel_x: f32,
    stick_x: i32,
    threshold: f32,
    accel_max: f32,
    target_max: f32,
) -> f32 {
    let stick = fighter_stick_axis_to_f32(stick_x.clamp(-127, 127) as i8);
    let (accel, target_vel) = if stick.abs() >= threshold {
        (stick * accel_max, stick * target_max)
    } else {
        (0.0, 0.0)
    };
    source_ft_common_8007d2e8_anim_vel(self_vel_x, accel, target_vel)
}

fn source_ft_common_8007d2e8_anim_vel(self_vel_x: f32, mut accel: f32, target_vel: f32) -> f32 {
    if target_vel == 0.0 {
        accel = -self_vel_x;
    } else if !(self_vel_x * accel < 0.0) {
        if accel > 0.0 {
            if self_vel_x + accel > target_vel {
                accel = target_vel - self_vel_x;
            }
        } else if self_vel_x + accel < target_vel {
            accel = target_vel - self_vel_x;
        }
    }
    accel
}

fn source_special_hi_transn_self_velocity(player: &PlayerState) -> (f32, f32) {
    let source_frame = player.motion_frame.saturating_add(2);
    source_root_motion_delta(player.motion_state, source_frame)
        .map(|delta| {
            (
                delta.z * source_root_motion_model_scale(player) * f32::from(player.facing),
                delta.y * source_root_motion_model_scale(player),
            )
        })
        .unwrap_or((0.0, 0.0))
}

fn apply_dash_velocity(
    player: &mut PlayerState,
    stick_x: i32,
    stage: StageProfile,
    common_data: MeleeCommonData,
) {
    let (accel, target_velocity) = dash_run_accel_and_target(player.profile, stick_x);
    let next_velocity = apply_ground_accel_toward_target(
        player.ground_velocity_x,
        accel,
        target_velocity,
        run_ground_friction(player, stage, common_data),
        player.profile.ground_max_horizontal_velocity,
    );
    stage_ground_velocity_x(player, next_velocity);
}

fn apply_dash_physics(
    player: &mut PlayerState,
    stick_x: i32,
    stage: StageProfile,
    common_data: MeleeCommonData,
) {
    let stick_x = fighter_input_cleanup_stick_axis(stick_x, common_data.main_stick_deadzone_x);
    if player.dash_x0 != 0.0 {
        player.dash_x0 = 0.0;
    } else if stick_x == 0 {
        apply_run_ground_traction(player, stage, common_data);
    } else {
        apply_dash_velocity(player, stick_x, stage, common_data);
    }
}

fn apply_run_velocity(
    player: &mut PlayerState,
    stick_x: i32,
    stage: StageProfile,
    common_data: MeleeCommonData,
) {
    let (accel, target_velocity) = dash_run_accel_and_target(player.profile, stick_x);
    let accel = apply_remaining_velocity_taper(
        player.ground_velocity_x,
        accel,
        target_velocity,
        common_data.run_accel_taper,
    );
    let next_velocity = apply_ground_accel_toward_target(
        player.ground_velocity_x,
        accel,
        target_velocity,
        run_ground_friction(player, stage, common_data),
        player.profile.ground_max_horizontal_velocity,
    );
    stage_ground_velocity_x(player, next_velocity);
}

fn enter_wait_from_walk(player: &mut PlayerState) {
    player.set_motion_state_alias(MotionState::Wait);
    player.motion_frame = 0;
}

fn is_same_direction_run(stick_x: i32, facing: i8, common_data: MeleeCommonData) -> bool {
    stick_x.abs() >= common_data.run_x as i32 && stick_x.signum() == facing as i32
}

fn is_opposite_run_turn(stick_x: i32, facing: i8, common_data: MeleeCommonData) -> bool {
    stick_x * facing as i32 <= common_data.turn_run_x as i32
}

fn run_direct_releases_to_wait(stick_x: i32, facing: i8, common_data: MeleeCommonData) -> bool {
    stick_x * (facing as i32) < 0 || stick_x.abs() < common_data.walk_x as i32
}

fn apply_ground_traction(
    player: &mut PlayerState,
    stage: StageProfile,
    common_data: MeleeCommonData,
) {
    let mut traction = player.profile.ground_friction
        * floor_friction_multiplier_for_bottom(stage, player.position);
    if player.ground_velocity_x.abs() > player.profile.walk_max_velocity {
        traction = traction * common_data.high_speed_ground_friction_multiplier;
    }
    let next_velocity = apply_friction_to_zero(player.ground_velocity_x, traction);
    stage_ground_velocity_x(player, next_velocity);
    apply_source_ground_movement(player, stage);
}

fn apply_catch_ground_physics(
    player: &mut PlayerState,
    stage: StageProfile,
    common_data: MeleeCommonData,
) {
    let friction = player.profile.ground_friction * common_data.catch_ground_friction_multiplier;
    apply_source_ground_friction(player, stage, friction);
}

fn apply_source_ground_friction(player: &mut PlayerState, stage: StageProfile, friction: f32) {
    let floor_multiplier = floor_friction_multiplier_for_bottom(stage, player.position);
    let friction = if floor_multiplier < 1.0 {
        friction * floor_multiplier
    } else {
        friction
    };
    let next_velocity = apply_friction_to_zero(player.ground_velocity_x, friction);
    stage_ground_velocity_x(player, next_velocity);
    apply_source_ground_movement(player, stage);
}

fn apply_run_ground_traction(
    player: &mut PlayerState,
    stage: StageProfile,
    common_data: MeleeCommonData,
) {
    let next_velocity = apply_friction_to_zero(
        player.ground_velocity_x,
        run_ground_friction(player, stage, common_data),
    );
    stage_ground_velocity_x(player, next_velocity);
    apply_source_ground_movement(player, stage);
}

fn run_ground_friction(
    player: &PlayerState,
    stage: StageProfile,
    common_data: MeleeCommonData,
) -> f32 {
    player.profile.ground_friction
        * common_data.run_ground_friction_multiplier
        * floor_friction_multiplier_for_bottom(stage, player.position)
}

fn dash_iasa_decayed_ground_velocity(velocity_x: f32, common_data: MeleeCommonData) -> f32 {
    const DEFAULT_FLOOR_FRICTION_MULTIPLIER: f32 = 1.0;
    velocity_x - velocity_x * common_data.dash_velocity_decay * DEFAULT_FLOOR_FRICTION_MULTIPLIER
}

fn stick_scaled_velocity_source(stick_x: i32, full_stick_velocity: f32) -> f32 {
    let axis = fighter_stick_axis_to_f32(stick_x.clamp(-127, 127) as i8);
    axis * full_stick_velocity
}

fn dash_run_accel_and_target(profile: crate::FighterProfile, stick_x: i32) -> (f32, f32) {
    let stick = fighter_stick_axis_to_f32(stick_x.clamp(-127, 127) as i8);
    let base_accel = if stick > 0.0 {
        profile.dash_run_acceleration_b
    } else {
        -profile.dash_run_acceleration_b
    };
    let accel = stick * profile.dash_run_acceleration_a + base_accel;
    let target_velocity = stick * profile.dash_run_terminal_velocity;

    (accel, target_velocity)
}

fn apply_remaining_velocity_taper(
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

fn air_accel_for_velocity_source(
    current_velocity: f32,
    mut accel: f32,
    target_velocity: f32,
    friction: f32,
    max_horizontal_velocity: f32,
) -> f32 {
    if current_velocity * accel < 0.0 {
        return accel;
    }

    if accel > 0.0 && current_velocity + accel > target_velocity {
        accel = -friction;
        if current_velocity + accel < target_velocity {
            accel = target_velocity - current_velocity;
        }
        if current_velocity + accel > max_horizontal_velocity {
            accel = max_horizontal_velocity - current_velocity;
        }
    } else if accel < 0.0 && current_velocity + accel < target_velocity {
        accel = friction;
        if current_velocity + accel > target_velocity {
            accel = target_velocity - current_velocity;
        }
        if current_velocity + accel < -max_horizontal_velocity {
            accel = -max_horizontal_velocity - current_velocity;
        }
    }

    accel
}

fn apply_escape_air_decay(player: &mut PlayerState, common_data: MeleeCommonData) {
    seed_source_self_velocity_from_projected_velocity(player);
    player.source_self_velocity_x *= common_data.escapeair_decay;
    player.source_self_velocity_y *= common_data.escapeair_decay;
    player.velocity.x = source_units_to_milli(player.source_self_velocity_x);
    player.velocity.y = source_units_to_milli(player.source_self_velocity_y);
}

fn set_source_self_velocity(player: &mut PlayerState, velocity_x: f32, velocity_y: f32) {
    player.source_self_velocity_x = velocity_x;
    player.source_self_velocity_y = velocity_y;
    player.velocity.x = source_units_to_milli(velocity_x);
    player.velocity.y = source_units_to_milli(velocity_y);
}

fn set_source_self_velocity_x(player: &mut PlayerState, velocity_x: f32) {
    player.source_self_velocity_x = velocity_x;
    player.velocity.x = source_units_to_milli(velocity_x);
}

fn set_source_self_velocity_y(player: &mut PlayerState, velocity_y: f32) {
    player.source_self_velocity_y = velocity_y;
    player.velocity.y = source_units_to_milli(velocity_y);
}

fn seed_source_self_velocity_from_projected_velocity(player: &mut PlayerState) {
    if player.source_self_velocity_x == 0.0 && player.velocity.x != 0 {
        player.source_self_velocity_x = milli_to_source_units(player.velocity.x);
    }
    if player.source_self_velocity_y == 0.0 && player.velocity.y != 0 {
        player.source_self_velocity_y = milli_to_source_units(player.velocity.y);
    }
}

fn fighter_input_cleanup_stick_axis(value: i32, deadzone: i8) -> i32 {
    if value.abs() <= deadzone.abs() as i32 {
        0
    } else {
        value
    }
}

fn escape_air_velocity(stick_x: i32, stick_y: i32, common_data: MeleeCommonData) -> (f32, f32) {
    if stick_x.abs() < common_data.escapeair_deadzone_x as i32
        && stick_y.abs() < common_data.escapeair_deadzone_y as i32
    {
        return (0.0, 0.0);
    }

    let stick_x = fighter_stick_axis_to_f32(stick_x.clamp(-127, 127) as i8);
    let stick_y = fighter_stick_axis_to_f32(stick_y.clamp(-127, 127) as i8);
    let angle = stick_y.atan2(stick_x);
    (
        common_data.escapeair_force * angle.cos(),
        common_data.escapeair_force * angle.sin(),
    )
}

fn apply_ground_accel_toward_target(
    current_velocity: f32,
    mut accel: f32,
    target_velocity: f32,
    friction_per_tick: f32,
    max_velocity: f32,
) -> f32 {
    let friction_per_tick = friction_per_tick.abs();
    if target_velocity == 0.0 {
        return apply_friction_to_zero(current_velocity, friction_per_tick);
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

fn apply_friction_to_zero(current_velocity: f32, friction: f32) -> f32 {
    if current_velocity > friction {
        current_velocity - friction
    } else if current_velocity < -friction {
        current_velocity + friction
    } else {
        0.0
    }
}

fn wait_action_state(facts: MeleeInputFacts) -> Option<MotionState> {
    if facts.special_pressed {
        if let Some(special_state) = special_state_from_direction(facts.special_direction) {
            return Some(special_state);
        }
    }
    if facts.grab_pressed {
        return Some(MotionState::Catch);
    }
    grounded_attack_state_from_source_order(facts)
}

fn walk_action_state(facts: MeleeInputFacts) -> Option<MotionState> {
    if facts.grab_pressed || (facts.shield_held && facts.attack_pressed) {
        return Some(MotionState::Catch);
    }
    if facts.special_pressed {
        if let Some(special_state) = walk_special_state_from_direction(facts.special_direction) {
            return Some(special_state);
        }
    }
    grounded_attack_state_from_source_order(facts)
}

fn fresh_walk_forward_dash_direction(
    facts: MeleeInputFacts,
    x_tap_timer: u8,
    facing: i8,
    common_data: MeleeCommonData,
) -> i8 {
    if is_fresh_walk_dash_tap(x_tap_timer, common_data) {
        facts.forward_dash_direction(facing)
    } else {
        0
    }
}

fn fresh_walk_smash_turn_direction(
    facts: MeleeInputFacts,
    x_tap_timer: u8,
    facing: i8,
    common_data: MeleeCommonData,
) -> i8 {
    if is_fresh_walk_dash_tap(x_tap_timer, common_data) {
        facts.smash_turn_direction(facing)
    } else {
        0
    }
}

fn is_fresh_walk_dash_tap(x_tap_timer: u8, common_data: MeleeCommonData) -> bool {
    x_tap_timer < common_data.dash_tap_window
}

fn dash_action_state(
    facts: MeleeInputFacts,
    motion_frame: u8,
    started_from_tap: bool,
    facing: i8,
    trigger_timer: u8,
    common_data: MeleeCommonData,
) -> Option<MotionState> {
    if facts.special_pressed && matches!(facts.special_direction, (1 | -1, 0)) {
        return Some(MotionState::SpecialSStart);
    }
    if facts.shield_held && facts.attack_pressed {
        return Some(MotionState::CatchDash);
    }
    if motion_frame < common_data.dash_early_action_window
        && dash_early_side_smash_input(facts, facing)
    {
        return Some(MotionState::AttackS4);
    }
    if started_from_tap
        && motion_frame <= common_data.dash_defensive_action_window
        && facts.digital_shield_held
    {
        return Some(MotionState::EscapeF);
    }
    let in_late_dash_attack_window = (common_data.dash_early_action_window
        ..common_data.dash_late_action_window)
        .contains(&motion_frame);
    if in_late_dash_attack_window && facts.attack_pressed {
        return Some(MotionState::AttackDash);
    }
    if facts.digital_shield_pressed && trigger_timer < common_data.guard_reflect_input_window {
        return Some(MotionState::GuardReflect);
    }
    None
}

fn dash_allows_opposite_dashback(player: &PlayerState, common_data: MeleeCommonData) -> bool {
    !player.dash_started_from_tap || player.motion_frame >= common_data.dash_early_action_window
}

fn dash_early_side_smash_input(facts: MeleeInputFacts, facing: i8) -> bool {
    let forward_attack = facts.attack_pressed && facts.held_dash_x_direction == facing;
    let cstick_side_smash = matches!(facts.cstick_smash_direction, (1 | -1, 0));
    forward_attack || cstick_side_smash
}

fn run_action_state(facts: MeleeInputFacts) -> Option<MotionState> {
    if facts.special_pressed {
        if let Some(special_state) = walk_special_state_from_direction(facts.special_direction) {
            return Some(special_state);
        }
    }
    if facts.shield_held && facts.attack_pressed {
        return Some(MotionState::CatchDash);
    }
    facts.attack_pressed.then_some(MotionState::AttackDash)
}

#[derive(Clone, Copy)]
struct GuardInputContext {
    facing: i8,
    stage: StageProfile,
    y_tap_timer: u8,
    stick_y: i8,
    ucf_shield_drop_amendment: bool,
    common_data: MeleeCommonData,
}

fn turn_action_state(facts: MeleeInputFacts) -> Option<MotionState> {
    if facts.special_pressed {
        match facts.special_direction {
            (1 | -1, 0) => return Some(MotionState::SpecialSStart),
            (0, -1) => return Some(MotionState::SpecialLw),
            (0, 1) => return Some(MotionState::SpecialHi),
            _ => {}
        }
    }
    if facts.grab_pressed {
        return Some(MotionState::Catch);
    }
    grounded_attack_state_from_source_order(facts)
}

fn grounded_attack_state_from_source_order(facts: MeleeInputFacts) -> Option<MotionState> {
    if facts.smash_attack_direction.0 != 0 || facts.cstick_smash_direction.0 != 0 {
        return Some(MotionState::AttackS4);
    }
    if facts.smash_attack_direction == (0, 1) || facts.cstick_smash_direction == (0, 1) {
        return Some(MotionState::AttackHi4);
    }
    if facts.smash_attack_direction == (0, -1) || facts.cstick_smash_direction == (0, -1) {
        return Some(MotionState::AttackLw4);
    }
    if let Some(tilt_state) = tilt_state_from_direction(facts.tilt_attack_direction) {
        return Some(tilt_state);
    }
    facts.neutral_attack_pressed.then_some(MotionState::Attack1)
}

fn guard_on_action_state(
    player: &mut PlayerState,
    facts: MeleeInputFacts,
    context: GuardInputContext,
) -> Option<MotionState> {
    if facts.cstick_spot_dodge {
        return Some(MotionState::EscapeN);
    }

    if context.ucf_shield_drop_amendment {
        if let Some(pass_state) = guard_platform_pass_state(player, facts, context) {
            return Some(pass_state);
        }
    }

    if facts.main_stick_spot_dodge {
        return Some(MotionState::EscapeN);
    }

    if facts.roll_direction != 0 {
        return if facts.roll_direction == context.facing {
            Some(MotionState::EscapeF)
        } else {
            Some(MotionState::EscapeB)
        };
    }

    if player.guard_catch_dash_window != 0 {
        if facts.attack_pressed {
            return Some(MotionState::CatchDash);
        }
        player.guard_catch_dash_window -= 1;
    }

    if facts.source_held.lr() && facts.attack_pressed {
        return Some(MotionState::Catch);
    }

    if facts.jump_pressed {
        return Some(MotionState::KneeBend);
    }

    guard_platform_pass_state(player, facts, context)
}

fn latch_guard_release_if_needed(player: &mut PlayerState, facts: MeleeInputFacts) {
    if !facts.source_held.lr() {
        player.guard_release_latched = true;
    }
}

fn guard_on_release_exits_this_tick(player: &PlayerState) -> bool {
    player.guard_release_latched
        && player.motion_frame.saturating_add(1) >= guard_startup_total_frames(player)
}

fn guard_action_state(
    player: &PlayerState,
    facts: MeleeInputFacts,
    context: GuardInputContext,
) -> Option<MotionState> {
    if facts.cstick_spot_dodge {
        return Some(MotionState::EscapeN);
    }

    if context.ucf_shield_drop_amendment {
        if let Some(pass_state) = guard_platform_pass_state(player, facts, context) {
            return Some(pass_state);
        }
    }

    if facts.main_stick_spot_dodge {
        return Some(MotionState::EscapeN);
    }

    if facts.roll_direction != 0 {
        return if facts.roll_direction == context.facing {
            Some(MotionState::EscapeF)
        } else {
            Some(MotionState::EscapeB)
        };
    }

    if facts.source_held.lr() && facts.attack_pressed {
        return Some(MotionState::Catch);
    }

    if facts.jump_pressed {
        return Some(MotionState::KneeBend);
    }

    guard_platform_pass_state(player, facts, context)
}

fn guard_platform_pass_state(
    player: &PlayerState,
    facts: MeleeInputFacts,
    context: GuardInputContext,
) -> Option<MotionState> {
    if !facts.source_held.lr() {
        return None;
    }

    platform_pass_gate(
        player,
        context.stage,
        context.stick_y,
        context.y_tap_timer,
        context.common_data,
    )
    .then_some(MotionState::Pass)
}

fn platform_pass_gate(
    player: &PlayerState,
    stage: StageProfile,
    stick_y: i8,
    y_tap_timer: u8,
    common_data: MeleeCommonData,
) -> bool {
    stick_y <= -common_data.platform_pass_y
        && y_tap_timer < common_data.platform_pass_y_tap_window
        && grounded_on_soft_platform(player, stage)
}

fn grounded_on_soft_platform(player: &PlayerState, stage: StageProfile) -> bool {
    player.grounded
        && floor_surface_for_bottom(stage, player.position)
            .is_some_and(|surface| surface.kind == StageSurfaceKind::Soft)
}

fn guard_off_action_state(facts: MeleeInputFacts) -> Option<MotionState> {
    facts
        .spot_dodge
        .then_some(MotionState::EscapeN)
        .or_else(|| facts.jump_pressed.then_some(MotionState::KneeBend))
}

fn apply_landing_iasa(
    player: &mut PlayerState,
    facts: MeleeInputFacts,
    stick_x: i32,
    stage: StageProfile,
    common_data: MeleeCommonData,
    trigger_timer: u8,
) -> bool {
    if facts.special_pressed && matches!(facts.special_direction, (1 | -1, 0)) {
        enter_action_state(
            player,
            MotionState::SpecialSStart,
            stick_x,
            stage,
            common_data,
        );
        return true;
    }

    if facts.grab_pressed || (facts.shield_held && facts.attack_pressed) {
        enter_action_state(player, MotionState::Catch, stick_x, stage, common_data);
        return true;
    }

    if let Some(attack_state) = grounded_attack_state_from_source_order(facts) {
        enter_action_state(player, attack_state, stick_x, stage, common_data);
        return true;
    }

    if facts.digital_shield_pressed && trigger_timer < common_data.guard_reflect_input_window {
        enter_guard_reflect(player, common_data);
        apply_ground_traction(player, stage, common_data);
        return true;
    }

    if decomp_guard_held_input(facts, common_data) {
        enter_guard(player, facts, common_data);
        source_update_guard_shield_visual(player, facts, common_data);
        apply_ground_traction(player, stage, common_data);
        return true;
    }

    if facts.normal_jump_pressed {
        enter_knee_bend(player, facts.normal_jump_input);
        apply_ground_traction(player, stage, common_data);
        return true;
    }

    let forward_dash = facts.forward_dash_direction(player.facing);
    if forward_dash != 0 {
        enter_dash(player, forward_dash, true);
        return true;
    }

    if player.motion_frame < player.profile.normal_landing_lag_ticks.saturating_add(1)
        && facts.crouch
    {
        enter_squat_wait(player);
        return true;
    }

    let smash_turn = facts.smash_turn_direction(player.facing);
    if smash_turn != 0 {
        enter_smash_turn(player, smash_turn);
        return true;
    }

    let standing_turn = facts.standing_turn_direction(player.facing);
    if standing_turn != 0 {
        enter_standing_turn(player, standing_turn);
        return true;
    }

    if facts.walk_direction != 0 {
        enter_walk(
            player,
            walk_motion_state(player, common_data),
            stick_x,
            common_data,
        );
        return true;
    }

    false
}

fn decomp_guard_held_input(facts: MeleeInputFacts, _common_data: MeleeCommonData) -> bool {
    facts.source_held.lr()
}

fn attack_air_landing_state(motion_state: MotionState) -> Option<MotionState> {
    match motion_state {
        MotionState::AttackAirN => Some(MotionState::LandingAirN),
        MotionState::AttackAirF => Some(MotionState::LandingAirF),
        MotionState::AttackAirB => Some(MotionState::LandingAirB),
        MotionState::AttackAirHi => Some(MotionState::LandingAirHi),
        MotionState::AttackAirLw => Some(MotionState::LandingAirLw),
        _ => None,
    }
}

fn landing_air_base_lag_ticks(motion_state: MotionState, profile: crate::FighterProfile) -> u8 {
    match motion_state {
        MotionState::LandingAirN => profile.landing_air_n_lag_ticks,
        MotionState::LandingAirF => profile.landing_air_f_lag_ticks,
        MotionState::LandingAirB => profile.landing_air_b_lag_ticks,
        MotionState::LandingAirHi => profile.landing_air_hi_lag_ticks,
        MotionState::LandingAirLw => profile.landing_air_lw_lag_ticks,
        _ => 0,
    }
}

fn landing_air_lag_with_lcancel(
    base_lag: u8,
    trigger_timer: u8,
    common_data: MeleeCommonData,
) -> u8 {
    if trigger_timer >= common_data.lcancel_window || common_data.lcancel_divisor <= 0.0 {
        return base_lag;
    }

    (((base_lag as f32) / common_data.lcancel_divisor) as u8)
        .max(1)
        .min(base_lag)
}

fn knee_bend_action_state(
    facts: MeleeInputFacts,
    stick_y: i8,
    common_data: MeleeCommonData,
) -> Option<MotionState> {
    if facts.special_pressed && facts.special_direction == (0, 1) {
        return Some(MotionState::SpecialHi);
    }

    if facts.grab_pressed || (facts.shield_held && facts.attack_pressed) {
        return Some(MotionState::Catch);
    }

    let attack_up_smash = facts.attack_pressed && stick_y >= common_data.smash_y;
    if facts.cstick_smash_direction == (0, 1) || attack_up_smash {
        return Some(MotionState::AttackHi4);
    }

    None
}

fn walk_special_state_from_direction(direction: (i8, i8)) -> Option<MotionState> {
    match direction {
        (1 | -1, 0) => Some(MotionState::SpecialSStart),
        (0, 1) => Some(MotionState::SpecialHi),
        (0, 0) => Some(MotionState::SpecialN),
        (0, -1) => Some(MotionState::SpecialLw),
        NO_GROUNDED_SPECIAL_DIRECTION => None,
        _ => None,
    }
}

fn special_state_from_direction(direction: (i8, i8)) -> Option<MotionState> {
    match direction {
        (1 | -1, 0) => Some(MotionState::SpecialSStart),
        (0, 1) => Some(MotionState::SpecialHi),
        (0, -1) => Some(MotionState::SpecialLw),
        (0, 0) => Some(MotionState::SpecialN),
        NO_GROUNDED_SPECIAL_DIRECTION => None,
        _ => None,
    }
}

fn air_special_state_from_direction(direction: (i8, i8)) -> MotionState {
    match direction {
        (0, 1) => MotionState::SpecialAirHi,
        (0, -1) => MotionState::SpecialAirLw,
        (1 | -1, 0) => MotionState::SpecialAirSStart,
        _ => MotionState::SpecialAirN,
    }
}

fn air_attack_state_from_direction(direction: (i8, i8), facing: i8) -> MotionState {
    match direction {
        (0, 1) => MotionState::AttackAirHi,
        (0, -1) => MotionState::AttackAirLw,
        (1 | -1, 0) if direction.0 == facing => MotionState::AttackAirF,
        (1 | -1, 0) => MotionState::AttackAirB,
        _ => MotionState::AttackAirN,
    }
}

fn tilt_state_from_direction(direction: (i8, i8)) -> Option<MotionState> {
    match direction {
        (1 | -1, 0) => Some(MotionState::AttackS3),
        (0, 1) => Some(MotionState::AttackHi3),
        (0, -1) => Some(MotionState::AttackLw3),
        _ => None,
    }
}
