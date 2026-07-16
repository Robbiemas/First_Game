use crate::{
    collision::{
        source_damage_accumulator_after_stages, source_damage_result_for_victim, source_env_damage,
        source_grab_confirms, source_hit_confirms, EcbDiamond, SourceCollisionCapsule,
        SourceCollisionFrame, SourceDamageAccumulator, SourceDamageResult, SourceDamageResultInput,
        SourceDamageStage, SourceGrabConfirm, SourceHitConfirm, SourceHitboxAttributes,
        SourceHitboxLifecycleId, SourceInstalledThrowHitbox, SourceThrowHitboxAttributes,
        SOURCE_SHIELD_HURTBOX_ID,
    },
    fighter_stick_axis_to_f32,
    stage::{
        StageCollisionLineKind, StageCollisionProfile, StageProfile, StageRespawnPlatform,
        StageSurface, StageSurfaceKind,
    },
    time::Frame,
    units::{milli_to_source_units, source_units_to_milli, MELEE_UNIT_SCALE},
    MeleeCommonData, MeleeInputFacts, MeleeInputSnapshot, MeleeInputTimers, MeleeJumpInput,
    PlayerInput, WalkSpeedBucket,
};
use std::fmt;
use std::sync::Arc;

#[path = "generated/falcon_ecb.rs"]
mod falcon_ecb;

