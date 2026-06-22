use crate::input::{MeleeInputConfig, MeleeInputThresholds};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonDataProvenance {
    ProvisionalMole,
    ExtractedPlCo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommonDataFieldSource {
    pub rust_name: &'static str,
    pub source_name: &'static str,
    pub offset: u16,
    pub provenance: CommonDataProvenance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonDataExtractError {
    TooShort {
        field: &'static str,
        offset: usize,
        required_len: usize,
        actual_len: usize,
    },
    NonFiniteFloat {
        field: &'static str,
        offset: usize,
    },
    OutOfRange {
        field: &'static str,
        offset: usize,
        value: i32,
        min: i32,
        max: i32,
    },
}

impl fmt::Display for CommonDataExtractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommonDataExtractError::TooShort {
                field,
                offset,
                required_len,
                actual_len,
            } => write!(
                f,
                "PlCo common data is too short for {field} at 0x{offset:x}: \
                 need {required_len} bytes, got {actual_len}"
            ),
            CommonDataExtractError::NonFiniteFloat { field, offset } => write!(
                f,
                "PlCo common data field {field} at 0x{offset:x} is not finite"
            ),
            CommonDataExtractError::OutOfRange {
                field,
                offset,
                value,
                min,
                max,
            } => write!(
                f,
                "PlCo common data field {field} at 0x{offset:x} produced {value}, \
                 outside {min}..={max}"
            ),
        }
    }
}

impl std::error::Error for CommonDataExtractError {}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeleeCommonData {
    pub tap_x_threshold: i8,
    pub tap_y_threshold: i8,
    pub trigger_threshold: u8,
    pub trigger_timer_threshold: u8,
    pub main_stick_deadzone_x: i8,
    pub main_stick_deadzone_y: i8,
    pub c_stick_deadzone_x: i8,
    pub c_stick_deadzone_y: i8,
    pub trigger_deadzone: u8,
    pub z_shield_analog: u8,
    pub walk_x: i8,
    pub walk_slow_x: i8,
    pub walk_middle_x: i8,
    pub walk_fast_x: i8,
    pub dash_x: i8,
    pub dash_tap_window: u8,
    pub turn_x: i8,
    pub turn_run_x: i8,
    pub tilt_x: i8,
    pub tilt_y: i8,
    pub throw_down_y: i8,
    pub smash_y: i8,
    pub crouch_y: i8,
    pub crouch_release_y: i8,
    pub tap_jump_y: i8,
    pub tap_jump_window: u8,
    pub tap_jump_release_y: i8,
    pub fast_fall_y: i8,
    pub fast_fall_window: u8,
    pub lcancel_window: u8,
    pub lcancel_divisor: f32,
    pub knockback_weight_multiplier: f32,
    pub knockback_decay: f32,
    pub knockback_cap: f32,
    pub knockback_damage_scale: f32,
    pub knockback_hit_count_scale: f32,
    pub knockback_weight_set_damage: f32,
    pub throw_knockback_weight: f32,
    pub knockback_result_scale: f32,
    pub knockback_result_offset: f32,
    pub stale_move_damage_reductions: [f32; 9],
    pub damage_knockback_velocity_scale: f32,
    pub damage_ground_knockback_friction_multiplier: f32,
    pub damage_knockback_frame_decay: f32,
    pub damage_sakurai_air_angle_radians: f32,
    pub damage_sakurai_ground_angle_degrees: f32,
    pub damage_sakurai_ground_min_knockback: f32,
    pub damage_sakurai_ground_max_knockback: f32,
    pub damage_duration_scale: f32,
    pub damage_motion_tier_1_threshold: f32,
    pub damage_motion_tier_2_threshold: f32,
    pub damage_motion_tier_3_threshold: f32,
    pub damage_fly_top_angle_min_radians: f32,
    pub damage_fly_top_angle_max_radians: f32,
    pub damage_fly_top_random_percent_threshold: u16,
    pub damage_fly_top_random_chance: f32,
    pub damage_landing_down_bound_knockback_threshold: f32,
    pub damage_landing_basic_knockback_threshold: f32,
    pub passive_input_age_threshold: u8,
    pub passive_window_max: f32,
    pub passive_stand_stick_x: f32,
    pub special_air_drift_stick_threshold: f32,
    pub down_stand_stick_y: i8,
    pub down_wait_timer: f32,
    pub hitlag_max_frames: f32,
    pub hitlag_damage_scale: f32,
    pub hitlag_base_frames: f32,
    pub hitlag_crouch_multiplier: f32,
    pub di_angle_degrees: f32,
    pub trigger_di_knockback_multiplier: f32,
    pub air_speed_clamp_friction: f32,
    pub c_stick: i8,
    pub aerial_neutral_x: i8,
    pub aerial_neutral_y: i8,
    pub aerial_vertical_angle_tan_milli: i32,
    pub air_jump_backward_x: i8,
    pub escape_x: i8,
    pub escape_x_tap_window: u8,
    pub escape_y: i8,
    pub escape_y_tap_window: u8,
    pub special_side_x: i8,
    pub special_vertical_y: i8,
    pub escapeair_iasa_timer_ticks: u8,
    pub escapeair_animation_ticks: u8,
    pub escapeair_deadzone_x: i8,
    pub escapeair_deadzone_y: i8,
    pub escapeair_force: f32,
    pub escapeair_decay: f32,
    pub escapeair_landing_lag_ticks: u8,
    pub throw_collision_lockout_ticks: u16,
    pub walk_middle_velocity_ratio: f32,
    pub walk_fast_velocity_ratio: f32,
    pub walk_accel_taper: f32,
    pub run_accel_taper: f32,
    pub run_ground_friction_multiplier: f32,
    pub catch_ground_friction_multiplier: f32,
    pub high_speed_ground_friction_multiplier: f32,
    pub run_brake_animation_pause_velocity: f32,
    pub animation_velocity_scale: f32,
    pub fall_animation_drift_threshold: f32,
    pub fall_animation_blend: f32,
    pub landing_wait_y_velocity_threshold: f32,
    pub player_nudge_x: f32,
    pub player_nudge_z: f32,
    pub player_nudge_z_clamp: f32,
    pub transformed_player_nudge_z: f32,
    pub transformed_player_nudge_z_clamp: f32,
    pub shield_start_health: f32,
    pub shield_release_lockout_frames: u8,
    pub shield_hold_drain: f32,
    pub shield_regen: f32,
    pub shield_break_reset_health: f32,
    pub shield_hit_drain_damage_scale: f32,
    pub shield_hit_drain_base: f32,
    pub shield_hit_lightshield_min: f32,
    pub shield_hit_lightshield_max: f32,
    pub shield_hold_lightshield_min: f32,
    pub shield_hold_lightshield_max: f32,
    pub fallspecial_platform_landing_y: i8,
    pub platform_pass_y: i8,
    pub platform_pass_y_tap_window: u8,
    pub pass_initial_y_velocity: f32,
    pub platform_drop_delay_ticks: u8,
    pub cliff_grab_block_stick_y: i8,
    pub cliff_quick_percent_threshold: u16,
    pub cliff_wait_low_percent_ticks: u16,
    pub cliff_wait_high_percent_ticks: u16,
    pub cliff_option_stick_threshold: i8,
    pub ledge_cooldown_ticks: u16,
    pub cliff_wait_hurt_intangible_ticks: u16,
    pub sdi_min_stick_mag: f32,
    pub sdi_stick_window: u8,
    pub sdi_pos_scale: f32,
    pub asdi_pos_scale: f32,
    pub rebirth_ticks: u8,
    pub rebirth_wait_ticks: u8,
    pub rebirth_hurt_intangible_ticks: u16,
    pub top_blast_fall_ko_chance: u8,
    pub dead_wait_ticks: u8,
    pub dead_up_star_wait_ticks: u8,
    pub dead_up_star_rise_ticks: u8,
    pub dead_up_star_exit_ticks: u8,
    pub dead_up_fall_wait_ticks: u8,
    pub dead_up_fall_anim_ticks: u8,
    pub dead_up_fall_hit_camera_ticks: u8,
    pub dead_up_fall_drift_ticks: u8,
    pub dead_up_fall_exit_ticks: u8,
    pub entry_start_ticks: u8,
    pub entry_end_ticks: u8,
    pub entry_initial_scale_y: f32,
    pub entry_collision_landing_lag_ticks: u8,
    pub dash_early_action_window: u8,
    pub dash_defensive_action_window: u8,
    pub dash_late_action_window: u8,
    pub dash_velocity_decay: f32,
    pub run_x: i8,
    pub guard_on_catch_dash_window: u8,
    pub guard_reflect_input_window: u8,
    pub run_turn_run_no_interrupt_frames: u8,
}

