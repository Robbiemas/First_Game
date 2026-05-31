const ATTACK_BIT: u64 = 1 << 0;
const SPECIAL_BIT: u64 = 1 << 1;
const JUMP_BIT: u64 = 1 << 2;
const SHIELD_BIT: u64 = 1 << 3;
const GRAB_BIT: u64 = 1 << 4;
const START_BIT: u64 = 1 << 5;
const LEFT_TRIGGER_DIGITAL_BIT: u64 = 1 << 6;
const RIGHT_TRIGGER_DIGITAL_BIT: u64 = 1 << 7;
const JUMP_SECONDARY_BIT: u64 = 1 << 28;
const DPAD_UP_BIT: u64 = 1 << 24;
const DPAD_DOWN_BIT: u64 = 1 << 25;
const DPAD_LEFT_BIT: u64 = 1 << 26;
const DPAD_RIGHT_BIT: u64 = 1 << 27;
pub const UCF_DASHBACK_AMENDMENT_BIT: u64 = 1 << 29;
const STICK_X_SHIFT: u32 = 8;
const STICK_Y_SHIFT: u32 = 16;
const C_STICK_X_SHIFT: u32 = 32;
const C_STICK_Y_SHIFT: u32 = 40;
const LEFT_TRIGGER_ANALOG_SHIFT: u32 = 48;
const RIGHT_TRIGGER_ANALOG_SHIFT: u32 = 56;
pub const NO_GROUNDED_SPECIAL_DIRECTION: (i8, i8) = (0, -2);
const STICK_BYTE_MASK: u64 = 0xff;
const USED_BITS: u64 = ATTACK_BIT
    | SPECIAL_BIT
    | JUMP_BIT
    | SHIELD_BIT
    | GRAB_BIT
    | START_BIT
    | LEFT_TRIGGER_DIGITAL_BIT
    | RIGHT_TRIGGER_DIGITAL_BIT
    | JUMP_SECONDARY_BIT
    | DPAD_UP_BIT
    | DPAD_DOWN_BIT
    | DPAD_LEFT_BIT
    | DPAD_RIGHT_BIT
    | UCF_DASHBACK_AMENDMENT_BIT
    | (STICK_BYTE_MASK << STICK_X_SHIFT)
    | (STICK_BYTE_MASK << STICK_Y_SHIFT)
    | (STICK_BYTE_MASK << C_STICK_X_SHIFT)
    | (STICK_BYTE_MASK << C_STICK_Y_SHIFT)
    | (STICK_BYTE_MASK << LEFT_TRIGGER_ANALOG_SHIFT)
    | (STICK_BYTE_MASK << RIGHT_TRIGGER_ANALOG_SHIFT);

const GC_A_BIT: u16 = 1 << 0;
const GC_B_BIT: u16 = 1 << 1;
const GC_X_BIT: u16 = 1 << 2;
const GC_Y_BIT: u16 = 1 << 3;
const GC_DPAD_LEFT_BIT: u16 = 1 << 4;
const GC_DPAD_RIGHT_BIT: u16 = 1 << 5;
const GC_DPAD_DOWN_BIT: u16 = 1 << 6;
const GC_DPAD_UP_BIT: u16 = 1 << 7;
const GC_START_BIT: u16 = 1 << 8;
const GC_Z_BIT: u16 = 1 << 9;
const GC_R_BIT: u16 = 1 << 10;
const GC_L_BIT: u16 = 1 << 11;
const GC_USED_BUTTON_BITS: u16 = 0x0fff;
const SOURCE_DPAD_LEFT_BIT: u32 = 1 << 0;
const SOURCE_DPAD_RIGHT_BIT: u32 = 1 << 1;
const SOURCE_DPAD_DOWN_BIT: u32 = 1 << 2;
const SOURCE_DPAD_UP_BIT: u32 = 1 << 3;
const SOURCE_Z_BIT: u32 = 1 << 4;
const SOURCE_R_BIT: u32 = 1 << 5;
const SOURCE_L_BIT: u32 = 1 << 6;
const SOURCE_A_BIT: u32 = 1 << 8;
const SOURCE_B_BIT: u32 = 1 << 9;
const SOURCE_X_BIT: u32 = 1 << 10;
const SOURCE_Y_BIT: u32 = 1 << 11;
const SOURCE_START_BIT: u32 = 1 << 12;
const SOURCE_LR_BIT: u32 = 1 << 31;
const SOURCE_USED_BUTTON_BITS: u32 = SOURCE_DPAD_LEFT_BIT
    | SOURCE_DPAD_RIGHT_BIT
    | SOURCE_DPAD_DOWN_BIT
    | SOURCE_DPAD_UP_BIT
    | SOURCE_Z_BIT
    | SOURCE_R_BIT
    | SOURCE_L_BIT
    | SOURCE_A_BIT
    | SOURCE_B_BIT
    | SOURCE_X_BIT
    | SOURCE_Y_BIT
    | SOURCE_START_BIT
    | SOURCE_LR_BIT;
const MAX_MELEE_INPUT_TIMER: u8 = 0xfe;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GameCubeButtonState {
    bits: u16,
}

impl GameCubeButtonState {
    pub const fn empty() -> Self {
        Self { bits: 0 }
    }

    pub const fn from_bits(bits: u16) -> Self {
        Self {
            bits: bits & GC_USED_BUTTON_BITS,
        }
    }

    pub const fn bits(self) -> u16 {
        self.bits
    }

    pub const fn with_a(self, pressed: bool) -> Self {
        self.with_button(GC_A_BIT, pressed)
    }

    pub const fn with_b(self, pressed: bool) -> Self {
        self.with_button(GC_B_BIT, pressed)
    }

    pub const fn with_x(self, pressed: bool) -> Self {
        self.with_button(GC_X_BIT, pressed)
    }

    pub const fn with_y(self, pressed: bool) -> Self {
        self.with_button(GC_Y_BIT, pressed)
    }

    pub const fn with_dpad_left(self, pressed: bool) -> Self {
        self.with_button(GC_DPAD_LEFT_BIT, pressed)
    }

    pub const fn with_dpad_right(self, pressed: bool) -> Self {
        self.with_button(GC_DPAD_RIGHT_BIT, pressed)
    }

    pub const fn with_dpad_down(self, pressed: bool) -> Self {
        self.with_button(GC_DPAD_DOWN_BIT, pressed)
    }

    pub const fn with_dpad_up(self, pressed: bool) -> Self {
        self.with_button(GC_DPAD_UP_BIT, pressed)
    }

    pub const fn with_start(self, pressed: bool) -> Self {
        self.with_button(GC_START_BIT, pressed)
    }