pub(crate) const SOURCE_AOBJ_REWINDED: u32 = 1 << 26;
pub(crate) const SOURCE_AOBJ_FIRST_PLAY: u32 = 1 << 27;
pub(crate) const SOURCE_AOBJ_NO_UPDATE: u32 = 1 << 28;
pub(crate) const SOURCE_AOBJ_LOOP: u32 = 1 << 29;
pub(crate) const SOURCE_AOBJ_NO_ANIM: u32 = 1 << 30;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SourceAObjDescriptor {
    pub(crate) end_frame: f32,
    pub(crate) rewind_frame: f32,
    pub(crate) flags: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SourceAObjState {
    pub(crate) flags: u32,
    pub(crate) curr_frame: f32,
    pub(crate) rewind_frame: f32,
    pub(crate) end_frame: f32,
    pub(crate) framerate: f32,
}

impl SourceAObjState {
    pub(crate) fn requested(
        curr_frame: f32,
        framerate: f32,
        rewind_frame: f32,
        end_frame: f32,
        descriptor_flags: u32,
    ) -> Self {
        Self {
            flags: (descriptor_flags & (SOURCE_AOBJ_LOOP | SOURCE_AOBJ_NO_UPDATE))
                | SOURCE_AOBJ_FIRST_PLAY,
            curr_frame,
            rewind_frame,
            end_frame,
            framerate,
        }
    }

    pub(crate) fn interpret_frame(&mut self) -> Option<f32> {
        if self.flags & SOURCE_AOBJ_NO_ANIM != 0 {
            return None;
        }

        if self.flags & SOURCE_AOBJ_FIRST_PLAY != 0 {
            self.flags &= !SOURCE_AOBJ_FIRST_PLAY;
        } else {
            self.curr_frame += self.framerate;
        }

        if self.flags & SOURCE_AOBJ_LOOP != 0 && self.curr_frame >= self.end_frame {
            if self.rewind_frame < self.end_frame {
                let period = self.end_frame - self.rewind_frame;
                self.curr_frame =
                    (self.curr_frame - self.rewind_frame) % period + self.rewind_frame;
            } else {
                self.curr_frame = self.end_frame;
            }
            self.flags |= SOURCE_AOBJ_REWINDED;
        } else {
            self.flags &= !SOURCE_AOBJ_REWINDED;
        }

        let evaluated_frame = self.curr_frame;
        if self.flags & SOURCE_AOBJ_LOOP == 0 && self.curr_frame >= self.end_frame {
            self.flags |= SOURCE_AOBJ_NO_ANIM;
        }
        Some(evaluated_frame)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SourceFighterPlayback {
    primary: SourceAObjState,
    secondary: Option<SourceAObjState>,
}

impl Default for SourceFighterPlayback {
    fn default() -> Self {
        Self {
            primary: SourceAObjState {
                flags: SOURCE_AOBJ_NO_ANIM,
                curr_frame: 0.0,
                rewind_frame: 0.0,
                end_frame: 0.0,
                framerate: 1.0,
            },
            secondary: None,
        }
    }
}

#[allow(dead_code)] // Task 3B consumes this migration API.
impl SourceFighterPlayback {
    pub(crate) fn install_primary_descriptor(
        &mut self,
        frame: f32,
        rate: f32,
        descriptor: SourceAObjDescriptor,
    ) {
        self.primary = SourceAObjState::requested(
            frame,
            rate,
            descriptor.rewind_frame,
            descriptor.end_frame,
            descriptor.flags,
        );
    }

    pub(crate) fn request_primary_frame(&mut self, frame: f32) {
        self.primary.curr_frame = frame;
        self.primary.flags = (self.primary.flags & !SOURCE_AOBJ_NO_ANIM) | SOURCE_AOBJ_FIRST_PLAY;
    }

    pub(crate) fn primary(&self) -> &SourceAObjState {
        &self.primary
    }

    pub(crate) fn secondary(&self) -> Option<&SourceAObjState> {
        self.secondary.as_ref()
    }

    pub(crate) fn set_secondary(&mut self, secondary: Option<SourceAObjState>) {
        self.secondary = secondary;
    }

    pub(crate) fn interpret_primary(&mut self) -> Option<f32> {
        self.primary.interpret_frame()
    }
}
#[path = "generated/fighter_common.rs"]
mod fighter_common;
#[path = "generated/source_root_motion.rs"]
mod source_root_motion;

pub const PLAYER_COUNT: usize = 2;
pub const DEFAULT_STOCK_COUNT: i8 = 4;
pub const PLAYER_STATE_NONE: u8 = 0;
pub const PLAYER_STATE_IN_GAME: u8 = 2;
pub const SOURCE_COLLISION_STATE_NORMAL: u8 = 0;
pub const SOURCE_COLLISION_STATE_HURT_INTANGIBLE: u8 = 1;
pub const SOURCE_COLLISION_STATE_HIT_AND_HURT_INTANGIBLE: u8 = 2;
pub(crate) const EXPIRED_INPUT_TIMER: u8 = 0xfe;
// Fallback for Melee's JObj-sourced ECB path while non-sampled actions are
// still being migrated from extracted per-frame data.
pub(crate) const SOURCE_JOBJ_ECB_BOTTOM_OFFSET_Y: i32 = 2_790;
const FALLBACK_ECB_WIDTH_UNITS: i32 = 4_000;
const PLAYER_ONE_DEFAULT_SPAWN_X: i32 = -20_000;
const PLAYER_TWO_DEFAULT_SPAWN_X: i32 = 20_000;
const HSD_RAND_INITIAL_SEED: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceDeathDirection {
    Down,
    Left,
    Right,
    Up,
}

impl SourceDeathDirection {
    pub const fn motion_state(self) -> MotionState {
        match self {
            Self::Down => MotionState::DeadDown,
            Self::Left => MotionState::DeadLeft,
            Self::Right => MotionState::DeadRight,
            Self::Up => MotionState::DeadUp,
        }
    }
}

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

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct SourceFighterEcb {
    pub(crate) top: SourceVec2,
    pub(crate) right: SourceVec2,
    pub(crate) bottom: SourceVec2,
    pub(crate) left: SourceVec2,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SourceCollEcbSnapshot {
    pub top: SourceVec2,
    pub right: SourceVec2,
    pub bottom: SourceVec2,
    pub left: SourceVec2,
}

impl From<SourceFighterEcb> for SourceCollEcbSnapshot {
    fn from(ecb: SourceFighterEcb) -> Self {
        Self {
            top: ecb.top,
            right: ecb.right,
            bottom: ecb.bottom,
            left: ecb.left,
        }
    }
}

pub(crate) const SOURCE_COLL_ECB_DEFAULT: SourceFighterEcb = SourceFighterEcb {
    top: SourceVec2 { x: 0.0, y: 8.0 },
    right: SourceVec2 { x: 4.0, y: 4.0 },
    bottom: SourceVec2 { x: 0.0, y: 0.0 },
    left: SourceVec2 { x: -4.0, y: 4.0 },
};

pub(crate) const SOURCE_COLL_ECB_ZERO: SourceFighterEcb = SourceFighterEcb {
    top: SourceVec2 { x: 0.0, y: 0.0 },
    right: SourceVec2 { x: 0.0, y: 0.0 },
    bottom: SourceVec2 { x: 0.0, y: 0.0 },
    left: SourceVec2 { x: 0.0, y: 0.0 },
};

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

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SourceVec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SourceBounds3 {
    pub min: SourceVec3,
    pub max: SourceVec3,
}

impl SourceBounds3 {
    pub const fn width_x(self) -> f32 {
        self.max.x - self.min.x
    }

    pub const fn height_y(self) -> f32 {
        self.max.y - self.min.y
    }

    pub const fn depth_z(self) -> f32 {
        self.max.z - self.min.z
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FighterCommonAccessoryProfile {
    pub symbol: &'static str,
    pub source_dat: &'static str,
    pub root_symbol: &'static str,
    pub pointer_table_slot: usize,
    pub joint_root_data_offset: u32,
    pub joint_count: usize,
    pub mesh_primitive_count: usize,
    pub mesh_vertex_emit_count: usize,
    pub mesh_unique_position_index_count: usize,
    pub mesh_bounds: SourceBounds3,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FighterCameraBox {
    pub x0: SourceVec3,
    pub xc: SourceVec3,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FighterEntryPlatformProfile {
    pub source_model_symbol: &'static str,
    pub decomp_ref: &'static str,
    pub vertical_offset_ratio: f32,
    pub accessory: FighterCommonAccessoryProfile,
}

impl FighterEntryPlatformProfile {
    pub const COMMON_TROPHY_PLATFORM: Self = Self {
        source_model_symbol: "Fighter_804D6514",
        decomp_ref: ".research/doldecomp-melee/src/melee/ft/ft_0C31.c::ftCo_800C6408",
        vertical_offset_ratio: 1.497345,
        accessory: fighter_common::COMMON_TROPHY_PLATFORM_ACCESSORY,
    };
}

impl FighterCameraBox {
    /// Captain Falcon `ftDataCaptain.x3C` -> `UnkFloat6_Camera`.
    /// Decomp consumers: `ftCamera_80076018` and `ftCamera_UpdateCameraBox`.
    pub const CAPTAIN_FALCON: Self = Self {
        x0: SourceVec3 {
            x: 10.0,
            y: 22.0,
            z: -9.0,
        },
        xc: SourceVec3 {
            x: 16.0,
            y: -9.0,
            z: 13.699999809265137,
        },
    };
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
    pub escape_f_throw_flags_b3_frame: u8,
    pub escape_b_throw_flags_b3_frame: u8,
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
        guard_off_total_frames: 16,
        escape_n_total_frames: 23,
        escape_f_total_frames: 31,
        escape_b_total_frames: 31,
        escape_f_throw_flags_b3_frame: 20,
        escape_b_throw_flags_b3_frame: 20,
        escape_air_skip_decay_frame: 30,
        turn_run_total_frames: 22,
        turn_run_cmd_var1_frame: 9,
        run_brake_total_frames: 28,
        run_brake_cmd_var0_set_frame: 0,
        run_brake_cmd_var0_clear_frame: 15,
        squat_total_frames: 4,
        squat_rv_total_frames: 10,
    };

    pub const fn falcon_like() -> Self {
        Self::FALCON_LIKE
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CaptainSpecialAttrs {
    pub specialn_stick_range_y_neg: f32,
    pub specialn_stick_range_y_pos: f32,
    pub specialn_angle_diff: f32,
    pub specialn_vel_x: f32,
    pub specialn_vel_mul: f32,
    pub specials_gr_vel_x: f32,
    pub specials_grav: f32,
    pub specials_terminal_vel: f32,
    pub specials_unk0: f32,
    pub specials_unk1: f32,
    pub specials_unk2: f32,
    pub specials_unk3: f32,
    pub specials_unk4: f32,
    pub specials_unk5: f32,
    pub specials_miss_landing_lag: f32,
    pub specials_hit_landing_lag: f32,
    pub specialhi_air_friction_mul: f32,
    pub specialhi_horz_vel: f32,
    pub specialhi_freefall_air_spd_mul: f32,
    pub specialhi_landing_lag: f32,
    pub specialhi_unk0: f32,
    pub specialhi_unk1: f32,
    pub specialhi_input_var: f32,
    pub specialhi_unk2: f32,
    pub specialhi_catch_grav: f32,
    pub specialhi_air_var: i32,
    pub x68: f32,
    pub speciallw_unk1: u32,
    pub speciallw_flame_particle_angle: f32,
    pub speciallw_on_hit_spd_modifier: f32,
    pub speciallw_unk2: i32,
    pub speciallw_ground_lag_mul: f32,
    pub speciallw_landing_lag_mul: f32,
    pub speciallw_ground_traction: f32,
    pub speciallw_air_landing_traction: f32,
}

impl CaptainSpecialAttrs {
    pub const FALCON: Self = Self {
        specialn_stick_range_y_neg: 0.125,
        specialn_stick_range_y_pos: 0.625,
        specialn_angle_diff: 30.0,
        specialn_vel_x: 1.9500000476837158,
        specialn_vel_mul: 0.9200000166893005,
        specials_gr_vel_x: 0.18000000715255737,
        specials_grav: 0.05000000074505806,
        specials_terminal_vel: 3.180000066757202,
        specials_unk0: 12.0,
        specials_unk1: 0.0,
        specials_unk2: 6.0,
        specials_unk3: 12.0,
        specials_unk4: -1.0,
        specials_unk5: 11.0,
        specials_miss_landing_lag: 20.0,
        specials_hit_landing_lag: 40.0,
        specialhi_air_friction_mul: 1.100000023841858,
        specialhi_horz_vel: 0.8500000238418579,
        specialhi_freefall_air_spd_mul: 0.7200000286102295,
        specialhi_landing_lag: 30.0,
        specialhi_unk0: 6.0,
        specialhi_unk1: 4.0,
        specialhi_input_var: 0.22499999403953552,
        specialhi_unk2: 15.0,
        specialhi_catch_grav: 0.30000001192092896,
        specialhi_air_var: 0,
        x68: f32::from_bits(0x00000002),
        speciallw_unk1: 4,
        speciallw_flame_particle_angle: 60.0,
        speciallw_on_hit_spd_modifier: 0.6000000238418579,
        speciallw_unk2: 4,
        speciallw_ground_lag_mul: 1.0,
        speciallw_landing_lag_mul: 1.0,
        speciallw_ground_traction: 1.600000023841858,
        speciallw_air_landing_traction: 3.0,
    };

    pub fn from_ftcaptain_dat_attrs_bytes(
        bytes: &[u8],
    ) -> Result<Self, FighterProfileExtractError> {
        Ok(Self {
            specialn_stick_range_y_neg: read_profile_f32(
                bytes,
                0x00,
                "specialn_stick_range_y_neg",
            )?,
            specialn_stick_range_y_pos: read_profile_f32(
                bytes,
                0x04,
                "specialn_stick_range_y_pos",
            )?,
            specialn_angle_diff: read_profile_f32(bytes, 0x08, "specialn_angle_diff")?,
            specialn_vel_x: read_profile_f32(bytes, 0x0c, "specialn_vel_x")?,
            specialn_vel_mul: read_profile_f32(bytes, 0x10, "specialn_vel_mul")?,
            specials_gr_vel_x: read_profile_f32(bytes, 0x14, "specials_gr_vel_x")?,
            specials_grav: read_profile_f32(bytes, 0x18, "specials_grav")?,
            specials_terminal_vel: read_profile_f32(bytes, 0x1c, "specials_terminal_vel")?,
            specials_unk0: read_profile_f32(bytes, 0x20, "specials_unk0")?,
            specials_unk1: read_profile_f32(bytes, 0x24, "specials_unk1")?,
            specials_unk2: read_profile_f32(bytes, 0x28, "specials_unk2")?,
            specials_unk3: read_profile_f32(bytes, 0x2c, "specials_unk3")?,
            specials_unk4: read_profile_f32(bytes, 0x30, "specials_unk4")?,
            specials_unk5: read_profile_f32(bytes, 0x34, "specials_unk5")?,
            specials_miss_landing_lag: read_profile_f32(bytes, 0x38, "specials_miss_landing_lag")?,
            specials_hit_landing_lag: read_profile_f32(bytes, 0x3c, "specials_hit_landing_lag")?,
            specialhi_air_friction_mul: read_profile_f32(
                bytes,
                0x40,
                "specialhi_air_friction_mul",
            )?,
            specialhi_horz_vel: read_profile_f32(bytes, 0x44, "specialhi_horz_vel")?,
            specialhi_freefall_air_spd_mul: read_profile_f32(
                bytes,
                0x48,
                "specialhi_freefall_air_spd_mul",
            )?,
            specialhi_landing_lag: read_profile_f32(bytes, 0x4c, "specialhi_landing_lag")?,
            specialhi_unk0: read_profile_f32(bytes, 0x50, "specialhi_unk0")?,
            specialhi_unk1: read_profile_f32(bytes, 0x54, "specialhi_unk1")?,
            specialhi_input_var: read_profile_f32(bytes, 0x58, "specialhi_input_var")?,
            specialhi_unk2: read_profile_f32(bytes, 0x5c, "specialhi_unk2")?,
            specialhi_catch_grav: read_profile_f32(bytes, 0x60, "specialhi_catch_grav")?,
            specialhi_air_var: read_profile_i32(bytes, 0x64, "specialhi_air_var")?,
            x68: read_profile_f32(bytes, 0x68, "x68")?,
            speciallw_unk1: read_profile_u32(bytes, 0x6c, "speciallw_unk1")?,
            speciallw_flame_particle_angle: read_profile_f32(
                bytes,
                0x70,
                "speciallw_flame_particle_angle",
            )?,
            speciallw_on_hit_spd_modifier: read_profile_f32(
                bytes,
                0x74,
                "speciallw_on_hit_spd_modifier",
            )?,
            speciallw_unk2: read_profile_i32(bytes, 0x78, "speciallw_unk2")?,
            speciallw_ground_lag_mul: read_profile_f32(bytes, 0x7c, "speciallw_ground_lag_mul")?,
            speciallw_landing_lag_mul: read_profile_f32(bytes, 0x80, "speciallw_landing_lag_mul")?,
            speciallw_ground_traction: read_profile_f32(bytes, 0x84, "speciallw_ground_traction")?,
            speciallw_air_landing_traction: read_profile_f32(
                bytes,
                0x88,
                "speciallw_air_landing_traction",
            )?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FighterProfile {
    pub reference_character: &'static str,
    pub action_frames: FighterActionFrames,
    pub captain_special_attrs: CaptainSpecialAttrs,
    pub camera_box: FighterCameraBox,
    pub source_create_x1a70: SourceVec3,
    pub model_scaling: f32,
    pub initial_shield_size: f32,
    pub shield_break_initial_velocity: f32,
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
    pub weight_independent_throws_mask: u8,
    pub gravity: f32,
    pub terminal_velocity: f32,
    pub fast_fall_velocity: f32,
    pub jump_vertical_initial_velocity: f32,
    pub hop_vertical_initial_velocity: f32,
    pub full_hop_height: i32,
    pub short_hop_height: i32,
    pub double_jump_height: i32,
    pub ledge_snap_x_milli: i32,
    pub ledge_snap_y_milli: i32,
    pub ledge_snap_height_milli: i32,
    pub ledge_jump_horizontal_velocity: f32,
    pub ledge_jump_vertical_velocity: f32,
    pub entry_platform: FighterEntryPlatformProfile,
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
        captain_special_attrs: CaptainSpecialAttrs::FALCON,
        camera_box: FighterCameraBox::CAPTAIN_FALCON,
        source_create_x1a70: SourceVec3 {
            x: 0.0,
            y: -13.84749698638916,
            z: 0.4883970022201538,
        },
        model_scaling: 0.9700000286102295,
        initial_shield_size: 15.0,
        shield_break_initial_velocity: 2.700000047683716,
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
        weight_independent_throws_mask: 0x07,
        gravity: 0.12999999523162842,
        terminal_velocity: 2.9000000953674316,
        fast_fall_velocity: 3.5,
        jump_vertical_initial_velocity: 3.0999999046325684,
        hop_vertical_initial_velocity: 1.899999976158142,
        full_hop_height: 38_520,
        short_hop_height: 14_850,
        double_jump_height: 28_560,
        ledge_snap_x_milli: 9_000,
        ledge_snap_y_milli: 17_000,
        ledge_snap_height_milli: 11_000,
        ledge_jump_horizontal_velocity: 1.0,
        ledge_jump_vertical_velocity: 3.299999952316284,
        entry_platform: FighterEntryPlatformProfile::COMMON_TROPHY_PLATFORM,
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

    pub const fn from_ftdata_x3c_camera_box(mut self, camera_box: FighterCameraBox) -> Self {
        self.camera_box = camera_box;
        self
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
        profile.weight_independent_throws_mask =
            read_profile_u8(bytes, 0x180, "weight_independent_throws_mask")?;
        profile.action_frames.rapid_jab_window =
            read_profile_u8_from_i32(bytes, 0x98, "rapid_jab_window")?;
        profile.standing_turn_direction_change_frames =
            read_profile_u8_from_f32(bytes, 0x84, "frames_to_change_direction_on_standing_turn")?;
        profile.model_scaling = read_profile_f32(bytes, 0x8c, "model_scaling")?;
        profile.initial_shield_size = read_profile_f32(bytes, 0x90, "initial_shield_size")?;
        profile.shield_break_initial_velocity =
            read_profile_f32(bytes, 0x94, "shield_break_initial_velocity")?;
        profile.ledge_jump_horizontal_velocity =
            read_profile_f32(bytes, 0xa8, "ledge_jump_horizontal_velocity")?;
        profile.ledge_jump_vertical_velocity =
            read_profile_f32(bytes, 0xac, "ledge_jump_vertical_velocity")?;
        let trophy_scale = read_profile_f32(bytes, 0x110, "trophy_scale")?;
        profile.entry_platform_offset_y = round_profile_f32_to_i32(
            trophy_scale * profile.entry_platform.vertical_offset_ratio * 1000.0,
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

fn read_profile_u8(
    bytes: &[u8],
    offset: usize,
    field: &'static str,
) -> Result<u8, FighterProfileExtractError> {
    Ok(read_profile_bytes::<1>(bytes, offset, field)?[0])
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

fn read_profile_i32(
    bytes: &[u8],
    offset: usize,
    field: &'static str,
) -> Result<i32, FighterProfileExtractError> {
    Ok(i32::from_be_bytes(read_profile_bytes(
        bytes, offset, field,
    )?))
}

fn read_profile_u32(
    bytes: &[u8],
    offset: usize,
    field: &'static str,
) -> Result<u32, FighterProfileExtractError> {
    Ok(u32::from_be_bytes(read_profile_bytes(
        bytes, offset, field,
    )?))
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
    DeadDown,
    DeadLeft,
    DeadRight,
    DeadUp,
    DeadUpStar,
    DeadUpStarIce,
    DeadUpFall,
    DeadUpFallHitCamera,
    DeadUpFallHitCameraFlat,
    DeadUpFallIce,
    DeadUpFallHitCameraIce,
    Sleep,
    Rebirth,
    RebirthWait,
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
    DamageFall,
    JumpAerialF,
    JumpAerialB,
    GuardOn,
    Guard,
    GuardOff,
    GuardSetOff,
    GuardReflect,
    ShieldBreakFly,
    ShieldBreakFall,
    ShieldBreakDownU,
    ShieldBreakDownD,
    ShieldBreakStandU,
    ShieldBreakStandD,
    Furafura,
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
    CliffCatch,
    CliffWait,
    CliffClimbSlow,
    CliffClimbQuick,
    CliffAttackSlow,
    CliffAttackQuick,
    CliffEscapeSlow,
    CliffEscapeQuick,
    CliffJumpSlow1,
    CliffJumpSlow2,
    CliffJumpQuick1,
    CliffJumpQuick2,
}

pub const RUST_MOTION_STATE_VARIANTS: &[&str] = &[
    "Wait",
    "DeadDown",
    "DeadLeft",
    "DeadRight",
    "DeadUp",
    "DeadUpStar",
    "DeadUpStarIce",
    "DeadUpFall",
    "DeadUpFallHitCamera",
    "DeadUpFallHitCameraFlat",
    "DeadUpFallIce",
    "DeadUpFallHitCameraIce",
    "Sleep",
    "Rebirth",
    "RebirthWait",
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
    "DamageFall",
    "JumpAerialF",
    "JumpAerialB",
    "GuardOn",
    "Guard",
    "GuardOff",
    "GuardSetOff",
    "GuardReflect",
    "ShieldBreakFly",
    "ShieldBreakFall",
    "ShieldBreakDownU",
    "ShieldBreakDownD",
    "ShieldBreakStandU",
    "ShieldBreakStandD",
    "Furafura",
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
    "CliffCatch",
    "CliffWait",
    "CliffClimbSlow",
    "CliffClimbQuick",
    "CliffAttackSlow",
    "CliffAttackQuick",
    "CliffEscapeSlow",
    "CliffEscapeQuick",
    "CliffJumpSlow1",
    "CliffJumpSlow2",
    "CliffJumpQuick1",
    "CliffJumpQuick2",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceStateCallback {
    pub phase: &'static str,
    pub function: &'static str,
    pub effect: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceStateTransition {
    pub to: MotionState,
    pub trigger: &'static str,
    pub function: &'static str,
    pub ordering: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceStateSequence {
    pub motion_state: MotionState,
    pub source_file: &'static str,
    pub callbacks: &'static [SourceStateCallback],
    pub transitions: &'static [SourceStateTransition],
}

pub const fn source_state_sequence_for_motion_state(
    motion_state: MotionState,
) -> Option<SourceStateSequence> {
    match motion_state {
        MotionState::Landing => Some(SOURCE_LANDING_SEQUENCE),
        MotionState::KneeBend => Some(SOURCE_KNEE_BEND_SEQUENCE),
        _ => None,
    }
}

const SOURCE_LANDING_SEQUENCE: SourceStateSequence = SourceStateSequence {
    motion_state: MotionState::Landing,
    source_file: ".research/doldecomp-melee/src/melee/ft/chara/ftCommon/ftCo_Landing.c",
    callbacks: &[
        SourceStateCallback {
            phase: "Anim",
            function: "ftCo_Landing_Anim",
            effect: "Wait exit through ft_8008A2BC when no animation frames remain",
        },
        SourceStateCallback {
            phase: "IASA",
            function: "ftCo_Landing_IASA",
            effect: "Post normal_landing_lag interrupt table when allow_interrupt is true",
        },
        SourceStateCallback {
            phase: "Phys",
            function: "ftCo_Landing_Phys / ft_80084F3C",
            effect: "Grounded friction before ground movement commit",
        },
        SourceStateCallback {
            phase: "Coll",
            function: "ftCo_Landing_Coll / ft_80084280",
            effect: "Ground collision validation and fall handoff on floor loss",
        },
    ],
    transitions: &[
        SourceStateTransition {
            to: MotionState::Wait,
            trigger: "landing animation completes",
            function: "ftCo_Landing_Anim / ft_8008A2BC",
            ordering: "Anim callback before IASA/Phys/Coll",
        },
        SourceStateTransition {
            to: MotionState::KneeBend,
            trigger: "post-lag jump input while landing allow_interrupt is true",
            function: "ftCo_Landing_IASA / ftCo_Jump_CheckInput",
            ordering: "IASA after Anim, before Landing_Phys",
        },
        SourceStateTransition {
            to: MotionState::Fall,
            trigger: "landing loses floor contact",
            function: "ftCo_Landing_Coll / ft_80084280",
            ordering: "Coll after Phys",
        },
    ],
};

const SOURCE_KNEE_BEND_SEQUENCE: SourceStateSequence = SourceStateSequence {
    motion_state: MotionState::KneeBend,
    source_file: ".research/doldecomp-melee/src/melee/ft/chara/ftCommon/ftCo_KneeBend.c",
    callbacks: &[
        SourceStateCallback {
            phase: "Anim",
            function: "ftCo_KneeBend_Anim",
            effect: "Jump takeoff through ftCo_Jump_Enter when jump_startup_time is reached",
        },
        SourceStateCallback {
            phase: "IASA",
            function: "ftCo_KneeBend_IASA",
            effect: "Attack100, catch, up-smash, then short-hop release checks",
        },
        SourceStateCallback {
            phase: "Phys",
            function: "ftCo_KneeBend_Phys / ft_80084F3C",
            effect: "Grounded friction while jump squat remains grounded",
        },
        SourceStateCallback {
            phase: "Coll",
            function: "ftCo_KneeBend_Coll / ft_80083F88",
            effect: "Ground collision maintenance during jump squat",
        },
    ],
    transitions: &[
        SourceStateTransition {
            to: MotionState::JumpF,
            trigger: "jumpsquat completes with forward-facing jump",
            function: "ftCo_KneeBend_Anim / ftCo_Jump_Enter",
            ordering: "Anim callback before IASA/Phys; no fresh KneeBend_Phys on takeoff tick",
        },
        SourceStateTransition {
            to: MotionState::JumpB,
            trigger: "jumpsquat completes with backward-facing jump",
            function: "ftCo_KneeBend_Anim / ftCo_Jump_Enter",
            ordering: "Anim callback before IASA/Phys; no fresh KneeBend_Phys on takeoff tick",
        },
    ],
};

pub fn motion_state_for_runtime_variant(state: &str) -> Option<MotionState> {
    Some(match state {
        "Wait" => MotionState::Wait,
        "DeadDown" => MotionState::DeadDown,
        "DeadLeft" => MotionState::DeadLeft,
        "DeadRight" => MotionState::DeadRight,
        "DeadUp" => MotionState::DeadUp,
        "DeadUpStar" => MotionState::DeadUpStar,
        "DeadUpStarIce" => MotionState::DeadUpStarIce,
        "DeadUpFall" => MotionState::DeadUpFall,
        "DeadUpFallHitCamera" => MotionState::DeadUpFallHitCamera,
        "DeadUpFallHitCameraFlat" => MotionState::DeadUpFallHitCameraFlat,
        "DeadUpFallIce" => MotionState::DeadUpFallIce,
        "DeadUpFallHitCameraIce" => MotionState::DeadUpFallHitCameraIce,
        "Sleep" => MotionState::Sleep,
        "Rebirth" => MotionState::Rebirth,
        "RebirthWait" => MotionState::RebirthWait,
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
        "DamageFall" => MotionState::DamageFall,
        "JumpAerialF" => MotionState::JumpAerialF,
        "JumpAerialB" => MotionState::JumpAerialB,
        "GuardOn" => MotionState::GuardOn,
        "Guard" => MotionState::Guard,
        "GuardOff" => MotionState::GuardOff,
        "GuardSetOff" => MotionState::GuardSetOff,
        "GuardReflect" => MotionState::GuardReflect,
        "ShieldBreakFly" => MotionState::ShieldBreakFly,
        "ShieldBreakFall" => MotionState::ShieldBreakFall,
        "ShieldBreakDownU" => MotionState::ShieldBreakDownU,
        "ShieldBreakDownD" => MotionState::ShieldBreakDownD,
        "ShieldBreakStandU" => MotionState::ShieldBreakStandU,
        "ShieldBreakStandD" => MotionState::ShieldBreakStandD,
        "Furafura" => MotionState::Furafura,
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
        "CliffCatch" => MotionState::CliffCatch,
        "CliffWait" => MotionState::CliffWait,
        "CliffClimbSlow" => MotionState::CliffClimbSlow,
        "CliffClimbQuick" => MotionState::CliffClimbQuick,
        "CliffAttackSlow" => MotionState::CliffAttackSlow,
        "CliffAttackQuick" => MotionState::CliffAttackQuick,
        "CliffEscapeSlow" => MotionState::CliffEscapeSlow,
        "CliffEscapeQuick" => MotionState::CliffEscapeQuick,
        "CliffJumpSlow1" => MotionState::CliffJumpSlow1,
        "CliffJumpSlow2" => MotionState::CliffJumpSlow2,
        "CliffJumpQuick1" => MotionState::CliffJumpQuick1,
        "CliffJumpQuick2" => MotionState::CliffJumpQuick2,
        _ => return None,
    })
}

pub fn runtime_motion_state_for_source_key(source_action_key: &str) -> Option<&'static str> {
    if matches!(
        source_action_key,
        "DeadDown"
            | "DeadLeft"
            | "DeadRight"
            | "DeadUp"
            | "DeadUpStar"
            | "DeadUpStarIce"
            | "DeadUpFall"
            | "DeadUpFallHitCamera"
            | "DeadUpFallHitCameraFlat"
            | "DeadUpFallIce"
            | "DeadUpFallHitCameraIce"
            | "Sleep"
            | "Rebirth"
            | "RebirthWait"
    ) {
        return None;
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MeleeMotionStateId(u16);

impl MeleeMotionStateId {
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceActionTableIndex(u16);

impl SourceActionTableIndex {
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

pub const fn source_move_id_for_action_state_id(action_state_id: MeleeActionStateId) -> u8 {
    match action_state_id.get() {
        44 => 2,
        45 => 3,
        46 => 4,
        47..=49 => 5,
        50 => 6,
        51..=53 => 7,
        56 => 8,
        57 => 9,
        58..=62 => 10,
        63 => 11,
        64 => 12,
        65 => 13,
        66 => 14,
        67 => 15,
        68 => 16,
        69 => 17,
        219 => 54,
        220 => 55,
        221 => 56,
        222 => 57,
        347 | 348 => 18,
        349..=352 => 19,
        353..=356 | 363 => 20,
        357..=362 => 21,
        _ => 1,
    }
}

pub const fn is_source_dead_motion_state(motion_state: MotionState) -> bool {
    matches!(
        motion_state,
        MotionState::DeadDown
            | MotionState::DeadLeft
            | MotionState::DeadRight
            | MotionState::DeadUp
            | MotionState::DeadUpStar
            | MotionState::DeadUpStarIce
            | MotionState::DeadUpFall
            | MotionState::DeadUpFallHitCamera
            | MotionState::DeadUpFallHitCameraFlat
            | MotionState::DeadUpFallIce
            | MotionState::DeadUpFallHitCameraIce
    )
}

pub const fn is_source_rebirth_motion_state(motion_state: MotionState) -> bool {
    matches!(
        motion_state,
        MotionState::Rebirth | MotionState::RebirthWait
    )
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceRetainedModelPose {
    pub action_state_id: Option<MeleeActionStateId>,
    pub source_action_key: SourceActionKey,
    pub motion_state: MotionState,
    pub frame_milli: i32,
    pub model_facing: i8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceDownBoundPose {
    pub hip_mtx_0_1: f32,
    pub hip_mtx_0_2: f32,
    pub hip_mtx_1_1: f32,
    pub hip_mtx_1_2: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SourcePosePoint {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl SourcePosePoint {
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SourceCapturePose {
    pub capture_anchor: SourcePosePoint,
    pub xrotn: SourcePosePoint,
    pub transn2: SourcePosePoint,
    pub x1a70: SourcePosePoint,
    pub thrown_hitbox: SourcePosePoint,
    pub thrown_hitbox_scale: f32,
}

pub const SOURCE_ACTION_SCRIPT_EVENT_CAPACITY: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceActionScriptEvent {
    None,
    SetCmdVar { cmd_var: u8, value: u32 },
    AllowInterrupt,
    SetAirborneState { state: u8 },
    SetCollisionState { state: u8 },
    SetJabCombo { disabled: bool },
    SetJabRapid { state: bool },
    SetThrowFlag { hit_idx: u32, flag_bit: Option<u8> },
    SetThrowHitbox(SourceThrowHitboxAttributes),
}

impl Default for SourceActionScriptEvent {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceActionScriptEvents {
    count: u8,
    events: [SourceActionScriptEvent; SOURCE_ACTION_SCRIPT_EVENT_CAPACITY],
}

impl SourceActionScriptEvents {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn single(event: SourceActionScriptEvent) -> Self {
        let mut events = Self::default();
        let _ = events.push(event);
        events
    }

    pub fn push(&mut self, event: SourceActionScriptEvent) -> bool {
        let index = usize::from(self.count);
        if index >= self.events.len() {
            return false;
        }
        self.events[index] = event;
        self.count = self.count.saturating_add(1);
        true
    }

    pub fn iter(&self) -> impl Iterator<Item = SourceActionScriptEvent> + '_ {
        self.events[..usize::from(self.count)].iter().copied()
    }
}

impl Default for SourceActionScriptEvents {
    fn default() -> Self {
        Self {
            count: 0,
            events: [SourceActionScriptEvent::None; SOURCE_ACTION_SCRIPT_EVENT_CAPACITY],
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SourceActionPoseMetadata {
    pub down_bound_pose: Option<SourceDownBoundPose>,
    pub capture_pose: Option<SourceCapturePose>,
    pub script_events: SourceActionScriptEvents,
    pub primary_hitbox: Option<SourceHitboxAttributes>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotionStateSourceBinding {
    pub runtime_motion_state: Option<MotionState>,
    pub melee_motion_state_id: Option<MeleeMotionStateId>,
    pub source_action_table_index: SourceActionTableIndex,
    pub source_action_key: SourceActionKey,
    // Compatibility aliases while source identity call sites migrate to typed fields.
    pub motion_state: MotionState,
    pub action_state_id: MeleeActionStateId,
    pub source_action_table_id: u16,
}

impl MotionStateSourceBinding {
    const fn new(
        motion_state: MotionState,
        source_action_table_id: u16,
        source_action_key: &'static str,
    ) -> Self {
        let action_state_id = melee_action_state_id_for_motion_state(motion_state);
        Self {
            runtime_motion_state: Some(motion_state),
            melee_motion_state_id: Some(melee_motion_state_id_for_motion_state(motion_state)),
            source_action_table_index: SourceActionTableIndex::new(source_action_table_id),
            source_action_key: SourceActionKey::new(source_action_key),
            motion_state,
            action_state_id,
            source_action_table_id,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceSpecialActionBinding {
    pub motion_state: Option<MotionState>,
    pub action_state_id: MeleeActionStateId,
    pub source_action_table_id: u16,
    pub source_action_key: SourceActionKey,
    pub total_frames: u8,
}

impl SourceSpecialActionBinding {
    const fn new(
        motion_state: Option<MotionState>,
        action_state_id: u16,
        source_action_table_id: u16,
        source_action_key: &'static str,
        total_frames: u8,
    ) -> Self {
        Self {
            motion_state,
            action_state_id: MeleeActionStateId::new(action_state_id),
            source_action_table_id,
            source_action_key: SourceActionKey::new(source_action_key),
            total_frames,
        }
    }
}

pub const FALCON_SOURCE_SPECIAL_ACTION_BINDINGS: &[SourceSpecialActionBinding] = &[
    // Runtime IDs are ftCa_MS_* values from ftCa_Init.c; source table IDs are
    // Captain Falcon action-animation table slots in PlCaAJ.dat.
    SourceSpecialActionBinding::new(Some(MotionState::SpecialN), 347, 301, "SpecialN", 100),
    SourceSpecialActionBinding::new(Some(MotionState::SpecialAirN), 348, 302, "SpecialAirN", 100),
    SourceSpecialActionBinding::new(
        Some(MotionState::SpecialSStart),
        349,
        303,
        "SpecialSStart",
        80,
    ),
    SourceSpecialActionBinding::new(Some(MotionState::SpecialS), 350, 304, "SpecialS", 25),
    SourceSpecialActionBinding::new(
        Some(MotionState::SpecialAirSStart),
        351,
        305,
        "SpecialAirSStart",
        80,
    ),
    SourceSpecialActionBinding::new(Some(MotionState::SpecialAirS), 352, 306, "SpecialAirS", 45),
    SourceSpecialActionBinding::new(Some(MotionState::SpecialHi), 353, 307, "SpecialHi", 65),
    SourceSpecialActionBinding::new(
        Some(MotionState::SpecialAirHi),
        354,
        308,
        "SpecialAirHi",
        65,
    ),
    SourceSpecialActionBinding::new(None, 355, 309, "SpecialHiCatch", 16),
    SourceSpecialActionBinding::new(None, 356, 310, "SpecialHiThrow", 60),
    SourceSpecialActionBinding::new(Some(MotionState::SpecialLw), 357, 311, "SpecialLw", 40),
    SourceSpecialActionBinding::new(None, 358, 312, "SpecialLwEnd", 30),
    SourceSpecialActionBinding::new(
        Some(MotionState::SpecialAirLw),
        359,
        313,
        "SpecialAirLw",
        30,
    ),
    SourceSpecialActionBinding::new(None, 360, 314, "SpecialAirLwEnd", 45),
    SourceSpecialActionBinding::new(None, 361, 316, "SpecialAirLwEndAir", 29),
    SourceSpecialActionBinding::new(None, 362, 315, "SpecialLwEndAir", 30),
    SourceSpecialActionBinding::new(None, 363, 317, "SpecialHiThrow", 60),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceCharacterSpecialActionBindings {
    pub reference_character_aliases: &'static [&'static str],
    pub bindings: &'static [SourceSpecialActionBinding],
}

impl SourceCharacterSpecialActionBindings {
    const fn new(
        reference_character_aliases: &'static [&'static str],
        bindings: &'static [SourceSpecialActionBinding],
    ) -> Self {
        Self {
            reference_character_aliases,
            bindings,
        }
    }
}

pub const FALCON_SOURCE_CHARACTER_ALIASES: &[&str] = &["captain", "captain_falcon", "falcon"];

pub const SOURCE_SPECIAL_ACTION_BINDINGS_BY_CHARACTER: &[SourceCharacterSpecialActionBindings] =
    &[SourceCharacterSpecialActionBindings::new(
        FALCON_SOURCE_CHARACTER_ALIASES,
        FALCON_SOURCE_SPECIAL_ACTION_BINDINGS,
    )];

pub fn source_special_action_bindings_for_character(
    source_character: Option<&str>,
) -> &'static [SourceSpecialActionBinding] {
    let Some(source_character) = source_character else {
        return &[];
    };
    SOURCE_SPECIAL_ACTION_BINDINGS_BY_CHARACTER
        .iter()
        .find(|entry| {
            entry
                .reference_character_aliases
                .contains(&source_character)
        })
        .map(|entry| entry.bindings)
        .unwrap_or(&[])
}

pub fn source_special_action_binding_for_motion_state(
    motion_state: MotionState,
) -> Option<SourceSpecialActionBinding> {
    FALCON_SOURCE_SPECIAL_ACTION_BINDINGS
        .iter()
        .copied()
        .find(|binding| binding.motion_state == Some(motion_state))
}

pub fn source_special_action_binding_for_runtime_id(
    action_state_id: MeleeActionStateId,
) -> Option<SourceSpecialActionBinding> {
    FALCON_SOURCE_SPECIAL_ACTION_BINDINGS
        .iter()
        .copied()
        .find(|binding| binding.action_state_id == action_state_id)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceSpecialCaptureTransition {
    pub reference_character: &'static str,
    pub active_action_state_id: MeleeActionStateId,
    pub attacker_action_state_id: MeleeActionStateId,
    pub attacker_source_action_key: SourceActionKey,
    pub victim_action_state_id: MeleeActionStateId,
    pub victim_source_action_key: SourceActionKey,
    pub constrain_airborne_victim_to_attacker_transn2: bool,
}

impl SourceSpecialCaptureTransition {
    const fn new(
        reference_character: &'static str,
        active_action_state_id: u16,
        attacker_action_state_id: u16,
        attacker_source_action_key: &'static str,
        victim_action_state_id: u16,
        victim_source_action_key: &'static str,
        constrain_airborne_victim_to_attacker_transn2: bool,
    ) -> Self {
        Self {
            reference_character,
            active_action_state_id: MeleeActionStateId::new(active_action_state_id),
            attacker_action_state_id: MeleeActionStateId::new(attacker_action_state_id),
            attacker_source_action_key: SourceActionKey::new(attacker_source_action_key),
            victim_action_state_id: MeleeActionStateId::new(victim_action_state_id),
            victim_source_action_key: SourceActionKey::new(victim_source_action_key),
            constrain_airborne_victim_to_attacker_transn2,
        }
    }
}

pub const SOURCE_SPECIAL_CAPTURE_TRANSITIONS: &[SourceSpecialCaptureTransition] = &[
    // ftCa_SpecialHi_Enter/ftCa_SpecialAirHi_Enter install
    // ftCa_SpecialLw_800E5128 and ftCo_8009CA0C through ftCommon_8007E2D0.
    SourceSpecialCaptureTransition::new(
        "captain_falcon",
        353,
        355,
        "SpecialHiCatch",
        275,
        "TCaptainSpecialHi",
        true,
    ),
    SourceSpecialCaptureTransition::new(
        "captain_falcon",
        354,
        355,
        "SpecialHiCatch",
        275,
        "TCaptainSpecialHi",
        true,
    ),
];

pub fn source_special_capture_transition_for_action_state_id(
    reference_character: &str,
    action_state_id: MeleeActionStateId,
) -> Option<SourceSpecialCaptureTransition> {
    SOURCE_SPECIAL_CAPTURE_TRANSITIONS
        .iter()
        .copied()
        .find(|transition| {
            transition.reference_character == reference_character
                && transition.active_action_state_id == action_state_id
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceSpecialHitDetectTransition {
    pub reference_character: &'static str,
    pub active_action_state_id: MeleeActionStateId,
    pub next_action_state_id: MeleeActionStateId,
    pub next_motion_state: MotionState,
    pub next_source_action_key: SourceActionKey,
    pub scale_ground_velocity_by_specials_gr_vel_x: bool,
    pub clear_self_velocity_y: bool,
}

impl SourceSpecialHitDetectTransition {
    const fn new(
        reference_character: &'static str,
        active_action_state_id: u16,
        next_action_state_id: u16,
        next_motion_state: MotionState,
        next_source_action_key: &'static str,
        scale_ground_velocity_by_specials_gr_vel_x: bool,
        clear_self_velocity_y: bool,
    ) -> Self {
        Self {
            reference_character,
            active_action_state_id: MeleeActionStateId::new(active_action_state_id),
            next_action_state_id: MeleeActionStateId::new(next_action_state_id),
            next_motion_state,
            next_source_action_key: SourceActionKey::new(next_source_action_key),
            scale_ground_velocity_by_specials_gr_vel_x,
            clear_self_velocity_y,
        }
    }
}

pub const SOURCE_SPECIAL_HIT_DETECT_TRANSITIONS: &[SourceSpecialHitDetectTransition] = &[
    // ftCa_SpecialS_OnDetect gates on fp->cmd_vars[0] and routes detected
    // fighter/item contact through onDetectGround/onDetectAir.
    SourceSpecialHitDetectTransition::new(
        "captain_falcon",
        349,
        350,
        MotionState::SpecialS,
        "SpecialS",
        true,
        true,
    ),
    SourceSpecialHitDetectTransition::new(
        "captain_falcon",
        351,
        352,
        MotionState::SpecialAirS,
        "SpecialAirS",
        false,
        false,
    ),
];

pub fn source_special_hit_detect_transition_for_action_state_id(
    reference_character: &str,
    action_state_id: MeleeActionStateId,
) -> Option<SourceSpecialHitDetectTransition> {
    SOURCE_SPECIAL_HIT_DETECT_TRANSITIONS
        .iter()
        .copied()
        .find(|transition| {
            transition.reference_character == reference_character
                && transition.active_action_state_id == action_state_id
        })
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
    CanonicalSourceActionBinding::new(188, 188, "DownFowardU"),
    CanonicalSourceActionBinding::new(189, 189, "DownBackU"),
    CanonicalSourceActionBinding::new(191, 289, "DownBoundD"),
    CanonicalSourceActionBinding::new(192, 192, "DownWaitD"),
    CanonicalSourceActionBinding::new(194, 291, "DownStandD"),
    CanonicalSourceActionBinding::new(195, 195, "DownAttackD"),
    CanonicalSourceActionBinding::new(196, 196, "DownFowardD"),
    CanonicalSourceActionBinding::new(197, 197, "DownBackD"),
    CanonicalSourceActionBinding::new(198, 198, "DownSpotD"),
    CanonicalSourceActionBinding::new(199, 199, "Passive"),
    CanonicalSourceActionBinding::new(200, 200, "PassiveStandF"),
    CanonicalSourceActionBinding::new(201, 201, "PassiveStandB"),
    CanonicalSourceActionBinding::new(213, 242, "Catch"),
    CanonicalSourceActionBinding::new(215, 243, "CatchDash"),
    CanonicalSourceActionBinding::new(216, 244, "CatchWait"),
    CanonicalSourceActionBinding::new(217, 245, "CatchAttack"),
    CanonicalSourceActionBinding::new(218, 246, "CatchCut"),
    CanonicalSourceActionBinding::new(219, 247, "ThrowF"),
    CanonicalSourceActionBinding::new(220, 248, "ThrowB"),
    CanonicalSourceActionBinding::new(221, 249, "ThrowHi"),
    CanonicalSourceActionBinding::new(222, 250, "ThrowLw"),
    CanonicalSourceActionBinding::new(223, 251, "CapturePulledHi"),
    CanonicalSourceActionBinding::new(224, 252, "CaptureWaitHi"),
    CanonicalSourceActionBinding::new(225, 253, "CaptureDamageHi"),
    CanonicalSourceActionBinding::new(226, 254, "CapturePulledLw"),
    CanonicalSourceActionBinding::new(227, 255, "CaptureWaitLw"),
    CanonicalSourceActionBinding::new(228, 256, "CaptureDamageLw"),
    CanonicalSourceActionBinding::new(229, 257, "CaptureCut"),
    CanonicalSourceActionBinding::new(230, 258, "CaptureJump"),
    CanonicalSourceActionBinding::new(239, 262, "TCaptainThrowF"),
    CanonicalSourceActionBinding::new(240, 263, "TCaptainThrowB"),
    CanonicalSourceActionBinding::new(241, 264, "TCaptainThrowHi"),
    CanonicalSourceActionBinding::new(242, 265, "TCaptainThrowLw"),
    CanonicalSourceActionBinding::new(275, 276, "TCaptainSpecialHi"),
];

pub fn canonical_source_action_binding_for_source_table_id(
    source_action_table_id: u16,
) -> Option<CanonicalSourceActionBinding> {
    CANONICAL_SOURCE_ONLY_ACTION_BINDINGS
        .iter()
        .copied()
        .find(|binding| binding.source_action_table_id == source_action_table_id)
}

pub fn canonical_source_action_binding_for_runtime_id(
    action_state_id: MeleeActionStateId,
) -> Option<CanonicalSourceActionBinding> {
    CANONICAL_SOURCE_ONLY_ACTION_BINDINGS
        .iter()
        .copied()
        .find(|binding| binding.action_state_id == action_state_id)
}

pub const fn melee_action_state_id_for_motion_state(
    motion_state: MotionState,
) -> MeleeActionStateId {
    MeleeActionStateId::new(match motion_state {
        MotionState::DeadDown => 0,
        MotionState::DeadLeft => 1,
        MotionState::DeadRight => 2,
        MotionState::DeadUp => 3,
        MotionState::DeadUpStar => 4,
        MotionState::DeadUpStarIce => 5,
        MotionState::DeadUpFall => 6,
        MotionState::DeadUpFallHitCamera => 7,
        MotionState::DeadUpFallHitCameraFlat => 8,
        MotionState::DeadUpFallIce => 9,
        MotionState::DeadUpFallHitCameraIce => 10,
        MotionState::Sleep => 11,
        MotionState::Rebirth => 12,
        MotionState::RebirthWait => 13,
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
        MotionState::DamageFall => 38,
        MotionState::JumpAerialF => 27,
        MotionState::JumpAerialB => 28,
        MotionState::GuardOn => 178,
        MotionState::Guard => 179,
        MotionState::GuardOff => 180,
        MotionState::GuardSetOff => 181,
        MotionState::GuardReflect => 182,
        MotionState::ShieldBreakFly => 205,
        MotionState::ShieldBreakFall => 206,
        MotionState::ShieldBreakDownU => 207,
        MotionState::ShieldBreakDownD => 208,
        MotionState::ShieldBreakStandU => 209,
        MotionState::ShieldBreakStandD => 210,
        MotionState::Furafura => 211,
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
        MotionState::CliffCatch => 252,
        MotionState::CliffWait => 253,
        MotionState::CliffClimbSlow => 254,
        MotionState::CliffClimbQuick => 255,
        MotionState::CliffAttackSlow => 256,
        MotionState::CliffAttackQuick => 257,
        MotionState::CliffEscapeSlow => 258,
        MotionState::CliffEscapeQuick => 259,
        MotionState::CliffJumpSlow1 => 260,
        MotionState::CliffJumpSlow2 => 261,
        MotionState::CliffJumpQuick1 => 262,
        MotionState::CliffJumpQuick2 => 263,
    })
}

pub const fn melee_motion_state_id_for_motion_state(
    motion_state: MotionState,
) -> MeleeMotionStateId {
    MeleeMotionStateId::new(melee_action_state_id_for_motion_state(motion_state).get())
}

pub const fn source_binding_for_motion_state(
    motion_state: MotionState,
) -> Option<MotionStateSourceBinding> {
    match motion_state {
        MotionState::DeadDown
        | MotionState::DeadLeft
        | MotionState::DeadRight
        | MotionState::DeadUp
        | MotionState::DeadUpStar
        | MotionState::DeadUpStarIce
        | MotionState::DeadUpFall
        | MotionState::DeadUpFallHitCamera
        | MotionState::DeadUpFallHitCameraFlat
        | MotionState::DeadUpFallIce
        | MotionState::DeadUpFallHitCameraIce
        | MotionState::Sleep
        | MotionState::Rebirth
        | MotionState::RebirthWait => None,
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
        MotionState::DamageFall => Some(MotionStateSourceBinding::new(
            MotionState::DamageFall,
            1,
            "DamageFall",
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
        MotionState::ShieldBreakFly
        | MotionState::ShieldBreakFall
        | MotionState::ShieldBreakDownU
        | MotionState::ShieldBreakDownD
        | MotionState::ShieldBreakStandU
        | MotionState::ShieldBreakStandD => None,
        MotionState::Furafura => Some(MotionStateSourceBinding::new(
            MotionState::Furafura,
            205,
            "FuraFura",
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
        MotionState::CliffCatch => Some(MotionStateSourceBinding::new(
            MotionState::CliffCatch,
            216,
            "CliffCatch",
        )),
        MotionState::CliffWait => Some(MotionStateSourceBinding::new(
            MotionState::CliffWait,
            217,
            "CliffWait1",
        )),
        MotionState::CliffClimbSlow => Some(MotionStateSourceBinding::new(
            MotionState::CliffClimbSlow,
            219,
            "CliffClimbSlow",
        )),
        MotionState::CliffClimbQuick => Some(MotionStateSourceBinding::new(
            MotionState::CliffClimbQuick,
            220,
            "CliffClimbQuick",
        )),
        MotionState::CliffAttackSlow => Some(MotionStateSourceBinding::new(
            MotionState::CliffAttackSlow,
            221,
            "CliffAttackSlow",
        )),
        MotionState::CliffAttackQuick => Some(MotionStateSourceBinding::new(
            MotionState::CliffAttackQuick,
            222,
            "CliffAttackQuick",
        )),
        MotionState::CliffEscapeSlow => Some(MotionStateSourceBinding::new(
            MotionState::CliffEscapeSlow,
            223,
            "CliffEscapeSlow",
        )),
        MotionState::CliffEscapeQuick => Some(MotionStateSourceBinding::new(
            MotionState::CliffEscapeQuick,
            224,
            "CliffEscapeQuick",
        )),
        MotionState::CliffJumpSlow1 => Some(MotionStateSourceBinding::new(
            MotionState::CliffJumpSlow1,
            225,
            "CliffJumpSlow1",
        )),
        MotionState::CliffJumpSlow2 => Some(MotionStateSourceBinding::new(
            MotionState::CliffJumpSlow2,
            226,
            "CliffJumpSlow2",
        )),
        MotionState::CliffJumpQuick1 => Some(MotionStateSourceBinding::new(
            MotionState::CliffJumpQuick1,
            227,
            "CliffJumpQuick1",
        )),
        MotionState::CliffJumpQuick2 => Some(MotionStateSourceBinding::new(
            MotionState::CliffJumpQuick2,
            228,
            "CliffJumpQuick2",
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

fn motion_state_keeps_previous_model_pose(motion_state: MotionState) -> bool {
    matches!(
        motion_state,
        MotionState::ShieldBreakFly
            | MotionState::ShieldBreakFall
            | MotionState::ShieldBreakDownU
            | MotionState::ShieldBreakDownD
            | MotionState::ShieldBreakStandU
            | MotionState::ShieldBreakStandD
    )
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerState {
    pub profile: FighterProfile,
    pub costume_index: u8,
    pub player_state: u8,
    pub stocks: i8,
    pub position: Vec2,
    pub source_position: SourceVec2,
    pub source_position_z: f32,
    pub source_previous_position: SourceVec2,
    pub source_previous_position_z: f32,
    pub(crate) source_coll_last_pos: SourceVec2,
    pub(crate) source_coll_cur_pos: SourceVec2,
    pub(crate) source_coll_prev_pos: SourceVec2,
    pub(crate) source_coll_x28_vec: SourceVec2,
    pub velocity: Vec2,
    pub source_self_velocity_x: f32,
    pub source_self_velocity_y: f32,
    pub source_knockback_velocity_x: f32,
    pub source_knockback_velocity_y: f32,
    pub source_ground_knockback_velocity: f32,
    pub source_attacker_shield_velocity_x: f32,
    pub player_nudge_x: f32,
    pub player_nudge_z: f32,
    pub ecb_bottom_offset_y: i32,
    pub ecb_bottom_lock_timer: u8,
    pub(crate) source_coll_ecb: SourceFighterEcb,
    pub(crate) source_coll_prev_ecb: SourceFighterEcb,
    pub(crate) source_coll_desired_ecb: SourceFighterEcb,
    pub(crate) source_coll_xe4_ecb: SourceFighterEcb,
    pub(crate) source_coll_x64_ecb: SourceFighterEcb,
    pub(crate) source_coll_facing_dir: i8,
    pub(crate) source_coll_x34_b5: bool,
    pub(crate) source_coll_x34_b6: bool,
    pub(crate) source_coll_x130_clear: bool,
    pub(crate) source_coll_x130_locked: bool,
    pub(crate) source_coll_floor_surface_index: Option<u8>,
    pub(crate) source_coll_floor_line_index: Option<u16>,
    pub(crate) source_coll_floor_skip_line_index: Option<u16>,
    pub(crate) source_coll_ledge_id_left: Option<u16>,
    pub(crate) source_coll_ledge_id_right: Option<u16>,
    pub(crate) source_coll_env_flags: u32,
    pub(crate) source_coll_prev_env_flags: u32,
    pub jumps_remaining: u8,
    pub grounded: bool,
    pub fast_falling: bool,
    pub facing: i8,
    pub(crate) source_model_facing: i8,
    pub attack_frame: u8,
    pub damage_percent: f32,
    pub damage_percent_temp: f32,
    pub damage_applied: u16,
    pub damage_knockback: f32,
    pub damage_angle: u16,
    pub damage_element: u8,
    pub hitlag_frames: u8,
    pub source_allow_sdi: bool,
    pub source_x2219_b5: bool,
    pub damage_hitstun_frames: u16,
    pub(crate) source_attack_id: u8,
    pub(crate) source_attack_instance: u16,
    pub(crate) source_stale_move_table: SourceStaleMoveTable,
    pub melee_action_state_id: Option<MeleeActionStateId>,
    pub source_action_key: Option<SourceActionKey>,
    pub source_action_total_frames: u8,
    pub(crate) source_retained_model_pose: Option<SourceRetainedModelPose>,
    pub source_down_bound_pose: Option<SourceDownBoundPose>,
    pub source_force_damage_down_bound: bool,
    pub source_down_bound_use_z_axis: bool,
    pub source_down_bound_reverse_face_up: bool,
    pub source_down_wait_timer: f32,
    pub source_lr_digital_press_timer: u8,
    pub source_lcancel_timer: u8,
    pub source_previous_lr_digital_press_timer: u8,
    pub source_jab_followup_timer: u8,
    pub source_jab_followup_queued: bool,
    pub source_jab_combo_enabled: bool,
    pub source_jab_rapid_enabled: bool,
    pub source_rapid_jab_input_count: u8,
    pub source_attack100_loop_has_started: bool,
    pub source_attack100_loop_continue_input: bool,
    pub source_common_timer: u8,
    pub source_dead_phase: u8,
    pub source_rebirth_target_x: f32,
    pub source_rebirth_target_y: f32,
    pub source_rebirth_platform_index: u16,
    pub source_rebirth_stage_point_index: u16,
    pub source_collision_state: u8,
    pub source_hurt_collision_state: u8,
    pub source_hurt_collision_lockout_timer: u16,
    pub source_hit_intangible_timer: u16,
    pub source_hurt_intangible_timer: u16,
    pub motion_state_alias: Option<MotionState>,
    pub motion_state: MotionState,
    pub source_motion_entry_facing: i8,
    pub motion_frame: u8,
    pub source_motion_anim_frame: f32,
    pub motion_anim_frame_milli: i32,
    pub(crate) source_playback: SourceFighterPlayback,
    pub(crate) source_fall_anim_blend: f32,
    pub(crate) source_fall_anim_pose: MotionState,
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
    pub source_allow_interrupt: bool,
    pub motion_cmd_var0: u32,
    pub motion_cmd_var1: u32,
    pub motion_throw_flags: u8,
    pub source_grab_timer: f32,
    pub source_grab_mash_x: i8,
    pub source_grab_mash_y: i8,
    pub source_capture_wait_timer: f32,
    pub source_capture_wait_anim_timer: f32,
    pub source_capture_wait_mashed: bool,
    pub source_capture_wait_jump_queued: bool,
    pub source_throw_x4: bool,
    pub source_throw_hitboxes: [Option<SourceInstalledThrowHitbox>; 2],
    pub source_thrown_unk_bool: bool,
    pub source_thrown_anim_timer: f32,
    pub source_thrown_hitbox_owner_index: Option<u8>,
    pub source_thrown_hitbox_team_unk: u8,
    pub source_thrown_hitbox_grabber_player_id: Option<u8>,
    pub source_victim_index: Option<u8>,
    pub source_x1a5c_index: Option<u8>,
    pub source_x221b_b5: bool,
    pub source_x2222_b3: bool,
    pub source_x2226_b2: bool,
    pub source_x1a70: SourceVec3,
    pub source_x34_scale_y: f32,
    pub captain_special_hi_x0: u16,
    pub captain_special_hi_vel_x: f32,
    pub captain_special_hi_vel_y: f32,
    pub captain_special_hi_x2_b0: bool,
    pub captain_special_hi_x2_b1: bool,
    pub landing_lag_ticks: u8,
    pub run_brake_x0: bool,
    pub run_brake_frames_remaining: u8,
    pub turn_run_x14: bool,
    pub turn_run_resume_advances: bool,
    pub turn_run_completion_pending: bool,
    pub turn_run_completion_enters_run: bool,
    pub motion_anim_rate_milli: i32,
    pub source_motion_anim_rate: f32,
    pub shield_turn_facing_after: i8,
    pub shield_turn_frame: u8,
    pub guard_catch_dash_window: u8,
    pub guard_release_latched: bool,
    pub shield_health: f32,
    pub lightshield_amount: f32,
    pub source_collision_lightshield_amount: f32,
    pub shield_release_lockout_frames: u8,
    pub source_shield_collision_active: bool,
    pub source_shield_hit_active: bool,
    pub source_shield_hit_update_pos: bool,
    pub source_shield_hit_position: SourcePosePoint,
    pub source_shield_aim_angle_degrees: f32,
    pub source_shield_aim_magnitude: f32,
    pub source_x221c_b1: bool,
    pub source_x221c_b2: bool,
    pub source_x221c_b3: bool,
    pub source_guard_reflect_timer: f32,
    pub source_guard_reflect_damage_skip_timer: f32,
    pub jump_input: MeleeJumpInput,
    pub short_hop: bool,
    pub escape_air_iasa_timer: u8,
    pub floor_skip_surface: Option<u8>,
    pub platform_pass_pending: bool,
    pub platform_pass_timer: u8,
    pub source_cliff_ledge_id: Option<u16>,
    pub source_cliff_stick_gate: bool,
    pub source_cliff_wait_timer: u16,
    pub source_ledge_cooldown_timer: u16,
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
            costume_index: 0,
            player_state: PLAYER_STATE_IN_GAME,
            stocks: DEFAULT_STOCK_COUNT,
            position: Vec2 { x, y },
            source_position: SourceVec2 {
                x: x as f32 / MELEE_UNIT_SCALE as f32,
                y: y as f32 / MELEE_UNIT_SCALE as f32,
            },
            source_position_z: 0.0,
            source_previous_position: SourceVec2 {
                x: x as f32 / MELEE_UNIT_SCALE as f32,
                y: y as f32 / MELEE_UNIT_SCALE as f32,
            },
            source_previous_position_z: 0.0,
            source_coll_last_pos: SourceVec2 {
                x: x as f32 / MELEE_UNIT_SCALE as f32,
                y: y as f32 / MELEE_UNIT_SCALE as f32,
            },
            source_coll_cur_pos: SourceVec2 {
                x: x as f32 / MELEE_UNIT_SCALE as f32,
                y: y as f32 / MELEE_UNIT_SCALE as f32,
            },
            source_coll_prev_pos: SourceVec2 {
                x: x as f32 / MELEE_UNIT_SCALE as f32,
                y: y as f32 / MELEE_UNIT_SCALE as f32,
            },
            source_coll_x28_vec: SourceVec2 {
                x: x as f32 / MELEE_UNIT_SCALE as f32,
                y: y as f32 / MELEE_UNIT_SCALE as f32,
            },
            velocity: Vec2 { x: 0, y: 0 },
            source_self_velocity_x: 0.0,
            source_self_velocity_y: 0.0,
            source_knockback_velocity_x: 0.0,
            source_knockback_velocity_y: 0.0,
            source_ground_knockback_velocity: 0.0,
            source_attacker_shield_velocity_x: 0.0,
            player_nudge_x: 0.0,
            player_nudge_z: 0.0,
            ecb_bottom_offset_y: SOURCE_JOBJ_ECB_BOTTOM_OFFSET_Y,
            ecb_bottom_lock_timer: 0,
            source_coll_ecb: SOURCE_COLL_ECB_DEFAULT,
            source_coll_prev_ecb: SOURCE_COLL_ECB_DEFAULT,
            source_coll_desired_ecb: SOURCE_COLL_ECB_DEFAULT,
            source_coll_xe4_ecb: SOURCE_COLL_ECB_DEFAULT,
            source_coll_x64_ecb: SOURCE_COLL_ECB_DEFAULT,
            source_coll_facing_dir: -1,
            source_coll_x34_b5: false,
            source_coll_x34_b6: false,
            source_coll_x130_clear: false,
            source_coll_x130_locked: false,
            source_coll_floor_surface_index: None,
            source_coll_floor_line_index: None,
            source_coll_floor_skip_line_index: None,
            source_coll_ledge_id_left: None,
            source_coll_ledge_id_right: None,
            source_coll_env_flags: 0,
            source_coll_prev_env_flags: 0,
            jumps_remaining: profile.reusable_air_jumps(),
            grounded: true,
            fast_falling: false,
            facing,
            source_model_facing: facing,
            attack_frame: 0,
            damage_percent: 0.0,
            damage_percent_temp: 0.0,
            damage_applied: 0,
            damage_knockback: 0.0,
            damage_angle: 0,
            damage_element: 0,
            hitlag_frames: 0,
            source_allow_sdi: false,
            source_x2219_b5: false,
            damage_hitstun_frames: 0,
            source_attack_id: 1,
            source_attack_instance: 0,
            source_stale_move_table: SourceStaleMoveTable::EMPTY,
            melee_action_state_id: Some(melee_action_state_id_for_motion_state(MotionState::Wait)),
            source_action_key: Some(SourceActionKey::new("Wait1")),
            source_action_total_frames: 0,
            source_retained_model_pose: None,
            source_down_bound_pose: None,
            source_force_damage_down_bound: false,
            source_down_bound_use_z_axis: false,
            source_down_bound_reverse_face_up: false,
            source_down_wait_timer: 0.0,
            source_lr_digital_press_timer: 0xff,
            source_lcancel_timer: 0xff,
            source_previous_lr_digital_press_timer: 0xff,
            source_jab_followup_timer: 0,
            source_jab_followup_queued: false,
            source_jab_combo_enabled: false,
            source_jab_rapid_enabled: false,
            source_rapid_jab_input_count: 0,
            source_attack100_loop_has_started: false,
            source_attack100_loop_continue_input: false,
            source_common_timer: 0,
            source_dead_phase: 0,
            source_rebirth_target_x: 0.0,
            source_rebirth_target_y: 0.0,
            source_rebirth_platform_index: 0,
            source_rebirth_stage_point_index: 0,
            source_collision_state: SOURCE_COLLISION_STATE_NORMAL,
            source_hurt_collision_state: 0,
            source_hurt_collision_lockout_timer: 0,
            source_hit_intangible_timer: 0,
            source_hurt_intangible_timer: 0,
            motion_state_alias: Some(MotionState::Wait),
            motion_state: MotionState::Wait,
            source_motion_entry_facing: facing,
            motion_frame: 0,
            source_motion_anim_frame: 0.0,
            motion_anim_frame_milli: 0,
            source_playback: SourceFighterPlayback {
                primary: SourceAObjState {
                    flags: SOURCE_AOBJ_NO_ANIM,
                    curr_frame: 0.0,
                    rewind_frame: 0.0,
                    end_frame: 0.0,
                    framerate: 1.0,
                },
                secondary: None,
            },
            source_fall_anim_blend: 0.0,
            source_fall_anim_pose: MotionState::Fall,
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
            source_allow_interrupt: false,
            motion_cmd_var0: 0,
            motion_cmd_var1: 0,
            motion_throw_flags: 0,
            source_grab_timer: 0.0,
            source_grab_mash_x: 0,
            source_grab_mash_y: 0,
            source_capture_wait_timer: 0.0,
            source_capture_wait_anim_timer: 0.0,
            source_capture_wait_mashed: false,
            source_capture_wait_jump_queued: false,
            source_throw_x4: false,
            source_throw_hitboxes: [None; 2],
            source_thrown_unk_bool: false,
            source_thrown_anim_timer: 0.0,
            source_thrown_hitbox_owner_index: None,
            source_thrown_hitbox_team_unk: 0,
            source_thrown_hitbox_grabber_player_id: None,
            source_victim_index: None,
            source_x1a5c_index: None,
            source_x221b_b5: false,
            source_x2222_b3: false,
            source_x2226_b2: false,
            source_x1a70: profile.source_create_x1a70,
            source_x34_scale_y: 1.0,
            captain_special_hi_x0: 0,
            captain_special_hi_vel_x: 0.0,
            captain_special_hi_vel_y: 0.0,
            captain_special_hi_x2_b0: false,
            captain_special_hi_x2_b1: false,
            landing_lag_ticks: 0,
            run_brake_x0: false,
            run_brake_frames_remaining: 0,
            turn_run_x14: false,
            turn_run_resume_advances: false,
            turn_run_completion_pending: false,
            turn_run_completion_enters_run: false,
            motion_anim_rate_milli: 1_000,
            source_motion_anim_rate: 1.0,
            shield_turn_facing_after: facing,
            shield_turn_frame: 0,
            guard_catch_dash_window: 0,
            guard_release_latched: false,
            shield_health: MeleeCommonData::PROVISIONAL.shield_start_health,
            lightshield_amount: 0.0,
            source_collision_lightshield_amount: 0.0,
            shield_release_lockout_frames: 0,
            source_shield_collision_active: false,
            source_shield_hit_active: false,
            source_shield_hit_update_pos: false,
            source_shield_hit_position: SourcePosePoint::ZERO,
            source_shield_aim_angle_degrees: 10.0,
            source_shield_aim_magnitude: 0.0,
            source_x221c_b1: false,
            source_x221c_b2: false,
            source_x221c_b3: false,
            source_guard_reflect_timer: 0.0,
            source_guard_reflect_damage_skip_timer: 0.0,
            jump_input: MeleeJumpInput::None,
            short_hop: false,
            escape_air_iasa_timer: 0,
            floor_skip_surface: None,
            platform_pass_pending: false,
            platform_pass_timer: 0,
            source_cliff_ledge_id: None,
            source_cliff_stick_gate: false,
            source_cliff_wait_timer: 0,
            source_ledge_cooldown_timer: 0,
            entry_base_y: y,
            entry_platform_offset_y: profile.entry_platform_offset_y,
            entry_timer: 0,
        }
    }

    pub fn set_source_motion_anim_frame(&mut self, frame: f32) {
        self.source_motion_anim_frame = if frame.is_finite() {
            frame.max(0.0)
        } else {
            0.0
        };
        self.motion_anim_frame_milli = (self.source_motion_anim_frame * 1000.0).round() as i32;
        self.source_playback.primary.curr_frame = self.source_motion_anim_frame;
    }

    pub fn set_source_motion_anim_frame_milli(&mut self, frame_milli: i32) {
        self.motion_anim_frame_milli = frame_milli.max(0);
        self.source_motion_anim_frame = self.motion_anim_frame_milli as f32 / 1000.0;
        self.source_playback.primary.curr_frame = self.source_motion_anim_frame;
    }

    pub fn cur_anim_frame(self) -> f32 {
        self.source_motion_anim_frame
    }

    pub fn cur_anim_frame_milli(self) -> i32 {
        self.motion_anim_frame_milli
    }

    pub fn frame_speed_mul(self) -> f32 {
        let source_rate_milli = (self.source_motion_anim_rate * 1_000.0).round() as i32;
        if source_rate_milli == self.motion_anim_rate_milli {
            self.source_motion_anim_rate
        } else {
            self.motion_anim_rate_milli as f32 / 1_000.0
        }
    }

    pub fn set_source_motion_anim_rate(&mut self, rate: f32) {
        self.source_motion_anim_rate = if rate.is_finite() { rate.max(0.0) } else { 0.0 };
        self.motion_anim_rate_milli = (self.source_motion_anim_rate * 1_000.0).round() as i32;
        self.source_playback.primary.framerate = self.source_motion_anim_rate;
    }

    pub fn set_source_motion_anim_rate_milli(&mut self, rate_milli: i32) {
        self.motion_anim_rate_milli = rate_milli.max(0);
        self.source_motion_anim_rate = self.motion_anim_rate_milli as f32 / 1_000.0;
        self.source_playback.primary.framerate = self.source_motion_anim_rate;
    }

    #[allow(dead_code)] // Task 3B migrates callback dispatch onto this authority.
    pub(crate) fn install_source_primary_anim(
        &mut self,
        frame: f32,
        rate: f32,
        descriptor: SourceAObjDescriptor,
    ) {
        self.source_playback
            .install_primary_descriptor(frame, rate, descriptor);
        self.sync_legacy_animation_fields_from_playback();
    }

    pub(crate) fn install_current_source_primary_anim(&mut self, frame: f32, rate: f32) -> bool {
        let Some(descriptor) = source_migrated_primary_anim_descriptor_for_player(self) else {
            return false;
        };
        self.install_source_primary_anim(frame, rate, descriptor);
        true
    }

    #[allow(dead_code)]
    pub(crate) fn interpret_source_primary_anim(&mut self) -> Option<f32> {
        let frame = self.source_playback.interpret_primary();
        self.sync_legacy_animation_fields_from_playback();
        frame
    }

    #[allow(dead_code)]
    pub(crate) fn source_playback(&self) -> &SourceFighterPlayback {
        &self.source_playback
    }

    pub(crate) fn source_primary_anim_has_frames_remaining(&self) -> bool {
        self.source_playback.primary.flags & SOURCE_AOBJ_NO_ANIM == 0
    }

    pub(crate) fn source_primary_anim_is_first_play(&self) -> bool {
        self.source_playback.primary.flags & SOURCE_AOBJ_FIRST_PLAY != 0
    }

    #[allow(dead_code)]
    pub(crate) fn set_source_secondary_anim(&mut self, secondary: Option<SourceAObjState>) {
        self.source_playback.set_secondary(secondary);
    }

    fn sync_legacy_animation_fields_from_playback(&mut self) {
        let primary = self.source_playback.primary;
        self.source_motion_anim_frame = primary.curr_frame;
        self.motion_anim_frame_milli = (primary.curr_frame * 1_000.0).round() as i32;
        self.source_motion_anim_rate = primary.framerate;
        self.motion_anim_rate_milli = (primary.framerate * 1_000.0).round() as i32;
    }

    pub fn frame_speed_mul_milli(self) -> i32 {
        self.motion_anim_rate_milli
    }

    pub fn clear_source_guard_shield_object(&mut self) {
        self.source_shield_collision_active = false;
        self.source_shield_hit_active = false;
        self.source_shield_hit_update_pos = false;
    }

    fn current_source_model_pose_for_retention(&self) -> Option<SourceRetainedModelPose> {
        if let Some(pose) = self.source_retained_model_pose {
            return Some(pose);
        }
        let motion_state = self.motion_state;
        let binding = source_binding_for_motion_state(motion_state);
        let source_action_key = self
            .source_action_key
            .or_else(|| binding.map(|binding| binding.source_action_key))?;
        let action_state_id = self
            .melee_action_state_id
            .or_else(|| Some(melee_action_state_id_for_motion_state(motion_state)));
        Some(SourceRetainedModelPose {
            action_state_id,
            source_action_key,
            motion_state,
            frame_milli: player_animation_pose_frame_milli(*self),
            model_facing: player_model_facing(self),
        })
    }

    pub fn set_source_floor_for_diagnostic(
        &mut self,
        surface_index: Option<u8>,
        line_index: Option<u16>,
    ) {
        self.source_coll_floor_surface_index = surface_index;
        self.source_coll_floor_line_index = line_index;
    }

    pub fn source_floor_for_diagnostic(self) -> (Option<u8>, Option<u16>) {
        (
            self.source_coll_floor_surface_index,
            self.source_coll_floor_line_index,
        )
    }

    pub fn source_collision_env_flags_for_diagnostic(self) -> u32 {
        self.source_coll_env_flags
    }

    pub fn set_source_ecb_bottom_lock_for_diagnostic(
        &mut self,
        timer: u8,
        bottom_x: f32,
        bottom_y: f32,
    ) {
        self.ecb_bottom_lock_timer = timer;
        self.source_coll_x130_locked = timer > 0;
        self.source_coll_ecb.bottom = SourceVec2 {
            x: bottom_x,
            y: bottom_y,
        };
        self.source_coll_prev_ecb.bottom = SourceVec2 {
            x: bottom_x,
            y: bottom_y,
        };
        self.source_coll_desired_ecb.bottom = SourceVec2 {
            x: bottom_x,
            y: bottom_y,
        };
        self.source_coll_xe4_ecb.bottom = SourceVec2 {
            x: bottom_x,
            y: bottom_y,
        };
        self.source_coll_x64_ecb.bottom = SourceVec2 {
            x: bottom_x,
            y: bottom_y,
        };
    }

    pub fn set_source_collision_ecb_from_world_for_diagnostic(&mut self, world_ecb: EcbDiamond) {
        let source_ecb = source_fighter_ecb_from_local_ecb(world_ecb_to_local(
            world_ecb,
            self.position,
            self.facing,
        ));
        self.source_coll_ecb = source_ecb;
        self.source_coll_prev_ecb = source_ecb;
        self.source_coll_desired_ecb = source_ecb;
        self.source_coll_xe4_ecb = source_ecb;
        self.source_coll_x64_ecb = source_ecb;
    }

    pub fn set_motion_state_alias(&mut self, motion_state: MotionState) {
        let retained_model_pose = if motion_state_keeps_previous_model_pose(motion_state) {
            self.current_source_model_pose_for_retention()
        } else {
            None
        };
        self.motion_state = motion_state;
        self.motion_state_alias = Some(motion_state);
        self.source_motion_entry_facing = self.facing;
        self.melee_action_state_id = Some(melee_action_state_id_for_motion_state(motion_state));
        self.damage_hitstun_frames = 0;
        self.source_allow_sdi = false;
        self.source_action_total_frames =
            source_special_action_binding_for_motion_state(motion_state)
                .map(|binding| binding.total_frames)
                .or_else(|| action_sample_frame_count_for_motion_state(motion_state))
                .unwrap_or(0);
        self.source_down_bound_pose = None;
        self.source_down_wait_timer = 0.0;
        self.source_common_timer = 0;
        self.source_dead_phase = 0;
        self.source_rebirth_target_x = 0.0;
        self.source_rebirth_target_y = 0.0;
        self.source_rebirth_platform_index = 0;
        self.source_rebirth_stage_point_index = 0;
        self.source_thrown_hitbox_owner_index = None;
        self.source_thrown_hitbox_team_unk = 0;
        self.source_thrown_hitbox_grabber_player_id = None;
        self.source_x2222_b3 = false;
        self.source_retained_model_pose = retained_model_pose;
        match source_binding_for_motion_state(motion_state) {
            Some(binding) => {
                self.source_action_key = Some(binding.source_action_key);
            }
            None => {
                self.source_action_key = None;
            }
        }
    }

    pub fn reset_for_entry_spawn(
        &mut self,
        x: i32,
        y: i32,
        facing: i8,
        profile: FighterProfile,
        shield_health: f32,
        entry_timer: u8,
    ) {
        let stocks = self.stocks;
        let costume_index = self.costume_index;
        *self = Self::new_with_profile(x, y, facing, profile);
        self.costume_index = costume_index;
        self.stocks = stocks;
        self.player_state = if stocks > 0 {
            PLAYER_STATE_IN_GAME
        } else {
            PLAYER_STATE_NONE
        };
        self.shield_health = shield_health;
        self.set_motion_state_alias(MotionState::Entry);
        self.entry_timer = entry_timer;
        self.grounded = false;
    }

    pub fn enter_source_dead_state(&mut self, direction: SourceDeathDirection) {
        self.enter_source_dead_motion_state(direction.motion_state());
    }

    pub fn enter_source_dead_motion_state(&mut self, motion_state: MotionState) {
        self.set_motion_state_alias(motion_state);
        self.motion_frame = 0;
        self.set_source_motion_anim_frame(0.0);
        self.source_common_timer = 0;
        self.source_dead_phase = 0;
        self.source_rebirth_target_x = 0.0;
        self.source_rebirth_target_y = 0.0;
        self.source_rebirth_platform_index = 0;
        self.source_rebirth_stage_point_index = 0;
        self.velocity = Vec2 { x: 0, y: 0 };
        self.source_self_velocity_x = 0.0;
        self.source_self_velocity_y = 0.0;
        self.source_knockback_velocity_x = 0.0;
        self.source_knockback_velocity_y = 0.0;
        self.source_ground_knockback_velocity = 0.0;
        self.ground_velocity_x = 0.0;
        self.ground_accel_x = 0.0;
        self.ground_accel_x2 = 0.0;
        self.hitlag_frames = 0;
        self.source_allow_sdi = false;
        self.source_x2219_b5 = false;
        self.damage_hitstun_frames = 0;
    }

    pub fn enter_source_dead_motion_state_with_common_data(
        &mut self,
        motion_state: MotionState,
        common_data: MeleeCommonData,
    ) {
        self.enter_source_dead_motion_state(motion_state);
        self.source_common_timer = match motion_state {
            MotionState::DeadDown
            | MotionState::DeadLeft
            | MotionState::DeadRight
            | MotionState::DeadUp => common_data.dead_wait_ticks,
            MotionState::DeadUpStar | MotionState::DeadUpStarIce => {
                common_data.dead_up_star_wait_ticks
            }
            MotionState::DeadUpFall
            | MotionState::DeadUpFallHitCamera
            | MotionState::DeadUpFallHitCameraFlat
            | MotionState::DeadUpFallIce
            | MotionState::DeadUpFallHitCameraIce => common_data.dead_up_fall_wait_ticks,
            _ => 0,
        };
    }

    pub fn enter_source_rebirth_state(
        &mut self,
        platform: StageRespawnPlatform,
        shield_health: f32,
        rebirth_ticks: u8,
    ) {
        let profile = self.profile;
        let stocks = self.stocks;
        let costume_index = self.costume_index;
        let start_x = platform.final_x + platform.offset_x;
        let start_y = platform.top_y;
        *self = Self::new_with_profile(start_x, start_y, platform.facing, profile);
        self.costume_index = costume_index;
        self.stocks = stocks;
        self.player_state = PLAYER_STATE_IN_GAME;
        self.shield_health = shield_health;
        self.set_motion_state_alias(MotionState::Rebirth);
        self.source_common_timer = rebirth_ticks;
        self.source_rebirth_target_x = milli_to_source_units(platform.final_x);
        self.source_rebirth_target_y = milli_to_source_units(platform.final_y + platform.offset_y);
        self.source_rebirth_platform_index = platform.platform_index;
        self.source_rebirth_stage_point_index = platform.stage_point_index;
        self.grounded = false;
    }

    pub fn enter_source_rebirth_wait_state(&mut self, rebirth_wait_ticks: u8) {
        self.set_motion_state_alias(MotionState::RebirthWait);
        self.source_common_timer = rebirth_wait_ticks;
        self.velocity = Vec2 { x: 0, y: 0 };
        self.source_self_velocity_x = 0.0;
        self.source_self_velocity_y = 0.0;
        self.source_knockback_velocity_x = 0.0;
        self.source_knockback_velocity_y = 0.0;
        self.source_ground_knockback_velocity = 0.0;
        self.grounded = false;
    }

    pub fn apply_source_hurt_intangible_timer(&mut self, ticks: u16) {
        if ticks > self.source_hurt_intangible_timer {
            self.source_hurt_intangible_timer = ticks;
        }
        self.source_collision_state = if self.source_hit_intangible_timer != 0 {
            SOURCE_COLLISION_STATE_HIT_AND_HURT_INTANGIBLE
        } else {
            SOURCE_COLLISION_STATE_HURT_INTANGIBLE
        };
    }

    pub fn apply_source_hurt_collision_lockout_timer(&mut self, ticks: u16) {
        if ticks > self.source_hurt_collision_lockout_timer {
            self.source_hurt_collision_lockout_timer = ticks;
        }
        self.source_hurt_collision_state = 1;
    }

    pub const fn source_allows_hurt_collision(self) -> bool {
        self.source_collision_state == SOURCE_COLLISION_STATE_NORMAL
    }
}

pub(crate) fn active_ecb_for_player(
    player: &PlayerState,
    common_data: MeleeCommonData,
) -> EcbDiamond {
    local_ecb_to_world(
        active_local_ecb_for_pose_frame(
            player,
            player_animation_pose_frame_value(*player),
            common_data,
        ),
        player.position,
        player_model_facing(player),
    )
}

#[allow(dead_code)]
pub(crate) fn active_ecb_for_motion_frame(
    player: &PlayerState,
    motion_frame: u8,
    common_data: MeleeCommonData,
) -> EcbDiamond {
    local_ecb_to_world(
        active_local_ecb_for_pose_frame_milli(player, i32::from(motion_frame) * 1_000, common_data),
        player.position,
        player_model_facing(player),
    )
}

pub(crate) fn active_ecb_bottom_offset_y(
    player: &PlayerState,
    motion_frame: u8,
    common_data: MeleeCommonData,
) -> i32 {
    active_local_ecb_for_pose_frame_milli(player, i32::from(motion_frame) * 1_000, common_data)
        .bottom
        .y
}

pub(crate) fn action_sample_frame_count_for_motion_state(motion_state: MotionState) -> Option<u8> {
    falcon_ecb::falcon_ecb_sample_count_for_motion_state(motion_state)
}

pub fn has_source_ecb_pose_data_for_motion_state(motion_state: MotionState) -> bool {
    falcon_ecb::falcon_has_source_ecb_jobj_for_motion_state(motion_state)
}

pub fn source_root_motion_delta(
    motion_state: MotionState,
    source_frame: u8,
) -> Option<crate::collision::Vec3> {
    source_root_motion::transn_offset(motion_state, source_frame)
}

pub fn source_root_motion_delta_for_action_key(
    source_action_key: SourceActionKey,
    source_frame: u8,
) -> Option<crate::collision::Vec3> {
    source_root_motion::transn_offset_for_action_key(source_action_key, source_frame)
}

pub fn source_root_motion_position(
    motion_state: MotionState,
    source_frame: u8,
) -> Option<crate::collision::Vec3> {
    source_root_motion::transn_position(motion_state, source_frame)
}

pub fn source_root_motion_position_for_action_key(
    source_action_key: SourceActionKey,
    source_frame: u8,
) -> Option<crate::collision::Vec3> {
    source_root_motion::transn_position_for_action_key(source_action_key, source_frame)
}

pub fn source_root_motion_frame_count(motion_state: MotionState) -> Option<u8> {
    source_root_motion::transn_frame_count(motion_state)
}

pub fn source_root_motion_frame_count_for_action_key(
    source_action_key: SourceActionKey,
) -> Option<u8> {
    source_root_motion::transn_frame_count_for_action_key(source_action_key)
}

#[allow(dead_code)]
pub(crate) fn active_source_local_ecb_for_player(
    player: &PlayerState,
    common_data: MeleeCommonData,
) -> SourceFighterEcb {
    active_source_local_ecb_for_pose_frame(
        player,
        player_animation_pose_frame_value(*player),
        common_data,
    )
}

#[allow(dead_code)]
pub(crate) fn live_source_local_ecb_for_player_pose_frame_milli(
    player: &PlayerState,
    pose_frame_milli: i32,
    common_data: MeleeCommonData,
) -> SourceFighterEcb {
    live_source_local_ecb_for_player_pose_frame_with_flags(
        player,
        pose_frame_from_milli(pose_frame_milli),
        common_data,
        6,
    )
}

#[allow(dead_code)]
pub(crate) fn live_source_local_ecb_for_player_pose_frame_milli_with_flags(
    player: &PlayerState,
    pose_frame_milli: i32,
    common_data: MeleeCommonData,
    flags: u32,
) -> SourceFighterEcb {
    live_source_local_ecb_for_player_pose_frame_with_flags(
        player,
        pose_frame_from_milli(pose_frame_milli),
        common_data,
        flags,
    )
}

pub(crate) fn live_source_local_ecb_for_player_pose_frame_with_flags(
    player: &PlayerState,
    pose_frame: f32,
    common_data: MeleeCommonData,
    flags: u32,
) -> SourceFighterEcb {
    source_local_ecb_for_pose_frame(
        player,
        pose_frame,
        common_data,
        SourcePoseSelection::ActiveDirectional,
        flags,
    )
}

fn active_local_ecb_for_pose_frame_milli(
    player: &PlayerState,
    pose_frame_milli: i32,
    common_data: MeleeCommonData,
) -> EcbDiamond {
    active_local_ecb_for_pose_frame(player, pose_frame_from_milli(pose_frame_milli), common_data)
}

fn active_local_ecb_for_pose_frame(
    player: &PlayerState,
    pose_frame: f32,
    common_data: MeleeCommonData,
) -> EcbDiamond {
    source_fighter_ecb_to_local_ecb(active_source_local_ecb_for_pose_frame(
        player,
        pose_frame,
        common_data,
    ))
}

#[allow(dead_code)]
fn active_source_local_ecb_for_pose_frame_milli(
    player: &PlayerState,
    pose_frame_milli: i32,
    common_data: MeleeCommonData,
) -> SourceFighterEcb {
    active_source_local_ecb_for_pose_frame(
        player,
        pose_frame_from_milli(pose_frame_milli),
        common_data,
    )
}

fn active_source_local_ecb_for_pose_frame(
    player: &PlayerState,
    pose_frame: f32,
    common_data: MeleeCommonData,
) -> SourceFighterEcb {
    source_local_ecb_for_pose_frame(
        player,
        pose_frame,
        common_data,
        SourcePoseSelection::ActiveDirectional,
        6,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourcePoseSelection {
    ActiveDirectional,
}

#[allow(dead_code)]
fn source_local_ecb_for_pose_frame_milli(
    player: &PlayerState,
    pose_frame_milli: i32,
    common_data: MeleeCommonData,
    selection: SourcePoseSelection,
    flags: u32,
) -> SourceFighterEcb {
    source_local_ecb_for_pose_frame(
        player,
        pose_frame_from_milli(pose_frame_milli),
        common_data,
        selection,
        flags,
    )
}

fn source_local_ecb_for_pose_frame(
    player: &PlayerState,
    pose_frame: f32,
    common_data: MeleeCommonData,
    selection: SourcePoseSelection,
    flags: u32,
) -> SourceFighterEcb {
    if player
        .melee_action_state_id
        .is_some_and(is_source_damage_action_state_id)
    {
        if let Some(ecb) = source_action_table_ecb_for_player(player, pose_frame, flags) {
            return ecb;
        }
    }

    let motion_state = match selection {
        SourcePoseSelection::ActiveDirectional => active_pose_motion_state(player, common_data),
    };
    let sample_frame = action_pose_sample_frame(
        player,
        motion_state,
        pose_frame,
        common_data,
        action_sample_frame_count_for_motion_state(motion_state)
            .map(usize::from)
            .unwrap_or(0),
    );
    let sample_frame = decomp_non_loop_pose_frame(
        sample_frame,
        action_sample_frame_count_for_motion_state(motion_state)
            .map(usize::from)
            .unwrap_or(0),
    );
    if let Some(ecb) =
        source_fall_blended_ecb(player, motion_state, sample_frame, flags).or_else(|| {
            falcon_ecb::falcon_source_ecb_jobj_for_motion_state_frame_with_flags_and_costume(
                motion_state,
                sample_frame,
                flags,
                player.costume_index,
            )
        })
    {
        return ecb;
    }

    if let Some(ecb) = source_action_table_ecb_for_player(player, pose_frame, flags) {
        return ecb;
    }

    source_fighter_ecb_from_local_ecb(fallback_local_ecb(player, fallback_bottom_offset_y(player)))
}

fn source_action_table_ecb_for_player(
    player: &PlayerState,
    pose_frame: f32,
    flags: u32,
) -> Option<SourceFighterEcb> {
    let source_action_table_id = source_action_table_id_for_player(*player)?;
    let sample_frame =
        decomp_non_loop_pose_frame(pose_frame, usize::from(player.source_action_total_frames));
    falcon_ecb::falcon_source_ecb_jobj_for_action_table_id_frame_with_flags_and_costume(
        source_action_table_id,
        sample_frame,
        flags,
        player.costume_index,
    )
}

fn source_fall_blended_ecb(
    player: &PlayerState,
    motion_state: MotionState,
    sample_frame: f32,
    flags: u32,
) -> Option<SourceFighterEcb> {
    let (neutral, directional) = fall_blend_pose_pair(motion_state, player.source_fall_anim_pose)?;
    let blend = player.source_fall_anim_blend.clamp(0.0, 1.0);
    if blend <= f32::EPSILON {
        return falcon_ecb::falcon_source_ecb_jobj_for_motion_state_frame_with_flags_and_costume(
            neutral,
            sample_frame,
            flags,
            player.costume_index,
        );
    }

    // ftCo_Fall_Anim_Inner evaluates a newly selected secondary skeleton once,
    // then ftCo_800CC988 evaluates it again before blending/copying its SRTs.
    // The live skeleton receives only the ordinary frame evaluation, so the
    // secondary FallF/FallB pose remains one animation evaluation ahead.
    let directional_sample_frame = sample_frame + 1.0;
    falcon_ecb::falcon_source_ecb_jobj_blend_for_motion_states_frames_with_flags_and_costume(
        neutral,
        directional,
        sample_frame,
        directional_sample_frame,
        blend,
        flags,
        player.costume_index,
    )
}

fn fall_blend_pose_pair(
    motion_state: MotionState,
    active_pose: MotionState,
) -> Option<(MotionState, MotionState)> {
    let (neutral, forward, backward) = match motion_state {
        MotionState::Fall | MotionState::FallF | MotionState::FallB => {
            (MotionState::Fall, MotionState::FallF, MotionState::FallB)
        }
        MotionState::FallAerial | MotionState::FallAerialF | MotionState::FallAerialB => (
            MotionState::FallAerial,
            MotionState::FallAerialF,
            MotionState::FallAerialB,
        ),
        MotionState::FallSpecial | MotionState::FallSpecialF | MotionState::FallSpecialB => (
            MotionState::FallSpecial,
            MotionState::FallSpecialF,
            MotionState::FallSpecialB,
        ),
        _ => return None,
    };
    let directional = match active_pose {
        pose if pose == forward || pose == backward => pose,
        _ => neutral,
    };
    Some((neutral, directional))
}

#[allow(dead_code)]
fn decomp_non_loop_pose_frame_milli(pose_frame_milli: i32, total_frames: usize) -> i32 {
    pose_frame_to_milli(decomp_non_loop_pose_frame(
        pose_frame_from_milli(pose_frame_milli),
        total_frames,
    ))
}

fn decomp_non_loop_pose_frame(pose_frame: f32, total_frames: usize) -> f32 {
    if total_frames == 0 {
        return finite_pose_frame(pose_frame);
    }
    let max_frame = total_frames as f32;
    finite_pose_frame(pose_frame).clamp(0.0, max_frame)
}

fn pose_frame_from_milli(pose_frame_milli: i32) -> f32 {
    pose_frame_milli.max(0) as f32 / 1_000.0
}

#[allow(dead_code)]
fn pose_frame_to_milli(pose_frame: f32) -> i32 {
    (finite_pose_frame(pose_frame) * 1_000.0).round() as i32
}

fn finite_pose_frame(pose_frame: f32) -> f32 {
    if pose_frame.is_finite() {
        pose_frame.max(0.0)
    } else {
        0.0
    }
}

fn source_action_table_id_for_player(player: PlayerState) -> Option<u16> {
    if player.motion_state_alias.is_some() {
        return None;
    }
    let action_state_id = player.melee_action_state_id?;
    if let Some(binding) = source_binding_for_motion_state(player.motion_state) {
        if binding.action_state_id == action_state_id {
            return Some(binding.source_action_table_id);
        }
    }
    if let Some(binding) = source_special_action_binding_for_runtime_id(action_state_id) {
        return Some(binding.source_action_table_id);
    }
    canonical_source_action_binding_for_runtime_id(action_state_id)
        .map(|binding| binding.source_action_table_id)
}

fn source_action_table_id_for_live_identity(player: PlayerState) -> Option<u16> {
    let action_state_id = player.melee_action_state_id?;
    let source_action_key = player.source_action_key?;
    if let Some(binding) = source_binding_for_motion_state(player.motion_state) {
        if binding.action_state_id == action_state_id
            && binding.source_action_key == source_action_key
        {
            return Some(binding.source_action_table_id);
        }
    }
    if let Some(binding) = source_special_action_binding_for_runtime_id(action_state_id) {
        if binding.source_action_key == source_action_key {
            return Some(binding.source_action_table_id);
        }
    }
    canonical_source_action_binding_for_runtime_id(action_state_id)
        .filter(|binding| binding.source_action_key == source_action_key)
        .map(|binding| binding.source_action_table_id)
}

fn source_migrated_primary_anim_descriptor_for_player(
    player: &PlayerState,
) -> Option<SourceAObjDescriptor> {
    if player
        .melee_action_state_id
        .is_some_and(|action_state_id| (75..=86).contains(&action_state_id.get()))
    {
        let source_action_table_id = source_action_table_id_for_live_identity(*player)?;
        return falcon_ecb::falcon_aobj_descriptor_for_action_table_id(source_action_table_id);
    }
    if let Some(source_action_key) = player.source_action_key {
        if matches!(
            source_action_key.as_str(),
            "Dash"
                | "Turn"
                | "AttackAirN"
                | "AttackAirF"
                | "AttackAirB"
                | "AttackAirHi"
                | "AttackAirLw"
                | "Attack100Start"
                | "Attack100Loop"
                | "Attack100End"
        ) {
            let source_action_table_id = source_action_table_id_for_live_identity(*player)?;
            return falcon_ecb::falcon_aobj_descriptor_for_action_table_id(source_action_table_id);
        }
    }
    None
}

fn source_aobj_descriptor_for_motion_state(
    motion_state: MotionState,
) -> Option<SourceAObjDescriptor> {
    let source_action_table_id =
        source_binding_for_motion_state(motion_state)?.source_action_table_id;
    falcon_ecb::falcon_aobj_descriptor_for_action_table_id(source_action_table_id)
}

#[allow(dead_code)]
fn action_pose_sample_frame_milli(
    player: &PlayerState,
    motion_state: MotionState,
    pose_frame_milli: i32,
    _common_data: MeleeCommonData,
    sample_count: usize,
) -> i32 {
    pose_frame_to_milli(action_pose_sample_frame(
        player,
        motion_state,
        pose_frame_from_milli(pose_frame_milli),
        _common_data,
        sample_count,
    ))
}

fn action_pose_sample_frame(
    player: &PlayerState,
    motion_state: MotionState,
    pose_frame: f32,
    _common_data: MeleeCommonData,
    sample_count: usize,
) -> f32 {
    let pose_frame = match motion_state {
        MotionState::LandingFallSpecial => pose_frame,
        MotionState::LandingAirN
        | MotionState::LandingAirF
        | MotionState::LandingAirB
        | MotionState::LandingAirHi
        | MotionState::LandingAirLw => {
            landing_air_pose_sample_frame(player, motion_state, pose_frame, sample_count)
        }
        _ => pose_frame,
    };
    let Some(descriptor) = source_aobj_descriptor_for_motion_state(motion_state) else {
        return pose_frame;
    };
    let mut aobj = SourceAObjState::requested(
        pose_frame,
        0.0,
        descriptor.rewind_frame,
        descriptor.end_frame,
        descriptor.flags,
    );
    aobj.interpret_frame().unwrap_or(pose_frame)
}

#[allow(dead_code)]
fn landing_air_pose_sample_frame_milli(
    player: &PlayerState,
    motion_state: MotionState,
    pose_frame_milli: i32,
    sample_count: usize,
) -> i32 {
    pose_frame_to_milli(landing_air_pose_sample_frame(
        player,
        motion_state,
        pose_frame_from_milli(pose_frame_milli),
        sample_count,
    ))
}

fn landing_air_pose_sample_frame(
    player: &PlayerState,
    motion_state: MotionState,
    pose_frame: f32,
    sample_count: usize,
) -> f32 {
    let landing_lag = if player.landing_lag_ticks == 0 {
        landing_air_profile_lag_ticks(motion_state, player.profile)
    } else {
        player.landing_lag_ticks
    };
    scaled_landing_pose_sample_frame_impl(pose_frame, landing_lag, sample_count)
}

pub fn landing_fall_special_lag_ticks(player: &PlayerState, common_data: MeleeCommonData) -> u8 {
    if player.landing_lag_ticks == 0 {
        common_data.escapeair_landing_lag_ticks
    } else {
        player.landing_lag_ticks
    }
}

#[allow(dead_code)]
fn scaled_landing_pose_sample_frame_milli_impl(
    pose_frame_milli: i32,
    landing_lag: u8,
    sample_count: usize,
) -> i32 {
    pose_frame_to_milli(scaled_landing_pose_sample_frame_impl(
        pose_frame_from_milli(pose_frame_milli),
        landing_lag,
        sample_count,
    ))
}

fn scaled_landing_pose_sample_frame_impl(
    pose_frame: f32,
    landing_lag: u8,
    sample_count: usize,
) -> f32 {
    if landing_lag == 0 || sample_count == 0 {
        return finite_pose_frame(pose_frame);
    }

    // ftCo_LandingAir_EnterWithMsidLag and ftCo_LandingFallSpecial_Enter
    // both scale the landing figatree by (source_frames + 0.1) / landing_lag.
    let action_frames = sample_count as f32 + 0.1;
    let scaled = finite_pose_frame(pose_frame) * action_frames / f32::from(landing_lag);
    let max_frame = sample_count.saturating_sub(1) as f32;
    scaled.clamp(0.0, max_frame)
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

    let self_vel_x = if player.source_self_velocity_x != 0.0 || player.velocity.x == 0 {
        player.source_self_velocity_x
    } else {
        milli_to_source_units(player.velocity.x)
    };
    let drift_ratio = self_vel_x.abs() / player.profile.air_drift_max.abs();
    if drift_ratio <= common_data.fall_animation_drift_threshold {
        return player.motion_state;
    }

    if self_vel_x * f32::from(player.facing) > 0.0 {
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

fn source_fighter_ecb_to_local_ecb(source: SourceFighterEcb) -> EcbDiamond {
    EcbDiamond {
        top: source_vec2_to_local_vec2(source.top),
        right: source_vec2_to_local_vec2(source.right),
        bottom: source_vec2_to_local_vec2(source.bottom),
        left: source_vec2_to_local_vec2(source.left),
    }
}

fn source_fighter_ecb_from_local_ecb(local: EcbDiamond) -> SourceFighterEcb {
    SourceFighterEcb {
        top: local_vec2_to_source_vec2(local.top),
        right: local_vec2_to_source_vec2(local.right),
        bottom: local_vec2_to_source_vec2(local.bottom),
        left: local_vec2_to_source_vec2(local.left),
    }
}

fn source_vec2_to_local_vec2(source: SourceVec2) -> Vec2 {
    Vec2 {
        x: source_units_to_milli(source.x),
        y: source_units_to_milli(source.y),
    }
}

fn local_vec2_to_source_vec2(local: Vec2) -> SourceVec2 {
    SourceVec2 {
        x: milli_to_source_units(local.x),
        y: milli_to_source_units(local.y),
    }
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

fn world_ecb_to_local(world: EcbDiamond, root_position: Vec2, facing: i8) -> EcbDiamond {
    let (right, left) = if facing < 0 {
        (world.left, world.right)
    } else {
        (world.right, world.left)
    };
    EcbDiamond {
        top: world_point_to_local(world.top, root_position, facing),
        right: world_point_to_local(right, root_position, facing),
        bottom: world_point_to_local(world.bottom, root_position, facing),
        left: world_point_to_local(left, root_position, facing),
    }
}

fn local_point_to_world(local: Vec2, root_position: Vec2, facing: i8) -> Vec2 {
    let facing_sign = if facing < 0 { -1 } else { 1 };
    Vec2 {
        x: root_position.x + local.x * facing_sign,
        y: root_position.y + local.y,
    }
}

fn world_point_to_local(world: Vec2, root_position: Vec2, facing: i8) -> Vec2 {
    let facing_sign = if facing < 0 { -1 } else { 1 };
    Vec2 {
        x: (world.x - root_position.x) * facing_sign,
        y: world.y - root_position.y,
    }
}

pub(crate) fn player_model_facing(player: &PlayerState) -> i8 {
    player.source_model_facing
}

fn player_source_pose_motion_state(
    player: PlayerState,
    common_data: MeleeCommonData,
) -> MotionState {
    active_pose_motion_state(&player, common_data)
}

fn player_source_pose_frame(player: PlayerState) -> u8 {
    let frame = player_animation_pose_frame(player);
    frame
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerRenderSnapshot {
    pub player_state: u8,
    pub stocks: i8,
    pub position: Vec2,
    pub source_position: SourceVec2,
    pub source_position_z: f32,
    pub source_previous_position: SourceVec2,
    pub source_previous_position_z: f32,
    pub velocity: Vec2,
    pub camera_box: FighterCameraBox,
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
    pub source_collision_state: u8,
    pub source_coll_last_pos: SourceVec2,
    pub source_coll_cur_pos: SourceVec2,
    pub source_coll_prev_pos: SourceVec2,
    pub source_coll_ecb: SourceCollEcbSnapshot,
    pub source_coll_prev_ecb: SourceCollEcbSnapshot,
    pub source_coll_desired_ecb: SourceCollEcbSnapshot,
    pub ecb_bottom_lock_timer: u8,
    pub source_coll_x130_locked: bool,
    pub source_coll_floor_surface_index: Option<u8>,
    pub source_coll_floor_line_index: Option<u16>,
    pub source_coll_floor_skip_line_index: Option<u16>,
    pub source_coll_ledge_id_left: Option<u16>,
    pub source_coll_ledge_id_right: Option<u16>,
    pub floor_skip_surface: Option<u8>,
    pub source_coll_env_flags: u32,
    pub source_coll_prev_env_flags: u32,
    pub profile_weight: f32,
    pub profile_initial_shield_size: f32,
    pub profile_model_scaling: f32,
    pub profile_is_yoshi: bool,
    pub melee_action_state_id: Option<MeleeActionStateId>,
    pub source_action_key: Option<SourceActionKey>,
    pub source_action_total_frames: u8,
    pub source_thrown_hitbox_owner_index: Option<u8>,
    pub source_thrown_hitbox_team_unk: u8,
    pub source_thrown_hitbox_grabber_player_id: Option<u8>,
    pub motion_state_alias: Option<MotionState>,
    pub motion_state: MotionState,
    pub state_frame: u8,
    pub animation_frame: u8,
    pub animation_frame_milli: i32,
    pub source_motion_anim_frame: f32,
    pub source_pose_action_state_id: Option<MeleeActionStateId>,
    pub source_pose_action_key: Option<SourceActionKey>,
    pub source_pose_motion_state: MotionState,
    pub source_pose_frame: u8,
    pub source_pose_model_facing: i8,
    pub source_fall_anim_blend: f32,
    pub source_fall_anim_pose: MotionState,
    pub source_victim_index: Option<u8>,
    pub source_x1a5c_index: Option<u8>,
    pub source_x2222_b3: bool,
    pub source_x2226_b2: bool,
    pub source_self_velocity_x: f32,
    pub source_self_velocity_y: f32,
    pub source_knockback_velocity_x: f32,
    pub source_knockback_velocity_y: f32,
    pub source_ground_knockback_velocity: f32,
    pub player_nudge_x: f32,
    pub player_nudge_z: f32,
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
    pub source_motion_anim_rate: f32,
    pub shield_health: f32,
    pub lightshield_amount: f32,
    pub shield_release_lockout_frames: u8,
    pub source_shield_collision_active: bool,
    pub source_shield_hit_active: bool,
    pub source_shield_hit_update_pos: bool,
    pub source_shield_hit_position: SourcePosePoint,
    pub source_shield_aim_angle_degrees: f32,
    pub source_shield_aim_magnitude: f32,
    pub source_x221c_b1: bool,
    pub source_x221c_b2: bool,
    pub source_x221c_b3: bool,
    pub source_guard_reflect_timer: f32,
    pub source_guard_reflect_damage_skip_timer: f32,
    pub entry_base_y: i32,
    pub entry_platform: FighterEntryPlatformProfile,
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
        let retained_model_pose = player.source_retained_model_pose;
        let source_pose_action_state_id = retained_model_pose
            .map(|pose| pose.action_state_id)
            .unwrap_or(source_pose_action_state_id);
        let source_pose_action_key = retained_model_pose
            .map(|pose| Some(pose.source_action_key))
            .unwrap_or(source_pose_action_key);
        let source_pose_motion_state = retained_model_pose
            .map(|pose| pose.motion_state)
            .unwrap_or(source_pose_motion_state);
        let source_pose_frame = retained_model_pose
            .map(|pose| (pose.frame_milli / 1_000).clamp(0, u8::MAX as i32) as u8)
            .unwrap_or_else(|| player_source_pose_frame(player));
        let source_pose_model_facing = retained_model_pose
            .map(|pose| pose.model_facing)
            .unwrap_or_else(|| player_model_facing(&player));
        Self {
            player_state: player.player_state,
            stocks: player.stocks,
            position: player.position,
            source_position: player.source_position,
            source_position_z: player.source_position_z,
            source_previous_position: player.source_previous_position,
            source_previous_position_z: player.source_previous_position_z,
            velocity: player.velocity,
            camera_box: player.profile.camera_box,
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
            source_collision_state: player.source_collision_state,
            source_coll_last_pos: player.source_coll_last_pos,
            source_coll_cur_pos: player.source_coll_cur_pos,
            source_coll_prev_pos: player.source_coll_prev_pos,
            source_coll_ecb: player.source_coll_ecb.into(),
            source_coll_prev_ecb: player.source_coll_prev_ecb.into(),
            source_coll_desired_ecb: player.source_coll_desired_ecb.into(),
            ecb_bottom_lock_timer: player.ecb_bottom_lock_timer,
            source_coll_x130_locked: player.source_coll_x130_locked,
            source_coll_floor_surface_index: player.source_coll_floor_surface_index,
            source_coll_floor_line_index: player.source_coll_floor_line_index,
            source_coll_floor_skip_line_index: player.source_coll_floor_skip_line_index,
            source_coll_ledge_id_left: player.source_coll_ledge_id_left,
            source_coll_ledge_id_right: player.source_coll_ledge_id_right,
            floor_skip_surface: player.floor_skip_surface,
            source_coll_env_flags: player.source_coll_env_flags,
            source_coll_prev_env_flags: player.source_coll_prev_env_flags,
            profile_weight: player.profile.weight,
            profile_initial_shield_size: player.profile.initial_shield_size,
            profile_model_scaling: player.profile.model_scaling,
            profile_is_yoshi: player.profile.reference_character == "yoshi",
            melee_action_state_id: player.melee_action_state_id,
            source_action_key: player.source_action_key,
            source_action_total_frames: player.source_action_total_frames,
            source_thrown_hitbox_owner_index: player.source_thrown_hitbox_owner_index,
            source_thrown_hitbox_team_unk: player.source_thrown_hitbox_team_unk,
            source_thrown_hitbox_grabber_player_id: player.source_thrown_hitbox_grabber_player_id,
            motion_state_alias: player.motion_state_alias,
            motion_state: player.motion_state,
            state_frame: player.motion_frame,
            animation_frame: player_animation_pose_frame(player),
            animation_frame_milli: player_animation_pose_frame_milli(player),
            source_motion_anim_frame: player.source_motion_anim_frame,
            source_pose_action_state_id,
            source_pose_action_key,
            source_pose_motion_state,
            source_pose_frame,
            source_pose_model_facing,
            source_fall_anim_blend: player.source_fall_anim_blend,
            source_fall_anim_pose: player.source_fall_anim_pose,
            source_victim_index: player.source_victim_index,
            source_x1a5c_index: player.source_x1a5c_index,
            source_x2222_b3: player.source_x2222_b3,
            source_x2226_b2: player.source_x2226_b2,
            source_self_velocity_x: player.source_self_velocity_x,
            source_self_velocity_y: player.source_self_velocity_y,
            source_knockback_velocity_x: player.source_knockback_velocity_x,
            source_knockback_velocity_y: player.source_knockback_velocity_y,
            source_ground_knockback_velocity: player.source_ground_knockback_velocity,
            player_nudge_x: player.player_nudge_x,
            player_nudge_z: player.player_nudge_z,
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
            source_motion_anim_rate: player.source_motion_anim_rate,
            shield_health: player.shield_health,
            lightshield_amount: player.lightshield_amount,
            shield_release_lockout_frames: player.shield_release_lockout_frames,
            source_shield_collision_active: player.source_shield_collision_active,
            source_shield_hit_active: player.source_shield_hit_active,
            source_shield_hit_update_pos: player.source_shield_hit_update_pos,
            source_shield_hit_position: player.source_shield_hit_position,
            source_shield_aim_angle_degrees: player.source_shield_aim_angle_degrees,
            source_shield_aim_magnitude: player.source_shield_aim_magnitude,
            source_x221c_b1: player.source_x221c_b1,
            source_x221c_b2: player.source_x221c_b2,
            source_x221c_b3: player.source_x221c_b3,
            source_guard_reflect_timer: player.source_guard_reflect_timer,
            source_guard_reflect_damage_skip_timer: player.source_guard_reflect_damage_skip_timer,
            entry_base_y: player.entry_base_y,
            entry_platform: player.profile.entry_platform,
            entry_platform_offset_y: player.entry_platform_offset_y,
            entry_timer: player.entry_timer,
            debug_input_facts,
        }
    }
}

fn player_animation_pose_frame(player: PlayerState) -> u8 {
    (player_animation_pose_frame_milli(player) / 1_000).clamp(0, u8::MAX as i32) as u8
}

fn player_animation_pose_frame_value(player: PlayerState) -> f32 {
    if source_binding_for_motion_state(player.motion_state).is_some()
        || player.source_action_key.is_some()
        || has_source_ecb_pose_data_for_motion_state(player.motion_state)
    {
        return player.cur_anim_frame();
    }
    match player.motion_state {
        MotionState::WalkSlow
        | MotionState::WalkMiddle
        | MotionState::WalkFast
        | MotionState::Run
        | MotionState::RunBrake
        | MotionState::TurnRun => player.cur_anim_frame(),
        _ => f32::from(player.motion_frame),
    }
}

fn player_animation_pose_frame_milli(player: PlayerState) -> i32 {
    if source_binding_for_motion_state(player.motion_state).is_some()
        || player.source_action_key.is_some()
        || has_source_ecb_pose_data_for_motion_state(player.motion_state)
    {
        return player.cur_anim_frame_milli();
    }
    match player.motion_state {
        MotionState::WalkSlow
        | MotionState::WalkMiddle
        | MotionState::WalkFast
        | MotionState::Run
        | MotionState::RunBrake
        | MotionState::TurnRun => player.cur_anim_frame_milli(),
        _ => i32::from(player.motion_frame) * 1_000,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldSnapshot {
    pub frame: Frame,
    pub stage: StageProfile,
    pub common_data: MeleeCommonData,
    pub match_phase: MatchPhase,
    pub match_phase_timer: u16,
    pub players: [PlayerRenderSnapshot; PLAYER_COUNT],
    pub checksum: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourceCollisionStep {
    pub grab_confirms: Vec<SourceGrabConfirm>,
    pub geometry_confirms: Vec<SourceHitConfirm>,
    pub raw_confirms: Vec<SourceHitConfirm>,
    pub logged_confirms: Vec<SourceHitConfirm>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceStaleMoveEntry {
    pub move_id: u8,
    pub attack_instance: u16,
}

impl SourceStaleMoveEntry {
    pub const EMPTY: Self = Self {
        move_id: 0,
        attack_instance: 0,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceStaleMoveTable {
    pub current_index: u8,
    pub entries: [SourceStaleMoveEntry; 10],
}

impl SourceStaleMoveTable {
    pub const EMPTY: Self = Self {
        current_index: 0,
        entries: [SourceStaleMoveEntry::EMPTY; 10],
    };

    pub(crate) fn damage_multiplier(self, reductions: [f32; 9], move_id: u8) -> f32 {
        if move_id == 1 {
            return 1.0;
        }

        let mut multiplier = 1.0;
        let mut index = if self.current_index != 0 {
            self.current_index - 1
        } else {
            9
        } as usize;
        for reduction in reductions {
            let entry = self.entries[index];
            if entry.move_id == 0 {
                return multiplier;
            }
            if entry.move_id == move_id {
                multiplier -= reduction;
            }
            index = if index != 0 { index - 1 } else { 9 };
        }
        multiplier
    }

    fn record_fighter_hit(&mut self, move_id: u8, attack_instance: u16) {
        if move_id == 1 {
            return;
        }
        let entry = SourceStaleMoveEntry {
            move_id,
            attack_instance,
        };
        if self.entries.contains(&entry) {
            return;
        }
        let index = self.current_index as usize;
        self.entries[index] = entry;
        self.current_index = if self.current_index == 9 {
            0
        } else {
            self.current_index + 1
        };
    }
}

macro_rules! player_rollback_snapshot_fields {
    ($emit:ident) => {
        $emit! {
            position: Vec2,
            costume_index: u8,
            player_state: u8,
            stocks: i8,
            source_position: SourceVec2,
            source_position_z: f32,
            source_previous_position: SourceVec2,
            source_previous_position_z: f32,
            source_coll_last_pos: SourceVec2,
            source_coll_cur_pos: SourceVec2,
            source_coll_prev_pos: SourceVec2,
            source_coll_x28_vec: SourceVec2,
            velocity: Vec2,
            source_self_velocity_x: f32,
            source_self_velocity_y: f32,
            source_knockback_velocity_x: f32,
            source_knockback_velocity_y: f32,
            source_ground_knockback_velocity: f32,
            source_attacker_shield_velocity_x: f32,
            player_nudge_x: f32,
            player_nudge_z: f32,
            ecb_bottom_offset_y: i32,
            ecb_bottom_lock_timer: u8,
            source_coll_ecb: SourceFighterEcb,
            source_coll_prev_ecb: SourceFighterEcb,
            source_coll_desired_ecb: SourceFighterEcb,
            source_coll_xe4_ecb: SourceFighterEcb,
            source_coll_x64_ecb: SourceFighterEcb,
            source_coll_facing_dir: i8,
            source_coll_x34_b5: bool,
            source_coll_x34_b6: bool,
            source_coll_x130_clear: bool,
            source_coll_x130_locked: bool,
            source_coll_floor_surface_index: Option<u8>,
            source_coll_floor_line_index: Option<u16>,
            source_coll_floor_skip_line_index: Option<u16>,
            source_coll_ledge_id_left: Option<u16>,
            source_coll_ledge_id_right: Option<u16>,
            source_coll_env_flags: u32,
            source_coll_prev_env_flags: u32,
            jumps_remaining: u8,
            grounded: bool,
            fast_falling: bool,
            facing: i8,
            source_model_facing: i8,
            attack_frame: u8,
            damage_percent: f32,
            damage_percent_temp: f32,
            damage_applied: u16,
            damage_knockback: f32,
            damage_angle: u16,
            damage_element: u8,
            hitlag_frames: u8,
            source_allow_sdi: bool,
            source_x2219_b5: bool,
            damage_hitstun_frames: u16,
            source_attack_id: u8,
            source_attack_instance: u16,
            source_stale_move_table: SourceStaleMoveTable,
            melee_action_state_id: Option<MeleeActionStateId>,
            source_action_key: Option<SourceActionKey>,
            source_action_total_frames: u8,
            source_retained_model_pose: Option<SourceRetainedModelPose>,
            source_down_bound_pose: Option<SourceDownBoundPose>,
            source_force_damage_down_bound: bool,
            source_down_bound_use_z_axis: bool,
            source_down_bound_reverse_face_up: bool,
            source_down_wait_timer: f32,
            source_lr_digital_press_timer: u8,
            source_lcancel_timer: u8,
            source_previous_lr_digital_press_timer: u8,
            source_jab_followup_timer: u8,
            source_jab_followup_queued: bool,
            source_jab_combo_enabled: bool,
            source_jab_rapid_enabled: bool,
            source_rapid_jab_input_count: u8,
            source_attack100_loop_has_started: bool,
            source_attack100_loop_continue_input: bool,
            source_common_timer: u8,
            source_dead_phase: u8,
            source_rebirth_target_x: f32,
            source_rebirth_target_y: f32,
            source_rebirth_platform_index: u16,
            source_rebirth_stage_point_index: u16,
            source_collision_state: u8,
            source_hurt_collision_state: u8,
            source_hurt_collision_lockout_timer: u16,
            source_hit_intangible_timer: u16,
            source_hurt_intangible_timer: u16,
            motion_state_alias: Option<MotionState>,
            motion_state: MotionState,
            source_motion_entry_facing: i8,
            motion_frame: u8,
            source_motion_anim_frame: f32,
            motion_anim_frame_milli: i32,
            source_playback: SourceFighterPlayback,
            source_fall_anim_blend: f32,
            source_fall_anim_pose: MotionState,
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
            source_allow_interrupt: bool,
            motion_cmd_var0: u32,
            motion_cmd_var1: u32,
            motion_throw_flags: u8,
            source_grab_timer: f32,
            source_grab_mash_x: i8,
            source_grab_mash_y: i8,
            source_capture_wait_timer: f32,
            source_capture_wait_anim_timer: f32,
            source_capture_wait_mashed: bool,
            source_capture_wait_jump_queued: bool,
            source_throw_x4: bool,
            source_throw_hitboxes: [Option<SourceInstalledThrowHitbox>; 2],
            source_thrown_unk_bool: bool,
            source_thrown_anim_timer: f32,
            source_thrown_hitbox_owner_index: Option<u8>,
            source_thrown_hitbox_team_unk: u8,
            source_thrown_hitbox_grabber_player_id: Option<u8>,
            source_victim_index: Option<u8>,
            source_x1a5c_index: Option<u8>,
            source_x221b_b5: bool,
            source_x2222_b3: bool,
            source_x2226_b2: bool,
            source_x1a70: SourceVec3,
            source_x34_scale_y: f32,
            captain_special_hi_x0: u16,
            captain_special_hi_vel_x: f32,
            captain_special_hi_vel_y: f32,
            captain_special_hi_x2_b0: bool,
            captain_special_hi_x2_b1: bool,
            landing_lag_ticks: u8,
            run_brake_x0: bool,
            run_brake_frames_remaining: u8,
            turn_run_x14: bool,
            turn_run_resume_advances: bool,
            turn_run_completion_pending: bool,
            turn_run_completion_enters_run: bool,
            motion_anim_rate_milli: i32,
            source_motion_anim_rate: f32,
            shield_health: f32,
            lightshield_amount: f32,
            source_collision_lightshield_amount: f32,
            shield_release_lockout_frames: u8,
            source_shield_collision_active: bool,
            source_shield_hit_active: bool,
            source_shield_hit_update_pos: bool,
            source_shield_hit_position: SourcePosePoint,
            source_shield_aim_angle_degrees: f32,
            source_shield_aim_magnitude: f32,
            source_x221c_b1: bool,
            source_x221c_b2: bool,
            source_x221c_b3: bool,
            source_guard_reflect_timer: f32,
            source_guard_reflect_damage_skip_timer: f32,
            shield_turn_facing_after: i8,
            shield_turn_frame: u8,
            guard_catch_dash_window: u8,
            guard_release_latched: bool,
            jump_input: MeleeJumpInput,
            short_hop: bool,
            escape_air_iasa_timer: u8,
            floor_skip_surface: Option<u8>,
            platform_pass_pending: bool,
            platform_pass_timer: u8,
            source_cliff_ledge_id: Option<u16>,
            source_cliff_stick_gate: bool,
            source_cliff_wait_timer: u16,
            source_ledge_cooldown_timer: u16,
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
    hsd_rng_seed: u32,
    engine_features: EngineFeatureToggles,
    match_flow_enabled: bool,
    match_phase: MatchPhase,
    match_phase_timer: u16,
    players: [PlayerRollbackSnapshot; PLAYER_COUNT],
    source_stale_attack_instance: u16,
    previous_inputs: [PlayerInput; PLAYER_COUNT],
    input_timers: [MeleeInputTimers; PLAYER_COUNT],
    last_input_facts: [MeleeInputFacts; PLAYER_COUNT],
    source_hit_victim_log: Arc<Vec<SourceHitVictimLogEntry>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineFeatureToggles {
    pub shield_turnaround_during_guard: bool,
}

impl EngineFeatureToggles {
    pub const fn parity() -> Self {
        Self {
            shield_turnaround_during_guard: false,
        }
    }

    pub const fn with_shield_turnaround_during_guard(mut self, enabled: bool) -> Self {
        self.shield_turnaround_during_guard = enabled;
        self
    }
}

impl Default for EngineFeatureToggles {
    fn default() -> Self {
        Self::parity()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchPhase {
    Ready,
    Go,
    Playing,
    StockPause,
    GameEnd,
}

impl MatchPhase {
    const fn checksum_id(self) -> u8 {
        match self {
            Self::Ready => 0,
            Self::Go => 1,
            Self::Playing => 2,
            Self::StockPause => 3,
            Self::GameEnd => 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct World {
    frame: Frame,
    hsd_rng_seed: u32,
    engine_features: EngineFeatureToggles,
    match_flow_enabled: bool,
    match_phase: MatchPhase,
    match_phase_timer: u16,
    stage: StageProfile,
    common_data: MeleeCommonData,
    players: [PlayerState; PLAYER_COUNT],
    source_stale_attack_instance: u16,
    previous_inputs: [PlayerInput; PLAYER_COUNT],
    input_timers: [MeleeInputTimers; PLAYER_COUNT],
    last_input_facts: [MeleeInputFacts; PLAYER_COUNT],
    source_hit_victim_log: Arc<Vec<SourceHitVictimLogEntry>>,
}

impl World {
    pub fn for_two_players() -> Self {
        Self::for_two_players_with_profiles([FighterProfile::FALCON_LIKE; PLAYER_COUNT])
    }

    pub fn for_two_players_with_profiles(profiles: [FighterProfile; PLAYER_COUNT]) -> Self {
        Self::for_two_players_on_stage_with_profiles(StageProfile::battlefield(), profiles)
    }

    pub fn for_two_players_with_common_data(common_data: MeleeCommonData) -> Self {
        Self::for_two_players_on_stage_with_profiles_and_common_data(
            StageProfile::battlefield(),
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
        let mut players = [
            PlayerState::new_with_profile(PLAYER_ONE_DEFAULT_SPAWN_X, 0, 1, profiles[0]),
            PlayerState::new_with_profile(PLAYER_TWO_DEFAULT_SPAWN_X, 0, -1, profiles[1]),
        ];
        for player in &mut players {
            player.shield_health = common_data.shield_start_health;
        }

        Self {
            frame: Frame(0),
            hsd_rng_seed: HSD_RAND_INITIAL_SEED,
            engine_features: EngineFeatureToggles::parity(),
            match_flow_enabled: false,
            match_phase: MatchPhase::Playing,
            match_phase_timer: 0,
            stage,
            common_data,
            players,
            source_stale_attack_instance: 1,
            previous_inputs: [PlayerInput::neutral(), PlayerInput::neutral()],
            input_timers: [MeleeInputTimers::expired(); PLAYER_COUNT],
            last_input_facts: [MeleeInputFacts::default(); PLAYER_COUNT],
            source_hit_victim_log: Arc::new(Vec::new()),
        }
    }

    pub fn for_slippi_battlefield_singles_match_start() -> Self {
        let stage = StageProfile::battlefield();
        let mut world = Self::for_two_players_on_stage_with_profiles_and_common_data(
            stage,
            [FighterProfile::FALCON_LIKE; PLAYER_COUNT],
            MeleeCommonData::provisional_mole(),
        );

        for (index, player) in world.players.iter_mut().enumerate() {
            let spawn = stage.spawn_points[index];
            player.reset_for_entry_spawn(
                spawn.x,
                spawn.y,
                spawn.facing,
                FighterProfile::FALCON_LIKE,
                world.common_data.shield_start_health,
                5 * (index as u8 + 1),
            );
        }

        world.match_flow_enabled = true;
        world.sync_match_phase_from_players();
        world
    }

    pub const fn frame(&self) -> Frame {
        self.frame
    }

    pub(crate) fn set_frame(&mut self, frame: Frame) {
        self.frame = frame;
    }

    pub const fn hsd_rng_seed(&self) -> u32 {
        self.hsd_rng_seed
    }

    pub fn set_hsd_rng_seed_for_diagnostic(&mut self, seed: u32) {
        self.hsd_rng_seed = seed;
    }

    pub fn hsd_randi_for_diagnostic(&mut self, max_value: i32) -> i32 {
        self.hsd_randi(max_value)
    }

    fn hsd_rand(&mut self) -> i32 {
        self.hsd_rng_seed = self
            .hsd_rng_seed
            .wrapping_mul(214_013)
            .wrapping_add(2_531_011);
        (self.hsd_rng_seed >> 16) as i32
    }

    fn hsd_randi(&mut self, max_value: i32) -> i32 {
        max_value * self.hsd_rand() / (1 << 16)
    }

    pub const fn engine_features(&self) -> EngineFeatureToggles {
        self.engine_features
    }

    pub const fn match_flow_enabled(&self) -> bool {
        self.match_flow_enabled
    }

    pub const fn match_phase(&self) -> MatchPhase {
        self.match_phase
    }

    pub const fn match_phase_timer(&self) -> u16 {
        self.match_phase_timer
    }

    pub(crate) fn sync_match_phase_from_players(&mut self) {
        if !self.match_flow_enabled {
            self.match_phase = MatchPhase::Playing;
            self.match_phase_timer = 0;
            return;
        }

        if self
            .players
            .iter()
            .all(|player| player.player_state == PLAYER_STATE_NONE)
        {
            self.match_phase = MatchPhase::GameEnd;
            self.match_phase_timer = 0;
            return;
        }

        if self.players.iter().any(|player| {
            player.player_state == PLAYER_STATE_IN_GAME
                && matches!(
                    player.motion_state,
                    MotionState::Entry | MotionState::EntryStart | MotionState::EntryEnd
                )
        }) {
            self.match_phase = MatchPhase::Ready;
            self.match_phase_timer = self
                .players
                .iter()
                .filter(|player| {
                    player.player_state == PLAYER_STATE_IN_GAME
                        && matches!(
                            player.motion_state,
                            MotionState::Entry | MotionState::EntryStart | MotionState::EntryEnd
                        )
                })
                .map(|player| u16::from(player.entry_timer))
                .max()
                .unwrap_or(0);
            return;
        }

        if self.players.iter().any(|player| {
            player.player_state == PLAYER_STATE_IN_GAME
                && (is_source_dead_motion_state(player.motion_state)
                    || is_source_rebirth_motion_state(player.motion_state))
        }) {
            self.match_phase = MatchPhase::StockPause;
            self.match_phase_timer = self
                .players
                .iter()
                .map(|player| u16::from(player.source_common_timer))
                .max()
                .unwrap_or(0);
            return;
        }

        self.match_phase = MatchPhase::Playing;
        self.match_phase_timer = 0;
    }

    pub fn set_engine_features(&mut self, features: EngineFeatureToggles) {
        self.engine_features = features;
        if !self.engine_features.shield_turnaround_during_guard {
            for player in &mut self.players {
                player.shield_turn_facing_after = player.facing;
                player.shield_turn_frame = 0;
            }
        }
    }

    pub const fn stage(&self) -> StageProfile {
        self.stage
    }

    pub const fn common_data(&self) -> MeleeCommonData {
        self.common_data
    }

    pub(crate) fn update_source_attack_ids_from_action_state_changes(
        &mut self,
        previous_action_state_ids: [Option<MeleeActionStateId>; PLAYER_COUNT],
    ) {
        for (player_index, previous_action_state_id) in
            previous_action_state_ids.into_iter().enumerate()
        {
            self.update_source_attack_id_from_action_state_change(
                player_index,
                previous_action_state_id,
            );
        }
    }

    fn update_source_attack_id_from_action_state_change(
        &mut self,
        player_index: usize,
        previous_action_state_id: Option<MeleeActionStateId>,
    ) -> bool {
        let Some(player) = self.players.get(player_index) else {
            return false;
        };
        let current_action_state_id = player.melee_action_state_id;
        if current_action_state_id == previous_action_state_id {
            return false;
        }
        let move_id = current_action_state_id.map_or(1, source_move_id_for_action_state_id);
        self.source_change_player_attack_move_id(player_index, move_id)
    }

    fn source_change_player_attack_move_id(&mut self, player_index: usize, move_id: u8) -> bool {
        let Some(player) = self.players.get(player_index) else {
            return false;
        };
        if move_id != 1 && move_id == player.source_attack_id {
            return false;
        }
        let attack_instance = self.source_increment_attack_instance();
        let Some(player) = self.players.get_mut(player_index) else {
            return false;
        };
        player.source_attack_id = move_id;
        player.source_attack_instance = attack_instance;
        true
    }

    fn source_increment_attack_instance(&mut self) -> u16 {
        let before = self.source_stale_attack_instance;
        self.source_stale_attack_instance = self.source_stale_attack_instance.wrapping_add(1);
        if self.source_stale_attack_instance == 0 {
            self.source_stale_attack_instance = 1;
        }
        before
    }

    pub const fn players(&self) -> &[PlayerState; PLAYER_COUNT] {
        &self.players
    }

    pub(crate) fn players_mut(&mut self) -> &mut [PlayerState; PLAYER_COUNT] {
        &mut self.players
    }

    pub fn set_player_costume_index(&mut self, player_index: usize, costume_index: u8) -> bool {
        let Some(player) = self.players.get_mut(player_index) else {
            return false;
        };
        player.costume_index = costume_index;
        true
    }

    pub fn reset_player_to_stage_spawn(&mut self, player_index: usize) -> bool {
        let Some(player) = self.players.get_mut(player_index) else {
            return false;
        };
        let Some(spawn) = self.stage.spawn_points.get(player_index).copied() else {
            return false;
        };

        let profile = player.profile;
        let entry_timer = 5 * (player_index as u8 + 1);
        player.reset_for_entry_spawn(
            spawn.x,
            spawn.y,
            spawn.facing,
            profile,
            self.common_data.shield_start_health,
            entry_timer,
        );
        true
    }

    pub fn lose_player_stock_for_stage_blast_zone(
        &mut self,
        player_index: usize,
        direction: SourceDeathDirection,
    ) -> bool {
        {
            let Some(player) = self.players.get_mut(player_index) else {
                return false;
            };
            if player.player_state != PLAYER_STATE_IN_GAME || player.stocks <= 0 {
                return false;
            }
            player.stocks -= 1;
        }

        let motion_state = if direction == SourceDeathDirection::Up {
            let roll = self.hsd_randi(100) + 1;
            if roll <= self.common_data.top_blast_fall_ko_chance as i32 {
                MotionState::DeadUpFall
            } else {
                MotionState::DeadUpStar
            }
        } else {
            direction.motion_state()
        };

        let player = &mut self.players[player_index];
        player.enter_source_dead_motion_state_with_common_data(motion_state, self.common_data);
        if player.stocks <= 0 {
            player.player_state = PLAYER_STATE_NONE;
        }
        true
    }

    pub fn resolve_stage_blast_zone_ko_for_player(&mut self, player_index: usize) -> bool {
        let Some(player) = self.players.get(player_index) else {
            return false;
        };
        if player.player_state != PLAYER_STATE_IN_GAME {
            return false;
        }
        if is_source_dead_motion_state(player.motion_state)
            || is_source_rebirth_motion_state(player.motion_state)
        {
            return false;
        }

        let position = player.position;
        let blast_zones = self.stage.blast_zones;
        let direction = if position.x > blast_zones.right_x {
            Some(SourceDeathDirection::Right)
        } else if position.x < blast_zones.left_x {
            Some(SourceDeathDirection::Left)
        } else if position.y > blast_zones.top_y {
            Some(SourceDeathDirection::Up)
        } else if position.y < blast_zones.bottom_y {
            Some(SourceDeathDirection::Down)
        } else {
            None
        };

        direction.is_some_and(|direction| {
            self.lose_player_stock_for_stage_blast_zone(player_index, direction)
        })
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
            hsd_rng_seed: self.hsd_rng_seed,
            engine_features: self.engine_features,
            match_flow_enabled: self.match_flow_enabled,
            match_phase: self.match_phase,
            match_phase_timer: self.match_phase_timer,
            players: self.players.map(PlayerRollbackSnapshot::from_player),
            source_stale_attack_instance: self.source_stale_attack_instance,
            previous_inputs: self.previous_inputs,
            input_timers: self.input_timers,
            last_input_facts: self.last_input_facts,
            source_hit_victim_log: self.source_hit_victim_log.clone(),
        }
    }

    pub fn restore_rollback_snapshot(&mut self, snapshot: &WorldRollbackSnapshot) {
        self.frame = snapshot.frame;
        self.hsd_rng_seed = snapshot.hsd_rng_seed;
        self.engine_features = snapshot.engine_features;
        self.match_flow_enabled = snapshot.match_flow_enabled;
        self.match_phase = snapshot.match_phase;
        self.match_phase_timer = snapshot.match_phase_timer;
        for (player, player_snapshot) in self.players.iter_mut().zip(snapshot.players) {
            player_snapshot.restore_into(player);
        }
        self.source_stale_attack_instance = snapshot.source_stale_attack_instance;
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
        let previous_action_state_id = player.melee_action_state_id;
        if state.motion_state_alias.is_some()
            && state.motion_state_alias != Some(state.motion_state)
        {
            state.set_motion_state_alias(state.motion_state);
        }
        let normalized_anim_frame =
            if state.source_motion_anim_frame == 0.0 && state.motion_anim_frame_milli != 0 {
                state.motion_anim_frame_milli as f32 / 1000.0
            } else {
                state.source_motion_anim_frame
            };
        let normalized_anim_rate = state.frame_speed_mul();
        state.set_source_motion_anim_frame(normalized_anim_frame);
        state.set_source_motion_anim_rate(normalized_anim_rate);
        let diagnostic_action_needs_task_3b2_playback =
            state.source_action_key.is_some_and(|key| {
                matches!(
                    key.as_str(),
                    "Turn"
                        | "AttackAirN"
                        | "AttackAirF"
                        | "AttackAirB"
                        | "AttackAirHi"
                        | "AttackAirLw"
                )
            });
        if diagnostic_action_needs_task_3b2_playback
            && state.source_playback.primary.flags & SOURCE_AOBJ_NO_ANIM != 0
            && state.source_playback.primary.rewind_frame == 0.0
            && state.source_playback.primary.end_frame == 0.0
        {
            if let Some(descriptor) = source_migrated_primary_anim_descriptor_for_player(&state) {
                state.install_source_primary_anim(
                    normalized_anim_frame,
                    normalized_anim_rate,
                    descriptor,
                );
                let _ = state.interpret_source_primary_anim();
            }
        }
        let projected_source_position = state.source_position.to_milli();
        if projected_source_position != state.position {
            let source_position_changed = state.source_position != player.source_position;
            let public_position_changed = state.position != player.position;
            if source_position_changed || !public_position_changed {
                state.position = projected_source_position;
            } else {
                state.source_position = SourceVec2::from_milli(state.position);
            }
        }
        if state.source_coll_last_pos == player.source_coll_last_pos
            && state.source_coll_cur_pos == player.source_coll_cur_pos
            && state.source_coll_prev_pos == player.source_coll_prev_pos
            && state.source_coll_x28_vec == player.source_coll_x28_vec
            && state.source_position != player.source_position
        {
            state.source_coll_last_pos = state.source_position;
            state.source_coll_cur_pos = state.source_position;
            state.source_coll_prev_pos = state.source_position;
            state.source_coll_x28_vec = state.source_position;
        }
        let is_source_damage_state = state
            .melee_action_state_id
            .is_some_and(is_source_damage_action_state_id);
        if (!state.grounded || is_source_damage_state)
            && state.source_knockback_velocity_x == 0.0
            && state.source_knockback_velocity_y == 0.0
        {
            if is_source_damage_state {
                state.source_knockback_velocity_x = milli_to_source_units(state.velocity.x);
                state.source_knockback_velocity_y = milli_to_source_units(state.velocity.y);
                state.source_self_velocity_x = 0.0;
                state.source_self_velocity_y = 0.0;
            } else {
                if source_units_to_milli(state.source_self_velocity_x) != state.velocity.x {
                    state.source_self_velocity_x = milli_to_source_units(state.velocity.x);
                }
                if source_units_to_milli(state.source_self_velocity_y) != state.velocity.y {
                    state.source_self_velocity_y = milli_to_source_units(state.velocity.y);
                }
            }
        }
        *player = state;
        self.update_source_attack_id_from_action_state_change(
            player_index,
            previous_action_state_id,
        );
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
        self.apply_source_collision_frame_with_action_total_frames(collision_frame, |_| None)
    }

    pub fn apply_source_collision_frame_with_action_total_frames(
        &mut self,
        collision_frame: &SourceCollisionFrame,
        mut action_total_frames: impl FnMut(MeleeActionStateId) -> Option<u8>,
    ) -> SourceCollisionStep {
        self.retain_source_hit_victim_log_for_frame(collision_frame);
        let grab_confirms = self.apply_source_grab_confirms(
            source_grab_confirms(collision_frame)
                .into_iter()
                .filter(|confirm| {
                    self.players
                        .get(confirm.victim_index)
                        .is_some_and(|player| player.source_allows_hurt_collision())
                })
                .collect(),
            &mut action_total_frames,
        );
        let geometry_confirms = source_hit_confirms(collision_frame);
        let raw_confirms = geometry_confirms
            .clone()
            .into_iter()
            .filter(|confirm| self.source_hit_confirm_allowed_by_thrown_hitbox_state(*confirm))
            .filter(|confirm| {
                self.players
                    .get(confirm.victim_index)
                    .is_some_and(|player| player.source_allows_hurt_collision())
            })
            .collect::<Vec<_>>();
        let logged_confirms = self.source_hit_confirms_not_in_victim_log(raw_confirms.clone());
        self.apply_source_special_hit_detect_transitions(
            &logged_confirms,
            &mut action_total_frames,
        );
        let current_pose_confirms = logged_confirms
            .iter()
            .copied()
            .filter(|confirm| self.source_hit_confirm_matches_current_victim_pose(*confirm))
            .collect::<Vec<_>>();
        self.release_stale_pose_confirms_from_victim_log(&logged_confirms, &current_pose_confirms);
        self.apply_source_deal_damage_hitlag_from_confirms(&current_pose_confirms);
        self.apply_source_shield_health_from_confirms(&current_pose_confirms);
        self.apply_source_shield_setoff_from_confirms(
            &current_pose_confirms,
            &mut action_total_frames,
        );
        let damage_gate_confirms = current_pose_confirms
            .iter()
            .copied()
            .filter(|confirm| !Self::source_hit_confirm_targets_shield(*confirm))
            .filter(|confirm| {
                !Self::source_hit_confirm_blocked_by_shield(*confirm, &logged_confirms)
            })
            .filter(|confirm| !self.source_hit_confirm_routes_hurtbox_detect_callback(*confirm))
            .filter(|confirm| self.source_hit_confirm_allowed_by_victim_damage_gate(*confirm))
            .collect::<Vec<_>>();
        let mut confirms = Vec::new();
        let mut held_victim_confirms = Vec::new();
        for confirm in damage_gate_confirms {
            if self.source_hit_confirm_targets_active_held_victim(confirm) {
                held_victim_confirms.push(confirm);
            } else {
                confirms.push(confirm);
            }
        }
        let stages = self.source_damage_stages_from_confirms_with_stale(&confirms);
        let held_victim_stages =
            self.source_damage_stages_from_confirms_with_stale(&held_victim_confirms);
        let mut applied_stages = stages.clone();
        applied_stages.extend(held_victim_stages);
        confirms.extend(held_victim_confirms);
        self.update_source_stale_moves_from_confirms(&current_pose_confirms);
        let results = self.source_damage_results_for_stages(&stages);
        let applied_stage_count = self.apply_source_damage_stages(&applied_stages);
        SourceCollisionStep {
            grab_confirms,
            geometry_confirms,
            raw_confirms,
            logged_confirms,
            confirms,
            stages: applied_stages,
            results,
            applied_stage_count,
        }
    }

    fn apply_source_grab_confirms(
        &mut self,
        confirms: Vec<SourceGrabConfirm>,
        action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
    ) -> Vec<SourceGrabConfirm> {
        let mut closest_by_grabber: [Option<(SourceGrabConfirm, f32)>; PLAYER_COUNT] =
            [None; PLAYER_COUNT];
        for confirm in confirms {
            if confirm.grabber_index >= PLAYER_COUNT || confirm.victim_index >= PLAYER_COUNT {
                continue;
            }
            let Some(distance) = self.source_grab_distance(confirm) else {
                continue;
            };
            let closest = &mut closest_by_grabber[confirm.grabber_index];
            if closest
                .as_ref()
                .is_none_or(|(_, closest_distance)| distance < *closest_distance)
            {
                *closest = Some((confirm, distance));
            }
        }

        let mut applied = Vec::new();
        for confirm in closest_by_grabber
            .into_iter()
            .filter_map(|entry| entry.map(|(confirm, _)| confirm))
        {
            if self.apply_source_grab_confirm(confirm, action_total_frames) {
                applied.push(confirm);
            }
        }
        applied
    }

    fn source_grab_distance(&self, confirm: SourceGrabConfirm) -> Option<f32> {
        if confirm.grabber_index == confirm.victim_index {
            return None;
        }
        let grabber = self.players.get(confirm.grabber_index)?;
        let victim = self.players.get(confirm.victim_index)?;
        let action_state_id = confirm.action_state_id.or(grabber.melee_action_state_id)?;
        if source_grab_pull_action_state(action_state_id).is_none()
            && source_special_capture_transition_for_action_state_id(
                grabber.profile.reference_character,
                action_state_id,
            )
            .is_none()
        {
            return None;
        }
        Some((victim.source_position.x - grabber.source_position.x).abs())
    }

    fn apply_source_grab_confirm(
        &mut self,
        confirm: SourceGrabConfirm,
        action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
    ) -> bool {
        let Ok(grabber_u8) = u8::try_from(confirm.grabber_index) else {
            return false;
        };
        let Ok(victim_u8) = u8::try_from(confirm.victim_index) else {
            return false;
        };
        let Some((grabber, victim)) = players_two_mut(
            &mut self.players,
            confirm.grabber_index,
            confirm.victim_index,
        ) else {
            return false;
        };
        if !victim.source_allows_hurt_collision() {
            return false;
        }
        let Some(action_state_id) = confirm.action_state_id.or(grabber.melee_action_state_id)
        else {
            return false;
        };
        if let Some(transition) = source_special_capture_transition_for_action_state_id(
            grabber.profile.reference_character,
            action_state_id,
        ) {
            let victim_was_airborne = !victim.grounded;
            let attacker_facing = grabber.facing;
            enter_source_only_action_state(
                grabber,
                transition.attacker_action_state_id,
                transition.attacker_source_action_key,
                action_total_frames,
            );
            grabber.source_victim_index = Some(victim_u8);
            grabber.source_x1a5c_index = Some(victim_u8);
            grabber.source_x221b_b5 = false;
            grabber.source_x2226_b2 = false;

            enter_source_only_action_state(
                victim,
                transition.victim_action_state_id,
                transition.victim_source_action_key,
                action_total_frames,
            );
            victim.source_victim_index = Some(grabber_u8);
            victim.source_x1a5c_index = Some(grabber_u8);
            victim.source_x221b_b5 = false;
            victim.source_x2226_b2 =
                transition.constrain_airborne_victim_to_attacker_transn2 && victim_was_airborne;
            victim.facing = -attacker_facing;
            victim.source_model_facing = victim.facing;
            victim.source_self_velocity_x = 0.0;
            victim.source_self_velocity_y = 0.0;
            victim.source_knockback_velocity_x = 0.0;
            victim.source_knockback_velocity_y = 0.0;
            victim.source_ground_knockback_velocity = 0.0;
            victim.velocity = Vec2 { x: 0, y: 0 };
            victim.ground_velocity_x = 0.0;
            victim.ground_accel_x = 0.0;
            victim.ground_accel_x2 = 0.0;
            return true;
        }
        let Some((grabber_action_state_id, grabber_source_action_key)) =
            source_grab_pull_action_state(action_state_id)
        else {
            return false;
        };
        let grabber_motion_frame = grabber.motion_frame;
        let grabber_source_motion_anim_frame = grabber.source_motion_anim_frame;
        enter_source_only_action_state(
            grabber,
            grabber_action_state_id,
            grabber_source_action_key,
            action_total_frames,
        );
        grabber.motion_frame = grabber_motion_frame;
        grabber.set_source_motion_anim_frame(grabber_source_motion_anim_frame);
        clear_source_grab_ground_velocity(grabber);
        let grabber_facing = grabber.facing;
        grabber.source_victim_index = Some(victim_u8);
        grabber.source_x1a5c_index = Some(victim_u8);
        grabber.source_x221b_b5 = true;
        grabber.source_x2226_b2 = false;

        let victim_action_state_id = if victim.grounded {
            MeleeActionStateId::new(226)
        } else {
            MeleeActionStateId::new(223)
        };
        let Some(victim_binding) =
            canonical_source_action_binding_for_runtime_id(victim_action_state_id)
        else {
            return false;
        };
        enter_source_only_action_state(
            victim,
            victim_action_state_id,
            victim_binding.source_action_key,
            action_total_frames,
        );
        victim.source_victim_index = Some(grabber_u8);
        victim.source_x1a5c_index = Some(grabber_u8);
        victim.source_x221b_b5 = false;
        victim.source_x2226_b2 = false;
        victim.facing = -grabber_facing;
        victim.source_model_facing = victim.facing;
        victim.source_motion_entry_facing = victim.facing;
        victim.source_self_velocity_x = 0.0;
        victim.source_self_velocity_y = 0.0;
        victim.source_knockback_velocity_x = 0.0;
        victim.source_knockback_velocity_y = 0.0;
        victim.source_ground_knockback_velocity = 0.0;
        victim.velocity = Vec2 { x: 0, y: 0 };
        victim.ground_velocity_x = 0.0;
        victim.ground_accel_x = 0.0;
        victim.ground_accel_x2 = 0.0;
        true
    }

    fn retain_source_hit_victim_log_for_frame(&mut self, collision_frame: &SourceCollisionFrame) {
        let active_hitboxes = collision_frame
            .hits
            .iter()
            .filter_map(|hit| hit.hitbox.map(|hitbox| source_hitbox_log_key(*hit, hitbox)))
            .collect::<Vec<_>>();
        Arc::make_mut(&mut self.source_hit_victim_log)
            .retain(|entry| active_hitboxes.contains(&entry.hitbox));
    }

    fn source_hit_confirms_not_in_victim_log(
        &mut self,
        confirms: Vec<SourceHitConfirm>,
    ) -> Vec<SourceHitConfirm> {
        let mut accepted = Vec::new();
        for confirm in confirms.iter().copied() {
            let hitbox = source_hitbox_log_key(confirm.collision.hit, confirm.hitbox);
            let entry = SourceHitVictimLogEntry {
                hitbox,
                victim_index: confirm.victim_index,
            };
            if self.source_hit_victim_log.contains(&entry) {
                continue;
            }
            Arc::make_mut(&mut self.source_hit_victim_log).push(entry);
            accepted.push(confirm);
        }
        accepted
    }

    fn source_hit_confirm_targets_shield(confirm: SourceHitConfirm) -> bool {
        confirm.hurtbox_id == SOURCE_SHIELD_HURTBOX_ID
    }

    fn source_hit_confirm_blocked_by_shield(
        confirm: SourceHitConfirm,
        confirms: &[SourceHitConfirm],
    ) -> bool {
        confirms.iter().copied().any(|shield_confirm| {
            Self::source_hit_confirm_targets_shield(shield_confirm)
                && shield_confirm.attacker_index == confirm.attacker_index
                && shield_confirm.victim_index == confirm.victim_index
                && shield_confirm.hitbox_id == confirm.hitbox_id
        })
    }

    fn source_hit_confirm_routes_hurtbox_detect_callback(&self, confirm: SourceHitConfirm) -> bool {
        let Some(attacker) = self.players.get(confirm.attacker_index) else {
            return false;
        };
        let Some(action_state_id) = confirm.action_state_id.or(attacker.melee_action_state_id)
        else {
            return false;
        };
        source_special_hit_detect_transition_for_action_state_id(
            attacker.profile.reference_character,
            action_state_id,
        )
        .is_some()
    }

    fn apply_source_shield_health_from_confirms(&mut self, confirms: &[SourceHitConfirm]) {
        for confirm in confirms
            .iter()
            .copied()
            .filter(|confirm| Self::source_hit_confirm_targets_shield(*confirm))
        {
            let env_damage =
                source_env_damage(self.source_stale_scaled_damage(
                    confirm.attacker_index,
                    confirm.hitbox.damage as f32,
                ));
            let Some(victim) = self.players.get_mut(confirm.victim_index) else {
                continue;
            };
            if !victim.source_shield_collision_active {
                continue;
            }
            if victim.source_x221c_b2 {
                continue;
            }
            let shield_damage_taken =
                (i32::from(env_damage) + i32::from(confirm.hitbox.shield_damage)).max(0) as f32;
            let lightshield_scale = victim.source_collision_lightshield_amount
                * (self.common_data.shield_hit_lightshield_max
                    - self.common_data.shield_hit_lightshield_min)
                + self.common_data.shield_hit_lightshield_min;
            victim.shield_health -= self.common_data.shield_hit_drain_damage_scale
                * (shield_damage_taken * (1.0 - lightshield_scale))
                + self.common_data.shield_hit_drain_base;
            if victim.shield_health < 0.0 {
                enter_source_shield_break_fly(victim, self.common_data);
            }
        }
    }

    fn apply_source_shield_setoff_from_confirms(
        &mut self,
        confirms: &[SourceHitConfirm],
        action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
    ) {
        let mut best_by_victim: [Option<SourceHitConfirm>; PLAYER_COUNT] = [None; PLAYER_COUNT];
        for confirm in confirms
            .iter()
            .copied()
            .filter(|confirm| Self::source_hit_confirm_targets_shield(*confirm))
        {
            if self.source_hit_confirm_routes_hurtbox_detect_callback(confirm) {
                continue;
            }
            let Some(victim) = self.players.get(confirm.victim_index) else {
                continue;
            };
            if !source_player_can_enter_guard_setoff(*victim) {
                continue;
            }
            let env_damage =
                source_env_damage(self.source_stale_scaled_damage(
                    confirm.attacker_index,
                    confirm.hitbox.damage as f32,
                ));
            let best = &mut best_by_victim[confirm.victim_index];
            if best.as_ref().is_none_or(|previous| {
                env_damage
                    > source_env_damage(self.source_stale_scaled_damage(
                        previous.attacker_index,
                        previous.hitbox.damage as f32,
                    ))
            }) {
                *best = Some(confirm);
            }
        }

        for confirm in best_by_victim.into_iter().flatten() {
            let env_damage =
                source_env_damage(self.source_stale_scaled_damage(
                    confirm.attacker_index,
                    confirm.hitbox.damage as f32,
                ));
            let hitlag_env_damage = env_damage;
            let attacker_x = self
                .players
                .get(confirm.attacker_index)
                .map(|attacker| attacker.source_position.x)
                .unwrap_or(0.0);
            let (victim_x, victim_lightshield) = self
                .players
                .get(confirm.victim_index)
                .map(|victim| {
                    (
                        victim.source_position.x,
                        victim.source_collision_lightshield_amount,
                    )
                })
                .unwrap_or((attacker_x, 0.0));
            if let Some(attacker) = self.players.get_mut(confirm.attacker_index) {
                if attacker.grounded {
                    let recoil = (victim_lightshield
                        * f32::from(env_damage)
                        * self.common_data.attacker_shield_knockback_damage_scale)
                        + self.common_data.attacker_shield_knockback_base;
                    attacker.source_attacker_shield_velocity_x =
                        recoil * if victim_x > attacker_x { -1.0 } else { 1.0 };
                }
            }
            let Some(victim) = self.players.get_mut(confirm.victim_index) else {
                continue;
            };
            if !source_player_can_enter_guard_setoff(*victim) {
                continue;
            }
            let setoff_frames = source_guard_setoff_frames(
                self.common_data,
                env_damage,
                victim.source_collision_lightshield_amount,
            );
            let action_state_id = melee_action_state_id_for_motion_state(MotionState::GuardSetOff);
            victim.set_motion_state_alias(MotionState::GuardSetOff);
            victim.source_action_total_frames =
                action_total_frames(action_state_id).unwrap_or(victim.source_action_total_frames);
            let sampled_end_frame = f32::from(victim.source_action_total_frames);
            let jobj_end_frame = sampled_end_frame + 1.0;
            let setoff_anim_rate = (jobj_end_frame + 0.1) / setoff_frames.max(f32::EPSILON);
            let setoff_ticks = (jobj_end_frame / setoff_anim_rate)
                .ceil()
                .clamp(1.0, f32::from(u8::MAX)) as u8;
            victim.motion_frame = 0;
            victim.set_source_motion_anim_frame(0.0);
            victim.set_source_motion_anim_rate_milli(1_000);
            victim.source_common_timer = setoff_ticks;
            victim.source_shield_collision_active = true;
            victim.source_shield_hit_active = true;
            victim.source_shield_hit_update_pos = true;
            victim.grounded = true;
            victim.source_self_velocity_x = 0.0;
            victim.source_self_velocity_y = 0.0;
            victim.source_knockback_velocity_x = 0.0;
            victim.source_knockback_velocity_y = 0.0;
            victim.source_ground_knockback_velocity = 0.0;
            victim.velocity.y = 0;
            let hitlag_frames =
                source_hitlag_frames_for_env_damage(self.common_data, hitlag_env_damage);
            victim.hitlag_frames = victim.hitlag_frames.max(hitlag_frames);
            victim.source_allow_sdi = hitlag_frames != 0;
            victim.source_x2219_b5 = hitlag_frames != 0;
            if confirm.hitbox.element != 10 {
                let pushback = source_guard_setoff_pushback(
                    self.common_data,
                    env_damage,
                    victim.source_collision_lightshield_amount,
                );
                let push_dir = if victim.source_position.x > attacker_x {
                    1.0
                } else {
                    -1.0
                };
                victim.ground_velocity_x = pushback * push_dir;
                victim.velocity.x = source_units_to_milli(victim.ground_velocity_x);
            }
        }
    }

    fn apply_source_special_hit_detect_transitions(
        &mut self,
        confirms: &[SourceHitConfirm],
        action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
    ) {
        for confirm in confirms {
            let Some(attacker) = self.players.get_mut(confirm.attacker_index) else {
                continue;
            };
            let Some(action_state_id) = confirm.action_state_id.or(attacker.melee_action_state_id)
            else {
                continue;
            };
            let Some(transition) = source_special_hit_detect_transition_for_action_state_id(
                attacker.profile.reference_character,
                action_state_id,
            ) else {
                continue;
            };
            if attacker.motion_cmd_var0 == 0 {
                continue;
            }
            attacker.set_motion_state_alias(transition.next_motion_state);
            attacker.melee_action_state_id = Some(transition.next_action_state_id);
            attacker.source_action_key = Some(transition.next_source_action_key);
            attacker.source_action_total_frames =
                action_total_frames(transition.next_action_state_id).unwrap_or(0);
            attacker.motion_frame = 0;
            attacker.set_source_motion_anim_frame(0.0);
            attacker.set_source_motion_anim_rate_milli(1_000);
            if transition.clear_self_velocity_y {
                attacker.source_self_velocity_y = 0.0;
                attacker.velocity.y = 0;
            }
            if transition.scale_ground_velocity_by_specials_gr_vel_x {
                attacker.ground_velocity_x *=
                    attacker.profile.captain_special_attrs.specials_gr_vel_x;
                attacker.velocity.x = source_units_to_milli(attacker.ground_velocity_x);
            }
        }
    }

    fn release_stale_pose_confirms_from_victim_log(
        &mut self,
        logged_confirms: &[SourceHitConfirm],
        current_pose_confirms: &[SourceHitConfirm],
    ) {
        let stale_entries = logged_confirms
            .iter()
            .copied()
            .filter(|confirm| !current_pose_confirms.contains(confirm))
            .map(|confirm| SourceHitVictimLogEntry {
                hitbox: source_hitbox_log_key(confirm.collision.hit, confirm.hitbox),
                victim_index: confirm.victim_index,
            })
            .filter(|entry| {
                !current_pose_confirms.iter().copied().any(|confirm| {
                    confirm.victim_index == entry.victim_index
                        && source_hitbox_log_key(confirm.collision.hit, confirm.hitbox)
                            == entry.hitbox
                })
            })
            .collect::<Vec<_>>();
        Arc::make_mut(&mut self.source_hit_victim_log)
            .retain(|entry| !stale_entries.contains(entry));
    }
    fn source_hit_confirm_matches_current_victim_pose(&self, confirm: SourceHitConfirm) -> bool {
        let Some(captured_action_state_id) = confirm.collision.hurt.action_state_id else {
            return true;
        };
        self.players
            .get(confirm.victim_index)
            .is_none_or(|victim| victim.melee_action_state_id == Some(captured_action_state_id))
    }
    fn source_hit_confirm_allowed_by_thrown_hitbox_state(&self, confirm: SourceHitConfirm) -> bool {
        let Some(attacker) = self.players.get(confirm.attacker_index) else {
            return false;
        };
        let Some(victim) = self.players.get(confirm.victim_index) else {
            return false;
        };
        let Ok(attacker_index) = u8::try_from(confirm.attacker_index) else {
            return false;
        };
        let Ok(victim_index) = u8::try_from(confirm.victim_index) else {
            return false;
        };
        if confirm
            .collision
            .hit
            .hitbox_flags
            .skip_if_thrown_hitbox_owner_absent()
            && attacker.source_thrown_hitbox_owner_index.is_none()
        {
            return false;
        }
        if attacker.source_thrown_hitbox_owner_index == Some(victim_index) {
            return false;
        }
        if attacker.source_thrown_hitbox_grabber_player_id == Some(victim_index) {
            return false;
        }
        if confirm.collision.hit.hitbox_flags.hit_grabbed_victim_only()
            && victim.source_victim_index.is_some()
            && victim.source_x221b_b5
            && victim.source_victim_index != Some(attacker_index)
        {
            return false;
        }
        true
    }

    fn source_hit_confirm_targets_active_held_victim(&self, confirm: SourceHitConfirm) -> bool {
        if confirm.attacker_index == confirm.victim_index {
            return false;
        }
        let Some(attacker) = self.players.get(confirm.attacker_index) else {
            return false;
        };
        let Some(victim) = self.players.get(confirm.victim_index) else {
            return false;
        };
        let Ok(attacker_index) = u8::try_from(confirm.attacker_index) else {
            return false;
        };
        let Ok(victim_index) = u8::try_from(confirm.victim_index) else {
            return false;
        };
        let action_state_id = confirm.action_state_id.or(attacker.melee_action_state_id);
        action_state_id.is_some_and(source_held_victim_damage_action_state_id)
            && attacker.source_victim_index == Some(victim_index)
            && attacker.source_x1a5c_index == Some(victim_index)
            && victim.source_victim_index == Some(attacker_index)
            && victim.source_x1a5c_index == Some(attacker_index)
            && victim
                .melee_action_state_id
                .is_some_and(source_held_victim_action_state_id)
    }

    fn source_hit_confirm_allowed_by_victim_damage_gate(&self, confirm: SourceHitConfirm) -> bool {
        self.players
            .get(confirm.victim_index)
            .is_some_and(|victim| victim.source_hurt_collision_state == 0)
    }

    fn source_damage_stages_from_confirms_with_stale(
        &self,
        confirms: &[SourceHitConfirm],
    ) -> Vec<SourceDamageStage> {
        confirms
            .iter()
            .map(|confirm| {
                let scaled_damage = confirm.hitbox.damage as f32;
                let damage = self.source_stale_scaled_damage(confirm.attacker_index, scaled_damage);
                SourceDamageStage {
                    attacker_index: confirm.attacker_index,
                    victim_index: confirm.victim_index,
                    hitbox_id: confirm.hitbox_id,
                    hurtbox_id: confirm.hurtbox_id,
                    action_state_id: confirm.action_state_id,
                    source_action_key: confirm.source_action_key,
                    source_frame: confirm.source_frame,
                    damaged_hurt_height: confirm.damaged_hurt_height,
                    damage,
                    env_damage: source_env_damage(damage),
                    unk_count: scaled_damage as u16,
                    hitbox: confirm.hitbox,
                }
            })
            .collect()
    }

    fn source_stale_scaled_damage(&self, attacker_index: usize, scaled_damage: f32) -> f32 {
        let Some(attacker) = self.players.get(attacker_index) else {
            return scaled_damage;
        };
        let multiplier = attacker.source_stale_move_table.damage_multiplier(
            self.common_data.stale_move_damage_reductions,
            attacker.source_attack_id,
        );
        if multiplier == 1.0 {
            scaled_damage
        } else {
            scaled_damage * multiplier
        }
    }

    fn update_source_stale_moves_from_confirms(&mut self, confirms: &[SourceHitConfirm]) {
        for confirm in confirms {
            if confirm.hitbox.damage == 0 || Self::source_hit_confirm_targets_shield(*confirm) {
                continue;
            }
            self.update_source_stale_moves_from_fighter(
                confirm.attacker_index,
                confirm.victim_index,
            );
        }
    }

    fn update_source_stale_moves_from_fighter(
        &mut self,
        attacker_index: usize,
        victim_index: usize,
    ) -> bool {
        if attacker_index == victim_index {
            return false;
        }
        let Some(attacker) = self.players.get_mut(attacker_index) else {
            return false;
        };
        let before = attacker.source_stale_move_table;
        attacker
            .source_stale_move_table
            .record_fighter_hit(attacker.source_attack_id, attacker.source_attack_instance);
        attacker.source_stale_move_table != before
    }

    fn apply_source_deal_damage_hitlag_from_confirms(&mut self, confirms: &[SourceHitConfirm]) {
        for attacker_index in 0..self.players.len() {
            let max_env_damage = confirms
                .iter()
                .filter(|confirm| confirm.attacker_index == attacker_index)
                .map(|confirm| {
                    let scaled_damage = self
                        .source_stale_scaled_damage(attacker_index, confirm.hitbox.damage as f32);
                    source_env_damage(scaled_damage)
                })
                .max()
                .unwrap_or(0);
            if max_env_damage == 0 {
                continue;
            }
            let hitlag_frames =
                source_hitlag_frames_for_env_damage(self.common_data, max_env_damage);
            self.enter_source_hitlag_recursive(attacker_index, hitlag_frames);
        }
    }

    fn enter_source_hitlag_recursive(&mut self, player_index: usize, hitlag_frames: u8) {
        if player_index >= self.players.len() || hitlag_frames == 0 {
            return;
        }
        if hitlag_frames > self.players[player_index].hitlag_frames {
            self.players[player_index].hitlag_frames = hitlag_frames;
        }
        let mut visited = [false; PLAYER_COUNT];
        self.enter_source_x2219_b5_recursive(player_index, &mut visited);
    }

    fn enter_source_x2219_b5_recursive(
        &mut self,
        player_index: usize,
        visited: &mut [bool; PLAYER_COUNT],
    ) {
        if player_index >= self.players.len() || visited[player_index] {
            return;
        }
        visited[player_index] = true;
        self.players[player_index].source_x2219_b5 = true;
        let Some(linked_index) = self.players[player_index]
            .source_x1a5c_index
            .map(usize::from)
        else {
            return;
        };
        self.enter_source_x2219_b5_recursive(linked_index, visited);
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
        mut action_total_frames: impl FnMut(MeleeActionStateId) -> Option<u8>,
    ) -> usize {
        let mut applied = 0;
        for result in results {
            let Some(attacker) = self.players.get(result.stage.attacker_index).copied() else {
                continue;
            };
            let Some(victim) = self.players.get(result.stage.victim_index).copied() else {
                continue;
            };
            let damage_facing_dir =
                source_damage_facing_dir(attacker.source_position.x, victim.source_position.x);
            if self.apply_source_damage_result_with_facing(
                *result,
                damage_facing_dir,
                None,
                None,
                true,
                &mut action_total_frames,
            ) {
                applied += 1;
            }
        }
        applied
    }

    pub(crate) fn apply_source_throw_release_damage_with_action_total_frames(
        &mut self,
        thrower_index: usize,
        victim_index: usize,
        throw_hitbox: SourceInstalledThrowHitbox,
        source_release_transn2_position: Option<SourceVec2>,
        source_release_last_pos: Option<SourceVec2>,
        victim_input: PlayerInput,
        mut action_total_frames: impl FnMut(MeleeActionStateId) -> Option<u8>,
    ) -> bool {
        if thrower_index == victim_index {
            return false;
        }
        let Some(thrower) = self.players.get(thrower_index).copied() else {
            return false;
        };
        let Some(victim) = self.players.get(victim_index).copied() else {
            return false;
        };
        let hitbox = source_hitbox_attributes_from_throw_hitbox(throw_hitbox);
        let damage = throw_hitbox.damage;
        let stage = SourceDamageStage {
            attacker_index: thrower_index,
            victim_index,
            hitbox_id: u64::from(throw_hitbox.hitbox.hitbox_idx),
            hurtbox_id: 0,
            action_state_id: thrower.melee_action_state_id,
            source_action_key: thrower.source_action_key,
            source_frame: None,
            damaged_hurt_height: 1,
            damage,
            env_damage: source_env_damage(damage),
            unk_count: throw_hitbox.unk_count,
            hitbox,
        };
        self.update_source_stale_moves_from_fighter(thrower_index, victim_index);
        let accumulator = source_damage_accumulator_after_stages(
            SourceDamageAccumulator {
                victim_index,
                percent_temp: victim.damage_percent_temp,
                applied_damage: victim.damage_applied,
            },
            &[stage],
        );
        let Some(result) = source_damage_result_for_victim(
            self.common_data,
            &[stage],
            SourceDamageResultInput {
                victim_index,
                victim_percent: victim.damage_percent,
                victim_percent_temp: accumulator.percent_temp,
                victim_weight: self.common_data.throw_knockback_weight,
                stage: 1.0,
                attack: 1.0,
                defense: 1.0,
            },
        ) else {
            return false;
        };

        if let Some(victim) = self.players.get_mut(victim_index) {
            if let Ok(thrower_u8) = u8::try_from(thrower_index) {
                victim.source_thrown_hitbox_owner_index = Some(thrower_u8);
                victim.source_thrown_hitbox_team_unk = 0;
                victim.source_thrown_hitbox_grabber_player_id = Some(thrower_u8);
            }
        }

        if let Some(thrower) = self.players.get_mut(thrower_index) {
            thrower.source_x1a5c_index = None;
            thrower.source_victim_index = None;
            thrower.source_x221b_b5 = false;
        }
        if let Some(victim) = self.players.get_mut(victim_index) {
            victim.source_x1a5c_index = None;
            victim.source_victim_index = None;
            apply_source_throw_release_position_handoff(
                victim,
                source_release_transn2_position,
                source_release_last_pos,
            );
            victim.source_x2226_b2 = false;
        }

        self.apply_source_damage_stages(&[stage]);
        let thrower_facing = if thrower.facing < 0 { -1.0 } else { 1.0 };
        // ftCo_800DD724 hands thrown victims to ftCo_800DE7C0. That helper
        // forces motion 90 only for ThrowLw (motion_id 222), then applies DI.
        let forced_action_state_id = (thrower.melee_action_state_id
            == Some(MeleeActionStateId::new(222)))
        .then_some(MeleeActionStateId::new(90));
        let damage_facing_dir = -thrower_facing;
        let final_facing_override = (throw_hitbox.hitbox.angle > 90
            && throw_hitbox.hitbox.angle < 270)
            .then_some(-damage_facing_dir);
        let applied = self.apply_source_damage_result_with_facing(
            result,
            damage_facing_dir,
            final_facing_override,
            forced_action_state_id,
            false,
            &mut action_total_frames,
        );
        if applied {
            if let Some(victim) = self.players.get_mut(victim_index) {
                apply_source_damage_immediate_di(victim, victim_input, self.common_data);
            }
            self.commit_staged_source_damage();
        }
        applied
    }

    fn apply_source_damage_result_with_facing(
        &mut self,
        result: SourceDamageResult,
        damage_facing_dir: f32,
        final_facing_override: Option<f32>,
        forced_action_state_id: Option<MeleeActionStateId>,
        apply_collision_hitlag: bool,
        action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
    ) -> bool {
        let victim_hitlag_frames = {
            let Some(victim) = self.players.get_mut(result.stage.victim_index) else {
                return false;
            };
            let victim_grounded_before_damage = victim.grounded;
            let angle = source_damage_angle_radians(
                self.common_data,
                result,
                victim_grounded_before_damage,
            );
            let speed = result.knockback * self.common_data.damage_knockback_velocity_scale;
            let damage_vector = SourceVec2 {
                x: -speed * angle.cos() * damage_facing_dir,
                y: speed * angle.sin(),
            };
            let (velocity_x, velocity_y, ground_knockback_velocity, grounded_after_damage) =
                source_damage_entry_velocity(
                    self.stage,
                    self.common_data,
                    victim,
                    result,
                    damage_vector,
                    victim_grounded_before_damage,
                );
            let damage_action_state_id = forced_action_state_id.unwrap_or_else(|| {
                source_damage_action_state_id(
                    self.common_data,
                    result,
                    angle,
                    victim_grounded_before_damage,
                    grounded_after_damage,
                )
            });

            victim.source_self_velocity_x = 0.0;
            victim.source_self_velocity_y = 0.0;
            victim.source_knockback_velocity_x = velocity_x;
            victim.source_knockback_velocity_y = velocity_y;
            victim.source_ground_knockback_velocity = ground_knockback_velocity;
            victim.velocity.x = source_units_to_milli(
                victim.source_self_velocity_x + victim.source_knockback_velocity_x,
            );
            victim.velocity.y = source_units_to_milli(
                victim.source_self_velocity_y + victim.source_knockback_velocity_y,
            );
            victim.ground_velocity_x = 0.0;
            victim.ground_accel_x = 0.0;
            victim.ground_accel_x2 = 0.0;
            victim.grounded = grounded_after_damage;
            if victim_grounded_before_damage
                && !grounded_after_damage
                && (87..=91).contains(&damage_action_state_id.get())
            {
                // All grounded launches call ftCommon_8007D5D4. Standard damage
                // still needs its locked-bottom ASDI floor behavior before the
                // lock can be enabled there without false landings.
                source_ft_common_8007d5d4_damage_ground_to_air(victim);
            }
            victim.damage_knockback = result.knockback;
            victim.damage_angle = result.angle;
            victim.damage_element = result.element;
            victim.facing =
                source_damage_facing_i8(final_facing_override.unwrap_or(damage_facing_dir));
            victim.source_model_facing = victim.facing;
            victim.source_motion_entry_facing = victim.facing;
            victim.hitlag_frames = if apply_collision_hitlag {
                source_damage_hitlag_frames(self.common_data, result)
            } else {
                0
            };
            victim.source_allow_sdi = victim.hitlag_frames != 0;
            victim.damage_hitstun_frames = source_damage_hitstun_frames(self.common_data, result);
            victim.clear_source_guard_shield_object();
            victim.melee_action_state_id = Some(damage_action_state_id);
            victim.source_action_key =
                canonical_source_action_binding_for_runtime_id(damage_action_state_id)
                    .map(|binding| binding.source_action_key);
            victim.source_action_total_frames =
                action_total_frames(damage_action_state_id).unwrap_or(0);
            victim.source_down_bound_pose = None;
            victim.source_down_wait_timer = 0.0;
            victim.motion_state_alias = None;
            victim.motion_frame = 0;
            victim.set_source_motion_anim_frame(0.0);
            victim.set_source_motion_anim_rate_milli(1_000);
            if victim.install_current_source_primary_anim(0.0, 1.0) {
                let _ = victim.interpret_source_primary_anim();
                let _ = victim.interpret_source_primary_anim();
            }
            victim.source_retained_model_pose = None;
            if let Some(timers) = self.input_timers.get_mut(result.stage.victim_index) {
                timers.x_tap = EXPIRED_INPUT_TIMER;
                timers.y_tap = EXPIRED_INPUT_TIMER;
            }
            victim.hitlag_frames
        };
        if victim_hitlag_frames > 0 {
            self.enter_source_hitlag_recursive(result.stage.victim_index, victim_hitlag_frames);
        }
        true
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
            stage: self.stage,
            common_data: self.common_data,
            match_phase: self.match_phase,
            match_phase_timer: self.match_phase_timer,
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
        mix_u32(&mut hash, self.hsd_rng_seed);
        mix_u8(
            &mut hash,
            self.engine_features.shield_turnaround_during_guard as u8,
        );
        mix_u8(&mut hash, self.match_flow_enabled as u8);
        mix_u8(&mut hash, self.match_phase.checksum_id());
        mix_u32(&mut hash, u32::from(self.match_phase_timer));
        mix_stage_profile(&mut hash, self.stage);
        mix_common_data(&mut hash, self.common_data);
        mix_u32(&mut hash, u32::from(self.source_stale_attack_instance));
        for player in self.players {
            mix_fighter_profile(&mut hash, player.profile);
            mix_u8(&mut hash, player.costume_index);
            mix_u8(&mut hash, player.player_state);
            mix_u8(&mut hash, player.stocks as u8);
            mix_i32(&mut hash, player.position.x);
            mix_i32(&mut hash, player.position.y);
            mix_f32(&mut hash, player.source_position.x);
            mix_f32(&mut hash, player.source_position.y);
            mix_f32(&mut hash, player.source_position_z);
            mix_source_vec2(&mut hash, player.source_previous_position);
            mix_f32(&mut hash, player.source_previous_position_z);
            mix_source_vec2(&mut hash, player.source_coll_last_pos);
            mix_source_vec2(&mut hash, player.source_coll_cur_pos);
            mix_source_vec2(&mut hash, player.source_coll_prev_pos);
            mix_source_vec2(&mut hash, player.source_coll_x28_vec);
            mix_i32(&mut hash, player.velocity.x);
            mix_i32(&mut hash, player.velocity.y);
            mix_f32(&mut hash, player.source_self_velocity_x);
            mix_f32(&mut hash, player.source_self_velocity_y);
            mix_f32(&mut hash, player.source_knockback_velocity_x);
            mix_f32(&mut hash, player.source_knockback_velocity_y);
            mix_f32(&mut hash, player.source_ground_knockback_velocity);
            mix_f32(&mut hash, player.source_attacker_shield_velocity_x);
            mix_f32(&mut hash, player.player_nudge_x);
            mix_f32(&mut hash, player.player_nudge_z);
            mix_i32(&mut hash, player.ecb_bottom_offset_y);
            mix_u8(&mut hash, player.ecb_bottom_lock_timer);
            mix_source_fighter_ecb(&mut hash, player.source_coll_ecb);
            mix_source_fighter_ecb(&mut hash, player.source_coll_prev_ecb);
            mix_source_fighter_ecb(&mut hash, player.source_coll_desired_ecb);
            mix_source_fighter_ecb(&mut hash, player.source_coll_xe4_ecb);
            mix_source_fighter_ecb(&mut hash, player.source_coll_x64_ecb);
            mix_u8(&mut hash, player.source_coll_facing_dir as u8);
            mix_u8(&mut hash, player.source_coll_x34_b5 as u8);
            mix_u8(&mut hash, player.source_coll_x34_b6 as u8);
            mix_u8(&mut hash, player.source_coll_x130_clear as u8);
            mix_u8(&mut hash, player.source_coll_x130_locked as u8);
            mix_optional_u8(&mut hash, player.source_coll_floor_surface_index);
            mix_optional_u16(&mut hash, player.source_coll_floor_line_index);
            mix_optional_u16(&mut hash, player.source_coll_floor_skip_line_index);
            mix_optional_u16(&mut hash, player.source_coll_ledge_id_left);
            mix_optional_u16(&mut hash, player.source_coll_ledge_id_right);
            mix_u32(&mut hash, player.source_coll_env_flags);
            mix_u32(&mut hash, player.source_coll_prev_env_flags);
            mix_u8(&mut hash, player.jumps_remaining);
            mix_u8(&mut hash, player.grounded as u8);
            mix_u8(&mut hash, player.fast_falling as u8);
            mix_u8(&mut hash, player.facing as u8);
            mix_u8(&mut hash, player.source_model_facing as u8);
            mix_u8(&mut hash, player.attack_frame);
            mix_f32(&mut hash, player.damage_percent);
            mix_f32(&mut hash, player.damage_percent_temp);
            mix_u32(&mut hash, player.damage_applied as u32);
            mix_f32(&mut hash, player.damage_knockback);
            mix_u32(&mut hash, player.damage_angle as u32);
            mix_u8(&mut hash, player.damage_element);
            mix_u8(&mut hash, player.hitlag_frames);
            mix_u8(&mut hash, player.source_allow_sdi as u8);
            mix_u8(&mut hash, player.source_x2219_b5 as u8);
            mix_u32(&mut hash, player.damage_hitstun_frames as u32);
            mix_u8(&mut hash, player.source_attack_id);
            mix_u32(&mut hash, u32::from(player.source_attack_instance));
            mix_source_stale_move_table(&mut hash, player.source_stale_move_table);
            mix_u8(&mut hash, motion_state_id(player.motion_state));
            mix_f32(&mut hash, player.source_fall_anim_blend);
            mix_u8(&mut hash, motion_state_id(player.source_fall_anim_pose));
            mix_optional_action_state_id(&mut hash, player.melee_action_state_id);
            mix_optional_source_action_key(&mut hash, player.source_action_key);
            mix_u8(&mut hash, player.source_action_total_frames);
            mix_optional_source_retained_model_pose(&mut hash, player.source_retained_model_pose);
            mix_optional_source_down_bound_pose(&mut hash, player.source_down_bound_pose);
            mix_u8(&mut hash, player.source_force_damage_down_bound as u8);
            mix_u8(&mut hash, player.source_down_bound_use_z_axis as u8);
            mix_u8(&mut hash, player.source_down_bound_reverse_face_up as u8);
            mix_f32(&mut hash, player.source_down_wait_timer);
            mix_u8(&mut hash, player.source_lr_digital_press_timer);
            mix_u8(&mut hash, player.source_lcancel_timer);
            mix_u8(&mut hash, player.source_previous_lr_digital_press_timer);
            mix_u8(&mut hash, player.source_jab_followup_timer);
            mix_u8(&mut hash, player.source_jab_followup_queued as u8);
            mix_u8(&mut hash, player.source_jab_combo_enabled as u8);
            mix_u8(&mut hash, player.source_jab_rapid_enabled as u8);
            mix_u8(&mut hash, player.source_rapid_jab_input_count);
            mix_u8(&mut hash, player.source_attack100_loop_has_started as u8);
            mix_u8(&mut hash, player.source_attack100_loop_continue_input as u8);
            mix_u8(&mut hash, player.source_common_timer);
            mix_u8(&mut hash, player.source_dead_phase);
            mix_f32(&mut hash, player.source_rebirth_target_x);
            mix_f32(&mut hash, player.source_rebirth_target_y);
            mix_u32(&mut hash, u32::from(player.source_rebirth_platform_index));
            mix_u32(
                &mut hash,
                u32::from(player.source_rebirth_stage_point_index),
            );
            mix_u8(&mut hash, player.source_collision_state);
            mix_u8(&mut hash, player.source_hurt_collision_state);
            mix_u32(
                &mut hash,
                u32::from(player.source_hurt_collision_lockout_timer),
            );
            mix_u32(&mut hash, player.source_hit_intangible_timer as u32);
            mix_u32(&mut hash, player.source_hurt_intangible_timer as u32);
            mix_optional_motion_state(&mut hash, player.motion_state_alias);
            mix_u8(&mut hash, player.source_motion_entry_facing as u8);
            mix_u8(&mut hash, player.motion_frame);
            mix_f32(&mut hash, player.source_motion_anim_frame);
            mix_i32(&mut hash, player.motion_anim_frame_milli);
            mix_source_fighter_playback(&mut hash, player.source_playback);
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
            mix_u8(&mut hash, player.source_allow_interrupt as u8);
            mix_u32(&mut hash, player.motion_cmd_var0);
            mix_u32(&mut hash, player.motion_cmd_var1);
            mix_u8(&mut hash, player.motion_throw_flags);
            mix_f32(&mut hash, player.source_grab_timer);
            mix_u8(&mut hash, player.source_grab_mash_x as u8);
            mix_u8(&mut hash, player.source_grab_mash_y as u8);
            mix_f32(&mut hash, player.source_capture_wait_timer);
            mix_f32(&mut hash, player.source_capture_wait_anim_timer);
            mix_u8(&mut hash, player.source_capture_wait_mashed as u8);
            mix_u8(&mut hash, player.source_capture_wait_jump_queued as u8);
            mix_u8(&mut hash, player.source_throw_x4 as u8);
            for hitbox in player.source_throw_hitboxes {
                mix_optional_source_installed_throw_hitbox(&mut hash, hitbox);
            }
            mix_u8(&mut hash, player.source_thrown_unk_bool as u8);
            mix_f32(&mut hash, player.source_thrown_anim_timer);
            mix_optional_u8(&mut hash, player.source_thrown_hitbox_owner_index);
            mix_u8(&mut hash, player.source_thrown_hitbox_team_unk);
            mix_optional_u8(&mut hash, player.source_thrown_hitbox_grabber_player_id);
            mix_optional_u8(&mut hash, player.source_victim_index);
            mix_optional_u8(&mut hash, player.source_x1a5c_index);
            mix_u8(&mut hash, player.source_x221b_b5 as u8);
            mix_u8(&mut hash, player.source_x2222_b3 as u8);
            mix_u8(&mut hash, player.source_x2226_b2 as u8);
            mix_source_vec3(&mut hash, player.source_x1a70);
            mix_f32(&mut hash, player.source_x34_scale_y);
            mix_u32(&mut hash, u32::from(player.captain_special_hi_x0));
            mix_f32(&mut hash, player.captain_special_hi_vel_x);
            mix_f32(&mut hash, player.captain_special_hi_vel_y);
            mix_u8(&mut hash, player.captain_special_hi_x2_b0 as u8);
            mix_u8(&mut hash, player.captain_special_hi_x2_b1 as u8);
            mix_u8(&mut hash, player.landing_lag_ticks);
            mix_u8(&mut hash, player.run_brake_x0 as u8);
            mix_u8(&mut hash, player.run_brake_frames_remaining);
            mix_u8(&mut hash, player.turn_run_x14 as u8);
            mix_u8(&mut hash, player.turn_run_resume_advances as u8);
            mix_u8(&mut hash, player.turn_run_completion_pending as u8);
            mix_u8(&mut hash, player.turn_run_completion_enters_run as u8);
            mix_i32(&mut hash, player.motion_anim_rate_milli);
            mix_f32(&mut hash, player.source_motion_anim_rate);
            mix_f32(&mut hash, player.shield_health);
            mix_f32(&mut hash, player.lightshield_amount);
            mix_f32(&mut hash, player.source_collision_lightshield_amount);
            mix_u8(&mut hash, player.shield_release_lockout_frames);
            mix_u8(&mut hash, player.source_shield_collision_active as u8);
            mix_u8(&mut hash, player.source_shield_hit_active as u8);
            mix_u8(&mut hash, player.source_shield_hit_update_pos as u8);
            mix_f32(&mut hash, player.source_shield_hit_position.x);
            mix_f32(&mut hash, player.source_shield_hit_position.y);
            mix_f32(&mut hash, player.source_shield_hit_position.z);
            mix_f32(&mut hash, player.source_shield_aim_angle_degrees);
            mix_f32(&mut hash, player.source_shield_aim_magnitude);
            mix_u8(&mut hash, player.source_x221c_b1 as u8);
            mix_u8(&mut hash, player.source_x221c_b2 as u8);
            mix_u8(&mut hash, player.source_x221c_b3 as u8);
            mix_f32(&mut hash, player.source_guard_reflect_timer);
            mix_f32(&mut hash, player.source_guard_reflect_damage_skip_timer);
            mix_u8(&mut hash, player.shield_turn_facing_after as u8);
            mix_u8(&mut hash, player.shield_turn_frame);
            mix_u8(&mut hash, player.guard_catch_dash_window);
            mix_u8(&mut hash, player.guard_release_latched as u8);
            mix_u8(&mut hash, jump_input_id(player.jump_input));
            mix_u8(&mut hash, player.short_hop as u8);
            mix_u8(&mut hash, player.escape_air_iasa_timer);
            mix_u8(&mut hash, player.floor_skip_surface.unwrap_or(u8::MAX));
            mix_u8(&mut hash, player.platform_pass_pending as u8);
            mix_u8(&mut hash, player.platform_pass_timer);
            mix_u32(
                &mut hash,
                player
                    .source_cliff_ledge_id
                    .map(u32::from)
                    .unwrap_or(u32::MAX),
            );
            mix_u8(&mut hash, player.source_cliff_stick_gate as u8);
            mix_u32(&mut hash, u32::from(player.source_cliff_wait_timer));
            mix_u32(&mut hash, u32::from(player.source_ledge_cooldown_timer));
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
        for facts in self.last_input_facts {
            mix_melee_input_facts(&mut hash, facts);
        }
        mix_u32(&mut hash, self.source_hit_victim_log.len() as u32);
        for entry in self.source_hit_victim_log.iter() {
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

fn apply_source_throw_release_position_handoff(
    victim: &mut PlayerState,
    source_release_transn2_position: Option<SourceVec2>,
    source_release_last_pos: Option<SourceVec2>,
) {
    if !victim.source_x2226_b2 {
        return;
    }
    let Some(transn2_position) = source_release_transn2_position else {
        return;
    };
    let facing = if victim.facing < 0 { -1.0 } else { 1.0 };
    let release_position = SourceVec2 {
        x: transn2_position.x + facing * (victim.source_x1a70.z * victim.source_x34_scale_y),
        y: transn2_position.y + victim.source_x1a70.y * victim.source_x34_scale_y,
    };
    victim.source_position = release_position;
    victim.position = release_position.to_milli();
    victim.source_coll_cur_pos = release_position;
    if let Some(last_pos) = source_release_last_pos {
        victim.source_coll_last_pos = last_pos;
    }
}

fn players_two_mut(
    players: &mut [PlayerState; PLAYER_COUNT],
    first: usize,
    second: usize,
) -> Option<(&mut PlayerState, &mut PlayerState)> {
    if first == second || first >= PLAYER_COUNT || second >= PLAYER_COUNT {
        return None;
    }
    if first < second {
        let (left, right) = players.split_at_mut(second);
        Some((&mut left[first], &mut right[0]))
    } else {
        let (left, right) = players.split_at_mut(first);
        Some((&mut right[0], &mut left[second]))
    }
}

fn source_grab_pull_action_state(
    action_state_id: MeleeActionStateId,
) -> Option<(MeleeActionStateId, SourceActionKey)> {
    match action_state_id.get() {
        212 => Some((MeleeActionStateId::new(213), SourceActionKey::new("Catch"))),
        214 => Some((
            MeleeActionStateId::new(215),
            SourceActionKey::new("CatchDash"),
        )),
        _ => None,
    }
}

fn source_throw_action_state_id(action_state_id: MeleeActionStateId) -> bool {
    matches!(action_state_id.get(), 219..=222)
}

fn source_held_victim_damage_action_state_id(action_state_id: MeleeActionStateId) -> bool {
    source_throw_action_state_id(action_state_id) || action_state_id.get() == 217
}

fn source_held_victim_action_state_id(action_state_id: MeleeActionStateId) -> bool {
    matches!(
        action_state_id.get(),
        223 | 224 | 226 | 227 | 230 | 239..=242 | 275
    )
}

fn enter_source_only_action_state(
    player: &mut PlayerState,
    action_state_id: MeleeActionStateId,
    source_action_key: SourceActionKey,
    action_total_frames: &mut impl FnMut(MeleeActionStateId) -> Option<u8>,
) {
    player.clear_source_guard_shield_object();
    player.melee_action_state_id = Some(action_state_id);
    player.source_action_key = Some(source_action_key);
    player.source_action_total_frames = action_total_frames(action_state_id).unwrap_or(0);
    player.source_down_bound_pose = None;
    player.source_down_wait_timer = 0.0;
    player.motion_state_alias = None;
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.set_source_motion_anim_rate_milli(1_000);
    clear_source_thrown_hitbox_owner_state(player);
    clear_source_throw_control_state(player);
    player.hitlag_frames = 0;
    player.source_allow_sdi = false;
    player.source_x2219_b5 = false;
    player.damage_hitstun_frames = 0;
    player.source_x2222_b3 = false;
    player.source_x2226_b2 = false;
}

fn clear_source_thrown_hitbox_owner_state(player: &mut PlayerState) {
    player.source_thrown_hitbox_owner_index = None;
    player.source_thrown_hitbox_team_unk = 0;
    player.source_thrown_hitbox_grabber_player_id = None;
}

fn clear_source_throw_control_state(player: &mut PlayerState) {
    player.source_throw_x4 = false;
    player.source_thrown_unk_bool = false;
    player.source_thrown_anim_timer = 0.0;
}

fn clear_source_grab_ground_velocity(player: &mut PlayerState) {
    player.ground_velocity_x = 0.0;
    player.source_self_velocity_x = 0.0;
    player.velocity.x = 0;
    player.ground_accel_x = 0.0;
    player.ground_accel_x2 = 0.0;
    player.dash_entry_velocity_delta = 0.0;
}

fn source_profile_is_purin(profile: FighterProfile) -> bool {
    matches!(profile.reference_character, "purin" | "jigglypuff")
}

fn source_player_can_enter_guard_setoff(player: PlayerState) -> bool {
    player.source_shield_collision_active
        && player
            .melee_action_state_id
            .is_some_and(|action_state_id| matches!(action_state_id.get(), 178 | 179 | 181 | 182))
}

fn source_guard_setoff_frames(
    common_data: MeleeCommonData,
    env_damage: u16,
    lightshield_amount: f32,
) -> f32 {
    let lightshield_scale = lightshield_amount.clamp(0.0, 1.0)
        * (common_data.shield_setoff_lightshield_max - common_data.shield_setoff_lightshield_min)
        + common_data.shield_setoff_lightshield_min;
    common_data.shield_setoff_duration_damage_scale
        * (env_damage as f32 * (1.0 - lightshield_scale))
        + common_data.shield_setoff_duration_base
}

fn source_guard_setoff_pushback(
    common_data: MeleeCommonData,
    env_damage: u16,
    lightshield_amount: f32,
) -> f32 {
    (source_guard_setoff_frames(common_data, env_damage, lightshield_amount)
        * common_data.shield_setoff_pushback_scale
        * common_data.shield_setoff_nonreflect_pushback_multiplier)
        .min(common_data.shield_setoff_pushback_cap)
}

pub(crate) fn enter_source_shield_break_fly(
    player: &mut PlayerState,
    common_data: MeleeCommonData,
) {
    player.shield_health = common_data.shield_break_reset_health;
    player.clear_source_guard_shield_object();
    player.set_motion_state_alias(MotionState::ShieldBreakFly);
    player.motion_frame = 0;
    player.set_source_motion_anim_frame(0.0);
    player.set_source_motion_anim_rate_milli(1_000);
    player.grounded = false;
    player.source_self_velocity_x = 0.0;
    player.source_self_velocity_y = player.profile.shield_break_initial_velocity;
    if source_profile_is_purin(player.profile) {
        player.source_x2222_b3 = true;
    }
    player.source_knockback_velocity_x = 0.0;
    player.source_knockback_velocity_y = 0.0;
    player.source_ground_knockback_velocity = 0.0;
    player.velocity = Vec2 {
        x: 0,
        y: source_units_to_milli(player.source_self_velocity_y),
    };
    player.ground_velocity_x = 0.0;
    player.ground_accel_x = 0.0;
    player.ground_accel_x2 = 0.0;
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

fn source_damage_facing_dir(attacker_source_x: f32, victim_source_x: f32) -> f32 {
    if victim_source_x > attacker_source_x {
        -1.0
    } else {
        1.0
    }
}

fn source_damage_facing_i8(facing_dir: f32) -> i8 {
    if facing_dir < 0.0 {
        -1
    } else {
        1
    }
}

fn source_hitbox_attributes_from_throw_hitbox(
    hitbox: SourceInstalledThrowHitbox,
) -> SourceHitboxAttributes {
    let raw = hitbox.hitbox;
    SourceHitboxAttributes {
        bone: 0,
        hit_group: raw.hitbox_idx,
        damage: hitbox.unk_count,
        angle: raw.angle,
        knockback_growth: raw.hit_x24,
        weight_set_knockback: raw.hit_x28,
        base_knockback: raw.hit_x2c,
        element: raw.element,
        shield_damage: 0,
        hit_grounded: true,
        hit_aerial: true,
    }
}

fn apply_source_damage_immediate_di(
    player: &mut PlayerState,
    input: PlayerInput,
    common_data: MeleeCommonData,
) {
    let stick_x = fighter_stick_axis_to_f32(input.stick_x());
    let stick_y = fighter_stick_axis_to_f32(input.stick_y());
    if stick_x == 0.0 && stick_y == 0.0 {
        return;
    }

    let kb_x = player.source_knockback_velocity_x;
    let kb_y = player.source_knockback_velocity_y;
    let kb_vel_x_neg = -kb_x;
    let kb_mag_sq = kb_vel_x_neg * kb_vel_x_neg + kb_y * kb_y;
    if kb_mag_sq < 0.00001 {
        return;
    }

    let f3 = kb_y * stick_x + kb_vel_x_neg * stick_y;
    let mut f30 = f3 * f3 / kb_mag_sq;
    let cross_z = kb_x * stick_y - kb_y * stick_x;
    if cross_z < 0.0 {
        f30 = -f30;
    }
    let kb_mag = (kb_x * kb_x + kb_y * kb_y).sqrt();
    let angle = kb_y.atan2(kb_x) + common_data.di_angle_degrees.to_radians() * f30;
    player.source_knockback_velocity_x = kb_mag * angle.cos();
    player.source_knockback_velocity_y = kb_mag * angle.sin();
    source_sync_damage_velocity_projection(player);
}

fn source_sync_damage_velocity_projection(player: &mut PlayerState) {
    player.velocity.x =
        source_units_to_milli(player.source_self_velocity_x + player.source_knockback_velocity_x);
    player.velocity.y =
        source_units_to_milli(player.source_self_velocity_y + player.source_knockback_velocity_y);
}

fn source_ft_common_8007d5d4_damage_ground_to_air(player: &mut PlayerState) {
    player.grounded = false;
    player.ground_velocity_x = 0.0;
    player.ground_accel_x = 0.0;
    player.ground_accel_x2 = 0.0;
    player.jumps_remaining = player.profile.max_jumps.saturating_sub(1);
    player.ecb_bottom_offset_y = 0;
    player.ecb_bottom_lock_timer = 10;
    player.source_coll_x130_locked = true;
}

fn source_damage_entry_velocity(
    stage: StageProfile,
    common_data: MeleeCommonData,
    victim: &PlayerState,
    result: SourceDamageResult,
    damage_vector: SourceVec2,
    victim_grounded_before_damage: bool,
) -> (f32, f32, f32, bool) {
    if !victim_grounded_before_damage {
        let knockback_scale = if source_damage_check_air_motion(victim, common_data) {
            common_data.damage_air_v_cancel_knockback_scale
        } else {
            1.0
        };
        return (
            damage_vector.x * knockback_scale,
            damage_vector.y * knockback_scale,
            0.0,
            false,
        );
    }

    let floor_normal = source_ground_normal_for_damage_entry(stage, victim);
    let normal_angle = source_vec2_angle(floor_normal, damage_vector);
    if normal_angle < std::f32::consts::FRAC_PI_2 {
        return (damage_vector.x, damage_vector.y, 0.0, false);
    }

    let tier = source_damage_motion_tier(common_data, result.knockback);
    if tier == 3 {
        let velocity_y = if normal_angle
            > std::f32::consts::FRAC_PI_2 + common_data.damage_floor_reflect_angle_radians
        {
            -damage_vector.y * common_data.damage_floor_reflect_velocity_scale
        } else {
            damage_vector.y
        };
        return (damage_vector.x, velocity_y, 0.0, false);
    }

    let ground_knockback_velocity = damage_vector.x;
    (
        floor_normal.y * ground_knockback_velocity,
        -floor_normal.x * ground_knockback_velocity,
        ground_knockback_velocity,
        true,
    )
}

fn source_damage_check_air_motion(victim: &PlayerState, common_data: MeleeCommonData) -> bool {
    matches!(
        victim.motion_state,
        MotionState::JumpF
            | MotionState::JumpB
            | MotionState::JumpAerialF
            | MotionState::JumpAerialB
            | MotionState::Fall
            | MotionState::FallF
            | MotionState::FallB
            | MotionState::FallAerial
            | MotionState::FallAerialF
            | MotionState::FallAerialB
            | MotionState::FallSpecial
            | MotionState::FallSpecialF
            | MotionState::FallSpecialB
            | MotionState::DamageFall
            | MotionState::EscapeAir
    ) && victim.source_lr_digital_press_timer <= common_data.damage_air_v_cancel_window
        && victim.source_previous_lr_digital_press_timer >= common_data.passive_input_age_threshold
}

fn source_ground_normal_for_damage_entry(stage: StageProfile, player: &PlayerState) -> SourceVec2 {
    if let Some(melee_stage) = stage.melee_stage_profile() {
        if let Some(line_id) = player.source_coll_floor_line_index.map(usize::from) {
            if matches!(
                source_collision_line_kind(melee_stage.collision, line_id),
                Some(StageCollisionLineKind::Floor | StageCollisionLineKind::SoftFloor)
            ) {
                if let Some(normal) = source_collision_line_normal(melee_stage.collision, line_id) {
                    return source_floor_normal_oriented_up(normal);
                }
            }
        }
    }
    SourceVec2 { x: 0.0, y: 1.0 }
}

fn source_collision_line_kind(
    collision: StageCollisionProfile,
    line_id: usize,
) -> Option<StageCollisionLineKind> {
    collision.lines.get(line_id).map(|line| line.kind)
}

fn source_collision_line_normal(
    collision: StageCollisionProfile,
    line_id: usize,
) -> Option<SourceVec2> {
    let line = collision.scaled_line(line_id)?;
    let x = -(line.y1 - line.y0);
    let y = line.x1 - line.x0;
    let len = (x * x + y * y).sqrt();
    (len > f32::EPSILON).then_some(SourceVec2 {
        x: x / len,
        y: y / len,
    })
}

fn source_floor_normal_oriented_up(normal: SourceVec2) -> SourceVec2 {
    if normal.y < 0.0 {
        SourceVec2 {
            x: -normal.x,
            y: -normal.y,
        }
    } else {
        normal
    }
}

fn source_vec2_angle(a: SourceVec2, b: SourceVec2) -> f32 {
    let a_len = (a.x * a.x + a.y * a.y).sqrt();
    let b_len = (b.x * b.x + b.y * b.y).sqrt();
    if a_len <= f32::EPSILON || b_len <= f32::EPSILON {
        return 0.0;
    }
    ((a.x * b.x + a.y * b.y) / (a_len * b_len))
        .clamp(-1.0, 1.0)
        .acos()
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

fn source_damage_action_state_id(
    common_data: MeleeCommonData,
    result: SourceDamageResult,
    damage_angle_radians: f32,
    victim_grounded_before_damage: bool,
    victim_grounded_after_damage: bool,
) -> MeleeActionStateId {
    const GROUND_DAMAGE_STATES: [[u16; 3]; 4] =
        [[81, 78, 75], [82, 79, 76], [83, 80, 77], [89, 88, 87]];
    const AIR_DAMAGE_STATES: [[u16; 3]; 4] =
        [[84, 84, 84], [85, 85, 85], [86, 86, 86], [89, 88, 87]];

    let tier = source_damage_motion_tier(common_data, result.knockback);
    debug_assert!(
        result.stage.damaged_hurt_height <= 2,
        "source hurt height must be 0=low, 1=mid, or 2=high"
    );
    let hurt_height = usize::from(result.stage.damaged_hurt_height);
    let table = if victim_grounded_before_damage {
        GROUND_DAMAGE_STATES
    } else {
        AIR_DAMAGE_STATES
    };
    if !victim_grounded_after_damage && tier == 3 {
        if damage_angle_radians > common_data.damage_fly_top_angle_min_radians
            && damage_angle_radians < common_data.damage_fly_top_angle_max_radians
        {
            return MeleeActionStateId::new(90);
        }
    }
    MeleeActionStateId::new(table[tier][hurt_height])
}

fn source_damage_hitlag_frames(common_data: MeleeCommonData, result: SourceDamageResult) -> u8 {
    let multiplier = if result.element == 2 {
        common_data.hitlag_electric_multiplier
    } else {
        1.0
    };
    source_hitlag_frames_for_env_damage_and_multiplier(
        common_data,
        result.stage.env_damage,
        multiplier,
    )
}

fn source_hitlag_frames_for_env_damage(common_data: MeleeCommonData, env_damage: u16) -> u8 {
    source_hitlag_frames_for_env_damage_and_multiplier(common_data, env_damage, 1.0)
}

fn source_hitlag_frames_for_env_damage_and_multiplier(
    common_data: MeleeCommonData,
    env_damage: u16,
    multiplier: f32,
) -> u8 {
    let tmp = (env_damage as f32 * common_data.hitlag_damage_scale + common_data.hitlag_base_frames)
        as i32;
    let raw = (tmp as f32 * multiplier) as i32;
    raw.clamp(0, common_data.hitlag_max_frames as i32) as u8
}

fn source_damage_hitstun_frames(common_data: MeleeCommonData, result: SourceDamageResult) -> u16 {
    let raw = (result.knockback * common_data.damage_duration_scale) as i32;
    raw.max(1).min(u16::MAX as i32) as u16
}

const fn motion_state_id(state: MotionState) -> u8 {
    match state {
        MotionState::Wait => 0,
        MotionState::DeadDown => 73,
        MotionState::DeadLeft => 74,
        MotionState::DeadRight => 75,
        MotionState::DeadUp => 76,
        MotionState::DeadUpStar => 77,
        MotionState::DeadUpStarIce => 78,
        MotionState::DeadUpFall => 79,
        MotionState::DeadUpFallHitCamera => 80,
        MotionState::DeadUpFallHitCameraFlat => 81,
        MotionState::DeadUpFallIce => 82,
        MotionState::DeadUpFallHitCameraIce => 83,
        MotionState::Sleep => 84,
        MotionState::Rebirth => 85,
        MotionState::RebirthWait => 86,
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
        MotionState::DamageFall => 94,
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
        MotionState::ShieldBreakFly => 87,
        MotionState::ShieldBreakFall => 88,
        MotionState::ShieldBreakDownU => 89,
        MotionState::ShieldBreakDownD => 90,
        MotionState::ShieldBreakStandU => 91,
        MotionState::ShieldBreakStandD => 92,
        MotionState::Furafura => 93,
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
        MotionState::CliffCatch => 50,
        MotionState::CliffWait => 51,
        MotionState::CliffClimbSlow => 73,
        MotionState::CliffClimbQuick => 74,
        MotionState::CliffAttackSlow => 75,
        MotionState::CliffAttackQuick => 76,
        MotionState::CliffEscapeSlow => 77,
        MotionState::CliffEscapeQuick => 78,
        MotionState::CliffJumpSlow1 => 79,
        MotionState::CliffJumpSlow2 => 80,
        MotionState::CliffJumpQuick1 => 81,
        MotionState::CliffJumpQuick2 => 82,
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

fn mix_melee_input_facts(hash: &mut u64, facts: MeleeInputFacts) {
    mix_i8_pair(hash, facts.lstick);
    mix_i8_pair(hash, facts.cstick);
    mix_u8(hash, facts.walk_direction as u8);
    mix_u8(
        hash,
        match facts.walk_speed_bucket {
            WalkSpeedBucket::None => 0,
            WalkSpeedBucket::Slow => 1,
            WalkSpeedBucket::Middle => 2,
            WalkSpeedBucket::Fast => 3,
        },
    );
    mix_u8(hash, facts.turn_direction as u8);
    mix_i8_pair(hash, facts.tilt_direction);
    mix_u8(hash, facts.horizontal_smash_direction as u8);
    mix_u8(hash, facts.held_dash_x_direction as u8);
    mix_u8(hash, facts.dash_direction as u8);
    for value in [
        facts.main_stick_spot_dodge,
        facts.cstick_spot_dodge,
        facts.crouch,
        facts.tap_jump,
        facts.button_jump_pressed,
        facts.button_jump_held,
        facts.cstick_jump,
    ] {
        mix_u8(hash, value as u8);
    }
    mix_u8(hash, jump_input_id(facts.normal_jump_input));
    mix_u8(hash, facts.normal_jump_pressed as u8);
    mix_u8(hash, jump_input_id(facts.jump_input));
    for value in [
        facts.jump_pressed,
        facts.fast_fall,
        facts.lstick_jump_released,
        facts.cstick_jump_released,
    ] {
        mix_u8(hash, value as u8);
    }
    mix_u32(hash, facts.source_held.bits());
    mix_u32(hash, facts.source_pressed.bits());
    mix_u32(hash, facts.source_released.bits());
    for value in [
        facts.shield_held,
        facts.shield_pressed,
        facts.shield_released,
    ] {
        mix_u8(hash, value as u8);
    }
    mix_u8(hash, facts.analog_shield);
    for value in [
        facts.analog_shield_pressed,
        facts.digital_shield_held,
        facts.digital_shield_pressed,
        facts.air_dodge_pressed,
        facts.spot_dodge,
    ] {
        mix_u8(hash, value as u8);
    }
    mix_u8(hash, facts.roll_direction as u8);
    for value in [
        facts.left_trigger_analog_held,
        facts.right_trigger_analog_held,
        facts.left_trigger_analog_pressed,
        facts.right_trigger_analog_pressed,
        facts.left_trigger_digital_pressed,
        facts.right_trigger_digital_pressed,
    ] {
        mix_u8(hash, value as u8);
    }
    mix_i8_pair(hash, facts.cstick_direction);
    mix_u8(hash, facts.attack_pressed as u8);
    mix_u8(hash, facts.air_attack_pressed as u8);
    mix_i8_pair(hash, facts.air_attack_direction);
    mix_u8(hash, facts.special_pressed as u8);
    mix_i8_pair(hash, facts.special_direction);
    mix_i8_pair(hash, facts.air_special_direction);
    mix_u8(hash, facts.grab_pressed as u8);
    mix_u8(hash, facts.neutral_attack_pressed as u8);
    mix_i8_pair(hash, facts.tilt_attack_direction);
    mix_i8_pair(hash, facts.smash_attack_direction);
    mix_i8_pair(hash, facts.cstick_smash_direction);
    for value in [
        facts.dpad_up,
        facts.dpad_down,
        facts.dpad_left,
        facts.dpad_right,
    ] {
        mix_u8(hash, value as u8);
    }
}

fn mix_i8_pair(hash: &mut u64, value: (i8, i8)) {
    mix_u8(hash, value.0 as u8);
    mix_u8(hash, value.1 as u8);
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

fn mix_source_fighter_ecb(hash: &mut u64, ecb: SourceFighterEcb) {
    mix_source_vec2(hash, ecb.top);
    mix_source_vec2(hash, ecb.right);
    mix_source_vec2(hash, ecb.bottom);
    mix_source_vec2(hash, ecb.left);
}

fn mix_source_vec2(hash: &mut u64, value: SourceVec2) {
    mix_f32(hash, value.x);
    mix_f32(hash, value.y);
}

fn mix_source_vec3(hash: &mut u64, value: SourceVec3) {
    mix_f32(hash, value.x);
    mix_f32(hash, value.y);
    mix_f32(hash, value.z);
}

fn mix_optional_u8(hash: &mut u64, value: Option<u8>) {
    match value {
        Some(value) => {
            mix_u8(hash, 1);
            mix_u8(hash, value);
        }
        None => mix_u8(hash, 0),
    }
}

fn mix_optional_u16(hash: &mut u64, value: Option<u16>) {
    match value {
        Some(value) => {
            mix_u8(hash, 1);
            mix_u32(hash, u32::from(value));
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

fn mix_optional_source_retained_model_pose(hash: &mut u64, value: Option<SourceRetainedModelPose>) {
    match value {
        Some(value) => {
            mix_u8(hash, 1);
            mix_optional_action_state_id(hash, value.action_state_id);
            mix_str(hash, value.source_action_key.as_str());
            mix_u8(hash, motion_state_id(value.motion_state));
            mix_i32(hash, value.frame_milli);
            mix_u8(hash, value.model_facing as u8);
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

fn mix_optional_source_installed_throw_hitbox(
    hash: &mut u64,
    value: Option<SourceInstalledThrowHitbox>,
) {
    match value {
        Some(value) => {
            let hitbox = value.hitbox;
            mix_u8(hash, 1);
            mix_u8(hash, hitbox.hitbox_idx);
            mix_u32(hash, hitbox.damage);
            mix_u32(hash, u32::from(hitbox.angle));
            mix_u32(hash, u32::from(hitbox.hit_x24));
            mix_u32(hash, u32::from(hitbox.hit_x28));
            mix_u32(hash, u32::from(hitbox.hit_x2c));
            mix_u8(hash, hitbox.element);
            mix_u8(hash, hitbox.sfx_severity);
            mix_u8(hash, hitbox.sfx_kind);
            mix_f32(hash, value.damage);
            mix_u32(hash, u32::from(value.unk_count));
        }
        None => mix_u8(hash, 0),
    }
}

fn mix_source_stale_move_table(hash: &mut u64, table: SourceStaleMoveTable) {
    mix_u8(hash, table.current_index);
    for entry in table.entries {
        mix_u8(hash, entry.move_id);
        mix_u32(hash, u32::from(entry.attack_instance));
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
    mix_u8(hash, common.throw_down_y as u8);
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
    mix_f32(hash, common.throw_knockback_weight);
    mix_f32(hash, common.knockback_result_scale);
    mix_f32(hash, common.knockback_result_offset);
    for reduction in common.stale_move_damage_reductions {
        mix_f32(hash, reduction);
    }
    mix_f32(hash, common.damage_knockback_velocity_scale);
    mix_f32(hash, common.damage_ground_knockback_friction_multiplier);
    mix_f32(hash, common.damage_knockback_frame_decay);
    mix_f32(hash, common.damage_sakurai_air_angle_radians);
    mix_f32(hash, common.damage_sakurai_ground_angle_degrees);
    mix_f32(hash, common.damage_sakurai_ground_min_knockback);
    mix_f32(hash, common.damage_sakurai_ground_max_knockback);
    mix_f32(hash, common.damage_duration_scale);
    mix_f32(hash, common.damage_motion_tier_1_threshold);
    mix_f32(hash, common.damage_motion_tier_2_threshold);
    mix_f32(hash, common.damage_motion_tier_3_threshold);
    mix_f32(hash, common.damage_ground_knockback_init_clamp);
    mix_f32(hash, common.damage_fly_top_angle_min_radians);
    mix_f32(hash, common.damage_fly_top_angle_max_radians);
    mix_u32(
        hash,
        u32::from(common.damage_fly_top_random_percent_threshold),
    );
    mix_f32(hash, common.damage_fly_top_random_chance);
    mix_f32(hash, common.damage_landing_down_bound_knockback_threshold);
    mix_f32(hash, common.damage_landing_basic_knockback_threshold);
    mix_f32(hash, common.damage_floor_reflect_angle_radians);
    mix_f32(hash, common.damage_floor_reflect_velocity_scale);
    mix_u8(hash, common.damage_air_v_cancel_window);
    mix_f32(hash, common.damage_air_v_cancel_knockback_scale);
    mix_u8(hash, common.passive_input_age_threshold);
    mix_f32(hash, common.passive_window_max);
    mix_f32(hash, common.passive_stand_stick_x);
    mix_f32(hash, common.special_air_drift_stick_threshold);
    mix_u8(hash, common.grab_mash_stick_threshold as u8);
    mix_i32(hash, i32::from(common.down_stand_stick_y));
    mix_i32(hash, i32::from(common.down_roll_stick_x));
    mix_f32(hash, common.down_wait_timer);
    mix_f32(hash, common.hitlag_max_frames);
    mix_f32(hash, common.hitlag_damage_scale);
    mix_f32(hash, common.hitlag_base_frames);
    mix_f32(hash, common.hitlag_crouch_multiplier);
    mix_f32(hash, common.hitlag_electric_multiplier);
    mix_f32(hash, common.di_angle_degrees);
    mix_f32(hash, common.trigger_di_knockback_multiplier);
    mix_f32(hash, common.air_speed_clamp_friction);
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
    mix_u32(hash, u32::from(common.throw_collision_lockout_ticks));
    mix_f32(hash, common.throw_weight_animation_scale);
    mix_f32(hash, common.grab_timer_base);
    mix_f32(hash, common.grab_timer_handicap_scale);
    mix_f32(hash, common.grab_timer_handicap_offset);
    mix_f32(hash, common.grab_timer_rank_scale);
    mix_f32(hash, common.grab_timer_rank_offset);
    mix_f32(hash, common.grab_timer_percent_scale);
    mix_f32(hash, common.catch_cut_ground_velocity);
    mix_f32(hash, common.capture_jump_velocity_x);
    mix_f32(hash, common.capture_jump_velocity_y);
    mix_f32(hash, common.grab_timer_decrement);
    mix_f32(hash, common.grab_mash_timer_decrement);
    mix_f32(hash, common.capture_wait_jump_input_window);
    mix_f32(hash, common.capture_wait_mash_anim_timer);
    mix_f32(hash, common.capture_wait_mash_anim_rate);
    mix_f32(hash, common.capture_pulled_high_delta_y);
    mix_f32(hash, common.walk_middle_velocity_ratio);
    mix_f32(hash, common.walk_fast_velocity_ratio);
    mix_f32(hash, common.walk_accel_taper);
    mix_f32(hash, common.run_accel_taper);
    mix_f32(hash, common.run_ground_friction_multiplier);
    mix_f32(hash, common.catch_ground_friction_multiplier);
    mix_f32(hash, common.high_speed_ground_friction_multiplier);
    mix_f32(hash, common.run_brake_animation_pause_velocity);
    mix_f32(hash, common.animation_velocity_scale);
    mix_f32(hash, common.fall_animation_drift_threshold);
    mix_f32(hash, common.fall_animation_blend);
    mix_f32(hash, common.shield_aim_smoothing);
    mix_f32(hash, common.player_nudge_x);
    mix_f32(hash, common.player_nudge_z);
    mix_f32(hash, common.player_nudge_z_clamp);
    mix_f32(hash, common.transformed_player_nudge_z);
    mix_f32(hash, common.transformed_player_nudge_z_clamp);
    mix_f32(hash, common.shield_start_health);
    mix_f32(hash, common.shield_size_health_scale);
    mix_u8(hash, common.shield_release_lockout_frames);
    mix_f32(hash, common.shield_hold_drain);
    mix_f32(hash, common.shield_regen);
    mix_f32(hash, common.shield_break_reset_health);
    mix_f32(hash, common.shield_hit_drain_damage_scale);
    mix_f32(hash, common.shield_hit_drain_base);
    mix_f32(hash, common.shield_setoff_duration_damage_scale);
    mix_f32(hash, common.shield_setoff_duration_base);
    mix_f32(hash, common.shield_setoff_pushback_scale);
    mix_f32(hash, common.shield_setoff_pushback_cap);
    mix_f32(hash, common.shield_setoff_nonreflect_pushback_multiplier);
    mix_f32(hash, common.attacker_shield_knockback_damage_scale);
    mix_f32(hash, common.attacker_shield_knockback_base);
    mix_f32(hash, common.attacker_shield_knockback_frame_decay);
    mix_f32(hash, common.attacker_shield_ground_friction_multiplier);
    mix_f32(hash, common.shield_size_light_min);
    mix_f32(hash, common.shield_size_light_max);
    mix_f32(hash, common.shield_hit_lightshield_min);
    mix_f32(hash, common.shield_hit_lightshield_max);
    mix_f32(hash, common.shield_setoff_lightshield_min);
    mix_f32(hash, common.shield_setoff_lightshield_max);
    mix_f32(hash, common.shield_hold_lightshield_min);
    mix_f32(hash, common.shield_hold_lightshield_max);
    mix_f32(hash, common.shield_break_furafura_percent_base);
    mix_f32(hash, common.shield_break_furafura_timer_base);
    mix_f32(hash, common.shield_break_furafura_timer_decrement);
    mix_f32(hash, common.shield_break_furafura_mash_decrement);
    mix_u8(hash, common.fallspecial_platform_landing_y as u8);
    mix_u8(hash, common.platform_pass_y as u8);
    mix_u8(hash, common.platform_pass_y_tap_window);
    mix_f32(hash, common.pass_initial_y_velocity);
    mix_u8(hash, common.platform_drop_delay_ticks);
    mix_u8(hash, common.cliff_grab_block_stick_y as u8);
    mix_u32(hash, u32::from(common.cliff_quick_percent_threshold));
    mix_u32(hash, u32::from(common.cliff_wait_low_percent_ticks));
    mix_u32(hash, u32::from(common.cliff_wait_high_percent_ticks));
    mix_u8(hash, common.cliff_option_stick_threshold as u8);
    mix_u32(hash, u32::from(common.ledge_cooldown_ticks));
    mix_u32(hash, u32::from(common.cliff_wait_hurt_intangible_ticks));
    mix_f32(hash, common.sdi_min_stick_mag);
    mix_u8(hash, common.sdi_stick_window);
    mix_f32(hash, common.sdi_pos_scale);
    mix_f32(hash, common.asdi_pos_scale);
    mix_u8(hash, common.rebirth_ticks);
    mix_u8(hash, common.rebirth_wait_ticks);
    mix_u32(hash, common.rebirth_hurt_intangible_ticks as u32);
    mix_u8(hash, common.top_blast_fall_ko_chance);
    mix_u8(hash, common.dead_wait_ticks);
    mix_u8(hash, common.dead_up_star_wait_ticks);
    mix_u8(hash, common.dead_up_star_rise_ticks);
    mix_u8(hash, common.dead_up_star_exit_ticks);
    mix_u8(hash, common.dead_up_fall_wait_ticks);
    mix_u8(hash, common.dead_up_fall_anim_ticks);
    mix_u8(hash, common.dead_up_fall_hit_camera_ticks);
    mix_u8(hash, common.dead_up_fall_drift_ticks);
    mix_u8(hash, common.dead_up_fall_exit_ticks);
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
    mix_f32(hash, common.guard_reflect_timer);
    mix_f32(hash, common.guard_reflect_damage_skip_timer);
    mix_u8(hash, common.run_turn_run_no_interrupt_frames);
}

fn mix_fighter_profile(hash: &mut u64, profile: FighterProfile) {
    mix_f32(hash, profile.model_scaling);
    mix_f32(hash, profile.initial_shield_size);
    mix_f32(hash, profile.shield_break_initial_velocity);
    mix_source_vec3(hash, profile.source_create_x1a70);
    mix_f32(hash, profile.walk_initial_velocity);
    mix_fighter_action_frames(hash, profile.action_frames);
    mix_captain_special_attrs(hash, profile.captain_special_attrs);
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
    mix_u8(hash, profile.weight_independent_throws_mask);
    mix_f32(hash, profile.gravity);
    mix_f32(hash, profile.terminal_velocity);
    mix_f32(hash, profile.fast_fall_velocity);
    mix_f32(hash, profile.jump_vertical_initial_velocity);
    mix_f32(hash, profile.ledge_jump_horizontal_velocity);
    mix_f32(hash, profile.ledge_jump_vertical_velocity);
    mix_f32(hash, profile.hop_vertical_initial_velocity);
    mix_i32(hash, profile.full_hop_height);
    mix_i32(hash, profile.short_hop_height);
    mix_i32(hash, profile.double_jump_height);
    mix_i32(hash, profile.ledge_snap_x_milli);
    mix_i32(hash, profile.ledge_snap_y_milli);
    mix_i32(hash, profile.ledge_snap_height_milli);
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

fn mix_captain_special_attrs(hash: &mut u64, attrs: CaptainSpecialAttrs) {
    mix_f32(hash, attrs.specialn_stick_range_y_neg);
    mix_f32(hash, attrs.specialn_stick_range_y_pos);
    mix_f32(hash, attrs.specialn_angle_diff);
    mix_f32(hash, attrs.specialn_vel_x);
    mix_f32(hash, attrs.specialn_vel_mul);
    mix_f32(hash, attrs.specials_gr_vel_x);
    mix_f32(hash, attrs.specials_grav);
    mix_f32(hash, attrs.specials_terminal_vel);
    mix_f32(hash, attrs.specials_unk0);
    mix_f32(hash, attrs.specials_unk1);
    mix_f32(hash, attrs.specials_unk2);
    mix_f32(hash, attrs.specials_unk3);
    mix_f32(hash, attrs.specials_unk4);
    mix_f32(hash, attrs.specials_unk5);
    mix_f32(hash, attrs.specials_miss_landing_lag);
    mix_f32(hash, attrs.specials_hit_landing_lag);
    mix_f32(hash, attrs.specialhi_air_friction_mul);
    mix_f32(hash, attrs.specialhi_horz_vel);
    mix_f32(hash, attrs.specialhi_freefall_air_spd_mul);
    mix_f32(hash, attrs.specialhi_landing_lag);
    mix_f32(hash, attrs.specialhi_unk0);
    mix_f32(hash, attrs.specialhi_unk1);
    mix_f32(hash, attrs.specialhi_input_var);
    mix_f32(hash, attrs.specialhi_unk2);
    mix_f32(hash, attrs.specialhi_catch_grav);
    mix_i32(hash, attrs.specialhi_air_var);
    mix_f32(hash, attrs.x68);
    mix_u32(hash, attrs.speciallw_unk1);
    mix_f32(hash, attrs.speciallw_flame_particle_angle);
    mix_f32(hash, attrs.speciallw_on_hit_spd_modifier);
    mix_i32(hash, attrs.speciallw_unk2);
    mix_f32(hash, attrs.speciallw_ground_lag_mul);
    mix_f32(hash, attrs.speciallw_landing_lag_mul);
    mix_f32(hash, attrs.speciallw_ground_traction);
    mix_f32(hash, attrs.speciallw_air_landing_traction);
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
    mix_u8(hash, action_frames.escape_f_throw_flags_b3_frame);
    mix_u8(hash, action_frames.escape_b_throw_flags_b3_frame);
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

fn mix_source_aobj_state(hash: &mut u64, aobj: SourceAObjState) {
    mix_u32(hash, aobj.flags);
    mix_f32(hash, aobj.curr_frame);
    mix_f32(hash, aobj.rewind_frame);
    mix_f32(hash, aobj.end_frame);
    mix_f32(hash, aobj.framerate);
}

fn mix_source_fighter_playback(hash: &mut u64, playback: SourceFighterPlayback) {
    mix_source_aobj_state(hash, playback.primary);
    match playback.secondary {
        Some(secondary) => {
            mix_u8(hash, 1);
            mix_source_aobj_state(hash, secondary);
        }
        None => mix_u8(hash, 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rollback_snapshot_restores_all_recent_authoritative_player_fields() {
        let mut world = World::for_two_players();
        let player = &mut world.players_mut()[0];
        player.source_previous_position = SourceVec2 { x: 1.25, y: -2.5 };
        player.source_previous_position_z = 3.75;
        player.source_attacker_shield_velocity_x = -4.5;
        player.source_coll_floor_skip_line_index = Some(7);
        player.source_coll_ledge_id_left = Some(8);
        player.source_coll_ledge_id_right = Some(9);
        player.source_fall_anim_blend = 0.375;
        player.source_fall_anim_pose = MotionState::FallAerialB;
        player.source_allow_interrupt = true;
        player.source_x1a70 = SourceVec3 {
            x: 10.0,
            y: 11.0,
            z: 12.0,
        };
        player.source_x34_scale_y = 1.25;
        player.captain_special_hi_x0 = 13;
        player.captain_special_hi_vel_x = 14.0;
        player.captain_special_hi_vel_y = -15.0;
        player.captain_special_hi_x2_b0 = true;
        player.captain_special_hi_x2_b1 = true;
        player.source_collision_lightshield_amount = 0.625;

        let expected_player = world.players()[0];
        let expected_checksum = world.checksum();
        let snapshot = world.rollback_snapshot();
        assert!(Arc::ptr_eq(
            &world.source_hit_victim_log,
            &snapshot.source_hit_victim_log
        ));

        world.players_mut()[0] = PlayerState::new(0, 0, 1);
        assert_ne!(world.checksum(), expected_checksum);
        world.restore_rollback_snapshot(&snapshot);

        assert_eq!(world.players()[0], expected_player);
        assert_eq!(world.checksum(), expected_checksum);
    }

    #[test]
    fn checksum_covers_rollback_owned_last_input_facts() {
        let mut world = World::for_two_players();
        let before = world.checksum();
        let mut facts = [MeleeInputFacts::default(); PLAYER_COUNT];
        facts[0].attack_pressed = true;

        world.set_last_input_facts(facts);

        assert_ne!(world.checksum(), before);
    }

    #[test]
    fn source_aobj_first_play_evaluates_requested_frame_without_advancing() {
        let mut aobj = SourceAObjState::requested(4.5, 1.0, 0.0, 8.0, 0);

        assert_eq!(aobj.interpret_frame(), Some(4.5));
        assert_eq!(aobj.curr_frame, 4.5);
        assert_eq!(aobj.flags & SOURCE_AOBJ_FIRST_PLAY, 0);

        assert_eq!(aobj.interpret_frame(), Some(5.5));
        assert_eq!(aobj.curr_frame, 5.5);
    }

    #[test]
    fn source_aobj_non_loop_evaluates_endpoint_then_stops() {
        let mut aobj = SourceAObjState::requested(7.0, 1.0, 0.0, 8.0, 0);
        assert_eq!(aobj.interpret_frame(), Some(7.0));
        assert_eq!(aobj.interpret_frame(), Some(8.0));
        assert_ne!(aobj.flags & SOURCE_AOBJ_NO_ANIM, 0);
        assert_eq!(aobj.interpret_frame(), None);
    }

    #[test]
    fn source_aobj_loop_wraps_over_rewind_to_end_interval() {
        let mut aobj = SourceAObjState::requested(7.5, 1.0, 2.0, 8.0, SOURCE_AOBJ_LOOP);
        assert_eq!(aobj.interpret_frame(), Some(7.5));
        assert_eq!(aobj.interpret_frame(), Some(2.5));
        assert_ne!(aobj.flags & SOURCE_AOBJ_REWINDED, 0);
    }

    #[test]
    fn persistent_fighter_playback_installs_interprets_loops_and_stops() {
        let mut playback = SourceFighterPlayback::default();
        playback.install_primary_descriptor(
            7.5,
            1.0,
            SourceAObjDescriptor {
                end_frame: 8.0,
                rewind_frame: 2.0,
                flags: SOURCE_AOBJ_LOOP,
            },
        );

        assert_eq!(playback.interpret_primary(), Some(7.5));
        assert_eq!(playback.interpret_primary(), Some(2.5));
        assert_ne!(playback.primary().flags & SOURCE_AOBJ_REWINDED, 0);

        playback.install_primary_descriptor(
            7.0,
            1.0,
            SourceAObjDescriptor {
                end_frame: 8.0,
                rewind_frame: 0.0,
                flags: 0,
            },
        );
        assert_eq!(playback.interpret_primary(), Some(7.0));
        assert_eq!(playback.interpret_primary(), Some(8.0));
        assert_eq!(playback.interpret_primary(), None);
    }

    #[test]
    fn requesting_existing_primary_frame_preserves_non_animation_flags() {
        let mut playback = SourceFighterPlayback::default();
        playback.install_primary_descriptor(
            1.0,
            0.5,
            SourceAObjDescriptor {
                end_frame: 8.0,
                rewind_frame: 2.0,
                flags: SOURCE_AOBJ_LOOP | SOURCE_AOBJ_NO_UPDATE,
            },
        );
        playback.primary.flags |= SOURCE_AOBJ_REWINDED | SOURCE_AOBJ_NO_ANIM;

        playback.request_primary_frame(3.25);

        assert_eq!(playback.primary.curr_frame, 3.25);
        assert_eq!(
            playback.primary.flags,
            SOURCE_AOBJ_LOOP
                | SOURCE_AOBJ_NO_UPDATE
                | SOURCE_AOBJ_REWINDED
                | SOURCE_AOBJ_FIRST_PLAY
        );
        assert_eq!(playback.primary.framerate, 0.5);
        assert_eq!(playback.primary.rewind_frame, 2.0);
        assert_eq!(playback.primary.end_frame, 8.0);
    }

    #[test]
    fn legacy_animation_frame_setters_keep_primary_playback_synchronized() {
        let mut player = PlayerState::new(0, 0, 1);

        player.set_source_motion_anim_frame_milli(3_250);
        assert_eq!(player.source_playback().primary().curr_frame, 3.25);

        player.set_source_motion_anim_rate(0.5);
        assert_eq!(player.source_playback().primary().framerate, 0.5);

        player.set_source_motion_anim_rate_milli(750);
        assert_eq!(player.source_motion_anim_rate, 0.75);
        assert_eq!(player.source_playback().primary().framerate, 0.75);
    }

    #[test]
    fn diagnostic_state_normalization_reconciles_primary_playback() {
        let mut world = World::for_two_players();
        let mut state = PlayerState::new(0, 0, 1);
        state.source_motion_anim_frame = 3.25;
        state.motion_anim_frame_milli = 3_250;
        state.source_motion_anim_rate = 1.0;
        state.motion_anim_rate_milli = 750;
        state.source_playback.primary.curr_frame = 9.0;
        state.source_playback.primary.framerate = 1.0;

        assert!(world.set_player_state_for_diagnostic(0, state));

        let primary = world.players()[0].source_playback().primary();
        assert_eq!(primary.curr_frame, 3.25);
        assert_eq!(primary.framerate, 0.75);
    }

    #[test]
    fn persistent_fighter_playback_round_trips_through_rollback() {
        let mut world = World::for_two_players();
        world.players_mut()[0].install_source_primary_anim(
            3.25,
            0.5,
            SourceAObjDescriptor {
                end_frame: 9.0,
                rewind_frame: 1.0,
                flags: SOURCE_AOBJ_LOOP | SOURCE_AOBJ_NO_UPDATE,
            },
        );
        world.players_mut()[0].set_source_secondary_anim(Some(SourceAObjState::requested(
            4.0,
            0.25,
            2.0,
            6.0,
            SOURCE_AOBJ_LOOP,
        )));
        let expected = *world.players()[0].source_playback();
        let snapshot = world.rollback_snapshot();

        world.players_mut()[0].source_playback = SourceFighterPlayback::default();
        world.restore_rollback_snapshot(&snapshot);

        assert_eq!(world.players()[0].source_playback, expected);
    }

    #[test]
    fn checksum_is_sensitive_to_every_persistent_playback_field() {
        let default_playback = SourceFighterPlayback::default();
        let playback_variants = [
            SourceFighterPlayback {
                primary: SourceAObjState {
                    flags: 0,
                    ..default_playback.primary
                },
                ..default_playback
            },
            SourceFighterPlayback {
                primary: SourceAObjState {
                    curr_frame: 1.5,
                    ..default_playback.primary
                },
                ..default_playback
            },
            SourceFighterPlayback {
                primary: SourceAObjState {
                    rewind_frame: 2.0,
                    ..default_playback.primary
                },
                ..default_playback
            },
            SourceFighterPlayback {
                primary: SourceAObjState {
                    end_frame: 12.0,
                    ..default_playback.primary
                },
                ..default_playback
            },
            SourceFighterPlayback {
                primary: SourceAObjState {
                    framerate: 0.75,
                    ..default_playback.primary
                },
                ..default_playback
            },
            SourceFighterPlayback {
                secondary: Some(SourceAObjState {
                    flags: 0,
                    ..default_playback.primary
                }),
                ..default_playback
            },
            SourceFighterPlayback {
                secondary: Some(SourceAObjState {
                    curr_frame: 1.5,
                    ..default_playback.primary
                }),
                ..default_playback
            },
            SourceFighterPlayback {
                secondary: Some(SourceAObjState {
                    rewind_frame: 2.0,
                    ..default_playback.primary
                }),
                ..default_playback
            },
            SourceFighterPlayback {
                secondary: Some(SourceAObjState {
                    end_frame: 12.0,
                    ..default_playback.primary
                }),
                ..default_playback
            },
            SourceFighterPlayback {
                secondary: Some(SourceAObjState {
                    framerate: 0.75,
                    ..default_playback.primary
                }),
                ..default_playback
            },
        ];

        for playback in playback_variants {
            let mut world = World::for_two_players();
            let baseline = world.checksum();
            world.players_mut()[0].source_playback = playback;

            assert_ne!(world.checksum(), baseline);
        }
    }

    #[test]
    fn fall_aobj_descriptor_uses_extracted_figatree_endpoint() {
        let player = PlayerState::new(0, 0, 1);
        let mut fall = player;
        fall.set_motion_state_alias(MotionState::Fall);

        let descriptor = source_aobj_descriptor_for_motion_state(fall.motion_state)
            .expect("Fall must have extracted AObj descriptor metadata");
        assert_eq!(descriptor.end_frame, 8.0);
        assert_eq!(descriptor.rewind_frame, 0.0);
        assert_ne!(descriptor.flags & SOURCE_AOBJ_LOOP, 0);
        assert_eq!(
            action_pose_sample_frame(
                &fall,
                MotionState::Fall,
                8.0,
                MeleeCommonData::PROVISIONAL,
                9
            ),
            0.0,
            "Fall's Fighter_WaitAnimData loop flag must wrap its AObj over [0, end_frame)"
        );
    }

    #[test]
    fn escape_air_binding_preserves_motion_state_and_action_table_id_spaces() {
        let binding = source_binding_for_motion_state(MotionState::EscapeAir)
            .expect("EscapeAir must have a source binding");

        assert_eq!(
            binding.melee_motion_state_id,
            Some(MeleeMotionStateId::new(236))
        );
        assert_eq!(
            binding.source_action_table_index,
            SourceActionTableIndex::new(44)
        );
        assert_eq!(binding.source_action_key.as_str(), "EscapeAir");
        assert_eq!(binding.runtime_motion_state, Some(MotionState::EscapeAir));
    }

    #[test]
    fn source_aobj_binding_uses_live_identity_without_changing_legacy_alias_pose_binding() {
        let mut dash = PlayerState::new(0, 0, 1);
        dash.set_motion_state_alias(MotionState::Dash);
        let dash_binding =
            source_binding_for_motion_state(MotionState::Dash).expect("Dash source binding");
        assert_eq!(
            source_action_table_id_for_player(dash),
            None,
            "legacy live-pose sampling must keep aliases out of the action-table path"
        );
        assert_eq!(
            source_migrated_primary_anim_descriptor_for_player(&dash),
            falcon_ecb::falcon_aobj_descriptor_for_action_table_id(
                dash_binding.source_action_table_id
            ),
            "AObj installation must still bind Dash by its live action identity"
        );

        let mut attack100_start = PlayerState::new(0, 0, 1);
        attack100_start.motion_state = MotionState::Attack1;
        attack100_start.motion_state_alias = None;
        attack100_start.melee_action_state_id = Some(MeleeActionStateId::new(47));
        attack100_start.source_action_key = Some(SourceActionKey::new("Attack100Start"));
        assert_eq!(source_action_table_id_for_player(attack100_start), Some(49));

        attack100_start.source_action_key = Some(SourceActionKey::new("Attack100Loop"));
        assert_eq!(
            source_migrated_primary_anim_descriptor_for_player(&attack100_start),
            None,
            "a migrated animation must not bind a descriptor for a different source action key"
        );

        let mut damage_n2 = PlayerState::new(0, 0, 1);
        damage_n2.melee_action_state_id = Some(MeleeActionStateId::new(79));
        damage_n2.source_action_key = Some(SourceActionKey::new("DamageN2"));
        assert_eq!(
            source_migrated_primary_anim_descriptor_for_player(&damage_n2),
            falcon_ecb::falcon_aobj_descriptor_for_action_table_id(169),
            "common Damage p1 playback binds the exact source action identity"
        );
        assert_eq!(
            source_action_table_id_for_player(damage_n2),
            None,
            "migrating Damage playback must not widen legacy ECB pose selection"
        );
    }

    #[test]
    fn common_throw_actions_use_decomp_motion_state_move_ids() {
        assert_eq!(
            source_move_id_for_action_state_id(MeleeActionStateId::new(219)),
            54,
            "ftmotionstates.c ftCo_MS_ThrowF stores FtMoveId_ThrowF << 24"
        );
        assert_eq!(
            source_move_id_for_action_state_id(MeleeActionStateId::new(220)),
            55,
            "ftmotionstates.c ftCo_MS_ThrowB stores FtMoveId_ThrowB << 24"
        );
        assert_eq!(
            source_move_id_for_action_state_id(MeleeActionStateId::new(221)),
            56,
            "ftmotionstates.c ftCo_MS_ThrowHi stores FtMoveId_ThrowHi << 24"
        );
        assert_eq!(
            source_move_id_for_action_state_id(MeleeActionStateId::new(222)),
            57,
            "ftmotionstates.c ftCo_MS_ThrowLw stores FtMoveId_ThrowLw << 24"
        );
    }

    fn assert_source_ecb_close(actual: SourceFighterEcb, expected: SourceFighterEcb) {
        const EPSILON: f32 = 0.0001;
        for (label, actual_value, expected_value) in [
            ("top.x", actual.top.x, expected.top.x),
            ("top.y", actual.top.y, expected.top.y),
            ("right.x", actual.right.x, expected.right.x),
            ("right.y", actual.right.y, expected.right.y),
            ("bottom.x", actual.bottom.x, expected.bottom.x),
            ("bottom.y", actual.bottom.y, expected.bottom.y),
            ("left.x", actual.left.x, expected.left.x),
            ("left.y", actual.left.y, expected.left.y),
        ] {
            assert!(
                (actual_value - expected_value).abs() <= EPSILON,
                "{label} differs: live {actual_value:.6}, baked {expected_value:.6}; live={:?}, baked={:?}",
                actual,
                expected
            );
        }
    }

    fn source_ecb(
        top: (f32, f32),
        right: (f32, f32),
        bottom: (f32, f32),
        left: (f32, f32),
    ) -> SourceFighterEcb {
        SourceFighterEcb {
            top: SourceVec2 { x: top.0, y: top.1 },
            right: SourceVec2 {
                x: right.0,
                y: right.1,
            },
            bottom: SourceVec2 {
                x: bottom.0,
                y: bottom.1,
            },
            left: SourceVec2 {
                x: left.0,
                y: left.1,
            },
        }
    }

    #[test]
    fn jumpaerialf_live_jobj_ecb_matches_extracted_source_pose_frame_10() {
        let live =
            falcon_ecb::falcon_source_ecb_jobj_for_motion_state(MotionState::JumpAerialF, 10_000)
                .expect("JumpAerialF should have live JObj ECB data");
        let extracted = source_ecb(
            (0.0, 11.493333),
            (2.6196253, 8.329991),
            (0.0, 5.1666503),
            (-2.6196253, 8.329991),
        );

        assert_source_ecb_close(live, extracted);
    }

    #[test]
    fn active_live_jobj_ecb_preserves_decomp_float_pose_frame() {
        let mut player = PlayerState::new(0, 0, 1);
        player.set_motion_state_alias(MotionState::AttackAirHi);
        player.set_source_motion_anim_frame(2.9977806);

        let actual = active_source_local_ecb_for_player(&player, MeleeCommonData::PROVISIONAL);
        let expected = source_ecb(
            (0.0, 18.575452),
            (3.115442, 10.884598),
            (0.0, 3.193743),
            (-3.115442, 10.884598),
        );

        assert!(
            (player.cur_anim_frame() - 2.9977806).abs() <= f32::EPSILON,
            "PlayerState must retain the decomp HSD_AObj f32 frame before ECB sampling"
        );
        assert_source_ecb_close(actual, expected);
    }

    #[test]
    fn specialhi_live_jobj_ecb_matches_extracted_source_pose_frame_55() {
        let live =
            falcon_ecb::falcon_source_ecb_jobj_for_motion_state(MotionState::SpecialHi, 55_000)
                .expect("SpecialHi should have live JObj ECB data");
        let extracted = source_ecb(
            (0.0, 12.37972),
            (2.3788543, 10.093167),
            (0.0, 7.8066134),
            (-2.3788543, 10.093167),
        );

        assert_source_ecb_close(live, extracted);
    }

    #[test]
    fn specialhi_live_jobj_ecb_matches_extracted_source_pose_frame_60() {
        let live =
            falcon_ecb::falcon_source_ecb_jobj_for_motion_state(MotionState::SpecialHi, 60_000)
                .expect("SpecialHi should have live JObj ECB data");
        let extracted = source_ecb(
            (0.0, 14.468869),
            (2.1326225, 12.125606),
            (0.0, 9.782342),
            (-2.1326225, 12.125606),
        );

        assert_source_ecb_close(live, extracted);
    }

    #[test]
    fn landingfallspecial_live_jobj_ecb_matches_extracted_source_pose_frame_0() {
        let live =
            falcon_ecb::falcon_source_ecb_jobj_for_motion_state(MotionState::LandingFallSpecial, 0)
                .expect("LandingFallSpecial should have live JObj ECB data");
        let extracted = source_ecb(
            (0.0, 9.347474),
            (4.511843, 4.673737),
            (0.0, 0.0),
            (-4.511843, 4.673737),
        );

        assert_source_ecb_close(live, extracted);
    }

    #[test]
    fn fall_runtime_ecb_wraps_to_first_source_pose_after_loop_endpoint() {
        let mut player = PlayerState::new(0, 0, 1);
        player.set_motion_state_alias(MotionState::Fall);
        player.motion_anim_frame_milli = 8_000;

        let actual = live_source_local_ecb_for_player_pose_frame_milli(
            &player,
            8_000,
            MeleeCommonData::PROVISIONAL,
        );
        let expected = source_ecb(
            (0.0, 10.779321),
            (3.9122283, 6.3886514),
            (0.0, 1.9979814),
            (-3.9122283, 6.3886514),
        );

        assert_source_ecb_close(actual, expected);
    }

    #[test]
    fn source_jobj_ecb_stays_live_while_coll_bottom_is_locked() {
        let mut player = PlayerState::new(0, 0, 1);
        player.set_motion_state_alias(MotionState::SpecialHi);
        player.motion_anim_frame_milli = 55_000;
        player.ecb_bottom_offset_y = 0;
        player.ecb_bottom_lock_timer = 5;
        player.source_coll_x130_locked = true;

        let actual = live_source_local_ecb_for_player_pose_frame_milli(
            &player,
            55_000,
            MeleeCommonData::PROVISIONAL,
        );
        let expected =
            falcon_ecb::falcon_source_ecb_jobj_for_motion_state(MotionState::SpecialHi, 55_000)
                .expect("SpecialHi should have live JObj ECB data");

        assert_source_ecb_close(actual, expected);
    }

    #[test]
    fn falcon_costume_skeleton_changes_live_jobj_ecb() {
        let neutral =
            falcon_ecb::falcon_source_ecb_jobj_for_motion_state_frame_with_flags_and_costume(
                MotionState::AttackAirHi,
                5.0,
                6,
                0,
            )
            .expect("neutral AttackAirHi should have live JObj ECB data");
        let blue =
            falcon_ecb::falcon_source_ecb_jobj_for_motion_state_frame_with_flags_and_costume(
                MotionState::AttackAirHi,
                5.0,
                6,
                5,
            )
            .expect("blue AttackAirHi should have live JObj ECB data");

        assert_source_ecb_close(
            blue,
            source_ecb(
                (0.0, 11.636957),
                (4.7629714, 8.532203),
                (0.0, 5.4274487),
                (-4.7629714, 8.532203),
            ),
        );
        assert_ne!(neutral.top.y.to_bits(), blue.top.y.to_bits());
        assert_ne!(neutral.left.y.to_bits(), blue.left.y.to_bits());
    }

    #[test]
    fn fighter_lifecycle_resets_preserve_costume_index() {
        let mut player = PlayerState::new(0, 0, 1);
        player.costume_index = 5;
        player.reset_for_entry_spawn(1_000, 2_000, -1, player.profile, 60.0, 5);
        assert_eq!(player.costume_index, 5);

        player.enter_source_rebirth_state(
            StageProfile::battlefield().respawn_platforms[0],
            60.0,
            30,
        );
        assert_eq!(player.costume_index, 5);
    }

    #[test]
    fn active_source_jobj_ecb_stays_live_while_coll_bottom_is_locked() {
        let mut player = PlayerState::new(0, 0, 1);
        player.set_motion_state_alias(MotionState::SpecialHi);
        player.motion_anim_frame_milli = 55_000;
        player.ecb_bottom_offset_y = 0;
        player.ecb_bottom_lock_timer = 5;
        player.source_coll_x130_locked = true;

        let actual = active_source_local_ecb_for_pose_frame_milli(
            &player,
            55_000,
            MeleeCommonData::PROVISIONAL,
        );
        let expected =
            falcon_ecb::falcon_source_ecb_jobj_for_motion_state(MotionState::SpecialHi, 55_000)
                .expect("SpecialHi should have live JObj ECB data");

        assert_source_ecb_close(actual, expected);
    }
}