impl MeleeCommonData {
    pub const PROVISIONAL: Self = Self {
        tap_x_threshold: 32,
        tap_y_threshold: 32,
        trigger_threshold: 1,
        trigger_timer_threshold: 64,
        main_stick_deadzone_x: 36,
        main_stick_deadzone_y: 36,
        c_stick_deadzone_x: 36,
        c_stick_deadzone_y: 36,
        trigger_deadzone: 77,
        z_shield_analog: 89,
        walk_x: 23,
        walk_slow_x: 20,
        walk_middle_x: 50,
        walk_fast_x: 90,
        dash_x: 102,
        dash_tap_window: 2,
        turn_x: -32,
        turn_run_x: -48,
        tilt_x: 32,
        tilt_y: 32,
        throw_down_y: -32,
        smash_y: 80,
        crouch_y: 87,
        crouch_release_y: 79,
        tap_jump_y: 84,
        tap_jump_window: 4,
        tap_jump_release_y: 38,
        fast_fall_y: 84,
        fast_fall_window: 4,
        lcancel_window: 7,
        lcancel_divisor: 2.0,
        knockback_weight_multiplier: 0.009999999776482582,
        knockback_decay: 2.0,
        knockback_cap: 2500.0,
        knockback_damage_scale: 0.10000000149011612,
        knockback_hit_count_scale: 0.05000000074505806,
        knockback_weight_set_damage: 10.0,
        throw_knockback_weight: 100.0,
        knockback_result_scale: 1.399999976158142,
        knockback_result_offset: 18.0,
        stale_move_damage_reductions: [
            0.09000000357627869,
            0.07999999821186066,
            0.07000000029802322,
            0.05999999865889549,
            0.05000000074505806,
            0.03999999910593033,
            0.029999999329447746,
            0.019999999552965164,
            0.009999999776482582,
        ],
        damage_knockback_velocity_scale: 0.029999999329447746,
        damage_ground_knockback_friction_multiplier: 1.0,
        damage_knockback_frame_decay: 0.050999999046325684,
        damage_sakurai_air_angle_radians: 0.7853981852531433,
        damage_sakurai_ground_angle_degrees: 44.0,
        damage_sakurai_ground_min_knockback: 32.0,
        damage_sakurai_ground_max_knockback: 32.099998474121094,
        damage_duration_scale: 0.4000000059604645,
        damage_motion_tier_1_threshold: 10.0,
        damage_motion_tier_2_threshold: 21.0,
        damage_motion_tier_3_threshold: 32.0,
        damage_fly_top_angle_min_radians: 1.2217304706573486,
        damage_fly_top_angle_max_radians: 1.919862151145935,
        damage_fly_top_random_percent_threshold: 100,
        damage_fly_top_random_chance: 0.30000001192092896,
        damage_landing_down_bound_knockback_threshold: 5.0,
        damage_landing_basic_knockback_threshold: 0.5,
        passive_input_age_threshold: 40,
        passive_window_max: 20.0,
        passive_stand_stick_x: 0.20000000298023224,
        special_air_drift_stick_threshold: 0.10000000149011612,
        down_stand_stick_y: 25,
        down_wait_timer: 220.0,
        hitlag_max_frames: 20.0,
        hitlag_damage_scale: 0.3333333432674408,
        hitlag_base_frames: 3.0,
        hitlag_crouch_multiplier: 0.6666666865348816,
        di_angle_degrees: 18.0,
        trigger_di_knockback_multiplier: 1.0,
        air_speed_clamp_friction: 0.029999999329447746,
        c_stick: 40,
        aerial_neutral_x: 32,
        aerial_neutral_y: 32,
        aerial_vertical_angle_tan_milli: 1192,
        air_jump_backward_x: 16,
        escape_x: 89,
        escape_x_tap_window: 4,
        escape_y: -89,
        escape_y_tap_window: 4,
        special_side_x: 40,
        special_vertical_y: 40,
        escapeair_iasa_timer_ticks: 3,
        escapeair_animation_ticks: 50,
        escapeair_deadzone_x: 32,
        escapeair_deadzone_y: 32,
        escapeair_force: 3.0999999046325684,
        escapeair_decay: 0.8999999761581421,
        escapeair_landing_lag_ticks: 10,
        throw_collision_lockout_ticks: 8,
        walk_middle_velocity_ratio: 0.4000000059604645,
        walk_fast_velocity_ratio: 0.800000011920929,
        walk_accel_taper: 0.5,
        run_accel_taper: 0.4000000059604645,
        run_ground_friction_multiplier: 1.0,
        catch_ground_friction_multiplier: 1.0,
        high_speed_ground_friction_multiplier: 2.0,
        run_brake_animation_pause_velocity: 0.0,
        animation_velocity_scale: 1.2999999523162842,
        fall_animation_drift_threshold: 0.10000000149011612,
        fall_animation_blend: 0.5,
        landing_wait_y_velocity_threshold: 1.0,
        player_nudge_x: 0.30000001192092896,
        player_nudge_z: 0.10000000149011612,
        player_nudge_z_clamp: 1.399999976158142,
        transformed_player_nudge_z: 0.20000000298023224,
        transformed_player_nudge_z_clamp: 3.799999952316284,
        shield_start_health: 60.0,
        shield_release_lockout_frames: 8,
        shield_hold_drain: 0.14000000059604645,
        shield_regen: 0.07000000029802322,
        shield_break_reset_health: 30.0,
        shield_hit_drain_damage_scale: 1.0,
        shield_hit_drain_base: 0.0,
        shield_hit_lightshield_min: 0.10000000149011612,
        shield_hit_lightshield_max: 0.30000001192092896,
        shield_hold_lightshield_min: 0.10000000149011612,
        shield_hold_lightshield_max: 2.0,
        fallspecial_platform_landing_y: -71,
        platform_pass_y: 84,
        platform_pass_y_tap_window: 6,
        pass_initial_y_velocity: -0.5,
        platform_drop_delay_ticks: 2,
        cliff_grab_block_stick_y: 84,
        cliff_quick_percent_threshold: 100,
        cliff_wait_low_percent_ticks: 640,
        cliff_wait_high_percent_ticks: 480,
        cliff_option_stick_threshold: 32,
        ledge_cooldown_ticks: 30,
        cliff_wait_hurt_intangible_ticks: 30,
        sdi_min_stick_mag: 0.699999988079071,
        sdi_stick_window: 4,
        sdi_pos_scale: 6.0,
        asdi_pos_scale: 3.0,
        rebirth_ticks: 60,
        rebirth_wait_ticks: 240,
        rebirth_hurt_intangible_ticks: 120,
        top_blast_fall_ko_chance: 16,
        dead_wait_ticks: 60,
        dead_up_star_wait_ticks: 1,
        dead_up_star_rise_ticks: 130,
        dead_up_star_exit_ticks: 45,
        dead_up_fall_wait_ticks: 1,
        dead_up_fall_anim_ticks: 50,
        dead_up_fall_hit_camera_ticks: 3,
        dead_up_fall_drift_ticks: 40,
        dead_up_fall_exit_ticks: 35,
        entry_start_ticks: 30,
        entry_end_ticks: 30,
        entry_initial_scale_y: 0.009999999776482582,
        entry_collision_landing_lag_ticks: 120,
        dash_early_action_window: 4,
        dash_defensive_action_window: 3,
        dash_late_action_window: 20,
        dash_velocity_decay: 0.75,
        run_x: 79,
        guard_on_catch_dash_window: 3,
        guard_reflect_input_window: 2,
        run_turn_run_no_interrupt_frames: 10,
    };

