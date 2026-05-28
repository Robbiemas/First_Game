use crate::input::NO_GROUNDED_SPECIAL_DIRECTION;
use crate::{
    state::EXPIRED_INPUT_TIMER, Frame, MeleeInputFacts, MeleeInputThresholds, MeleeJumpInput,
    MotionState, PlayerInput, PlayerState, WalkSpeedBucket, World,
};

const GROUND_Y: i32 = 0;
const JUMP_H_INITIAL_VELOCITY_PER_STICK: i32 = 4;
const AIR_JUMP_H_INITIAL_VELOCITY_PER_STICK: i32 = 4;
const AIR_JUMP_BACKWARD_X: i32 =
    crate::common_data::MeleeCommonData::PROVISIONAL.air_jump_backward_x as i32;
const JUMP_H_MAX_VELOCITY: i32 = 1_000;
const GROUND_TO_AIR_JUMP_MOMENTUM_PERCENT: i32 = 80;
const AIR_DRIFT_TARGET_SPEED_PER_STICK: i32 = 6;
const AIR_DRIFT_ACCEL_PER_TICK: i32 = 18;
const AIR_FRICTION_PER_TICK: i32 = 12;
const AIR_DRIFT_MAX_SPEED_PER_TICK: i32 = 1_000;
const JUMP_CANCEL_UP_SMASH_Y: i8 = crate::common_data::MeleeCommonData::PROVISIONAL.smash_y;
const FAST_FALL_STICK_THRESHOLD: i8 = -80;
const FAST_FALL_TAP_WINDOW: u8 = 2;
const ATTACK_ACTIVE_TICKS: u8 = 12;
const FALCON_TURN_FRAMES: u8 = 11;
const FALCON_STANDING_TURN_FACING_FLIP_FRAME: u8 = 5;
const SHIELD_TURN_FRAMES: u8 = 5;
const TURN_LATCH_ATTACK: u8 = 0x01;
const TURN_LATCH_SPECIAL: u8 = 0x02;
const DASH_STICK_THRESHOLD: i32 = crate::common_data::MeleeCommonData::PROVISIONAL.dash_x as i32;
const DASH_TAP_WINDOW: u8 = crate::common_data::MeleeCommonData::PROVISIONAL.dash_tap_window;
const RUN_STICK_THRESHOLD: i32 = 64;
const ESCAPE_AIR_DEADZONE_X: i8 =
    crate::common_data::MeleeCommonData::PROVISIONAL.escapeair_deadzone_x;
const ESCAPE_AIR_DEADZONE_Y: i8 =
    crate::common_data::MeleeCommonData::PROVISIONAL.escapeair_deadzone_y;
const ESCAPE_AIR_FORCE: i32 = crate::common_data::MeleeCommonData::PROVISIONAL.escapeair_force;
const ESCAPE_AIR_IASA_TIMER_TICKS: u8 =
    crate::common_data::MeleeCommonData::PROVISIONAL.escapeair_iasa_timer_ticks;
const ESCAPE_AIR_ANIMATION_TICKS: u8 =
    crate::common_data::MeleeCommonData::PROVISIONAL.escapeair_animation_ticks;
const ESCAPE_AIR_DECAY_PERCENT: i32 =
    crate::common_data::MeleeCommonData::PROVISIONAL.escapeair_decay_percent;
const ESCAPE_AIR_LANDING_FALL_SPECIAL_TICKS: u8 =
    crate::common_data::MeleeCommonData::PROVISIONAL.escapeair_landing_lag_ticks;
const FALCON_ATTACK1_FRAMES: u8 = 21;
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
const FALCON_ESCAPE_N_FRAMES: u8 = 23;
const FALCON_ESCAPE_F_FRAMES: u8 = 31;
const FALCON_ESCAPE_B_FRAMES: u8 = 31;
const FALCON_ATTACK_DASH_FRAMES: u8 = 39;
const FALCON_CATCH_DASH_FRAMES: u8 = 40;
const FALCON_LANDING_FRAMES: u8 = 4;
const GUARD_ON_TICKS: u8 = 4;
const DASH_EARLY_ACTION_WINDOW: u8 =
    crate::common_data::MeleeCommonData::PROVISIONAL.dash_early_action_window;
