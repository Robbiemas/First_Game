use crate::input::NO_GROUNDED_SPECIAL_DIRECTION;
use crate::{
    collision::{
        floor_surface_for_bottom, floor_surface_index_for_bottom, has_floor_support,
        landing_contact_for_bottom_with_floor_skip,
    },
    fighter_stick_axis_to_f32,
    state::{
        action_sample_frame_count_for_motion_state, active_ecb_bottom_offset_y,
        EXPIRED_INPUT_TIMER, SOURCE_JOBJ_ECB_BOTTOM_OFFSET_Y,
    },
    Frame, MeleeCommonData, MeleeInputFacts, MeleeJumpInput, MotionState, PlayerInput, PlayerState,
    StageProfile, StageSurfaceKind, Vec2, World,
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
const FALCON_SPECIAL_N_FRAMES: u8 = 99;
const FALCON_SPECIAL_S_FRAMES: u8 = FALCON_SPECIAL_N_FRAMES;
const FALCON_SPECIAL_HI_FRAMES: u8 = FALCON_SPECIAL_N_FRAMES;
const FALCON_SPECIAL_LW_FRAMES: u8 = FALCON_SPECIAL_N_FRAMES;
const FALCON_CATCH_DASH_FRAMES: u8 = 40;
const FALCON_ATTACK_HI3_IASA: u8 = 38;
const FALCON_ATTACK_LW3_IASA: u8 = 35;
const FALCON_ATTACK_S4_IASA: u8 = 60;
const FALCON_ATTACK_HI4_IASA: u8 = 40;
const FALCON_ATTACK_LW4_IASA: u8 = 45;
const FALCON_SPECIAL_N_IASA: u8 = 65;
const GROUND_TO_AIR_ECB_LOCK_FRAMES: u8 = 10;
const UCF_DASHBACK_SOURCE_TURN_FRAME: u8 = 1;

pub fn step_world(world: &mut World, frame: Frame, inputs: &[PlayerInput; 2]) {
    world.set_frame(frame);
    let previous_inputs = *world.previous_inputs();
    let mut input_timers = *world.input_timers();
    let mut last_input_facts = [MeleeInputFacts::default(); 2];
    let stage = world.stage();
    let common_data = world.common_data();

    for (player_index, ((player, input), previous_input)) in world
        .players_mut()
        .iter_mut()
        .zip(inputs.iter().copied())
        .zip(previous_inputs.iter().copied())
        .enumerate()
    {
        let stick_x = input.stick_x() as i32;
        let stick_y = input.stick_y();
        let input_snapshot = input.melee_snapshot_with_config(
            previous_input,
            input_timers[player_index],
            common_data.input_config(),
        );
        let input_facts = input_snapshot.facts(common_data.input_thresholds());
        last_input_facts[player_index] = input_facts;
        input_timers[player_index].x_tap = input_snapshot.x_tap_timer;
        input_timers[player_index].y_tap = input_snapshot.y_tap_timer;
        input_timers[player_index].trigger = input_snapshot.trigger_timer;
        let x_tap_timer = input_timers[player_index].x_tap;
        let y_tap_timer = input_timers[player_index].y_tap;
        let trigger_timer = input_timers[player_index].trigger;
        tick_ecb_bottom_lock(player);
        let previous_position = player.position;
        let previous_ecb_bottom = ecb_bottom_world_position_for_motion_frame(
            player,
            previous_position,
            player.motion_frame,
            common_data,
        );
        let mut skip_position_update_this_tick = false;
        let mut skip_airborne_vertical_physics_this_tick = false;
        begin_ground_velocity_tick(player);

        match player.motion_state {
            MotionState::Entry => {
                advance_entry(player, common_data);
                skip_position_update_this_tick = true;
            }
            MotionState::EntryStart => {
                advance_entry_start(player, common_data);
                skip_position_update_this_tick = true;
            }
            MotionState::EntryEnd => {
                advance_entry_end(player, common_data);
                skip_position_update_this_tick = player.motion_state == MotionState::EntryEnd;
            }
            MotionState::Wait => {
                apply_wait_state_inputs(
                    player,
                    input_facts,
                    stick_x,
                    common_data,
                    &mut input_timers[player_index].x_tap,
                );
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
                    enter_action_state(player, action_state, stick_x);
                } else if input_facts.shield_held {
                    enter_guard(player);
                } else if input_facts.normal_jump_pressed {
                    enter_knee_bend_from_ground(player, input_facts.normal_jump_input, common_data);
                } else if walk_forward_dash != 0 {
                    enter_dash(player, walk_forward_dash, true);
                    input_timers[player_index].x_tap = EXPIRED_INPUT_TIMER;
                } else if walk_smash_turn != 0 {
                    enter_smash_turn(player, walk_smash_turn);
                    apply_ground_traction(player, common_data);
                } else if input_facts.crouch {
                    enter_squat(player);
                } else if input_facts.walk_direction == player.facing {
                    let next_walk_state = walk_motion_state(player, common_data);
                    if next_walk_state != player.motion_state {
                        player.motion_state = next_walk_state;
                        player.motion_anim_rate_milli = 1_000;
                        player.walk_anim_velocity_x = player.ground_velocity_x;
                    }
                    apply_walk_velocity(player, stick_x, common_data);
                } else {
                    enter_wait_from_walk(player);
                }
            }
            MotionState::Dash => {
                dash_anim_tick(player);
                if !player.grounded {
                    enter_fall(player);
                } else if player.motion_state != MotionState::Dash {
                    // Dash_Anim can fall back before IASA/Phys.
                } else if let Some(action_state) = dash_action_state(
                    input_facts,
                    player.motion_frame,
                    player.facing,
                    trigger_timer,
                    common_data,
                ) {
                    enter_dash_iasa_action_state(player, action_state, stick_x, common_data);
                } else {
                    let smash_turn_direction = input_facts.smash_turn_direction(player.facing);
                    if dash_allows_opposite_dashback(player, common_data)
                        && smash_turn_direction != 0
                    {
                        let ground_velocity = staged_ground_velocity_x(player);
                        enter_smash_turn(player, smash_turn_direction);
                        set_ground_velocity_x(
                            player,
                            dash_iasa_decayed_ground_velocity(ground_velocity, common_data),
                        );
                        apply_ground_traction(player, common_data);
                    } else if input_facts.shield_held {
                        enter_guard_from_run(player, common_data);
                    } else if input_facts.normal_jump_pressed {
                        enter_knee_bend_from_ground(
                            player,
                            input_facts.normal_jump_input,
                            common_data,
                        );
                    } else if player.motion_cmd_var0 != 0
                        && is_same_direction_run(stick_x, player.facing, common_data)
                    {
                        enter_run(player);
                        apply_run_velocity(player, stick_x, common_data);
                    } else {
                        apply_dash_physics(player, stick_x, common_data);
                    }
                }
            }
            MotionState::Run => {
                run_anim_tick(player);
                if !player.grounded {
                    enter_fall(player);
                } else if let Some(action_state) = run_action_state(input_facts) {
                    enter_action_state(player, action_state, stick_x);
                } else if input_facts.shield_held {
                    enter_guard_from_run(player, common_data);
                } else if input_facts.normal_jump_pressed {
                    enter_knee_bend_from_ground(player, input_facts.normal_jump_input, common_data);
                } else if player.run_no_interrupt_frames > 0 {
                    if stick_x == 0 {
                        apply_run_ground_traction(player, common_data);
                    } else {
                        apply_run_velocity(player, stick_x, common_data);
                    }
                } else if is_same_direction_run(stick_x, player.facing, common_data) {
                    apply_run_velocity(player, stick_x, common_data);
                } else if is_opposite_run_turn(stick_x, player.facing, common_data) {
                    enter_turn_run(player, stick_x, common_data, 0);
                } else {
                    enter_run_brake(player, common_data);
                }
            }
            MotionState::RunDirect => {
                if !player.grounded {
                    enter_fall(player);
                } else if let Some(action_state) = run_action_state(input_facts) {
                    enter_action_state(player, action_state, stick_x);
                } else if input_facts.shield_held {
                    enter_guard_from_run(player, common_data);
                } else if input_facts.normal_jump_pressed {
                    enter_knee_bend_from_ground(player, input_facts.normal_jump_input, common_data);
                } else if is_same_direction_run(stick_x, player.facing, common_data) {
                    enter_run_from_run_direct(player);
                    apply_run_velocity(player, stick_x, common_data);
                } else if run_direct_releases_to_wait(stick_x, player.facing, common_data) {
                    enter_wait_from_walk(player);
                    apply_ground_traction(player, common_data);
                } else {
                    player.motion_frame = player.motion_frame.saturating_add(1);
                    if stick_x == 0 {
                        apply_run_ground_traction(player, common_data);
                    } else {
                        apply_run_velocity(player, stick_x, common_data);
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
                    enter_knee_bend_from_ground(player, input_facts.normal_jump_input, common_data);
                } else if player.motion_cmd_var0 != 0
                    && is_opposite_run_turn(stick_x, player.facing, common_data)
                {
                    enter_turn_run(player, stick_x, common_data, player.motion_frame);
                } else if input_facts.crouch {
                    enter_squat(player);
                } else {
                    apply_run_ground_traction(player, common_data);
                }
            }
            MotionState::TurnRun => {
                turn_run_anim_tick(player, stick_x, common_data);
                if !player.grounded {
                    clear_turn_state(player);
                    enter_fall(player);
                } else if player.motion_state != MotionState::TurnRun {
                    // TurnRun_Anim can complete into Run or fall back before IASA/Phys.
                } else if input_facts.normal_jump_pressed {
                    enter_knee_bend_from_ground(player, input_facts.normal_jump_input, common_data);
                } else {
                    apply_turn_run_velocity(player, stick_x, common_data);
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

                    let turn_facts = turn_effective_input_facts(player, input_facts);
                    if let Some(action_state) = turn_action_state(turn_facts) {
                        if player.facing != player.turn_facing_after {
                            player.facing = player.turn_facing_after;
                        }
                        enter_action_state(player, action_state, stick_x);
                    } else if input_facts.shield_held {
                        enter_guard(player);
                    } else if input_facts.normal_jump_pressed {
                        enter_knee_bend_from_ground(
                            player,
                            input_facts.normal_jump_input,
                            common_data,
                        );
                    } else {
                        arm_turn_dash_after_if_fresh(player, stick_x, x_tap_timer, common_data);
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
                        apply_ground_traction(player, common_data);
                        if player.motion_frame >= player.profile.standing_turn_total_frames {
                            player.motion_state = MotionState::Wait;
                            player.motion_frame = 0;
                            clear_turn_state(player);
                        }
                    }
                }
            }
            MotionState::Squat => {
                if !player.grounded {
                    enter_fall(player);
                } else if let Some(action_state) = wait_action_state(input_facts) {
                    enter_action_state(player, action_state, stick_x);
                } else if input_facts.shield_held {
                    enter_guard(player);
                } else if input_facts.normal_jump_pressed {
                    enter_knee_bend_from_ground(player, input_facts.normal_jump_input, common_data);
                } else if arm_squat_platform_pass(player, stage, stick_y, y_tap_timer, common_data)
                {
                    advance_squat_frame(player);
                } else if player.platform_pass_pending
                    && advance_squat_platform_pass(player, stage, common_data, stick_x)
                {
                    input_timers[player_index].y_tap = EXPIRED_INPUT_TIMER;
                } else {
                    advance_squat_frame(player);
                }
            }
            MotionState::SquatWait => {
                if !player.grounded {
                    enter_fall(player);
                } else if let Some(action_state) = wait_action_state(input_facts) {
                    enter_action_state(player, action_state, stick_x);
                } else if input_facts.shield_held {
                    enter_guard(player);
                } else if input_facts.normal_jump_pressed {
                    enter_knee_bend_from_ground(player, input_facts.normal_jump_input, common_data);
                } else if input_facts.forward_dash_direction(player.facing) != 0 {
                    enter_dash(player, player.facing, true);
                    input_timers[player_index].x_tap = EXPIRED_INPUT_TIMER;
                } else if crouch_released(stick_y, common_data) {
                    enter_squat_rv(player);
                } else {
                    player.motion_frame = player.motion_frame.saturating_add(1);
                    clear_ground_horizontal_velocity(player);
                    player.velocity.y = 0;
                }
            }
            MotionState::SquatRv => {
                if !player.grounded {
                    enter_fall(player);
                } else if let Some(action_state) = wait_action_state(input_facts) {
                    enter_action_state(player, action_state, stick_x);
                } else if input_facts.shield_held {
                    enter_guard(player);
                } else if input_facts.normal_jump_pressed {
                    enter_knee_bend_from_ground(player, input_facts.normal_jump_input, common_data);
                } else if input_facts.walk_direction != 0 {
                    enter_walk(
                        player,
                        walk_motion_state(player, common_data),
                        stick_x,
                        common_data,
                    );
                } else {
                    player.motion_frame = player.motion_frame.saturating_add(1);
                    clear_ground_horizontal_velocity(player);
                    player.velocity.y = 0;
                    if player.motion_frame >= player.profile.action_frames.squat_rv_total_frames {
                        player.motion_state = MotionState::Wait;
                        player.motion_frame = 0;
                    }
                }
            }
            MotionState::SpecialN
            | MotionState::SpecialSStart
            | MotionState::SpecialS
            | MotionState::SpecialHi
            | MotionState::SpecialLw
            | MotionState::Catch
            | MotionState::CatchDash
            | MotionState::Attack1
            | MotionState::AttackDash
            | MotionState::AttackS3
            | MotionState::AttackHi3
            | MotionState::AttackLw3
            | MotionState::AttackS4
            | MotionState::AttackHi4
            | MotionState::AttackLw4
            | MotionState::EscapeN
            | MotionState::EscapeF
            | MotionState::EscapeB => {
                player.motion_frame = player.motion_frame.saturating_add(1);
                clear_ground_horizontal_velocity(player);
                if let Some(next_state) =
                    grounded_action_iasa_state(player, input_facts, common_data)
                {
                    enter_iasa_state(
                        player,
                        next_state,
                        input_facts.normal_jump_input,
                        stick_x,
                        stage,
                        common_data,
                    );
                } else if player.motion_frame >= grounded_action_total_frames(player) {
                    player.motion_state = MotionState::Wait;
                    player.motion_frame = 0;
                }
            }
            MotionState::SpecialAirN
            | MotionState::SpecialAirSStart
            | MotionState::SpecialAirS
            | MotionState::SpecialAirHi
            | MotionState::SpecialAirLw
            | MotionState::AttackAirN
            | MotionState::AttackAirF
            | MotionState::AttackAirB
            | MotionState::AttackAirHi
            | MotionState::AttackAirLw => {
                player.motion_frame = player.motion_frame.saturating_add(1);
            }
            MotionState::GuardOn => {
                if !player.grounded {
                    clear_guard_state(player);
                    enter_fall(player);
                } else if !input_facts.shield_held {
                    enter_guard_off(player);
                } else if let Some(action_state) = guard_on_action_state(
                    player,
                    input_facts,
                    GuardInputContext {
                        facing: player.facing,
                        stage,
                        y_tap_timer,
                        stick_y,
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
                            input_facts.jump_input,
                            stick_x,
                            stage,
                            common_data,
                        );
                    }
                } else {
                    player.motion_frame = player.motion_frame.saturating_add(1);
                    update_shield_turn(player, input_facts);
                    player.velocity.y = 0;
                    if player.motion_frame >= player.profile.action_frames.guard_on_total_frames {
                        enter_guard_steady(player);
                    }
                }
            }
            MotionState::Guard | MotionState::GuardReflect => {
                if !player.grounded {
                    clear_guard_state(player);
                    enter_fall(player);
                } else if !input_facts.shield_held {
                    enter_guard_off(player);
                } else if let Some(action_state) = guard_action_state(
                    player,
                    input_facts,
                    GuardInputContext {
                        facing: player.facing,
                        stage,
                        y_tap_timer,
                        stick_y,
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
                            input_facts.jump_input,
                            stick_x,
                            stage,
                            common_data,
                        );
                    }
                } else if input_facts.shield_held {
                    player.motion_frame = player.motion_frame.saturating_add(1);
                    update_shield_turn(player, input_facts);
                    player.velocity.y = 0;
                }
            }
            MotionState::GuardOff => {
                if !player.grounded {
                    enter_fall(player);
                } else {
                    player.motion_frame = player.motion_frame.saturating_add(1);
                    apply_ground_traction(player, common_data);
                    player.velocity.y = 0;
                    if let Some(action_state) = guard_off_action_state(input_facts) {
                        enter_iasa_state(
                            player,
                            action_state,
                            input_facts.jump_input,
                            stick_x,
                            stage,
                            common_data,
                        );
                    } else if player.motion_frame
                        >= player.profile.action_frames.guard_off_total_frames
                    {
                        player.motion_state = MotionState::Wait;
                        player.motion_frame = 0;
                    }
                }
            }
            MotionState::GuardSetOff => {
                if !player.grounded {
                    clear_guard_state(player);
                    enter_fall(player);
                } else {
                    player.motion_frame = player.motion_frame.saturating_add(1);
                    apply_ground_traction(player, common_data);
                    player.velocity.y = 0;
                }
            }
            MotionState::KneeBend => {
                player.motion_frame = player.motion_frame.saturating_add(1);
                if player.motion_frame >= player.profile.jumpsquat_frames {
                    apply_jump_takeoff_velocity(player, stick_x);
                    player.velocity.y = ground_jump_vertical_velocity(player);
                    player.grounded = false;
                    player.motion_state = ground_jump_motion_state(player, stick_x, common_data);
                    player.motion_frame = 0;
                    player.fast_falling = false;
                    player.jumps_remaining = player.profile.reusable_air_jumps();
                    lock_ground_to_air_ecb_bottom(player);
                    apply_airborne_iasa_actions(player, input_facts, stick_x, stick_y, common_data);
                    // ftCo_Jump_Phys_Inner returns before ordinary airborne
                    // gravity on the first Jump tick, but Slippi post-frame
                    // data shows the newly seeded velocity has translated.
                    skip_airborne_vertical_physics_this_tick = true;
                } else if let Some(action_state) =
                    knee_bend_action_state(input_facts, stick_y, common_data)
                {
                    enter_action_state(player, action_state, stick_x);
                } else {
                    if input_facts.short_hop_released_for(player.jump_input) {
                        player.short_hop = true;
                    }
                    apply_ground_traction(player, common_data);
                }
            }
            MotionState::JumpAerialF | MotionState::JumpAerialB => {
                player.motion_frame = player.motion_frame.saturating_add(1);
                if action_sample_frame_count_for_motion_state(player.motion_state)
                    .is_some_and(|frames| player.motion_frame >= frames)
                {
                    enter_fall_aerial(player);
                }
                apply_airborne_iasa_or_drift(player, input_facts, stick_x, stick_y, common_data);
            }
            MotionState::JumpF | MotionState::JumpB => {
                player.motion_frame = player.motion_frame.saturating_add(1);
                if action_sample_frame_count_for_motion_state(player.motion_state)
                    .is_some_and(|frames| player.motion_frame >= frames)
                {
                    enter_fall(player);
                }
                apply_airborne_iasa_or_drift(player, input_facts, stick_x, stick_y, common_data);
            }
            MotionState::Fall
            | MotionState::FallF
            | MotionState::FallB
            | MotionState::FallAerial
            | MotionState::FallAerialF
            | MotionState::FallAerialB => {
                player.motion_frame = player.motion_frame.saturating_add(1);
                apply_airborne_iasa_or_drift(player, input_facts, stick_x, stick_y, common_data);
            }
            MotionState::Pass => {
                player.motion_frame = player.motion_frame.saturating_add(1);
                if action_sample_frame_count_for_motion_state(player.motion_state)
                    .is_some_and(|frames| player.motion_frame >= frames)
                {
                    enter_fall(player);
                }
                apply_airborne_iasa_or_drift(player, input_facts, stick_x, stick_y, common_data);
            }
            MotionState::EscapeAir => {
                player.motion_frame = player.motion_frame.saturating_add(1);
                player.escape_air_iasa_timer = player.escape_air_iasa_timer.saturating_sub(1);
                if action_sample_frame_count_for_motion_state(player.motion_state)
                    .is_some_and(|frames| player.motion_frame >= frames)
                {
                    enter_fall_special(player);
                }
            }
            MotionState::FallSpecial | MotionState::FallSpecialF | MotionState::FallSpecialB => {
                if input_facts.normal_jump_pressed && player.jumps_remaining > 0 {
                    enter_air_jump(player, stick_x, common_data);
                } else {
                    player.motion_frame = player.motion_frame.saturating_add(1);
                    apply_air_drift(player, stick_x);
                }
            }
            MotionState::LandingFallSpecial => {
                if !has_floor_support(stage, player.position) {
                    enter_fall(player);
                    continue;
                }
                player.motion_frame = player.motion_frame.saturating_add(1);
                apply_ground_traction(player, common_data);
                player.velocity.y = 0;
                if player.motion_frame >= common_data.escapeair_landing_lag_ticks {
                    player.motion_state = MotionState::Wait;
                    player.motion_frame = 0;
                    apply_wait_state_inputs(
                        player,
                        input_facts,
                        stick_x,
                        common_data,
                        &mut input_timers[player_index].x_tap,
                    );
                }
            }
            MotionState::Landing
            | MotionState::LandingAirN
            | MotionState::LandingAirF
            | MotionState::LandingAirB
            | MotionState::LandingAirHi
            | MotionState::LandingAirLw => {
                if !has_floor_support(stage, player.position) {
                    enter_fall(player);
                    continue;
                }
                player.motion_frame = player.motion_frame.saturating_add(1);
                apply_ground_traction(player, common_data);
                player.velocity.y = 0;
                if player.motion_frame >= landing_lag_ticks(player.motion_state, player.profile)
                    && apply_landing_iasa(player, input_facts, stick_x, common_data)
                    && player.motion_state == MotionState::Dash
                {
                    input_timers[player_index].x_tap = EXPIRED_INPUT_TIMER;
                }
            }
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
        ) {
            player.motion_cmd_var0 = 0;
            player.motion_cmd_var1 = 0;
            player.run_brake_x0 = false;
            player.run_brake_frames_remaining = 0;
            player.turn_run_x14 = false;
            player.motion_anim_rate_milli = 1_000;
        }
        if !matches!(
            player.motion_state,
            MotionState::WalkSlow | MotionState::WalkMiddle | MotionState::WalkFast
        ) {
            player.walk_anim_velocity_x = 0;
            player.walk_accel_mul_milli = 1_000;
        }
        if player.motion_state != MotionState::Dash {
            player.dash_entry_velocity_delta = 0;
            player.dash_started_from_tap = false;
        }

        if skip_position_update_this_tick {
            clear_ground_accels(player);
            continue;
        }

        if player.grounded {
            player.position.x += ground_position_delta_x(player);
            commit_ground_velocity(player);
        } else if player.motion_state == MotionState::EscapeAir {
            apply_escape_air_decay(player, common_data);
            player.position.x += player.velocity.x;
            clear_ground_accels(player);
        } else {
            player.position.x += player.velocity.x;
            clear_ground_accels(player);
        }

        if player.grounded && !has_floor_support(stage, player.position) {
            enter_fall(player);
            continue;
        }

        if !player.grounded {
            if player.motion_state == MotionState::EscapeAir {
                player.position.y += player.velocity.y;

                if let Some(contact) = airborne_landing_contact(
                    stage,
                    player,
                    previous_ecb_bottom,
                    None,
                    false,
                    common_data,
                ) {
                    snap_player_to_floor_contact(player, contact.y);
                    enter_landing_fall_special(player);
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
                        player.velocity.y = -player.profile.fast_fall_speed_per_tick;
                    } else {
                        player.velocity.y = (player.velocity.y - player.profile.gravity_per_tick)
                            .max(-player.profile.fall_speed_per_tick);
                    }
                }
                player.position.y += player.velocity.y;

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
                    snap_player_to_floor_contact(player, contact.y);
                    if matches!(
                        landing_state,
                        MotionState::EscapeAir
                            | MotionState::FallSpecial
                            | MotionState::FallSpecialF
                            | MotionState::FallSpecialB
                    ) {
                        enter_landing_fall_special(player);
                    } else {
                        enter_landing_from_airborne(player, landing_state);
                    }
                }
            }
        }
    }

    world.set_input_timers(input_timers);
    world.set_last_input_facts(last_input_facts);
    world.set_previous_inputs(*inputs);
    world.set_frame(frame.next());
}