    pub const fn provisional_mole() -> Self {
        Self::PROVISIONAL
    }

    pub fn from_plco_bytes(bytes: &[u8]) -> Result<Self, CommonDataExtractError> {
        let mut data = Self::PROVISIONAL;

        data.main_stick_deadzone_x = read_stick_i8(bytes, 0x00, "x0")?;
        data.main_stick_deadzone_y = read_stick_i8(bytes, 0x04, "x4")?;
        data.c_stick_deadzone_x = data.main_stick_deadzone_x;
        data.c_stick_deadzone_y = data.main_stick_deadzone_y;
        data.tap_x_threshold = read_stick_i8(bytes, 0x08, "x8_someStickThreshold")?;
        data.tap_y_threshold = read_stick_i8(bytes, 0x0c, "xC")?;
        data.trigger_deadzone = read_trigger_u8(bytes, 0x10, "x10")?;
        data.z_shield_analog = read_trigger_u8(bytes, 0x14, "x14")?;
        data.trigger_timer_threshold = read_trigger_u8(bytes, 0x18, "x18")?;
        data.passive_input_age_threshold = read_u8_from_i32(bytes, 0x1c, "x1C")?;
        data.aerial_vertical_angle_tan_milli =
            read_radian_tangent_milli(bytes, 0x20, "x20_radians")?;
        data.walk_x = read_stick_i8(bytes, 0x24, "x24")?;
        data.walk_middle_velocity_ratio = read_f32(bytes, 0x28, "x28")?;
        data.walk_fast_velocity_ratio = read_f32(bytes, 0x2c, "x2C")?;
        data.walk_accel_taper = read_f32(bytes, 0x30, "x30")?;
        data.turn_x = read_stick_i8(bytes, 0x34, "x34")?;
        data.turn_run_x = read_stick_i8(bytes, 0x38, "x38_someLStickXThreshold")?;
        data.dash_x = read_stick_i8(bytes, 0x3c, "x3C")?;
        data.dash_tap_window = read_u8_from_i32(bytes, 0x40, "x40")?;
        data.dash_early_action_window = read_u8_from_f32(bytes, 0x44, "x44")?;
        data.dash_defensive_action_window = read_u8_from_f32(bytes, 0x48, "x48")?;
        data.dash_late_action_window = read_u8_from_f32(bytes, 0x4c, "x4C")?;
        data.dash_velocity_decay = read_f32(bytes, 0x54, "x54")?;
        data.run_x = read_stick_i8(bytes, 0x58, "x58_someLStickXThreshold")?;
        data.run_accel_taper = read_f32(bytes, 0x5c, "x5C")?;
        data.run_ground_friction_multiplier = read_f32(bytes, 0x60, "x60_someFrictionMul")?;
        data.catch_ground_friction_multiplier = read_f32(bytes, 0x64, "x64")?;
        data.guard_on_catch_dash_window = read_u8_from_f32(bytes, 0x68, "x68")?;
        data.high_speed_ground_friction_multiplier = read_f32(bytes, 0x6c, "x6C")?;
        data.tap_jump_y = read_stick_i8(bytes, 0x70, "tap_jump_threshold")?;
        data.tap_jump_window = read_u8_from_i32(bytes, 0x74, "x74")?;
        data.air_jump_backward_x = read_stick_i8(bytes, 0x78, "x78")?;
        data.tap_jump_release_y = read_stick_i8(bytes, 0x7c, "tap_jump_release_threshold")?;
        data.fast_fall_y = read_stick_i8(bytes, 0x88, "x88")?;
        data.fast_fall_window = read_u8_from_i32(bytes, 0x8c, "x8C")?;
        data.crouch_y = read_stick_i8(bytes, 0x90, "x90")?;
        data.crouch_release_y = read_stick_i8(bytes, 0x94, "x94")?;
        data.tilt_x = read_stick_i8(bytes, 0x98, "x98")?;
        data.tilt_y = read_stick_i8(bytes, 0xac, "attackhi3_stick_threshold_y")?;
        data.throw_down_y = read_stick_i8(bytes, 0xb0, "xB0")?;
        data.aerial_neutral_x = read_stick_i8(bytes, 0xdc, "xDC")?;
        data.aerial_neutral_y = read_stick_i8(bytes, 0xe0, "xE0")?;
        data.lcancel_window = read_u8_from_i32(bytes, 0xe4, "xE4")?;
        data.lcancel_divisor = read_f32(bytes, 0xe8, "xE8")?;
        data.knockback_weight_multiplier = read_f32(bytes, 0xf4, "xF4")?;
        data.knockback_decay = read_f32(bytes, 0xf8, "xF8")?;
        data.knockback_cap = read_f32(bytes, 0x108, "x108")?;
        data.throw_knockback_weight = read_f32(bytes, 0x10c, "x10C")?;
        data.knockback_damage_scale = read_f32(bytes, 0x110, "x110")?;
        data.knockback_hit_count_scale = read_f32(bytes, 0x114, "x114")?;
        data.knockback_weight_set_damage = read_f32(bytes, 0x118, "x118")?;
        data.knockback_result_scale = read_f32(bytes, 0x11c, "x11C")?;
        data.knockback_result_offset = read_f32(bytes, 0x120, "x120")?;
        data.damage_knockback_velocity_scale = read_f32(bytes, 0x100, "x100")?;
        data.damage_ground_knockback_friction_multiplier = read_f32(bytes, 0x200, "x200")?;
        data.damage_knockback_frame_decay = read_f32(bytes, 0x204, "x204_knockbackFrameDecay")?;
        data.damage_sakurai_air_angle_radians = read_f32(bytes, 0x144, "x144_radians")?;
        data.damage_sakurai_ground_angle_degrees = read_f32(bytes, 0x148, "x148")?;
        data.damage_sakurai_ground_min_knockback = read_f32(bytes, 0x14c, "x14C")?;
        data.damage_sakurai_ground_max_knockback = read_f32(bytes, 0x150, "x150")?;
        data.damage_duration_scale = read_f32(bytes, 0x154, "x154")?;
        data.damage_motion_tier_1_threshold = read_f32(bytes, 0x158, "x158")?;
        data.damage_motion_tier_2_threshold = read_f32(bytes, 0x15c, "x15C")?;
        data.damage_motion_tier_3_threshold = read_f32(bytes, 0x160, "x160")?;
        data.damage_fly_top_angle_min_radians = read_f32(bytes, 0x234, "x234")?;
        data.damage_fly_top_angle_max_radians = read_f32(bytes, 0x238, "x238")?;
        data.damage_fly_top_random_percent_threshold = read_u16_from_i32(bytes, 0x23c, "x23C")?;
        data.damage_fly_top_random_chance = read_f32(bytes, 0x240, "x240")?;
        data.hitlag_max_frames = read_f32(bytes, 0x194, "x194_unkHitLagFrames")?;
        data.hitlag_damage_scale = read_f32(bytes, 0x198, "x198")?;
        data.hitlag_base_frames = read_f32(bytes, 0x19c, "x19C")?;
        data.hitlag_crouch_multiplier = read_f32(bytes, 0x1a0, "x1A0")?;
        data.di_angle_degrees = read_f32(bytes, 0x1a8, "x1A8")?;
        data.trigger_di_knockback_multiplier = read_f32(bytes, 0x1ac, "x1AC")?;
        data.air_speed_clamp_friction = read_f32(bytes, 0x1fc, "x1FC")?;
        data.damage_landing_down_bound_knockback_threshold = read_f32(bytes, 0x1e0, "x1E0")?;
        data.damage_landing_basic_knockback_threshold = read_f32(bytes, 0x1e4, "x1E4")?;
        data.down_stand_stick_y = read_stick_i8(bytes, 0x244, "x244")?;
        data.passive_window_max = read_f32(bytes, 0x250, "x250")?;
        data.passive_stand_stick_x = read_f32(bytes, 0x254, "x254")?;
        data.special_air_drift_stick_threshold = read_f32(bytes, 0x258, "x258")?;
        data.fallspecial_platform_landing_y = read_stick_i8(bytes, 0x25c, "x25C")?;
        data.guard_reflect_input_window = read_u8_from_i32(bytes, 0x2a0, "x2A0")?;
        data.escape_y = read_stick_i8(bytes, 0x314, "x314")?;
        data.escape_y_tap_window = read_u8_from_i32(bytes, 0x318, "x318")?;
        data.escape_x = read_stick_i8(bytes, 0x31c, "x31C")?;
        data.escape_x_tap_window = read_u8_from_i32(bytes, 0x320, "x320")?;
        data.landing_wait_y_velocity_threshold = read_f32(bytes, 0x310, "x310")?;

        data.escapeair_deadzone_x = read_stick_i8(bytes, 0x32c, "escapeair_deadzone.x")?;
        data.escapeair_deadzone_y = read_stick_i8(bytes, 0x330, "escapeair_deadzone.y")?;

        data.escapeair_iasa_timer_ticks = read_u8_from_i32(bytes, 0x334, "x334")?;
        data.escapeair_force = read_f32(bytes, 0x338, "escapeair_force")?;
        data.escapeair_decay = read_f32(bytes, 0x33c, "escapeair_decay")?;
        data.escapeair_landing_lag_ticks = read_u8_from_f32(bytes, 0x344, "x344")?;
        data.throw_collision_lockout_ticks = read_u16_from_i32(bytes, 0x348, "x348")?;
        data.down_wait_timer = read_f32(bytes, 0x424, "x424")?;
        data.run_brake_animation_pause_velocity = read_f32(bytes, 0x42c, "x42C")?;
        data.run_turn_run_no_interrupt_frames = read_u8_from_f32(bytes, 0x430, "x430")?;
        data.animation_velocity_scale = read_f32(bytes, 0x440, "x440")?;
        data.fall_animation_drift_threshold = read_f32(bytes, 0x444, "x444")?;
        data.fall_animation_blend = read_f32(bytes, 0x448, "x448")?;
        data.player_nudge_x = read_f32(bytes, 0x450, "x450")?;
        data.player_nudge_z = read_f32(bytes, 0x454, "x454")?;
        data.player_nudge_z_clamp = read_f32(bytes, 0x458, "x458")?;
        data.transformed_player_nudge_z = read_f32(bytes, 0x45c, "x45C")?;
        data.transformed_player_nudge_z_clamp = read_f32(bytes, 0x460, "x460")?;
        data.shield_start_health = read_f32(bytes, 0x260, "x260_startShieldHealth")?;
        data.shield_release_lockout_frames = read_u8_from_f32(bytes, 0x268, "x268")?;
        data.shield_hold_drain = read_f32(bytes, 0x278, "x278")?;
        data.shield_regen = read_f32(bytes, 0x27c, "x27C")?;
        data.shield_break_reset_health = read_f32(bytes, 0x280, "x280_unkShieldHealth")?;
        data.shield_hit_drain_damage_scale = read_f32(bytes, 0x284, "x284")?;
        data.shield_hit_drain_base = read_f32(bytes, 0x288, "x288")?;
        data.shield_hit_lightshield_min = read_f32(bytes, 0x2dc, "x2DC")?;
        data.shield_hit_lightshield_max = read_f32(bytes, 0x2e0, "x2E0")?;
        data.shield_hold_lightshield_min = read_f32(bytes, 0x2ec, "x2EC")?;
        data.shield_hold_lightshield_max = read_f32(bytes, 0x2f0, "x2F0")?;
        data.platform_pass_y = read_stick_i8(bytes, 0x464, "x464")?;
        data.platform_pass_y_tap_window = read_u8_from_f32(bytes, 0x468, "x468")?;
        data.pass_initial_y_velocity = read_f32(bytes, 0x46c, "x46C")?;
        data.platform_drop_delay_ticks = read_u8_from_f32(bytes, 0x470, "x470")?;
        data.cliff_grab_block_stick_y = read_stick_i8(bytes, 0x480, "x480")?;
        data.cliff_quick_percent_threshold = read_u16_from_i32(bytes, 0x488, "x488")?;
        data.cliff_wait_low_percent_ticks = read_u16_from_f32(bytes, 0x48c, "x48C")?;
        data.cliff_wait_high_percent_ticks = read_u16_from_f32(bytes, 0x490, "x490")?;
        data.cliff_option_stick_threshold = read_stick_i8(bytes, 0x494, "x494")?;
        data.ledge_cooldown_ticks = read_u16_from_i32(bytes, 0x498, "ledge_cooldown")?;
        data.cliff_wait_hurt_intangible_ticks = read_u16_from_i32(bytes, 0x49c, "x49C")?;
        data.sdi_min_stick_mag = read_f32(bytes, 0x4b0, "sdi_min_stick_mag")?;
        data.sdi_stick_window = read_u8_from_i32(bytes, 0x4b4, "sdi_stick_window")?;
        data.sdi_pos_scale = read_f32(bytes, 0x4b8, "sdi_pos_scale")?;
        data.asdi_pos_scale = read_f32(bytes, 0x4bc, "x4BC")?;
        data.rebirth_ticks = read_u8_from_i32(bytes, 0x5d0, "x5D0")?;
        data.rebirth_wait_ticks = read_u8_from_i32(bytes, 0x5d4, "x5D4")?;
        data.rebirth_hurt_intangible_ticks = read_u16_from_i32(bytes, 0x5d8, "x5D8")?;
        data.top_blast_fall_ko_chance = read_u8_from_i32(bytes, 0x520, "x520")?;
        data.dead_wait_ticks = read_u8_from_i32(bytes, 0x500, "x500")?;
        data.dead_up_star_wait_ticks = read_u8_from_i32(bytes, 0x504, "x504")?;
        data.dead_up_star_rise_ticks = read_u8_from_i32(bytes, 0x508, "x508")?;
        data.dead_up_star_exit_ticks = read_u8_from_i32(bytes, 0x50c, "x50C")?;
        data.dead_up_fall_wait_ticks = read_u8_from_i32(bytes, 0x524, "x524")?;
        data.dead_up_fall_anim_ticks = read_u8_from_i32(bytes, 0x528, "x528")?;
        data.dead_up_fall_hit_camera_ticks = read_u8_from_i32(bytes, 0x52c, "x52C")?;
        data.dead_up_fall_drift_ticks = read_u8_from_i32(bytes, 0x530, "x530")?;
        data.dead_up_fall_exit_ticks = read_u8_from_i32(bytes, 0x534, "x534")?;
        data.entry_start_ticks = read_u8_from_i32(bytes, 0x6bc, "x6BC")?;
        data.entry_end_ticks = read_u8_from_i32(bytes, 0x6c0, "x6C0")?;
        data.entry_initial_scale_y = read_f32(bytes, 0x6c4, "x6C4")?;
        data.entry_collision_landing_lag_ticks = read_u8_from_i32(bytes, 0x6c8, "x6C8")?;

        Ok(data)
    }

