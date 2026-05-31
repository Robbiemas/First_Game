use mole_core::MeleeCommonData;

pub use mole_core::{GameCubeButtonState, GameCubePadStatus, PlayerInput};

const GAMECUBE_STICK_CENTER: i16 = 128;
pub const HSD_STICK_RADIUS: i8 = 127;
pub const UCF_VERSION: &str = "0.84";
pub const UCF_CARDINAL_AXIS: i8 = 80;
pub const UCF_CARDINAL_SNAP_RANGE: i8 = 6;
pub const UCF_TILT_INTENT_DELTA: i16 = 75;
pub const UCF_SHIELD_DROP_DELTA: i16 = 44;
// UCF's sdrop-up precheck uses -0.6125 on the Melee float stick scale.
pub const UCF_SHIELD_DROP_MIN_Y: i8 = 78;
const UCF_PAD_BUFFER_SIZE: usize = 4;
const UCF_PAD_BUFFER_MASK: usize = UCF_PAD_BUFFER_SIZE - 1;
const MAX_MELEE_INPUT_TIMER: u8 = 0xfe;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputMappingConfig {
    pub trigger_deadzone: u8,
    pub ucf_enabled: bool,
}

impl Default for InputMappingConfig {
    fn default() -> Self {
        Self {
            trigger_deadzone: 0,
            ucf_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputOrigin {
    pad: GameCubePadStatus,
}

impl InputOrigin {
    pub const fn from_stable_sample(pad: GameCubePadStatus) -> Self {
        Self { pad }
    }

    pub fn map_gamecube_pad(self, pad: GameCubePadStatus) -> PlayerInput {
        map_gamecube_pad_to_player_input(gamecube_pad_with_origin(pad, self.pad))
    }

    pub const fn pad(self) -> GameCubePadStatus {
        self.pad
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GameCubeInputMapper {
    config: InputMappingConfig,
    ucf: UcfInputPreprocessor,
}

impl GameCubeInputMapper {
    pub fn new(config: InputMappingConfig) -> Self {
        Self {
            config,
            ucf: UcfInputPreprocessor::default(),
        }
    }

    pub fn map_gamecube_pad(&mut self, pad: GameCubePadStatus) -> PlayerInput {
        let adjusted = GameCubePadStatus {
            left_trigger: map_trigger_analog(pad.left_trigger, self.config.trigger_deadzone),
            right_trigger: map_trigger_analog(pad.right_trigger, self.config.trigger_deadzone),
            ..pad
        };
        let native = hsd_clamp_gamecube_pad(adjusted);
        let adjusted = if self.config.ucf_enabled {
            self.ucf.preprocess_pad_with_native_result(adjusted, native)
        } else {
            UcfPreprocessedPad {
                pad: native,
                dashback_amendment: false,
            }
        };
        map_gamecube_pad_to_player_input_with_config(adjusted.pad, InputMappingConfig::default())
            .with_ucf_dashback_amendment(adjusted.dashback_amendment)
    }
}

impl Default for GameCubeInputMapper {
    fn default() -> Self {
        Self::new(InputMappingConfig::default())
    }
}

pub fn map_gamecube_pad_to_player_input(pad: GameCubePadStatus) -> PlayerInput {
    map_gamecube_pad_to_player_input_with_config(pad, InputMappingConfig::default())
}

pub fn map_gamecube_pad_to_player_input_with_config(
    pad: GameCubePadStatus,
    config: InputMappingConfig,
) -> PlayerInput {
    let (stick_x, stick_y) = pad.main_stick_i8();
    let (c_stick_x, c_stick_y) = pad.c_stick_i8();
    let left_trigger = map_trigger_analog(pad.left_trigger, config.trigger_deadzone);
    let right_trigger = map_trigger_analog(pad.right_trigger, config.trigger_deadzone);

    PlayerInput::neutral()
        .with_left_stick(stick_x, stick_y)
        .with_c_stick(c_stick_x, c_stick_y)
        .with_left_trigger_analog(left_trigger)
        .with_right_trigger_analog(right_trigger)
        .with_left_trigger_digital(pad.buttons.l())
        .with_right_trigger_digital(pad.buttons.r())
        .with_attack(pad.buttons.a())
        .with_special(pad.buttons.b())
        .with_jump_primary(pad.buttons.x())
        .with_jump_secondary(pad.buttons.y())
        .with_grab(pad.buttons.z())
        .with_start(pad.buttons.start())
        .with_dpad_up(pad.buttons.dpad_up())
        .with_dpad_down(pad.buttons.dpad_down())
        .with_dpad_left(pad.buttons.dpad_left())
        .with_dpad_right(pad.buttons.dpad_right())
}

pub const fn map_trigger_analog(value: u8, deadzone: u8) -> u8 {
    if value <= deadzone || deadzone == u8::MAX {
        0
    } else {
        let numerator = (value - deadzone) as u16 * u8::MAX as u16;
        let denominator = (u8::MAX - deadzone) as u16;
        (numerator / denominator) as u8
    }
}

pub fn native_gamecube_pad_with_origin(
    pad: GameCubePadStatus,
    origin: GameCubePadStatus,
) -> GameCubePadStatus {
    hsd_clamp_gamecube_pad(gamecube_pad_with_origin(pad, origin))
}

pub fn hsd_clamp_gamecube_pad(pad: GameCubePadStatus) -> GameCubePadStatus {
    let (stick_x, stick_y) = hsd_clamp_stick(pad.main_stick_i8());
    let (c_stick_x, c_stick_y) = hsd_clamp_stick(pad.c_stick_i8());
    GameCubePadStatus {
        stick_x: i8_to_gamecube_axis(stick_x),
        stick_y: i8_to_gamecube_axis(stick_y),
        c_stick_x: i8_to_gamecube_axis(c_stick_x),
        c_stick_y: i8_to_gamecube_axis(c_stick_y),
        ..pad
    }
}

fn hsd_clamp_stick(stick: (i8, i8)) -> (i8, i8) {
    let x = stick.0 as f32;
    let y = stick.1 as f32;
    let radius = (x * x + y * y).sqrt();
    let max = HSD_STICK_RADIUS as f32;

    if radius > max {
        ((x * max / radius) as i8, (y * max / radius) as i8)
    } else {
        stick
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UcfPreprocessedPad {
    pub pad: GameCubePadStatus,
    pub dashback_amendment: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UcfInputPreprocessor {
    pad_buffer: [(i8, i8); UCF_PAD_BUFFER_SIZE],
    pad_buffer_index: usize,
    prev_lstick: (i8, i8),
    stick_x_hold_timer: u8,
    stick_y_hold_timer: u8,
    sdrop_up_frames: u8,
}

impl UcfInputPreprocessor {
    pub const fn new() -> Self {
        Self {
            pad_buffer: [(0, 0); UCF_PAD_BUFFER_SIZE],
            pad_buffer_index: 0,
            prev_lstick: (0, 0),
            stick_x_hold_timer: MAX_MELEE_INPUT_TIMER,
            stick_y_hold_timer: MAX_MELEE_INPUT_TIMER,
            sdrop_up_frames: 0,
        }
    }

    pub fn preprocess_pad(&mut self, pad: GameCubePadStatus) -> GameCubePadStatus {
        self.preprocess_pad_with_native(pad, hsd_clamp_gamecube_pad(pad))
    }

    pub fn preprocess_pad_with_native(
        &mut self,
        pad: GameCubePadStatus,
        native: GameCubePadStatus,
    ) -> GameCubePadStatus {
        self.preprocess_pad_with_native_result(pad, native).pad
    }

    pub fn preprocess_pad_with_native_result(
        &mut self,
        pad: GameCubePadStatus,
        native: GameCubePadStatus,
    ) -> UcfPreprocessedPad {
        let raw_lstick = pad.main_stick_i8();
        let raw_cstick = pad.c_stick_i8();
        self.pad_buffer_index = (self.pad_buffer_index + 1) & UCF_PAD_BUFFER_MASK;
        self.pad_buffer[self.pad_buffer_index] = raw_lstick;
        let previous_stick = self.pad_buffer
            [(self.pad_buffer_index + UCF_PAD_BUFFER_SIZE - 2) & UCF_PAD_BUFFER_MASK];
        let native_lstick = native.main_stick_i8();
        let native_cstick = native.c_stick_i8();

        let common = MeleeCommonData::provisional_mole();
        let cleaned_lstick = (
            clean_axis(native_lstick.0, common.main_stick_deadzone_x),
            clean_axis(native_lstick.1, common.main_stick_deadzone_y),
        );
        self.stick_x_hold_timer = update_axis_hold_timer(
            self.stick_x_hold_timer,
            self.prev_lstick.0,
            cleaned_lstick.0,
            common.tap_x_threshold,
        );
        self.stick_y_hold_timer = update_axis_hold_timer(
            self.stick_y_hold_timer,
            self.prev_lstick.1,
            cleaned_lstick.1,
            common.tap_y_threshold,
        );
        self.prev_lstick = cleaned_lstick;

        let (stick_x, mut stick_y) = apply_ucf_cardinals(raw_lstick, native_lstick);
        let (c_stick_x, c_stick_y) = apply_ucf_cardinals(raw_cstick, native_cstick);
        let dashback_amendment = check_ucf_dashback(
            self.stick_x_hold_timer,
            previous_stick,
            raw_lstick,
            (stick_x, stick_y),
            common,
        );
        if check_sdrop_up(
            self.sdrop_up_frames,
            self.stick_y_hold_timer,
            previous_stick,
            raw_lstick,
            (stick_x, stick_y),
        ) {
            self.sdrop_up_frames = self.sdrop_up_frames.saturating_add(1);
        } else {
            self.sdrop_up_frames = 0;
        }
        let shield_active = pad.buttons.l()
            || pad.buttons.r()
            || pad.left_trigger >= common.z_shield_analog
            || pad.right_trigger >= common.z_shield_analog;
        if shield_active && self.sdrop_up_frames >= 2 {
            stick_y = -common.platform_pass_y;
        }

        UcfPreprocessedPad {
            pad: GameCubePadStatus {
                stick_x: i8_to_gamecube_axis(stick_x),
                stick_y: i8_to_gamecube_axis(stick_y),
                c_stick_x: i8_to_gamecube_axis(c_stick_x),
                c_stick_y: i8_to_gamecube_axis(c_stick_y),
                ..native
            },
            dashback_amendment,
        }
    }
}

impl Default for UcfInputPreprocessor {
    fn default() -> Self {
        Self::new()
    }
}

fn apply_ucf_cardinals(raw: (i8, i8), native: (i8, i8)) -> (i8, i8) {
    if threshold_abs(raw.0) >= UCF_CARDINAL_AXIS as i16
        && threshold_abs(raw.1) <= UCF_CARDINAL_SNAP_RANGE as i16
    {
        (full_axis(raw.0), 0)
    } else if threshold_abs(raw.1) >= UCF_CARDINAL_AXIS as i16
        && threshold_abs(raw.0) <= UCF_CARDINAL_SNAP_RANGE as i16
    {
        (0, full_axis(raw.1))
    } else {
        native
    }
}

fn check_sdrop_up(
    sdrop_up_frames: u8,
    stick_y_hold_timer: u8,
    previous_stick: (i8, i8),
    raw_lstick: (i8, i8),
    processed_lstick: (i8, i8),
) -> bool {
    if processed_lstick.1 > -UCF_SHIELD_DROP_MIN_Y {
        return false;
    }
    if !is_ucf_shield_drop_rim_coord(processed_lstick) {
        return false;
    }
    if sdrop_up_frames != 0 {
        return true;
    }

    stick_y_hold_timer < 2
        && ucf_axis_delta_exceeds(previous_stick.1, raw_lstick.1, UCF_SHIELD_DROP_DELTA)
}

fn check_ucf_dashback(
    stick_x_hold_timer: u8,
    previous_stick: (i8, i8),
    raw_lstick: (i8, i8),
    processed_lstick: (i8, i8),
    common: MeleeCommonData,
) -> bool {
    stick_x_hold_timer < 2
        && threshold_abs(processed_lstick.0) >= threshold_abs(common.dash_x)
        && ucf_axis_delta_exceeds(previous_stick.0, raw_lstick.0, UCF_TILT_INTENT_DELTA)
}

fn update_axis_hold_timer(timer: u8, previous: i8, current: i8, threshold: i8) -> u8 {
    let threshold = threshold_abs(threshold);
    let previous = previous as i16;
    let current = current as i16;

    if current >= threshold {
        if previous >= threshold {
            increment_melee_timer(timer)
        } else {
            0
        }
    } else if current <= -threshold {
        if previous <= -threshold {
            increment_melee_timer(timer)
        } else {
            0
        }
    } else {
        MAX_MELEE_INPUT_TIMER
    }
}

fn increment_melee_timer(timer: u8) -> u8 {
    timer.saturating_add(1).min(MAX_MELEE_INPUT_TIMER)
}

fn full_axis(value: i8) -> i8 {
    if value < 0 {
        -HSD_STICK_RADIUS
    } else {
        HSD_STICK_RADIUS
    }
}

fn ucf_axis_delta_exceeds(previous: i8, current: i8, threshold: i16) -> bool {
    let delta = current as i32 - previous as i32;
    let threshold = threshold as i32;
    delta * delta > threshold * threshold
}

fn is_ucf_shield_drop_rim_coord(stick: (i8, i8)) -> bool {
    let x = ucf_rim_axis_coord(stick.0);
    let y = ucf_rim_axis_coord(stick.1);
    x * x + y * y > 80 * 80 && (stick.1 as i16) <= -threshold_abs(UCF_SHIELD_DROP_MIN_Y)
}

fn ucf_rim_axis_coord(axis: i8) -> i32 {
    let abs_axis = (axis as i16).abs() as i32;
    let denominator = if axis < 0 { 128 } else { 127 };
    let numerator = abs_axis * 80;
    let mut coord = numerator / denominator;
    if numerator != 0 && numerator % denominator == 0 {
        coord -= 1;
    }
    coord + 2
}

fn threshold_abs(value: i8) -> i16 {
    if value == i8::MIN {
        128
    } else {
        value.abs() as i16
    }
}

fn clean_axis(value: i8, deadzone: i8) -> i8 {
    if (value as i16).abs() <= threshold_abs(deadzone) {
        0
    } else {
        value
    }
}

fn i8_to_gamecube_axis(value: i8) -> u8 {
    (value as i16 + GAMECUBE_STICK_CENTER).clamp(0, u8::MAX as i16) as u8
}

pub fn gamecube_pad_with_origin(
    pad: GameCubePadStatus,
    origin: GameCubePadStatus,
) -> GameCubePadStatus {
    GameCubePadStatus {
        stick_x: gamecube_axis_with_origin(pad.stick_x, origin.stick_x),
        stick_y: gamecube_axis_with_origin(pad.stick_y, origin.stick_y),
        c_stick_x: gamecube_axis_with_origin(pad.c_stick_x, origin.c_stick_x),
        c_stick_y: gamecube_axis_with_origin(pad.c_stick_y, origin.c_stick_y),
        left_trigger: gamecube_trigger_with_origin(pad.left_trigger, origin.left_trigger),
        right_trigger: gamecube_trigger_with_origin(pad.right_trigger, origin.right_trigger),
        buttons: pad.buttons,
    }
}

pub const fn gamecube_trigger_with_origin(value: u8, origin: u8) -> u8 {
    value.saturating_sub(origin)
}

pub const fn gamecube_axis_with_origin(value: u8, origin: u8) -> u8 {
    let recentered = GAMECUBE_STICK_CENTER + value as i16 - origin as i16;
    if recentered < 0 {
        0
    } else if recentered > u8::MAX as i16 {
        u8::MAX
    } else {
        recentered as u8
    }
}
