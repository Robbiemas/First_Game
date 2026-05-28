use mole_core::{Frame, PlayerInput};
use sdl3::event::Event;
use sdl3::gamepad::{Axis, Button, Gamepad};
use sdl3::keyboard::Scancode;
use sdl3::{EventPump, GamepadSubsystem, Sdl};

use crate::{map_physical_input, InputSource, PhysicalInput};

const DIGITAL_AXIS_MAX: i16 = 32_767;
const DIGITAL_AXIS_MIN: i16 = -32_768;

pub fn configure_sdl_controller_hints() {
    sdl3::hint::set("SDL_JOYSTICK_HIDAPI", "1");
    sdl3::hint::set("SDL_JOYSTICK_HIDAPI_GAMECUBE", "1");
}

pub struct SdlInputSource {
    event_pump: EventPump,
    gamepad_subsystem: GamepadSubsystem,
    gamepads: Vec<Gamepad>,
    quit_requested: bool,
}

impl SdlInputSource {
    pub fn new(sdl: &Sdl) -> Result<Self, String> {
        let event_pump = sdl.event_pump().map_err(|error| error.to_string())?;
        let gamepad_subsystem = sdl.gamepad().map_err(|error| error.to_string())?;
        gamepad_subsystem.set_events_processing_state(true);

        let mut source = Self {
            event_pump,
            gamepad_subsystem,
            gamepads: Vec::new(),
            quit_requested: false,
        };
        source.refresh_gamepads();
        Ok(source)
    }

    pub fn gamepad_count(&self) -> usize {
        self.gamepads.len()
    }

    pub fn quit_requested(&self) -> bool {
        self.quit_requested
    }

    pub fn refresh_gamepads(&mut self) {
        self.gamepads.clear();
        let Ok(ids) = self.gamepad_subsystem.gamepads() else {
            return;
        };

        for id in ids {
            if let Ok(gamepad) = self.gamepad_subsystem.open(id) {
                self.gamepads.push(gamepad);
                if self.gamepads.len() == 2 {
                    break;
                }
            }
        }
    }
}

impl InputSource for SdlInputSource {
    fn poll_inputs(&mut self, _frame: Frame) -> [PlayerInput; 2] {
        let mut gamepad_list_changed = false;
        for event in self.event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => self.quit_requested = true,
                Event::ControllerDeviceAdded { .. } | Event::ControllerDeviceRemoved { .. } => {
                    gamepad_list_changed = true;
                }
                _ => {}
            }
        }
        if gamepad_list_changed {
            self.refresh_gamepads();
        }

        self.gamepad_subsystem.update();

        let keyboard = self.event_pump.keyboard_state();
        let mut inputs = [
            map_physical_input(keyboard_physical_input(&keyboard)),
            PlayerInput::neutral(),
        ];

        for (index, gamepad) in self.gamepads.iter().take(2).enumerate() {
            inputs[index] = merge_player_inputs(
                inputs[index],
                map_physical_input(gamepad_physical_input(gamepad)),
            );
        }

        inputs
    }
}

fn keyboard_physical_input(keyboard: &sdl3::keyboard::KeyboardState<'_>) -> PhysicalInput {
    let left =
        keyboard.is_scancode_pressed(Scancode::A) || keyboard.is_scancode_pressed(Scancode::Left);
    let right =
        keyboard.is_scancode_pressed(Scancode::D) || keyboard.is_scancode_pressed(Scancode::Right);
    let up =
        keyboard.is_scancode_pressed(Scancode::W) || keyboard.is_scancode_pressed(Scancode::Up);
    let down =
        keyboard.is_scancode_pressed(Scancode::S) || keyboard.is_scancode_pressed(Scancode::Down);

    PhysicalInput {
        left_x: digital_axis(left, right),
        left_y: digital_axis(down, up),
        attack: keyboard.is_scancode_pressed(Scancode::Space)
            || keyboard.is_scancode_pressed(Scancode::J),
        special: keyboard.is_scancode_pressed(Scancode::U),
        jump_primary: up || keyboard.is_scancode_pressed(Scancode::K),
        jump_secondary: false,
        shield: keyboard.is_scancode_pressed(Scancode::L)
            || keyboard.is_scancode_pressed(Scancode::LShift)
            || keyboard.is_scancode_pressed(Scancode::RShift),
        grab: keyboard.is_scancode_pressed(Scancode::I),
        start: keyboard.is_scancode_pressed(Scancode::Return)
            || keyboard.is_scancode_pressed(Scancode::P),
        ..PhysicalInput::default()
    }
}