    pub const fn input_config(self) -> MeleeInputConfig {
        MeleeInputConfig {
            tap_x_threshold: self.tap_x_threshold,
            tap_y_threshold: self.tap_y_threshold,
            trigger_threshold: self.trigger_threshold,
            trigger_timer_threshold: self.trigger_timer_threshold,
            main_stick_deadzone_x: self.main_stick_deadzone_x,
            main_stick_deadzone_y: self.main_stick_deadzone_y,
            c_stick_deadzone_x: self.c_stick_deadzone_x,
            c_stick_deadzone_y: self.c_stick_deadzone_y,
            trigger_deadzone: self.trigger_deadzone,
        }
    }

    pub const fn input_thresholds(self) -> MeleeInputThresholds {
        MeleeInputThresholds {
            walk_x: self.walk_x,
            walk_slow_x: self.walk_slow_x,
            walk_middle_x: self.walk_middle_x,
            walk_fast_x: self.walk_fast_x,
            dash_x: self.dash_x,
            dash_tap_window: self.dash_tap_window,
            turn_x: self.turn_x,
            tilt_x: self.tilt_x,
            tilt_y: self.tilt_y,
            smash_y: self.smash_y,
            crouch_y: self.crouch_y,
            tap_jump_y: self.tap_jump_y,
            tap_jump_window: self.tap_jump_window,
            tap_jump_release_y: self.tap_jump_release_y,
            fast_fall_y: self.fast_fall_y,
            fast_fall_window: self.fast_fall_window,
            c_stick: self.c_stick,
            aerial_neutral_x: self.aerial_neutral_x,
            aerial_neutral_y: self.aerial_neutral_y,
            aerial_vertical_angle_tan_milli: self.aerial_vertical_angle_tan_milli,
            z_shield_analog: self.z_shield_analog,
            escape_x: self.escape_x,
            escape_x_tap_window: self.escape_x_tap_window,
            escape_y: self.escape_y,
            escape_y_tap_window: self.escape_y_tap_window,
            special_side_x: self.special_side_x,
            special_vertical_y: self.special_vertical_y,
        }
    }
}

