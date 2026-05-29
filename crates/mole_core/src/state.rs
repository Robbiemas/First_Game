use crate::{
    stage::{StageProfile, StageSurface, StageSurfaceKind},
    time::Frame,
    MeleeCommonData, MeleeInputFacts, MeleeInputSnapshot, MeleeInputTimers, MeleeJumpInput,
    PlayerInput,
};
use std::fmt;

pub const PLAYER_COUNT: usize = 2;
pub(crate) const EXPIRED_INPUT_TIMER: u8 = 0xfe;
const PLAYER_ONE_DEFAULT_SPAWN_X: i32 = -20_000;
const PLAYER_TWO_DEFAULT_SPAWN_X: i32 = 20_000;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Vec2 {
    pub x: i32,
    pub y: i32,
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
    pub attack_dash_total_frames: u8,
    pub attack_dash_iasa_frame: u8,
    pub guard_on_total_frames: u8,
    pub guard_off_total_frames: u8,
    pub escape_n_total_frames: u8,
    pub escape_f_total_frames: u8,
    pub escape_b_total_frames: u8,
    pub squat_total_frames: u8,
    pub squat_rv_total_frames: u8,
}

impl FighterActionFrames {
    pub const FALCON_LIKE: Self = Self {
        attack1_total_frames: 21,
        attack1_iasa_frame: 16,
        attack_dash_total_frames: 39,
        attack_dash_iasa_frame: 38,
        guard_on_total_frames: 4,
        guard_off_total_frames: 15,
        escape_n_total_frames: 23,
        escape_f_total_frames: 31,
        escape_b_total_frames: 31,
        squat_total_frames: 4,
        squat_rv_total_frames: 4,
    };

