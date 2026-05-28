use crate::{time::Frame, MeleeInputSnapshot, MeleeInputTimers, MeleeJumpInput, PlayerInput};

pub const PLAYER_COUNT: usize = 2;
pub(crate) const EXPIRED_INPUT_TIMER: u8 = 0xfe;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Vec2 {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MotionState {
    #[default]
    Wait,
    Walk,
    Dash,
    Run,
    RunBrake,
    TurnRun,
    Turn,
    Squat,
    SpecialN,
    SpecialS,
    SpecialHi,
    SpecialLw,
    SpecialAirN,
    SpecialAirS,
    SpecialAirHi,
    SpecialAirLw,
    AttackAirN,
    AttackAirF,
    AttackAirB,
    AttackAirHi,
    AttackAirLw,
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
    Air,
    JumpAerialF,
    JumpAerialB,
    GuardOn,
    Guard,
    GuardOff,
    EscapeN,
    EscapeF,
    EscapeB,
    EscapeAir,
    FallSpecial,
    LandingFallSpecial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerState {
    pub position: Vec2,
    pub velocity: Vec2,
    pub jumps_remaining: u8,
    pub grounded: bool,
    pub fast_falling: bool,
    pub facing: i8,
    pub attack_frame: u8,
    pub motion_state: MotionState,
    pub motion_frame: u8,
    pub turn_facing_after: i8,
    pub turn_has_turned: bool,
    pub turn_just_turned: bool,
    pub turn_frames_to_turn: u8,
    pub turn_dash_after_direction: i8,
    pub turn_latched_buttons: u8,
    pub turn_run_accel_mul: i8,
    pub run_no_interrupt_frames: u8,
    pub shield_turn_facing_after: i8,
    pub shield_turn_frame: u8,
    pub guard_catch_dash_window: u8,
    pub jump_input: MeleeJumpInput,
    pub short_hop: bool,
    pub escape_air_iasa_timer: u8,
}

impl PlayerState {
    pub const fn new(x: i32, y: i32, facing: i8) -> Self {
        Self {
            position: Vec2 { x, y },
            velocity: Vec2 { x: 0, y: 0 },
            jumps_remaining: 1,
            grounded: true,
            fast_falling: false,
            facing,
            attack_frame: 0,
            motion_state: MotionState::Wait,
            motion_frame: 0,
            turn_facing_after: facing,
            turn_has_turned: false,
            turn_just_turned: false,
            turn_frames_to_turn: 0,
            turn_dash_after_direction: 0,
            turn_latched_buttons: 0,
            turn_run_accel_mul: facing,
            run_no_interrupt_frames: 0,
            shield_turn_facing_after: facing,
            shield_turn_frame: 0,
            guard_catch_dash_window: 0,
            jump_input: MeleeJumpInput::None,
            short_hop: false,
            escape_air_iasa_timer: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct World {
    frame: Frame,
    players: [PlayerState; PLAYER_COUNT],
    previous_inputs: [PlayerInput; PLAYER_COUNT],
    input_timers: [MeleeInputTimers; PLAYER_COUNT],
}

impl World {
    pub fn for_two_players() -> Self {
        Self {
            frame: Frame(0),
            players: [
                PlayerState::new(-1_000, 0, 1),
                PlayerState::new(1_000, 0, -1),
            ],
            previous_inputs: [PlayerInput::neutral(), PlayerInput::neutral()],
            input_timers: [MeleeInputTimers::expired(); PLAYER_COUNT],
        }
    }

    pub const fn frame(&self) -> Frame {
        self.frame
    }

    pub(crate) fn set_frame(&mut self, frame: Frame) {
        self.frame = frame;
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

    pub fn melee_input_snapshot(
        &self,
        player_index: usize,
        current_input: PlayerInput,
    ) -> Option<MeleeInputSnapshot> {
        let previous = *self.previous_inputs.get(player_index)?;
        let timers = *self.input_timers.get(player_index)?;

        Some(current_input.melee_snapshot(previous, timers))
    }

    pub fn checksum(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        mix_u32(&mut hash, self.frame.0);
        for player in self.players {
            mix_i32(&mut hash, player.position.x);
            mix_i32(&mut hash, player.position.y);
            mix_i32(&mut hash, player.velocity.x);
            mix_i32(&mut hash, player.velocity.y);
            mix_u8(&mut hash, player.jumps_remaining);
            mix_u8(&mut hash, player.grounded as u8);
            mix_u8(&mut hash, player.fast_falling as u8);
            mix_u8(&mut hash, player.facing as u8);
            mix_u8(&mut hash, player.attack_frame);
            mix_u8(&mut hash, motion_state_id(player.motion_state));
            mix_u8(&mut hash, player.motion_frame);
            mix_u8(&mut hash, player.turn_facing_after as u8);
            mix_u8(&mut hash, player.turn_has_turned as u8);
            mix_u8(&mut hash, player.turn_just_turned as u8);
            mix_u8(&mut hash, player.turn_frames_to_turn);
            mix_u8(&mut hash, player.turn_dash_after_direction as u8);
            mix_u8(&mut hash, player.turn_latched_buttons);
            mix_u8(&mut hash, player.turn_run_accel_mul as u8);
            mix_u8(&mut hash, player.run_no_interrupt_frames);
            mix_u8(&mut hash, player.shield_turn_facing_after as u8);
            mix_u8(&mut hash, player.shield_turn_frame);
            mix_u8(&mut hash, player.guard_catch_dash_window);
            mix_u8(&mut hash, jump_input_id(player.jump_input));
            mix_u8(&mut hash, player.short_hop as u8);
            mix_u8(&mut hash, player.escape_air_iasa_timer);
        }
        for input in self.previous_inputs {
            mix_u64(&mut hash, input.bits());
        }
        for timer in self.input_timers {
            mix_u8(&mut hash, timer.x_tap);
            mix_u8(&mut hash, timer.y_tap);
            mix_u8(&mut hash, timer.trigger);
        }
        hash
    }
}

const fn motion_state_id(state: MotionState) -> u8 {
    match state {
        MotionState::Wait => 0,
        MotionState::Walk => 1,
        MotionState::Dash => 2,
        MotionState::Run => 3,
        MotionState::RunBrake => 4,
        MotionState::TurnRun => 5,
        MotionState::Turn => 6,
        MotionState::Squat => 7,
        MotionState::SpecialN => 8,
        MotionState::Catch => 9,
        MotionState::Attack1 => 10,
        MotionState::AttackS3 => 11,
        MotionState::AttackHi3 => 12,
        MotionState::AttackLw3 => 13,
        MotionState::AttackS4 => 14,
        MotionState::AttackHi4 => 15,
        MotionState::AttackLw4 => 16,
        MotionState::KneeBend => 17,
        MotionState::Air => 18,
        MotionState::Guard => 19,
        MotionState::EscapeAir => 20,
        MotionState::SpecialS => 21,
        MotionState::SpecialHi => 22,
        MotionState::SpecialLw => 23,
        MotionState::SpecialAirN => 24,
        MotionState::SpecialAirS => 25,
        MotionState::SpecialAirHi => 26,
        MotionState::SpecialAirLw => 27,
        MotionState::AttackAirN => 28,
        MotionState::AttackAirF => 29,
        MotionState::AttackAirB => 30,
        MotionState::AttackAirHi => 31,
        MotionState::AttackAirLw => 32,
        MotionState::EscapeN => 33,
        MotionState::EscapeF => 34,
        MotionState::EscapeB => 35,
        MotionState::GuardOff => 36,
        MotionState::LandingFallSpecial => 37,
        MotionState::FallSpecial => 38,
        MotionState::CatchDash => 39,
        MotionState::AttackDash => 40,
        MotionState::GuardOn => 41,
        MotionState::JumpAerialF => 42,
        MotionState::JumpAerialB => 43,
        MotionState::JumpF => 44,
        MotionState::JumpB => 45,
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

fn mix_u64(hash: &mut u64, value: u64) {
    for byte in value.to_le_bytes() {
        mix_u8(hash, byte);
    }
}

fn mix_i32(hash: &mut u64, value: i32) {
    for byte in value.to_le_bytes() {
        mix_u8(hash, byte);
    }
}