fn read_bytes<const LEN: usize>(
    bytes: &[u8],
    offset: usize,
    field: &'static str,
) -> Result<[u8; LEN], CommonDataExtractError> {
    let required_len = offset + LEN;
    let Some(slice) = bytes.get(offset..required_len) else {
        return Err(CommonDataExtractError::TooShort {
            field,
            offset,
            required_len,
            actual_len: bytes.len(),
        });
    };

    let mut result = [0_u8; LEN];
    result.copy_from_slice(slice);
    Ok(result)
}

fn read_f32(
    bytes: &[u8],
    offset: usize,
    field: &'static str,
) -> Result<f32, CommonDataExtractError> {
    let value = f32::from_be_bytes(read_bytes(bytes, offset, field)?);
    if !value.is_finite() {
        return Err(CommonDataExtractError::NonFiniteFloat { field, offset });
    }
    Ok(value)
}

fn read_i32(
    bytes: &[u8],
    offset: usize,
    field: &'static str,
) -> Result<i32, CommonDataExtractError> {
    Ok(i32::from_be_bytes(read_bytes(bytes, offset, field)?))
}

fn read_stick_i8(
    bytes: &[u8],
    offset: usize,
    field: &'static str,
) -> Result<i8, CommonDataExtractError> {
    let value = round_f32_to_i32(read_f32(bytes, offset, field)? * 127.0, field, offset)?;
    range_i32(value, -127, 127, field, offset).map(|value| value as i8)
}

