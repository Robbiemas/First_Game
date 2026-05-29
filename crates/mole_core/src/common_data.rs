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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeleeCommonData {
    pub tap_x_threshold: i8,
    pub tap_y_threshold: i8,
    pub trigger_threshold: u8,
    pub trigger_timer_threshold: u8,
    pub main_stick_deadzone: i8,
    pub c_stick_deadzone: i8,
    pub trigger_deadzone: u8,
    pub z_shield_analog: u8,
    pub walk_x: i8,
    pub walk_slow_x: i8,
    pub walk_middle_x: i8,
    pub walk_fast_x: i8,
    pub dash_x: i8,
    pub dash_tap_window: u8,
    pub turn_x: i8,
    pub tilt_x: i8,
    pub tilt_y: i8,
    pub smash_y: i8,
    pub crouch_y: i8,
    pub crouch_release_y: i8,
    pub tap_jump_y: i8,
    pub tap_jump_window: u8,
    pub tap_jump_release_y: i8,
    pub fast_fall_y: i8,
    pub fast_fall_window: u8,
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
    pub escapeair_force: i32,
    pub escapeair_decay_percent: i32,
    pub escapeair_landing_lag_ticks: u8,
    pub fallspecial_platform_landing_y: i8,
    pub platform_pass_y: i8,
    pub platform_pass_y_tap_window: u8,
    pub pass_initial_y_velocity: i32,
    pub platform_drop_delay_ticks: u8,
    pub dash_early_action_window: u8,
    pub dash_defensive_action_window: u8,
    pub dash_late_action_window: u8,
    pub run_x: i8,
    pub guard_on_catch_dash_window: u8,
    pub run_turn_run_no_interrupt_frames: u8,
}

impl MeleeCommonData {
    pub const PROVISIONAL: Self = Self {
        tap_x_threshold: 36,
        tap_y_threshold: 36,
        trigger_threshold: 1,
        trigger_timer_threshold: 140,
        main_stick_deadzone: 0,
        c_stick_deadzone: 0,
        trigger_deadzone: 0,
        z_shield_analog: 49,
        walk_x: 20,
        walk_slow_x: 20,
        walk_middle_x: 50,
        walk_fast_x: 90,
        dash_x: 80,
        dash_tap_window: 3,
        turn_x: 24,
        tilt_x: 24,
        tilt_y: 24,
        smash_y: 80,
        crouch_y: 36,
        crouch_release_y: 36,
        tap_jump_y: 80,
        tap_jump_window: 3,
        tap_jump_release_y: 40,
        fast_fall_y: 80,
        fast_fall_window: 2,
        c_stick: 40,
        aerial_neutral_x: 40,
        aerial_neutral_y: 40,
        aerial_vertical_angle_tan_milli: 1000,
        air_jump_backward_x: 20,
        escape_x: 80,
        escape_x_tap_window: 3,
        escape_y: 89,
        escape_y_tap_window: 3,
        special_side_x: 40,
        special_vertical_y: 40,
        escapeair_iasa_timer_ticks: 15,
        escapeair_animation_ticks: 20,
        escapeair_deadzone_x: 20,
        escapeair_deadzone_y: 20,
        escapeair_force: 800,
        escapeair_decay_percent: 90,
        escapeair_landing_lag_ticks: 10,
        fallspecial_platform_landing_y: -80,
        platform_pass_y: 84,
        platform_pass_y_tap_window: 3,
        pass_initial_y_velocity: -1_200,
        platform_drop_delay_ticks: 4,
        dash_early_action_window: 1,
        dash_defensive_action_window: 1,
        dash_late_action_window: 15,
        run_x: 64,
        guard_on_catch_dash_window: 4,
        run_turn_run_no_interrupt_frames: 1,
    };

    pub const fn provisional_mole() -> Self {
        Self::PROVISIONAL
    }

