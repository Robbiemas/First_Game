use mole_core::{MeleeInputProcessor, MeleeInputSnapshot};

pub use mole_core::{GameCubeButtonState, GameCubePadStatus, PlayerInput};

const GAMECUBE_STICK_CENTER: i16 = 128;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InputMappingConfig {
    pub trigger_deadzone: u8,
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
    processor: MeleeInputProcessor,
}

impl GameCubeInputMapper {
    pub fn new(config: InputMappingConfig) -> Self {
        Self {
            config,
            processor: MeleeInputProcessor::default(),
        }
    }

    pub fn map_gamecube_pad(&mut self, pad: GameCubePadStatus) -> PlayerInput {
        let adjusted = GameCubePadStatus {
            left_trigger: map_trigger_analog(pad.left_trigger, self.config.trigger_deadzone),
            right_trigger: map_trigger_analog(pad.right_trigger, self.config.trigger_deadzone),
            ..pad
        };
        player_input_from_snapshot(self.processor.update(adjusted))
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

fn player_input_from_snapshot(snapshot: MeleeInputSnapshot) -> PlayerInput {
    PlayerInput::neutral()
        .with_left_stick(snapshot.lstick.0, snapshot.lstick.1)
        .with_c_stick(snapshot.cstick.0, snapshot.cstick.1)
        .with_left_trigger_analog(snapshot.left_trigger)
        .with_right_trigger_analog(snapshot.right_trigger)
        .with_left_trigger_digital(snapshot.held.l())
        .with_right_trigger_digital(snapshot.held.r())
        .with_attack(snapshot.held.a())
        .with_special(snapshot.held.b())
        .with_jump_primary(snapshot.held.x())
        .with_jump_secondary(snapshot.held.y())
        .with_grab(snapshot.held.z())
        .with_start(snapshot.held.start())
        .with_dpad_up(snapshot.held.dpad_up())
        .with_dpad_down(snapshot.held.dpad_down())
        .with_dpad_left(snapshot.held.dpad_left())
        .with_dpad_right(snapshot.held.dpad_right())
        .with_ucf_x_tilt_intent(snapshot.ucf_x_tilt_intent)
        .with_ucf_shield_drop_tilt_intent(snapshot.ucf_shield_drop_tilt_intent)
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