fn read_trigger_u8(
    bytes: &[u8],
    offset: usize,
    field: &'static str,
) -> Result<u8, CommonDataExtractError> {
    let value = round_f32_to_i32(read_f32(bytes, offset, field)? * 255.0, field, offset)?;
    range_i32(value, 0, u8::MAX as i32, field, offset).map(|value| value as u8)
}

fn read_radian_tangent_milli(
    bytes: &[u8],
    offset: usize,
    field: &'static str,
) -> Result<i32, CommonDataExtractError> {
    let tangent = read_f32(bytes, offset, field)?.tan();
    if !tangent.is_finite() {
        return Err(CommonDataExtractError::NonFiniteFloat { field, offset });
    }
    round_f32_to_i32(tangent * 1000.0, field, offset)
}

fn read_u8_from_i32(
    bytes: &[u8],
    offset: usize,
    field: &'static str,
) -> Result<u8, CommonDataExtractError> {
    let value = read_i32(bytes, offset, field)?;
    range_i32(value, 0, u8::MAX as i32, field, offset).map(|value| value as u8)
}

fn read_u16_from_i32(
    bytes: &[u8],
    offset: usize,
    field: &'static str,
) -> Result<u16, CommonDataExtractError> {
    let value = read_i32(bytes, offset, field)?;
    range_i32(value, 0, u16::MAX as i32, field, offset).map(|value| value as u16)
}

fn read_u8_from_f32(
    bytes: &[u8],
    offset: usize,
    field: &'static str,
) -> Result<u8, CommonDataExtractError> {
    let value = round_f32_to_i32(read_f32(bytes, offset, field)?, field, offset)?;
    range_i32(value, 0, u8::MAX as i32, field, offset).map(|value| value as u8)
}

fn read_u16_from_f32(
    bytes: &[u8],
    offset: usize,
    field: &'static str,
) -> Result<u16, CommonDataExtractError> {
    let value = round_f32_to_i32(read_f32(bytes, offset, field)?, field, offset)?;
    range_i32(value, 0, u16::MAX as i32, field, offset).map(|value| value as u16)
}

fn round_f32_to_i32(
    value: f32,
    field: &'static str,
    offset: usize,
) -> Result<i32, CommonDataExtractError> {
    if !value.is_finite() {
        return Err(CommonDataExtractError::NonFiniteFloat { field, offset });
    }

    let rounded = value.round();
    if rounded < i32::MIN as f32 || rounded > i32::MAX as f32 {
        return Err(CommonDataExtractError::OutOfRange {
            field,
            offset,
            value: if rounded.is_sign_negative() {
                i32::MIN
            } else {
                i32::MAX
            },
            min: i32::MIN,
            max: i32::MAX,
        });
    }

    Ok(rounded as i32)
}

fn range_i32(
    value: i32,
    min: i32,
    max: i32,
    field: &'static str,
    offset: usize,
) -> Result<i32, CommonDataExtractError> {
    if value < min || value > max {
        return Err(CommonDataExtractError::OutOfRange {
            field,
            offset,
            value,
            min,
            max,
        });
    }
    Ok(value)
}