fn gamepad_physical_input(gamepad: &Gamepad) -> PhysicalInput {
    PhysicalInput {
        left_x: gamepad.axis(Axis::LeftX),
        left_y: invert_axis(gamepad.axis(Axis::LeftY)),
        c_x: gamepad.axis(Axis::RightX),
        c_y: invert_axis(gamepad.axis(Axis::RightY)),
        left_trigger: trigger_axis_to_u8(gamepad.axis(Axis::TriggerLeft)),
        right_trigger: trigger_axis_to_u8(gamepad.axis(Axis::TriggerRight)),
        attack: gamepad.button(Button::South),
        special: gamepad.button(Button::East),
        jump_primary: gamepad.button(Button::West),
        jump_secondary: gamepad.button(Button::North),
        shield: false,
        grab: gamepad.button(Button::Back)
            || gamepad.button(Button::Misc1)
            || gamepad.button(Button::LeftPaddle1)
            || gamepad.button(Button::RightPaddle1),
        left_trigger_pressed: gamepad.button(Button::LeftShoulder),
        right_trigger_pressed: gamepad.button(Button::RightShoulder),
        start: gamepad.button(Button::Start),
        ..PhysicalInput::default()
    }
}

fn digital_axis(negative: bool, positive: bool) -> i16 {
    match (negative, positive) {
        (true, false) => DIGITAL_AXIS_MIN,
        (false, true) => DIGITAL_AXIS_MAX,
        _ => 0,
    }
}

fn invert_axis(value: i16) -> i16 {
    (-(value as i32)).clamp(i16::MIN as i32, i16::MAX as i32) as i16
}

fn trigger_axis_to_u8(value: i16) -> u8 {
    let positive = (value as i32).clamp(0, i16::MAX as i32);
    (positive * 255 / i16::MAX as i32) as u8
}

fn merge_player_inputs(primary: PlayerInput, overlay: PlayerInput) -> PlayerInput {
    let x = if overlay.stick_x() != 0 {
        overlay.stick_x()
    } else {
        primary.stick_x()
    };
    let y = if overlay.stick_y() != 0 {
        overlay.stick_y()
    } else {
        primary.stick_y()
    };
    let c_x = if overlay.c_stick_x() != 0 {
        overlay.c_stick_x()
    } else {
        primary.c_stick_x()
    };
    let c_y = if overlay.c_stick_y() != 0 {
        overlay.c_stick_y()
    } else {
        primary.c_stick_y()
    };

    PlayerInput::neutral()
        .with_left_stick(x, y)
        .with_c_stick(c_x, c_y)
        .with_left_trigger_analog(
            primary
                .left_trigger_analog()
                .max(overlay.left_trigger_analog()),
        )
        .with_right_trigger_analog(
            primary
                .right_trigger_analog()
                .max(overlay.right_trigger_analog()),
        )
        .with_left_trigger_digital(primary.left_trigger_digital() || overlay.left_trigger_digital())
        .with_right_trigger_digital(
            primary.right_trigger_digital() || overlay.right_trigger_digital(),
        )
        .with_attack(primary.attack() || overlay.attack())
        .with_special(primary.special() || overlay.special())
        .with_jump_primary(primary.jump_primary() || overlay.jump_primary())
        .with_jump_secondary(primary.jump_secondary() || overlay.jump_secondary())
        .with_shield(primary.explicit_shield() || overlay.explicit_shield())
        .with_grab(primary.grab() || overlay.grab())
        .with_start(primary.start() || overlay.start())
        .with_dpad_up(primary.dpad_up() || overlay.dpad_up())
        .with_dpad_down(primary.dpad_down() || overlay.dpad_down())
        .with_dpad_left(primary.dpad_left() || overlay.dpad_left())
        .with_dpad_right(primary.dpad_right() || overlay.dpad_right())
}

#[cfg(test)]
mod tests {
    use super::merge_player_inputs;
    use mole_core::PlayerInput;

    #[test]
    fn merge_preserves_trigger_identity_without_promoting_it_to_generic_shield() {
        let keyboard_shield = PlayerInput::neutral().with_shield(true);
        let trigger_shield = PlayerInput::neutral()
            .with_left_trigger_analog(80)
            .with_right_trigger_digital(true);

        let trigger_only = merge_player_inputs(PlayerInput::neutral(), trigger_shield);
        assert!(trigger_only.shield());
        assert!(!trigger_only.explicit_shield());
        assert_eq!(trigger_only.left_trigger_analog(), 80);
        assert!(trigger_only.right_trigger_digital());

        let merged = merge_player_inputs(keyboard_shield, trigger_shield);
        assert!(merged.shield());
        assert!(merged.explicit_shield());
        assert_eq!(merged.left_trigger_analog(), 80);
        assert!(merged.right_trigger_digital());
    }
}