    pub const fn with_z(self, pressed: bool) -> Self {
        self.with_button(GC_Z_BIT, pressed)
    }

    pub const fn with_l(self, pressed: bool) -> Self {
        self.with_button(GC_L_BIT, pressed)
    }

    pub const fn with_r(self, pressed: bool) -> Self {
        self.with_button(GC_R_BIT, pressed)
    }

    pub const fn a(self) -> bool {
        self.has_button(GC_A_BIT)
    }

    pub const fn b(self) -> bool {
        self.has_button(GC_B_BIT)
    }

    pub const fn x(self) -> bool {
        self.has_button(GC_X_BIT)
    }

    pub const fn y(self) -> bool {
        self.has_button(GC_Y_BIT)
    }

    pub const fn dpad_left(self) -> bool {
        self.has_button(GC_DPAD_LEFT_BIT)
    }

    pub const fn dpad_right(self) -> bool {
        self.has_button(GC_DPAD_RIGHT_BIT)
    }

    pub const fn dpad_down(self) -> bool {
        self.has_button(GC_DPAD_DOWN_BIT)
    }

    pub const fn dpad_up(self) -> bool {
        self.has_button(GC_DPAD_UP_BIT)
    }

    pub const fn start(self) -> bool {
        self.has_button(GC_START_BIT)
    }

    pub const fn z(self) -> bool {
        self.has_button(GC_Z_BIT)
    }

    pub const fn r(self) -> bool {
        self.has_button(GC_R_BIT)
    }

    pub const fn l(self) -> bool {
        self.has_button(GC_L_BIT)
    }

    const fn with_button(self, bit: u16, pressed: bool) -> Self {
        if pressed {
            Self {
                bits: self.bits | bit,
            }
        } else {
            Self {
                bits: self.bits & !bit,
            }
        }
    }