    pub const fn falcon_like() -> Self {
        Self::FALCON_LIKE
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FighterProfile {
    pub reference_character: &'static str,
    pub action_frames: FighterActionFrames,
    pub walk_target_speed_per_stick: i32,
    pub walk_initial_accel_per_stick: i32,
    pub walk_accel_per_tick: i32,
    pub walk_friction_per_tick: i32,
    pub walk_speed_per_tick: i32,
    pub run_speed_per_tick: i32,
    pub initial_dash_speed_per_tick: i32,
    pub dash_run_accel_stick_per_tick: i32,
    pub dash_run_accel_base_per_tick: i32,
    pub max_run_brake_frames: Option<u8>,
    pub traction_per_tick: i32,
    pub ground_max_horizontal_velocity_per_tick: i32,
    pub ground_to_air_jump_momentum_milli: i32,
    pub jump_horizontal_initial_velocity_per_tick: i32,
    pub jump_horizontal_max_velocity_per_tick: i32,
    pub air_jump_horizontal_velocity_per_tick: i32,
    pub max_jumps: u8,
    pub air_drift_stick_accel_per_tick: i32,
    pub air_drift_base_accel_per_tick: i32,
    pub air_drift_max_velocity_per_tick: i32,
    pub air_friction_per_tick: i32,
    pub air_max_horizontal_velocity_per_tick: i32,
    pub gravity_per_tick: i32,
    pub fall_speed_per_tick: i32,
    pub fast_fall_speed_per_tick: i32,
    pub full_hop_jump_force_per_tick: i32,
    pub short_hop_jump_force_per_tick: i32,
    pub air_jump_force_per_tick: i32,
    pub full_hop_height: i32,
    pub short_hop_height: i32,
    pub double_jump_height: i32,
    pub standing_height_units: i32,
    pub jumpsquat_frames: u8,
    pub dash_frames: u8,
    pub standing_turn_direction_change_frames: u8,
    pub standing_turn_total_frames: u8,
    pub normal_landing_lag_ticks: u8,
}

impl FighterProfile {
    pub const FALCON_LIKE: Self = Self {
        reference_character: "captain_falcon",
        action_frames: FighterActionFrames::FALCON_LIKE,
        walk_target_speed_per_stick: 6,
        walk_initial_accel_per_stick: 1,
        walk_accel_per_tick: 20,
        walk_friction_per_tick: 72,
        walk_speed_per_tick: 850,
        run_speed_per_tick: 2_300,
        initial_dash_speed_per_tick: 2_000,
        dash_run_accel_stick_per_tick: 10,
        dash_run_accel_base_per_tick: 150,
        max_run_brake_frames: None,
        traction_per_tick: 80,
        ground_max_horizontal_velocity_per_tick: 2_300,
        ground_to_air_jump_momentum_milli: 800,
        jump_horizontal_initial_velocity_per_tick: 400,
        jump_horizontal_max_velocity_per_tick: 1_000,
        air_jump_horizontal_velocity_per_tick: 400,
        max_jumps: 1,
        air_drift_stick_accel_per_tick: 40,
        air_drift_base_accel_per_tick: 20,
        air_drift_max_velocity_per_tick: 1_120,
        air_friction_per_tick: 10,
        air_max_horizontal_velocity_per_tick: 1_120,
        gravity_per_tick: 130,
        fall_speed_per_tick: 2_900,
        fast_fall_speed_per_tick: 3_500,
        full_hop_jump_force_per_tick: 3_100,
        short_hop_jump_force_per_tick: 1_900,
        air_jump_force_per_tick: 2_790,
        full_hop_height: 38_520,
        short_hop_height: 14_850,
        double_jump_height: 28_560,
        // Provisional visual scale: current 136 px standing sprite at the old 6 px/unit art calibration.
        standing_height_units: 22_667,
        jumpsquat_frames: 4,
        dash_frames: 15,
        standing_turn_direction_change_frames: 5,
        standing_turn_total_frames: 11,
        normal_landing_lag_ticks: 4,
    };

    pub const fn falcon_like() -> Self {
        Self::FALCON_LIKE
    }

    pub fn from_ftco_dat_attrs_bytes(
        reference_character: &'static str,
        bytes: &[u8],
    ) -> Result<Self, FighterProfileExtractError> {
        let mut profile = Self::FALCON_LIKE;
        profile.reference_character = reference_character;

        let jump_v_initial_velocity = read_profile_f32(bytes, 0x40, "jump_v_initial_velocity")?;
        let air_jump_v_multiplier = read_profile_f32(bytes, 0x50, "air_jump_v_multiplier")?;

        profile.walk_accel_per_tick = read_profile_milli_i32(bytes, 0x04, "walk_accel")?;
        profile.walk_speed_per_tick = read_profile_milli_i32(bytes, 0x08, "walk_max_vel")?;
        profile.traction_per_tick = read_profile_milli_i32(bytes, 0x18, "gr_friction")?;
        profile.initial_dash_speed_per_tick =
            read_profile_milli_i32(bytes, 0x1c, "dash_initial_velocity")?;
        profile.dash_run_accel_stick_per_tick =
            read_profile_milli_i32(bytes, 0x20, "dash_run_acceleration_a")?;
        profile.dash_run_accel_base_per_tick =
            read_profile_milli_i32(bytes, 0x24, "dash_run_acceleration_b")?;
        profile.run_speed_per_tick =
            read_profile_milli_i32(bytes, 0x28, "dash_run_terminal_velocity")?;
        profile.max_run_brake_frames = Some(read_profile_u8_from_f32(
            bytes,
            0x30,
            "max_run_brake_frames",
        )?);
        profile.ground_max_horizontal_velocity_per_tick =
            read_profile_milli_i32(bytes, 0x34, "ground_max_horizontal_velocity")?;
        profile.jumpsquat_frames = read_profile_u8_from_f32(bytes, 0x38, "jump_startup_time")?;
        profile.jump_horizontal_initial_velocity_per_tick =
            read_profile_milli_i32(bytes, 0x3c, "jump_h_initial_velocity")?;
        profile.full_hop_jump_force_per_tick = round_profile_f32_to_i32(
            jump_v_initial_velocity * 1000.0,
            "jump_v_initial_velocity",
            0x40,
        )?;
        profile.ground_to_air_jump_momentum_milli =
            read_profile_milli_i32(bytes, 0x44, "ground_to_air_jump_momentum_multiplier")?;
        profile.jump_horizontal_max_velocity_per_tick =
            read_profile_milli_i32(bytes, 0x48, "jump_h_max_velocity")?;
        profile.short_hop_jump_force_per_tick =
            read_profile_milli_i32(bytes, 0x4c, "hop_v_initial_velocity")?;
        profile.air_jump_force_per_tick = round_profile_f32_to_i32(
            jump_v_initial_velocity * air_jump_v_multiplier * 1000.0,
            "jump_v_initial_velocity*air_jump_v_multiplier",
            0x50,
        )?;
        profile.air_jump_horizontal_velocity_per_tick =
            read_profile_milli_i32(bytes, 0x54, "air_jump_h_multiplier")?;
        profile.max_jumps = read_profile_u8_from_i32(bytes, 0x58, "max_jumps")?;
        profile.gravity_per_tick = read_profile_milli_i32(bytes, 0x5c, "grav")?;
        profile.fall_speed_per_tick = read_profile_milli_i32(bytes, 0x60, "terminal_vel")?;
        profile.air_drift_stick_accel_per_tick =
            read_profile_milli_i32(bytes, 0x64, "air_drift_stick_mul")?;
        profile.air_drift_base_accel_per_tick =
            read_profile_milli_i32(bytes, 0x68, "aerial_drift_base")?;
        profile.air_drift_max_velocity_per_tick =
            read_profile_milli_i32(bytes, 0x6c, "air_drift_max")?;
        profile.air_friction_per_tick = read_profile_milli_i32(bytes, 0x70, "aerial_friction")?;
        profile.fast_fall_speed_per_tick =
            read_profile_milli_i32(bytes, 0x74, "fast_fall_velocity")?;
        profile.air_max_horizontal_velocity_per_tick =
            read_profile_milli_i32(bytes, 0x78, "air_max_horizontal_velocity")?;
        profile.standing_turn_direction_change_frames =
            read_profile_u8_from_f32(bytes, 0x84, "frames_to_change_direction_on_standing_turn")?;
        profile.normal_landing_lag_ticks =
            read_profile_u8_from_f32(bytes, 0xe4, "normal_landing_lag")?;

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

fn read_profile_milli_i32(
    bytes: &[u8],
    offset: usize,
    field: &'static str,
) -> Result<i32, FighterProfileExtractError> {
    round_profile_f32_to_i32(
        read_profile_f32(bytes, offset, field)? * 1000.0,
        field,
        offset,
    )
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
    WalkSlow,
    WalkMiddle,
    WalkFast,
    Dash,
    Run,
    RunBrake,
    TurnRun,
    Turn,
    Squat,
    SquatWait,
    SquatRv,
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
    Fall,
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
    Landing,
    Pass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerState {
    pub profile: FighterProfile,
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
    pub floor_skip_surface: Option<u8>,
}

impl PlayerState {
    pub const fn new(x: i32, y: i32, facing: i8) -> Self {
        Self::new_with_profile(x, y, facing, FighterProfile::FALCON_LIKE)
    }

    pub const fn new_with_profile(x: i32, y: i32, facing: i8, profile: FighterProfile) -> Self {
        Self {
            profile,
            position: Vec2 { x, y },
            velocity: Vec2 { x: 0, y: 0 },
            jumps_remaining: profile.max_jumps,
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
            floor_skip_surface: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerRenderSnapshot {
    pub position: Vec2,
    pub velocity: Vec2,
    pub facing: i8,
    pub motion_state: MotionState,
    pub state_frame: u8,
    pub animation_frame: u8,
    pub debug_input_facts: MeleeInputFacts,
}

impl PlayerRenderSnapshot {
    fn from_player(player: PlayerState, debug_input_facts: MeleeInputFacts) -> Self {
        Self {
            position: player.position,
            velocity: player.velocity,
            facing: player.facing,
            motion_state: player.motion_state,
            state_frame: player.motion_frame,
            animation_frame: player.attack_frame,
            debug_input_facts,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldSnapshot {
    pub frame: Frame,
    pub players: [PlayerRenderSnapshot; PLAYER_COUNT],
    pub checksum: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct World {
    frame: Frame,
    stage: StageProfile,
    common_data: MeleeCommonData,
    players: [PlayerState; PLAYER_COUNT],
    previous_inputs: [PlayerInput; PLAYER_COUNT],
    input_timers: [MeleeInputTimers; PLAYER_COUNT],
    last_input_facts: [MeleeInputFacts; PLAYER_COUNT],
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
        }
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
            players: [
                PlayerRenderSnapshot::from_player(self.players[0], self.last_input_facts[0]),
                PlayerRenderSnapshot::from_player(self.players[1], self.last_input_facts[1]),
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
            mix_u8(&mut hash, player.floor_skip_surface.unwrap_or(u8::MAX));
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
        MotionState::WalkSlow => 1,
        MotionState::WalkMiddle => 2,
        MotionState::WalkFast => 3,
        MotionState::Dash => 4,
        MotionState::Run => 5,
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
        MotionState::Air => 20,
        MotionState::Guard => 21,
        MotionState::EscapeAir => 22,
        MotionState::SpecialS => 23,
        MotionState::SpecialHi => 24,
        MotionState::SpecialLw => 25,
        MotionState::SpecialAirN => 26,
        MotionState::SpecialAirS => 27,
        MotionState::SpecialAirHi => 28,
        MotionState::SpecialAirLw => 29,
        MotionState::AttackAirN => 30,
        MotionState::AttackAirF => 31,
        MotionState::AttackAirB => 32,
        MotionState::AttackAirHi => 33,
        MotionState::AttackAirLw => 34,
        MotionState::EscapeN => 35,
        MotionState::EscapeF => 36,
        MotionState::EscapeB => 37,
        MotionState::GuardOff => 38,
        MotionState::LandingFallSpecial => 39,
        MotionState::FallSpecial => 40,
        MotionState::CatchDash => 41,
        MotionState::AttackDash => 42,
        MotionState::GuardOn => 43,
        MotionState::JumpAerialF => 44,
        MotionState::JumpAerialB => 45,
        MotionState::JumpF => 46,
        MotionState::JumpB => 47,
        MotionState::Landing => 48,
        MotionState::Pass => 49,
        MotionState::Fall => 50,
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

fn mix_str(hash: &mut u64, value: &str) {
    for byte in value.as_bytes() {
        mix_u8(hash, *byte);
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
}

fn mix_stage_surface(hash: &mut u64, surface: StageSurface) {
    mix_str(hash, surface.name);
    mix_u8(hash, stage_surface_kind_id(surface.kind));
    mix_i32(hash, surface.left_x);
    mix_i32(hash, surface.right_x);
    mix_i32(hash, surface.y);
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
    mix_u8(hash, common.main_stick_deadzone as u8);
    mix_u8(hash, common.c_stick_deadzone as u8);
    mix_u8(hash, common.trigger_deadzone);
    mix_u8(hash, common.z_shield_analog);
    mix_u8(hash, common.walk_x as u8);
    mix_u8(hash, common.walk_slow_x as u8);
    mix_u8(hash, common.walk_middle_x as u8);
    mix_u8(hash, common.walk_fast_x as u8);
    mix_u8(hash, common.dash_x as u8);
    mix_u8(hash, common.dash_tap_window);
    mix_u8(hash, common.turn_x as u8);
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
    mix_i32(hash, common.escapeair_force);
    mix_i32(hash, common.escapeair_decay_milli);
    mix_u8(hash, common.escapeair_landing_lag_ticks);
    mix_u8(hash, common.fallspecial_platform_landing_y as u8);
    mix_u8(hash, common.platform_pass_y as u8);
    mix_u8(hash, common.platform_pass_y_tap_window);
    mix_i32(hash, common.pass_initial_y_velocity);
    mix_u8(hash, common.platform_drop_delay_ticks);
    mix_u8(hash, common.dash_early_action_window);
    mix_u8(hash, common.dash_defensive_action_window);
    mix_u8(hash, common.dash_late_action_window);
    mix_u8(hash, common.run_x as u8);
    mix_u8(hash, common.guard_on_catch_dash_window);
    mix_u8(hash, common.run_turn_run_no_interrupt_frames);
}

fn mix_fighter_profile(hash: &mut u64, profile: FighterProfile) {
    mix_i32(hash, profile.walk_target_speed_per_stick);
    mix_i32(hash, profile.walk_initial_accel_per_stick);
    mix_fighter_action_frames(hash, profile.action_frames);
    mix_i32(hash, profile.walk_accel_per_tick);
    mix_i32(hash, profile.walk_friction_per_tick);
    mix_i32(hash, profile.walk_speed_per_tick);
    mix_i32(hash, profile.run_speed_per_tick);
    mix_i32(hash, profile.initial_dash_speed_per_tick);
    mix_i32(hash, profile.dash_run_accel_stick_per_tick);
    mix_i32(hash, profile.dash_run_accel_base_per_tick);
    match profile.max_run_brake_frames {
        Some(frames) => {
            mix_u8(hash, 1);
            mix_u8(hash, frames);
        }
        None => mix_u8(hash, 0),
    }
    mix_i32(hash, profile.traction_per_tick);
    mix_i32(hash, profile.ground_max_horizontal_velocity_per_tick);
    mix_i32(hash, profile.ground_to_air_jump_momentum_milli);
    mix_i32(hash, profile.jump_horizontal_initial_velocity_per_tick);
    mix_i32(hash, profile.jump_horizontal_max_velocity_per_tick);
    mix_i32(hash, profile.air_jump_horizontal_velocity_per_tick);
    mix_u8(hash, profile.max_jumps);
    mix_i32(hash, profile.air_drift_stick_accel_per_tick);
    mix_i32(hash, profile.air_drift_base_accel_per_tick);
    mix_i32(hash, profile.air_drift_max_velocity_per_tick);
    mix_i32(hash, profile.air_friction_per_tick);
    mix_i32(hash, profile.air_max_horizontal_velocity_per_tick);
    mix_i32(hash, profile.gravity_per_tick);
    mix_i32(hash, profile.fall_speed_per_tick);
    mix_i32(hash, profile.fast_fall_speed_per_tick);
    mix_i32(hash, profile.full_hop_jump_force_per_tick);
    mix_i32(hash, profile.short_hop_jump_force_per_tick);
    mix_i32(hash, profile.air_jump_force_per_tick);
    mix_i32(hash, profile.full_hop_height);
    mix_i32(hash, profile.short_hop_height);
    mix_i32(hash, profile.double_jump_height);
    mix_i32(hash, profile.standing_height_units);
    mix_u8(hash, profile.jumpsquat_frames);
    mix_u8(hash, profile.dash_frames);
    mix_u8(hash, profile.standing_turn_direction_change_frames);
    mix_u8(hash, profile.standing_turn_total_frames);
    mix_u8(hash, profile.normal_landing_lag_ticks);
}

fn mix_fighter_action_frames(hash: &mut u64, action_frames: FighterActionFrames) {
    mix_u8(hash, action_frames.attack1_total_frames);
    mix_u8(hash, action_frames.attack1_iasa_frame);
    mix_u8(hash, action_frames.attack_dash_total_frames);
    mix_u8(hash, action_frames.attack_dash_iasa_frame);
    mix_u8(hash, action_frames.guard_on_total_frames);
    mix_u8(hash, action_frames.guard_off_total_frames);
    mix_u8(hash, action_frames.escape_n_total_frames);
    mix_u8(hash, action_frames.escape_f_total_frames);
    mix_u8(hash, action_frames.escape_b_total_frames);
    mix_u8(hash, action_frames.squat_total_frames);
    mix_u8(hash, action_frames.squat_rv_total_frames);
}

fn mix_i32(hash: &mut u64, value: i32) {
    for byte in value.to_le_bytes() {
        mix_u8(hash, byte);
    }
}