    pub fn from_plco_bytes(bytes: &[u8]) -> Result<Self, CommonDataExtractError> {
        let mut data = Self::PROVISIONAL;

        data.tap_x_threshold = read_stick_i8(bytes, 0x08, "x8_someStickThreshold")?;
        data.tap_y_threshold = read_stick_i8(bytes, 0x0c, "xC")?;
        data.trigger_deadzone = read_trigger_u8(bytes, 0x10, "x10")?;
        data.z_shield_analog = read_trigger_u8(bytes, 0x14, "x14")?;
        data.trigger_timer_threshold = read_trigger_u8(bytes, 0x18, "x18")?;
        data.aerial_vertical_angle_tan_milli =
            read_radian_tangent_milli(bytes, 0x20, "x20_radians")?;
        data.walk_x = read_stick_i8(bytes, 0x24, "x24")?;
        data.walk_slow_x = read_stick_i8(bytes, 0x28, "x28")?;
        data.walk_middle_x = read_stick_i8(bytes, 0x2c, "x2C")?;
        data.walk_fast_x = read_stick_i8(bytes, 0x30, "x30")?;
        data.turn_x = read_stick_i8(bytes, 0x34, "x34")?;
        data.dash_x = read_stick_i8(bytes, 0x3c, "x3C")?;
        data.dash_tap_window = read_u8_from_i32(bytes, 0x40, "x40")?;
        data.dash_early_action_window = read_u8_from_f32(bytes, 0x44, "x44")?;
        data.dash_defensive_action_window = read_u8_from_f32(bytes, 0x48, "x48")?;
        data.dash_late_action_window = read_u8_from_f32(bytes, 0x4c, "x4C")?;
        data.run_x = read_stick_i8(bytes, 0x58, "x58_someLStickXThreshold")?;
        data.guard_on_catch_dash_window = read_u8_from_f32(bytes, 0x68, "x68")?;
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
        data.aerial_neutral_x = read_stick_i8(bytes, 0xdc, "xDC")?;
        data.aerial_neutral_y = read_stick_i8(bytes, 0xe0, "xE0")?;
        data.fallspecial_platform_landing_y = read_stick_i8(bytes, 0x25c, "x25C")?;
        data.escape_y = read_stick_i8(bytes, 0x314, "x314")?;
        data.escape_y_tap_window = read_u8_from_i32(bytes, 0x318, "x318")?;
        data.escape_x = read_stick_i8(bytes, 0x31c, "x31C")?;
        data.escape_x_tap_window = read_u8_from_i32(bytes, 0x320, "x320")?;

        data.escapeair_deadzone_x = read_stick_i8(bytes, 0x32c, "escapeair_deadzone.x")?;
        data.escapeair_deadzone_y = read_stick_i8(bytes, 0x330, "escapeair_deadzone.y")?;

        data.escapeair_iasa_timer_ticks = read_u8_from_i32(bytes, 0x334, "x334")?;
        data.escapeair_force = read_milli_i32(bytes, 0x338, "escapeair_force")?;
        data.escapeair_decay_percent = read_percent_i32(bytes, 0x33c, "escapeair_decay")?;
        data.escapeair_landing_lag_ticks = read_u8_from_f32(bytes, 0x344, "x344")?;
        data.run_turn_run_no_interrupt_frames = read_u8_from_f32(bytes, 0x430, "x430")?;
        data.platform_pass_y = read_stick_i8(bytes, 0x464, "x464")?;
        data.platform_pass_y_tap_window = read_u8_from_f32(bytes, 0x468, "x468")?;
        data.pass_initial_y_velocity = read_milli_i32(bytes, 0x46c, "x46C")?;
        data.platform_drop_delay_ticks = read_u8_from_f32(bytes, 0x470, "x470")?;

        Ok(data)
    }

    pub const fn input_config(self) -> MeleeInputConfig {
        MeleeInputConfig {
            tap_x_threshold: self.tap_x_threshold,
            tap_y_threshold: self.tap_y_threshold,
            trigger_threshold: self.trigger_threshold,
            trigger_timer_threshold: self.trigger_timer_threshold,
            main_stick_deadzone: self.main_stick_deadzone,
            c_stick_deadzone: self.c_stick_deadzone,
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

fn read_percent_i32(
    bytes: &[u8],
    offset: usize,
    field: &'static str,
) -> Result<i32, CommonDataExtractError> {
    round_f32_to_i32(read_f32(bytes, offset, field)? * 100.0, field, offset)
}

fn read_milli_i32(
    bytes: &[u8],
    offset: usize,
    field: &'static str,
) -> Result<i32, CommonDataExtractError> {
    round_f32_to_i32(read_f32(bytes, offset, field)? * 1000.0, field, offset)
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

fn read_u8_from_f32(
    bytes: &[u8],
    offset: usize,
    field: &'static str,
) -> Result<u8, CommonDataExtractError> {
    let value = round_f32_to_i32(read_f32(bytes, offset, field)?, field, offset)?;
    range_i32(value, 0, u8::MAX as i32, field, offset).map(|value| value as u8)
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
        rust_name: "tap_x_threshold",
        source_name: "x8_someStickThreshold",
        offset: 0x08,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "tap_y_threshold",
        source_name: "xC_someStickThreshold",
        offset: 0x0c,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "trigger_deadzone",
        source_name: "x10_trigger_deadzone",
        offset: 0x10,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "z_shield_analog",
        source_name: "x14",
        offset: 0x14,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "trigger_timer_threshold",
        source_name: "x18",
        offset: 0x18,
        provenance: CommonDataProvenance::ProvisionalMole,
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
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "walk_slow_x",
        source_name: "x28",
        offset: 0x28,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "walk_middle_x",
        source_name: "x2C",
        offset: 0x2c,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "walk_fast_x",
        source_name: "x30",
        offset: 0x30,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "turn_x",
        source_name: "x34",
        offset: 0x34,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "dash_x",
        source_name: "x3C",
        offset: 0x3c,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "dash_tap_window",
        source_name: "x40",
        offset: 0x40,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "dash_early_action_window",
        source_name: "x44",
        offset: 0x44,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "dash_defensive_action_window",
        source_name: "x48",
        offset: 0x48,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "dash_late_action_window",
        source_name: "x4C",
        offset: 0x4c,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "run_x",
        source_name: "x58_someLStickXThreshold",
        offset: 0x58,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "guard_on_catch_dash_window",
        source_name: "x68",
        offset: 0x68,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "run_turn_run_no_interrupt_frames",
        source_name: "x430",
        offset: 0x430,
        provenance: CommonDataProvenance::ProvisionalMole,
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
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "escapeair_decay_percent",
        source_name: "escapeair_decay",
        offset: 0x33c,
        provenance: CommonDataProvenance::ProvisionalMole,
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
        provenance: CommonDataProvenance::ProvisionalMole,
    },
    CommonDataFieldSource {
        rust_name: "platform_drop_delay_ticks",
        source_name: "x470",
        offset: 0x470,
        provenance: CommonDataProvenance::ProvisionalMole,
    },
];

pub const fn input_common_data_field_sources() -> &'static [CommonDataFieldSource] {
    INPUT_COMMON_DATA_FIELD_SOURCES
}