fn apply_wait_state_inputs(
    player: &mut PlayerState,
    input_facts: MeleeInputFacts,
    stick_x: i32,
    common_data: MeleeCommonData,
    x_tap_timer: &mut u8,
) {
    if let Some(action_state) = wait_action_state(input_facts) {
        enter_action_state(player, action_state, stick_x);
    } else if player.grounded && input_facts.shield_held {
        enter_guard(player);
    } else if player.grounded && input_facts.normal_jump_pressed {
        enter_knee_bend_from_ground(player, input_facts.normal_jump_input, common_data);
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
            apply_ground_traction(player, common_data);
        } else if input_facts.crouch {
            enter_squat(player);
        } else if standing_turn != 0 {
            enter_standing_turn(player, standing_turn);
            apply_ground_traction(player, common_data);
        } else if input_facts.walk_direction != 0 {
            enter_walk(
                player,
                walk_motion_state(player, common_data),
                stick_x,
                common_data,
            );
        } else {
            apply_ground_traction(player, common_data);
            player.motion_frame = 0;
        }
    }
}

fn begin_ground_velocity_tick(player: &mut PlayerState) {
    clear_ground_accels(player);
    if player.grounded {
        player.ground_velocity_x = player.velocity.x;
    }
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
    player.motion_state = MotionState::EntryStart;
    player.motion_frame = 0;
    player.entry_timer = common_data.entry_start_ticks.saturating_sub(1);
    player.entry_base_y = player.position.y;
    player.entry_platform_offset_y = player.profile.entry_platform_offset_y;
    player.position.y = entry_position_y(
        player.entry_base_y,
        player.entry_platform_offset_y,
        1,
        common_data.entry_start_ticks,
    );
    player.grounded = false;
    player.velocity = Vec2 { x: 0, y: 0 };
}