pub const INPUT_COMMON_DATA_FIELD_SOURCES: &[CommonDataFieldSource] = &[
    CommonDataFieldSource {
        rust_name: "main_stick_deadzone_x",
        source_name: "x0",
        offset: 0x00,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "main_stick_deadzone_y",
        source_name: "x4",
        offset: 0x04,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "c_stick_deadzone_x",
        source_name: "x0",
        offset: 0x00,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "c_stick_deadzone_y",
        source_name: "x4",
        offset: 0x04,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "tap_x_threshold",
        source_name: "x8_someStickThreshold",
        offset: 0x08,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "tap_y_threshold",
        source_name: "xC_someStickThreshold",
        offset: 0x0c,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "trigger_deadzone",
        source_name: "x10_trigger_deadzone",
        offset: 0x10,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "z_shield_analog",
        source_name: "x14",
        offset: 0x14,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "trigger_timer_threshold",
        source_name: "x18",
        offset: 0x18,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "passive_input_age_threshold",
        source_name: "x1C",
        offset: 0x1c,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "aerial_vertical_angle_tan_milli",
        source_name: "x20_radians",
        offset: 0x20,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "walk_x",
        source_name: "x24",
        offset: 0x24,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "walk_slow_x",
        source_name: "provisional_walk_slow_x",
        offset: 0,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "walk_middle_x",
        source_name: "provisional_walk_middle_x",
        offset: 0,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "walk_fast_x",
        source_name: "provisional_walk_fast_x",
        offset: 0,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "walk_middle_velocity_ratio",
        source_name: "x28",
        offset: 0x28,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "walk_fast_velocity_ratio",
        source_name: "x2C",
        offset: 0x2c,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "walk_accel_taper",
        source_name: "x30",
        offset: 0x30,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "turn_x",
        source_name: "x34",
        offset: 0x34,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "turn_run_x",
        source_name: "x38_someLStickXThreshold",
        offset: 0x38,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "dash_x",
        source_name: "x3C",
        offset: 0x3c,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "dash_tap_window",
        source_name: "x40",
        offset: 0x40,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "dash_early_action_window",
        source_name: "x44",
        offset: 0x44,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "dash_defensive_action_window",
        source_name: "x48",
        offset: 0x48,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "dash_late_action_window",
        source_name: "x4C",
        offset: 0x4c,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "dash_velocity_decay",
        source_name: "x54",
        offset: 0x54,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "run_x",
        source_name: "x58_someLStickXThreshold",
        offset: 0x58,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "run_accel_taper",
        source_name: "x5C",
        offset: 0x5c,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "run_ground_friction_multiplier",
        source_name: "x60_someFrictionMul",
        offset: 0x60,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "catch_ground_friction_multiplier",
        source_name: "x64",
        offset: 0x64,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "guard_on_catch_dash_window",
        source_name: "x68",
        offset: 0x68,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "high_speed_ground_friction_multiplier",
        source_name: "x6C",
        offset: 0x6c,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "guard_reflect_input_window",
        source_name: "x2A0",
        offset: 0x2a0,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "throw_collision_lockout_ticks",
        source_name: "x348",
        offset: 0x348,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "down_wait_timer",
        source_name: "x424",
        offset: 0x424,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "run_turn_run_no_interrupt_frames",
        source_name: "x430",
        offset: 0x430,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "run_brake_animation_pause_velocity",
        source_name: "x42C",
        offset: 0x42c,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "animation_velocity_scale",
        source_name: "x440",
        offset: 0x440,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "fall_animation_drift_threshold",
        source_name: "x444",
        offset: 0x444,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "fall_animation_blend",
        source_name: "x448",
        offset: 0x448,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "player_nudge_x",
        source_name: "x450",
        offset: 0x450,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "player_nudge_z",
        source_name: "x454",
        offset: 0x454,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "player_nudge_z_clamp",
        source_name: "x458",
        offset: 0x458,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "transformed_player_nudge_z",
        source_name: "x45C",
        offset: 0x45c,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "transformed_player_nudge_z_clamp",
        source_name: "x460",
        offset: 0x460,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "tap_jump_y",
        source_name: "tap_jump_threshold",
        offset: 0x70,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "tap_jump_window",
        source_name: "x74",
        offset: 0x74,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "tap_jump_release_y",
        source_name: "tap_jump_release_threshold",
        offset: 0x7c,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "air_jump_backward_x",
        source_name: "x78",
        offset: 0x78,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "fast_fall_y",
        source_name: "x88",
        offset: 0x88,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "fast_fall_window",
        source_name: "x8C",
        offset: 0x8c,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "crouch_y",
        source_name: "x90",
        offset: 0x90,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "crouch_release_y",
        source_name: "x94",
        offset: 0x94,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "tilt_x",
        source_name: "x98",
        offset: 0x98,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "tilt_y",
        source_name: "attackhi3_stick_threshold_y",
        offset: 0xac,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "throw_down_y",
        source_name: "xB0",
        offset: 0xb0,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "aerial_neutral_x",
        source_name: "xDC",
        offset: 0xdc,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "aerial_neutral_y",
        source_name: "xE0",
        offset: 0xe0,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "lcancel_window",
        source_name: "xE4",
        offset: 0xe4,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "lcancel_divisor",
        source_name: "xE8",
        offset: 0xe8,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "knockback_weight_multiplier",
        source_name: "xF4",
        offset: 0xf4,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "knockback_decay",
        source_name: "xF8",
        offset: 0xf8,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "damage_knockback_velocity_scale",
        source_name: "x100",
        offset: 0x100,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "damage_ground_knockback_friction_multiplier",
        source_name: "x200",
        offset: 0x200,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "damage_knockback_frame_decay",
        source_name: "x204_knockbackFrameDecay",
        offset: 0x204,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "knockback_cap",
        source_name: "x108",
        offset: 0x108,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "throw_knockback_weight",
        source_name: "x10C",
        offset: 0x10c,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "knockback_damage_scale",
        source_name: "x110",
        offset: 0x110,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "knockback_hit_count_scale",
        source_name: "x114",
        offset: 0x114,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "knockback_weight_set_damage",
        source_name: "x118",
        offset: 0x118,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "knockback_result_scale",
        source_name: "x11C",
        offset: 0x11c,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "knockback_result_offset",
        source_name: "x120",
        offset: 0x120,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "damage_sakurai_air_angle_radians",
        source_name: "x144_radians",
        offset: 0x144,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "damage_sakurai_ground_angle_degrees",
        source_name: "x148",
        offset: 0x148,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "damage_sakurai_ground_min_knockback",
        source_name: "x14C",
        offset: 0x14c,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "damage_sakurai_ground_max_knockback",
        source_name: "x150",
        offset: 0x150,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "damage_duration_scale",
        source_name: "x154",
        offset: 0x154,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "damage_motion_tier_1_threshold",
        source_name: "x158",
        offset: 0x158,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "damage_motion_tier_2_threshold",
        source_name: "x15C",
        offset: 0x15c,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "damage_motion_tier_3_threshold",
        source_name: "x160",
        offset: 0x160,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "damage_fly_top_angle_min_radians",
        source_name: "x234",
        offset: 0x234,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "damage_fly_top_angle_max_radians",
        source_name: "x238",
        offset: 0x238,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "damage_fly_top_random_percent_threshold",
        source_name: "x23C",
        offset: 0x23c,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "damage_fly_top_random_chance",
        source_name: "x240",
        offset: 0x240,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "hitlag_max_frames",
        source_name: "x194_unkHitLagFrames",
        offset: 0x194,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "hitlag_damage_scale",
        source_name: "x198",
        offset: 0x198,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "hitlag_base_frames",
        source_name: "x19C",
        offset: 0x19c,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "hitlag_crouch_multiplier",
        source_name: "x1A0",
        offset: 0x1a0,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "di_angle_degrees",
        source_name: "x1A8",
        offset: 0x1a8,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "trigger_di_knockback_multiplier",
        source_name: "x1AC",
        offset: 0x1ac,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "air_speed_clamp_friction",
        source_name: "x1FC",
        offset: 0x1fc,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "damage_landing_down_bound_knockback_threshold",
        source_name: "x1E0",
        offset: 0x1e0,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "damage_landing_basic_knockback_threshold",
        source_name: "x1E4",
        offset: 0x1e4,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "down_stand_stick_y",
        source_name: "x244",
        offset: 0x244,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "passive_window_max",
        source_name: "x250",
        offset: 0x250,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "passive_stand_stick_x",
        source_name: "x254",
        offset: 0x254,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "special_air_drift_stick_threshold",
        source_name: "x258",
        offset: 0x258,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "shield_start_health",
        source_name: "x260_startShieldHealth",
        offset: 0x260,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "shield_release_lockout_frames",
        source_name: "x268",
        offset: 0x268,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "shield_hold_drain",
        source_name: "x278",
        offset: 0x278,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "shield_regen",
        source_name: "x27C",
        offset: 0x27c,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "shield_break_reset_health",
        source_name: "x280_unkShieldHealth",
        offset: 0x280,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "shield_hit_drain_damage_scale",
        source_name: "x284",
        offset: 0x284,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "shield_hit_drain_base",
        source_name: "x288",
        offset: 0x288,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "shield_hit_lightshield_min",
        source_name: "x2DC",
        offset: 0x2dc,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "shield_hit_lightshield_max",
        source_name: "x2E0",
        offset: 0x2e0,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "shield_hold_lightshield_min",
        source_name: "x2EC",
        offset: 0x2ec,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "shield_hold_lightshield_max",
        source_name: "x2F0",
        offset: 0x2f0,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "escape_y",
        source_name: "x314",
        offset: 0x314,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "escape_y_tap_window",
        source_name: "x318",
        offset: 0x318,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "escape_x",
        source_name: "x31C",
        offset: 0x31c,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "escape_x_tap_window",
        source_name: "x320",
        offset: 0x320,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "landing_wait_y_velocity_threshold",
        source_name: "x310",
        offset: 0x310,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "escape_x_cstick",
        source_name: "x324",
        offset: 0x324,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "escapeair_deadzone_x",
        source_name: "escapeair_deadzone.x",
        offset: 0x32c,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "escapeair_deadzone_y",
        source_name: "escapeair_deadzone.y",
        offset: 0x330,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "escapeair_iasa_timer_ticks",
        source_name: "x334",
        offset: 0x334,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "escapeair_force",
        source_name: "escapeair_force",
        offset: 0x338,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "escapeair_decay",
        source_name: "escapeair_decay",
        offset: 0x33c,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "fallspecial_mobility",
        source_name: "x340",
        offset: 0x340,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "fallspecial_platform_landing_y",
        source_name: "x25C",
        offset: 0x25c,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "escapeair_landing_lag",
        source_name: "x344",
        offset: 0x344,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "platform_pass_y",
        source_name: "x464",
        offset: 0x464,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "platform_pass_y_tap_window",
        source_name: "x468",
        offset: 0x468,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "pass_initial_y_velocity",
        source_name: "x46C",
        offset: 0x46c,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "platform_drop_delay_ticks",
        source_name: "x470",
        offset: 0x470,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "cliff_grab_block_stick_y",
        source_name: "x480",
        offset: 0x480,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "cliff_quick_percent_threshold",
        source_name: "x488",
        offset: 0x488,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "cliff_wait_low_percent_ticks",
        source_name: "x48C",
        offset: 0x48c,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "cliff_wait_high_percent_ticks",
        source_name: "x490",
        offset: 0x490,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "cliff_option_stick_threshold",
        source_name: "x494",
        offset: 0x494,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "ledge_cooldown_ticks",
        source_name: "ledge_cooldown",
        offset: 0x498,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "cliff_wait_hurt_intangible_ticks",
        source_name: "x49C",
        offset: 0x49c,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "sdi_min_stick_mag",
        source_name: "sdi_min_stick_mag",
        offset: 0x4b0,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "sdi_stick_window",
        source_name: "sdi_stick_window",
        offset: 0x4b4,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "sdi_pos_scale",
        source_name: "sdi_pos_scale",
        offset: 0x4b8,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "asdi_pos_scale",
        source_name: "x4BC",
        offset: 0x4bc,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "rebirth_ticks",
        source_name: "x5D0",
        offset: 0x5d0,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "rebirth_wait_ticks",
        source_name: "x5D4",
        offset: 0x5d4,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "rebirth_hurt_intangible_ticks",
        source_name: "x5D8",
        offset: 0x5d8,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "dead_wait_ticks",
        source_name: "x500",
        offset: 0x500,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "dead_up_star_wait_ticks",
        source_name: "x504",
        offset: 0x504,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "dead_up_star_rise_ticks",
        source_name: "x508",
        offset: 0x508,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "dead_up_star_exit_ticks",
        source_name: "x50C",
        offset: 0x50c,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "top_blast_fall_ko_chance",
        source_name: "x520",
        offset: 0x520,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "dead_up_fall_wait_ticks",
        source_name: "x524",
        offset: 0x524,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "dead_up_fall_anim_ticks",
        source_name: "x528",
        offset: 0x528,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "dead_up_fall_hit_camera_ticks",
        source_name: "x52C",
        offset: 0x52c,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "dead_up_fall_drift_ticks",
        source_name: "x530",
        offset: 0x530,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "dead_up_fall_exit_ticks",
        source_name: "x534",
        offset: 0x534,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "entry_start_ticks",
        source_name: "x6BC",
        offset: 0x6bc,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "entry_end_ticks",
        source_name: "x6C0",
        offset: 0x6c0,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "entry_initial_scale_y",
        source_name: "x6C4",
        offset: 0x6c4,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
    CommonDataFieldSource {
        rust_name: "entry_collision_landing_lag_ticks",
        source_name: "x6C8",
        offset: 0x6c8,
        provenance: CommonDataProvenance::ExtractedPlCo,
    },
];

pub const fn input_common_data_field_sources() -> &'static [CommonDataFieldSource] {
    INPUT_COMMON_DATA_FIELD_SOURCES
}
