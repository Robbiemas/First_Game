use mole_core::{
    Frame, GameCubePadStatus, MeleeInputFacts, MotionState, PlayerInput, Vec2, World,
    WorldSnapshot, TICK_NANOS,
};

#[cfg(feature = "sdl")]
pub mod sdl_input;

#[cfg(feature = "sdl")]
pub use sdl_input::{configure_sdl_controller_hints, SdlInputSource};

pub mod readout;
pub use readout::{ButtonReadout, InputReadout, MeleeReadout, PlayerReadout};

pub mod wup_input;
pub use wup_input::{map_wup_ports_to_player_inputs, parse_wup_report, WupInputMapper, WupPort};

#[cfg(feature = "wup")]
pub use wup_input::WupInputSource;

const DEFAULT_MAX_TICKS_PER_UPDATE: u32 = 5;
const AXIS_DEADZONE: i16 = 8_000;
const PLAYER_RENDER_WIDTH: u32 = 48;
const PLAYER_RENDER_HEIGHT: u32 = 72;
const WORLD_TO_SCREEN_SCALE: i32 = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedStepClock {
    accumulator_nanos: u64,
    max_ticks_per_update: u32,
}

impl Default for FixedStepClock {
    fn default() -> Self {
        Self {
            accumulator_nanos: 0,
            max_ticks_per_update: DEFAULT_MAX_TICKS_PER_UPDATE,
        }
    }
}

impl FixedStepClock {
    pub fn add_elapsed_nanos(&mut self, elapsed_nanos: u64) -> u32 {
        self.accumulator_nanos = self.accumulator_nanos.saturating_add(elapsed_nanos);
        let available_ticks = self.accumulator_nanos / TICK_NANOS;
        let emitted_ticks = available_ticks.min(self.max_ticks_per_update as u64) as u32;

        if available_ticks > self.max_ticks_per_update as u64 {
            self.accumulator_nanos = 0;
        } else {
            self.accumulator_nanos -= emitted_ticks as u64 * TICK_NANOS;
        }

        emitted_ticks
    }
}

