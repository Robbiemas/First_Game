use crate::input::NO_GROUNDED_SPECIAL_DIRECTION;
use crate::{
    collision::{
        floor_surface_for_bottom, floor_surface_index_for_bottom, has_floor_support,
        landing_contact_for_bottom, landing_contact_for_bottom_with_floor_skip,
    },
    state::EXPIRED_INPUT_TIMER,
    Frame, MeleeCommonData, MeleeInputFacts, MeleeJumpInput, MotionState, PlayerInput, PlayerState,
    StageProfile, StageSurfaceKind, WalkSpeedBucket, World,
};

const ATTACK_ACTIVE_TICKS: u8 = 12;
const SHIELD_TURN_FRAMES: u8 = 5;
const TURN_LATCH_ATTACK: u8 = 0x01;
const TURN_LATCH_SPECIAL: u8 = 0x02;
// UCF suppresses main-stick spotdodge for AXE-style rim input above -0.8000.
const UCF_AXE_SPOT_DODGE_SUPPRESSION_Y: i8 = 102;
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
        let previous_position = player.position;
        let mut skip_position_update_this_tick = false;

        match player.motion_state {
            MotionState::Wait => {
                if let Some(action_state) = wait_action_state(input_facts) {
                    enter_action_state(player, action_state, stick_x);
                } else if player.grounded && input_facts.shield_held {
                    enter_guard(player);
                } else if player.grounded && input_facts.normal_jump_pressed {
                    enter_knee_bend(player, input_facts.normal_jump_input);
                } else if !player.grounded {
                    player.motion_state = MotionState::Air;
                    player.motion_frame = 0;
                } else {
                    let forward_dash = input_facts.forward_dash_direction(player.facing);
                    let smash_turn = input_facts.smash_turn_direction(player.facing);
                    let standing_turn = input_facts.standing_turn_direction(player.facing);
                    if forward_dash != 0 {
                        enter_dash(player, forward_dash);
                        input_timers[player_index].x_tap = EXPIRED_INPUT_TIMER;
                    } else if smash_turn != 0 {
                        enter_smash_turn(player, smash_turn);
                    } else if input_facts.crouch {
                        enter_squat(player);
                    } else if standing_turn != 0 {
                        enter_standing_turn(player, standing_turn);
                    } else if input_facts.walk_direction != 0 {
                        enter_walk(
                            player,
                            walk_motion_state(input_facts.walk_speed_bucket),
                            stick_x,
                        );
                    } else {
                        player.velocity.x = 0;
                        player.motion_frame = 0;
                    }
                }
            }
            MotionState::WalkSlow | MotionState::WalkMiddle | MotionState::WalkFast => {
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
                    player.motion_state = MotionState::Air;
                    player.motion_frame = 0;
                } else if let Some(action_state) = walk_action_state(input_facts) {
                    enter_action_state(player, action_state, stick_x);
                } else if input_facts.shield_held {
                    enter_guard(player);
                } else if input_facts.normal_jump_pressed {
                    enter_knee_bend(player, input_facts.normal_jump_input);
                } else if walk_forward_dash != 0 {
                    enter_dash(player, walk_forward_dash);
                    input_timers[player_index].x_tap = EXPIRED_INPUT_TIMER;
                } else if walk_smash_turn != 0 {
                    enter_smash_turn(player, walk_smash_turn);
                } else if input_facts.crouch {
                    enter_squat(player);
                } else if input_facts.walk_direction == player.facing {
                    player.motion_frame = player.motion_frame.saturating_add(1);
                    player.motion_state = walk_motion_state(input_facts.walk_speed_bucket);
                    apply_walk_velocity(player, stick_x);
                } else {
                    enter_wait_from_walk(player);
                }
            }
            MotionState::Dash => {
                if !player.grounded {
                    player.motion_state = MotionState::Air;
                    player.motion_frame = 0;
                } else if let Some(action_state) =
                    dash_action_state(input_facts, player.motion_frame, player.facing, common_data)
                {
                    enter_action_state(player, action_state, stick_x);
                } else if input_facts.smash_turn_direction(player.facing) != 0 {
                    enter_smash_turn(player, input_facts.smash_turn_direction(player.facing));
                } else if input_facts.shield_held {
                    enter_guard_from_run(player, common_data);
                } else if input_facts.normal_jump_pressed {
                    enter_knee_bend(player, input_facts.normal_jump_input);
                } else {
                    player.motion_frame = player.motion_frame.saturating_add(1);
                    if stick_x == 0 {
                        apply_ground_traction(player);
                    } else {
                        apply_dash_velocity(player, stick_x);
                    }
                    if player.motion_frame >= player.profile.dash_frames {
                        exit_dash(player, stick_x, input_facts.walk_speed_bucket, common_data);
                    }
                }
            }
            MotionState::Run => {
                if !player.grounded {
                    player.motion_state = MotionState::Air;
                    player.motion_frame = 0;
                } else if let Some(action_state) = run_action_state(input_facts) {
                    enter_action_state(player, action_state, stick_x);
                } else if input_facts.shield_held {
                    enter_guard_from_run(player, common_data);
                } else if input_facts.normal_jump_pressed {
                    enter_knee_bend(player, input_facts.normal_jump_input);
                } else if player.run_no_interrupt_frames > 0 {
                    player.run_no_interrupt_frames -= 1;
                    player.motion_frame = player.motion_frame.saturating_add(1);
                    if stick_x == 0 {
                        apply_ground_traction(player);
                    } else {
                        apply_dash_velocity(player, stick_x);
                    }
                } else if is_same_direction_run(stick_x, player.facing, common_data) {
                    player.motion_frame = player.motion_frame.saturating_add(1);
                    apply_dash_velocity(player, stick_x);
                } else if is_opposite_run_turn(stick_x, player.facing, common_data) {
                    enter_turn_run(player);
                } else {
                    enter_run_brake(player);
                }
            }
            MotionState::RunBrake => {
                if !player.grounded {
                    player.motion_state = MotionState::Air;
                    player.motion_frame = 0;
                } else if input_facts.normal_jump_pressed {
                    enter_knee_bend(player, input_facts.normal_jump_input);
                } else if input_facts.crouch {
                    enter_squat(player);
                } else {
                    player.motion_frame = player.motion_frame.saturating_add(1);
                    apply_ground_traction(player);
                    let profile_brake_expired = player
                        .profile
                        .max_run_brake_frames
                        .is_some_and(|frames| player.motion_frame >= frames);
                    if player.velocity.x == 0 || profile_brake_expired {
                        player.motion_state = MotionState::Wait;
                        player.motion_frame = 0;
                    }
                }
            }
            MotionState::TurnRun => {
                if !player.grounded {
                    clear_turn_state(player);
                    player.motion_state = MotionState::Air;
                    player.motion_frame = 0;
                } else if input_facts.normal_jump_pressed {
                    enter_knee_bend(player, input_facts.normal_jump_input);
                } else {
                    player.motion_frame = player.motion_frame.saturating_add(1);
                    apply_turn_run_velocity(player, stick_x);
                    advance_turn_run_facing(player);

                    if player.turn_has_turned
                        && player.velocity.x * player.facing as i32 > 0
                        && is_same_direction_run(stick_x, player.facing, common_data)
                    {
                        enter_run_from_turn_run(player, common_data);
                    } else if player.velocity.x == 0 && stick_x == 0 {
                        player.motion_state = MotionState::Wait;
                        player.motion_frame = 0;
                        clear_turn_state(player);
                    }
                }
            }
            MotionState::Turn => {
                if !player.grounded {
                    clear_turn_state(player);
                    player.motion_state = MotionState::Air;
                    player.motion_frame = 0;
                } else {
                    advance_turn_anim(player);
                    player.velocity.x = 0;

                    let turn_facts = turn_effective_input_facts(player, input_facts);
                    if let Some(action_state) = turn_action_state(turn_facts) {
                        if player.facing != player.turn_facing_after {
                            player.facing = player.turn_facing_after;
                        }
                        enter_action_state(player, action_state, stick_x);
                    } else if input_facts.shield_held {
                        enter_guard(player);
                    } else if input_facts.normal_jump_pressed {
                        enter_knee_bend(player, input_facts.normal_jump_input);
                    } else {
                        arm_turn_dash_after_if_fresh(
                            player,
                            stick_x,
                            x_tap_timer,
                            input_facts,
                            common_data,
                        );
                        if player.turn_just_turned
                            && player.turn_dash_after_direction != 0
                            && stick_x * player.turn_facing_after as i32
                                >= common_data.dash_x as i32
                        {
                            enter_dash(player, player.turn_facing_after);
                            input_timers[player_index].x_tap = EXPIRED_INPUT_TIMER;
                        } else {
                            latch_turn_buttons(player, input_facts);
                            if player.turn_just_turned {
                                player.turn_just_turned = false;
                            }
                        }
                    }

                    if player.motion_state == MotionState::Turn
                        && player.motion_frame >= player.profile.standing_turn_total_frames
                    {
                        player.motion_state = MotionState::Wait;
                        player.motion_frame = 0;
                        clear_turn_state(player);
                    }
                }
            }
            MotionState::Squat => {
                if !player.grounded {
                    player.motion_state = MotionState::Air;
                    player.motion_frame = 0;
                } else if let Some(action_state) = wait_action_state(input_facts) {
                    enter_action_state(player, action_state, stick_x);
                } else if input_facts.shield_held {
                    enter_guard(player);
                } else if input_facts.normal_jump_pressed {
                    enter_knee_bend(player, input_facts.normal_jump_input);
                } else {
                    player.motion_frame = player.motion_frame.saturating_add(1);
                    player.velocity.x = 0;
                    player.velocity.y = 0;
                    if player.motion_frame >= player.profile.action_frames.squat_total_frames {
                        enter_squat_wait(player);
                    }
                }
            }
            MotionState::SquatWait => {
                if !player.grounded {
                    player.motion_state = MotionState::Air;
                    player.motion_frame = 0;
                } else if let Some(action_state) = wait_action_state(input_facts) {
                    enter_action_state(player, action_state, stick_x);
                } else if input_facts.shield_held {
                    enter_guard(player);
                } else if input_facts.normal_jump_pressed {
                    enter_knee_bend(player, input_facts.normal_jump_input);
                } else if input_facts.forward_dash_direction(player.facing) != 0 {
                    enter_dash(player, player.facing);
                    input_timers[player_index].x_tap = EXPIRED_INPUT_TIMER;
                } else if crouch_released(stick_y, common_data) {
                    enter_squat_rv(player);
                } else {
                    player.motion_frame = player.motion_frame.saturating_add(1);
                    player.velocity.x = 0;
                    player.velocity.y = 0;
                }
            }
            MotionState::SquatRv => {
                if !player.grounded {
                    player.motion_state = MotionState::Air;
                    player.motion_frame = 0;
                } else if let Some(action_state) = wait_action_state(input_facts) {
                    enter_action_state(player, action_state, stick_x);
                } else if input_facts.shield_held {
                    enter_guard(player);
                } else if input_facts.normal_jump_pressed {
                    enter_knee_bend(player, input_facts.normal_jump_input);
                } else if input_facts.walk_direction != 0 {
                    enter_walk(
                        player,
                        walk_motion_state(input_facts.walk_speed_bucket),
                        stick_x,
                    );
                } else {
                    player.motion_frame = player.motion_frame.saturating_add(1);
                    player.velocity.x = 0;
                    player.velocity.y = 0;
                    if player.motion_frame >= player.profile.action_frames.squat_rv_total_frames {
                        player.motion_state = MotionState::Wait;
                        player.motion_frame = 0;
                    }
                }
            }
            MotionState::SpecialN
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
                player.velocity.x = 0;
                if let Some(next_state) = grounded_action_iasa_state(player, input_facts) {
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
                    player.motion_state = MotionState::Air;
                    player.motion_frame = 0;
                } else if !input_facts.shield_held {
                    enter_guard_off(player);
                } else if let Some(action_state) = guard_on_action_state(
                    player,
                    input_facts,
                    GuardInputContext {
                        facing: player.facing,
                        stage,
                        x_tap_timer,
                        y_tap_timer,
                        stick_y,
                        common_data,
                    },
                ) {
                    if action_state == MotionState::Pass {
                        enter_pass(player, stage, common_data);
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
            MotionState::Guard => {
                if !player.grounded {
                    clear_guard_state(player);
                    player.motion_state = MotionState::Air;
                    player.motion_frame = 0;
                } else if !input_facts.shield_held {
                    enter_guard_off(player);
                } else if let Some(action_state) = guard_action_state(
                    player,
                    input_facts,
                    GuardInputContext {
                        facing: player.facing,
                        stage,
                        x_tap_timer,
                        y_tap_timer,
                        stick_y,
                        common_data,
                    },
                ) {
                    if action_state == MotionState::Pass {
                        enter_pass(player, stage, common_data);
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
                    player.motion_state = MotionState::Air;
                    player.motion_frame = 0;
                } else {
                    player.motion_frame = player.motion_frame.saturating_add(1);
                    apply_ground_traction(player);
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
            MotionState::KneeBend => {
                apply_ground_traction(player);
                player.motion_frame = player.motion_frame.saturating_add(1);
                if player.motion_frame >= player.profile.jumpsquat_frames {
                    apply_jump_takeoff_velocity(player, stick_x);
                    player.velocity.y = ground_jump_vertical_velocity(player);
                    player.grounded = false;
                    player.motion_state = ground_jump_motion_state(player, stick_x, common_data);
                    player.motion_frame = 0;
                    player.fast_falling = false;
                    player.jumps_remaining = player.profile.max_jumps;
                    // Melee's first Jump physics call seeds the state without
                    // translating the fighter. The bottom ECB vertex remains
                    // the canonical position until the next physics tick.
                    skip_position_update_this_tick = true;
                } else if let Some(action_state) =
                    knee_bend_action_state(input_facts, stick_y, common_data)
                {
                    enter_action_state(player, action_state, stick_x);
                } else {
                    if input_facts.short_hop_released_for(player.jump_input) {
                        player.short_hop = true;
                    }
                }
            }
            MotionState::Air
            | MotionState::Fall
            | MotionState::JumpF
            | MotionState::JumpB
            | MotionState::JumpAerialF
            | MotionState::JumpAerialB => {
                player.motion_frame = player.motion_frame.saturating_add(1);
                apply_airborne_iasa_or_drift(player, input_facts, stick_x, stick_y, common_data);
            }
            MotionState::Pass => {
                player.motion_frame = player.motion_frame.saturating_add(1);
                apply_airborne_iasa_or_drift(player, input_facts, stick_x, stick_y, common_data);
            }
            MotionState::EscapeAir => {
                player.motion_frame = player.motion_frame.saturating_add(1);
                player.escape_air_iasa_timer = player.escape_air_iasa_timer.saturating_sub(1);
                if player.motion_frame >= common_data.escapeair_animation_ticks {
                    enter_fall_special(player);
                }
            }
            MotionState::FallSpecial => {
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
                apply_ground_traction(player);
                player.velocity.y = 0;
                if player.motion_frame >= common_data.escapeair_landing_lag_ticks {
                    player.motion_state = MotionState::Wait;
                    player.motion_frame = 0;
                }
            }
            MotionState::Landing => {
                if !has_floor_support(stage, player.position) {
                    enter_fall(player);
                    continue;
                }
                player.motion_frame = player.motion_frame.saturating_add(1);
                apply_ground_traction(player);
                player.velocity.y = 0;
                if player.motion_frame >= player.profile.normal_landing_lag_ticks {
                    player.motion_state = MotionState::Wait;
                    player.motion_frame = 0;
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

        if skip_position_update_this_tick {
            continue;
        }

        player.position.x += player.velocity.x;

        if player.grounded && !has_floor_support(stage, player.position) {
            enter_fall(player);
            continue;
        }

        if !player.grounded {
            if player.motion_state == MotionState::EscapeAir {
                apply_escape_air_decay(player, common_data);
                player.position.y += player.velocity.y;

                if let Some(contact) =
                    landing_contact_for_bottom(stage, previous_position, player.position, false)
                {
                    land_player_on_contact(player, contact.y);
                    enter_landing_fall_special(player);
                }
            } else {
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
                }
                player.position.y += player.velocity.y;
                if !player.fast_falling {
                    player.velocity.y = (player.velocity.y - player.profile.gravity_per_tick)
                        .max(-player.profile.fall_speed_per_tick);
                }

                let floor_skip_surface = (player.motion_state == MotionState::Pass)
                    .then_some(player.floor_skip_surface)
                    .flatten();
                let drop_through_soft_platforms = player.motion_state == MotionState::FallSpecial
                    && fall_special_skips_soft_platforms(stick_y, common_data);
                if let Some(contact) = landing_contact_for_bottom_with_floor_skip(
                    stage,
                    previous_position,
                    player.position,
                    floor_skip_surface,
                    drop_through_soft_platforms,
                ) {
                    let landing_state = player.motion_state;
                    land_player_on_contact(player, contact.y);
                    if matches!(
                        landing_state,
                        MotionState::EscapeAir | MotionState::FallSpecial
                    ) {
                        enter_landing_fall_special(player);
                    } else {
                        enter_landing(player);
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

fn enter_knee_bend(player: &mut PlayerState, jump_input: MeleeJumpInput) {
    clear_shield_turn(player);
    clear_turn_state(player);
    player.motion_state = MotionState::KneeBend;
    player.motion_frame = 1;
    player.jump_input = jump_input;
    player.short_hop = false;
    player.velocity.y = 0;
}

fn apply_jump_takeoff_velocity(player: &mut PlayerState, stick_x: i32) {
    let profile = player.profile;
    let carried_velocity = player.velocity.x * profile.ground_to_air_jump_momentum_milli / 1_000;
    let stick_velocity =
        stick_scaled_velocity(stick_x, profile.jump_horizontal_initial_velocity_per_tick);
    player.velocity.x = (carried_velocity + stick_velocity).clamp(
        -profile.jump_horizontal_max_velocity_per_tick,
        profile.jump_horizontal_max_velocity_per_tick,
    );
}

fn ground_jump_vertical_velocity(player: &PlayerState) -> i32 {
    if player.short_hop {
        player.profile.short_hop_jump_force_per_tick
    } else {
        player.profile.full_hop_jump_force_per_tick
    }
}

fn enter_walk(player: &mut PlayerState, motion_state: MotionState, stick_x: i32) {
    clear_shield_turn(player);
    clear_turn_state(player);
    player.motion_state = motion_state;
    player.motion_frame = 0;
    apply_walk_velocity(player, stick_x);
}

fn walk_motion_state(bucket: WalkSpeedBucket) -> MotionState {
    match bucket {
        WalkSpeedBucket::Slow => MotionState::WalkSlow,
        WalkSpeedBucket::Middle => MotionState::WalkMiddle,
        WalkSpeedBucket::Fast => MotionState::WalkFast,
        WalkSpeedBucket::None => MotionState::WalkSlow,
    }
}

fn enter_dash(player: &mut PlayerState, direction: i8) {
    clear_shield_turn(player);
    clear_turn_state(player);
    player.motion_state = MotionState::Dash;
    player.motion_frame = 0;
    player.facing = direction;
    player.velocity.x = direction as i32 * player.profile.initial_dash_speed_per_tick;
}

fn enter_run(player: &mut PlayerState) {
    clear_shield_turn(player);
    clear_turn_state(player);
    player.motion_state = MotionState::Run;
    player.motion_frame = 0;
    player.run_no_interrupt_frames = 0;
}

fn enter_run_from_turn_run(player: &mut PlayerState, common_data: MeleeCommonData) {
    enter_run(player);
    player.run_no_interrupt_frames = common_data.run_turn_run_no_interrupt_frames;
}

fn enter_run_brake(player: &mut PlayerState) {
    clear_shield_turn(player);
    clear_turn_state(player);
    player.motion_state = MotionState::RunBrake;
    player.motion_frame = 0;
    apply_ground_traction(player);
}

fn enter_turn_run(player: &mut PlayerState) {
    clear_shield_turn(player);
    let accel_mul = player.facing;
    clear_turn_state(player);
    player.motion_state = MotionState::TurnRun;
    player.motion_frame = 0;
    player.turn_facing_after = -accel_mul;
    player.turn_run_accel_mul = accel_mul;
    player.turn_has_turned = false;
    player.turn_just_turned = false;
    apply_ground_traction(player);
}

fn enter_smash_turn(player: &mut PlayerState, direction: i8) {
    clear_shield_turn(player);
    let dash_after_direction = player.facing;
    player.motion_state = MotionState::Turn;
    player.motion_frame = 0;
    player.turn_facing_after = direction;
    player.turn_has_turned = false;
    player.turn_just_turned = false;
    player.turn_frames_to_turn = 0;
    player.turn_dash_after_direction = dash_after_direction;
    player.turn_latched_buttons = 0;
    player.velocity.x = 0;
}

fn enter_standing_turn(player: &mut PlayerState, direction: i8) {
    clear_shield_turn(player);
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
    player.velocity.x = 0;
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

fn turn_effective_input_facts(
    player: &PlayerState,
    input_facts: MeleeInputFacts,
) -> MeleeInputFacts {
    if !player.turn_just_turned || player.turn_latched_buttons == 0 {
        return input_facts;
    }

    let mut facts = input_facts;
    if player.turn_latched_buttons & TURN_LATCH_ATTACK != 0 {
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

    if player.turn_latched_buttons & TURN_LATCH_SPECIAL != 0 {
        facts.special_pressed = true;
        if facts.special_direction == (0, 0) {
            facts.special_direction = special_direction_from_turn_facts(facts);
        }
    }

    facts
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
    facts: MeleeInputFacts,
    common_data: MeleeCommonData,
) {
    if stick_x * player.turn_facing_after as i32 >= common_data.dash_x as i32
        && (x_tap_timer < common_data.dash_tap_window
            || facts.ucf_dashback_direction == player.turn_facing_after)
    {
        player.turn_dash_after_direction = player.turn_facing_after;
    }
}

fn apply_turn_run_velocity(player: &mut PlayerState, stick_x: i32) {
    let (accel, target_velocity) = dash_run_accel_and_target(player.profile, stick_x);
    if accel != 0 && player.turn_run_accel_mul as i32 * accel < 0 {
        player.velocity.x = apply_ground_accel_toward_target(
            player.velocity.x,
            accel,
            target_velocity,
            player.profile.traction_per_tick,
            player.profile.ground_max_horizontal_velocity_per_tick,
        );
    } else {
        apply_ground_traction(player);
    }
}

fn advance_turn_run_facing(player: &mut PlayerState) {
    if !player.turn_has_turned && player.velocity.x * player.turn_run_accel_mul as i32 <= 0 {
        player.facing = player.turn_facing_after;
        player.turn_has_turned = true;
        player.turn_just_turned = true;
    } else {
        player.turn_just_turned = false;
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

fn enter_squat(player: &mut PlayerState) {
    clear_shield_turn(player);
    clear_turn_state(player);
    player.motion_state = MotionState::Squat;
    player.motion_frame = 0;
    player.velocity.x = 0;
    player.velocity.y = 0;
}

fn enter_squat_wait(player: &mut PlayerState) {
    clear_shield_turn(player);
    clear_turn_state(player);
    player.motion_state = MotionState::SquatWait;
    player.motion_frame = 0;
    player.velocity.x = 0;
    player.velocity.y = 0;
}

fn enter_squat_rv(player: &mut PlayerState) {
    clear_shield_turn(player);
    clear_turn_state(player);
    player.motion_state = MotionState::SquatRv;
    player.motion_frame = 0;
    player.velocity.x = 0;
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
    player.motion_state = MotionState::GuardOn;
    player.motion_frame = 0;
    player.velocity.x = 0;
    player.velocity.y = 0;
    player.guard_catch_dash_window = catch_dash_window;
    clear_shield_turn(player);
}

fn enter_guard_steady(player: &mut PlayerState) {
    clear_turn_state(player);
    player.motion_state = MotionState::Guard;
    player.motion_frame = 0;
    player.velocity.x = 0;
    player.velocity.y = 0;
    player.guard_catch_dash_window = 0;
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
    player.motion_state = MotionState::GuardOff;
    player.motion_frame = 0;
    player.velocity.y = 0;
}

fn enter_pass(player: &mut PlayerState, stage: StageProfile, common_data: MeleeCommonData) {
    clear_guard_state(player);
    clear_turn_state(player);
    player.motion_state = MotionState::Pass;
    player.motion_frame = 0;
    player.grounded = false;
    player.fast_falling = false;
    player.velocity.y = common_data.pass_initial_y_velocity;
    player.floor_skip_surface =
        floor_surface_index_for_bottom(stage, player.position).map(|(index, _)| index);
}

fn enter_action_state(player: &mut PlayerState, motion_state: MotionState, stick_x: i32) {
    clear_guard_state(player);
    clear_turn_state(player);
    player.motion_state = motion_state;
    player.motion_frame = 0;
    if motion_state == MotionState::SpecialS && stick_x != 0 {
        player.facing = stick_x.signum() as i8;
    }
    player.velocity.x = 0;
    player.velocity.y = 0;
}

fn grounded_action_total_frames(player: &PlayerState) -> u8 {
    match player.motion_state {
        MotionState::SpecialN => FALCON_SPECIAL_N_FRAMES,
        MotionState::SpecialS => FALCON_SPECIAL_S_FRAMES,
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
            (input_facts.walk_direction != 0)
                .then_some(walk_motion_state(input_facts.walk_speed_bucket))
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
        MotionState::Pass => enter_pass(player, stage, common_data),
        MotionState::KneeBend => enter_knee_bend(player, jump_input),
        MotionState::Dash => enter_dash(player, player.facing),
        MotionState::Squat => enter_squat(player),
        MotionState::Turn => enter_smash_turn(player, -player.facing),
        MotionState::WalkSlow | MotionState::WalkMiddle | MotionState::WalkFast => {
            enter_walk(player, motion_state, stick_x);
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
    player.motion_state = MotionState::EscapeAir;
    player.motion_frame = 0;
    player.escape_air_iasa_timer = common_data.escapeair_iasa_timer_ticks;
    player.fast_falling = false;
    let (velocity_x, velocity_y) = escape_air_velocity(stick_x, stick_y as i32, common_data);
    player.velocity.x = velocity_x;
    player.velocity.y = velocity_y;
}

fn fall_special_skips_soft_platforms(stick_y: i8, common_data: MeleeCommonData) -> bool {
    stick_y <= common_data.fallspecial_platform_landing_y
}

fn enter_fall_special(player: &mut PlayerState) {
    player.motion_state = MotionState::FallSpecial;
    player.motion_frame = 0;
    player.escape_air_iasa_timer = 0;
    player.fast_falling = false;
}

fn enter_fall(player: &mut PlayerState) {
    player.motion_state = MotionState::Fall;
    player.motion_frame = 0;
    player.grounded = false;
    player.escape_air_iasa_timer = 0;
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
}

fn land_player_on_contact(player: &mut PlayerState, y: i32) {
    player.position.y = y;
    player.velocity.y = 0;
    player.grounded = true;
    player.fast_falling = false;
    player.jumps_remaining = player.profile.max_jumps;
    player.jump_input = Default::default();
    player.short_hop = false;
    player.floor_skip_surface = None;
}

fn enter_landing(player: &mut PlayerState) {
    clear_shield_turn(player);
    clear_turn_state(player);
    player.motion_state = MotionState::Landing;
    player.motion_frame = 0;
    player.grounded = true;
    player.fast_falling = false;
    player.velocity.y = 0;
}

fn enter_air_special(player: &mut PlayerState, motion_state: MotionState, stick_x: i32) {
    player.floor_skip_surface = None;
    player.motion_state = motion_state;
    player.motion_frame = 0;
    player.fast_falling = false;
    if motion_state == MotionState::SpecialAirS && stick_x != 0 {
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
    if input_facts.special_pressed {
        enter_air_special(
            player,
            air_special_state_from_direction(input_facts.air_special_direction),
            stick_x,
        );
    } else if input_facts.air_dodge_pressed {
        enter_escape_air(player, stick_x, stick_y, common_data);
    } else if input_facts.air_attack_pressed {
        enter_air_attack(
            player,
            air_attack_state_from_direction(input_facts.air_attack_direction, player.facing),
        );
    } else if input_facts.normal_jump_pressed && player.jumps_remaining > 0 {
        enter_air_jump(player, stick_x, common_data);
    } else {
        apply_air_drift(player, stick_x);
    }
}

fn enter_ground_escape(player: &mut PlayerState, motion_state: MotionState) {
    player.motion_state = motion_state;
    player.motion_frame = 0;
    player.velocity.x = 0;
    player.velocity.y = 0;
}

fn apply_walk_velocity(player: &mut PlayerState, stick_x: i32) {
    if stick_x > 0 {
        player.facing = 1;
    } else if stick_x < 0 {
        player.facing = -1;
    }

    let profile = player.profile;
    let target_velocity = stick_x * profile.walk_target_speed_per_stick;
    let mut accel = stick_x * profile.walk_initial_accel_per_stick;
    if stick_x > 0 {
        accel += profile.walk_accel_per_tick;
    } else if stick_x < 0 {
        accel -= profile.walk_accel_per_tick;
    }

    player.velocity.x = apply_ground_accel_toward_target(
        player.velocity.x,
        accel,
        target_velocity,
        profile.walk_friction_per_tick,
        profile.walk_speed_per_tick,
    );
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

fn apply_dash_velocity(player: &mut PlayerState, stick_x: i32) {
    let (accel, target_velocity) = dash_run_accel_and_target(player.profile, stick_x);
    player.velocity.x = apply_ground_accel_toward_target(
        player.velocity.x,
        accel,
        target_velocity,
        player.profile.traction_per_tick,
        player.profile.ground_max_horizontal_velocity_per_tick,
    );
}

fn exit_dash(
    player: &mut PlayerState,
    stick_x: i32,
    walk_bucket: WalkSpeedBucket,
    common_data: MeleeCommonData,
) {
    if is_same_direction_run(stick_x, player.facing, common_data) {
        enter_run(player);
    } else if stick_x != 0 {
        enter_walk_after_dash(player, stick_x, walk_bucket);
    } else {
        player.motion_state = MotionState::Wait;
        player.motion_frame = 0;
        player.velocity.x = 0;
    }
}

fn enter_walk_after_dash(player: &mut PlayerState, stick_x: i32, walk_bucket: WalkSpeedBucket) {
    player.motion_state = walk_motion_state(walk_bucket);
    player.motion_frame = 0;
    if stick_x > 0 {
        player.facing = 1;
    } else if stick_x < 0 {
        player.facing = -1;
    }
}

fn enter_wait_from_walk(player: &mut PlayerState) {
    player.motion_state = MotionState::Wait;
    player.motion_frame = 0;
    player.velocity.x = 0;
}

fn is_same_direction_run(stick_x: i32, facing: i8, common_data: MeleeCommonData) -> bool {
    stick_x.abs() >= common_data.run_x as i32 && stick_x.signum() == facing as i32
}

fn is_opposite_run_turn(stick_x: i32, facing: i8, common_data: MeleeCommonData) -> bool {
    stick_x.abs() >= common_data.run_x as i32 && stick_x.signum() == -(facing as i32)
}

fn apply_ground_traction(player: &mut PlayerState) {
    player.velocity.x = apply_friction_to_zero(player.velocity.x, player.profile.traction_per_tick);
}

fn stick_scaled_velocity(stick_x: i32, full_stick_velocity: i32) -> i32 {
    const FULL_NATIVE_STICK: i32 = 127;
    let scaled = stick_x * full_stick_velocity;
    if scaled >= 0 {
        (scaled + FULL_NATIVE_STICK / 2) / FULL_NATIVE_STICK
    } else {
        (scaled - FULL_NATIVE_STICK / 2) / FULL_NATIVE_STICK
    }
}

fn dash_run_accel_and_target(profile: crate::FighterProfile, stick_x: i32) -> (i32, i32) {
    let stick_accel = stick_scaled_velocity(stick_x, profile.dash_run_accel_stick_per_tick);
    let base_accel = stick_x.signum() * profile.dash_run_accel_base_per_tick;
    let target_velocity = stick_scaled_velocity(stick_x, profile.run_speed_per_tick);

    (stick_accel + base_accel, target_velocity)
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
    player.velocity.x = player.velocity.x * common_data.escapeair_decay_percent / 100;
    player.velocity.y = player.velocity.y * common_data.escapeair_decay_percent / 100;
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
    if is_fresh_walk_dash_tap(x_tap_timer, common_data) || facts.ucf_dashback_direction != 0 {
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
    if is_fresh_walk_dash_tap(x_tap_timer, common_data) || facts.ucf_dashback_direction != 0 {
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
    common_data: MeleeCommonData,
) -> Option<MotionState> {
    if facts.special_pressed && matches!(facts.special_direction, (1 | -1, 0)) {
        return Some(MotionState::SpecialS);
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
    (in_late_dash_attack_window && facts.attack_pressed).then_some(MotionState::AttackDash)
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
    x_tap_timer: u8,
    y_tap_timer: u8,
    stick_y: i8,
    common_data: MeleeCommonData,
}

fn turn_action_state(facts: MeleeInputFacts) -> Option<MotionState> {
    if facts.special_pressed {
        match facts.special_direction {
            (1 | -1, 0) => return Some(MotionState::SpecialS),
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
    if let Some(spot_dodge_state) = guard_spot_dodge_state(player, facts, context) {
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
    if let Some(spot_dodge_state) = guard_spot_dodge_state(player, facts, context) {
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

fn guard_spot_dodge_state(
    player: &PlayerState,
    facts: MeleeInputFacts,
    context: GuardInputContext,
) -> Option<MotionState> {
    if facts.cstick_spot_dodge {
        return Some(MotionState::EscapeN);
    }

    if facts.main_stick_spot_dodge && !ucf_suppresses_spot_dodge_for_pass(player, facts, context) {
        return Some(MotionState::EscapeN);
    }

    None
}

fn ucf_suppresses_spot_dodge_for_pass(
    player: &PlayerState,
    facts: MeleeInputFacts,
    context: GuardInputContext,
) -> bool {
    facts.ucf_shield_drop
        && context.x_tap_timer >= context.common_data.escape_x_tap_window
        && context.stick_y > -UCF_AXE_SPOT_DODGE_SUPPRESSION_Y
        && grounded_on_soft_platform(player, context.stage)
}

fn guard_platform_pass_state(
    player: &PlayerState,
    facts: MeleeInputFacts,
    context: GuardInputContext,
) -> Option<MotionState> {
    if !facts.source_held.lr() || !grounded_on_soft_platform(player, context.stage) {
        return None;
    }

    let source_pass_gate = context.stick_y <= -context.common_data.platform_pass_y
        && context.y_tap_timer < context.common_data.platform_pass_y_tap_window;
    (source_pass_gate || facts.ucf_shield_drop).then_some(MotionState::Pass)
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
        (1 | -1, 0) => Some(MotionState::SpecialS),
        (0, 1) => Some(MotionState::SpecialHi),
        (0, 0) => Some(MotionState::SpecialN),
        (0, -1) => Some(MotionState::SpecialLw),
        NO_GROUNDED_SPECIAL_DIRECTION => None,
        _ => None,
    }
}

fn special_state_from_direction(direction: (i8, i8)) -> Option<MotionState> {
    match direction {
        (1 | -1, 0) => Some(MotionState::SpecialS),
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
        (1 | -1, 0) => MotionState::SpecialAirS,
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