const DASH_DEFENSIVE_ACTION_WINDOW: u8 =
    crate::common_data::MeleeCommonData::PROVISIONAL.dash_defensive_action_window;
const DASH_LATE_ACTION_WINDOW: u8 =
    crate::common_data::MeleeCommonData::PROVISIONAL.dash_late_action_window;
const GUARD_ON_CATCH_DASH_WINDOW: u8 =
    crate::common_data::MeleeCommonData::PROVISIONAL.guard_on_catch_dash_window;
const RUN_TURN_RUN_NO_INTERRUPT_FRAMES: u8 =
    crate::common_data::MeleeCommonData::PROVISIONAL.run_turn_run_no_interrupt_frames;
const GUARD_OFF_FRAMES: u8 = 15;
const FALCON_ATTACK1_IASA: u8 = 16;
const FALCON_ATTACK_DASH_IASA: u8 = 38;
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

    for (player_index, ((player, input), previous_input)) in world
        .players_mut()
        .iter_mut()
        .zip(inputs.iter().copied())
        .zip(previous_inputs.iter().copied())
        .enumerate()
    {
        let stick_x = input.stick_x() as i32;
        let stick_y = input.stick_y();
        let input_snapshot = input.melee_snapshot(previous_input, input_timers[player_index]);
        let input_facts = input_snapshot.facts(MeleeInputThresholds::default());
        last_input_facts[player_index] = input_facts;
        input_timers[player_index].x_tap = input_snapshot.x_tap_timer;
        input_timers[player_index].y_tap = input_snapshot.y_tap_timer;
        input_timers[player_index].trigger = input_snapshot.trigger_timer;
        let x_tap_timer = input_timers[player_index].x_tap;

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
                let walk_forward_dash =
                    fresh_walk_forward_dash_direction(input_facts, x_tap_timer, player.facing);
                let walk_smash_turn =
                    fresh_walk_smash_turn_direction(input_facts, x_tap_timer, player.facing);

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
                    dash_action_state(input_facts, player.motion_frame, player.facing)
                {
                    enter_action_state(player, action_state, stick_x);
                } else if input_facts.smash_turn_direction(player.facing) != 0 {
                    enter_smash_turn(player, input_facts.smash_turn_direction(player.facing));
                } else if input_facts.shield_held {
                    enter_guard_from_run(player);
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
                        exit_dash(player, stick_x, input_facts.walk_speed_bucket);
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
                    enter_guard_from_run(player);
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
                } else if is_same_direction_run(stick_x, player.facing) {
                    player.motion_frame = player.motion_frame.saturating_add(1);
                    apply_dash_velocity(player, stick_x);
                } else if is_opposite_run_turn(stick_x, player.facing) {
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
                    if player.velocity.x == 0 {
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
                        && is_same_direction_run(stick_x, player.facing)
                    {
                        enter_run_from_turn_run(player);
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
                        arm_turn_dash_after_if_fresh(player, stick_x, x_tap_timer, input_facts);
                        if player.turn_just_turned
                            && player.turn_dash_after_direction != 0
                            && stick_x * player.turn_facing_after as i32 >= DASH_STICK_THRESHOLD
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
                        && player.motion_frame >= FALCON_TURN_FRAMES
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
                } else if input_facts.crouch {
                    player.motion_frame = player.motion_frame.saturating_add(1);
                    player.velocity.x = 0;
                } else {
                    player.motion_state = MotionState::Wait;
                    player.motion_frame = 0;
                    player.velocity.x = 0;
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
                    enter_iasa_state(player, next_state, input_facts.normal_jump_input, stick_x);
                } else if player.motion_frame >= grounded_action_total_frames(player.motion_state) {
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
                } else if let Some(action_state) =
                    guard_on_action_state(player, input_facts, player.facing)
                {
                    enter_iasa_state(player, action_state, input_facts.jump_input, stick_x);
                } else {
                    player.motion_frame = player.motion_frame.saturating_add(1);
                    update_shield_turn(player, input_facts);
                    player.velocity.y = 0;
                    if player.motion_frame >= GUARD_ON_TICKS {
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
                } else if let Some(action_state) = guard_action_state(input_facts, player.facing) {
                    enter_iasa_state(player, action_state, input_facts.jump_input, stick_x);
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
                        enter_iasa_state(player, action_state, input_facts.jump_input, stick_x);
                    } else if player.motion_frame >= GUARD_OFF_FRAMES {
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
                    player.motion_state = ground_jump_motion_state(player, stick_x);
                    player.motion_frame = 0;
                    player.fast_falling = false;
                    player.jumps_remaining = 1;
                } else if let Some(action_state) = knee_bend_action_state(input_facts, stick_y) {
                    enter_action_state(player, action_state, stick_x);
                } else {
                    if input_facts.short_hop_released_for(player.jump_input) {
                        player.short_hop = true;
                    }
                }
            }
            MotionState::Air
            | MotionState::JumpF
            | MotionState::JumpB
            | MotionState::JumpAerialF
            | MotionState::JumpAerialB => {
                player.motion_frame = player.motion_frame.saturating_add(1);
                if input_facts.special_pressed {
                    enter_air_special(
                        player,
                        air_special_state_from_direction(input_facts.air_special_direction),
                        stick_x,
                    );
                } else if input_facts.air_dodge_pressed {
                    enter_escape_air(player, stick_x, stick_y);
                } else if input_facts.air_attack_pressed {
                    enter_air_attack(
                        player,
                        air_attack_state_from_direction(
                            input_facts.air_attack_direction,
                            player.facing,
                        ),
                    );
                } else if input_facts.normal_jump_pressed && player.jumps_remaining > 0 {
                    enter_air_jump(player, stick_x);
                } else {
                    apply_air_drift(player, stick_x);
                }
            }
            MotionState::EscapeAir => {
                player.motion_frame = player.motion_frame.saturating_add(1);
                player.escape_air_iasa_timer = player.escape_air_iasa_timer.saturating_sub(1);
                if player.motion_frame >= ESCAPE_AIR_ANIMATION_TICKS {
                    enter_fall_special(player);
                }
            }
            MotionState::FallSpecial => {
                if input_facts.normal_jump_pressed && player.jumps_remaining > 0 {
                    enter_air_jump(player, stick_x);
                } else {
                    player.motion_frame = player.motion_frame.saturating_add(1);
                }
            }
            MotionState::LandingFallSpecial => {
                player.motion_frame = player.motion_frame.saturating_add(1);
                apply_ground_traction(player);
                player.velocity.y = 0;
                if player.motion_frame >= ESCAPE_AIR_LANDING_FALL_SPECIAL_TICKS {
                    player.motion_state = MotionState::Wait;
                    player.motion_frame = 0;
                }
            }
            MotionState::Landing => {
                player.motion_frame = player.motion_frame.saturating_add(1);
                apply_ground_traction(player);
                player.velocity.y = 0;
                if player.motion_frame >= FALCON_LANDING_FRAMES {
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

        player.position.x += player.velocity.x;

        if player.grounded {
            player.position.y = GROUND_Y;
        } else if player.motion_state == MotionState::EscapeAir {
            apply_escape_air_decay(player);
            player.position.y += player.velocity.y;

            if player.position.y <= GROUND_Y {
                player.position.y = GROUND_Y;
                player.velocity.y = 0;
                player.grounded = true;
                player.fast_falling = false;
                player.jumps_remaining = 1;
                player.jump_input = Default::default();
                player.short_hop = false;
                enter_landing_fall_special(player);
            }
        } else {
            let fast_fall_tap = stick_y <= FAST_FALL_STICK_THRESHOLD
                && input_timers[player_index].y_tap < FAST_FALL_TAP_WINDOW;
            let starts_fast_fall = !player.fast_falling && player.velocity.y < 0 && fast_fall_tap;
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

            if player.position.y <= GROUND_Y {
                let landing_state = player.motion_state;
                player.position.y = GROUND_Y;
                player.velocity.y = 0;
                player.grounded = true;
                player.fast_falling = false;
                player.jumps_remaining = 1;
                player.jump_input = Default::default();
                player.short_hop = false;
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
    let carried_velocity = player.velocity.x * GROUND_TO_AIR_JUMP_MOMENTUM_PERCENT / 100;
    let stick_velocity = stick_x * JUMP_H_INITIAL_VELOCITY_PER_STICK;
    player.velocity.x =
        (carried_velocity + stick_velocity).clamp(-JUMP_H_MAX_VELOCITY, JUMP_H_MAX_VELOCITY);
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

fn enter_run_from_turn_run(player: &mut PlayerState) {
    enter_run(player);
    player.run_no_interrupt_frames = RUN_TURN_RUN_NO_INTERRUPT_FRAMES;
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
    player.turn_frames_to_turn = FALCON_STANDING_TURN_FACING_FLIP_FRAME.saturating_sub(1);
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
) {
    if stick_x * player.turn_facing_after as i32 >= DASH_STICK_THRESHOLD
        && (x_tap_timer < DASH_TAP_WINDOW
            || facts.ucf_dashback_direction == player.turn_facing_after)
    {
        player.turn_dash_after_direction = player.turn_facing_after;
    }
}

fn apply_turn_run_velocity(player: &mut PlayerState, stick_x: i32) {
    let accel = stick_x * player.profile.dash_accel_per_stick;
    if accel != 0 && player.turn_run_accel_mul as i32 * accel < 0 {
        player.velocity.x = (player.velocity.x + accel).clamp(
            -player.profile.run_speed_per_tick,
            player.profile.run_speed_per_tick,
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

fn enter_guard(player: &mut PlayerState) {
    enter_guard_on(player, 0);
}

fn enter_guard_from_run(player: &mut PlayerState) {
    enter_guard_on(player, GUARD_ON_CATCH_DASH_WINDOW);
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

fn grounded_action_total_frames(motion_state: MotionState) -> u8 {
    match motion_state {
        MotionState::SpecialN => FALCON_SPECIAL_N_FRAMES,
        MotionState::SpecialS => FALCON_SPECIAL_S_FRAMES,
        MotionState::SpecialHi => FALCON_SPECIAL_HI_FRAMES,
        MotionState::SpecialLw => FALCON_SPECIAL_LW_FRAMES,
        MotionState::EscapeN => FALCON_ESCAPE_N_FRAMES,
        MotionState::EscapeF => FALCON_ESCAPE_F_FRAMES,
        MotionState::EscapeB => FALCON_ESCAPE_B_FRAMES,
        MotionState::Catch => FALCON_CATCH_FRAMES,
        MotionState::CatchDash => FALCON_CATCH_DASH_FRAMES,
        MotionState::Attack1 => FALCON_ATTACK1_FRAMES,
        MotionState::AttackDash => FALCON_ATTACK_DASH_FRAMES,
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
    if player.motion_frame < grounded_action_iasa_frame(player.motion_state)? {
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

fn grounded_action_iasa_frame(motion_state: MotionState) -> Option<u8> {
    match motion_state {
        MotionState::SpecialN => Some(FALCON_SPECIAL_N_IASA),
        MotionState::Attack1 => Some(FALCON_ATTACK1_IASA),
        MotionState::AttackDash => Some(FALCON_ATTACK_DASH_IASA),
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
) {
    match motion_state {
        MotionState::Guard => {
            enter_guard(player);
        }
        MotionState::GuardOff => enter_guard_off(player),
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

fn enter_escape_air(player: &mut PlayerState, stick_x: i32, stick_y: i8) {
    player.motion_state = MotionState::EscapeAir;
    player.motion_frame = 0;
    player.escape_air_iasa_timer = ESCAPE_AIR_IASA_TIMER_TICKS;
    player.fast_falling = false;
    let (velocity_x, velocity_y) = escape_air_velocity(stick_x, stick_y as i32);
    player.velocity.x = velocity_x;
    player.velocity.y = velocity_y;
}

fn enter_fall_special(player: &mut PlayerState) {
    player.motion_state = MotionState::FallSpecial;
    player.motion_frame = 0;
    player.escape_air_iasa_timer = 0;
    player.fast_falling = false;
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
    player.motion_state = motion_state;
    player.motion_frame = 0;
    player.fast_falling = false;
    if motion_state == MotionState::SpecialAirS && stick_x != 0 {
        player.facing = stick_x.signum() as i8;
    }
}

fn ground_jump_motion_state(player: &PlayerState, stick_x: i32) -> MotionState {
    if stick_x * player.facing as i32 > -AIR_JUMP_BACKWARD_X {
        MotionState::JumpF
    } else {
        MotionState::JumpB
    }
}

fn enter_air_jump(player: &mut PlayerState, stick_x: i32) {
    player.motion_state = if stick_x * player.facing as i32 > -AIR_JUMP_BACKWARD_X {
        MotionState::JumpAerialF
    } else {
        MotionState::JumpAerialB
    };
    player.motion_frame = 0;
    player.velocity.x = stick_x * AIR_JUMP_H_INITIAL_VELOCITY_PER_STICK;
    player.velocity.y = player.profile.air_jump_force_per_tick;
    player.fast_falling = false;
    player.jumps_remaining -= 1;
}

fn enter_air_attack(player: &mut PlayerState, motion_state: MotionState) {
    player.motion_state = motion_state;
    player.motion_frame = 0;
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
    let target_velocity = stick_x * AIR_DRIFT_TARGET_SPEED_PER_STICK;
    if target_velocity == 0 {
        player.velocity.x = apply_friction_to_zero(player.velocity.x, AIR_FRICTION_PER_TICK);
        return;
    }

    let difference = target_velocity - player.velocity.x;
    let accel = difference.signum().saturating_mul(AIR_DRIFT_ACCEL_PER_TICK);
    player.velocity.x = if difference.abs() <= AIR_DRIFT_ACCEL_PER_TICK {
        target_velocity
    } else {
        player.velocity.x + accel
    }
    .clamp(-AIR_DRIFT_MAX_SPEED_PER_TICK, AIR_DRIFT_MAX_SPEED_PER_TICK);
}

fn apply_dash_velocity(player: &mut PlayerState, stick_x: i32) {
    let velocity = player.velocity.x + stick_x * player.profile.dash_accel_per_stick;
    player.velocity.x = velocity.clamp(
        -player.profile.run_speed_per_tick,
        player.profile.run_speed_per_tick,
    );
}

fn exit_dash(player: &mut PlayerState, stick_x: i32, walk_bucket: WalkSpeedBucket) {
    if is_same_direction_run(stick_x, player.facing) {
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

fn is_same_direction_run(stick_x: i32, facing: i8) -> bool {
    stick_x.abs() >= RUN_STICK_THRESHOLD && stick_x.signum() == facing as i32
}

fn is_opposite_run_turn(stick_x: i32, facing: i8) -> bool {
    stick_x.abs() >= RUN_STICK_THRESHOLD && stick_x.signum() == -(facing as i32)
}

fn apply_ground_traction(player: &mut PlayerState) {
    player.velocity.x = player.velocity.x * (1_000 - player.profile.traction_per_tick) / 1_000;
    if player.velocity.x.abs() < 5 {
        player.velocity.x = 0;
    }
}

fn apply_escape_air_decay(player: &mut PlayerState) {
    player.velocity.x = player.velocity.x * ESCAPE_AIR_DECAY_PERCENT / 100;
    player.velocity.y = player.velocity.y * ESCAPE_AIR_DECAY_PERCENT / 100;
}

fn escape_air_velocity(stick_x: i32, stick_y: i32) -> (i32, i32) {
    if stick_x.abs() < ESCAPE_AIR_DEADZONE_X as i32 && stick_y.abs() < ESCAPE_AIR_DEADZONE_Y as i32
    {
        return (0, 0);
    }

    if stick_y == 0 {
        return (stick_x.signum() * ESCAPE_AIR_FORCE, 0);
    }
    if stick_x == 0 {
        return (0, stick_y.signum() * ESCAPE_AIR_FORCE);
    }

    let magnitude = scaled_vector_magnitude(stick_x, stick_y);
    (
        fixed_force_component(stick_x, magnitude),
        fixed_force_component(stick_y, magnitude),
    )
}

fn fixed_force_component(axis: i32, scaled_magnitude: i64) -> i32 {
    const SCALE: i64 = 1024;
    let component = (axis.abs() as i64 * ESCAPE_AIR_FORCE as i64 * SCALE + scaled_magnitude / 2)
        / scaled_magnitude;
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

fn fresh_walk_forward_dash_direction(facts: MeleeInputFacts, x_tap_timer: u8, facing: i8) -> i8 {
    if is_fresh_walk_dash_tap(x_tap_timer) || facts.ucf_dashback_direction != 0 {
        facts.forward_dash_direction(facing)
    } else {
        0
    }
}

fn fresh_walk_smash_turn_direction(facts: MeleeInputFacts, x_tap_timer: u8, facing: i8) -> i8 {
    if is_fresh_walk_dash_tap(x_tap_timer) || facts.ucf_dashback_direction != 0 {
        facts.smash_turn_direction(facing)
    } else {
        0
    }
}

fn is_fresh_walk_dash_tap(x_tap_timer: u8) -> bool {
    x_tap_timer < DASH_TAP_WINDOW
}

fn dash_action_state(facts: MeleeInputFacts, motion_frame: u8, facing: i8) -> Option<MotionState> {
    if facts.special_pressed && matches!(facts.special_direction, (1 | -1, 0)) {
        return Some(MotionState::SpecialS);
    }
    if facts.shield_held && facts.attack_pressed {
        return Some(MotionState::CatchDash);
    }
    if motion_frame < DASH_EARLY_ACTION_WINDOW && dash_early_side_smash_input(facts, facing) {
        return Some(MotionState::AttackS4);
    }
    if motion_frame < DASH_DEFENSIVE_ACTION_WINDOW && facts.shield_held {
        return Some(MotionState::EscapeF);
    }
    let in_late_dash_attack_window =
        (DASH_EARLY_ACTION_WINDOW..DASH_LATE_ACTION_WINDOW).contains(&motion_frame);
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
    facing: i8,
) -> Option<MotionState> {
    if facts.spot_dodge {
        return Some(MotionState::EscapeN);
    }

    if facts.roll_direction != 0 {
        return if facts.roll_direction == facing {
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

    facts.jump_pressed.then_some(MotionState::KneeBend)
}

fn guard_action_state(facts: MeleeInputFacts, facing: i8) -> Option<MotionState> {
    if facts.spot_dodge {
        return Some(MotionState::EscapeN);
    }

    if facts.roll_direction != 0 {
        return if facts.roll_direction == facing {
            Some(MotionState::EscapeF)
        } else {
            Some(MotionState::EscapeB)
        };
    }

    if facts.attack_pressed || facts.grab_pressed {
        return Some(MotionState::Catch);
    }

    facts.jump_pressed.then_some(MotionState::KneeBend)
}

fn guard_off_action_state(facts: MeleeInputFacts) -> Option<MotionState> {
    facts
        .spot_dodge
        .then_some(MotionState::EscapeN)
        .or_else(|| facts.jump_pressed.then_some(MotionState::KneeBend))
}

fn knee_bend_action_state(facts: MeleeInputFacts, stick_y: i8) -> Option<MotionState> {
    if facts.special_pressed && facts.special_direction == (0, 1) {
        return Some(MotionState::SpecialHi);
    }

    if facts.grab_pressed || (facts.shield_held && facts.attack_pressed) {
        return Some(MotionState::Catch);
    }

    let attack_up_smash = facts.attack_pressed && stick_y >= JUMP_CANCEL_UP_SMASH_Y;
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