fn advance_entry_start(player: &mut PlayerState, common_data: MeleeCommonData) {
    clear_ground_accels(player);
    player.velocity = Vec2 { x: 0, y: 0 };
    player.grounded = false;
    if player.entry_timer > 0 {
        player.entry_timer -= 1;
    }
    if player.entry_timer == 0 {
        enter_entry_end(player, common_data);
    } else {
        let progress = common_data.entry_start_ticks - player.entry_timer;
        player.position.y = entry_position_y(
            player.entry_base_y,
            player.entry_platform_offset_y,
            progress,
            common_data.entry_start_ticks,
        );
    }
}

fn enter_entry_end(player: &mut PlayerState, common_data: MeleeCommonData) {
    player.motion_state = MotionState::EntryEnd;
    player.motion_frame = 0;
    player.entry_timer = common_data.entry_end_ticks;
    player.position.y = player.entry_base_y + player.entry_platform_offset_y;
    player.grounded = false;
    player.velocity = Vec2 { x: 0, y: 0 };
}

fn advance_entry_end(player: &mut PlayerState, common_data: MeleeCommonData) {
    clear_ground_accels(player);
    player.velocity = Vec2 { x: 0, y: 0 };
    player.grounded = false;
    if player.entry_timer > 0 {
        player.entry_timer -= 1;
    }
    if player.entry_timer == 0 {
        player.motion_state = MotionState::Fall;
        player.motion_frame = 0;
    } else {
        player.position.y = entry_position_y(
            player.entry_base_y,
            player.entry_platform_offset_y,
            player.entry_timer,
            common_data.entry_start_ticks,
        );
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
    player.ground_accel_x = 0;
    player.ground_accel_x2 = 0;
}

fn ground_position_delta_x(player: &PlayerState) -> i32 {
    player.ground_velocity_x + player.ground_accel_x
}

fn commit_ground_velocity(player: &mut PlayerState) {
    let committed = (player.ground_velocity_x + player.ground_accel_x + player.ground_accel_x2)
        .clamp(
            -player.profile.ground_max_horizontal_velocity_per_tick,
            player.profile.ground_max_horizontal_velocity_per_tick,
        );
    player.ground_velocity_x = committed;
    player.velocity.x = committed;
    clear_ground_accels(player);
    player.dash_entry_velocity_delta = 0;
    player.dash_x0 = 0;
}

fn clear_ground_horizontal_velocity(player: &mut PlayerState) {
    player.ground_velocity_x = 0;
    player.velocity.x = 0;
    clear_ground_accels(player);
    player.dash_entry_velocity_delta = 0;
}

fn set_ground_velocity_x(player: &mut PlayerState, velocity_x: i32) {
    let velocity_x = velocity_x.clamp(
        -player.profile.ground_max_horizontal_velocity_per_tick,
        player.profile.ground_max_horizontal_velocity_per_tick,
    );
    player.ground_velocity_x = velocity_x;
    player.velocity.x = velocity_x;
    clear_ground_accels(player);
    player.dash_entry_velocity_delta = 0;
}

fn stage_ground_velocity_x(player: &mut PlayerState, next_velocity_x: i32) {
    let next_velocity_x = next_velocity_x.clamp(
        -player.profile.ground_max_horizontal_velocity_per_tick,
        player.profile.ground_max_horizontal_velocity_per_tick,
    );
    player.ground_accel_x = next_velocity_x - player.ground_velocity_x;
    player.velocity.x = next_velocity_x;
}

fn stage_dash_entry_ground_accel2(player: &mut PlayerState, delta: i32) {
    player.ground_accel_x2 = delta;
    player.dash_entry_velocity_delta = delta;
    player.velocity.x = (player.ground_velocity_x + player.ground_accel_x + delta).clamp(
        -player.profile.ground_max_horizontal_velocity_per_tick,
        player.profile.ground_max_horizontal_velocity_per_tick,
    );
}

fn staged_ground_velocity_x(player: &PlayerState) -> i32 {
    (player.ground_velocity_x + player.ground_accel_x + player.ground_accel_x2).clamp(
        -player.profile.ground_max_horizontal_velocity_per_tick,
        player.profile.ground_max_horizontal_velocity_per_tick,
    )
}

fn enter_knee_bend(player: &mut PlayerState, jump_input: MeleeJumpInput) {
    clear_shield_turn(player);
    clear_turn_state(player);
    clear_platform_pass_pending(player);
    player.motion_state = MotionState::KneeBend;
    player.motion_frame = 0;
    player.jump_input = jump_input;
    player.short_hop = false;
    player.velocity.y = 0;
}

fn enter_knee_bend_from_ground(
    player: &mut PlayerState,
    jump_input: MeleeJumpInput,
    common_data: MeleeCommonData,
) {
    enter_knee_bend(player, jump_input);
    apply_ground_traction(player, common_data);
}

fn apply_jump_takeoff_velocity(player: &mut PlayerState, stick_x: i32) {
    let profile = player.profile;
    let carried_velocity =
        staged_ground_velocity_x(player) * profile.ground_to_air_jump_momentum_milli / 1_000;
    let stick_velocity =
        stick_scaled_velocity(stick_x, profile.jump_horizontal_initial_velocity_per_tick);
    player.velocity.x = (carried_velocity + stick_velocity).clamp(
        -profile.jump_horizontal_max_velocity_per_tick,
        profile.jump_horizontal_max_velocity_per_tick,
    );
    clear_ground_accels(player);
}

fn ground_jump_vertical_velocity(player: &PlayerState) -> i32 {
    if player.short_hop {
        player.profile.short_hop_jump_force_per_tick
    } else {
        player.profile.full_hop_jump_force_per_tick
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
    player.motion_state = motion_state;
    player.motion_frame = 0;
    player.walk_accel_mul_milli = 1_000;
    player.walk_anim_velocity_x = player.ground_velocity_x;
    player.motion_anim_rate_milli = 1_000;
    apply_walk_velocity(player, stick_x, common_data);
}

fn walk_motion_state(player: &PlayerState, common_data: MeleeCommonData) -> MotionState {
    let walk_velocity = player.ground_velocity_x.abs();
    let middle_velocity =
        player.profile.walk_speed_per_tick * common_data.walk_middle_velocity_ratio_milli / 1000
            * player.walk_accel_mul_milli
            / 1000;
    let fast_velocity =
        player.profile.walk_speed_per_tick * common_data.walk_fast_velocity_ratio_milli / 1000
            * player.walk_accel_mul_milli
            / 1000;

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
    player.motion_state = MotionState::Dash;
    player.motion_frame = 0;
    player.facing = direction;
    player.dash_started_from_tap = started_from_tap;
    let dash_entry_delta =
        source_dash_entry_velocity_delta(player.ground_velocity_x, direction, player.profile);
    player.dash_x0 = dash_entry_delta;
    stage_dash_entry_ground_accel2(player, dash_entry_delta);
}

fn source_dash_entry_velocity_delta(
    current_velocity: i32,
    direction: i8,
    profile: crate::FighterProfile,
) -> i32 {
    let initial_velocity = direction as i32 * profile.initial_dash_speed_per_tick;
    if current_velocity * (direction as i32) < 0 {
        initial_velocity
    } else {
        initial_velocity - current_velocity
    }
}

fn clear_motion_script_state(player: &mut PlayerState) {
    player.motion_cmd_var0 = 0;
    player.motion_cmd_var1 = 0;
    player.dash_x0 = 0;
    player.walk_anim_velocity_x = 0;
    player.walk_accel_mul_milli = 1_000;
    player.run_brake_x0 = false;
    player.run_brake_frames_remaining = 0;
    player.turn_run_x14 = false;
    player.motion_anim_rate_milli = 1_000;
}

fn advance_source_motion_frame(player: &mut PlayerState) {
    if player.motion_anim_rate_milli > 0 {
        player.motion_frame = player.motion_frame.saturating_add(1);
    }
}

fn walk_anim_tick(player: &mut PlayerState) {
    advance_source_motion_frame(player);
    update_walk_anim_rate(player);
}

fn update_walk_anim_rate(player: &mut PlayerState) {
    if player.ground_velocity_x * player.facing as i32 <= 0 {
        player.motion_anim_rate_milli = 0;
        return;
    }

    let denominator = match player.motion_state {
        MotionState::WalkSlow => player.profile.slow_walk_max_velocity_per_tick,
        MotionState::WalkMiddle => player.profile.mid_walk_animation_rate_per_tick,
        MotionState::WalkFast => player.profile.fast_walk_animation_rate_per_tick,
        _ => 0,
    };
    player.motion_anim_rate_milli = if denominator > 0 {
        player.ground_velocity_x.abs() * 1000 / denominator
    } else {
        0
    };
}

fn dash_anim_tick(player: &mut PlayerState) {
    apply_dash_script_events(player);
    advance_source_motion_frame(player);
    apply_dash_script_events(player);

    if player.motion_frame >= player.profile.action_frames.dash_total_frames {
        enter_wait_from_walk(player);
        clear_motion_script_state(player);
    }
}

fn apply_dash_script_events(player: &mut PlayerState) {
    let frames = player.profile.action_frames;
    if player.motion_frame == frames.dash_cmd_var0_clear_frame {
        player.motion_cmd_var0 = 0;
    }
    if player.motion_frame == frames.dash_cmd_var0_set_frame {
        player.motion_cmd_var0 = 1;
    }
}

fn run_anim_tick(player: &mut PlayerState) {
    advance_source_motion_frame(player);
    if player.run_no_interrupt_frames > 0 {
        player.run_no_interrupt_frames -= 1;
    }
}

fn run_brake_anim_tick(player: &mut PlayerState, common_data: MeleeCommonData) {
    apply_run_brake_script_events(player);
    advance_source_motion_frame(player);
    apply_run_brake_script_events(player);

    if player.motion_cmd_var1 != 0 {
        let gate = common_data.run_brake_animation_pause_velocity_milli;
        if !player.run_brake_x0 {
            if player.ground_velocity_x.abs() >= gate {
                player.motion_anim_rate_milli = 0;
                player.run_brake_x0 = true;
            }
        } else if player.ground_velocity_x.abs() <= gate {
            player.motion_anim_rate_milli = 1_000;
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

fn turn_run_anim_tick(player: &mut PlayerState, stick_x: i32, common_data: MeleeCommonData) {
    player.turn_just_turned = false;
    apply_turn_run_script_events(player);
    update_turn_run_command_pause(player);
    advance_source_motion_frame(player);
    apply_turn_run_script_events(player);
    update_turn_run_command_pause(player);

    if player.motion_state != MotionState::TurnRun {
        return;
    }

    if player.motion_frame >= player.profile.action_frames.turn_run_total_frames {
        if is_same_direction_run(stick_x, player.facing, common_data) {
            enter_run_from_turn_run(player, common_data);
        } else {
            player.motion_state = MotionState::Wait;
            player.motion_frame = 0;
            clear_turn_state(player);
            clear_motion_script_state(player);
        }
    }
}

fn apply_turn_run_script_events(player: &mut PlayerState) {
    if player.motion_frame == player.profile.action_frames.turn_run_cmd_var1_frame {
        player.motion_cmd_var1 = 1;
    }
}

fn update_turn_run_command_pause(player: &mut PlayerState) {
    if player.motion_cmd_var1 == 0 {
        return;
    }

    if !player.turn_run_x14 {
        player.motion_anim_rate_milli = 0;
        player.turn_run_x14 = true;
        return;
    }

    if player.ground_velocity_x * player.turn_run_accel_mul as i32 <= 0 {
        player.motion_anim_rate_milli = 1_000;
        player.motion_cmd_var1 = 0;
        if !player.turn_has_turned {
            player.facing = player.turn_facing_after;
            player.turn_has_turned = true;
            player.turn_just_turned = true;
        }
    }
}

fn enter_run(player: &mut PlayerState) {
    clear_shield_turn(player);
    clear_turn_state(player);
    clear_motion_script_state(player);
    clear_platform_pass_pending(player);
    player.motion_state = MotionState::Run;
    player.motion_frame = 0;
    player.run_no_interrupt_frames = 0;
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
    player.motion_state = MotionState::Run;
    player.run_no_interrupt_frames = 0;
}

fn enter_run_brake(player: &mut PlayerState, common_data: MeleeCommonData) {
    clear_shield_turn(player);
    clear_turn_state(player);
    clear_motion_script_state(player);
    clear_platform_pass_pending(player);
    player.motion_state = MotionState::RunBrake;
    player.motion_frame = 0;
    player.run_brake_frames_remaining = player
        .profile
        .max_run_brake_frames
        .unwrap_or(player.profile.action_frames.run_brake_total_frames);
    apply_run_ground_traction(player, common_data);
}

fn enter_turn_run(
    player: &mut PlayerState,
    stick_x: i32,
    common_data: MeleeCommonData,
    anim_start: u8,
) {
    clear_shield_turn(player);
    let accel_mul = player.facing;
    clear_turn_state(player);
    clear_motion_script_state(player);
    clear_platform_pass_pending(player);
    player.motion_state = MotionState::TurnRun;
    player.motion_frame = anim_start;
    player.turn_facing_after = -accel_mul;
    player.turn_run_accel_mul = accel_mul;
    player.turn_has_turned = false;
    player.turn_just_turned = false;
    apply_turn_run_velocity(player, stick_x, common_data);
}

fn enter_smash_turn(player: &mut PlayerState, direction: i8) {
    clear_shield_turn(player);
    clear_platform_pass_pending(player);
    let dash_after_direction = player.facing;
    player.motion_state = MotionState::Turn;
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
    player.motion_state = MotionState::Turn;
    player.motion_frame = 0;
    player.turn_facing_after = direction;
    player.turn_has_turned = false;
    player.turn_just_turned = false;
    player.turn_frames_to_turn = player
        .profile
        .standing_turn_direction_change_frames
        .saturating_sub(1);
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

fn apply_turn_run_velocity(player: &mut PlayerState, stick_x: i32, common_data: MeleeCommonData) {
    let (accel, target_velocity) = dash_run_accel_and_target(player.profile, stick_x);
    if accel != 0 && player.turn_run_accel_mul as i32 * accel < 0 {
        let next_velocity = apply_ground_accel_toward_target(
            player.ground_velocity_x,
            accel,
            target_velocity,
            run_ground_friction(player, common_data),
            player.profile.ground_max_horizontal_velocity_per_tick,
        );
        stage_ground_velocity_x(player, next_velocity);
    } else {
        apply_run_ground_traction(player, common_data);
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
}

fn clear_platform_pass_pending(player: &mut PlayerState) {
    player.platform_pass_pending = false;
    player.platform_pass_timer = 0;
}

fn advance_squat_frame(player: &mut PlayerState) {
    player.motion_frame = player.motion_frame.saturating_add(1);
    clear_ground_horizontal_velocity(player);
    player.velocity.y = 0;
    if player.motion_frame >= player.profile.action_frames.squat_total_frames {
        enter_squat_wait(player);
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
    player.motion_state = MotionState::Squat;
    player.motion_frame = 0;
    clear_ground_horizontal_velocity(player);
    player.velocity.y = 0;
}

fn enter_squat_wait(player: &mut PlayerState) {
    clear_shield_turn(player);
    clear_turn_state(player);
    clear_platform_pass_pending(player);
    player.motion_state = MotionState::SquatWait;
    player.motion_frame = 0;
    clear_ground_horizontal_velocity(player);
    player.velocity.y = 0;
}

fn enter_squat_rv(player: &mut PlayerState) {
    clear_shield_turn(player);
    clear_turn_state(player);
    clear_platform_pass_pending(player);
    player.motion_state = MotionState::SquatRv;
    player.motion_frame = 0;
    clear_ground_horizontal_velocity(player);
    player.velocity.y = 0;
}

fn crouch_released(stick_y: i8, common_data: MeleeCommonData) -> bool {
    stick_y > -common_data.crouch_release_y
}

fn enter_guard(player: &mut PlayerState) {
    enter_guard_on(player, 0);
}

fn enter_guard_from_run(player: &mut PlayerState, common_data: MeleeCommonData) {
    enter_guard_on(player, common_data.guard_on_catch_dash_window);
}

fn enter_guard_on(player: &mut PlayerState, catch_dash_window: u8) {
    clear_turn_state(player);
    clear_platform_pass_pending(player);
    player.motion_state = MotionState::GuardOn;
    player.motion_frame = 0;
    clear_ground_horizontal_velocity(player);
    player.velocity.y = 0;
    player.guard_catch_dash_window = catch_dash_window;
    clear_shield_turn(player);
}

fn enter_guard_steady(player: &mut PlayerState) {
    clear_turn_state(player);
    clear_platform_pass_pending(player);
    player.motion_state = MotionState::Guard;
    player.motion_frame = 0;
    clear_ground_horizontal_velocity(player);
    player.velocity.y = 0;
    player.guard_catch_dash_window = 0;
}

fn enter_guard_reflect(player: &mut PlayerState) {
    clear_turn_state(player);
    clear_platform_pass_pending(player);
    player.motion_state = MotionState::GuardReflect;
    player.motion_frame = 0;
    player.velocity.y = 0;
    player.guard_catch_dash_window = 0;
    clear_shield_turn(player);
}

fn clear_guard_state(player: &mut PlayerState) {
    clear_shield_turn(player);
    player.guard_catch_dash_window = 0;
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
    player.motion_state = MotionState::GuardOff;
    player.motion_frame = 0;
    player.velocity.y = 0;
}

fn enter_pass(player: &mut PlayerState, stage: StageProfile, common_data: MeleeCommonData) {
    clear_guard_state(player);
    clear_turn_state(player);
    clear_platform_pass_pending(player);
    player.ecb_bottom_offset_y = 0;
    player.ecb_bottom_lock_timer = 0;
    player.motion_state = MotionState::Pass;
    player.motion_frame = 0;
    player.grounded = false;
    player.fast_falling = false;
    player.velocity.y = common_data.pass_initial_y_velocity;
    clear_ground_accels(player);
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
    apply_air_drift(player, stick_x);
}

fn enter_action_state(player: &mut PlayerState, motion_state: MotionState, stick_x: i32) {
    clear_guard_state(player);
    clear_turn_state(player);
    clear_platform_pass_pending(player);
    player.motion_state = motion_state;
    player.motion_frame = 0;
    if matches!(
        motion_state,
        MotionState::SpecialSStart | MotionState::SpecialS
    ) && stick_x != 0
    {
        player.facing = stick_x.signum() as i8;
    }
    if player.grounded {
        clear_ground_horizontal_velocity(player);
    } else {
        player.velocity.x = 0;
        clear_ground_accels(player);
    }
    player.velocity.y = 0;
}

fn enter_dash_iasa_action_state(
    player: &mut PlayerState,
    motion_state: MotionState,
    stick_x: i32,
    common_data: MeleeCommonData,
) {
    let ground_velocity = staged_ground_velocity_x(player);
    if motion_state == MotionState::GuardReflect {
        enter_guard_reflect(player);
    } else {
        enter_action_state(player, motion_state, stick_x);
    }
    if dash_iasa_action_applies_x54_decay(motion_state) {
        set_ground_velocity_x(
            player,
            dash_iasa_decayed_ground_velocity(ground_velocity, common_data),
        );
    }
    if motion_state == MotionState::GuardReflect {
        apply_ground_traction(player, common_data);
    }
}

fn dash_iasa_action_applies_x54_decay(motion_state: MotionState) -> bool {
    matches!(
        motion_state,
        MotionState::SpecialSStart
            | MotionState::SpecialS
            | MotionState::AttackS4
            | MotionState::EscapeF
            | MotionState::GuardReflect
    )
}

fn grounded_action_total_frames(player: &PlayerState) -> u8 {
    match player.motion_state {
        MotionState::SpecialN => FALCON_SPECIAL_N_FRAMES,
        MotionState::SpecialSStart | MotionState::SpecialS => FALCON_SPECIAL_S_FRAMES,
        MotionState::SpecialHi => FALCON_SPECIAL_HI_FRAMES,
        MotionState::SpecialLw => FALCON_SPECIAL_LW_FRAMES,
        MotionState::EscapeN => player.profile.action_frames.escape_n_total_frames,
        MotionState::EscapeF => player.profile.action_frames.escape_f_total_frames,
        MotionState::EscapeB => player.profile.action_frames.escape_b_total_frames,
        MotionState::Catch => FALCON_CATCH_FRAMES,
        MotionState::CatchDash => FALCON_CATCH_DASH_FRAMES,
        MotionState::Attack1 => player.profile.action_frames.attack1_total_frames,
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
        MotionState::Attack1 => Some(player.profile.action_frames.attack1_iasa_frame),
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
    jump_input: MeleeJumpInput,
    stick_x: i32,
    stage: StageProfile,
    common_data: MeleeCommonData,
) {
    match motion_state {
        MotionState::Guard => {
            enter_guard(player);
        }
        MotionState::GuardOff => enter_guard_off(player),
        MotionState::Pass => {
            enter_pass_with_same_frame_horizontal_physics(player, stage, common_data, stick_x)
        }
        MotionState::KneeBend => enter_knee_bend(player, jump_input),
        MotionState::Dash => enter_dash(player, player.facing, true),
        MotionState::Squat => enter_squat(player),
        MotionState::Turn => enter_smash_turn(player, -player.facing),
        MotionState::WalkSlow | MotionState::WalkMiddle | MotionState::WalkFast => {
            enter_walk(player, motion_state, stick_x, common_data);
        }
        MotionState::EscapeN | MotionState::EscapeF | MotionState::EscapeB => {
            enter_ground_escape(player, motion_state);
        }
        _ => enter_action_state(player, motion_state, stick_x),
    }
}

fn enter_escape_air(
    player: &mut PlayerState,
    stick_x: i32,
    stick_y: i8,
    common_data: MeleeCommonData,
) {
    player.floor_skip_surface = None;
    if player.ecb_bottom_lock_timer == 0 {
        player.ecb_bottom_offset_y = SOURCE_JOBJ_ECB_BOTTOM_OFFSET_Y;
    }
    player.motion_state = MotionState::EscapeAir;
    player.motion_frame = 0;
    player.escape_air_iasa_timer = common_data.escapeair_iasa_timer_ticks;
    player.fast_falling = false;
    let (velocity_x, velocity_y) = escape_air_velocity(stick_x, stick_y as i32, common_data);
    player.velocity.x = velocity_x;
    player.velocity.y = velocity_y;
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
            | MotionState::JumpF
            | MotionState::JumpB
            | MotionState::JumpAerialF
            | MotionState::JumpAerialB
            | MotionState::FallSpecial
            | MotionState::FallSpecialF
            | MotionState::FallSpecialB
    ) && stick_y <= common_data.fallspecial_platform_landing_y
}

fn enter_fall_special(player: &mut PlayerState) {
    player.motion_state = MotionState::FallSpecial;
    player.motion_frame = 0;
    player.escape_air_iasa_timer = 0;
    player.fast_falling = false;
}

fn enter_fall(player: &mut PlayerState) {
    let was_grounded = player.grounded;
    player.motion_state = MotionState::Fall;
    player.motion_frame = 0;
    player.grounded = false;
    if was_grounded {
        lock_ground_to_air_ecb_bottom(player);
    }
    player.velocity.x = player.velocity.x.clamp(
        -player.profile.air_drift_max_velocity_per_tick,
        player.profile.air_drift_max_velocity_per_tick,
    );
    player.ground_velocity_x = 0;
    player.escape_air_iasa_timer = 0;
    clear_ground_accels(player);
}

fn enter_fall_aerial(player: &mut PlayerState) {
    player.motion_state = MotionState::FallAerial;
    player.motion_frame = 0;
    player.grounded = false;
    player.escape_air_iasa_timer = 0;
    player.fast_falling = false;
    player.floor_skip_surface = None;
}

fn enter_landing_fall_special(player: &mut PlayerState) {
    clear_shield_turn(player);
    clear_turn_state(player);
    player.motion_state = MotionState::LandingFallSpecial;
    player.motion_frame = 0;
    player.escape_air_iasa_timer = 0;
    player.grounded = true;
    player.fast_falling = false;
    player.velocity.y = 0;
    set_ground_velocity_x(player, player.velocity.x);
}

fn airborne_landing_contact(
    stage: StageProfile,
    player: &PlayerState,
    previous_bottom: Vec2,
    floor_skip_surface: Option<u8>,
    drop_through_soft_platforms: bool,
    common_data: MeleeCommonData,
) -> Option<crate::StageLandingContact> {
    landing_contact_for_bottom_with_floor_skip(
        stage,
        previous_bottom,
        ecb_bottom_world_position_for_motion_frame(
            player,
            player.position,
            player.motion_frame,
            common_data,
        ),
        floor_skip_surface,
        drop_through_soft_platforms,
    )
}

fn ecb_bottom_world_position_for_motion_frame(
    player: &PlayerState,
    root_position: Vec2,
    motion_frame: u8,
    common_data: MeleeCommonData,
) -> Vec2 {
    Vec2 {
        x: root_position.x,
        y: root_position.y + active_ecb_bottom_offset_y(player, motion_frame, common_data),
    }
}

fn snap_player_to_floor_contact(player: &mut PlayerState, y: i32) {
    player.position.y = if player.ecb_bottom_offset_y > 0 {
        y
    } else {
        y - player.ecb_bottom_offset_y
    };
    player.ecb_bottom_lock_timer = 0;
    player.velocity.y = 0;
    player.grounded = true;
    player.fast_falling = false;
    set_ground_velocity_x(player, player.velocity.x);
    player.jumps_remaining = player.profile.reusable_air_jumps();
    player.jump_input = Default::default();
    player.short_hop = false;
    player.floor_skip_surface = None;
}

fn lock_ground_to_air_ecb_bottom(player: &mut PlayerState) {
    player.ecb_bottom_offset_y = 0;
    player.ecb_bottom_lock_timer = GROUND_TO_AIR_ECB_LOCK_FRAMES;
}

fn tick_ecb_bottom_lock(player: &mut PlayerState) {
    if player.ecb_bottom_lock_timer == 0 {
        return;
    }
    player.ecb_bottom_lock_timer -= 1;
    if player.ecb_bottom_lock_timer == 0 && !player.grounded {
        player.ecb_bottom_offset_y = SOURCE_JOBJ_ECB_BOTTOM_OFFSET_Y;
    }
}

fn enter_landing_from_airborne(player: &mut PlayerState, airborne_motion_state: MotionState) {
    let landing_motion_state = match airborne_motion_state {
        MotionState::AttackAirN => MotionState::LandingAirN,
        MotionState::AttackAirF => MotionState::LandingAirF,
        MotionState::AttackAirB => MotionState::LandingAirB,
        MotionState::AttackAirHi => MotionState::LandingAirHi,
        MotionState::AttackAirLw => MotionState::LandingAirLw,
        _ => MotionState::Landing,
    };
    enter_landing_as(player, landing_motion_state);
}

fn enter_landing_as(player: &mut PlayerState, motion_state: MotionState) {
    clear_shield_turn(player);
    clear_turn_state(player);
    player.motion_state = motion_state;
    player.motion_frame = 0;
    player.grounded = true;
    player.fast_falling = false;
    player.velocity.y = 0;
    set_ground_velocity_x(player, player.velocity.x);
}

fn enter_air_special(player: &mut PlayerState, motion_state: MotionState, stick_x: i32) {
    player.floor_skip_surface = None;
    player.motion_state = motion_state;
    player.motion_frame = 0;
    player.fast_falling = false;
    if matches!(
        motion_state,
        MotionState::SpecialAirSStart | MotionState::SpecialAirS
    ) && stick_x != 0
    {
        player.facing = stick_x.signum() as i8;
    }
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
    player.floor_skip_surface = None;
    player.ecb_bottom_offset_y = SOURCE_JOBJ_ECB_BOTTOM_OFFSET_Y;
    player.motion_state =
        if stick_x * player.facing as i32 > -(common_data.air_jump_backward_x as i32) {
            MotionState::JumpAerialF
        } else {
            MotionState::JumpAerialB
        };
    player.motion_frame = 0;
    player.velocity.x = stick_scaled_velocity(
        stick_x,
        player.profile.air_jump_horizontal_velocity_per_tick,
    );
    player.velocity.y = player.profile.air_jump_force_per_tick;
    player.fast_falling = false;
    player.jumps_remaining -= 1;
    apply_air_drift(player, stick_x);
}

fn enter_air_attack(player: &mut PlayerState, motion_state: MotionState) {
    player.floor_skip_surface = None;
    player.motion_state = motion_state;
    player.motion_frame = 0;
}

fn apply_airborne_iasa_or_drift(
    player: &mut PlayerState,
    input_facts: MeleeInputFacts,
    stick_x: i32,
    stick_y: i8,
    common_data: MeleeCommonData,
) {
    if !apply_airborne_iasa_actions(player, input_facts, stick_x, stick_y, common_data) {
        apply_air_drift(player, stick_x);
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
        true
    } else if input_facts.normal_jump_pressed && player.jumps_remaining > 0 {
        enter_air_jump(player, stick_x, common_data);
        true
    } else {
        false
    }
}

fn enter_ground_escape(player: &mut PlayerState, motion_state: MotionState) {
    player.motion_state = motion_state;
    player.motion_frame = 0;
    clear_ground_horizontal_velocity(player);
    player.velocity.y = 0;
}

fn apply_walk_velocity(player: &mut PlayerState, stick_x: i32, common_data: MeleeCommonData) {
    if stick_x > 0 {
        player.facing = 1;
    } else if stick_x < 0 {
        player.facing = -1;
    }

    let profile = player.profile;
    let accel_mul = player.walk_accel_mul_milli;
    let target_velocity =
        stick_scaled_velocity(stick_x, profile.walk_speed_per_tick) * accel_mul / 1000;
    let mut accel =
        stick_scaled_velocity(stick_x, profile.walk_initial_velocity_per_tick) * accel_mul / 1000;
    if stick_x > 0 {
        accel += profile.walk_accel_per_tick * accel_mul / 1000;
    } else if stick_x < 0 {
        accel -= profile.walk_accel_per_tick * accel_mul / 1000;
    }
    accel = apply_remaining_velocity_taper(
        player.ground_velocity_x,
        accel,
        target_velocity,
        common_data.walk_accel_taper_milli,
    );

    let next_velocity = apply_ground_accel_toward_target(
        player.ground_velocity_x,
        accel,
        target_velocity,
        profile.traction_per_tick,
        profile.ground_max_horizontal_velocity_per_tick,
    );
    player.walk_anim_velocity_x =
        target_velocity * common_data.animation_velocity_scale_milli / 1000;
    stage_ground_velocity_x(player, next_velocity);
}

fn apply_air_drift(player: &mut PlayerState, stick_x: i32) {
    let profile = player.profile;
    let target_velocity = stick_scaled_velocity(stick_x, profile.air_drift_max_velocity_per_tick);
    if target_velocity == 0 {
        player.velocity.x =
            apply_friction_to_zero(player.velocity.x, profile.air_friction_per_tick);
        return;
    }

    let stick_accel = stick_scaled_velocity(stick_x, profile.air_drift_stick_accel_per_tick);
    let base_accel = stick_x.signum() * profile.air_drift_base_accel_per_tick;
    let accel = air_accel_for_velocity(
        player.velocity.x,
        stick_accel + base_accel,
        target_velocity,
        profile.air_friction_per_tick,
        profile.air_max_horizontal_velocity_per_tick,
    );
    player.velocity.x += accel;
}

fn apply_dash_velocity(player: &mut PlayerState, stick_x: i32, common_data: MeleeCommonData) {
    let (accel, target_velocity) = dash_run_accel_and_target(player.profile, stick_x);
    let next_velocity = apply_ground_accel_toward_target(
        player.ground_velocity_x,
        accel,
        target_velocity,
        run_ground_friction(player, common_data),
        player.profile.ground_max_horizontal_velocity_per_tick,
    );
    stage_ground_velocity_x(player, next_velocity);
}

fn apply_dash_physics(player: &mut PlayerState, stick_x: i32, common_data: MeleeCommonData) {
    if player.dash_x0 != 0 {
        player.dash_x0 = 0;
    } else if stick_x == 0 {
        apply_run_ground_traction(player, common_data);
    } else {
        apply_dash_velocity(player, stick_x, common_data);
    }
}

fn apply_run_velocity(player: &mut PlayerState, stick_x: i32, common_data: MeleeCommonData) {
    let (accel, target_velocity) = dash_run_accel_and_target(player.profile, stick_x);
    let accel = apply_remaining_velocity_taper(
        player.ground_velocity_x,
        accel,
        target_velocity,
        common_data.run_accel_taper_milli,
    );
    let next_velocity = apply_ground_accel_toward_target(
        player.ground_velocity_x,
        accel,
        target_velocity,
        run_ground_friction(player, common_data),
        player.profile.ground_max_horizontal_velocity_per_tick,
    );
    stage_ground_velocity_x(player, next_velocity);
}

fn enter_wait_from_walk(player: &mut PlayerState) {
    player.motion_state = MotionState::Wait;
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

fn apply_ground_traction(player: &mut PlayerState, common_data: MeleeCommonData) {
    let mut traction = player.profile.traction_per_tick;
    if player.ground_velocity_x.abs() > player.profile.walk_speed_per_tick {
        traction = traction * common_data.high_speed_ground_friction_multiplier_milli / 1000;
    }
    let next_velocity = apply_friction_to_zero(player.ground_velocity_x, traction);
    stage_ground_velocity_x(player, next_velocity);
}

fn apply_run_ground_traction(player: &mut PlayerState, common_data: MeleeCommonData) {
    let next_velocity = apply_friction_to_zero(
        player.ground_velocity_x,
        run_ground_friction(player, common_data),
    );
    stage_ground_velocity_x(player, next_velocity);
}

fn run_ground_friction(player: &PlayerState, common_data: MeleeCommonData) -> i32 {
    player.profile.traction_per_tick * common_data.run_ground_friction_multiplier_milli / 1000
}

fn dash_iasa_decayed_ground_velocity(velocity_x: i32, common_data: MeleeCommonData) -> i32 {
    const DEFAULT_FLOOR_FRICTION_MULTIPLIER_MILLI: i64 = 1_000;
    let decay = velocity_x as i64
        * common_data.dash_velocity_decay_milli as i64
        * DEFAULT_FLOOR_FRICTION_MULTIPLIER_MILLI
        / 1_000_000;
    (velocity_x as i64 - decay) as i32
}

fn stick_scaled_velocity(stick_x: i32, full_stick_velocity: i32) -> i32 {
    let axis = fighter_stick_axis_to_f32(stick_x.clamp(-127, 127) as i8);
    round_source_units_to_milli(axis * full_stick_velocity as f32 / 1000.0)
}

fn dash_run_accel_and_target(profile: crate::FighterProfile, stick_x: i32) -> (i32, i32) {
    let stick = fighter_stick_axis_to_f32(stick_x.clamp(-127, 127) as i8);
    let base_accel = if stick > 0.0 {
        profile.dash_run_acceleration_b
    } else {
        -profile.dash_run_acceleration_b
    };
    let accel = stick * profile.dash_run_acceleration_a + base_accel;
    let target_velocity = stick * profile.dash_run_terminal_velocity;

    (
        round_source_units_to_milli(accel),
        round_source_units_to_milli(target_velocity),
    )
}

fn round_source_units_to_milli(value: f32) -> i32 {
    (value * 1000.0).round() as i32
}

fn apply_remaining_velocity_taper(
    current_velocity: i32,
    accel: i32,
    target_velocity: i32,
    taper_milli: i32,
) -> i32 {
    if target_velocity == 0 || taper_milli == 1_000 {
        return accel;
    }

    let same_direction_ratio = current_velocity.signum() == target_velocity.signum()
        && current_velocity.abs() < target_velocity.abs();
    if !same_direction_ratio {
        return accel;
    }

    let remaining = (target_velocity - current_velocity).abs() as i64;
    let denominator = target_velocity.abs() as i64 * 1_000;
    let scaled_abs = accel.abs() as i64 * remaining * taper_milli as i64 / denominator;
    scaled_abs as i32 * accel.signum()
}

fn air_accel_for_velocity(
    current_velocity: i32,
    mut accel: i32,
    target_velocity: i32,
    friction: i32,
    max_horizontal_velocity: i32,
) -> i32 {
    if current_velocity * accel < 0 {
        return accel;
    }

    if accel > 0 && current_velocity + accel > target_velocity {
        accel = -friction;
        if current_velocity + accel < target_velocity {
            accel = target_velocity - current_velocity;
        }
        if current_velocity + accel > max_horizontal_velocity {
            accel = max_horizontal_velocity - current_velocity;
        }
    } else if accel < 0 && current_velocity + accel < target_velocity {
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
    player.velocity.x = player.velocity.x * common_data.escapeair_decay_milli / 1000;
    player.velocity.y = player.velocity.y * common_data.escapeair_decay_milli / 1000;
}

fn escape_air_velocity(stick_x: i32, stick_y: i32, common_data: MeleeCommonData) -> (i32, i32) {
    if stick_x.abs() < common_data.escapeair_deadzone_x as i32
        && stick_y.abs() < common_data.escapeair_deadzone_y as i32
    {
        return (0, 0);
    }

    if stick_y == 0 {
        return (stick_x.signum() * common_data.escapeair_force, 0);
    }
    if stick_x == 0 {
        return (0, stick_y.signum() * common_data.escapeair_force);
    }

    let magnitude = scaled_vector_magnitude(stick_x, stick_y);
    (
        fixed_force_component(stick_x, magnitude, common_data.escapeair_force),
        fixed_force_component(stick_y, magnitude, common_data.escapeair_force),
    )
}

fn fixed_force_component(axis: i32, scaled_magnitude: i64, force: i32) -> i32 {
    const SCALE: i64 = 1024;
    let component =
        (axis.abs() as i64 * force as i64 * SCALE + scaled_magnitude / 2) / scaled_magnitude;
    component as i32 * axis.signum()
}

fn scaled_vector_magnitude(x: i32, y: i32) -> i64 {
    const SCALE: i64 = 1024;
    let x = x as i64;
    let y = y as i64;
    integer_sqrt((x * x + y * y) * SCALE * SCALE)
}

fn integer_sqrt(value: i64) -> i64 {
    if value <= 0 {
        return 0;
    }

    let mut low = 1;
    let mut high = value.min(1 << 32);
    while low <= high {
        let mid = low + (high - low) / 2;
        let square = mid * mid;
        if square == value {
            return mid;
        }
        if square < value {
            low = mid + 1;
        } else {
            high = mid - 1;
        }
    }
    high
}

fn apply_ground_accel_toward_target(
    current_velocity: i32,
    mut accel: i32,
    target_velocity: i32,
    friction_per_tick: i32,
    max_velocity: i32,
) -> i32 {
    let friction_per_tick = friction_per_tick.abs();
    if target_velocity == 0 {
        return apply_friction_to_zero(current_velocity, friction_per_tick);
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

fn apply_friction_to_zero(current_velocity: i32, friction: i32) -> i32 {
    if current_velocity > friction {
        current_velocity - friction
    } else if current_velocity < -friction {
        current_velocity + friction
    } else {
        0
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
    if motion_frame < common_data.dash_defensive_action_window && facts.shield_held {
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
    if let Some(spot_dodge_state) = guard_spot_dodge_state(facts) {
        return Some(spot_dodge_state);
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

    if facts.attack_pressed || facts.grab_pressed {
        return Some(MotionState::Catch);
    }

    if facts.jump_pressed {
        return Some(MotionState::KneeBend);
    }

    guard_platform_pass_state(player, facts, context)
}

fn guard_action_state(
    player: &PlayerState,
    facts: MeleeInputFacts,
    context: GuardInputContext,
) -> Option<MotionState> {
    if let Some(spot_dodge_state) = guard_spot_dodge_state(facts) {
        return Some(spot_dodge_state);
    }

    if facts.roll_direction != 0 {
        return if facts.roll_direction == context.facing {
            Some(MotionState::EscapeF)
        } else {
            Some(MotionState::EscapeB)
        };
    }

    if facts.attack_pressed || facts.grab_pressed {
        return Some(MotionState::Catch);
    }

    if facts.jump_pressed {
        return Some(MotionState::KneeBend);
    }

    guard_platform_pass_state(player, facts, context)
}

fn guard_spot_dodge_state(facts: MeleeInputFacts) -> Option<MotionState> {
    if facts.cstick_spot_dodge {
        return Some(MotionState::EscapeN);
    }

    if facts.main_stick_spot_dodge {
        return Some(MotionState::EscapeN);
    }

    None
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
    common_data: MeleeCommonData,
) -> bool {
    if facts.special_pressed && matches!(facts.special_direction, (1 | -1, 0)) {
        enter_action_state(player, MotionState::SpecialSStart, stick_x);
        return true;
    }

    if facts.grab_pressed || (facts.shield_held && facts.attack_pressed) {
        enter_action_state(player, MotionState::Catch, stick_x);
        return true;
    }

    if let Some(attack_state) = grounded_attack_state_from_source_order(facts) {
        enter_action_state(player, attack_state, stick_x);
        return true;
    }

    if facts.normal_jump_pressed {
        enter_knee_bend(player, facts.normal_jump_input);
        return true;
    }

    let forward_dash = facts.forward_dash_direction(player.facing);
    if forward_dash != 0 {
        enter_dash(player, forward_dash, true);
        return true;
    }

    if player.motion_frame
        < landing_lag_ticks(player.motion_state, player.profile).saturating_add(1)
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

fn landing_lag_ticks(motion_state: MotionState, profile: crate::FighterProfile) -> u8 {
    match motion_state {
        MotionState::LandingAirN => profile.landing_air_n_lag_ticks,
        MotionState::LandingAirF => profile.landing_air_f_lag_ticks,
        MotionState::LandingAirB => profile.landing_air_b_lag_ticks,
        MotionState::LandingAirHi => profile.landing_air_hi_lag_ticks,
        MotionState::LandingAirLw => profile.landing_air_lw_lag_ticks,
        _ => profile.normal_landing_lag_ticks,
    }
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
