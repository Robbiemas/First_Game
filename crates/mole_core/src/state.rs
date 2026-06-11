use crate::{
    collision::{
        source_damage_accumulator_after_stages, source_damage_result_for_victim,
        source_damage_stages_from_confirms, source_hit_confirms, EcbDiamond,
        SourceCollisionCapsule, SourceCollisionFrame, SourceDamageAccumulator, SourceDamageResult,
        SourceDamageResultInput, SourceDamageStage, SourceHitConfirm, SourceHitboxAttributes,
        SourceHitboxLifecycleId,
    },
    stage::{StageProfile, StageSurface, StageSurfaceKind},
    time::Frame,
    units::{milli_to_source_units, source_units_to_milli, MELEE_UNIT_SCALE},
    MeleeCommonData, MeleeInputFacts, MeleeInputSnapshot, MeleeInputTimers, MeleeJumpInput,
    PlayerInput,
};
use std::fmt;

#[path = "generated/falcon_ecb.rs"]
mod falcon_ecb;
#[path = "generated/source_root_motion.rs"]
mod source_root_motion;

pub const PLAYER_COUNT: usize = 2;
pub(crate) const EXPIRED_INPUT_TIMER: u8 = 0xfe;
// Fallback for Melee's JObj-sourced ECB path while non-sampled actions are
// still being migrated from extracted per-frame data.
pub(crate) const SOURCE_JOBJ_ECB_BOTTOM_OFFSET_Y: i32 = 2_790;
const FALLBACK_ECB_WIDTH_UNITS: i32 = 4_000;
const PLAYER_ONE_DEFAULT_SPAWN_X: i32 = -20_000;
const PLAYER_TWO_DEFAULT_SPAWN_X: i32 = 20_000;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Vec2 {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SourceVec2 {
    pub x: f32,
    pub y: f32,
}

impl SourceVec2 {
    pub fn from_milli(position: Vec2) -> Self {
        Self {
            x: milli_to_source_units(position.x),
            y: milli_to_source_units(position.y),
        }
    }

    pub fn to_milli(self) -> Vec2 {
        Vec2 {
            x: source_units_to_milli(self.x),
            y: source_units_to_milli(self.y),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FighterProfileExtractError {
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

impl fmt::Display for FighterProfileExtractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FighterProfileExtractError::TooShort {
                field,
                offset,
                required_len,
                actual_len,
            } => write!(
                f,
                "ftCo_DatAttrs is too short for {field} at 0x{offset:x}: \
                 need {required_len} bytes, got {actual_len}"
            ),
            FighterProfileExtractError::NonFiniteFloat { field, offset } => write!(
                f,
                "ftCo_DatAttrs field {field} at 0x{offset:x} is not finite"
            ),
            FighterProfileExtractError::OutOfRange {
                field,
                offset,
                value,
                min,
                max,
            } => write!(
                f,
                "ftCo_DatAttrs field {field} at 0x{offset:x} produced {value}, \
                 outside {min}..={max}"
            ),
        }
    }
}

impl std::error::Error for FighterProfileExtractError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FighterActionFrames {
    pub attack1_total_frames: u8,
    pub attack1_iasa_frame: u8,
    pub attack12_total_frames: u8,
    pub attack12_iasa_frame: u8,
    pub attack13_total_frames: u8,
    pub attack13_iasa_frame: u8,
    pub jab_2_input_window: u8,
    pub jab_3_input_window: u8,
    pub rapid_jab_window: u8,
    pub attack11_jab_combo_enable_frame: u8,
    pub attack12_jab_combo_enable_frame: u8,
    pub attack_dash_total_frames: u8,
    pub attack_dash_iasa_frame: u8,
    pub attack_air_n_landing_lag_set_frame: u8,
    pub attack_air_n_landing_lag_clear_frame: u8,
    pub attack_air_f_landing_lag_set_frame: u8,
    pub attack_air_f_landing_lag_clear_frame: u8,
    pub attack_air_b_landing_lag_set_frame: u8,
    pub attack_air_b_landing_lag_clear_frame: u8,
    pub attack_air_hi_landing_lag_set_frame: u8,
    pub attack_air_hi_landing_lag_clear_frame: u8,
    pub attack_air_lw_landing_lag_set_frame: u8,
    pub attack_air_lw_landing_lag_clear_frame: u8,
    pub dash_total_frames: u8,
    pub dash_cmd_var0_clear_frame: u8,
    pub dash_cmd_var0_set_frame: u8,
    pub guard_on_total_frames: u8,
    pub guard_off_total_frames: u8,
    pub escape_n_total_frames: u8,
    pub escape_f_total_frames: u8,
    pub escape_b_total_frames: u8,
    pub escape_air_skip_decay_frame: u8,
    pub turn_run_total_frames: u8,
    pub turn_run_cmd_var1_frame: u8,
    pub run_brake_total_frames: u8,
    pub run_brake_cmd_var0_set_frame: u8,
    pub run_brake_cmd_var0_clear_frame: u8,
    pub squat_total_frames: u8,
    pub squat_rv_total_frames: u8,
}

impl FighterActionFrames {
    pub const FALCON_LIKE: Self = Self {
        attack1_total_frames: 21,
        attack1_iasa_frame: 16,
        attack12_total_frames: 20,
        attack12_iasa_frame: 18,
        attack13_total_frames: 32,
        attack13_iasa_frame: 22,
        jab_2_input_window: 24,
        jab_3_input_window: 24,
        rapid_jab_window: 4,
        attack11_jab_combo_enable_frame: 9,
        attack12_jab_combo_enable_frame: 8,
        attack_dash_total_frames: 39,
        attack_dash_iasa_frame: 38,
        attack_air_n_landing_lag_set_frame: 4,
        attack_air_n_landing_lag_clear_frame: 34,
        attack_air_f_landing_lag_set_frame: 7,
        attack_air_f_landing_lag_clear_frame: 35,
        attack_air_b_landing_lag_set_frame: 7,
        attack_air_b_landing_lag_clear_frame: 21,
        attack_air_hi_landing_lag_set_frame: 0,
        attack_air_hi_landing_lag_clear_frame: 22,
        attack_air_lw_landing_lag_set_frame: 4,
        attack_air_lw_landing_lag_clear_frame: 36,
        dash_total_frames: 29,
        dash_cmd_var0_clear_frame: 0,
        dash_cmd_var0_set_frame: 16,
        guard_on_total_frames: 8,
        guard_off_total_frames: 15,
        escape_n_total_frames: 23,
        escape_f_total_frames: 31,
        escape_b_total_frames: 31,
        escape_air_skip_decay_frame: 30,
        turn_run_total_frames: 22,
        turn_run_cmd_var1_frame: 9,
        run_brake_total_frames: 28,
        run_brake_cmd_var0_set_frame: 0,
        run_brake_cmd_var0_clear_frame: 15,
        squat_total_frames: 4,
        squat_rv_total_frames: 4,
    };

    pub const fn falcon_like() -> Self {
        Self::FALCON_LIKE
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FighterProfile {
    pub reference_character: &'static str,
    pub action_frames: FighterActionFrames,
    pub walk_initial_velocity: f32,
    pub walk_accel: f32,
    pub walk_max_velocity: f32,
    pub slow_walk_max_velocity: f32,
    pub mid_walk_point: f32,
    pub fast_walk_min: f32,
    pub run_animation_scaling: f32,
    pub dash_initial_velocity: f32,
    pub dash_run_acceleration_a: f32,
    pub dash_run_acceleration_b: f32,
    pub dash_run_terminal_velocity: f32,
    pub max_run_brake_frames: Option<u8>,
    pub ground_friction: f32,
    pub ground_max_horizontal_velocity: f32,
    pub ground_to_air_jump_momentum_multiplier: f32,
    pub jump_horizontal_initial_velocity: f32,
    pub jump_horizontal_max_velocity: f32,
    pub air_jump_horizontal_multiplier: f32,
    pub air_jump_vertical_multiplier: f32,
    pub max_jumps: u8,
    pub air_drift_stick_multiplier: f32,
    pub aerial_drift_base: f32,
    pub air_drift_max: f32,
    pub aerial_friction: f32,
    pub air_max_horizontal_velocity: f32,
    pub weight: f32,
    pub gravity: f32,
    pub terminal_velocity: f32,
    pub fast_fall_velocity: f32,
    pub jump_vertical_initial_velocity: f32,
    pub hop_vertical_initial_velocity: f32,
    pub full_hop_height: i32,
    pub short_hop_height: i32,
    pub double_jump_height: i32,
    pub entry_platform_offset_y: i32,
    pub standing_height_units: i32,
    pub player_nudge_body_center_x: f32,
    pub player_nudge_body_half_width: f32,
    pub jumpsquat_frames: u8,
    pub dash_frames: u8,
    pub standing_turn_direction_change_frames: u8,
    pub standing_turn_total_frames: u8,
    pub normal_landing_lag_ticks: u8,
    pub landing_air_n_lag_ticks: u8,
    pub landing_air_f_lag_ticks: u8,
    pub landing_air_b_lag_ticks: u8,
    pub landing_air_hi_lag_ticks: u8,
    pub landing_air_lw_lag_ticks: u8,
}

impl FighterProfile {
    pub const FALCON_LIKE: Self = Self {
        reference_character: "captain_falcon",
        action_frames: FighterActionFrames::FALCON_LIKE,
        walk_initial_velocity: 0.15000000596046448,
        walk_accel: 0.10000000149011612,
        walk_max_velocity: 0.8500000238418579,
        slow_walk_max_velocity: 0.16500000655651093,
        mid_walk_point: 0.40700000524520874,
        fast_walk_min: 0.659600019454956,
        run_animation_scaling: 2.3299999237060547,
        dash_initial_velocity: 2.0,
        dash_run_acceleration_a: 0.15000000596046448,
        dash_run_acceleration_b: 0.009999999776482582,
        dash_run_terminal_velocity: 2.299999952316284,
        max_run_brake_frames: Some(30),
        ground_friction: 0.07999999821186066,
        ground_max_horizontal_velocity: 3.0,
        ground_to_air_jump_momentum_multiplier: 0.75,
        jump_horizontal_initial_velocity: 0.949999988079071,
        jump_horizontal_max_velocity: 2.0999999046325684,
        air_jump_horizontal_multiplier: 0.8999999761581421,
        air_jump_vertical_multiplier: 0.8999999761581421,
        max_jumps: 2,
        air_drift_stick_multiplier: 0.03999999910593033,
        aerial_drift_base: 0.019999999552965164,
        air_drift_max: 1.1200000047683716,
        aerial_friction: 0.009999999776482582,
        air_max_horizontal_velocity: 3.0,
        weight: 104.0,
        gravity: 0.12999999523162842,
        terminal_velocity: 2.9000000953674316,
        fast_fall_velocity: 3.5,
        jump_vertical_initial_velocity: 3.0999999046325684,
        hop_vertical_initial_velocity: 1.899999976158142,
        full_hop_height: 38_520,
        short_hop_height: 14_850,
        double_jump_height: 28_560,
        entry_platform_offset_y: 1_647,
        // Provisional visual scale: current 136 px standing sprite at the old 6 px/unit art calibration.
        standing_height_units: 22_667,
        player_nudge_body_center_x: 0.0,
        player_nudge_body_half_width: 3.5,
        jumpsquat_frames: 4,
        dash_frames: 15,
        standing_turn_direction_change_frames: 6,
        standing_turn_total_frames: 11,
        normal_landing_lag_ticks: 4,
        landing_air_n_lag_ticks: 15,
        landing_air_f_lag_ticks: 19,
        landing_air_b_lag_ticks: 18,
        landing_air_hi_lag_ticks: 15,
        landing_air_lw_lag_ticks: 24,
    };

    pub const fn falcon_like() -> Self {
        Self::FALCON_LIKE
    }

    pub const fn from_ftdata_x50_player_nudge_body_box(
        mut profile: Self,
        center_x: f32,
        half_width: f32,
    ) -> Self {
        profile.player_nudge_body_center_x = center_x;
        profile.player_nudge_body_half_width = half_width;
        profile
    }

    pub const fn reusable_air_jumps(self) -> u8 {
        if self.max_jumps == 0 {
            0
        } else {
            self.max_jumps - 1
        }
    }

    pub fn from_ftco_dat_attrs_bytes(
        reference_character: &'static str,
        bytes: &[u8],
    ) -> Result<Self, FighterProfileExtractError> {
        let mut profile = Self::FALCON_LIKE;
        profile.reference_character = reference_character;

        profile.walk_initial_velocity = read_profile_f32(bytes, 0x00, "walk_initial_velocity")?;
        profile.walk_accel = read_profile_f32(bytes, 0x04, "walk_accel")?;
        profile.walk_max_velocity = read_profile_f32(bytes, 0x08, "walk_max_vel")?;
        profile.slow_walk_max_velocity = read_profile_f32(bytes, 0x0c, "slow_walk_max_velocity")?;
        profile.mid_walk_point = read_profile_f32(bytes, 0x10, "mid_walk_threshold")?;
        profile.fast_walk_min = read_profile_f32(bytes, 0x14, "fast_walk_threshold")?;
        profile.ground_friction = read_profile_f32(bytes, 0x18, "gr_friction")?;
        profile.dash_initial_velocity = read_profile_f32(bytes, 0x1c, "dash_initial_velocity")?;
        profile.dash_run_acceleration_a = read_profile_f32(bytes, 0x20, "dash_run_acceleration_a")?;
        profile.dash_run_acceleration_b = read_profile_f32(bytes, 0x24, "dash_run_acceleration_b")?;
        profile.dash_run_terminal_velocity =
            read_profile_f32(bytes, 0x28, "dash_run_terminal_velocity")?;
        profile.run_animation_scaling = read_profile_f32(bytes, 0x2c, "run_animation_scaling")?;
        profile.max_run_brake_frames = Some(read_profile_u8_from_f32(
            bytes,
            0x30,
            "max_run_brake_frames",
        )?);
        profile.ground_max_horizontal_velocity =
            read_profile_f32(bytes, 0x34, "ground_max_horizontal_velocity")?;
        profile.jumpsquat_frames = read_profile_u8_from_f32(bytes, 0x38, "jump_startup_time")?;
        profile.jump_horizontal_initial_velocity =
            read_profile_f32(bytes, 0x3c, "jump_h_initial_velocity")?;
        profile.jump_vertical_initial_velocity =
            read_profile_f32(bytes, 0x40, "jump_v_initial_velocity")?;
        profile.ground_to_air_jump_momentum_multiplier =
            read_profile_f32(bytes, 0x44, "ground_to_air_jump_momentum_multiplier")?;
        profile.jump_horizontal_max_velocity =
            read_profile_f32(bytes, 0x48, "jump_h_max_velocity")?;
        profile.hop_vertical_initial_velocity =
            read_profile_f32(bytes, 0x4c, "hop_v_initial_velocity")?;
        profile.air_jump_vertical_multiplier =
            read_profile_f32(bytes, 0x50, "air_jump_v_multiplier")?;
        profile.air_jump_horizontal_multiplier =
            read_profile_f32(bytes, 0x54, "air_jump_h_multiplier")?;
        profile.max_jumps = read_profile_u8_from_i32(bytes, 0x58, "max_jumps")?;
        profile.gravity = read_profile_f32(bytes, 0x5c, "grav")?;
        profile.terminal_velocity = read_profile_f32(bytes, 0x60, "terminal_vel")?;
        profile.air_drift_stick_multiplier = read_profile_f32(bytes, 0x64, "air_drift_stick_mul")?;
        profile.aerial_drift_base = read_profile_f32(bytes, 0x68, "aerial_drift_base")?;
        profile.air_drift_max = read_profile_f32(bytes, 0x6c, "air_drift_max")?;
        profile.aerial_friction = read_profile_f32(bytes, 0x70, "aerial_friction")?;
        profile.fast_fall_velocity = read_profile_f32(bytes, 0x74, "fast_fall_velocity")?;
        profile.air_max_horizontal_velocity =
            read_profile_f32(bytes, 0x78, "air_max_horizontal_velocity")?;
        profile.action_frames.jab_2_input_window =
            read_profile_u8_from_f32(bytes, 0x7c, "jab_2_input_window")?;
        profile.action_frames.jab_3_input_window =
            read_profile_u8_from_f32(bytes, 0x80, "jab_3_input_window")?;
        profile.weight = read_profile_f32(bytes, 0x88, "weight")?;
        profile.action_frames.rapid_jab_window =
            read_profile_u8_from_i32(bytes, 0x98, "rapid_jab_window")?;
        profile.standing_turn_direction_change_frames =
            read_profile_u8_from_f32(bytes, 0x84, "frames_to_change_direction_on_standing_turn")?;
        let trophy_scale = read_profile_f32(bytes, 0x110, "trophy_scale")?;
        profile.entry_platform_offset_y = round_profile_f32_to_i32(
            trophy_scale * 1.497_345 * 1000.0,
            "trophy_scale*entry_platform_offset",
            0x110,
        )?;
        profile.normal_landing_lag_ticks =
            read_profile_u8_from_f32(bytes, 0xe4, "normal_landing_lag")?;
        profile.landing_air_n_lag_ticks = read_profile_u8_from_f32(bytes, 0xe8, "landingairn_lag")?;
        profile.landing_air_f_lag_ticks = read_profile_u8_from_f32(bytes, 0xec, "landingairf_lag")?;
        profile.landing_air_b_lag_ticks = read_profile_u8_from_f32(bytes, 0xf0, "landingairb_lag")?;
        profile.landing_air_hi_lag_ticks =
            read_profile_u8_from_f32(bytes, 0xf4, "landingairhi_lag")?;
        profile.landing_air_lw_lag_ticks =
            read_profile_u8_from_f32(bytes, 0xf8, "landingairlw_lag")?;

        Ok(profile)
    }
}

fn read_profile_bytes<const LEN: usize>(
    bytes: &[u8],
    offset: usize,
    field: &'static str,
) -> Result<[u8; LEN], FighterProfileExtractError> {
    let required_len = offset + LEN;
    let Some(slice) = bytes.get(offset..required_len) else {
        return Err(FighterProfileExtractError::TooShort {
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

fn read_profile_f32(
    bytes: &[u8],
    offset: usize,
    field: &'static str,
) -> Result<f32, FighterProfileExtractError> {
    let value = f32::from_be_bytes(read_profile_bytes(bytes, offset, field)?);
    if !value.is_finite() {
        return Err(FighterProfileExtractError::NonFiniteFloat { field, offset });
    }
    Ok(value)
}

fn read_profile_u8_from_f32(
    bytes: &[u8],
    offset: usize,
    field: &'static str,
) -> Result<u8, FighterProfileExtractError> {
    let value = round_profile_f32_to_i32(read_profile_f32(bytes, offset, field)?, field, offset)?;
    range_profile_i32(value, 0, u8::MAX as i32, field, offset).map(|value| value as u8)
}

fn read_profile_u8_from_i32(
    bytes: &[u8],
    offset: usize,
    field: &'static str,
) -> Result<u8, FighterProfileExtractError> {
    let value = i32::from_be_bytes(read_profile_bytes(bytes, offset, field)?);
    range_profile_i32(value, 0, u8::MAX as i32, field, offset).map(|value| value as u8)
}

fn round_profile_f32_to_i32(
    value: f32,
    field: &'static str,
    offset: usize,
) -> Result<i32, FighterProfileExtractError> {
    if !value.is_finite() {
        return Err(FighterProfileExtractError::NonFiniteFloat { field, offset });
    }

    let rounded = value.round();
    if rounded < i32::MIN as f32 || rounded > i32::MAX as f32 {
        return Err(FighterProfileExtractError::OutOfRange {
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

fn range_profile_i32(
    value: i32,
    min: i32,
    max: i32,
    field: &'static str,
    offset: usize,
) -> Result<i32, FighterProfileExtractError> {
    if value < min || value > max {
        return Err(FighterProfileExtractError::OutOfRange {
            field,
            offset,
            value,
            min,
            max,
        });
    }
    Ok(value)
}

impl Default for FighterProfile {
    fn default() -> Self {
        Self::falcon_like()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MotionState {
    #[default]
    Wait,
    Entry,
    EntryStart,
    EntryEnd,
    WalkSlow,
    WalkMiddle,
    WalkFast,
    Dash,
    Run,
    RunDirect,
    RunBrake,
    TurnRun,
    Turn,
    Squat,
    SquatWait,
    SquatRv,
    SpecialN,
    SpecialSStart,
    SpecialS,
    SpecialHi,
    SpecialLw,
    SpecialAirN,
    SpecialAirSStart,
    SpecialAirS,
    SpecialAirHi,
    SpecialAirLw,
    AttackAirN,
    AttackAirF,
    AttackAirB,
    AttackAirHi,
    AttackAirLw,
    LandingAirN,
    LandingAirF,
    LandingAirB,
    LandingAirHi,
    LandingAirLw,
    Catch,
    CatchDash,
    Attack1,
    AttackDash,
    AttackS3,
    AttackHi3,
    AttackLw3,
    AttackS4,
    AttackHi4,
    AttackLw4,
    KneeBend,
    JumpF,
    JumpB,
    Fall,
    FallF,
    FallB,
    FallAerial,
    FallAerialF,
    FallAerialB,
    JumpAerialF,
    JumpAerialB,
    GuardOn,
    Guard,
    GuardOff,
    GuardSetOff,
    GuardReflect,
    EscapeN,
    EscapeF,
    EscapeB,
    EscapeAir,
    FallSpecial,
    FallSpecialF,
    FallSpecialB,
    LandingFallSpecial,
    Landing,
    Pass,
}

pub const RUST_MOTION_STATE_VARIANTS: &[&str] = &[
    "Wait",
    "Entry",
    "EntryStart",
    "EntryEnd",
    "WalkSlow",
    "WalkMiddle",
    "WalkFast",
    "Dash",
    "Run",
    "RunDirect",
    "RunBrake",
    "TurnRun",
    "Turn",
    "Squat",
    "SquatWait",
    "SquatRv",
    "SpecialN",
    "SpecialSStart",
    "SpecialS",
    "SpecialHi",
    "SpecialLw",
    "SpecialAirN",
    "SpecialAirSStart",
    "SpecialAirS",
    "SpecialAirHi",
    "SpecialAirLw",
    "AttackAirN",
    "AttackAirF",
    "AttackAirB",
    "AttackAirHi",
    "AttackAirLw",
    "LandingAirN",
    "LandingAirF",
    "LandingAirB",
    "LandingAirHi",
    "LandingAirLw",
    "Catch",
    "CatchDash",
    "Attack1",
    "AttackDash",
    "AttackS3",
    "AttackHi3",
    "AttackLw3",
    "AttackS4",
    "AttackHi4",
    "AttackLw4",
    "KneeBend",
    "JumpF",
    "JumpB",
    "Fall",
    "FallF",
    "FallB",
    "FallAerial",
    "FallAerialF",
    "FallAerialB",
    "JumpAerialF",
    "JumpAerialB",
    "GuardOn",
    "Guard",
    "GuardOff",
    "GuardSetOff",
    "GuardReflect",
    "EscapeN",
    "EscapeF",
    "EscapeB",
    "EscapeAir",
    "FallSpecial",
    "FallSpecialF",
    "FallSpecialB",
    "LandingFallSpecial",
    "Landing",
    "Pass",
];

pub fn motion_state_for_runtime_variant(state: &str) -> Option<MotionState> {
    Some(match state {
        "Wait" => MotionState::Wait,
        "Entry" => MotionState::Entry,
        "EntryStart" => MotionState::EntryStart,
        "EntryEnd" => MotionState::EntryEnd,
        "WalkSlow" => MotionState::WalkSlow,
        "WalkMiddle" => MotionState::WalkMiddle,
        "WalkFast" => MotionState::WalkFast,
        "Dash" => MotionState::Dash,
        "Run" => MotionState::Run,
        "RunDirect" => MotionState::RunDirect,
        "RunBrake" => MotionState::RunBrake,
        "TurnRun" => MotionState::TurnRun,
        "Turn" => MotionState::Turn,
        "Squat" => MotionState::Squat,
        "SquatWait" => MotionState::SquatWait,
        "SquatRv" => MotionState::SquatRv,
        "SpecialN" => MotionState::SpecialN,
        "SpecialSStart" => MotionState::SpecialSStart,
        "SpecialS" => MotionState::SpecialS,
        "SpecialHi" => MotionState::SpecialHi,
        "SpecialLw" => MotionState::SpecialLw,
        "SpecialAirN" => MotionState::SpecialAirN,
        "SpecialAirSStart" => MotionState::SpecialAirSStart,
        "SpecialAirS" => MotionState::SpecialAirS,
        "SpecialAirHi" => MotionState::SpecialAirHi,
        "SpecialAirLw" => MotionState::SpecialAirLw,
        "AttackAirN" => MotionState::AttackAirN,
        "AttackAirF" => MotionState::AttackAirF,
        "AttackAirB" => MotionState::AttackAirB,
        "AttackAirHi" => MotionState::AttackAirHi,
        "AttackAirLw" => MotionState::AttackAirLw,
        "LandingAirN" => MotionState::LandingAirN,
        "LandingAirF" => MotionState::LandingAirF,
        "LandingAirB" => MotionState::LandingAirB,
        "LandingAirHi" => MotionState::LandingAirHi,
        "LandingAirLw" => MotionState::LandingAirLw,
        "Catch" => MotionState::Catch,
        "CatchDash" => MotionState::CatchDash,
        "Attack1" => MotionState::Attack1,
        "AttackDash" => MotionState::AttackDash,
        "AttackS3" => MotionState::AttackS3,
        "AttackHi3" => MotionState::AttackHi3,
        "AttackLw3" => MotionState::AttackLw3,
        "AttackS4" => MotionState::AttackS4,
        "AttackHi4" => MotionState::AttackHi4,
        "AttackLw4" => MotionState::AttackLw4,
        "KneeBend" => MotionState::KneeBend,
        "JumpF" => MotionState::JumpF,
        "JumpB" => MotionState::JumpB,
        "Fall" => MotionState::Fall,
        "FallF" => MotionState::FallF,
        "FallB" => MotionState::FallB,
        "FallAerial" => MotionState::FallAerial,
        "FallAerialF" => MotionState::FallAerialF,
        "FallAerialB" => MotionState::FallAerialB,
        "JumpAerialF" => MotionState::JumpAerialF,
        "JumpAerialB" => MotionState::JumpAerialB,
        "GuardOn" => MotionState::GuardOn,
        "Guard" => MotionState::Guard,
        "GuardOff" => MotionState::GuardOff,
        "GuardSetOff" => MotionState::GuardSetOff,
        "GuardReflect" => MotionState::GuardReflect,
        "EscapeN" => MotionState::EscapeN,
        "EscapeF" => MotionState::EscapeF,
        "EscapeB" => MotionState::EscapeB,
        "EscapeAir" => MotionState::EscapeAir,
        "FallSpecial" => MotionState::FallSpecial,
        "FallSpecialF" => MotionState::FallSpecialF,
        "FallSpecialB" => MotionState::FallSpecialB,
        "LandingFallSpecial" => MotionState::LandingFallSpecial,
        "Landing" => MotionState::Landing,
        "Pass" => MotionState::Pass,
        _ => return None,
    })
}

pub fn runtime_motion_state_for_source_key(source_action_key: &str) -> Option<&'static str> {
    if RUST_MOTION_STATE_VARIANTS.contains(&source_action_key) {
        return RUST_MOTION_STATE_VARIANTS
            .iter()
            .copied()
            .find(|state| *state == source_action_key);
    }
    let alias = match source_action_key {
        "Wait1" => Some("Wait"),
        "Attack11" => Some("Attack1"),
        "AttackS3S" => Some("AttackS3"),
        "AttackS4S" => Some("AttackS4"),
        _ => None,
    }?;
    RUST_MOTION_STATE_VARIANTS.contains(&alias).then_some(alias)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MeleeActionStateId(u16);

impl MeleeActionStateId {
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

pub const fn is_source_damage_action_state_id(action_state_id: MeleeActionStateId) -> bool {
    let id = action_state_id.get();
    id >= 75 && id <= 91
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceActionKey(&'static str);

impl SourceActionKey {
    pub const fn new(value: &'static str) -> Self {
        Self(value)
    }

    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceDownBoundPose {
    pub hip_mtx_0_1: f32,
    pub hip_mtx_0_2: f32,
    pub hip_mtx_1_1: f32,
    pub hip_mtx_1_2: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SourceActionPoseMetadata {
    pub down_bound_pose: Option<SourceDownBoundPose>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotionStateSourceBinding {
    pub motion_state: MotionState,
    pub action_state_id: MeleeActionStateId,
    pub source_action_table_id: u16,
    pub source_action_key: SourceActionKey,
}

impl MotionStateSourceBinding {
    const fn new(
        motion_state: MotionState,
        source_action_table_id: u16,
        source_action_key: &'static str,
    ) -> Self {
        Self {
            motion_state,
            action_state_id: melee_action_state_id_for_motion_state(motion_state),
            source_action_table_id,
            source_action_key: SourceActionKey::new(source_action_key),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanonicalSourceActionBinding {
    pub action_state_id: MeleeActionStateId,
    pub source_action_table_id: u16,
    pub source_action_key: SourceActionKey,
}

impl CanonicalSourceActionBinding {
    const fn new(
        action_state_id: u16,
        source_action_table_id: u16,
        source_action_key: &'static str,
    ) -> Self {
        Self {
            action_state_id: MeleeActionStateId::new(action_state_id),
            source_action_table_id,
            source_action_key: SourceActionKey::new(source_action_key),
        }
    }
}

pub const CANONICAL_SOURCE_ONLY_ACTION_BINDINGS: &[CanonicalSourceActionBinding] = &[
    CanonicalSourceActionBinding::new(45, 47, "Attack12"),
    CanonicalSourceActionBinding::new(46, 48, "Attack13"),
    CanonicalSourceActionBinding::new(47, 49, "Attack100Start"),
    CanonicalSourceActionBinding::new(48, 50, "Attack100Loop"),
    CanonicalSourceActionBinding::new(49, 51, "Attack100End"),
    CanonicalSourceActionBinding::new(75, 165, "DamageHi1"),
    CanonicalSourceActionBinding::new(76, 166, "DamageHi2"),
    CanonicalSourceActionBinding::new(77, 167, "DamageHi3"),
    CanonicalSourceActionBinding::new(78, 168, "DamageN1"),
    CanonicalSourceActionBinding::new(79, 169, "DamageN2"),
    CanonicalSourceActionBinding::new(80, 170, "DamageN3"),
    CanonicalSourceActionBinding::new(81, 171, "DamageLw1"),
    CanonicalSourceActionBinding::new(82, 172, "DamageLw2"),
    CanonicalSourceActionBinding::new(83, 173, "DamageLw3"),
    CanonicalSourceActionBinding::new(84, 174, "DamageAir1"),
    CanonicalSourceActionBinding::new(85, 175, "DamageAir2"),
    CanonicalSourceActionBinding::new(86, 176, "DamageAir3"),
    CanonicalSourceActionBinding::new(87, 177, "DamageFlyHi"),
    CanonicalSourceActionBinding::new(88, 178, "DamageFlyN"),
    CanonicalSourceActionBinding::new(89, 179, "DamageFlyLw"),
    CanonicalSourceActionBinding::new(90, 180, "DamageFlyTop"),
    CanonicalSourceActionBinding::new(91, 181, "DamageFlyRoll"),
    CanonicalSourceActionBinding::new(183, 288, "DownBoundU"),
    CanonicalSourceActionBinding::new(184, 184, "DownWaitU"),
    CanonicalSourceActionBinding::new(186, 290, "DownStandU"),
    CanonicalSourceActionBinding::new(187, 187, "DownAttackU"),
    CanonicalSourceActionBinding::new(191, 289, "DownBoundD"),
    CanonicalSourceActionBinding::new(192, 192, "DownWaitD"),
    CanonicalSourceActionBinding::new(194, 291, "DownStandD"),
    CanonicalSourceActionBinding::new(195, 195, "DownAttackD"),
    CanonicalSourceActionBinding::new(199, 199, "Passive"),
    CanonicalSourceActionBinding::new(200, 200, "PassiveStandF"),
    CanonicalSourceActionBinding::new(201, 201, "PassiveStandB"),
];

pub fn canonical_source_action_binding_for_source_table_id(
    source_action_table_id: u16,
) -> Option<CanonicalSourceActionBinding> {
    CANONICAL_SOURCE_ONLY_ACTION_BINDINGS
        .iter()
        .copied()
        .find(|binding| binding.source_action_table_id == source_action_table_id)
}

pub const fn melee_action_state_id_for_motion_state(
    motion_state: MotionState,
) -> MeleeActionStateId {
    MeleeActionStateId::new(match motion_state {
        MotionState::Wait => 14,
        MotionState::Entry => 322,
        MotionState::EntryStart => 323,
        MotionState::EntryEnd => 324,
        MotionState::WalkSlow => 15,
        MotionState::WalkMiddle => 16,
        MotionState::WalkFast => 17,
        MotionState::Dash => 20,
        MotionState::Run => 21,
        MotionState::RunDirect => 22,
        MotionState::RunBrake => 23,
        MotionState::TurnRun => 19,
        MotionState::Turn => 18,
        MotionState::Squat => 39,
        MotionState::SquatWait => 40,
        MotionState::SquatRv => 41,
        MotionState::SpecialN => 347,
        MotionState::SpecialSStart => 349,
        MotionState::SpecialS => 350,
        MotionState::SpecialHi => 353,
        MotionState::SpecialLw => 357,
        MotionState::SpecialAirN => 348,
        MotionState::SpecialAirSStart => 351,
        MotionState::SpecialAirS => 352,
        MotionState::SpecialAirHi => 354,
        MotionState::SpecialAirLw => 359,
        MotionState::AttackAirN => 65,
        MotionState::AttackAirF => 66,
        MotionState::AttackAirB => 67,
        MotionState::AttackAirHi => 68,
        MotionState::AttackAirLw => 69,
        MotionState::LandingAirN => 70,
        MotionState::LandingAirF => 71,
        MotionState::LandingAirB => 72,
        MotionState::LandingAirHi => 73,
        MotionState::LandingAirLw => 74,
        MotionState::Catch => 212,
        MotionState::CatchDash => 214,
        MotionState::Attack1 => 44,
        MotionState::AttackDash => 50,
        MotionState::AttackS3 => 53,
        MotionState::AttackHi3 => 56,
        MotionState::AttackLw3 => 57,
        MotionState::AttackS4 => 60,
        MotionState::AttackHi4 => 63,
        MotionState::AttackLw4 => 64,
        MotionState::KneeBend => 24,
        MotionState::JumpF => 25,
        MotionState::JumpB => 26,
        MotionState::Fall => 29,
        MotionState::FallF => 30,
        MotionState::FallB => 31,
        MotionState::FallAerial => 32,
        MotionState::FallAerialF => 33,
        MotionState::FallAerialB => 34,
        MotionState::JumpAerialF => 27,
        MotionState::JumpAerialB => 28,
        MotionState::GuardOn => 178,
        MotionState::Guard => 179,
        MotionState::GuardOff => 180,
        MotionState::GuardSetOff => 181,
        MotionState::GuardReflect => 182,
        MotionState::EscapeN => 235,
        MotionState::EscapeF => 233,
        MotionState::EscapeB => 234,
        MotionState::EscapeAir => 236,
        MotionState::FallSpecial => 35,
        MotionState::FallSpecialF => 36,
        MotionState::FallSpecialB => 37,
        MotionState::LandingFallSpecial => 43,
        MotionState::Landing => 42,
        MotionState::Pass => 244,
    })
}

pub const fn source_binding_for_motion_state(
    motion_state: MotionState,
) -> Option<MotionStateSourceBinding> {
    match motion_state {
        MotionState::Wait => Some(MotionStateSourceBinding::new(MotionState::Wait, 2, "Wait1")),
        MotionState::Entry => Some(MotionStateSourceBinding::new(
            MotionState::Entry,
            238,
            "Entry",
        )),
        MotionState::WalkSlow => Some(MotionStateSourceBinding::new(
            MotionState::WalkSlow,
            7,
            "WalkSlow",
        )),
        MotionState::WalkMiddle => Some(MotionStateSourceBinding::new(
            MotionState::WalkMiddle,
            8,
            "WalkMiddle",
        )),
        MotionState::WalkFast => Some(MotionStateSourceBinding::new(
            MotionState::WalkFast,
            9,
            "WalkFast",
        )),
        MotionState::Turn => Some(MotionStateSourceBinding::new(MotionState::Turn, 10, "Turn")),
        MotionState::TurnRun => Some(MotionStateSourceBinding::new(
            MotionState::TurnRun,
            11,
            "TurnRun",
        )),
        MotionState::Dash => Some(MotionStateSourceBinding::new(MotionState::Dash, 12, "Dash")),
        MotionState::Run => Some(MotionStateSourceBinding::new(MotionState::Run, 13, "Run")),
        MotionState::RunDirect => Some(MotionStateSourceBinding::new(
            MotionState::RunDirect,
            13,
            "Run",
        )),
        MotionState::RunBrake => Some(MotionStateSourceBinding::new(
            MotionState::RunBrake,
            14,
            "RunBrake",
        )),
        MotionState::Landing => Some(MotionStateSourceBinding::new(
            MotionState::Landing,
            15,
            "Landing",
        )),
        MotionState::KneeBend => Some(MotionStateSourceBinding::new(
            MotionState::KneeBend,
            15,
            "Landing",
        )),
        MotionState::JumpF => Some(MotionStateSourceBinding::new(
            MotionState::JumpF,
            16,
            "JumpF",
        )),
        MotionState::JumpB => Some(MotionStateSourceBinding::new(
            MotionState::JumpB,
            17,
            "JumpB",
        )),
        MotionState::JumpAerialF => Some(MotionStateSourceBinding::new(
            MotionState::JumpAerialF,
            18,
            "JumpAerialF",
        )),
        MotionState::JumpAerialB => Some(MotionStateSourceBinding::new(
            MotionState::JumpAerialB,
            19,
            "JumpAerialB",
        )),
        MotionState::Fall => Some(MotionStateSourceBinding::new(MotionState::Fall, 20, "Fall")),
        MotionState::FallF => Some(MotionStateSourceBinding::new(
            MotionState::FallF,
            21,
            "FallF",
        )),
        MotionState::FallB => Some(MotionStateSourceBinding::new(
            MotionState::FallB,
            22,
            "FallB",
        )),
        MotionState::FallAerial => Some(MotionStateSourceBinding::new(
            MotionState::FallAerial,
            23,
            "FallAerial",
        )),
        MotionState::FallAerialF => Some(MotionStateSourceBinding::new(
            MotionState::FallAerialF,
            24,
            "FallAerialF",
        )),
        MotionState::FallAerialB => Some(MotionStateSourceBinding::new(
            MotionState::FallAerialB,
            25,
            "FallAerialB",
        )),
        MotionState::FallSpecial => Some(MotionStateSourceBinding::new(
            MotionState::FallSpecial,
            26,
            "FallSpecial",
        )),
        MotionState::FallSpecialF => Some(MotionStateSourceBinding::new(
            MotionState::FallSpecialF,
            27,
            "FallSpecialF",
        )),
        MotionState::FallSpecialB => Some(MotionStateSourceBinding::new(
            MotionState::FallSpecialB,
            28,
            "FallSpecialB",
        )),
        MotionState::LandingFallSpecial => Some(MotionStateSourceBinding::new(
            MotionState::LandingFallSpecial,
            36,
            "Landing",
        )),
        MotionState::Squat => Some(MotionStateSourceBinding::new(
            MotionState::Squat,
            30,
            "Squat",
        )),
        MotionState::SquatWait => Some(MotionStateSourceBinding::new(
            MotionState::SquatWait,
            31,
            "SquatWait",
        )),
        MotionState::SquatRv => Some(MotionStateSourceBinding::new(
            MotionState::SquatRv,
            34,
            "SquatRv",
        )),
        MotionState::GuardOn => Some(MotionStateSourceBinding::new(
            MotionState::GuardOn,
            37,
            "GuardOn",
        )),
        MotionState::Guard => Some(MotionStateSourceBinding::new(
            MotionState::Guard,
            38,
            "Guard",
        )),
        MotionState::GuardOff => Some(MotionStateSourceBinding::new(
            MotionState::GuardOff,
            39,
            "GuardOff",
        )),
        MotionState::GuardSetOff => Some(MotionStateSourceBinding::new(
            MotionState::GuardSetOff,
            40,
            "GuardDamage",
        )),
        MotionState::GuardReflect => Some(MotionStateSourceBinding::new(
            MotionState::GuardReflect,
            37,
            "GuardOn",
        )),
        MotionState::EscapeN => Some(MotionStateSourceBinding::new(
            MotionState::EscapeN,
            41,
            "EscapeN",
        )),
        MotionState::EscapeF => Some(MotionStateSourceBinding::new(
            MotionState::EscapeF,
            42,
            "EscapeF",
        )),
        MotionState::EscapeB => Some(MotionStateSourceBinding::new(
            MotionState::EscapeB,
            43,
            "EscapeB",
        )),
        MotionState::EscapeAir => Some(MotionStateSourceBinding::new(
            MotionState::EscapeAir,
            44,
            "EscapeAir",
        )),
        MotionState::Attack1 => Some(MotionStateSourceBinding::new(
            MotionState::Attack1,
            46,
            "Attack11",
        )),
        MotionState::AttackDash => Some(MotionStateSourceBinding::new(
            MotionState::AttackDash,
            52,
            "AttackDash",
        )),
        MotionState::AttackS3 => Some(MotionStateSourceBinding::new(
            MotionState::AttackS3,
            55,
            "AttackS3S",
        )),
        MotionState::AttackHi3 => Some(MotionStateSourceBinding::new(
            MotionState::AttackHi3,
            58,
            "AttackHi3",
        )),
        MotionState::AttackLw3 => Some(MotionStateSourceBinding::new(
            MotionState::AttackLw3,
            59,
            "AttackLw3",
        )),
        MotionState::AttackS4 => Some(MotionStateSourceBinding::new(
            MotionState::AttackS4,
            62,
            "AttackS4S",
        )),
        MotionState::AttackHi4 => Some(MotionStateSourceBinding::new(
            MotionState::AttackHi4,
            66,
            "AttackHi4",
        )),
        MotionState::AttackLw4 => Some(MotionStateSourceBinding::new(
            MotionState::AttackLw4,
            67,
            "AttackLw4",
        )),
        MotionState::AttackAirN => Some(MotionStateSourceBinding::new(
            MotionState::AttackAirN,
            68,
            "AttackAirN",
        )),
        MotionState::AttackAirF => Some(MotionStateSourceBinding::new(
            MotionState::AttackAirF,
            69,
            "AttackAirF",
        )),
        MotionState::AttackAirB => Some(MotionStateSourceBinding::new(
            MotionState::AttackAirB,
            70,
            "AttackAirB",
        )),
        MotionState::AttackAirHi => Some(MotionStateSourceBinding::new(
            MotionState::AttackAirHi,
            71,
            "AttackAirHi",
        )),
        MotionState::AttackAirLw => Some(MotionStateSourceBinding::new(
            MotionState::AttackAirLw,
            72,
            "AttackAirLw",
        )),
        MotionState::LandingAirN => Some(MotionStateSourceBinding::new(
            MotionState::LandingAirN,
            73,
            "LandingAirN",
        )),
        MotionState::LandingAirF => Some(MotionStateSourceBinding::new(
            MotionState::LandingAirF,
            74,
            "LandingAirF",
        )),
        MotionState::LandingAirB => Some(MotionStateSourceBinding::new(
            MotionState::LandingAirB,
            75,
            "LandingAirB",
        )),
        MotionState::LandingAirHi => Some(MotionStateSourceBinding::new(
            MotionState::LandingAirHi,
            76,
            "LandingAirHi",
        )),
        MotionState::LandingAirLw => Some(MotionStateSourceBinding::new(
            MotionState::LandingAirLw,
            77,
            "LandingAirLw",
        )),
        MotionState::Pass => Some(MotionStateSourceBinding::new(
            MotionState::Pass,
            209,
            "Pass",
        )),
        MotionState::EntryStart => Some(MotionStateSourceBinding::new(
            MotionState::EntryStart,
            238,
            "Entry",
        )),
        MotionState::EntryEnd => Some(MotionStateSourceBinding::new(
            MotionState::EntryEnd,
            238,
            "Entry",
        )),
        MotionState::Catch => Some(MotionStateSourceBinding::new(
            MotionState::Catch,
            242,
            "Catch",
        )),
        MotionState::CatchDash => Some(MotionStateSourceBinding::new(
            MotionState::CatchDash,
            243,
            "CatchDash",
        )),
        MotionState::SpecialN => Some(MotionStateSourceBinding::new(
            MotionState::SpecialN,
            301,
            "SpecialN",
        )),
        MotionState::SpecialAirN => Some(MotionStateSourceBinding::new(
            MotionState::SpecialAirN,
            302,
            "SpecialAirN",
        )),
        MotionState::SpecialSStart => Some(MotionStateSourceBinding::new(
            MotionState::SpecialSStart,
            303,
            "SpecialSStart",
        )),
        MotionState::SpecialS => Some(MotionStateSourceBinding::new(
            MotionState::SpecialS,
            304,
            "SpecialS",
        )),
        MotionState::SpecialAirSStart => Some(MotionStateSourceBinding::new(
            MotionState::SpecialAirSStart,
            305,
            "SpecialAirSStart",
        )),
        MotionState::SpecialAirS => Some(MotionStateSourceBinding::new(
            MotionState::SpecialAirS,
            306,
            "SpecialAirS",
        )),
        MotionState::SpecialHi => Some(MotionStateSourceBinding::new(
            MotionState::SpecialHi,
            307,
            "SpecialHi",
        )),
        MotionState::SpecialAirHi => Some(MotionStateSourceBinding::new(
            MotionState::SpecialAirHi,
            308,
            "SpecialAirHi",
        )),
        MotionState::SpecialLw => Some(MotionStateSourceBinding::new(
            MotionState::SpecialLw,
            311,
            "SpecialLw",
        )),
        MotionState::SpecialAirLw => Some(MotionStateSourceBinding::new(
            MotionState::SpecialAirLw,
            313,
            "SpecialAirLw",
        )),
    }
}

const SOURCE_ACTION_FLAG_TRANSN_ROOT: u32 = 0x8000_0000;
const SOURCE_ACTION_FLAG_ALT_ROOT: u32 = 0x0400_0000;
const SOURCE_ACTION_GROUND_VELOCITY_FLAGS: u32 =
    SOURCE_ACTION_FLAG_TRANSN_ROOT | SOURCE_ACTION_FLAG_ALT_ROOT;

pub(crate) const fn source_action_anim_flags_raw_for_motion_state(
    motion_state: MotionState,
) -> Option<u32> {
    match motion_state {
        // Captain Falcon action animation table flags from
        // resources/melee/frame_data/dolphin_mole/source_manifest.json.
        MotionState::Wait => Some(0x0000_0002),
        MotionState::WalkSlow => Some(0x8000_0002),
        MotionState::WalkMiddle => Some(0x8000_0002),
        MotionState::WalkFast => Some(0x4000_0002),
        MotionState::Turn => Some(0x8000_0002),
        MotionState::TurnRun => Some(0x8000_0082),
        MotionState::Dash => Some(0x8000_0002),
        MotionState::Run | MotionState::RunDirect => Some(0x4000_0002),
        MotionState::RunBrake => Some(0x0000_0002),
        MotionState::EscapeF => Some(0x8000_00c2),
        MotionState::EscapeB => Some(0x8000_0002),
        _ => None,
    }
}

pub(crate) fn source_motion_change_clamps_ground_velocity(
    previous: MotionState,
    next: MotionState,
) -> bool {
    let Some(previous_flags) = source_action_anim_flags_raw_for_motion_state(previous) else {
        return false;
    };
    let Some(next_flags) = source_action_anim_flags_raw_for_motion_state(next) else {
        return false;
    };
    previous_flags & SOURCE_ACTION_GROUND_VELOCITY_FLAGS != 0
        && next_flags & SOURCE_ACTION_GROUND_VELOCITY_FLAGS == 0
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerState {
    pub profile: FighterProfile,
    pub position: Vec2,
    pub source_position: SourceVec2,
    pub velocity: Vec2,
    pub source_self_velocity_x: f32,
    pub source_self_velocity_y: f32,
    pub player_nudge_x: f32,
    pub player_nudge_z: f32,
    pub ecb_bottom_offset_y: i32,
    pub ecb_bottom_lock_timer: u8,
    pub jumps_remaining: u8,
    pub grounded: bool,
    pub fast_falling: bool,
    pub facing: i8,
    pub attack_frame: u8,
    pub damage_percent: f32,
    pub damage_percent_temp: f32,
    pub damage_applied: u16,
    pub damage_knockback: f32,
    pub damage_angle: u16,
    pub damage_element: u8,
    pub hitlag_frames: u8,
    pub damage_hitstun_frames: u16,
    pub melee_action_state_id: Option<MeleeActionStateId>,
    pub source_action_key: Option<SourceActionKey>,
    pub source_action_total_frames: u8,
    pub source_down_bound_pose: Option<SourceDownBoundPose>,
    pub source_force_damage_down_bound: bool,
    pub source_down_bound_use_z_axis: bool,
    pub source_down_bound_reverse_face_up: bool,
    pub source_down_wait_timer: f32,
    pub source_lr_digital_press_timer: u8,
    pub source_previous_lr_digital_press_timer: u8,
    pub source_jab_followup_timer: u8,
    pub source_jab_followup_queued: bool,
    pub source_jab_combo_enabled: bool,
    pub motion_state_alias: Option<MotionState>,
    pub motion_state: MotionState,
    pub motion_frame: u8,
    pub motion_anim_frame_milli: i32,
    pub ground_velocity_x: f32,
    pub ground_accel_x: f32,
    pub ground_accel_x2: f32,
    pub dash_entry_velocity_delta: f32,
    pub dash_x0: f32,
    pub dash_started_from_tap: bool,
    pub walk_anim_velocity_x: f32,
    pub walk_accel_mul_milli: i32,
    pub turn_facing_after: i8,
    pub turn_has_turned: bool,
    pub turn_just_turned: bool,
    pub turn_frames_to_turn: u8,
    pub turn_dash_after_direction: i8,
    pub turn_latched_buttons: u8,
    pub turn_run_accel_mul: i8,
    pub run_no_interrupt_frames: u8,
    pub motion_cmd_var0: u32,
    pub motion_cmd_var1: u32,
    pub landing_lag_ticks: u8,
    pub run_brake_x0: bool,
    pub run_brake_frames_remaining: u8,
    pub turn_run_x14: bool,
    pub turn_run_resume_advances: bool,
    pub turn_run_completion_pending: bool,
    pub turn_run_completion_enters_run: bool,
    pub motion_anim_rate_milli: i32,
    pub shield_turn_facing_after: i8,
    pub shield_turn_frame: u8,
    pub guard_catch_dash_window: u8,
    pub jump_input: MeleeJumpInput,
    pub short_hop: bool,
    pub escape_air_iasa_timer: u8,
    pub floor_skip_surface: Option<u8>,
    pub platform_pass_pending: bool,
    pub platform_pass_timer: u8,
    pub entry_base_y: i32,
    pub entry_platform_offset_y: i32,
    pub entry_timer: u8,
}

impl PlayerState {
    pub const fn new(x: i32, y: i32, facing: i8) -> Self {
        Self::new_with_profile(x, y, facing, FighterProfile::FALCON_LIKE)
    }

    pub const fn new_with_profile(x: i32, y: i32, facing: i8, profile: FighterProfile) -> Self {
        Self {
            profile,
            position: Vec2 { x, y },
            source_position: SourceVec2 {
                x: x as f32 / MELEE_UNIT_SCALE as f32,
                y: y as f32 / MELEE_UNIT_SCALE as f32,
            },
            velocity: Vec2 { x: 0, y: 0 },
            source_self_velocity_x: 0.0,
            source_self_velocity_y: 0.0,
            player_nudge_x: 0.0,
            player_nudge_z: 0.0,
            ecb_bottom_offset_y: SOURCE_JOBJ_ECB_BOTTOM_OFFSET_Y,
            ecb_bottom_lock_timer: 0,
            jumps_remaining: profile.reusable_air_jumps(),
            grounded: true,
            fast_falling: false,
            facing,
            attack_frame: 0,
            damage_percent: 0.0,
            damage_percent_temp: 0.0,
            damage_applied: 0,
            damage_knockback: 0.0,
            damage_angle: 0,
            damage_element: 0,
            hitlag_frames: 0,
            damage_hitstun_frames: 0,
            melee_action_state_id: Some(melee_action_state_id_for_motion_state(MotionState::Wait)),
            source_action_key: Some(SourceActionKey::new("Wait1")),
            source_action_total_frames: 0,
            source_down_bound_pose: None,
            source_force_damage_down_bound: false,
            source_down_bound_use_z_axis: false,
            source_down_bound_reverse_face_up: false,
            source_down_wait_timer: 0.0,
            source_lr_digital_press_timer: 0xff,
            source_previous_lr_digital_press_timer: 0xff,
            source_jab_followup_timer: 0,
            source_jab_followup_queued: false,
            source_jab_combo_enabled: false,
            motion_state_alias: Some(MotionState::Wait),
            motion_state: MotionState::Wait,
            motion_frame: 0,
            motion_anim_frame_milli: 0,
            ground_velocity_x: 0.0,
            ground_accel_x: 0.0,
            ground_accel_x2: 0.0,
            dash_entry_velocity_delta: 0.0,
            dash_x0: 0.0,
            dash_started_from_tap: false,
            walk_anim_velocity_x: 0.0,
            walk_accel_mul_milli: 1_000,
            turn_facing_after: facing,
            turn_has_turned: false,
            turn_just_turned: false,
            turn_frames_to_turn: 0,
            turn_dash_after_direction: 0,
            turn_latched_buttons: 0,
            turn_run_accel_mul: facing,
            run_no_interrupt_frames: 0,
            motion_cmd_var0: 0,
            motion_cmd_var1: 0,
            landing_lag_ticks: 0,
            run_brake_x0: false,
            run_brake_frames_remaining: 0,
            turn_run_x14: false,
            turn_run_resume_advances: false,
            turn_run_completion_pending: false,
            turn_run_completion_enters_run: false,
            motion_anim_rate_milli: 1_000,
            shield_turn_facing_after: facing,
            shield_turn_frame: 0,
            guard_catch_dash_window: 0,
            jump_input: MeleeJumpInput::None,
            short_hop: false,
            escape_air_iasa_timer: 0,
            floor_skip_surface: None,
            platform_pass_pending: false,
            platform_pass_timer: 0,
            entry_base_y: y,
            entry_platform_offset_y: profile.entry_platform_offset_y,
            entry_timer: 0,
        }
    }

    pub fn set_motion_state_alias(&mut self, motion_state: MotionState) {
        self.motion_state = motion_state;
        self.motion_state_alias = Some(motion_state);
        self.melee_action_state_id = Some(melee_action_state_id_for_motion_state(motion_state));
        self.damage_hitstun_frames = 0;
        self.source_action_total_frames =
            action_sample_frame_count_for_motion_state(motion_state).unwrap_or(0);
        self.source_down_bound_pose = None;
        self.source_down_wait_timer = 0.0;
        match source_binding_for_motion_state(motion_state) {
            Some(binding) => {
                self.source_action_key = Some(binding.source_action_key);
            }
            None => {
                self.source_action_key = None;
            }
        }
    }
}

pub(crate) fn active_ecb_for_player(
    player: &PlayerState,
    common_data: MeleeCommonData,
) -> EcbDiamond {
    active_ecb_for_motion_frame(
        player,
        player_source_pose_frame(*player).saturating_sub(1),
        common_data,
    )
}

pub(crate) fn active_ecb_for_motion_frame(
    player: &PlayerState,
    motion_frame: u8,
    common_data: MeleeCommonData,
) -> EcbDiamond {
    local_ecb_to_world(
        active_local_ecb(player, motion_frame, common_data),
        player.position,
        player_model_facing(player),
    )
}

pub(crate) fn active_ecb_bottom_offset_y(
    player: &PlayerState,
    motion_frame: u8,
    common_data: MeleeCommonData,
) -> i32 {
    active_local_ecb(player, motion_frame, common_data).bottom.y
}

pub(crate) fn action_sample_frame_count_for_motion_state(motion_state: MotionState) -> Option<u8> {
    let samples = falcon_ecb::falcon_ecb_samples_for_motion_state(motion_state)?;
    u8::try_from(samples.len()).ok()
}

pub fn source_root_motion_delta(
    motion_state: MotionState,
    source_frame: u8,
) -> Option<crate::collision::Vec3> {
    source_root_motion::transn_offset(motion_state, source_frame)
}

pub fn source_root_motion_position(
    motion_state: MotionState,
    source_frame: u8,
) -> Option<crate::collision::Vec3> {
    source_root_motion::transn_position(motion_state, source_frame)
}

fn active_local_ecb(
    player: &PlayerState,
    motion_frame: u8,
    common_data: MeleeCommonData,
) -> EcbDiamond {
    if player.ecb_bottom_lock_timer > 0 {
        return fallback_local_ecb(player, player.ecb_bottom_offset_y);
    }

    let motion_state = active_pose_motion_state(player, common_data);
    if let Some(samples) = falcon_ecb::falcon_ecb_samples_for_motion_state(motion_state) {
        sampled_ecb(
            samples,
            action_pose_sample_frame(
                player,
                motion_state,
                motion_frame,
                common_data,
                samples.len(),
            ),
        )
    } else {
        fallback_local_ecb(player, fallback_bottom_offset_y(player))
    }
}

fn action_pose_sample_frame(
    player: &PlayerState,
    motion_state: MotionState,
    motion_frame: u8,
    common_data: MeleeCommonData,
    sample_count: usize,
) -> u8 {
    match motion_state {
        MotionState::LandingFallSpecial => {
            landing_fall_special_pose_sample_frame(motion_frame, common_data, sample_count)
        }
        MotionState::LandingAirN
        | MotionState::LandingAirF
        | MotionState::LandingAirB
        | MotionState::LandingAirHi
        | MotionState::LandingAirLw => {
            landing_air_pose_sample_frame(player, motion_state, motion_frame, sample_count)
        }
        _ => motion_frame,
    }
}

fn landing_air_pose_sample_frame(
    player: &PlayerState,
    motion_state: MotionState,
    motion_frame: u8,
    sample_count: usize,
) -> u8 {
    let landing_lag = if player.landing_lag_ticks == 0 {
        landing_air_profile_lag_ticks(motion_state, player.profile)
    } else {
        player.landing_lag_ticks
    };
    scaled_landing_pose_sample_frame(motion_frame, landing_lag, sample_count)
}

fn landing_fall_special_pose_sample_frame(
    motion_frame: u8,
    common_data: MeleeCommonData,
    sample_count: usize,
) -> u8 {
    scaled_landing_pose_sample_frame(
        motion_frame,
        common_data.escapeair_landing_lag_ticks,
        sample_count,
    )
}

fn scaled_landing_pose_sample_frame(motion_frame: u8, landing_lag: u8, sample_count: usize) -> u8 {
    if landing_lag == 0 || sample_count == 0 {
        return motion_frame;
    }

    // ftCo_LandingAir_EnterWithMsidLag and ftCo_LandingFallSpecial_Enter
    // both scale the landing figatree by (source_frames + 0.1) / landing_lag.
    let action_frames_tenths = sample_count.saturating_mul(10).saturating_add(1);
    let scaled = usize::from(motion_frame).saturating_mul(action_frames_tenths)
        / (usize::from(landing_lag) * 10);
    u8::try_from(scaled.min(sample_count.saturating_sub(1))).unwrap_or(u8::MAX)
}

fn landing_air_profile_lag_ticks(motion_state: MotionState, profile: FighterProfile) -> u8 {
    match motion_state {
        MotionState::LandingAirN => profile.landing_air_n_lag_ticks,
        MotionState::LandingAirF => profile.landing_air_f_lag_ticks,
        MotionState::LandingAirB => profile.landing_air_b_lag_ticks,
        MotionState::LandingAirHi => profile.landing_air_hi_lag_ticks,
        MotionState::LandingAirLw => profile.landing_air_lw_lag_ticks,
        _ => 0,
    }
}

fn active_pose_motion_state(player: &PlayerState, common_data: MeleeCommonData) -> MotionState {
    let Some((forward, backward)) = fall_directional_pose_pair(player.motion_state) else {
        return player.motion_state;
    };
    if common_data.fall_animation_blend == 0.0 || player.profile.air_drift_max == 0.0 {
        return player.motion_state;
    }

    let drift_ratio =
        (milli_to_source_units(player.velocity.x).abs()) / player.profile.air_drift_max.abs();
    if drift_ratio <= common_data.fall_animation_drift_threshold {
        return player.motion_state;
    }

    if player.velocity.x * player.facing as i32 > 0 {
        forward
    } else {
        backward
    }
}

fn fall_directional_pose_pair(motion_state: MotionState) -> Option<(MotionState, MotionState)> {
    match motion_state {
        MotionState::Fall => Some((MotionState::FallF, MotionState::FallB)),
        MotionState::FallAerial => Some((MotionState::FallAerialF, MotionState::FallAerialB)),
        MotionState::FallSpecial => Some((MotionState::FallSpecialF, MotionState::FallSpecialB)),
        _ => None,
    }
}

fn fallback_bottom_offset_y(player: &PlayerState) -> i32 {
    if player.grounded {
        0
    } else {
        player.ecb_bottom_offset_y
    }
}

fn fallback_local_ecb(player: &PlayerState, bottom_offset_y: i32) -> EcbDiamond {
    EcbDiamond::from_bottom_center_and_size(
        Vec2 {
            x: 0,
            y: bottom_offset_y,
        },
        FALLBACK_ECB_WIDTH_UNITS,
        player.profile.standing_height_units,
    )
}

fn sampled_ecb(samples: &[EcbDiamond], motion_frame: u8) -> EcbDiamond {
    let index = usize::from(motion_frame).min(samples.len().saturating_sub(1));
    samples[index]
}

fn local_ecb_to_world(local: EcbDiamond, root_position: Vec2, facing: i8) -> EcbDiamond {
    let top = local_point_to_world(local.top, root_position, facing);
    let bottom = local_point_to_world(local.bottom, root_position, facing);
    let side_a = local_point_to_world(local.right, root_position, facing);
    let side_b = local_point_to_world(local.left, root_position, facing);
    let (left, right) = if side_a.x <= side_b.x {
        (side_a, side_b)
    } else {
        (side_b, side_a)
    };

    EcbDiamond {
        top,
        right,
        bottom,
        left,
    }
}

fn local_point_to_world(local: Vec2, root_position: Vec2, facing: i8) -> Vec2 {
    let facing_sign = if facing < 0 { -1 } else { 1 };
    Vec2 {
        x: root_position.x + local.x * facing_sign,
        y: root_position.y + local.y,
    }
}

fn player_model_facing(player: &PlayerState) -> i8 {
    if player.motion_state == MotionState::TurnRun {
        player.turn_run_accel_mul
    } else {
        player.facing
    }
}

fn player_source_pose_motion_state(
    player: PlayerState,
    common_data: MeleeCommonData,
) -> MotionState {
    active_pose_motion_state(&player, common_data)
}

fn player_source_pose_frame(player: PlayerState) -> u8 {
    let frame = player_animation_pose_frame(player).saturating_add(1);
    if player.source_action_total_frames > 0 {
        frame.min(player.source_action_total_frames)
    } else {
        frame
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerRenderSnapshot {
    pub position: Vec2,
    pub source_position: SourceVec2,
    pub velocity: Vec2,
    pub active_ecb: EcbDiamond,
    pub grounded: bool,
    pub facing: i8,
    pub damage_percent: f32,
    pub damage_percent_temp: f32,
    pub damage_applied: u16,
    pub damage_knockback: f32,
    pub damage_angle: u16,
    pub damage_element: u8,
    pub hitlag_frames: u8,
    pub damage_hitstun_frames: u16,
    pub profile_weight: f32,
    pub melee_action_state_id: Option<MeleeActionStateId>,
    pub source_action_key: Option<SourceActionKey>,
    pub source_action_total_frames: u8,
    pub motion_state_alias: Option<MotionState>,
    pub motion_state: MotionState,
    pub state_frame: u8,
    pub animation_frame: u8,
    pub animation_frame_milli: i32,
    pub source_pose_action_state_id: Option<MeleeActionStateId>,
    pub source_pose_action_key: Option<SourceActionKey>,
    pub source_pose_motion_state: MotionState,
    pub source_pose_frame: u8,
    pub source_pose_model_facing: i8,
    pub ground_velocity_x: f32,
    pub ground_accel_x: f32,
    pub ground_accel_x2: f32,
    pub dash_entry_velocity_delta: f32,
    pub dash_x0: f32,
    pub walk_anim_velocity_x: f32,
    pub walk_accel_mul_milli: i32,
    pub turn_facing_after: i8,
    pub turn_has_turned: bool,
    pub turn_just_turned: bool,
    pub turn_frames_to_turn: u8,
    pub turn_dash_after_direction: i8,
    pub turn_latched_buttons: u8,
    pub run_no_interrupt_frames: u8,
    pub motion_cmd_var0: u32,
    pub motion_cmd_var1: u32,
    pub landing_lag_ticks: u8,
    pub run_brake_x0: bool,
    pub run_brake_frames_remaining: u8,
    pub turn_run_accel_mul: i8,
    pub turn_run_x14: bool,
    pub turn_run_resume_advances: bool,
    pub turn_run_completion_pending: bool,
    pub turn_run_completion_enters_run: bool,
    pub motion_anim_rate_milli: i32,
    pub entry_base_y: i32,
    pub entry_platform_offset_y: i32,
    pub entry_timer: u8,
    pub debug_input_facts: MeleeInputFacts,
}

impl PlayerRenderSnapshot {
    fn from_player(
        player: PlayerState,
        debug_input_facts: MeleeInputFacts,
        common_data: MeleeCommonData,
    ) -> Self {
        let source_pose_motion_state = player_source_pose_motion_state(player, common_data);
        let source_pose_binding = source_binding_for_motion_state(source_pose_motion_state);
        let source_pose_action_state_id = if player.motion_state_alias.is_some() {
            Some(melee_action_state_id_for_motion_state(
                source_pose_motion_state,
            ))
        } else {
            player.melee_action_state_id
        };
        let source_pose_action_key = if player.motion_state_alias.is_some() {
            source_pose_binding.map(|binding| binding.source_action_key)
        } else {
            player.source_action_key
        };
        Self {
            position: player.position,
            source_position: player.source_position,
            velocity: player.velocity,
            active_ecb: active_ecb_for_player(&player, common_data),
            grounded: player.grounded,
            facing: player.facing,
            damage_percent: player.damage_percent,
            damage_percent_temp: player.damage_percent_temp,
            damage_applied: player.damage_applied,
            damage_knockback: player.damage_knockback,
            damage_angle: player.damage_angle,
            damage_element: player.damage_element,
            hitlag_frames: player.hitlag_frames,
            damage_hitstun_frames: player.damage_hitstun_frames,
            profile_weight: player.profile.weight,
            melee_action_state_id: player.melee_action_state_id,
            source_action_key: player.source_action_key,
            source_action_total_frames: player.source_action_total_frames,
            motion_state_alias: player.motion_state_alias,
            motion_state: player.motion_state,
            state_frame: player.motion_frame,
            animation_frame: player_animation_pose_frame(player),
            animation_frame_milli: player_animation_pose_frame_milli(player),
            source_pose_action_state_id,
            source_pose_action_key,
            source_pose_motion_state,
            source_pose_frame: player_source_pose_frame(player),
            source_pose_model_facing: player_model_facing(&player),
            ground_velocity_x: player.ground_velocity_x,
            ground_accel_x: player.ground_accel_x,
            ground_accel_x2: player.ground_accel_x2,
            dash_entry_velocity_delta: player.dash_entry_velocity_delta,
            dash_x0: player.dash_x0,
            walk_anim_velocity_x: player.walk_anim_velocity_x,
            walk_accel_mul_milli: player.walk_accel_mul_milli,
            turn_facing_after: player.turn_facing_after,
            turn_has_turned: player.turn_has_turned,
            turn_just_turned: player.turn_just_turned,
            turn_frames_to_turn: player.turn_frames_to_turn,
            turn_dash_after_direction: player.turn_dash_after_direction,
            turn_latched_buttons: player.turn_latched_buttons,
            run_no_interrupt_frames: player.run_no_interrupt_frames,
            motion_cmd_var0: player.motion_cmd_var0,
            motion_cmd_var1: player.motion_cmd_var1,
            landing_lag_ticks: player.landing_lag_ticks,
            run_brake_x0: player.run_brake_x0,
            run_brake_frames_remaining: player.run_brake_frames_remaining,
            turn_run_accel_mul: player.turn_run_accel_mul,
            turn_run_x14: player.turn_run_x14,
            turn_run_resume_advances: player.turn_run_resume_advances,
            turn_run_completion_pending: player.turn_run_completion_pending,
            turn_run_completion_enters_run: player.turn_run_completion_enters_run,
            motion_anim_rate_milli: player.motion_anim_rate_milli,
            entry_base_y: player.entry_base_y,
            entry_platform_offset_y: player.entry_platform_offset_y,
            entry_timer: player.entry_timer,
            debug_input_facts,
        }
    }
}

fn player_animation_pose_frame(player: PlayerState) -> u8 {
    (player_animation_pose_frame_milli(player) / 1_000).clamp(0, u8::MAX as i32) as u8
}

fn player_animation_pose_frame_milli(player: PlayerState) -> i32 {
    match player.motion_state {
        MotionState::WalkSlow
        | MotionState::WalkMiddle
        | MotionState::WalkFast
        | MotionState::Run
        | MotionState::RunBrake
        | MotionState::TurnRun => player.motion_anim_frame_milli,
        _ => i32::from(player.motion_frame) * 1_000,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldSnapshot {
    pub frame: Frame,
    pub common_data: MeleeCommonData,
    pub players: [PlayerRenderSnapshot; PLAYER_COUNT],
    pub checksum: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourceCollisionStep {
    pub confirms: Vec<SourceHitConfirm>,
    pub stages: Vec<SourceDamageStage>,
    pub results: Vec<SourceDamageResult>,
    pub applied_stage_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SourceHitboxLogKey {
    attacker_index: usize,
    action_state_id: Option<MeleeActionStateId>,
    source_action_key: Option<SourceActionKey>,
    hit_group: u8,
    lifecycle_id: Option<SourceHitboxLifecycleId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SourceHitVictimLogEntry {
    hitbox: SourceHitboxLogKey,
    victim_index: usize,
}

macro_rules! player_rollback_snapshot_fields {
    ($emit:ident) => {
        $emit! {
            position: Vec2,
            source_position: SourceVec2,
            velocity: Vec2,
            source_self_velocity_x: f32,
            source_self_velocity_y: f32,
            player_nudge_x: f32,
            player_nudge_z: f32,
            ecb_bottom_offset_y: i32,
            ecb_bottom_lock_timer: u8,
            jumps_remaining: u8,
            grounded: bool,
            fast_falling: bool,
            facing: i8,
            attack_frame: u8,
            damage_percent: f32,
            damage_percent_temp: f32,
            damage_applied: u16,
            damage_knockback: f32,
            damage_angle: u16,
            damage_element: u8,
            hitlag_frames: u8,
            damage_hitstun_frames: u16,
            melee_action_state_id: Option<MeleeActionStateId>,
            source_action_key: Option<SourceActionKey>,
            source_action_total_frames: u8,
            source_down_bound_pose: Option<SourceDownBoundPose>,
            source_force_damage_down_bound: bool,
            source_down_bound_use_z_axis: bool,
            source_down_bound_reverse_face_up: bool,
            source_down_wait_timer: f32,
            source_lr_digital_press_timer: u8,
            source_previous_lr_digital_press_timer: u8,
            source_jab_followup_timer: u8,
            source_jab_followup_queued: bool,
            source_jab_combo_enabled: bool,
            motion_state_alias: Option<MotionState>,
            motion_state: MotionState,
            motion_frame: u8,
            motion_anim_frame_milli: i32,
            ground_velocity_x: f32,
            ground_accel_x: f32,
            ground_accel_x2: f32,
            dash_entry_velocity_delta: f32,
            dash_x0: f32,
            dash_started_from_tap: bool,
            walk_anim_velocity_x: f32,
            walk_accel_mul_milli: i32,
            turn_facing_after: i8,
            turn_has_turned: bool,
            turn_just_turned: bool,
            turn_frames_to_turn: u8,
            turn_dash_after_direction: i8,
            turn_latched_buttons: u8,
            turn_run_accel_mul: i8,
            run_no_interrupt_frames: u8,
            motion_cmd_var0: u32,
            motion_cmd_var1: u32,
            landing_lag_ticks: u8,
            run_brake_x0: bool,
            run_brake_frames_remaining: u8,
            turn_run_x14: bool,
            turn_run_resume_advances: bool,
            turn_run_completion_pending: bool,
            turn_run_completion_enters_run: bool,
            motion_anim_rate_milli: i32,
            shield_turn_facing_after: i8,
            shield_turn_frame: u8,
            guard_catch_dash_window: u8,
            jump_input: MeleeJumpInput,
            short_hop: bool,
            escape_air_iasa_timer: u8,
            floor_skip_surface: Option<u8>,
            platform_pass_pending: bool,
            platform_pass_timer: u8,
            entry_base_y: i32,
            entry_platform_offset_y: i32,
            entry_timer: u8,
        }
    };
}

macro_rules! define_player_rollback_snapshot {
    ($($field:ident: $field_type:ty,)+) => {
        #[derive(Debug, Clone, Copy, PartialEq)]
        struct PlayerRollbackSnapshot {
            $($field: $field_type,)+
        }

        impl PlayerRollbackSnapshot {
            fn from_player(player: PlayerState) -> Self {
                Self {
                    $($field: player.$field,)+
                }
            }

            fn restore_into(&self, player: &mut PlayerState) {
                $(player.$field = self.$field;)+
            }
        }
    };
}

player_rollback_snapshot_fields!(define_player_rollback_snapshot);

#[derive(Debug, Clone, PartialEq)]
pub struct WorldRollbackSnapshot {
    frame: Frame,
    players: [PlayerRollbackSnapshot; PLAYER_COUNT],
    previous_inputs: [PlayerInput; PLAYER_COUNT],
    input_timers: [MeleeInputTimers; PLAYER_COUNT],
    last_input_facts: [MeleeInputFacts; PLAYER_COUNT],
    source_hit_victim_log: Vec<SourceHitVictimLogEntry>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct World {
    frame: Frame,
    stage: StageProfile,
    common_data: MeleeCommonData,
    players: [PlayerState; PLAYER_COUNT],
    previous_inputs: [PlayerInput; PLAYER_COUNT],
    input_timers: [MeleeInputTimers; PLAYER_COUNT],
    last_input_facts: [MeleeInputFacts; PLAYER_COUNT],
    source_hit_victim_log: Vec<SourceHitVictimLogEntry>,
}

impl World {
    pub fn for_two_players() -> Self {
        Self::for_two_players_with_profiles([FighterProfile::FALCON_LIKE; PLAYER_COUNT])
    }

    pub fn for_two_players_with_profiles(profiles: [FighterProfile; PLAYER_COUNT]) -> Self {
        Self::for_two_players_on_stage_with_profiles(StageProfile::battlefield_test(), profiles)
    }

    pub fn for_two_players_with_common_data(common_data: MeleeCommonData) -> Self {
        Self::for_two_players_on_stage_with_profiles_and_common_data(
            StageProfile::battlefield_test(),
            [FighterProfile::FALCON_LIKE; PLAYER_COUNT],
            common_data,
        )
    }

    pub fn for_two_players_on_stage(stage: StageProfile) -> Self {
        Self::for_two_players_on_stage_with_profiles(
            stage,
            [FighterProfile::FALCON_LIKE; PLAYER_COUNT],
        )
    }

    pub fn for_two_players_on_stage_with_profiles(
        stage: StageProfile,
        profiles: [FighterProfile; PLAYER_COUNT],
    ) -> Self {
        Self::for_two_players_on_stage_with_profiles_and_common_data(
            stage,
            profiles,
            MeleeCommonData::provisional_mole(),
        )
    }

    pub fn for_two_players_on_stage_with_profiles_and_common_data(
        stage: StageProfile,
        profiles: [FighterProfile; PLAYER_COUNT],
        common_data: MeleeCommonData,
    ) -> Self {
        Self {
            frame: Frame(0),
            stage,
            common_data,
            players: [
                PlayerState::new_with_profile(PLAYER_ONE_DEFAULT_SPAWN_X, 0, 1, profiles[0]),
                PlayerState::new_with_profile(PLAYER_TWO_DEFAULT_SPAWN_X, 0, -1, profiles[1]),
            ],
            previous_inputs: [PlayerInput::neutral(), PlayerInput::neutral()],
            input_timers: [MeleeInputTimers::expired(); PLAYER_COUNT],
            last_input_facts: [MeleeInputFacts::default(); PLAYER_COUNT],
            source_hit_victim_log: Vec::new(),
        }
    }

    pub fn for_slippi_battlefield_singles_match_start() -> Self {
        let stage = StageProfile::battlefield_test();
        let mut world = Self::for_two_players_on_stage_with_profiles_and_common_data(
            stage,
            [FighterProfile::FALCON_LIKE; PLAYER_COUNT],
            MeleeCommonData::provisional_mole(),
        );

        for (index, player) in world.players.iter_mut().enumerate() {
            let spawn = stage.spawn_points[index];
            *player = PlayerState::new_with_profile(
                spawn.x,
                spawn.y,
                spawn.facing,
                FighterProfile::FALCON_LIKE,
            );
            player.set_motion_state_alias(MotionState::Entry);
            player.entry_timer = 5 * (index as u8 + 1);
            player.grounded = false;
        }

        world
    }

    pub const fn frame(&self) -> Frame {
        self.frame
    }

    pub(crate) fn set_frame(&mut self, frame: Frame) {
        self.frame = frame;
    }

    pub const fn stage(&self) -> StageProfile {
        self.stage
    }

    pub const fn common_data(&self) -> MeleeCommonData {
        self.common_data
    }

    pub const fn players(&self) -> &[PlayerState; PLAYER_COUNT] {
        &self.players
    }

    pub(crate) fn players_mut(&mut self) -> &mut [PlayerState; PLAYER_COUNT] {
        &mut self.players
    }

    pub(crate) const fn previous_inputs(&self) -> &[PlayerInput; PLAYER_COUNT] {
        &self.previous_inputs
    }

    pub(crate) fn set_previous_inputs(&mut self, inputs: [PlayerInput; PLAYER_COUNT]) {
        self.previous_inputs = inputs;
    }

    pub const fn input_timers(&self) -> &[MeleeInputTimers; PLAYER_COUNT] {
        &self.input_timers
    }

    pub(crate) fn set_input_timers(&mut self, timers: [MeleeInputTimers; PLAYER_COUNT]) {
        self.input_timers = timers;
    }

    pub const fn last_input_facts(&self) -> &[MeleeInputFacts; PLAYER_COUNT] {
        &self.last_input_facts
    }

    pub(crate) fn set_last_input_facts(&mut self, facts: [MeleeInputFacts; PLAYER_COUNT]) {
        self.last_input_facts = facts;
    }

    pub fn rollback_snapshot(&self) -> WorldRollbackSnapshot {
        WorldRollbackSnapshot {
            frame: self.frame,
            players: self.players.map(PlayerRollbackSnapshot::from_player),
            previous_inputs: self.previous_inputs,
            input_timers: self.input_timers,
            last_input_facts: self.last_input_facts,
            source_hit_victim_log: self.source_hit_victim_log.clone(),
        }
    }

    pub fn restore_rollback_snapshot(&mut self, snapshot: &WorldRollbackSnapshot) {
        self.frame = snapshot.frame;
        for (player, player_snapshot) in self.players.iter_mut().zip(snapshot.players) {
            player_snapshot.restore_into(player);
        }
        self.previous_inputs = snapshot.previous_inputs;
        self.input_timers = snapshot.input_timers;
        self.last_input_facts = snapshot.last_input_facts;
        self.source_hit_victim_log = snapshot.source_hit_victim_log.clone();
    }

    pub fn set_player_state_for_diagnostic(
        &mut self,
        player_index: usize,
        mut state: PlayerState,
    ) -> bool {
        let Some(player) = self.players.get_mut(player_index) else {
            return false;
        };
        if state.motion_state_alias.is_some() {
            state.set_motion_state_alias(state.motion_state);
        }
        if state.source_position.to_milli() != state.position {
            state.source_position = SourceVec2::from_milli(state.position);
        }
        if !state.grounded
            || state
                .melee_action_state_id
                .is_some_and(is_source_damage_action_state_id)
        {
            if source_units_to_milli(state.source_self_velocity_x) != state.velocity.x {
                state.source_self_velocity_x = milli_to_source_units(state.velocity.x);
            }
            if source_units_to_milli(state.source_self_velocity_y) != state.velocity.y {
                state.source_self_velocity_y = milli_to_source_units(state.velocity.y);
            }
        }
        *player = state;
        true
    }

    pub fn apply_source_damage_stages(&mut self, stages: &[SourceDamageStage]) -> usize {
        let mut applied = 0;
        for stage in stages {
            let Some(player) = self.players.get_mut(stage.victim_index) else {
                continue;
            };
            player.damage_percent_temp += stage.damage;
            if stage.env_damage > player.damage_applied {
                player.damage_applied = stage.env_damage;
            }
            applied += 1;
        }
        applied
    }

    pub fn apply_source_collision_frame(
        &mut self,
        collision_frame: &SourceCollisionFrame,
    ) -> SourceCollisionStep {
        self.retain_source_hit_victim_log_for_frame(collision_frame);
        let confirms =
            self.source_hit_confirms_not_in_victim_log(source_hit_confirms(collision_frame));
        let stages = source_damage_stages_from_confirms(&confirms);
        let results = self.source_damage_results_for_stages(&stages);
        let applied_stage_count = self.apply_source_damage_stages(&stages);
        SourceCollisionStep {
            confirms,
            stages,
            results,
            applied_stage_count,
        }
    }

    fn retain_source_hit_victim_log_for_frame(&mut self, collision_frame: &SourceCollisionFrame) {
        let active_hitboxes = collision_frame
            .hits
            .iter()
            .filter_map(|hit| hit.hitbox.map(|hitbox| source_hitbox_log_key(*hit, hitbox)))
            .collect::<Vec<_>>();
        self.source_hit_victim_log
            .retain(|entry| active_hitboxes.contains(&entry.hitbox));
    }

    fn source_hit_confirms_not_in_victim_log(
        &mut self,
        confirms: Vec<SourceHitConfirm>,
    ) -> Vec<SourceHitConfirm> {
        let mut accepted = Vec::new();
        for confirm in confirms {
            let hitbox = source_hitbox_log_key(confirm.collision.hit, confirm.hitbox);
            let entry = SourceHitVictimLogEntry {
                hitbox,
                victim_index: confirm.victim_index,
            };
            if self.source_hit_victim_log.contains(&entry) {
                continue;
            }
            self.source_hit_victim_log.push(entry);
            accepted.push(confirm);
        }
        accepted
    }

    pub fn source_damage_results_for_stages(
        &self,
        stages: &[SourceDamageStage],
    ) -> Vec<SourceDamageResult> {
        let mut results = Vec::new();
        for (victim_index, player) in self.players.iter().enumerate() {
            let accumulator = source_damage_accumulator_after_stages(
                SourceDamageAccumulator {
                    victim_index,
                    percent_temp: player.damage_percent_temp,
                    applied_damage: player.damage_applied,
                },
                stages,
            );
            if let Some(result) = source_damage_result_for_victim(
                self.common_data,
                stages,
                SourceDamageResultInput {
                    victim_index,
                    victim_percent: player.damage_percent,
                    victim_percent_temp: accumulator.percent_temp,
                    victim_weight: player.profile.weight,
                    stage: 1.0,
                    attack: 1.0,
                    defense: 1.0,
                },
            ) {
                results.push(result);
            }
        }
        results
    }

    pub fn apply_source_damage_results(&mut self, results: &[SourceDamageResult]) -> usize {
        self.apply_source_damage_results_with_action_total_frames(results, |_| None)
    }

    pub fn apply_source_damage_results_with_action_total_frames(
        &mut self,
        results: &[SourceDamageResult],
        action_total_frames: impl Fn(MeleeActionStateId) -> Option<u8>,
    ) -> usize {
        let mut applied = 0;
        for result in results {
            let Some(attacker) = self.players.get(result.stage.attacker_index).copied() else {
                continue;
            };
            let Some(victim) = self.players.get_mut(result.stage.victim_index) else {
                continue;
            };
            let angle = source_damage_angle_radians(self.common_data, *result, victim.grounded);
            let speed = result.knockback * self.common_data.damage_knockback_velocity_scale;
            let direction = source_damage_launch_direction(attacker.position, victim.position);
            let velocity_x = speed * angle.cos().abs() * direction;
            let velocity_y = speed * angle.sin();

            victim.source_self_velocity_x = velocity_x;
            victim.source_self_velocity_y = velocity_y;
            victim.velocity.x = source_units_to_milli(velocity_x);
            victim.velocity.y = source_units_to_milli(velocity_y);
            victim.ground_velocity_x = 0.0;
            victim.ground_accel_x = 0.0;
            victim.ground_accel_x2 = 0.0;
            if victim.velocity.y > 0 {
                victim.grounded = false;
            }
            victim.damage_knockback = result.knockback;
            victim.damage_angle = result.angle;
            victim.damage_element = result.element;
            victim.hitlag_frames = source_damage_hitlag_frames(self.common_data, *result);
            victim.damage_hitstun_frames = source_damage_hitstun_frames(self.common_data, *result);
            let damage_action_state_id =
                source_damage_action_state_id(self.common_data, *result, victim.grounded);
            victim.melee_action_state_id = Some(damage_action_state_id);
            victim.source_action_key = None;
            victim.source_action_total_frames =
                action_total_frames(damage_action_state_id).unwrap_or(0);
            victim.source_down_bound_pose = None;
            victim.source_down_wait_timer = 0.0;
            victim.motion_state_alias = None;
            victim.motion_frame = 0;
            applied += 1;
        }
        applied
    }

    pub fn commit_staged_source_damage(&mut self) -> usize {
        let mut committed = 0;
        for player in &mut self.players {
            if player.damage_percent_temp == 0.0 && player.damage_applied == 0 {
                continue;
            }
            player.damage_percent = (player.damage_percent + player.damage_percent_temp).min(999.0);
            player.damage_percent_temp = 0.0;
            player.damage_applied = 0;
            committed += 1;
        }
        committed
    }

    pub fn set_input_history_for_diagnostic(
        &mut self,
        previous_inputs: [PlayerInput; PLAYER_COUNT],
        input_timers: [MeleeInputTimers; PLAYER_COUNT],
    ) {
        self.previous_inputs = previous_inputs;
        self.input_timers = input_timers;
    }

    pub fn melee_input_snapshot(
        &self,
        player_index: usize,
        current_input: PlayerInput,
    ) -> Option<MeleeInputSnapshot> {
        let previous = *self.previous_inputs.get(player_index)?;
        let timers = *self.input_timers.get(player_index)?;

        Some(current_input.melee_snapshot_with_config(
            previous,
            timers,
            self.common_data.input_config(),
        ))
    }

    pub fn snapshot(&self) -> WorldSnapshot {
        WorldSnapshot {
            frame: self.frame,
            common_data: self.common_data,
            players: [
                PlayerRenderSnapshot::from_player(
                    self.players[0],
                    self.last_input_facts[0],
                    self.common_data,
                ),
                PlayerRenderSnapshot::from_player(
                    self.players[1],
                    self.last_input_facts[1],
                    self.common_data,
                ),
            ],
            checksum: self.checksum(),
        }
    }

    pub fn checksum(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        mix_u32(&mut hash, self.frame.0);
        mix_stage_profile(&mut hash, self.stage);
        mix_common_data(&mut hash, self.common_data);
        for player in self.players {
            mix_fighter_profile(&mut hash, player.profile);
            mix_i32(&mut hash, player.position.x);
            mix_i32(&mut hash, player.position.y);
            mix_f32(&mut hash, player.source_position.x);
            mix_f32(&mut hash, player.source_position.y);
            mix_i32(&mut hash, player.velocity.x);
            mix_i32(&mut hash, player.velocity.y);
            mix_f32(&mut hash, player.source_self_velocity_x);
            mix_f32(&mut hash, player.source_self_velocity_y);
            mix_f32(&mut hash, player.player_nudge_x);
            mix_f32(&mut hash, player.player_nudge_z);
            mix_i32(&mut hash, player.ecb_bottom_offset_y);
            mix_u8(&mut hash, player.ecb_bottom_lock_timer);
            mix_u8(&mut hash, player.jumps_remaining);
            mix_u8(&mut hash, player.grounded as u8);
            mix_u8(&mut hash, player.fast_falling as u8);
            mix_u8(&mut hash, player.facing as u8);
            mix_u8(&mut hash, player.attack_frame);
            mix_f32(&mut hash, player.damage_percent);
            mix_f32(&mut hash, player.damage_percent_temp);
            mix_u32(&mut hash, player.damage_applied as u32);
            mix_f32(&mut hash, player.damage_knockback);
            mix_u32(&mut hash, player.damage_angle as u32);
            mix_u8(&mut hash, player.damage_element);
            mix_u8(&mut hash, player.hitlag_frames);
            mix_u32(&mut hash, player.damage_hitstun_frames as u32);
            mix_u8(&mut hash, motion_state_id(player.motion_state));
            mix_optional_action_state_id(&mut hash, player.melee_action_state_id);
            mix_optional_source_action_key(&mut hash, player.source_action_key);
            mix_u8(&mut hash, player.source_action_total_frames);
            mix_optional_source_down_bound_pose(&mut hash, player.source_down_bound_pose);
            mix_u8(&mut hash, player.source_force_damage_down_bound as u8);
            mix_u8(&mut hash, player.source_down_bound_use_z_axis as u8);
            mix_u8(&mut hash, player.source_down_bound_reverse_face_up as u8);
            mix_f32(&mut hash, player.source_down_wait_timer);
            mix_u8(&mut hash, player.source_lr_digital_press_timer);
            mix_u8(&mut hash, player.source_previous_lr_digital_press_timer);
            mix_u8(&mut hash, player.source_jab_followup_timer);
            mix_u8(&mut hash, player.source_jab_followup_queued as u8);
            mix_u8(&mut hash, player.source_jab_combo_enabled as u8);
            mix_optional_motion_state(&mut hash, player.motion_state_alias);
            mix_u8(&mut hash, player.motion_frame);
            mix_i32(&mut hash, player.motion_anim_frame_milli);
            mix_f32(&mut hash, player.ground_velocity_x);
            mix_f32(&mut hash, player.ground_accel_x);
            mix_f32(&mut hash, player.ground_accel_x2);
            mix_f32(&mut hash, player.dash_entry_velocity_delta);
            mix_f32(&mut hash, player.dash_x0);
            mix_u8(&mut hash, player.dash_started_from_tap as u8);
            mix_f32(&mut hash, player.walk_anim_velocity_x);
            mix_i32(&mut hash, player.walk_accel_mul_milli);
            mix_u8(&mut hash, player.turn_facing_after as u8);
            mix_u8(&mut hash, player.turn_has_turned as u8);
            mix_u8(&mut hash, player.turn_just_turned as u8);
            mix_u8(&mut hash, player.turn_frames_to_turn);
            mix_u8(&mut hash, player.turn_dash_after_direction as u8);
            mix_u8(&mut hash, player.turn_latched_buttons);
            mix_u8(&mut hash, player.turn_run_accel_mul as u8);
            mix_u8(&mut hash, player.run_no_interrupt_frames);
            mix_u32(&mut hash, player.motion_cmd_var0);
            mix_u32(&mut hash, player.motion_cmd_var1);
            mix_u8(&mut hash, player.landing_lag_ticks);
            mix_u8(&mut hash, player.run_brake_x0 as u8);
            mix_u8(&mut hash, player.run_brake_frames_remaining);
            mix_u8(&mut hash, player.turn_run_x14 as u8);
            mix_u8(&mut hash, player.turn_run_resume_advances as u8);
            mix_u8(&mut hash, player.turn_run_completion_pending as u8);
            mix_u8(&mut hash, player.turn_run_completion_enters_run as u8);
            mix_i32(&mut hash, player.motion_anim_rate_milli);
            mix_u8(&mut hash, player.shield_turn_facing_after as u8);
            mix_u8(&mut hash, player.shield_turn_frame);
            mix_u8(&mut hash, player.guard_catch_dash_window);
            mix_u8(&mut hash, jump_input_id(player.jump_input));
            mix_u8(&mut hash, player.short_hop as u8);
            mix_u8(&mut hash, player.escape_air_iasa_timer);
            mix_u8(&mut hash, player.floor_skip_surface.unwrap_or(u8::MAX));
            mix_u8(&mut hash, player.platform_pass_pending as u8);
            mix_u8(&mut hash, player.platform_pass_timer);
            mix_i32(&mut hash, player.entry_base_y);
            mix_i32(&mut hash, player.entry_platform_offset_y);
            mix_u8(&mut hash, player.entry_timer);
        }
        for input in self.previous_inputs {
            mix_u64(&mut hash, input.bits());
        }
        for timer in self.input_timers {
            mix_u8(&mut hash, timer.x_tap);
            mix_u8(&mut hash, timer.y_tap);
            mix_u8(&mut hash, timer.trigger);
        }
        mix_u32(&mut hash, self.source_hit_victim_log.len() as u32);
        for entry in &self.source_hit_victim_log {
            mix_u32(&mut hash, entry.hitbox.attacker_index as u32);
            mix_optional_action_state_id(&mut hash, entry.hitbox.action_state_id);
            mix_optional_source_action_key(&mut hash, entry.hitbox.source_action_key);
            mix_u8(&mut hash, entry.hitbox.hit_group);
            mix_optional_source_hitbox_lifecycle_id(&mut hash, entry.hitbox.lifecycle_id);
            mix_u32(&mut hash, entry.victim_index as u32);
        }
        hash
    }
}

fn source_hitbox_log_key(
    hit: SourceCollisionCapsule,
    hitbox: SourceHitboxAttributes,
) -> SourceHitboxLogKey {
    SourceHitboxLogKey {
        attacker_index: hit.owner_index,
        action_state_id: hit.action_state_id,
        source_action_key: hit.source_action_key,
        hit_group: hitbox.hit_group,
        lifecycle_id: hit.hitbox_lifecycle_id,
    }
}

fn source_damage_launch_direction(attacker_position: Vec2, victim_position: Vec2) -> f32 {
    if victim_position.x < attacker_position.x {
        -1.0
    } else {
        1.0
    }
}

fn source_damage_angle_radians(
    common_data: MeleeCommonData,
    result: SourceDamageResult,
    victim_grounded: bool,
) -> f32 {
    if result.angle != 361 {
        return (result.angle as f32).to_radians();
    }

    if !victim_grounded {
        return common_data.damage_sakurai_air_angle_radians;
    }

    if result.knockback < common_data.damage_sakurai_ground_min_knockback {
        return 0.0;
    }

    let range = common_data.damage_sakurai_ground_max_knockback
        - common_data.damage_sakurai_ground_min_knockback;
    if range <= 0.0 {
        return common_data.damage_sakurai_ground_angle_degrees.to_radians();
    }

    let degrees = common_data.damage_sakurai_ground_angle_degrees
        * ((result.knockback - common_data.damage_sakurai_ground_min_knockback) / range)
        + 1.0;
    degrees
        .min(common_data.damage_sakurai_ground_angle_degrees)
        .to_radians()
}

fn source_damage_motion_tier(common_data: MeleeCommonData, knockback: f32) -> usize {
    let scaled = knockback * common_data.damage_duration_scale;
    if scaled < common_data.damage_motion_tier_1_threshold {
        0
    } else if scaled < common_data.damage_motion_tier_2_threshold {
        1
    } else if scaled < common_data.damage_motion_tier_3_threshold {
        2
    } else {
        3
    }
}

fn source_damage_direction_index(angle_radians: f32) -> usize {
    let sin = angle_radians.sin();
    let cos = angle_radians.cos().abs();
    if sin > cos {
        2
    } else if -sin > cos {
        0
    } else {
        1
    }
}

fn source_damage_action_state_id(
    common_data: MeleeCommonData,
    result: SourceDamageResult,
    victim_grounded: bool,
) -> MeleeActionStateId {
    const GROUND_DAMAGE_STATES: [[u16; 3]; 4] =
        [[81, 78, 75], [82, 79, 76], [83, 80, 77], [89, 88, 87]];
    const AIR_DAMAGE_STATES: [[u16; 3]; 4] =
        [[84, 84, 84], [85, 85, 85], [86, 86, 86], [89, 88, 87]];

    let angle = source_damage_angle_radians(common_data, result, victim_grounded);
    let tier = source_damage_motion_tier(common_data, result.knockback);
    let direction = source_damage_direction_index(angle);
    let table = if victim_grounded {
        GROUND_DAMAGE_STATES
    } else {
        AIR_DAMAGE_STATES
    };
    MeleeActionStateId::new(table[tier][direction])
}

fn source_damage_hitlag_frames(common_data: MeleeCommonData, result: SourceDamageResult) -> u8 {
    let raw = (result.stage.damage * common_data.hitlag_damage_scale
        + common_data.hitlag_base_frames) as i32;
    raw.clamp(0, common_data.hitlag_max_frames as i32) as u8
}

fn source_damage_hitstun_frames(common_data: MeleeCommonData, result: SourceDamageResult) -> u16 {
    let raw = (result.knockback * common_data.damage_duration_scale) as i32;
    raw.max(1).min(u16::MAX as i32) as u16
}

const fn motion_state_id(state: MotionState) -> u8 {
    match state {
        MotionState::Wait => 0,
        MotionState::Entry => 53,
        MotionState::EntryStart => 54,
        MotionState::EntryEnd => 55,
        MotionState::WalkSlow => 1,
        MotionState::WalkMiddle => 2,
        MotionState::WalkFast => 3,
        MotionState::Dash => 4,
        MotionState::Run => 5,
        MotionState::RunDirect => 71,
        MotionState::RunBrake => 6,
        MotionState::TurnRun => 7,
        MotionState::Turn => 8,
        MotionState::Squat => 9,
        MotionState::SquatWait => 51,
        MotionState::SquatRv => 52,
        MotionState::SpecialN => 10,
        MotionState::Catch => 11,
        MotionState::Attack1 => 12,
        MotionState::AttackS3 => 13,
        MotionState::AttackHi3 => 14,
        MotionState::AttackLw3 => 15,
        MotionState::AttackS4 => 16,
        MotionState::AttackHi4 => 17,
        MotionState::AttackLw4 => 18,
        MotionState::KneeBend => 19,
        MotionState::Fall => 50,
        MotionState::FallF => 57,
        MotionState::FallB => 58,
        MotionState::FallAerial => 59,
        MotionState::FallAerialF => 60,
        MotionState::FallAerialB => 61,
        MotionState::Guard => 21,
        MotionState::EscapeAir => 22,
        MotionState::SpecialSStart => 23,
        MotionState::SpecialS => 69,
        MotionState::SpecialHi => 24,
        MotionState::SpecialLw => 25,
        MotionState::SpecialAirN => 26,
        MotionState::SpecialAirSStart => 27,
        MotionState::SpecialAirS => 70,
        MotionState::SpecialAirHi => 28,
        MotionState::SpecialAirLw => 29,
        MotionState::AttackAirN => 30,
        MotionState::AttackAirF => 31,
        MotionState::AttackAirB => 32,
        MotionState::AttackAirHi => 33,
        MotionState::AttackAirLw => 34,
        MotionState::LandingAirN => 64,
        MotionState::LandingAirF => 65,
        MotionState::LandingAirB => 66,
        MotionState::LandingAirHi => 67,
        MotionState::LandingAirLw => 68,
        MotionState::EscapeN => 35,
        MotionState::EscapeF => 36,
        MotionState::EscapeB => 37,
        MotionState::GuardOff => 38,
        MotionState::GuardSetOff => 72,
        MotionState::GuardReflect => 56,
        MotionState::LandingFallSpecial => 39,
        MotionState::FallSpecial => 40,
        MotionState::FallSpecialF => 62,
        MotionState::FallSpecialB => 63,
        MotionState::CatchDash => 41,
        MotionState::AttackDash => 42,
        MotionState::GuardOn => 43,
        MotionState::JumpAerialF => 44,
        MotionState::JumpAerialB => 45,
        MotionState::JumpF => 46,
        MotionState::JumpB => 47,
        MotionState::Landing => 48,
        MotionState::Pass => 49,
    }
}

const fn jump_input_id(input: MeleeJumpInput) -> u8 {
    match input {
        MeleeJumpInput::None => 0,
        MeleeJumpInput::LStick => 1,
        MeleeJumpInput::XY => 2,
        MeleeJumpInput::CStick => 3,
    }
}

fn mix_u8(hash: &mut u64, value: u8) {
    *hash ^= value as u64;
    *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
}

fn mix_u32(hash: &mut u64, value: u32) {
    for byte in value.to_le_bytes() {
        mix_u8(hash, byte);
    }
}

fn mix_f32(hash: &mut u64, value: f32) {
    mix_u32(hash, value.to_bits());
}

fn mix_u64(hash: &mut u64, value: u64) {
    for byte in value.to_le_bytes() {
        mix_u8(hash, byte);
    }
}

fn mix_str(hash: &mut u64, value: &str) {
    for byte in value.as_bytes() {
        mix_u8(hash, *byte);
    }
}

fn mix_optional_action_state_id(hash: &mut u64, value: Option<MeleeActionStateId>) {
    match value {
        Some(value) => {
            mix_u8(hash, 1);
            mix_u32(hash, u32::from(value.get()));
        }
        None => mix_u8(hash, 0),
    }
}

fn mix_optional_source_action_key(hash: &mut u64, value: Option<SourceActionKey>) {
    match value {
        Some(value) => {
            mix_u8(hash, 1);
            mix_str(hash, value.as_str());
        }
        None => mix_u8(hash, 0),
    }
}

fn mix_optional_source_down_bound_pose(hash: &mut u64, value: Option<SourceDownBoundPose>) {
    match value {
        Some(value) => {
            mix_u8(hash, 1);
            mix_f32(hash, value.hip_mtx_0_1);
            mix_f32(hash, value.hip_mtx_0_2);
            mix_f32(hash, value.hip_mtx_1_1);
            mix_f32(hash, value.hip_mtx_1_2);
        }
        None => mix_u8(hash, 0),
    }
}

fn mix_optional_source_hitbox_lifecycle_id(hash: &mut u64, value: Option<SourceHitboxLifecycleId>) {
    match value {
        Some(value) => {
            mix_u8(hash, 1);
            mix_u64(hash, value.get());
        }
        None => mix_u8(hash, 0),
    }
}

fn mix_optional_motion_state(hash: &mut u64, value: Option<MotionState>) {
    match value {
        Some(value) => {
            mix_u8(hash, 1);
            mix_u8(hash, motion_state_id(value));
        }
        None => mix_u8(hash, 0),
    }
}

fn mix_stage_profile(hash: &mut u64, stage: StageProfile) {
    mix_str(hash, stage.name);
    mix_stage_surface(hash, stage.main_floor);
    for surface in stage.soft_platforms {
        mix_stage_surface(hash, surface);
    }
    mix_i32(hash, stage.blast_zones.left_x);
    mix_i32(hash, stage.blast_zones.right_x);
    mix_i32(hash, stage.blast_zones.top_y);
    mix_i32(hash, stage.blast_zones.bottom_y);
    for spawn in stage.spawn_points {
        mix_i32(hash, spawn.x);
        mix_i32(hash, spawn.y);
        mix_u8(hash, spawn.facing as u8);
    }
}

fn mix_stage_surface(hash: &mut u64, surface: StageSurface) {
    mix_str(hash, surface.name);
    mix_u8(hash, stage_surface_kind_id(surface.kind));
    mix_i32(hash, surface.left_x);
    mix_i32(hash, surface.right_x);
    mix_i32(hash, surface.y);
    mix_f32(hash, surface.friction_multiplier);
}

fn stage_surface_kind_id(kind: StageSurfaceKind) -> u8 {
    match kind {
        StageSurfaceKind::Solid => 0,
        StageSurfaceKind::Soft => 1,
    }
}

fn mix_common_data(hash: &mut u64, common: MeleeCommonData) {
    mix_u8(hash, common.tap_x_threshold as u8);
    mix_u8(hash, common.tap_y_threshold as u8);
    mix_u8(hash, common.trigger_threshold);
    mix_u8(hash, common.trigger_timer_threshold);
    mix_u8(hash, common.main_stick_deadzone_x as u8);
    mix_u8(hash, common.main_stick_deadzone_y as u8);
    mix_u8(hash, common.c_stick_deadzone_x as u8);
    mix_u8(hash, common.c_stick_deadzone_y as u8);
    mix_u8(hash, common.trigger_deadzone);
    mix_u8(hash, common.z_shield_analog);
    mix_u8(hash, common.walk_x as u8);
    mix_u8(hash, common.walk_slow_x as u8);
    mix_u8(hash, common.walk_middle_x as u8);
    mix_u8(hash, common.walk_fast_x as u8);
    mix_u8(hash, common.dash_x as u8);
    mix_u8(hash, common.dash_tap_window);
    mix_u8(hash, common.turn_x as u8);
    mix_u8(hash, common.turn_run_x as u8);
    mix_u8(hash, common.tilt_x as u8);
    mix_u8(hash, common.tilt_y as u8);
    mix_u8(hash, common.smash_y as u8);
    mix_u8(hash, common.crouch_y as u8);
    mix_u8(hash, common.crouch_release_y as u8);
    mix_u8(hash, common.tap_jump_y as u8);
    mix_u8(hash, common.tap_jump_window);
    mix_u8(hash, common.tap_jump_release_y as u8);
    mix_u8(hash, common.fast_fall_y as u8);
    mix_u8(hash, common.fast_fall_window);
    mix_u8(hash, common.lcancel_window);
    mix_f32(hash, common.lcancel_divisor);
    mix_f32(hash, common.knockback_weight_multiplier);
    mix_f32(hash, common.knockback_decay);
    mix_f32(hash, common.knockback_cap);
    mix_f32(hash, common.knockback_damage_scale);
    mix_f32(hash, common.knockback_hit_count_scale);
    mix_f32(hash, common.knockback_weight_set_damage);
    mix_f32(hash, common.knockback_result_scale);
    mix_f32(hash, common.knockback_result_offset);
    mix_f32(hash, common.damage_knockback_velocity_scale);
    mix_f32(hash, common.damage_sakurai_air_angle_radians);
    mix_f32(hash, common.damage_sakurai_ground_angle_degrees);
    mix_f32(hash, common.damage_sakurai_ground_min_knockback);
    mix_f32(hash, common.damage_sakurai_ground_max_knockback);
    mix_f32(hash, common.damage_duration_scale);
    mix_f32(hash, common.damage_motion_tier_1_threshold);
    mix_f32(hash, common.damage_motion_tier_2_threshold);
    mix_f32(hash, common.damage_motion_tier_3_threshold);
    mix_f32(hash, common.damage_landing_down_bound_knockback_threshold);
    mix_f32(hash, common.damage_landing_basic_knockback_threshold);
    mix_u8(hash, common.passive_input_age_threshold);
    mix_f32(hash, common.passive_window_max);
    mix_f32(hash, common.passive_stand_stick_x);
    mix_i32(hash, i32::from(common.down_stand_stick_y));
    mix_f32(hash, common.down_wait_timer);
    mix_f32(hash, common.hitlag_max_frames);
    mix_f32(hash, common.hitlag_damage_scale);
    mix_f32(hash, common.hitlag_base_frames);
    mix_f32(hash, common.hitlag_crouch_multiplier);
    mix_u8(hash, common.c_stick as u8);
    mix_u8(hash, common.aerial_neutral_x as u8);
    mix_u8(hash, common.aerial_neutral_y as u8);
    mix_i32(hash, common.aerial_vertical_angle_tan_milli);
    mix_u8(hash, common.air_jump_backward_x as u8);
    mix_u8(hash, common.escape_x as u8);
    mix_u8(hash, common.escape_x_tap_window);
    mix_u8(hash, common.escape_y as u8);
    mix_u8(hash, common.escape_y_tap_window);
    mix_u8(hash, common.special_side_x as u8);
    mix_u8(hash, common.special_vertical_y as u8);
    mix_u8(hash, common.escapeair_iasa_timer_ticks);
    mix_u8(hash, common.escapeair_animation_ticks);
    mix_u8(hash, common.escapeair_deadzone_x as u8);
    mix_u8(hash, common.escapeair_deadzone_y as u8);
    mix_f32(hash, common.escapeair_force);
    mix_f32(hash, common.escapeair_decay);
    mix_u8(hash, common.escapeair_landing_lag_ticks);
    mix_f32(hash, common.walk_middle_velocity_ratio);
    mix_f32(hash, common.walk_fast_velocity_ratio);
    mix_f32(hash, common.walk_accel_taper);
    mix_f32(hash, common.run_accel_taper);
    mix_f32(hash, common.run_ground_friction_multiplier);
    mix_f32(hash, common.high_speed_ground_friction_multiplier);
    mix_f32(hash, common.run_brake_animation_pause_velocity);
    mix_f32(hash, common.animation_velocity_scale);
    mix_f32(hash, common.fall_animation_drift_threshold);
    mix_f32(hash, common.fall_animation_blend);
    mix_f32(hash, common.player_nudge_x);
    mix_f32(hash, common.player_nudge_z);
    mix_f32(hash, common.player_nudge_z_clamp);
    mix_f32(hash, common.transformed_player_nudge_z);
    mix_f32(hash, common.transformed_player_nudge_z_clamp);
    mix_u8(hash, common.fallspecial_platform_landing_y as u8);
    mix_u8(hash, common.platform_pass_y as u8);
    mix_u8(hash, common.platform_pass_y_tap_window);
    mix_f32(hash, common.pass_initial_y_velocity);
    mix_u8(hash, common.platform_drop_delay_ticks);
    mix_u8(hash, common.entry_start_ticks);
    mix_u8(hash, common.entry_end_ticks);
    mix_f32(hash, common.entry_initial_scale_y);
    mix_u8(hash, common.entry_collision_landing_lag_ticks);
    mix_u8(hash, common.dash_early_action_window);
    mix_u8(hash, common.dash_defensive_action_window);
    mix_u8(hash, common.dash_late_action_window);
    mix_f32(hash, common.dash_velocity_decay);
    mix_u8(hash, common.run_x as u8);
    mix_u8(hash, common.guard_on_catch_dash_window);
    mix_u8(hash, common.guard_reflect_input_window);
    mix_u8(hash, common.run_turn_run_no_interrupt_frames);
}

fn mix_fighter_profile(hash: &mut u64, profile: FighterProfile) {
    mix_f32(hash, profile.walk_initial_velocity);
    mix_fighter_action_frames(hash, profile.action_frames);
    mix_f32(hash, profile.walk_accel);
    mix_f32(hash, profile.walk_max_velocity);
    mix_f32(hash, profile.slow_walk_max_velocity);
    mix_f32(hash, profile.mid_walk_point);
    mix_f32(hash, profile.fast_walk_min);
    mix_f32(hash, profile.run_animation_scaling);
    mix_f32(hash, profile.dash_initial_velocity);
    mix_f32(hash, profile.dash_run_acceleration_a);
    mix_f32(hash, profile.dash_run_acceleration_b);
    mix_f32(hash, profile.dash_run_terminal_velocity);
    match profile.max_run_brake_frames {
        Some(frames) => {
            mix_u8(hash, 1);
            mix_u8(hash, frames);
        }
        None => mix_u8(hash, 0),
    }
    mix_f32(hash, profile.ground_friction);
    mix_f32(hash, profile.ground_max_horizontal_velocity);
    mix_f32(hash, profile.ground_to_air_jump_momentum_multiplier);
    mix_f32(hash, profile.jump_horizontal_initial_velocity);
    mix_f32(hash, profile.jump_horizontal_max_velocity);
    mix_f32(hash, profile.air_jump_horizontal_multiplier);
    mix_f32(hash, profile.air_jump_vertical_multiplier);
    mix_u8(hash, profile.max_jumps);
    mix_f32(hash, profile.air_drift_stick_multiplier);
    mix_f32(hash, profile.aerial_drift_base);
    mix_f32(hash, profile.air_drift_max);
    mix_f32(hash, profile.aerial_friction);
    mix_f32(hash, profile.air_max_horizontal_velocity);
    mix_f32(hash, profile.weight);
    mix_f32(hash, profile.gravity);
    mix_f32(hash, profile.terminal_velocity);
    mix_f32(hash, profile.fast_fall_velocity);
    mix_f32(hash, profile.jump_vertical_initial_velocity);
    mix_f32(hash, profile.hop_vertical_initial_velocity);
    mix_i32(hash, profile.full_hop_height);
    mix_i32(hash, profile.short_hop_height);
    mix_i32(hash, profile.double_jump_height);
    mix_i32(hash, profile.entry_platform_offset_y);
    mix_i32(hash, profile.standing_height_units);
    mix_f32(hash, profile.player_nudge_body_center_x);
    mix_f32(hash, profile.player_nudge_body_half_width);
    mix_u8(hash, profile.jumpsquat_frames);
    mix_u8(hash, profile.dash_frames);
    mix_u8(hash, profile.standing_turn_direction_change_frames);
    mix_u8(hash, profile.standing_turn_total_frames);
    mix_u8(hash, profile.normal_landing_lag_ticks);
    mix_u8(hash, profile.landing_air_n_lag_ticks);
    mix_u8(hash, profile.landing_air_f_lag_ticks);
    mix_u8(hash, profile.landing_air_b_lag_ticks);
    mix_u8(hash, profile.landing_air_hi_lag_ticks);
    mix_u8(hash, profile.landing_air_lw_lag_ticks);
}

fn mix_fighter_action_frames(hash: &mut u64, action_frames: FighterActionFrames) {
    mix_u8(hash, action_frames.attack1_total_frames);
    mix_u8(hash, action_frames.attack1_iasa_frame);
    mix_u8(hash, action_frames.attack12_total_frames);
    mix_u8(hash, action_frames.attack12_iasa_frame);
    mix_u8(hash, action_frames.attack13_total_frames);
    mix_u8(hash, action_frames.attack13_iasa_frame);
    mix_u8(hash, action_frames.jab_2_input_window);
    mix_u8(hash, action_frames.jab_3_input_window);
    mix_u8(hash, action_frames.rapid_jab_window);
    mix_u8(hash, action_frames.attack11_jab_combo_enable_frame);
    mix_u8(hash, action_frames.attack12_jab_combo_enable_frame);
    mix_u8(hash, action_frames.attack_dash_total_frames);
    mix_u8(hash, action_frames.attack_dash_iasa_frame);
    mix_u8(hash, action_frames.attack_air_n_landing_lag_set_frame);
    mix_u8(hash, action_frames.attack_air_n_landing_lag_clear_frame);
    mix_u8(hash, action_frames.attack_air_f_landing_lag_set_frame);
    mix_u8(hash, action_frames.attack_air_f_landing_lag_clear_frame);
    mix_u8(hash, action_frames.attack_air_b_landing_lag_set_frame);
    mix_u8(hash, action_frames.attack_air_b_landing_lag_clear_frame);
    mix_u8(hash, action_frames.attack_air_hi_landing_lag_set_frame);
    mix_u8(hash, action_frames.attack_air_hi_landing_lag_clear_frame);
    mix_u8(hash, action_frames.attack_air_lw_landing_lag_set_frame);
    mix_u8(hash, action_frames.attack_air_lw_landing_lag_clear_frame);
    mix_u8(hash, action_frames.dash_total_frames);
    mix_u8(hash, action_frames.dash_cmd_var0_clear_frame);
    mix_u8(hash, action_frames.dash_cmd_var0_set_frame);
    mix_u8(hash, action_frames.guard_on_total_frames);
    mix_u8(hash, action_frames.guard_off_total_frames);
    mix_u8(hash, action_frames.escape_n_total_frames);
    mix_u8(hash, action_frames.escape_f_total_frames);
    mix_u8(hash, action_frames.escape_b_total_frames);
    mix_u8(hash, action_frames.escape_air_skip_decay_frame);
    mix_u8(hash, action_frames.turn_run_total_frames);
    mix_u8(hash, action_frames.turn_run_cmd_var1_frame);
    mix_u8(hash, action_frames.run_brake_total_frames);
    mix_u8(hash, action_frames.run_brake_cmd_var0_set_frame);
    mix_u8(hash, action_frames.run_brake_cmd_var0_clear_frame);
    mix_u8(hash, action_frames.squat_total_frames);
    mix_u8(hash, action_frames.squat_rv_total_frames);
}

fn mix_i32(hash: &mut u64, value: i32) {
    for byte in value.to_le_bytes() {
        mix_u8(hash, byte);
    }
}