    const fn has_button(self, bit: u16) -> bool {
        self.bits & bit != 0
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MeleeSourceButtonState {
    bits: u32,
}

impl MeleeSourceButtonState {
    pub const fn empty() -> Self {
        Self { bits: 0 }
    }

    pub const fn from_bits(bits: u32) -> Self {
        Self {
            bits: bits & SOURCE_USED_BUTTON_BITS,
        }
    }

    pub const fn bits(self) -> u32 {
        self.bits
    }

    pub const fn dpad_left(self) -> bool {
        self.has_button(SOURCE_DPAD_LEFT_BIT)
    }

    pub const fn dpad_right(self) -> bool {
        self.has_button(SOURCE_DPAD_RIGHT_BIT)
    }

    pub const fn dpad_down(self) -> bool {
        self.has_button(SOURCE_DPAD_DOWN_BIT)
    }

    pub const fn dpad_up(self) -> bool {
        self.has_button(SOURCE_DPAD_UP_BIT)
    }

    pub const fn z(self) -> bool {
        self.has_button(SOURCE_Z_BIT)
    }

    pub const fn r(self) -> bool {
        self.has_button(SOURCE_R_BIT)
    }

    pub const fn l(self) -> bool {
        self.has_button(SOURCE_L_BIT)
    }

    pub const fn a(self) -> bool {
        self.has_button(SOURCE_A_BIT)
    }

    pub const fn b(self) -> bool {
        self.has_button(SOURCE_B_BIT)
    }

    pub const fn x(self) -> bool {
        self.has_button(SOURCE_X_BIT)
    }

    pub const fn y(self) -> bool {
        self.has_button(SOURCE_Y_BIT)
    }

    pub const fn start(self) -> bool {
        self.has_button(SOURCE_START_BIT)
    }

    pub const fn lr(self) -> bool {
        self.has_button(SOURCE_LR_BIT)
    }

    const fn pressed_since(self, previous: Self) -> Self {
        Self::from_bits(self.bits & !previous.bits)
    }

    const fn released_since(self, current: Self) -> Self {
        Self::from_bits(self.bits & !current.bits)
    }

    const fn with_button(self, bit: u32, pressed: bool) -> Self {
        if pressed {
            Self {
                bits: self.bits | bit,
            }
        } else {
            Self {
                bits: self.bits & !bit,
            }
        }
    }

    const fn has_button(self, bit: u32) -> bool {
        self.bits & bit != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameCubePadStatus {
    pub stick_x: u8,
    pub stick_y: u8,
    pub c_stick_x: u8,
    pub c_stick_y: u8,
    pub left_trigger: u8,
    pub right_trigger: u8,
    pub buttons: GameCubeButtonState,
}

impl Default for GameCubePadStatus {
    fn default() -> Self {
        Self::neutral()
    }
}

impl GameCubePadStatus {
    pub const fn neutral() -> Self {
        Self {
            stick_x: 128,
            stick_y: 128,
            c_stick_x: 128,
            c_stick_y: 128,
            left_trigger: 0,
            right_trigger: 0,
            buttons: GameCubeButtonState::empty(),
        }
    }

    pub const fn main_stick_i16(self) -> (i16, i16) {
        (
            gamecube_axis_to_i16(self.stick_x),
            gamecube_axis_to_i16(self.stick_y),
        )
    }

    pub const fn c_stick_i16(self) -> (i16, i16) {
        (
            gamecube_axis_to_i16(self.c_stick_x),
            gamecube_axis_to_i16(self.c_stick_y),
        )
    }

    pub const fn main_stick_i8(self) -> (i8, i8) {
        (
            gamecube_axis_to_i8(self.stick_x),
            gamecube_axis_to_i8(self.stick_y),
        )
    }

    pub const fn c_stick_i8(self) -> (i8, i8) {
        (
            gamecube_axis_to_i8(self.c_stick_x),
            gamecube_axis_to_i8(self.c_stick_y),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeleeInputConfig {
    pub tap_x_threshold: i8,
    pub tap_y_threshold: i8,
    pub trigger_threshold: u8,
    pub trigger_timer_threshold: u8,
    pub main_stick_deadzone_x: i8,
    pub main_stick_deadzone_y: i8,
    pub c_stick_deadzone_x: i8,
    pub c_stick_deadzone_y: i8,
    pub trigger_deadzone: u8,
}

const DEFAULT_MELEE_INPUT_CONFIG: MeleeInputConfig =
    crate::common_data::MeleeCommonData::PROVISIONAL.input_config();

impl Default for MeleeInputConfig {
    fn default() -> Self {
        DEFAULT_MELEE_INPUT_CONFIG
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeleeInputSnapshot {
    pub lstick: (i8, i8),
    pub prev_lstick: (i8, i8),
    pub cstick: (i8, i8),
    pub prev_cstick: (i8, i8),
    pub left_trigger: u8,
    pub right_trigger: u8,
    pub prev_left_trigger: u8,
    pub prev_right_trigger: u8,
    pub held: GameCubeButtonState,
    pub pressed: GameCubeButtonState,
    pub released: GameCubeButtonState,
    pub shield_held: bool,
    pub shield_pressed: bool,
    pub shield_released: bool,
    pub left_trigger_analog_held: bool,
    pub right_trigger_analog_held: bool,
    pub left_trigger_analog_pressed: bool,
    pub right_trigger_analog_pressed: bool,
    pub x_tap_timer: u8,
    pub y_tap_timer: u8,
    pub trigger_timer: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeleeInputTimers {
    pub x_tap: u8,
    pub y_tap: u8,
    pub trigger: u8,
}

impl MeleeInputTimers {
    pub const fn expired() -> Self {
        Self {
            x_tap: MAX_MELEE_INPUT_TIMER,
            y_tap: MAX_MELEE_INPUT_TIMER,
            trigger: MAX_MELEE_INPUT_TIMER,
        }
    }

    pub fn update(self, previous: PlayerInput, current: PlayerInput) -> Self {
        self.update_with_config(previous, current, MeleeInputConfig::default())
    }

    pub fn update_with_config(
        self,
        previous: PlayerInput,
        current: PlayerInput,
        config: MeleeInputConfig,
    ) -> Self {
        Self {
            x_tap: update_axis_tap_timer(
                self.x_tap,
                previous.stick_x(),
                current.stick_x(),
                config.tap_x_threshold,
            ),
            y_tap: update_axis_tap_timer(
                self.y_tap,
                previous.stick_y(),
                current.stick_y(),
                config.tap_y_threshold,
            ),
            trigger: update_binary_timer(
                self.trigger,
                previous.trigger_timer_active_with_config(config),
                current.trigger_timer_active_with_config(config),
            ),
        }
    }
}

impl Default for MeleeInputTimers {
    fn default() -> Self {
        Self::expired()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeleeInputThresholds {
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
    pub tap_jump_y: i8,
    pub tap_jump_window: u8,
    pub tap_jump_release_y: i8,
    pub fast_fall_y: i8,
    pub fast_fall_window: u8,
    pub c_stick: i8,
    pub aerial_neutral_x: i8,
    pub aerial_neutral_y: i8,
    pub aerial_vertical_angle_tan_milli: i32,
    pub z_shield_analog: u8,
    pub escape_x: i8,
    pub escape_x_tap_window: u8,
    pub escape_y: i8,
    pub escape_y_tap_window: u8,
    pub special_side_x: i8,
    pub special_vertical_y: i8,
}

impl Default for MeleeInputThresholds {
    fn default() -> Self {
        crate::common_data::MeleeCommonData::provisional_mole().input_thresholds()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MeleeJumpInput {
    #[default]
    None,
    LStick,
    XY,
    CStick,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum WalkSpeedBucket {
    #[default]
    None,
    Slow,
    Middle,
    Fast,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MeleeInputFacts {
    pub walk_direction: i8,
    pub walk_speed_bucket: WalkSpeedBucket,
    pub turn_direction: i8,
    pub tilt_direction: (i8, i8),
    pub horizontal_smash_direction: i8,
    pub held_dash_x_direction: i8,
    pub dash_direction: i8,
    pub main_stick_spot_dodge: bool,
    pub cstick_spot_dodge: bool,
    pub crouch: bool,
    pub tap_jump: bool,
    pub button_jump_pressed: bool,
    pub button_jump_held: bool,
    pub cstick_jump: bool,
    pub normal_jump_input: MeleeJumpInput,
    pub normal_jump_pressed: bool,
    pub jump_input: MeleeJumpInput,
    pub jump_pressed: bool,
    pub fast_fall: bool,
    pub lstick_jump_released: bool,
    pub cstick_jump_released: bool,
    pub source_held: MeleeSourceButtonState,
    pub source_pressed: MeleeSourceButtonState,
    pub source_released: MeleeSourceButtonState,
    pub shield_held: bool,
    pub shield_pressed: bool,
    pub shield_released: bool,
    pub analog_shield: u8,
    pub analog_shield_pressed: bool,
    pub digital_shield_pressed: bool,
    pub air_dodge_pressed: bool,
    pub spot_dodge: bool,
    pub roll_direction: i8,
    pub left_trigger_analog_held: bool,
    pub right_trigger_analog_held: bool,
    pub left_trigger_analog_pressed: bool,
    pub right_trigger_analog_pressed: bool,
    pub left_trigger_digital_pressed: bool,
    pub right_trigger_digital_pressed: bool,
    pub cstick_direction: (i8, i8),
    pub attack_pressed: bool,
    pub air_attack_pressed: bool,
    pub air_attack_direction: (i8, i8),
    pub special_pressed: bool,
    pub special_direction: (i8, i8),
    pub air_special_direction: (i8, i8),
    pub grab_pressed: bool,
    pub neutral_attack_pressed: bool,
    pub tilt_attack_direction: (i8, i8),
    pub smash_attack_direction: (i8, i8),
    pub cstick_smash_direction: (i8, i8),
    pub dpad_up: bool,
    pub dpad_down: bool,
    pub dpad_left: bool,
    pub dpad_right: bool,
}

impl MeleeInputSnapshot {
    pub fn facts(self, thresholds: MeleeInputThresholds) -> MeleeInputFacts {
        let walk_direction = axis_direction(self.lstick.0, thresholds.walk_x);
        let walk_speed_bucket = walk_speed_bucket(self.lstick.0, thresholds);
        let turn_direction = axis_direction(self.lstick.0, thresholds.turn_x);
        let horizontal_smash_direction = if self.x_tap_timer < thresholds.dash_tap_window {
            axis_direction(self.lstick.0, thresholds.dash_x)
        } else {
            0
        };
        let held_dash_x_direction = axis_direction(self.lstick.0, thresholds.dash_x);
        let dash_direction = horizontal_smash_direction;
        let tilt_direction = (
            axis_direction(self.lstick.0, thresholds.tilt_x),
            axis_direction(self.lstick.1, thresholds.tilt_y),
        );
        let tap_jump = self.y_tap_timer < thresholds.tap_jump_window
            && (self.lstick.1 as i16) >= threshold_abs(thresholds.tap_jump_y);
        let button_jump_pressed = self.pressed.x() || self.pressed.y();
        let button_jump_held = self.held.x() || self.held.y();
        let cstick_jump = (self.cstick.1 as i16) >= threshold_abs(thresholds.tap_jump_y);
        let normal_jump_input = jump_input(tap_jump, button_jump_pressed, false);
        let jump_input = jump_input(tap_jump, button_jump_pressed, cstick_jump);
        let fast_fall = self.y_tap_timer < thresholds.fast_fall_window
            && (self.lstick.1 as i16) <= -threshold_abs(thresholds.fast_fall_y);
        let lstick_jump_released =
            (self.lstick.1 as i16) < threshold_abs(thresholds.tap_jump_release_y);
        let cstick_jump_released =
            (self.cstick.1 as i16) < threshold_abs(thresholds.tap_jump_release_y);
        let z_held = self.held.z();
        let z_pressed = self.pressed.z();
        let z_released = self.released.z();
        let previous_held = GameCubeButtonState::from_bits(
            (self.held.bits() & !self.pressed.bits()) | self.released.bits(),
        );
        let source_held =
            source_button_state(self.held, self.left_trigger != 0 || self.right_trigger != 0);
        let previous_source_held = source_button_state(
            previous_held,
            self.prev_left_trigger != 0 || self.prev_right_trigger != 0,
        );
        let source_pressed = source_held.pressed_since(previous_source_held);
        let source_released = previous_source_held.released_since(source_held);
        let attack_pressed = self.pressed.a() || z_pressed;
        let special_pressed = self.pressed.b();
        let special_direction =
            grounded_special_direction(special_pressed, self.lstick, thresholds);
        let air_special_direction =
            airborne_special_direction(special_pressed, self.lstick, thresholds);
        let grab_pressed = z_pressed;
        let vertical_smash_direction = if self.y_tap_timer < thresholds.dash_tap_window {
            axis_direction(self.lstick.1, thresholds.smash_y)
        } else {
            0
        };
        let smash_attack_direction = attack_direction(
            attack_pressed,
            horizontal_smash_direction,
            vertical_smash_direction,
        );
        let tilt_attack_direction = tilt_attack_direction(
            attack_pressed,
            self.lstick,
            thresholds,
            smash_attack_direction,
        );
        let neutral_attack_pressed =
            attack_pressed && tilt_attack_direction == (0, 0) && smash_attack_direction == (0, 0);
        let cstick_direction = (
            axis_direction(self.cstick.0, thresholds.c_stick),
            axis_direction(self.cstick.1, thresholds.c_stick),
        );
        let prev_cstick_direction = (
            axis_direction(self.prev_cstick.0, thresholds.c_stick),
            axis_direction(self.prev_cstick.1, thresholds.c_stick),
        );
        let cstick_smash_direction =
            priority_cstick_smash_direction(cstick_direction, prev_cstick_direction);
        let cstick_air_attack_pressed =
            cstick_air_attack_pressed(self.cstick, self.prev_cstick, thresholds);
        let air_attack_direction = if cstick_air_attack_pressed {
            air_attack_direction_from_stick(self.cstick, thresholds)
        } else if attack_pressed {
            air_attack_direction_from_stick(self.lstick, thresholds)
        } else {
            (0, 0)
        };

        let digital_shield_held = self.held.l() || self.held.r();
        let digital_shield_pressed = self.pressed.l() || self.pressed.r();
        let analog_shield_pressed =
            self.left_trigger_analog_pressed || self.right_trigger_analog_pressed;
        let trigger_analog_shield = if digital_shield_held {
            u8::MAX
        } else {
            self.left_trigger.max(self.right_trigger)
        };
        let analog_shield = if z_held {
            thresholds.z_shield_analog
        } else {
            trigger_analog_shield
        };
        let main_stick_spot_dodge =
            shield_main_stick_spot_dodge(self.lstick, self.y_tap_timer, thresholds);
        let cstick_spot_dodge = shield_cstick_spot_dodge(self.cstick, thresholds);
        let spot_dodge = main_stick_spot_dodge || cstick_spot_dodge;
        let roll_direction =
            shield_roll_direction(self.lstick, self.cstick, self.x_tap_timer, thresholds);

        MeleeInputFacts {
            walk_direction,
            walk_speed_bucket,
            turn_direction,
            tilt_direction,
            horizontal_smash_direction,
            held_dash_x_direction,
            dash_direction,
            main_stick_spot_dodge,
            cstick_spot_dodge,
            crouch: (self.lstick.1 as i16) <= -threshold_abs(thresholds.crouch_y),
            tap_jump,
            button_jump_pressed,
            button_jump_held,
            cstick_jump,
            normal_jump_input,
            normal_jump_pressed: normal_jump_input != MeleeJumpInput::None,
            jump_input,
            jump_pressed: jump_input != MeleeJumpInput::None,
            fast_fall,
            lstick_jump_released,
            cstick_jump_released,
            source_held,
            source_pressed,
            source_released,
            shield_held: self.shield_held || z_held,
            shield_pressed: self.shield_pressed || z_pressed,
            shield_released: self.shield_released || z_released,
            analog_shield,
            analog_shield_pressed,
            digital_shield_pressed,
            air_dodge_pressed: digital_shield_pressed,
            spot_dodge,
            roll_direction,
            left_trigger_analog_held: self.left_trigger_analog_held,
            right_trigger_analog_held: self.right_trigger_analog_held,
            left_trigger_analog_pressed: self.left_trigger_analog_pressed,
            right_trigger_analog_pressed: self.right_trigger_analog_pressed,
            left_trigger_digital_pressed: self.pressed.l(),
            right_trigger_digital_pressed: self.pressed.r(),
            cstick_direction,
            attack_pressed,
            air_attack_pressed: attack_pressed || cstick_air_attack_pressed,
            air_attack_direction,
            special_pressed,
            special_direction,
            air_special_direction,
            grab_pressed,
            neutral_attack_pressed,
            tilt_attack_direction,
            smash_attack_direction,
            cstick_smash_direction,
            dpad_up: self.held.dpad_up(),
            dpad_down: self.held.dpad_down(),
            dpad_left: self.held.dpad_left(),
            dpad_right: self.held.dpad_right(),
        }
    }
}

impl MeleeInputFacts {
    pub fn short_hop_released_for(self, jump_input: MeleeJumpInput) -> bool {
        match jump_input {
            MeleeJumpInput::None => false,
            MeleeJumpInput::LStick => self.lstick_jump_released,
            MeleeJumpInput::XY => !self.button_jump_held,
            MeleeJumpInput::CStick => self.cstick_jump_released,
        }
    }

    pub fn forward_dash_direction(self, facing: i8) -> i8 {
        let facing = sign_direction(facing);
        if facing != 0 && self.dash_direction == facing {
            self.dash_direction
        } else {
            0
        }
    }

    pub fn smash_turn_direction(self, facing: i8) -> i8 {
        let facing = sign_direction(facing);
        if facing != 0 && self.dash_direction == -facing {
            self.dash_direction
        } else {
            0
        }
    }

    pub fn standing_turn_direction(self, facing: i8) -> i8 {
        let facing = sign_direction(facing);
        if facing != 0 && self.turn_direction == -facing {
            self.turn_direction
        } else {
            0
        }
    }
}

fn source_button_state(
    raw: GameCubeButtonState,
    analog_trigger_shield_held: bool,
) -> MeleeSourceButtonState {
    let source = MeleeSourceButtonState::empty()
        .with_button(SOURCE_DPAD_LEFT_BIT, raw.dpad_left())
        .with_button(SOURCE_DPAD_RIGHT_BIT, raw.dpad_right())
        .with_button(SOURCE_DPAD_DOWN_BIT, raw.dpad_down())
        .with_button(SOURCE_DPAD_UP_BIT, raw.dpad_up())
        .with_button(SOURCE_Z_BIT, raw.z())
        .with_button(SOURCE_R_BIT, raw.r())
        .with_button(SOURCE_L_BIT, raw.l())
        .with_button(SOURCE_A_BIT, raw.a())
        .with_button(SOURCE_B_BIT, raw.b())
        .with_button(SOURCE_X_BIT, raw.x())
        .with_button(SOURCE_Y_BIT, raw.y())
        .with_button(SOURCE_START_BIT, raw.start());

    source
        .with_button(SOURCE_A_BIT, source.a() || raw.z())
        .with_button(
            SOURCE_LR_BIT,
            raw.l() || raw.r() || analog_trigger_shield_held || raw.z(),
        )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeleeInputProcessor {
    config: MeleeInputConfig,
    prev_lstick: (i8, i8),
    prev_cstick: (i8, i8),
    prev_left_trigger: u8,
    prev_right_trigger: u8,
    prev_held: GameCubeButtonState,
    prev_shield_held: bool,
    prev_left_trigger_analog_held: bool,
    prev_right_trigger_analog_held: bool,
    x_tap_timer: u8,
    y_tap_timer: u8,
    trigger_timer: u8,
    prev_trigger_active: bool,
}

impl MeleeInputProcessor {
    pub const fn new(config: MeleeInputConfig) -> Self {
        Self {
            config,
            prev_lstick: (0, 0),
            prev_cstick: (0, 0),
            prev_left_trigger: 0,
            prev_right_trigger: 0,
            prev_held: GameCubeButtonState::empty(),
            prev_shield_held: false,
            prev_left_trigger_analog_held: false,
            prev_right_trigger_analog_held: false,
            x_tap_timer: MAX_MELEE_INPUT_TIMER,
            y_tap_timer: MAX_MELEE_INPUT_TIMER,
            trigger_timer: MAX_MELEE_INPUT_TIMER,
            prev_trigger_active: false,
        }
    }

    pub fn update(&mut self, pad: GameCubePadStatus) -> MeleeInputSnapshot {
        let lstick = (
            clean_axis_to_i8(pad.stick_x, self.config.main_stick_deadzone_x),
            clean_axis_to_i8(pad.stick_y, self.config.main_stick_deadzone_y),
        );
        let cstick = (
            clean_axis_to_i8(pad.c_stick_x, self.config.c_stick_deadzone_x),
            clean_axis_to_i8(pad.c_stick_y, self.config.c_stick_deadzone_y),
        );
        let left_trigger = clean_trigger(pad.left_trigger, self.config.trigger_deadzone);
        let right_trigger = clean_trigger(pad.right_trigger, self.config.trigger_deadzone);
        let held = pad.buttons;
        let changed = self.prev_held.bits() ^ held.bits();
        let pressed = GameCubeButtonState::from_bits(held.bits() & changed);
        let released = GameCubeButtonState::from_bits(self.prev_held.bits() & changed);
        let left_trigger_analog_held = left_trigger >= self.config.trigger_threshold;
        let right_trigger_analog_held = right_trigger >= self.config.trigger_threshold;
        let left_trigger_analog_pressed =
            left_trigger_analog_held && !self.prev_left_trigger_analog_held;
        let right_trigger_analog_pressed =
            right_trigger_analog_held && !self.prev_right_trigger_analog_held;
        let shield_held = pad.buttons.l()
            || pad.buttons.r()
            || left_trigger_analog_held
            || right_trigger_analog_held;
        let trigger_timer_active = left_trigger >= self.config.trigger_timer_threshold
            || right_trigger >= self.config.trigger_timer_threshold
            || pad.buttons.l()
            || pad.buttons.r();

        self.x_tap_timer = update_axis_tap_timer(
            self.x_tap_timer,
            self.prev_lstick.0,
            lstick.0,
            self.config.tap_x_threshold,
        );
        self.y_tap_timer = update_axis_tap_timer(
            self.y_tap_timer,
            self.prev_lstick.1,
            lstick.1,
            self.config.tap_y_threshold,
        );
        self.trigger_timer = update_binary_timer(
            self.trigger_timer,
            self.prev_trigger_active,
            trigger_timer_active,
        );

        let snapshot = MeleeInputSnapshot {
            lstick,
            prev_lstick: self.prev_lstick,
            cstick,
            prev_cstick: self.prev_cstick,
            left_trigger,
            right_trigger,
            prev_left_trigger: self.prev_left_trigger,
            prev_right_trigger: self.prev_right_trigger,
            held,
            pressed,
            released,
            shield_held,
            shield_pressed: shield_held && !self.prev_shield_held,
            shield_released: !shield_held && self.prev_shield_held,
            left_trigger_analog_held,
            right_trigger_analog_held,
            left_trigger_analog_pressed,
            right_trigger_analog_pressed,
            x_tap_timer: self.x_tap_timer,
            y_tap_timer: self.y_tap_timer,
            trigger_timer: self.trigger_timer,
        };

        self.prev_lstick = lstick;
        self.prev_cstick = cstick;
        self.prev_left_trigger = left_trigger;
        self.prev_right_trigger = right_trigger;
        self.prev_held = held;
        self.prev_shield_held = shield_held;
        self.prev_left_trigger_analog_held = left_trigger_analog_held;
        self.prev_right_trigger_analog_held = right_trigger_analog_held;
        self.prev_trigger_active = trigger_timer_active;

        snapshot
    }
}

impl Default for MeleeInputProcessor {
    fn default() -> Self {
        Self::new(MeleeInputConfig::default())
    }
}

pub const fn gamecube_axis_to_i16(value: u8) -> i16 {
    ((value as i32 - 128) * 256) as i16
}

pub const fn gamecube_axis_to_i8(value: u8) -> i8 {
    (value as i16 - 128) as i8
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlayerInput {
    bits: u64,
}

impl PlayerInput {
    pub const fn neutral() -> Self {
        Self { bits: 0 }
    }

    pub const fn from_bits(bits: u64) -> Self {
        Self {
            bits: bits & USED_BITS,
        }
    }

    pub const fn bits(self) -> u64 {
        self.bits
    }

    pub fn with_left_stick(mut self, x: i8, y: i8) -> Self {
        self.bits &= !((STICK_BYTE_MASK << STICK_X_SHIFT) | (STICK_BYTE_MASK << STICK_Y_SHIFT));
        self.bits |= ((x as u8 as u64) << STICK_X_SHIFT) | ((y as u8 as u64) << STICK_Y_SHIFT);
        self
    }

    pub fn with_c_stick(mut self, x: i8, y: i8) -> Self {
        self.bits &= !((STICK_BYTE_MASK << C_STICK_X_SHIFT) | (STICK_BYTE_MASK << C_STICK_Y_SHIFT));
        self.bits |= ((x as u8 as u64) << C_STICK_X_SHIFT) | ((y as u8 as u64) << C_STICK_Y_SHIFT);
        self
    }

    pub fn with_left_trigger_analog(mut self, value: u8) -> Self {
        self.bits &= !(STICK_BYTE_MASK << LEFT_TRIGGER_ANALOG_SHIFT);
        self.bits |= (value as u64) << LEFT_TRIGGER_ANALOG_SHIFT;
        self
    }

    pub fn with_right_trigger_analog(mut self, value: u8) -> Self {
        self.bits &= !(STICK_BYTE_MASK << RIGHT_TRIGGER_ANALOG_SHIFT);
        self.bits |= (value as u64) << RIGHT_TRIGGER_ANALOG_SHIFT;
        self
    }

    pub fn with_attack(self, pressed: bool) -> Self {
        self.with_button(ATTACK_BIT, pressed)
    }

    pub fn with_special(self, pressed: bool) -> Self {
        self.with_button(SPECIAL_BIT, pressed)
    }

    pub fn with_jump(self, pressed: bool) -> Self {
        self.with_jump_primary(pressed)
    }

    pub fn with_jump_primary(self, pressed: bool) -> Self {
        self.with_button(JUMP_BIT, pressed)
    }

    pub fn with_jump_secondary(self, pressed: bool) -> Self {
        self.with_button(JUMP_SECONDARY_BIT, pressed)
    }

    pub fn with_shield(self, pressed: bool) -> Self {
        self.with_button(SHIELD_BIT, pressed)
    }

    pub fn with_grab(self, pressed: bool) -> Self {
        self.with_button(GRAB_BIT, pressed)
    }

    pub fn with_start(self, pressed: bool) -> Self {
        self.with_button(START_BIT, pressed)
    }

    pub fn with_left_trigger_digital(self, pressed: bool) -> Self {
        self.with_button(LEFT_TRIGGER_DIGITAL_BIT, pressed)
    }

    pub fn with_right_trigger_digital(self, pressed: bool) -> Self {
        self.with_button(RIGHT_TRIGGER_DIGITAL_BIT, pressed)
    }

    pub fn with_dpad_up(self, pressed: bool) -> Self {
        self.with_button(DPAD_UP_BIT, pressed)
    }

    pub fn with_dpad_down(self, pressed: bool) -> Self {
        self.with_button(DPAD_DOWN_BIT, pressed)
    }

    pub fn with_dpad_left(self, pressed: bool) -> Self {
        self.with_button(DPAD_LEFT_BIT, pressed)
    }

    pub fn with_dpad_right(self, pressed: bool) -> Self {
        self.with_button(DPAD_RIGHT_BIT, pressed)
    }

    pub fn with_ucf_dashback_amendment(self, active: bool) -> Self {
        self.with_button(UCF_DASHBACK_AMENDMENT_BIT, active)
    }

    pub const fn attack(self) -> bool {
        self.bits & ATTACK_BIT != 0
    }

    pub const fn special(self) -> bool {
        self.bits & SPECIAL_BIT != 0
    }

    pub const fn jump(self) -> bool {
        self.jump_primary() || self.jump_secondary()
    }

    pub const fn jump_primary(self) -> bool {
        self.bits & JUMP_BIT != 0
    }

    pub const fn jump_secondary(self) -> bool {
        self.bits & JUMP_SECONDARY_BIT != 0
    }

    pub const fn shield(self) -> bool {
        self.bits & SHIELD_BIT != 0 || self.trigger_active()
    }

    pub const fn shield_with_config(self, config: MeleeInputConfig) -> bool {
        self.bits & SHIELD_BIT != 0 || self.trigger_active_with_config(config)
    }

    pub const fn explicit_shield(self) -> bool {
        self.bits & SHIELD_BIT != 0
    }

    pub const fn grab(self) -> bool {
        self.bits & GRAB_BIT != 0
    }

    pub const fn start(self) -> bool {
        self.bits & START_BIT != 0
    }

    pub const fn left_trigger_digital(self) -> bool {
        self.bits & LEFT_TRIGGER_DIGITAL_BIT != 0
    }

    pub const fn right_trigger_digital(self) -> bool {
        self.bits & RIGHT_TRIGGER_DIGITAL_BIT != 0
    }

    pub const fn dpad_up(self) -> bool {
        self.bits & DPAD_UP_BIT != 0
    }

    pub const fn dpad_down(self) -> bool {
        self.bits & DPAD_DOWN_BIT != 0
    }

    pub const fn dpad_left(self) -> bool {
        self.bits & DPAD_LEFT_BIT != 0
    }

    pub const fn dpad_right(self) -> bool {
        self.bits & DPAD_RIGHT_BIT != 0
    }

    pub const fn ucf_dashback_amendment(self) -> bool {
        self.bits & UCF_DASHBACK_AMENDMENT_BIT != 0
    }

    pub const fn stick_x(self) -> i8 {
        ((self.bits >> STICK_X_SHIFT) as u8) as i8
    }

    pub const fn stick_y(self) -> i8 {
        ((self.bits >> STICK_Y_SHIFT) as u8) as i8
    }

    pub const fn c_stick_x(self) -> i8 {
        ((self.bits >> C_STICK_X_SHIFT) as u8) as i8
    }

    pub const fn c_stick_y(self) -> i8 {
        ((self.bits >> C_STICK_Y_SHIFT) as u8) as i8
    }

    pub const fn left_trigger_analog(self) -> u8 {
        (self.bits >> LEFT_TRIGGER_ANALOG_SHIFT) as u8
    }

    pub const fn right_trigger_analog(self) -> u8 {
        (self.bits >> RIGHT_TRIGGER_ANALOG_SHIFT) as u8
    }

    pub const fn trigger_active(self) -> bool {
        self.trigger_active_with_config(DEFAULT_MELEE_INPUT_CONFIG)
    }

    pub const fn trigger_active_with_config(self, config: MeleeInputConfig) -> bool {
        self.left_trigger_digital()
            || self.right_trigger_digital()
            || self.left_trigger_analog() >= config.trigger_threshold
            || self.right_trigger_analog() >= config.trigger_threshold
    }

    pub const fn trigger_timer_active(self) -> bool {
        self.trigger_timer_active_with_config(DEFAULT_MELEE_INPUT_CONFIG)
    }

    pub const fn trigger_timer_active_with_config(self, config: MeleeInputConfig) -> bool {
        self.left_trigger_digital()
            || self.right_trigger_digital()
            || self.left_trigger_analog() >= config.trigger_timer_threshold
            || self.right_trigger_analog() >= config.trigger_timer_threshold
    }

    pub fn melee_snapshot(
        self,
        previous: PlayerInput,
        timers: MeleeInputTimers,
    ) -> MeleeInputSnapshot {
        self.melee_snapshot_with_config(previous, timers, MeleeInputConfig::default())
    }

    pub fn melee_snapshot_with_config(
        self,
        previous: PlayerInput,
        timers: MeleeInputTimers,
        config: MeleeInputConfig,
    ) -> MeleeInputSnapshot {
        let updated_timers = timers.update_with_config(previous, self, config);
        let held = self.gamecube_buttons();
        let previous_held = previous.gamecube_buttons();
        let changed = previous_held.bits() ^ held.bits();
        let pressed = GameCubeButtonState::from_bits(held.bits() & changed);
        let released = GameCubeButtonState::from_bits(previous_held.bits() & changed);
        let left_trigger = self.left_trigger_analog();
        let right_trigger = self.right_trigger_analog();
        let previous_left_trigger = previous.left_trigger_analog();
        let previous_right_trigger = previous.right_trigger_analog();
        let left_trigger_analog_held = left_trigger >= config.trigger_threshold;
        let right_trigger_analog_held = right_trigger >= config.trigger_threshold;
        let left_trigger_analog_pressed =
            left_trigger_analog_held && previous_left_trigger < config.trigger_threshold;
        let right_trigger_analog_pressed =
            right_trigger_analog_held && previous_right_trigger < config.trigger_threshold;
        let shield_held = self.shield_with_config(config);
        let previous_shield_held = previous.shield_with_config(config);

        MeleeInputSnapshot {
            lstick: (self.stick_x(), self.stick_y()),
            prev_lstick: (previous.stick_x(), previous.stick_y()),
            cstick: (self.c_stick_x(), self.c_stick_y()),
            prev_cstick: (previous.c_stick_x(), previous.c_stick_y()),
            left_trigger,
            right_trigger,
            prev_left_trigger: previous_left_trigger,
            prev_right_trigger: previous_right_trigger,
            held,
            pressed,
            released,
            shield_held,
            shield_pressed: shield_held && !previous_shield_held,
            shield_released: !shield_held && previous_shield_held,
            left_trigger_analog_held,
            right_trigger_analog_held,
            left_trigger_analog_pressed,
            right_trigger_analog_pressed,
            x_tap_timer: updated_timers.x_tap,
            y_tap_timer: updated_timers.y_tap,
            trigger_timer: updated_timers.trigger,
        }
    }

    fn with_button(mut self, bit: u64, pressed: bool) -> Self {
        if pressed {
            self.bits |= bit;
        } else {
            self.bits &= !bit;
        }
        self
    }

    fn gamecube_buttons(self) -> GameCubeButtonState {
        GameCubeButtonState::empty()
            .with_a(self.attack())
            .with_b(self.special())
            .with_x(self.jump_primary())
            .with_y(self.jump_secondary())
            .with_z(self.grab())
            .with_l(self.left_trigger_digital())
            .with_r(self.right_trigger_digital())
            .with_start(self.start())
            .with_dpad_up(self.dpad_up())
            .with_dpad_down(self.dpad_down())
            .with_dpad_left(self.dpad_left())
            .with_dpad_right(self.dpad_right())
    }
}

fn update_axis_tap_timer(timer: u8, previous: i8, current: i8, threshold: i8) -> u8 {
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

fn update_binary_timer(timer: u8, previous_active: bool, current_active: bool) -> u8 {
    match (previous_active, current_active) {
        (true, true) => increment_melee_timer(timer),
        (false, true) => 0,
        (_, false) => MAX_MELEE_INPUT_TIMER,
    }
}

fn increment_melee_timer(timer: u8) -> u8 {
    timer.saturating_add(1).min(MAX_MELEE_INPUT_TIMER)
}

fn clean_axis_to_i8(value: u8, deadzone: i8) -> i8 {
    let axis = gamecube_axis_to_i8(value);
    if (axis as i16).abs() <= threshold_abs(deadzone) {
        0
    } else {
        axis
    }
}

fn clean_trigger(value: u8, deadzone: u8) -> u8 {
    if value <= deadzone {
        0
    } else {
        value
    }
}

fn axis_direction(value: i8, threshold: i8) -> i8 {
    let threshold = threshold_abs(threshold);
    let value = value as i16;
    if value >= threshold {
        1
    } else if value <= -threshold {
        -1
    } else {
        0
    }
}

fn walk_speed_bucket(stick_x: i8, thresholds: MeleeInputThresholds) -> WalkSpeedBucket {
    let magnitude = stick_x.saturating_abs() as i16;
    let slow_threshold =
        threshold_abs(thresholds.walk_slow_x).max(threshold_abs(thresholds.walk_x));
    let middle_threshold = threshold_abs(thresholds.walk_middle_x).max(slow_threshold);
    let fast_threshold = threshold_abs(thresholds.walk_fast_x).max(middle_threshold);

    if magnitude < slow_threshold {
        WalkSpeedBucket::None
    } else if magnitude < middle_threshold {
        WalkSpeedBucket::Slow
    } else if magnitude < fast_threshold {
        WalkSpeedBucket::Middle
    } else {
        WalkSpeedBucket::Fast
    }
}

fn sign_direction(value: i8) -> i8 {
    if value > 0 {
        1
    } else if value < 0 {
        -1
    } else {
        0
    }
}

fn edge_direction(current: i8, previous: i8) -> i8 {
    if current != 0 && current != previous {
        current
    } else {
        0
    }
}

fn priority_cstick_smash_direction(current: (i8, i8), previous: (i8, i8)) -> (i8, i8) {
    let x = edge_direction(current.0, previous.0);
    if x != 0 {
        return (x, 0);
    }

    let y = edge_direction(current.1, previous.1);
    if y != 0 {
        return (0, y);
    }

    (0, 0)
}

fn jump_input(tap_jump: bool, button_jump_pressed: bool, cstick_jump: bool) -> MeleeJumpInput {
    if tap_jump {
        MeleeJumpInput::LStick
    } else if button_jump_pressed {
        MeleeJumpInput::XY
    } else if cstick_jump {
        MeleeJumpInput::CStick
    } else {
        MeleeJumpInput::None
    }
}

fn attack_direction(attack_pressed: bool, horizontal: i8, vertical: i8) -> (i8, i8) {
    if !attack_pressed {
        return (0, 0);
    }

    if horizontal != 0 {
        (horizontal, 0)
    } else if vertical != 0 {
        (0, vertical)
    } else {
        (0, 0)
    }
}

fn grounded_special_direction(
    special_pressed: bool,
    stick: (i8, i8),
    thresholds: MeleeInputThresholds,
) -> (i8, i8) {
    if !special_pressed {
        return (0, 0);
    }

    let side = axis_direction(stick.0, thresholds.special_side_x);
    if side != 0 {
        return (side, 0);
    }

    let y = stick.1 as i16;
    let x_abs = (stick.0 as i16).abs();
    let y_abs = y.abs();
    let side_threshold = threshold_abs(thresholds.special_side_x);
    let vertical_threshold = threshold_abs(thresholds.special_vertical_y);

    if y >= vertical_threshold {
        (0, 1)
    } else if y < -vertical_threshold {
        (0, -1)
    } else if x_abs < side_threshold && y_abs < vertical_threshold {
        (0, 0)
    } else {
        NO_GROUNDED_SPECIAL_DIRECTION
    }
}

fn airborne_special_direction(
    special_pressed: bool,
    stick: (i8, i8),
    thresholds: MeleeInputThresholds,
) -> (i8, i8) {
    if !special_pressed {
        return (0, 0);
    }

    if (stick.1 as i16) >= threshold_abs(thresholds.special_vertical_y) {
        return (0, 1);
    }

    if (stick.1 as i16) <= -threshold_abs(thresholds.special_vertical_y) {
        return (0, -1);
    }

    let side = axis_direction(stick.0, thresholds.special_side_x);
    if side != 0 {
        (side, 0)
    } else {
        (0, 0)
    }
}

fn cstick_air_attack_pressed(
    current: (i8, i8),
    previous: (i8, i8),
    thresholds: MeleeInputThresholds,
) -> bool {
    (threshold_crossed(current.0, previous.0, thresholds.aerial_neutral_x)
        || threshold_crossed(current.1, previous.1, thresholds.aerial_neutral_y))
        && current != (0, 0)
}

fn threshold_crossed(current: i8, previous: i8, threshold: i8) -> bool {
    (previous as i16).abs() < threshold_abs(threshold)
        && (current as i16).abs() >= threshold_abs(threshold)
}

fn air_attack_direction_from_stick(stick: (i8, i8), thresholds: MeleeInputThresholds) -> (i8, i8) {
    if (stick.0 as i16).abs() < threshold_abs(thresholds.aerial_neutral_x)
        && (stick.1 as i16).abs() < threshold_abs(thresholds.aerial_neutral_y)
    {
        return (0, 0);
    }

    if aerial_vertical_angle_exceeds(stick, thresholds.aerial_vertical_angle_tan_milli) {
        if stick.1 > 0 {
            return (0, 1);
        }
        if stick.1 < 0 {
            return (0, -1);
        }
    }

    if stick.0 > 0 {
        (1, 0)
    } else if stick.0 < 0 {
        (-1, 0)
    } else {
        (0, 0)
    }
}

fn aerial_vertical_angle_exceeds(stick: (i8, i8), tan_milli: i32) -> bool {
    let y = stick.1 as i32;
    if y == 0 {
        return false;
    }
    let abs_x = (stick.0 as i32).abs();
    let abs_y = y.abs();
    if abs_x == 0 {
        return true;
    }

    abs_y * 1000 > abs_x * tan_milli
}

fn shield_main_stick_spot_dodge(
    lstick: (i8, i8),
    y_tap_timer: u8,
    thresholds: MeleeInputThresholds,
) -> bool {
    y_tap_timer < thresholds.escape_y_tap_window
        && (lstick.1 as i16) < -threshold_abs(thresholds.escape_y)
}

fn shield_cstick_spot_dodge(cstick: (i8, i8), thresholds: MeleeInputThresholds) -> bool {
    (cstick.1 as i16) <= -threshold_abs(thresholds.escape_y)
}

fn shield_roll_direction(
    lstick: (i8, i8),
    cstick: (i8, i8),
    x_tap_timer: u8,
    thresholds: MeleeInputThresholds,
) -> i8 {
    if x_tap_timer < thresholds.escape_x_tap_window {
        let direction = axis_direction(lstick.0, thresholds.escape_x);
        if direction != 0 {
            return direction;
        }
    }

    axis_direction(cstick.0, thresholds.escape_x)
}

fn tilt_attack_direction(
    attack_pressed: bool,
    stick: (i8, i8),
    thresholds: MeleeInputThresholds,
    smash_attack_direction: (i8, i8),
) -> (i8, i8) {
    if !attack_pressed || smash_attack_direction != (0, 0) {
        return (0, 0);
    }

    let direction = (
        axis_direction(stick.0, thresholds.tilt_x),
        axis_direction(stick.1, thresholds.tilt_y),
    );

    match direction {
        (0, 0) => (0, 0),
        (x, 0) => (x, 0),
        (0, y) => (0, y),
        (x, y) => {
            if is_side_tilt_angle(stick) {
                (x, 0)
            } else {
                (0, y)
            }
        }
    }
}

fn is_side_tilt_angle(stick: (i8, i8)) -> bool {
    // Structural stand-in for ftCommonData::x20_radians until the exact DAT value is extracted.
    (stick.1 as i16).abs() <= (stick.0 as i16).abs()
}

fn threshold_abs(value: i8) -> i16 {
    (value as i16).abs()
}