pub trait InputSource {
    fn poll_inputs(&mut self, frame: Frame) -> [PlayerInput; 2];
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PhysicalInput {
    pub left_x: i16,
    pub left_y: i16,
    pub c_x: i16,
    pub c_y: i16,
    pub left_trigger: u8,
    pub right_trigger: u8,
    pub attack: bool,
    pub special: bool,
    pub jump_primary: bool,
    pub jump_secondary: bool,
    pub shield: bool,
    pub grab: bool,
    pub left_trigger_pressed: bool,
    pub right_trigger_pressed: bool,
    pub start: bool,
    pub dpad_up: bool,
    pub dpad_down: bool,
    pub dpad_left: bool,
    pub dpad_right: bool,
}

pub fn map_physical_input(input: PhysicalInput) -> PlayerInput {
    PlayerInput::neutral()
        .with_left_stick(axis_to_i8(input.left_x), axis_to_i8(input.left_y))
        .with_c_stick(axis_to_i8(input.c_x), axis_to_i8(input.c_y))
        .with_left_trigger_analog(input.left_trigger)
        .with_right_trigger_analog(input.right_trigger)
        .with_left_trigger_digital(input.left_trigger_pressed)
        .with_right_trigger_digital(input.right_trigger_pressed)
        .with_attack(input.attack)
        .with_special(input.special)
        .with_jump_primary(input.jump_primary)
        .with_jump_secondary(input.jump_secondary)
        .with_shield(input.shield)
        .with_grab(input.grab)
        .with_start(input.start)
        .with_dpad_up(input.dpad_up)
        .with_dpad_down(input.dpad_down)
        .with_dpad_left(input.dpad_left)
        .with_dpad_right(input.dpad_right)
}

pub fn physical_input_from_gamecube_pad(pad: GameCubePadStatus) -> PhysicalInput {
    let (left_x, left_y) = pad.main_stick_i16();
    let (c_x, c_y) = pad.c_stick_i16();

    PhysicalInput {
        left_x,
        left_y,
        c_x,
        c_y,
        left_trigger: pad.left_trigger,
        right_trigger: pad.right_trigger,
        attack: pad.buttons.a(),
        special: pad.buttons.b(),
        jump_primary: pad.buttons.x(),
        jump_secondary: pad.buttons.y(),
        shield: false,
        grab: pad.buttons.z(),
        left_trigger_pressed: pad.buttons.l(),
        right_trigger_pressed: pad.buttons.r(),
        start: pad.buttons.start(),
        dpad_up: pad.buttons.dpad_up(),
        dpad_down: pad.buttons.dpad_down(),
        dpad_left: pad.buttons.dpad_left(),
        dpad_right: pad.buttons.dpad_right(),
    }
}

pub fn map_gamecube_pad_to_player_input(pad: GameCubePadStatus) -> PlayerInput {
    let (stick_x, stick_y) = pad.main_stick_i8();
    let (c_stick_x, c_stick_y) = pad.c_stick_i8();

    PlayerInput::neutral()
        .with_left_stick(stick_x, stick_y)
        .with_c_stick(c_stick_x, c_stick_y)
        .with_left_trigger_analog(pad.left_trigger)
        .with_right_trigger_analog(pad.right_trigger)
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

fn axis_to_i8(value: i16) -> i8 {
    let wide = value as i32;
    if wide.abs() < AXIS_DEADZONE as i32 {
        return 0;
    }

    let scaled = wide * 127 / 32_767;
    scaled.clamp(-127, 127) as i8
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderFrame {
    pub frame: Frame,
    pub player_positions: [Vec2; 2],
    pub player_facings: [i8; 2],
    pub player_motion_states: [MotionState; 2],
    pub player_state_frames: [u8; 2],
    pub player_animation_frames: [u8; 2],
    pub player_debug_input_facts: [MeleeInputFacts; 2],
    pub checksum: u64,
}

impl RenderFrame {
    pub fn from_world(world: &World) -> Self {
        Self::from_snapshot(world.snapshot())
    }

    pub fn from_snapshot(snapshot: WorldSnapshot) -> Self {
        Self {
            frame: snapshot.frame,
            player_positions: [snapshot.players[0].position, snapshot.players[1].position],
            player_facings: [snapshot.players[0].facing, snapshot.players[1].facing],
            player_motion_states: [
                snapshot.players[0].motion_state,
                snapshot.players[1].motion_state,
            ],
            player_state_frames: [
                snapshot.players[0].state_frame,
                snapshot.players[1].state_frame,
            ],
            player_animation_frames: [
                snapshot.players[0].animation_frame,
                snapshot.players[1].animation_frame,
            ],
            player_debug_input_facts: [
                snapshot.players[0].debug_input_facts,
                snapshot.players[1].debug_input_facts,
            ],
            checksum: snapshot.checksum,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl RenderColor {
    pub const BACKGROUND: Self = Self {
        r: 17,
        g: 19,
        b: 24,
        a: 255,
    };
    pub const STAGE: Self = Self {
        r: 180,
        g: 187,
        b: 196,
        a: 255,
    };
    pub const PLAYER_ONE: Self = Self {
        r: 74,
        g: 138,
        b: 255,
        a: 255,
    };
    pub const PLAYER_TWO: Self = Self {
        r: 255,
        g: 198,
        b: 87,
        a: 255,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub color: RenderColor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderScene {
    pub background: RenderColor,
    pub stage: RenderRect,
    pub players: [RenderRect; 2],
}

impl RenderScene {
    pub fn from_frame(frame: &RenderFrame, viewport_width: u32, viewport_height: u32) -> Self {
        let center_x = viewport_width as i32 / 2;
        let ground_y = viewport_height as i32 * 3 / 4;
        let player_colors = [RenderColor::PLAYER_ONE, RenderColor::PLAYER_TWO];

        Self {
            background: RenderColor::BACKGROUND,
            stage: RenderRect {
                x: viewport_width as i32 / 8,
                y: ground_y,
                width: viewport_width * 3 / 4,
                height: 8,
                color: RenderColor::STAGE,
            },
            players: [
                player_rect(frame, 0, center_x, ground_y, player_colors[0]),
                player_rect(frame, 1, center_x, ground_y, player_colors[1]),
            ],
        }
    }
}

fn player_rect(
    frame: &RenderFrame,
    index: usize,
    center_x: i32,
    ground_y: i32,
    color: RenderColor,
) -> RenderRect {
    let position = frame.player_positions[index];

    RenderRect {
        x: center_x + position.x / WORLD_TO_SCREEN_SCALE - PLAYER_RENDER_WIDTH as i32 / 2,
        y: ground_y - position.y / WORLD_TO_SCREEN_SCALE - PLAYER_RENDER_HEIGHT as i32,
        width: PLAYER_RENDER_WIDTH,
        height: PLAYER_RENDER_HEIGHT,
        color,
    }
}
