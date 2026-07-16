use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use mole_core::{
    is_source_damage_action_state_id, melee_action_state_id_for_motion_state,
    source_binding_for_motion_state, source_units_to_milli, step_world, Frame, MeleeActionStateId,
    MeleeCommonData, MeleeInputTimers, MeleeMotionStateId, MotionState, PlayerInput,
    PlayerRenderSnapshot, PlayerState, SourceActionTableIndex, SourceCollEcbSnapshot, SourceVec2,
    StageProfile, Vec2, World, WorldRollbackSnapshot, CANONICAL_SOURCE_ONLY_ACTION_BINDINGS,
    PLAYER_COUNT,
};
use serde::Deserialize;

const HSD_DPAD_LEFT: u32 = 1 << 0;
const HSD_DPAD_RIGHT: u32 = 1 << 1;
const HSD_DPAD_DOWN: u32 = 1 << 2;
const HSD_DPAD_UP: u32 = 1 << 3;
const HSD_Z: u32 = 1 << 4;
const HSD_R: u32 = 1 << 5;
const HSD_L: u32 = 1 << 6;
const HSD_A: u32 = 1 << 8;
const HSD_B: u32 = 1 << 9;
const HSD_X: u32 = 1 << 10;
const HSD_Y: u32 = 1 << 11;
const HSD_START: u32 = 1 << 12;
const SIGNIFICANT_POSITION_DRIFT_MILLI: i32 = 100;

fn step_match_start_replay_world(
    world: &mut World,
    core_frame: Frame,
    inputs: &[PlayerInput; PLAYER_COUNT],
) {
    crate::step_world_with_source_collisions(world, core_frame, inputs);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlippiCoreComparisonConfig {
    pub compare_players: [bool; PLAYER_COUNT],
    pub max_frames: Option<usize>,
}

impl Default for SlippiCoreComparisonConfig {
    fn default() -> Self {
        Self {
            compare_players: [true; PLAYER_COUNT],
            max_frames: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlippiCoreMismatch {
    pub frame: Frame,
    pub source_frame: i32,
    pub player_index: usize,
    pub expected_slippi_state_id: u16,
    pub expected_action_state_id: MeleeActionStateId,
    pub actual_action_state_id: Option<MeleeActionStateId>,
    pub expected_motion_state: Option<MotionState>,
    pub actual_motion_state: MotionState,
    pub expected_position: Vec2,
    pub actual_position: Vec2,
    pub expected_ground_velocity_x: i32,
    pub expected_air_velocity_x: i32,
    pub expected_velocity_y: i32,
    pub actual_velocity_x: i32,
    pub actual_velocity_y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlippiCorePositionDrift {
    pub frame: Frame,
    pub source_frame: i32,
    pub player_index: usize,
    pub expected_slippi_state_id: u16,
    pub expected_action_state_id: MeleeActionStateId,
    pub actual_action_state_id: Option<MeleeActionStateId>,
    pub expected_motion_state: Option<MotionState>,
    pub actual_motion_state: MotionState,
    pub expected_position: Vec2,
    pub actual_position: Vec2,
    pub expected_ground_velocity_x: i32,
    pub expected_air_velocity_x: i32,
    pub expected_velocity_y: i32,
    pub actual_velocity_x: i32,
    pub actual_velocity_y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlippiCoreComparisonMode {
    SeededPreFrameDiagnostic,
    SequentialMatchStart,
}

impl SlippiCoreComparisonMode {
    const fn label(self) -> &'static str {
        match self {
            Self::SeededPreFrameDiagnostic => "seeded pre-frame diagnostic",
            Self::SequentialMatchStart => "sequential Slippi match-start",
        }
    }

    const fn description(self) -> &'static str {
        match self {
            Self::SeededPreFrameDiagnostic => {
                "It seeds each comparable Rust frame from Slippi pre-frame state, so it is useful for local state/physics slices but not a full replay oracle."
            }
            Self::SequentialMatchStart => {
                "It starts the Rust world from Slippi Battlefield match-start spawn state and advances exported game-facing inputs sequentially."
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlippiCoreComparison {
    pub mode: SlippiCoreComparisonMode,
    pub source_replay_path: Option<String>,
    pub ucf_players: [bool; PLAYER_COUNT],
    pub ucf_dashback_amendment_frames: [usize; PLAYER_COUNT],
    pub frames_compared: usize,
    pub player_frames_compared: [usize; PLAYER_COUNT],
    pub unsupported_state_count: usize,
    pub unsupported_states: Vec<(u16, usize)>,
    pub state_mismatch_count: usize,
    pub max_abs_ground_velocity_diff: [i32; PLAYER_COUNT],
    pub first_position_drift: Option<SlippiCorePositionDrift>,
    pub first_position_drift_by_player: [Option<SlippiCorePositionDrift>; PLAYER_COUNT],
    pub first_state_mismatch: Option<SlippiCoreMismatch>,
    pub first_divergence: Option<SlippiCoreFirstDivergence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlippiCoreTraceConfig {
    pub player_index: usize,
    pub source_frame_start: i32,
    pub source_frame_end: i32,
    pub max_frames: Option<usize>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SlippiActionIdentity {
    pub melee_motion_state_id: Option<MeleeMotionStateId>,
    pub source_action_table_index: Option<SourceActionTableIndex>,
    pub source_action_key: Option<&'static str>,
}

impl SlippiActionIdentity {
    pub const fn missing() -> Self {
        Self {
            melee_motion_state_id: None,
            source_action_table_index: None,
            source_action_key: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlippiCoreTraceRow {
    pub core_frame: Frame,
    pub source_frame: i32,
    pub player_index: usize,
    pub input_stick_x: i8,
    pub input_stick_y: i8,
    pub input_button_bits: u32,
    pub input_left_trigger: u8,
    pub input_right_trigger: u8,
    pub input_ucf_dashback_amendment: bool,
    pub input_ucf_shield_drop_amendment: bool,
    pub actual_input_jump_pressed: bool,
    pub actual_input_normal_jump_pressed: bool,
    pub actual_input_shield_held: bool,
    pub actual_input_shield_pressed: bool,
    pub expected_slippi_state_id: u16,
    pub expected_action_state_id: MeleeActionStateId,
    pub actual_action_state_id: Option<MeleeActionStateId>,
    pub expected_action_identity: SlippiActionIdentity,
    pub actual_action_identity: SlippiActionIdentity,
    pub expected_motion_state: Option<MotionState>,
    pub actual_motion_state: MotionState,
    pub actual_grounded: bool,
    pub actual_motion_frame: u8,
    pub actual_motion_anim_frame_milli: i32,
    pub actual_source_fall_anim_blend: f32,
    pub actual_source_fall_anim_pose: MotionState,
    pub expected_facing: i8,
    pub actual_facing: i8,
    pub expected_position: Vec2,
    pub actual_position: Vec2,
    pub expected_source_position: SourceVec2,
    pub actual_source_position: SourceVec2,
    pub expected_ground_velocity_x: i32,
    pub expected_air_velocity_x: i32,
    pub expected_velocity_y: i32,
    pub expected_attack_velocity_x: i32,
    pub expected_attack_velocity_y: i32,
    pub expected_composed_velocity_x: i32,
    pub expected_composed_velocity_y: i32,
    pub expected_ground_velocity_x_source: f32,
    pub expected_air_velocity_x_source: f32,
    pub expected_velocity_y_source: f32,
    pub expected_attack_velocity_x_source: f32,
    pub expected_attack_velocity_y_source: f32,
    pub actual_velocity_x: i32,
    pub actual_velocity_y: i32,
    pub actual_source_self_velocity_x: f32,
    pub actual_source_self_velocity_y: f32,
    pub actual_source_knockback_velocity_x: f32,
    pub actual_source_knockback_velocity_y: f32,
    pub actual_source_ground_knockback_velocity: f32,
    pub actual_player_nudge_x: f32,
    pub actual_player_nudge_z: f32,
    pub actual_ground_velocity_x: f32,
    pub actual_ground_accel_x: f32,
    pub actual_ground_accel_x2: f32,
    pub actual_dash_entry_velocity_delta: f32,
    pub actual_dash_x0: f32,
    pub actual_source_coll_last_pos: SourceVec2,
    pub actual_source_coll_cur_pos: SourceVec2,
    pub actual_source_coll_prev_pos: SourceVec2,
    pub actual_source_coll_ecb: SourceCollEcbSnapshot,
    pub actual_source_coll_prev_ecb: SourceCollEcbSnapshot,
    pub actual_source_coll_desired_ecb: SourceCollEcbSnapshot,
    pub actual_ecb_bottom_lock_timer: u8,
    pub actual_source_coll_x130_locked: bool,
    pub actual_floor_skip_surface: Option<u8>,
    pub actual_source_floor_skip_line: Option<u16>,
    pub actual_source_floor_surface: Option<u8>,
    pub actual_source_floor_line: Option<u16>,
    pub actual_source_coll_env_flags: u32,
    pub actual_source_coll_prev_env_flags: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SlippiCoreTrace {
    pub source_replay_path: Option<String>,
    pub config: SlippiCoreTraceConfig,
    pub rows: Vec<SlippiCoreTraceRow>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlippiCoreDivergenceScanConfig {
    pub compare_players: [bool; PLAYER_COUNT],
    pub max_frames: Option<usize>,
    pub lookahead_frames: usize,
    pub max_scenarios: Option<usize>,
    pub position_tolerance_milli: i32,
    pub velocity_tolerance_milli: i32,
}

impl Default for SlippiCoreDivergenceScanConfig {
    fn default() -> Self {
        Self {
            compare_players: [true; PLAYER_COUNT],
            max_frames: None,
            lookahead_frames: 5,
            max_scenarios: None,
            position_tolerance_milli: SIGNIFICANT_POSITION_DRIFT_MILLI,
            velocity_tolerance_milli: 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlippiCoreDivergenceKind {
    UnsupportedState,
    MixedPhaseWitness,
    StateMismatch,
    PositionDrift,
    VelocityDrift,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SlippiCoreDivergenceScan {
    pub source_replay_path: Option<String>,
    pub config: SlippiCoreDivergenceScanConfig,
    pub frames_scanned: usize,
    pub scenarios: Vec<SlippiCoreDivergenceScenario>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SlippiVisualReplayFrame {
    pub core_frame: Frame,
    pub source_frame: i32,
    pub inputs: [PlayerInput; PLAYER_COUNT],
    pub raw_inputs: [SlippiRustPlayerInput; PLAYER_COUNT],
    pub expected_players: [Option<SlippiPostFrame>; PLAYER_COUNT],
}

#[derive(Debug, Clone, PartialEq)]
pub struct SlippiVisualReplayDivergence {
    pub kind: SlippiCoreDivergenceKind,
    pub player_index: usize,
    pub core_frame: Frame,
    pub source_frame: i32,
    pub max_abs_position_delta_milli: i32,
    pub max_abs_velocity_delta_milli: i32,
    pub row: SlippiCoreTraceRow,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SlippiVisualReplayDivergenceGate {
    lookahead_frames: usize,
    pending: Option<SlippiVisualReplayDivergence>,
    divergent_frames: usize,
}

impl SlippiVisualReplayDivergenceGate {
    pub fn new(lookahead_frames: usize) -> Self {
        Self {
            lookahead_frames,
            pending: None,
            divergent_frames: 0,
        }
    }

    pub fn observe(
        &mut self,
        divergence: Option<SlippiVisualReplayDivergence>,
    ) -> Option<SlippiVisualReplayDivergence> {
        let Some(divergence) = divergence else {
            self.pending = None;
            self.divergent_frames = 0;
            return None;
        };

        if !visual_replay_should_stop_on_divergence(&divergence) {
            return None;
        }

        if self.pending.is_none() {
            self.pending = Some(divergence);
            self.divergent_frames = 1;
        } else {
            self.divergent_frames = self.divergent_frames.saturating_add(1);
        }

        if self.divergent_frames > self.lookahead_frames {
            return self.pending.clone();
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SlippiCoreDivergenceScenario {
    pub scenario_index: usize,
    pub kind: SlippiCoreDivergenceKind,
    pub player_index: usize,
    pub core_frame: Frame,
    pub source_frame: i32,
    pub end_core_frame: Frame,
    pub end_source_frame: i32,
    pub duration_frames: usize,
    pub realigned_within_lookahead: bool,
    pub realign_core_frame: Option<Frame>,
    pub realign_source_frame: Option<i32>,
    pub rollback_replay_deterministic: bool,
    pub first_frame: SlippiCoreTraceRow,
    pub max_abs_position_delta_milli: i32,
    pub max_abs_velocity_delta_milli: i32,
    pub cascades_from_source_frame: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlippiCoreFirstDivergence {
    pub kind: SlippiCoreDivergenceKind,
    pub player_index: usize,
    pub core_frame: Frame,
    pub source_frame: i32,
}

#[derive(Debug, Clone, PartialEq)]
struct ActiveDivergenceScenario {
    kind: SlippiCoreDivergenceKind,
    player_index: usize,
    core_frame: Frame,
    source_frame: i32,
    end_core_frame: Frame,
    end_source_frame: i32,
    duration_frames: usize,
    realigned_within_lookahead: bool,
    realign_core_frame: Option<Frame>,
    realign_source_frame: Option<i32>,
    rollback_replay_deterministic: bool,
    first_frame: SlippiCoreTraceRow,
    max_abs_position_delta_milli: i32,
    max_abs_velocity_delta_milli: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SlippiCoreDivergenceStatus {
    kind: SlippiCoreDivergenceKind,
    max_abs_position_delta_milli: i32,
    max_abs_velocity_delta_milli: i32,
}

impl SlippiCoreComparison {
    pub fn report_markdown(&self) -> String {
        let mut lines = vec![
            "# Slippi To Rust Core Diagnostic".to_string(),
            String::new(),
            "This report replays Slippi-exported game-facing inputs through the Rust core."
                .to_string(),
            "It is a physics/state oracle pass, not a raw WUP adapter capture.".to_string(),
            format!("- Comparison mode: {}", self.mode.label()),
            self.mode.description().to_string(),
            String::new(),
        ];
        if let Some(path) = &self.source_replay_path {
            lines.push(format!("- Source replay: `{path}`"));
        }
        lines.extend([
            format!(
                "- Replay controller fixes: P1 {}, P2 {}",
                controller_fix_label(self.ucf_players[0]),
                controller_fix_label(self.ucf_players[1])
            ),
            format!(
                "- UCF dashback amendment frames consumed: P1 {}, P2 {}",
                self.ucf_dashback_amendment_frames[0], self.ucf_dashback_amendment_frames[1]
            ),
            format!("- Frames compared: {}", self.frames_compared),
            format!(
                "- Player frames compared: P1 {}, P2 {}",
                self.player_frames_compared[0], self.player_frames_compared[1]
            ),
            format!(
                "- Unsupported Melee states skipped: {}",
                self.unsupported_state_count
            ),
            format!("- State mismatches: {}", self.state_mismatch_count),
            format!(
                "- Max abs horizontal self velocity diff: P1 {}, P2 {}",
                self.max_abs_ground_velocity_diff[0], self.max_abs_ground_velocity_diff[1]
            ),
            String::new(),
        ]);

        if let Some(drift) = self.first_position_drift {
            lines.extend([
                "## First Significant Position Drift".to_string(),
                String::new(),
                format!("- Core frame: {}", drift.frame.0),
                format!("- Slippi frame: {}", drift.source_frame),
                format!("- Player: {}", drift.player_index + 1),
                format!(
                    "- Motion/action state: {}",
                    expected_state_label(
                        drift.expected_motion_state,
                        drift.expected_slippi_state_id,
                    )
                ),
                format!(
                    "- Position: Melee ({}, {}) vs Rust ({}, {})",
                    drift.expected_position.x,
                    drift.expected_position.y,
                    drift.actual_position.x,
                    drift.actual_position.y
                ),
                format!(
                    "- Position delta: Rust - Melee ({}, {})",
                    drift.actual_position.x - drift.expected_position.x,
                    drift.actual_position.y - drift.expected_position.y
                ),
                format!(
                    "- Ground velocity X: Melee {} vs Rust {}",
                    drift.expected_ground_velocity_x, drift.actual_velocity_x
                ),
                format!(
                    "- Air velocity X: Melee {} vs Rust {}",
                    drift.expected_air_velocity_x, drift.actual_velocity_x
                ),
                format!(
                    "- Velocity Y: Melee {} vs Rust {}",
                    drift.expected_velocity_y, drift.actual_velocity_y
                ),
                String::new(),
            ]);
        } else {
            lines.extend([
                "## First Significant Position Drift".to_string(),
                String::new(),
                format!(
                    "No matching-state position drift of at least {} milli-units found in this comparison window.",
                    SIGNIFICANT_POSITION_DRIFT_MILLI
                ),
                String::new(),
            ]);
        }

        if self
            .first_position_drift_by_player
            .iter()
            .any(Option::is_some)
        {
            lines.extend([
                "## First Significant Position Drift By Player".to_string(),
                String::new(),
                "| Player | Core frame | Slippi frame | State | Delta X | Delta Y |".to_string(),
                "| ---: | ---: | ---: | --- | ---: | ---: |".to_string(),
            ]);
            for (player_index, drift) in self.first_position_drift_by_player.iter().enumerate() {
                if let Some(drift) = drift {
                    lines.push(format!(
                        "| {} | {} | {} | {} | {} | {} |",
                        player_index + 1,
                        drift.frame.0,
                        drift.source_frame,
                        expected_state_label(
                            drift.expected_motion_state,
                            drift.expected_slippi_state_id,
                        ),
                        drift.actual_position.x - drift.expected_position.x,
                        drift.actual_position.y - drift.expected_position.y
                    ));
                }
            }
            lines.push(String::new());
        }

        if let Some(mismatch) = self.first_state_mismatch {
            lines.extend([
                "## First State Mismatch".to_string(),
                String::new(),
                format!("- Core frame: {}", mismatch.frame.0),
                format!("- Slippi frame: {}", mismatch.source_frame),
                format!("- Player: {}", mismatch.player_index + 1),
                format!(
                    "- Melee expected: {}",
                    expected_state_label(
                        mismatch.expected_motion_state,
                        mismatch.expected_slippi_state_id,
                    )
                ),
                format!("- Rust actual: {:?}", mismatch.actual_motion_state),
                format!(
                    "- Position: Melee ({}, {}) vs Rust ({}, {})",
                    mismatch.expected_position.x,
                    mismatch.expected_position.y,
                    mismatch.actual_position.x,
                    mismatch.actual_position.y
                ),
                format!(
                    "- Position delta: Rust - Melee ({}, {})",
                    mismatch.actual_position.x - mismatch.expected_position.x,
                    mismatch.actual_position.y - mismatch.expected_position.y
                ),
                format!(
                    "- Ground velocity X: Melee {} vs Rust {}",
                    mismatch.expected_ground_velocity_x, mismatch.actual_velocity_x
                ),
                format!(
                    "- Ground velocity X delta: Rust - Melee {}",
                    mismatch.actual_velocity_x - mismatch.expected_ground_velocity_x
                ),
                format!(
                    "- Air velocity X: Melee {} vs Rust {}",
                    mismatch.expected_air_velocity_x, mismatch.actual_velocity_x
                ),
                format!(
                    "- Air velocity X delta: Rust - Melee {}",
                    mismatch.actual_velocity_x - mismatch.expected_air_velocity_x
                ),
                format!(
                    "- Velocity Y: Melee {} vs Rust {}",
                    mismatch.expected_velocity_y, mismatch.actual_velocity_y
                ),
                format!(
                    "- Velocity Y delta: Rust - Melee {}",
                    mismatch.actual_velocity_y - mismatch.expected_velocity_y
                ),
                String::new(),
            ]);
        } else {
            lines.extend([
                "## First State Mismatch".to_string(),
                String::new(),
                "No mapped state mismatch found in this comparison window.".to_string(),
                String::new(),
            ]);
        }

        if !self.unsupported_states.is_empty() {
            lines.push("## Unsupported Melee States".to_string());
            lines.push(String::new());
            lines.push("| State | Count |".to_string());
            lines.push("| --- | ---: |".to_string());
            for (state_id, count) in self.unsupported_states.iter().take(12) {
                lines.push(format!(
                    "| {} ({}) | {} |",
                    slippi_action_state_name(*state_id),
                    state_id,
                    count
                ));
            }
            lines.push(String::new());
        }

        lines.push("## Interpretation".to_string());
        lines.push(String::new());
        lines.push(
            "If this report diverges at frame 0, the Rust world was not initialized from the Slippi pre-frame fighter state yet. Use later matching windows or add diagnostic initialization before treating that first mismatch as a mechanics bug."
                .to_string(),
        );
        lines.push(String::new());
        lines.join("\n")
    }
}

impl SlippiCoreTrace {
    pub fn report_markdown(&self) -> String {
        let mut lines = vec![
            "# Slippi Core Trace Window".to_string(),
            String::new(),
            "This report replays Slippi-exported game-facing inputs from match start and records a compact per-frame comparison window.".to_string(),
        ];
        if let Some(path) = &self.source_replay_path {
            lines.push(format!("- Source replay: `{path}`"));
        }
        lines.push(format!("- Player: {}", self.config.player_index + 1));
        lines.push(format!(
            "- Source frame window: {}..={}",
            self.config.source_frame_start, self.config.source_frame_end
        ));
        lines.push(String::new());
        lines.push("| Core | Source | Player | Stick X | Stick Y | Buttons | L | R | UCF DB | UCF SD | Expected | Actual | Actual Frame | Exp X | Act X | dX | Exp Y | Act Y | dY | Exp Gx | Exp Ax | Act Vx | Exp Vy | Act Vy |".to_string());
        lines.push("| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | :---: | :---: | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |".to_string());
        for row in &self.rows {
            let expected_name = row
                .expected_motion_state
                .map(|state| format!("{state:?}"))
                .unwrap_or_else(|| {
                    slippi_action_state_name(row.expected_slippi_state_id).to_string()
                });
            lines.push(format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} ({}) | {:?} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                row.core_frame.0,
                row.source_frame,
                row.player_index,
                row.input_stick_x,
                row.input_stick_y,
                row.input_button_bits,
                row.input_left_trigger,
                row.input_right_trigger,
                if row.input_ucf_dashback_amendment { "yes" } else { "no" },
                if row.input_ucf_shield_drop_amendment { "yes" } else { "no" },
                expected_name,
                row.expected_slippi_state_id,
                row.actual_motion_state,
                row.actual_motion_frame,
                row.expected_position.x,
                row.actual_position.x,
                row.actual_position.x - row.expected_position.x,
                row.expected_position.y,
                row.actual_position.y,
                row.actual_position.y - row.expected_position.y,
                row.expected_ground_velocity_x,
                row.expected_air_velocity_x,
                row.actual_velocity_x,
                row.expected_velocity_y,
                row.actual_velocity_y
            ));
        }
        if self.rows.is_empty() {
            lines.push(String::new());
            lines.push("No comparable rows were found in the requested window.".to_string());
        }
        lines.join("\n")
    }
}

#[derive(Debug)]
pub enum SlippiCoreDiagnosticError {
    Json(serde_json::Error),
    Io(io::Error),
}

impl std::fmt::Display for SlippiCoreDiagnosticError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json(error) => write!(f, "invalid Slippi export JSON: {error}"),
            Self::Io(error) => write!(f, "Slippi core diagnostic IO error: {error}"),
        }
    }
}

impl std::error::Error for SlippiCoreDiagnosticError {}

impl From<serde_json::Error> for SlippiCoreDiagnosticError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

impl From<io::Error> for SlippiCoreDiagnosticError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

pub fn compare_slippi_export_with_core(
    text: &str,
    config: SlippiCoreComparisonConfig,
) -> Result<SlippiCoreComparison, SlippiCoreDiagnosticError> {
    let export: SlippiExport = serde_json::from_str(text)?;
    let mut comparison =
        empty_comparison(&export, SlippiCoreComparisonMode::SeededPreFrameDiagnostic);
    let frame_limit = config.max_frames.unwrap_or(usize::MAX);
    let mut previous_inputs = [PlayerInput::neutral(); PLAYER_COUNT];
    let mut input_timers = [MeleeInputTimers::expired(); PLAYER_COUNT];
    let mut previous_posts: [Option<SlippiPostFrame>; PLAYER_COUNT] = [None, None];
    let mut unsupported_states = HashMap::<u16, usize>::new();

    for frame in export.frames.into_iter().take(frame_limit) {
        let mut world = World::for_two_players();
        let mut inputs = [PlayerInput::neutral(); PLAYER_COUNT];
        let mut raw_inputs = [SlippiRustPlayerInput::default(); PLAYER_COUNT];
        let mut comparable_players = [false; PLAYER_COUNT];
        for player_index in 0..PLAYER_COUNT {
            if let Some(player_frame) = frame.players.get(&player_index.to_string()) {
                if let Some(pre) = &player_frame.pre {
                    let input = effective_slippi_player_input(pre);
                    raw_inputs[player_index] = input;
                    if input.ucf_dashback_amendment {
                        comparison.ucf_dashback_amendment_frames[player_index] += 1;
                    }
                    inputs[player_index] = input.to_player_input();
                    if let Some(state) = player_state_from_slippi_pre(
                        pre,
                        previous_posts[player_index].as_ref(),
                        &mut unsupported_states,
                    ) {
                        world.set_player_state_for_diagnostic(player_index, state);
                        comparable_players[player_index] = true;
                    } else {
                        comparison.unsupported_state_count += 1;
                    }
                }
            }
        }

        world.set_input_history_for_diagnostic(previous_inputs, input_timers);
        step_world(&mut world, diagnostic_core_frame(frame.frame), &inputs);
        let snapshot = world.snapshot();
        input_timers = *world.input_timers();
        comparison.frames_compared += 1;

        for player_index in 0..PLAYER_COUNT {
            if !config.compare_players[player_index] {
                continue;
            }
            let Some(player_frame) = frame.players.get(&player_index.to_string()) else {
                continue;
            };
            let Some(post) = &player_frame.post else {
                continue;
            };
            previous_posts[player_index] = Some(post.clone());
            if !comparable_players[player_index] {
                continue;
            }
            let Some(expected_state) = slippi_action_state_to_expected(post.action_state_id) else {
                comparison.unsupported_state_count += 1;
                *unsupported_states.entry(post.action_state_id).or_default() += 1;
                continue;
            };

            comparison.player_frames_compared[player_index] += 1;
            let actual_player = snapshot.players[player_index];
            record_first_classified_divergence(
                &mut comparison,
                diagnostic_core_frame(frame.frame),
                frame.frame,
                player_index,
                raw_inputs[player_index],
                post,
                actual_player,
                world.stage(),
            );
            let actual = actual_player.motion_state;
            let actual_action_state_id = actual_player.melee_action_state_id;
            let states_match =
                slippi_expected_state_matches(expected_state, actual, actual_action_state_id);
            let actual_position = snapshot.players[player_index].position;
            let expected_position = slippi_post_position_to_core_milli(
                post,
                world.stage(),
                actual_player.source_coll_floor_surface_index,
                actual_position.y,
            );
            let expected_ground_velocity = post
                .self_induced_speeds
                .as_ref()
                .map(|speeds| slippi_units_to_core_milli(speeds.ground_x))
                .unwrap_or_default();
            let expected_air_velocity = post
                .self_induced_speeds
                .as_ref()
                .map(|speeds| slippi_units_to_core_milli(speeds.air_x))
                .unwrap_or_default();
            let expected_velocity_y = post
                .self_induced_speeds
                .as_ref()
                .map(|speeds| slippi_units_to_core_milli(speeds.y))
                .unwrap_or_default();
            let expected_attack_velocity_x = post
                .self_induced_speeds
                .as_ref()
                .map(|speeds| slippi_units_to_core_milli(speeds.attack_x))
                .unwrap_or_default();
            let expected_attack_velocity_y = post
                .self_induced_speeds
                .as_ref()
                .map(|speeds| slippi_units_to_core_milli(speeds.attack_y))
                .unwrap_or_default();
            let actual_velocity = snapshot.players[player_index].velocity.x;
            let actual_velocity_y = snapshot.players[player_index].velocity.y;
            let expected_grounded_for_velocity =
                expected_velocity_grounded(post, Some(expected_state), actual_player.grounded);
            let expected_horizontal_velocity = expected_base_horizontal_velocity_x(
                expected_ground_velocity,
                expected_air_velocity,
                expected_grounded_for_velocity,
            );
            let expected_horizontal_velocity = expected_composed_horizontal_velocity_x(
                Some(expected_state),
                expected_horizontal_velocity,
                expected_attack_velocity_x,
            );
            let expected_composed_velocity_y = expected_composed_velocity_y(
                Some(expected_state),
                expected_velocity_y,
                expected_attack_velocity_y,
            );
            let velocity_diff = (expected_horizontal_velocity - actual_velocity)
                .abs()
                .max((expected_composed_velocity_y - actual_velocity_y).abs());
            comparison.max_abs_ground_velocity_diff[player_index] =
                comparison.max_abs_ground_velocity_diff[player_index].max(velocity_diff);
            record_first_position_drift(
                &mut comparison,
                diagnostic_core_frame(frame.frame),
                frame.frame,
                player_index,
                post.action_state_id,
                expected_state.action_state_id,
                actual_action_state_id,
                expected_state.motion_state,
                actual,
                states_match,
                expected_position,
                actual_position,
                expected_ground_velocity,
                expected_air_velocity,
                expected_velocity_y,
                actual_velocity,
                actual_velocity_y,
            );

            if !states_match {
                comparison.state_mismatch_count += 1;
                comparison
                    .first_state_mismatch
                    .get_or_insert(SlippiCoreMismatch {
                        frame: diagnostic_core_frame(frame.frame),
                        source_frame: frame.frame,
                        player_index,
                        expected_slippi_state_id: post.action_state_id,
                        expected_action_state_id: expected_state.action_state_id,
                        actual_action_state_id,
                        expected_motion_state: expected_state.motion_state,
                        actual_motion_state: actual,
                        expected_position,
                        actual_position,
                        expected_ground_velocity_x: expected_ground_velocity,
                        expected_air_velocity_x: expected_air_velocity,
                        expected_velocity_y,
                        actual_velocity_x: actual_velocity,
                        actual_velocity_y,
                    });
            }
        }
        previous_inputs = inputs;
    }

    comparison.unsupported_states = unsupported_states.into_iter().collect();
    comparison
        .unsupported_states
        .sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    Ok(comparison)
}

pub fn compare_slippi_export_from_match_start_with_core(
    text: &str,
    config: SlippiCoreComparisonConfig,
) -> Result<SlippiCoreComparison, SlippiCoreDiagnosticError> {
    let mut export: SlippiExport = serde_json::from_str(text)?;
    let mut comparison = empty_comparison(&export, SlippiCoreComparisonMode::SequentialMatchStart);
    let frame_limit = config.max_frames.unwrap_or(usize::MAX);
    let mut unsupported_states = HashMap::<u16, usize>::new();
    let mut world = world_for_slippi_export(&export);
    export.frames.sort_by_key(|frame| frame.frame);

    for (core_frame_index, frame) in export.frames.into_iter().take(frame_limit).enumerate() {
        let mut inputs = [PlayerInput::neutral(); PLAYER_COUNT];
        let mut raw_inputs = [SlippiRustPlayerInput::default(); PLAYER_COUNT];
        for (player_index, input_slot) in inputs.iter_mut().enumerate() {
            if let Some(pre) = frame
                .players
                .get(&player_index.to_string())
                .and_then(|player_frame| player_frame.pre.as_ref())
            {
                let input = effective_slippi_player_input(pre);
                raw_inputs[player_index] = input;
                if input.ucf_dashback_amendment {
                    comparison.ucf_dashback_amendment_frames[player_index] += 1;
                }
                *input_slot = input.to_player_input();
            }
        }

        let core_frame = Frame(core_frame_index as u32);
        step_match_start_replay_world(&mut world, core_frame, &inputs);
        let snapshot = world.snapshot();
        comparison.frames_compared += 1;

        for player_index in 0..PLAYER_COUNT {
            if !config.compare_players[player_index] {
                continue;
            }
            let Some(player_frame) = frame.players.get(&player_index.to_string()) else {
                continue;
            };
            let Some(post) = &player_frame.post else {
                continue;
            };
            let Some(expected_state) = slippi_action_state_to_expected(post.action_state_id) else {
                comparison.unsupported_state_count += 1;
                *unsupported_states.entry(post.action_state_id).or_default() += 1;
                continue;
            };

            comparison.player_frames_compared[player_index] += 1;
            let actual_player = snapshot.players[player_index];
            record_first_classified_divergence(
                &mut comparison,
                core_frame,
                frame.frame,
                player_index,
                raw_inputs[player_index],
                post,
                actual_player,
                world.stage(),
            );
            let actual = actual_player.motion_state;
            let actual_action_state_id = actual_player.melee_action_state_id;
            let states_match =
                slippi_expected_state_matches(expected_state, actual, actual_action_state_id);
            let actual_position = snapshot.players[player_index].position;
            let expected_position = slippi_post_position_to_core_milli(
                post,
                world.stage(),
                actual_player.source_coll_floor_surface_index,
                actual_position.y,
            );
            let expected_ground_velocity = post
                .self_induced_speeds
                .as_ref()
                .map(|speeds| slippi_units_to_core_milli(speeds.ground_x))
                .unwrap_or_default();
            let expected_air_velocity = post
                .self_induced_speeds
                .as_ref()
                .map(|speeds| slippi_units_to_core_milli(speeds.air_x))
                .unwrap_or_default();
            let expected_velocity_y = post
                .self_induced_speeds
                .as_ref()
                .map(|speeds| slippi_units_to_core_milli(speeds.y))
                .unwrap_or_default();
            let expected_attack_velocity_x = post
                .self_induced_speeds
                .as_ref()
                .map(|speeds| slippi_units_to_core_milli(speeds.attack_x))
                .unwrap_or_default();
            let expected_attack_velocity_y = post
                .self_induced_speeds
                .as_ref()
                .map(|speeds| slippi_units_to_core_milli(speeds.attack_y))
                .unwrap_or_default();
            let actual_velocity = snapshot.players[player_index].velocity.x;
            let actual_velocity_y = snapshot.players[player_index].velocity.y;
            let expected_grounded_for_velocity =
                expected_velocity_grounded(post, Some(expected_state), actual_player.grounded);
            let expected_horizontal_velocity = expected_base_horizontal_velocity_x(
                expected_ground_velocity,
                expected_air_velocity,
                expected_grounded_for_velocity,
            );
            let expected_horizontal_velocity = expected_composed_horizontal_velocity_x(
                Some(expected_state),
                expected_horizontal_velocity,
                expected_attack_velocity_x,
            );
            let expected_composed_velocity_y = expected_composed_velocity_y(
                Some(expected_state),
                expected_velocity_y,
                expected_attack_velocity_y,
            );
            let velocity_diff = (expected_horizontal_velocity - actual_velocity)
                .abs()
                .max((expected_composed_velocity_y - actual_velocity_y).abs());
            comparison.max_abs_ground_velocity_diff[player_index] =
                comparison.max_abs_ground_velocity_diff[player_index].max(velocity_diff);
            record_first_position_drift(
                &mut comparison,
                core_frame,
                frame.frame,
                player_index,
                post.action_state_id,
                expected_state.action_state_id,
                actual_action_state_id,
                expected_state.motion_state,
                actual,
                states_match,
                expected_position,
                actual_position,
                expected_ground_velocity,
                expected_air_velocity,
                expected_velocity_y,
                actual_velocity,
                actual_velocity_y,
            );

            if !states_match {
                comparison.state_mismatch_count += 1;
                comparison
                    .first_state_mismatch
                    .get_or_insert(SlippiCoreMismatch {
                        frame: core_frame,
                        source_frame: frame.frame,
                        player_index,
                        expected_slippi_state_id: post.action_state_id,
                        expected_action_state_id: expected_state.action_state_id,
                        actual_action_state_id,
                        expected_motion_state: expected_state.motion_state,
                        actual_motion_state: actual,
                        expected_position,
                        actual_position,
                        expected_ground_velocity_x: expected_ground_velocity,
                        expected_air_velocity_x: expected_air_velocity,
                        expected_velocity_y,
                        actual_velocity_x: actual_velocity,
                        actual_velocity_y,
                    });
            }
        }
    }

    comparison.unsupported_states = unsupported_states.into_iter().collect();
    comparison
        .unsupported_states
        .sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    Ok(comparison)
}

pub fn trace_slippi_export_from_match_start_with_core(
    text: &str,
    config: SlippiCoreTraceConfig,
) -> Result<SlippiCoreTrace, SlippiCoreDiagnosticError> {
    let mut export: SlippiExport = serde_json::from_str(text)?;
    let source_replay_path = export
        .source
        .as_ref()
        .and_then(|source| source.replay_path.clone());
    let frame_limit = config.max_frames.unwrap_or(usize::MAX);
    let mut world = world_for_slippi_export(&export);
    let mut rows = Vec::new();
    export.frames.sort_by_key(|frame| frame.frame);

    for (core_frame_index, frame) in export.frames.into_iter().enumerate() {
        let mut inputs = [PlayerInput::neutral(); PLAYER_COUNT];
        let mut raw_inputs = [SlippiRustPlayerInput::default(); PLAYER_COUNT];
        for (player_index, input_slot) in inputs.iter_mut().enumerate() {
            if let Some(pre) = frame
                .players
                .get(&player_index.to_string())
                .and_then(|player_frame| player_frame.pre.as_ref())
            {
                let input = effective_slippi_player_input(pre);
                raw_inputs[player_index] = input;
                *input_slot = input.to_player_input();
            }
        }

        let core_frame = Frame(core_frame_index as u32);
        step_match_start_replay_world(&mut world, core_frame, &inputs);
        if frame.frame < config.source_frame_start || frame.frame > config.source_frame_end {
            continue;
        }
        if config.player_index >= PLAYER_COUNT {
            continue;
        }
        let Some(player_frame) = frame.players.get(&config.player_index.to_string()) else {
            continue;
        };
        let Some(post) = &player_frame.post else {
            continue;
        };
        let snapshot = world.snapshot();
        let expected_position = slippi_post_position_to_core_milli(
            post,
            world.stage(),
            snapshot.players[config.player_index].source_coll_floor_surface_index,
            snapshot.players[config.player_index].position.y,
        );
        let expected_source_position = post
            .position
            .map(|position| SourceVec2 {
                x: slippi_units_to_source_f32(Some(position[0])),
                y: slippi_units_to_source_f32(Some(position[1])),
            })
            .unwrap_or_default();
        let expected_ground_velocity_x = post
            .self_induced_speeds
            .as_ref()
            .map(|speeds| slippi_units_to_core_milli(speeds.ground_x))
            .unwrap_or_default();
        let expected_ground_velocity_x_source = post
            .self_induced_speeds
            .as_ref()
            .map(|speeds| slippi_units_to_source_f32(speeds.ground_x))
            .unwrap_or_default();
        let expected_air_velocity_x = post
            .self_induced_speeds
            .as_ref()
            .map(|speeds| slippi_units_to_core_milli(speeds.air_x))
            .unwrap_or_default();
        let expected_air_velocity_x_source = post
            .self_induced_speeds
            .as_ref()
            .map(|speeds| slippi_units_to_source_f32(speeds.air_x))
            .unwrap_or_default();
        let expected_velocity_y = post
            .self_induced_speeds
            .as_ref()
            .map(|speeds| slippi_units_to_core_milli(speeds.y))
            .unwrap_or_default();
        let expected_velocity_y_source = post
            .self_induced_speeds
            .as_ref()
            .map(|speeds| slippi_units_to_source_f32(speeds.y))
            .unwrap_or_default();
        let expected_attack_velocity_x = post
            .self_induced_speeds
            .as_ref()
            .map(|speeds| slippi_units_to_core_milli(speeds.attack_x))
            .unwrap_or_default();
        let expected_attack_velocity_x_source = post
            .self_induced_speeds
            .as_ref()
            .map(|speeds| slippi_units_to_source_f32(speeds.attack_x))
            .unwrap_or_default();
        let expected_attack_velocity_y = post
            .self_induced_speeds
            .as_ref()
            .map(|speeds| slippi_units_to_core_milli(speeds.attack_y))
            .unwrap_or_default();
        let expected_attack_velocity_y_source = post
            .self_induced_speeds
            .as_ref()
            .map(|speeds| slippi_units_to_source_f32(speeds.attack_y))
            .unwrap_or_default();
        let player = snapshot.players[config.player_index];
        let actual_source_floor_surface = player.source_coll_floor_surface_index;
        let actual_source_floor_line = player.source_coll_floor_line_index;
        let raw_input = raw_inputs[config.player_index];
        let expected_state = slippi_action_state_to_expected(post.action_state_id);
        let expected_facing = slippi_facing_to_i8(post.facing);
        let expected_grounded_for_velocity =
            expected_velocity_grounded(post, expected_state, player.grounded);
        let expected_base_horizontal_velocity_x = expected_base_horizontal_velocity_x(
            expected_ground_velocity_x,
            expected_air_velocity_x,
            expected_grounded_for_velocity,
        );
        let expected_composed_velocity_x = expected_composed_horizontal_velocity_x(
            expected_state,
            expected_base_horizontal_velocity_x,
            expected_attack_velocity_x,
        );
        let expected_composed_velocity_y = expected_composed_velocity_y(
            expected_state,
            expected_velocity_y,
            expected_attack_velocity_y,
        );
        let expected_action_identity =
            slippi_action_identity_for_expected_state(expected_state, post.action_state_id);
        let actual_action_identity = slippi_action_identity_for_player(&player);
        rows.push(SlippiCoreTraceRow {
            core_frame,
            source_frame: frame.frame,
            player_index: config.player_index,
            input_stick_x: raw_input.stick_x,
            input_stick_y: raw_input.stick_y,
            input_button_bits: raw_input.physical_button_bits,
            input_left_trigger: raw_input.left_trigger,
            input_right_trigger: raw_input.right_trigger,
            input_ucf_dashback_amendment: raw_input.ucf_dashback_amendment,
            input_ucf_shield_drop_amendment: raw_input.ucf_shield_drop_amendment,
            actual_input_jump_pressed: player.debug_input_facts.jump_pressed,
            actual_input_normal_jump_pressed: player.debug_input_facts.normal_jump_pressed,
            actual_input_shield_held: player.debug_input_facts.shield_held,
            actual_input_shield_pressed: player.debug_input_facts.shield_pressed,
            expected_slippi_state_id: post.action_state_id,
            expected_action_state_id: expected_state
                .map(|state| state.action_state_id)
                .unwrap_or_else(|| MeleeActionStateId::new(post.action_state_id)),
            actual_action_state_id: player.melee_action_state_id,
            expected_action_identity,
            actual_action_identity,
            expected_motion_state: expected_state.and_then(|state| state.motion_state),
            actual_motion_state: player.motion_state,
            actual_grounded: player.grounded,
            actual_motion_frame: player.state_frame,
            actual_motion_anim_frame_milli: player.animation_frame_milli,
            actual_source_fall_anim_blend: player.source_fall_anim_blend,
            actual_source_fall_anim_pose: player.source_fall_anim_pose,
            expected_facing,
            actual_facing: player.facing,
            expected_position,
            actual_position: player.position,
            expected_source_position,
            actual_source_position: player.source_position,
            expected_ground_velocity_x,
            expected_air_velocity_x,
            expected_velocity_y,
            expected_attack_velocity_x,
            expected_attack_velocity_y,
            expected_composed_velocity_x,
            expected_composed_velocity_y,
            expected_ground_velocity_x_source,
            expected_air_velocity_x_source,
            expected_velocity_y_source,
            expected_attack_velocity_x_source,
            expected_attack_velocity_y_source,
            actual_velocity_x: player.velocity.x,
            actual_velocity_y: player.velocity.y,
            actual_source_self_velocity_x: player.source_self_velocity_x,
            actual_source_self_velocity_y: player.source_self_velocity_y,
            actual_source_knockback_velocity_x: player.source_knockback_velocity_x,
            actual_source_knockback_velocity_y: player.source_knockback_velocity_y,
            actual_source_ground_knockback_velocity: player.source_ground_knockback_velocity,
            actual_player_nudge_x: player.player_nudge_x,
            actual_player_nudge_z: player.player_nudge_z,
            actual_ground_velocity_x: player.ground_velocity_x,
            actual_ground_accel_x: player.ground_accel_x,
            actual_ground_accel_x2: player.ground_accel_x2,
            actual_dash_entry_velocity_delta: player.dash_entry_velocity_delta,
            actual_dash_x0: player.dash_x0,
            actual_source_coll_last_pos: player.source_coll_last_pos,
            actual_source_coll_cur_pos: player.source_coll_cur_pos,
            actual_source_coll_prev_pos: player.source_coll_prev_pos,
            actual_source_coll_ecb: player.source_coll_ecb,
            actual_source_coll_prev_ecb: player.source_coll_prev_ecb,
            actual_source_coll_desired_ecb: player.source_coll_desired_ecb,
            actual_ecb_bottom_lock_timer: player.ecb_bottom_lock_timer,
            actual_source_coll_x130_locked: player.source_coll_x130_locked,
            actual_floor_skip_surface: player.floor_skip_surface,
            actual_source_floor_skip_line: player.source_coll_floor_skip_line_index,
            actual_source_floor_surface,
            actual_source_floor_line,
            actual_source_coll_env_flags: player.source_coll_env_flags,
            actual_source_coll_prev_env_flags: player.source_coll_prev_env_flags,
        });
        if rows.len() >= frame_limit {
            break;
        }
    }

    Ok(SlippiCoreTrace {
        source_replay_path,
        config,
        rows,
    })
}

pub fn trace_slippi_export_seeded_pre_frame_with_core(
    text: &str,
    config: SlippiCoreTraceConfig,
) -> Result<SlippiCoreTrace, SlippiCoreDiagnosticError> {
    let mut export: SlippiExport = serde_json::from_str(text)?;
    let source_replay_path = export
        .source
        .as_ref()
        .and_then(|source| source.replay_path.clone());
    let frame_limit = config.max_frames.unwrap_or(usize::MAX);
    let mut previous_inputs = [PlayerInput::neutral(); PLAYER_COUNT];
    let mut input_timers = [MeleeInputTimers::expired(); PLAYER_COUNT];
    let mut previous_posts: [Option<SlippiPostFrame>; PLAYER_COUNT] = [None, None];
    let mut unsupported_states = HashMap::<u16, usize>::new();
    let mut rows = Vec::new();
    export.frames.sort_by_key(|frame| frame.frame);

    for frame in export.frames.into_iter() {
        let mut world = World::for_two_players();
        let mut inputs = [PlayerInput::neutral(); PLAYER_COUNT];
        let mut raw_inputs = [SlippiRustPlayerInput::default(); PLAYER_COUNT];
        for player_index in 0..PLAYER_COUNT {
            if let Some(player_frame) = frame.players.get(&player_index.to_string()) {
                if let Some(pre) = &player_frame.pre {
                    let input = effective_slippi_player_input(pre);
                    raw_inputs[player_index] = input;
                    inputs[player_index] = input.to_player_input();
                    if let Some(state) = player_state_from_slippi_pre(
                        pre,
                        previous_posts[player_index].as_ref(),
                        &mut unsupported_states,
                    ) {
                        world.set_player_state_for_diagnostic(player_index, state);
                    }
                }
            }
        }

        world.set_input_history_for_diagnostic(previous_inputs, input_timers);
        let core_frame = diagnostic_core_frame(frame.frame);
        step_world(&mut world, core_frame, &inputs);
        let snapshot = world.snapshot();
        input_timers = *world.input_timers();

        for player_index in 0..PLAYER_COUNT {
            if let Some(post) = frame
                .players
                .get(&player_index.to_string())
                .and_then(|player_frame| player_frame.post.as_ref())
            {
                previous_posts[player_index] = Some(post.clone());
            }
        }
        previous_inputs = inputs;

        if frame.frame < config.source_frame_start || frame.frame > config.source_frame_end {
            continue;
        }
        if config.player_index >= PLAYER_COUNT {
            continue;
        }
        let Some(player_frame) = frame.players.get(&config.player_index.to_string()) else {
            continue;
        };
        let Some(post) = &player_frame.post else {
            continue;
        };
        let expected_position = slippi_post_position_to_core_milli(
            post,
            world.stage(),
            snapshot.players[config.player_index].source_coll_floor_surface_index,
            snapshot.players[config.player_index].position.y,
        );
        let expected_source_position = post
            .position
            .map(|position| SourceVec2 {
                x: slippi_units_to_source_f32(Some(position[0])),
                y: slippi_units_to_source_f32(Some(position[1])),
            })
            .unwrap_or_default();
        let expected_ground_velocity_x = post
            .self_induced_speeds
            .as_ref()
            .map(|speeds| slippi_units_to_core_milli(speeds.ground_x))
            .unwrap_or_default();
        let expected_ground_velocity_x_source = post
            .self_induced_speeds
            .as_ref()
            .map(|speeds| slippi_units_to_source_f32(speeds.ground_x))
            .unwrap_or_default();
        let expected_air_velocity_x = post
            .self_induced_speeds
            .as_ref()
            .map(|speeds| slippi_units_to_core_milli(speeds.air_x))
            .unwrap_or_default();
        let expected_air_velocity_x_source = post
            .self_induced_speeds
            .as_ref()
            .map(|speeds| slippi_units_to_source_f32(speeds.air_x))
            .unwrap_or_default();
        let expected_velocity_y = post
            .self_induced_speeds
            .as_ref()
            .map(|speeds| slippi_units_to_core_milli(speeds.y))
            .unwrap_or_default();
        let expected_velocity_y_source = post
            .self_induced_speeds
            .as_ref()
            .map(|speeds| slippi_units_to_source_f32(speeds.y))
            .unwrap_or_default();
        let expected_attack_velocity_x = post
            .self_induced_speeds
            .as_ref()
            .map(|speeds| slippi_units_to_core_milli(speeds.attack_x))
            .unwrap_or_default();
        let expected_attack_velocity_x_source = post
            .self_induced_speeds
            .as_ref()
            .map(|speeds| slippi_units_to_source_f32(speeds.attack_x))
            .unwrap_or_default();
        let expected_attack_velocity_y = post
            .self_induced_speeds
            .as_ref()
            .map(|speeds| slippi_units_to_core_milli(speeds.attack_y))
            .unwrap_or_default();
        let expected_attack_velocity_y_source = post
            .self_induced_speeds
            .as_ref()
            .map(|speeds| slippi_units_to_source_f32(speeds.attack_y))
            .unwrap_or_default();
        let player = snapshot.players[config.player_index];
        let actual_source_floor_surface = player.source_coll_floor_surface_index;
        let actual_source_floor_line = player.source_coll_floor_line_index;
        let raw_input = raw_inputs[config.player_index];
        let expected_state = slippi_action_state_to_expected(post.action_state_id);
        let expected_facing = slippi_facing_to_i8(post.facing);
        let expected_grounded_for_velocity =
            expected_velocity_grounded(post, expected_state, player.grounded);
        let expected_base_horizontal_velocity_x = expected_base_horizontal_velocity_x(
            expected_ground_velocity_x,
            expected_air_velocity_x,
            expected_grounded_for_velocity,
        );
        let expected_composed_velocity_x = expected_composed_horizontal_velocity_x(
            expected_state,
            expected_base_horizontal_velocity_x,
            expected_attack_velocity_x,
        );
        let expected_composed_velocity_y = expected_composed_velocity_y(
            expected_state,
            expected_velocity_y,
            expected_attack_velocity_y,
        );
        let expected_action_identity =
            slippi_action_identity_for_expected_state(expected_state, post.action_state_id);
        let actual_action_identity = slippi_action_identity_for_player(&player);
        rows.push(SlippiCoreTraceRow {
            core_frame,
            source_frame: frame.frame,
            player_index: config.player_index,
            input_stick_x: raw_input.stick_x,
            input_stick_y: raw_input.stick_y,
            input_button_bits: raw_input.physical_button_bits,
            input_left_trigger: raw_input.left_trigger,
            input_right_trigger: raw_input.right_trigger,
            input_ucf_dashback_amendment: raw_input.ucf_dashback_amendment,
            input_ucf_shield_drop_amendment: raw_input.ucf_shield_drop_amendment,
            actual_input_jump_pressed: player.debug_input_facts.jump_pressed,
            actual_input_normal_jump_pressed: player.debug_input_facts.normal_jump_pressed,
            actual_input_shield_held: player.debug_input_facts.shield_held,
            actual_input_shield_pressed: player.debug_input_facts.shield_pressed,
            expected_slippi_state_id: post.action_state_id,
            expected_action_state_id: expected_state
                .map(|state| state.action_state_id)
                .unwrap_or_else(|| MeleeActionStateId::new(post.action_state_id)),
            actual_action_state_id: player.melee_action_state_id,
            expected_action_identity,
            actual_action_identity,
            expected_motion_state: expected_state.and_then(|state| state.motion_state),
            actual_motion_state: player.motion_state,
            actual_grounded: player.grounded,
            actual_motion_frame: player.state_frame,
            actual_motion_anim_frame_milli: player.animation_frame_milli,
            actual_source_fall_anim_blend: player.source_fall_anim_blend,
            actual_source_fall_anim_pose: player.source_fall_anim_pose,
            expected_facing,
            actual_facing: player.facing,
            expected_position,
            actual_position: player.position,
            expected_source_position,
            actual_source_position: player.source_position,
            expected_ground_velocity_x,
            expected_air_velocity_x,
            expected_velocity_y,
            expected_attack_velocity_x,
            expected_attack_velocity_y,
            expected_composed_velocity_x,
            expected_composed_velocity_y,
            expected_ground_velocity_x_source,
            expected_air_velocity_x_source,
            expected_velocity_y_source,
            expected_attack_velocity_x_source,
            expected_attack_velocity_y_source,
            actual_velocity_x: player.velocity.x,
            actual_velocity_y: player.velocity.y,
            actual_source_self_velocity_x: player.source_self_velocity_x,
            actual_source_self_velocity_y: player.source_self_velocity_y,
            actual_source_knockback_velocity_x: player.source_knockback_velocity_x,
            actual_source_knockback_velocity_y: player.source_knockback_velocity_y,
            actual_source_ground_knockback_velocity: player.source_ground_knockback_velocity,
            actual_player_nudge_x: player.player_nudge_x,
            actual_player_nudge_z: player.player_nudge_z,
            actual_ground_velocity_x: player.ground_velocity_x,
            actual_ground_accel_x: player.ground_accel_x,
            actual_ground_accel_x2: player.ground_accel_x2,
            actual_dash_entry_velocity_delta: player.dash_entry_velocity_delta,
            actual_dash_x0: player.dash_x0,
            actual_source_coll_last_pos: player.source_coll_last_pos,
            actual_source_coll_cur_pos: player.source_coll_cur_pos,
            actual_source_coll_prev_pos: player.source_coll_prev_pos,
            actual_source_coll_ecb: player.source_coll_ecb,
            actual_source_coll_prev_ecb: player.source_coll_prev_ecb,
            actual_source_coll_desired_ecb: player.source_coll_desired_ecb,
            actual_ecb_bottom_lock_timer: player.ecb_bottom_lock_timer,
            actual_source_coll_x130_locked: player.source_coll_x130_locked,
            actual_floor_skip_surface: player.floor_skip_surface,
            actual_source_floor_skip_line: player.source_coll_floor_skip_line_index,
            actual_source_floor_surface,
            actual_source_floor_line,
            actual_source_coll_env_flags: player.source_coll_env_flags,
            actual_source_coll_prev_env_flags: player.source_coll_prev_env_flags,
        });
        if rows.len() >= frame_limit {
            break;
        }
    }

    Ok(SlippiCoreTrace {
        source_replay_path,
        config,
        rows,
    })
}

pub fn slippi_visual_replay_inputs_from_match_start(
    text: &str,
    max_frames: Option<usize>,
) -> Result<Vec<SlippiVisualReplayFrame>, SlippiCoreDiagnosticError> {
    let mut export: SlippiExport = serde_json::from_str(text)?;
    export.frames.sort_by_key(|frame| frame.frame);
    let frame_limit = max_frames.unwrap_or(usize::MAX);
    let mut frames = Vec::new();

    for (core_frame_index, frame) in export.frames.into_iter().take(frame_limit).enumerate() {
        let mut inputs = [PlayerInput::neutral(); PLAYER_COUNT];
        let mut raw_inputs = [SlippiRustPlayerInput::default(); PLAYER_COUNT];
        let mut expected_players: [Option<SlippiPostFrame>; PLAYER_COUNT] = [None, None];
        for (player_index, input_slot) in inputs.iter_mut().enumerate() {
            if let Some(pre) = frame
                .players
                .get(&player_index.to_string())
                .and_then(|player_frame| player_frame.pre.as_ref())
            {
                let input = effective_slippi_player_input(pre);
                raw_inputs[player_index] = input;
                *input_slot = input.to_player_input();
            }
            expected_players[player_index] = frame
                .players
                .get(&player_index.to_string())
                .and_then(|player_frame| player_frame.post.as_ref())
                .cloned();
        }
        frames.push(SlippiVisualReplayFrame {
            core_frame: Frame(core_frame_index as u32),
            source_frame: frame.frame,
            inputs,
            raw_inputs,
            expected_players,
        });
    }

    Ok(frames)
}

pub fn slippi_match_start_world_from_export(
    text: &str,
) -> Result<World, SlippiCoreDiagnosticError> {
    let export: SlippiExport = serde_json::from_str(text)?;
    Ok(world_for_slippi_export(&export))
}

pub fn slippi_visual_replay_divergence_for_frame(
    frame: &SlippiVisualReplayFrame,
    snapshot: crate::WorldSnapshot,
    config: SlippiCoreDivergenceScanConfig,
) -> Option<SlippiVisualReplayDivergence> {
    for player_index in 0..PLAYER_COUNT {
        if !config.compare_players[player_index] {
            continue;
        }
        let Some(post) = frame.expected_players[player_index].as_ref() else {
            continue;
        };
        let (row, status) = trace_row_and_divergence_status(
            frame.core_frame,
            frame.source_frame,
            player_index,
            frame.raw_inputs[player_index],
            post,
            snapshot.players[player_index],
            snapshot.stage,
            config.position_tolerance_milli,
            config.velocity_tolerance_milli,
        );
        if let Some(status) = status {
            return Some(SlippiVisualReplayDivergence {
                kind: status.kind,
                player_index,
                core_frame: frame.core_frame,
                source_frame: frame.source_frame,
                max_abs_position_delta_milli: status.max_abs_position_delta_milli,
                max_abs_velocity_delta_milli: status.max_abs_velocity_delta_milli,
                row,
            });
        }
    }
    None
}

fn visual_replay_should_stop_on_divergence_kind(kind: SlippiCoreDivergenceKind) -> bool {
    match kind {
        SlippiCoreDivergenceKind::UnsupportedState
        | SlippiCoreDivergenceKind::MixedPhaseWitness
        | SlippiCoreDivergenceKind::StateMismatch
        | SlippiCoreDivergenceKind::PositionDrift
        | SlippiCoreDivergenceKind::VelocityDrift => true,
    }
}

fn visual_replay_should_stop_on_divergence(divergence: &SlippiVisualReplayDivergence) -> bool {
    if divergence.kind == SlippiCoreDivergenceKind::MixedPhaseWitness
        && (is_same_state_floor_commit_velocity_mixed_phase_witness(&divergence.row)
            || is_same_state_composed_velocity_channel_mixed_phase_witness(&divergence.row))
    {
        return false;
    }
    visual_replay_should_stop_on_divergence_kind(divergence.kind)
}

pub fn scan_slippi_export_from_match_start_with_core(
    text: &str,
    config: SlippiCoreDivergenceScanConfig,
) -> Result<SlippiCoreDivergenceScan, SlippiCoreDiagnosticError> {
    let mut export: SlippiExport = serde_json::from_str(text)?;
    let source_replay_path = export
        .source
        .as_ref()
        .and_then(|source| source.replay_path.clone());
    let frame_limit = config.max_frames.unwrap_or(usize::MAX);
    let mut world = world_for_slippi_export(&export);
    let mut active: [Option<ActiveDivergenceScenario>; PLAYER_COUNT] =
        std::array::from_fn(|_| None);
    let mut scenarios = Vec::new();
    let mut frames_scanned = 0usize;
    export.frames.sort_by_key(|frame| frame.frame);

    for (core_frame_index, frame) in export.frames.iter().take(frame_limit).enumerate() {
        if config
            .max_scenarios
            .is_some_and(|limit| scenarios.len() >= limit)
        {
            break;
        }

        let before_snapshot = world.rollback_snapshot();
        let (inputs, raw_inputs) = slippi_frame_inputs(frame);
        let core_frame = Frame(core_frame_index as u32);
        step_match_start_replay_world(&mut world, core_frame, &inputs);
        let after_step_checksum = world.checksum();
        let snapshot = world.snapshot();
        frames_scanned += 1;

        for player_index in 0..PLAYER_COUNT {
            if !config.compare_players[player_index] {
                continue;
            }
            let Some(post) = frame
                .players
                .get(&player_index.to_string())
                .and_then(|player_frame| player_frame.post.as_ref())
            else {
                continue;
            };
            let (row, status) = trace_row_and_divergence_status(
                core_frame,
                frame.frame,
                player_index,
                raw_inputs[player_index],
                post,
                snapshot.players[player_index],
                world.stage(),
                config.position_tolerance_milli,
                config.velocity_tolerance_milli,
            );
            update_active_divergence_scenario(
                &mut active[player_index],
                &mut scenarios,
                status,
                row,
                &export.frames,
                core_frame_index,
                &before_snapshot,
                after_step_checksum,
                config,
            );
        }
    }

    for slot in active.iter_mut() {
        finish_active_divergence(slot, &mut scenarios);
    }
    scenarios.sort_by(|left, right| {
        left.source_frame
            .cmp(&right.source_frame)
            .then_with(|| left.player_index.cmp(&right.player_index))
            .then_with(|| divergence_kind_rank(left.kind).cmp(&divergence_kind_rank(right.kind)))
    });
    mark_mixed_phase_cascades(&mut scenarios);
    for (index, scenario) in scenarios.iter_mut().enumerate() {
        scenario.scenario_index = index + 1;
    }

    Ok(SlippiCoreDivergenceScan {
        source_replay_path,
        config,
        frames_scanned,
        scenarios,
    })
}

pub fn scan_slippi_export_seeded_pre_frame_with_core(
    text: &str,
    config: SlippiCoreDivergenceScanConfig,
) -> Result<SlippiCoreDivergenceScan, SlippiCoreDiagnosticError> {
    let mut export: SlippiExport = serde_json::from_str(text)?;
    let source_replay_path = export
        .source
        .as_ref()
        .and_then(|source| source.replay_path.clone());
    let frame_limit = config.max_frames.unwrap_or(usize::MAX);
    let mut previous_inputs = [PlayerInput::neutral(); PLAYER_COUNT];
    let mut input_timers = [MeleeInputTimers::expired(); PLAYER_COUNT];
    let mut previous_posts: [Option<SlippiPostFrame>; PLAYER_COUNT] = [None, None];
    let mut unsupported_states = HashMap::<u16, usize>::new();
    let mut active: [Option<ActiveDivergenceScenario>; PLAYER_COUNT] =
        std::array::from_fn(|_| None);
    let mut scenarios = Vec::new();
    let mut frames_scanned = 0usize;
    export.frames.sort_by_key(|frame| frame.frame);

    for frame in export.frames.iter().take(frame_limit) {
        if config
            .max_scenarios
            .is_some_and(|limit| scenarios.len() >= limit)
        {
            break;
        }

        let mut world = World::for_two_players();
        let mut inputs = [PlayerInput::neutral(); PLAYER_COUNT];
        let mut raw_inputs = [SlippiRustPlayerInput::default(); PLAYER_COUNT];
        let mut comparable_players = [false; PLAYER_COUNT];
        for player_index in 0..PLAYER_COUNT {
            let Some(pre) = frame
                .players
                .get(&player_index.to_string())
                .and_then(|player_frame| player_frame.pre.as_ref())
            else {
                continue;
            };

            let input = effective_slippi_player_input(pre);
            raw_inputs[player_index] = input;
            inputs[player_index] = input.to_player_input();
            if let Some(state) = player_state_from_slippi_pre(
                pre,
                previous_posts[player_index].as_ref(),
                &mut unsupported_states,
            ) {
                world.set_player_state_for_diagnostic(player_index, state);
                comparable_players[player_index] = true;
            }
        }

        world.set_input_history_for_diagnostic(previous_inputs, input_timers);
        let core_frame = diagnostic_core_frame(frame.frame);
        step_world(&mut world, core_frame, &inputs);
        let snapshot = world.snapshot();
        input_timers = *world.input_timers();
        frames_scanned += 1;

        for player_index in 0..PLAYER_COUNT {
            if !config.compare_players[player_index] || !comparable_players[player_index] {
                continue;
            }
            let Some(post) = frame
                .players
                .get(&player_index.to_string())
                .and_then(|player_frame| player_frame.post.as_ref())
            else {
                continue;
            };
            let (row, status) = trace_row_and_divergence_status(
                core_frame,
                frame.frame,
                player_index,
                raw_inputs[player_index],
                post,
                snapshot.players[player_index],
                world.stage(),
                config.position_tolerance_milli,
                config.velocity_tolerance_milli,
            );
            update_active_seeded_divergence_scenario(
                &mut active[player_index],
                &mut scenarios,
                status,
                row,
                config,
            );
        }

        for player_index in 0..PLAYER_COUNT {
            if let Some(post) = frame
                .players
                .get(&player_index.to_string())
                .and_then(|player_frame| player_frame.post.as_ref())
            {
                previous_posts[player_index] = Some(post.clone());
            }
        }
        previous_inputs = inputs;
    }

    for slot in active.iter_mut() {
        finish_active_divergence(slot, &mut scenarios);
    }
    scenarios.sort_by(|left, right| {
        left.source_frame
            .cmp(&right.source_frame)
            .then_with(|| left.player_index.cmp(&right.player_index))
            .then_with(|| divergence_kind_rank(left.kind).cmp(&divergence_kind_rank(right.kind)))
    });
    for (index, scenario) in scenarios.iter_mut().enumerate() {
        scenario.scenario_index = index + 1;
    }

    Ok(SlippiCoreDivergenceScan {
        source_replay_path,
        config,
        frames_scanned,
        scenarios,
    })
}

pub fn first_slippi_divergence_from_match_start_with_core(
    text: &str,
    config: SlippiCoreDivergenceScanConfig,
) -> Result<Option<SlippiCoreFirstDivergence>, SlippiCoreDiagnosticError> {
    let scan = scan_slippi_export_from_match_start_with_core(text, config)?;
    Ok(scan
        .scenarios
        .iter()
        .find(|scenario| {
            is_first_slippi_divergence_candidate(scenario.kind)
                && scenario.cascades_from_source_frame.is_none()
        })
        .map(|scenario| SlippiCoreFirstDivergence {
            kind: scenario.kind,
            player_index: scenario.player_index,
            core_frame: scenario.core_frame,
            source_frame: scenario.source_frame,
        }))
}

fn slippi_frame_inputs(
    frame: &SlippiFrame,
) -> (
    [PlayerInput; PLAYER_COUNT],
    [SlippiRustPlayerInput; PLAYER_COUNT],
) {
    let mut inputs = [PlayerInput::neutral(); PLAYER_COUNT];
    let mut raw_inputs = [SlippiRustPlayerInput::default(); PLAYER_COUNT];
    for (player_index, input_slot) in inputs.iter_mut().enumerate() {
        if let Some(pre) = frame
            .players
            .get(&player_index.to_string())
            .and_then(|player_frame| player_frame.pre.as_ref())
        {
            let input = effective_slippi_player_input(pre);
            raw_inputs[player_index] = input;
            *input_slot = input.to_player_input();
        }
    }
    (inputs, raw_inputs)
}

fn trace_row_and_divergence_status(
    core_frame: Frame,
    source_frame: i32,
    player_index: usize,
    raw_input: SlippiRustPlayerInput,
    post: &SlippiPostFrame,
    actual_player: PlayerRenderSnapshot,
    stage: StageProfile,
    position_tolerance_milli: i32,
    velocity_tolerance_milli: i32,
) -> (SlippiCoreTraceRow, Option<SlippiCoreDivergenceStatus>) {
    let expected_state = slippi_action_state_to_expected(post.action_state_id);
    let expected_facing = slippi_facing_to_i8(post.facing);
    let expected_position = slippi_post_position_to_core_milli(
        post,
        stage,
        actual_player.source_coll_floor_surface_index,
        actual_player.position.y,
    );
    let expected_source_position = post
        .position
        .map(|position| SourceVec2 {
            x: slippi_units_to_source_f32(Some(position[0])),
            y: slippi_units_to_source_f32(Some(position[1])),
        })
        .unwrap_or_default();
    let expected_ground_velocity_x = post
        .self_induced_speeds
        .as_ref()
        .map(|speeds| slippi_units_to_core_milli(speeds.ground_x))
        .unwrap_or_default();
    let expected_ground_velocity_x_source = post
        .self_induced_speeds
        .as_ref()
        .map(|speeds| slippi_units_to_source_f32(speeds.ground_x))
        .unwrap_or_default();
    let expected_air_velocity_x = post
        .self_induced_speeds
        .as_ref()
        .map(|speeds| slippi_units_to_core_milli(speeds.air_x))
        .unwrap_or_default();
    let expected_air_velocity_x_source = post
        .self_induced_speeds
        .as_ref()
        .map(|speeds| slippi_units_to_source_f32(speeds.air_x))
        .unwrap_or_default();
    let expected_velocity_y = post
        .self_induced_speeds
        .as_ref()
        .map(|speeds| slippi_units_to_core_milli(speeds.y))
        .unwrap_or_default();
    let expected_velocity_y_source = post
        .self_induced_speeds
        .as_ref()
        .map(|speeds| slippi_units_to_source_f32(speeds.y))
        .unwrap_or_default();
    let expected_attack_velocity_x = post
        .self_induced_speeds
        .as_ref()
        .map(|speeds| slippi_units_to_core_milli(speeds.attack_x))
        .unwrap_or_default();
    let expected_attack_velocity_x_source = post
        .self_induced_speeds
        .as_ref()
        .map(|speeds| slippi_units_to_source_f32(speeds.attack_x))
        .unwrap_or_default();
    let expected_attack_velocity_y = post
        .self_induced_speeds
        .as_ref()
        .map(|speeds| slippi_units_to_core_milli(speeds.attack_y))
        .unwrap_or_default();
    let expected_attack_velocity_y_source = post
        .self_induced_speeds
        .as_ref()
        .map(|speeds| slippi_units_to_source_f32(speeds.attack_y))
        .unwrap_or_default();
    let actual_source_floor_surface = actual_player.source_coll_floor_surface_index;
    let actual_source_floor_line = actual_player.source_coll_floor_line_index;
    let expected_grounded_for_velocity =
        expected_velocity_grounded(post, expected_state, actual_player.grounded);
    let expected_base_horizontal_velocity_x = expected_base_horizontal_velocity_x(
        expected_ground_velocity_x,
        expected_air_velocity_x,
        expected_grounded_for_velocity,
    );
    let expected_composed_velocity_x = expected_composed_horizontal_velocity_x(
        expected_state,
        expected_base_horizontal_velocity_x,
        expected_attack_velocity_x,
    );
    let expected_composed_velocity_y = expected_composed_velocity_y(
        expected_state,
        expected_velocity_y,
        expected_attack_velocity_y,
    );
    let expected_action_identity =
        slippi_action_identity_for_expected_state(expected_state, post.action_state_id);
    let actual_action_identity = slippi_action_identity_for_player(&actual_player);
    let row = SlippiCoreTraceRow {
        core_frame,
        source_frame,
        player_index,
        input_stick_x: raw_input.stick_x,
        input_stick_y: raw_input.stick_y,
        input_button_bits: raw_input.physical_button_bits,
        input_left_trigger: raw_input.left_trigger,
        input_right_trigger: raw_input.right_trigger,
        input_ucf_dashback_amendment: raw_input.ucf_dashback_amendment,
        input_ucf_shield_drop_amendment: raw_input.ucf_shield_drop_amendment,
        actual_input_jump_pressed: actual_player.debug_input_facts.jump_pressed,
        actual_input_normal_jump_pressed: actual_player.debug_input_facts.normal_jump_pressed,
        actual_input_shield_held: actual_player.debug_input_facts.shield_held,
        actual_input_shield_pressed: actual_player.debug_input_facts.shield_pressed,
        expected_slippi_state_id: post.action_state_id,
        expected_action_state_id: expected_state
            .map(|state| state.action_state_id)
            .unwrap_or_else(|| MeleeActionStateId::new(post.action_state_id)),
        actual_action_state_id: actual_player.melee_action_state_id,
        expected_action_identity,
        actual_action_identity,
        expected_motion_state: expected_state.and_then(|state| state.motion_state),
        actual_motion_state: actual_player.motion_state,
        actual_grounded: actual_player.grounded,
        actual_motion_frame: actual_player.state_frame,
        actual_motion_anim_frame_milli: actual_player.animation_frame_milli,
        actual_source_fall_anim_blend: actual_player.source_fall_anim_blend,
        actual_source_fall_anim_pose: actual_player.source_fall_anim_pose,
        expected_facing,
        actual_facing: actual_player.facing,
        expected_position,
        actual_position: actual_player.position,
        expected_source_position,
        actual_source_position: actual_player.source_position,
        expected_ground_velocity_x,
        expected_air_velocity_x,
        expected_velocity_y,
        expected_attack_velocity_x,
        expected_attack_velocity_y,
        expected_composed_velocity_x,
        expected_composed_velocity_y,
        expected_ground_velocity_x_source,
        expected_air_velocity_x_source,
        expected_velocity_y_source,
        expected_attack_velocity_x_source,
        expected_attack_velocity_y_source,
        actual_velocity_x: actual_player.velocity.x,
        actual_velocity_y: actual_player.velocity.y,
        actual_source_self_velocity_x: actual_player.source_self_velocity_x,
        actual_source_self_velocity_y: actual_player.source_self_velocity_y,
        actual_source_knockback_velocity_x: actual_player.source_knockback_velocity_x,
        actual_source_knockback_velocity_y: actual_player.source_knockback_velocity_y,
        actual_source_ground_knockback_velocity: actual_player.source_ground_knockback_velocity,
        actual_player_nudge_x: actual_player.player_nudge_x,
        actual_player_nudge_z: actual_player.player_nudge_z,
        actual_ground_velocity_x: actual_player.ground_velocity_x,
        actual_ground_accel_x: actual_player.ground_accel_x,
        actual_ground_accel_x2: actual_player.ground_accel_x2,
        actual_dash_entry_velocity_delta: actual_player.dash_entry_velocity_delta,
        actual_dash_x0: actual_player.dash_x0,
        actual_source_coll_last_pos: actual_player.source_coll_last_pos,
        actual_source_coll_cur_pos: actual_player.source_coll_cur_pos,
        actual_source_coll_prev_pos: actual_player.source_coll_prev_pos,
        actual_source_coll_ecb: actual_player.source_coll_ecb,
        actual_source_coll_prev_ecb: actual_player.source_coll_prev_ecb,
        actual_source_coll_desired_ecb: actual_player.source_coll_desired_ecb,
        actual_ecb_bottom_lock_timer: actual_player.ecb_bottom_lock_timer,
        actual_source_coll_x130_locked: actual_player.source_coll_x130_locked,
        actual_floor_skip_surface: actual_player.floor_skip_surface,
        actual_source_floor_skip_line: actual_player.source_coll_floor_skip_line_index,
        actual_source_floor_surface,
        actual_source_floor_line,
        actual_source_coll_env_flags: actual_player.source_coll_env_flags,
        actual_source_coll_prev_env_flags: actual_player.source_coll_prev_env_flags,
    };

    let Some(expected_state) = expected_state else {
        return (
            row,
            Some(SlippiCoreDivergenceStatus {
                kind: SlippiCoreDivergenceKind::UnsupportedState,
                max_abs_position_delta_milli: max_abs_position_delta(&row),
                max_abs_velocity_delta_milli: max_abs_velocity_delta_with_expected(
                    &row,
                    row.expected_composed_velocity_x,
                    row.expected_composed_velocity_y,
                ),
            }),
        );
    };
    let states_match = slippi_expected_state_matches(
        expected_state,
        actual_player.motion_state,
        actual_player.melee_action_state_id,
    );
    let max_abs_position_delta_milli = max_abs_position_delta(&row);
    let max_abs_velocity_delta_milli = max_abs_velocity_delta_with_expected(
        &row,
        expected_composed_velocity_x,
        expected_composed_velocity_y,
    );

    let kind = if !states_match {
        if is_airborne_floor_commit_mixed_phase_witness(
            &row,
            actual_player,
            max_abs_velocity_delta_milli,
            velocity_tolerance_milli,
        ) || is_slippi_grounded_commit_ahead_of_decomp_floor_check_witness(
            &row,
            actual_player,
            max_abs_velocity_delta_milli,
            velocity_tolerance_milli,
        ) || is_damage_floor_commit_mixed_phase_witness(&row, actual_player)
        {
            Some(SlippiCoreDivergenceKind::MixedPhaseWitness)
        } else {
            Some(SlippiCoreDivergenceKind::StateMismatch)
        }
    } else if max_abs_position_delta_milli >= position_tolerance_milli.max(0) {
        Some(SlippiCoreDivergenceKind::PositionDrift)
    } else if max_abs_velocity_delta_milli > velocity_tolerance_milli.max(0) {
        if is_same_state_floor_commit_velocity_mixed_phase_witness(&row)
            || is_same_state_composed_velocity_channel_mixed_phase_witness(&row)
        {
            Some(SlippiCoreDivergenceKind::MixedPhaseWitness)
        } else {
            Some(SlippiCoreDivergenceKind::VelocityDrift)
        }
    } else {
        None
    };

    (
        row,
        kind.map(|kind| SlippiCoreDivergenceStatus {
            kind,
            max_abs_position_delta_milli,
            max_abs_velocity_delta_milli,
        }),
    )
}

fn is_airborne_floor_commit_mixed_phase_witness(
    row: &SlippiCoreTraceRow,
    actual_player: PlayerRenderSnapshot,
    max_abs_velocity_delta_milli: i32,
    velocity_tolerance_milli: i32,
) -> bool {
    is_fall_platform_mixed_phase_witness(
        row,
        actual_player,
        max_abs_velocity_delta_milli,
        velocity_tolerance_milli,
    ) || is_escape_air_platform_mixed_phase_witness(
        row,
        actual_player,
        max_abs_velocity_delta_milli,
        velocity_tolerance_milli,
    )
}

fn is_same_state_floor_commit_velocity_mixed_phase_witness(row: &SlippiCoreTraceRow) -> bool {
    if row.actual_action_state_id != Some(row.expected_action_state_id) || !row.actual_grounded {
        return false;
    }

    if row.actual_source_floor_line.is_none()
        || row.actual_source_coll_prev_env_flags != 0
        || row.actual_source_coll_env_flags & 0x0001_8000 != 0x0001_8000
        || row.actual_source_self_velocity_y.abs() <= f32::EPSILON
    {
        return false;
    }

    let expected_self_velocity_y = source_units_to_milli(row.actual_source_self_velocity_y);
    let slippi_exposes_pre_floor_self_y =
        (row.expected_velocity_y - expected_self_velocity_y).abs() <= 1
            && row.actual_velocity_y == 0;
    let slippi_exposes_pre_floor_damage_sum = row.expected_attack_velocity_y != 0
        && row.actual_source_knockback_velocity_y.abs() > 0.0
        && (row.actual_velocity_y - row.expected_velocity_y).abs()
            == row.expected_attack_velocity_y.abs();

    slippi_exposes_pre_floor_self_y || slippi_exposes_pre_floor_damage_sum
}

fn is_same_state_composed_velocity_channel_mixed_phase_witness(row: &SlippiCoreTraceRow) -> bool {
    row.actual_action_state_id == Some(row.expected_action_state_id)
        && row.expected_motion_state == Some(row.actual_motion_state)
        && (row.actual_position.x - row.expected_position.x).abs() <= 1
        && (row.actual_position.y - row.expected_position.y).abs() <= 1
        && row.expected_air_velocity_x == row.actual_velocity_x
        && row.expected_velocity_y == row.actual_velocity_y
        && (row.expected_composed_velocity_x != row.actual_velocity_x
            || row.expected_composed_velocity_y != row.actual_velocity_y)
}

fn is_fall_platform_mixed_phase_witness(
    row: &SlippiCoreTraceRow,
    actual_player: PlayerRenderSnapshot,
    max_abs_velocity_delta_milli: i32,
    velocity_tolerance_milli: i32,
) -> bool {
    row.expected_action_state_id == MeleeActionStateId::new(29)
        && row.expected_motion_state == Some(MotionState::Fall)
        && actual_player.melee_action_state_id == Some(MeleeActionStateId::new(42))
        && actual_player.motion_state == MotionState::Landing
        && actual_player.grounded
        && actual_player.source_coll_floor_line_index.is_some()
        && actual_player.source_coll_env_flags & 0x0001_8000 == 0x0001_8000
        && max_abs_velocity_delta_milli <= velocity_tolerance_milli.max(0)
        && row.expected_ground_velocity_x == 0
        && row.actual_ground_velocity_x == 0.0
}

fn is_escape_air_platform_mixed_phase_witness(
    row: &SlippiCoreTraceRow,
    actual_player: PlayerRenderSnapshot,
    max_abs_velocity_delta_milli: i32,
    velocity_tolerance_milli: i32,
) -> bool {
    row.expected_action_state_id == MeleeActionStateId::new(236)
        && row.expected_motion_state == Some(MotionState::EscapeAir)
        && actual_player.melee_action_state_id == Some(MeleeActionStateId::new(43))
        && actual_player.motion_state == MotionState::LandingFallSpecial
        && actual_player.grounded
        && actual_player.source_coll_floor_line_index.is_some()
        && actual_player.source_coll_env_flags & 0x0001_8000 == 0x0001_8000
        && max_abs_velocity_delta_milli <= velocity_tolerance_milli.max(0)
        && row.expected_ground_velocity_x == 0
        && row.actual_ground_velocity_x.abs() > 0.0
}

fn is_slippi_grounded_commit_ahead_of_decomp_floor_check_witness(
    row: &SlippiCoreTraceRow,
    actual_player: PlayerRenderSnapshot,
    max_abs_velocity_delta_milli: i32,
    velocity_tolerance_milli: i32,
) -> bool {
    row.expected_motion_state == Some(MotionState::Wait)
        && actual_player.melee_action_state_id == Some(MeleeActionStateId::new(29))
        && actual_player.motion_state == MotionState::Fall
        && !actual_player.grounded
        && actual_player.source_coll_floor_line_index.is_some()
        && actual_player.source_coll_env_flags & 0x0001_8000 == 0
        && actual_player.source_coll_prev_ecb.bottom.y.abs() <= f32::EPSILON
        && actual_player.source_coll_ecb.bottom.y > 0.0
        && actual_player.ecb_bottom_lock_timer == 0
        && !actual_player.source_coll_x130_locked
        && row.expected_position.y > row.actual_position.y
        && max_abs_velocity_delta_milli <= velocity_tolerance_milli.max(0)
        && row.expected_velocity_y == row.actual_velocity_y
        && row.expected_air_velocity_x == row.actual_velocity_x
}

fn is_damage_floor_commit_mixed_phase_witness(
    row: &SlippiCoreTraceRow,
    actual_player: PlayerRenderSnapshot,
) -> bool {
    let expected_action_id = row.expected_action_state_id.get();
    let expected_self_velocity_x = source_units_to_milli(row.actual_source_self_velocity_x);
    let expected_self_velocity_y = source_units_to_milli(row.actual_source_self_velocity_y);
    let expected_attack_velocity_x = source_units_to_milli(row.actual_source_knockback_velocity_x);
    let expected_attack_velocity_y = source_units_to_milli(row.actual_source_knockback_velocity_y);

    (75..=86).contains(&expected_action_id)
        && actual_player.melee_action_state_id == Some(MeleeActionStateId::new(42))
        && actual_player.motion_state == MotionState::Landing
        && actual_player.grounded
        && actual_player.source_coll_floor_line_index.is_some()
        && actual_player.source_coll_env_flags & 0x0001_8000 == 0x0001_8000
        && actual_player.source_coll_prev_env_flags == 0
        && (row.actual_position.x - row.expected_position.x).abs() <= 1
        && row.actual_position.y > row.expected_position.y
        && ((row.expected_air_velocity_x - expected_self_velocity_x).abs() <= 1
            || (row.expected_ground_velocity_x - expected_self_velocity_x).abs() <= 1)
        && (row.expected_velocity_y - expected_self_velocity_y).abs() <= 1
        && (row.expected_attack_velocity_x - expected_attack_velocity_x).abs() <= 1
        && (row.expected_attack_velocity_y - expected_attack_velocity_y).abs() <= 1
}

#[allow(clippy::too_many_arguments)]
fn update_active_divergence_scenario(
    active: &mut Option<ActiveDivergenceScenario>,
    scenarios: &mut Vec<SlippiCoreDivergenceScenario>,
    status: Option<SlippiCoreDivergenceStatus>,
    row: SlippiCoreTraceRow,
    frames: &[SlippiFrame],
    frame_index: usize,
    before_snapshot: &WorldRollbackSnapshot,
    after_step_checksum: u64,
    config: SlippiCoreDivergenceScanConfig,
) {
    if let Some(status) = status {
        if let Some(active_scenario) = active.as_mut() {
            if active_scenario.kind == status.kind {
                active_scenario.end_core_frame = row.core_frame;
                active_scenario.end_source_frame = row.source_frame;
                active_scenario.duration_frames = active_scenario.duration_frames.saturating_add(1);
                active_scenario.max_abs_position_delta_milli = active_scenario
                    .max_abs_position_delta_milli
                    .max(status.max_abs_position_delta_milli);
                active_scenario.max_abs_velocity_delta_milli = active_scenario
                    .max_abs_velocity_delta_milli
                    .max(status.max_abs_velocity_delta_milli);
                return;
            }
            finish_active_divergence(active, scenarios);
        }

        *active = Some(start_active_divergence_scenario(
            status,
            row,
            frames,
            frame_index,
            before_snapshot,
            after_step_checksum,
            config,
        ));
        return;
    }

    if let Some(active_scenario) = active.as_mut() {
        if active_scenario.realign_core_frame.is_none() {
            active_scenario.realign_core_frame = Some(row.core_frame);
            active_scenario.realign_source_frame = Some(row.source_frame);
        }
        finish_active_divergence(active, scenarios);
    }
}

fn update_active_seeded_divergence_scenario(
    active: &mut Option<ActiveDivergenceScenario>,
    scenarios: &mut Vec<SlippiCoreDivergenceScenario>,
    status: Option<SlippiCoreDivergenceStatus>,
    row: SlippiCoreTraceRow,
    config: SlippiCoreDivergenceScanConfig,
) {
    if let Some(status) = status {
        if let Some(active_scenario) = active.as_mut() {
            if active_scenario.kind == status.kind {
                active_scenario.end_core_frame = row.core_frame;
                active_scenario.end_source_frame = row.source_frame;
                active_scenario.duration_frames = active_scenario.duration_frames.saturating_add(1);
                active_scenario.max_abs_position_delta_milli = active_scenario
                    .max_abs_position_delta_milli
                    .max(status.max_abs_position_delta_milli);
                active_scenario.max_abs_velocity_delta_milli = active_scenario
                    .max_abs_velocity_delta_milli
                    .max(status.max_abs_velocity_delta_milli);
                return;
            }
            finish_active_divergence(active, scenarios);
        }

        *active = Some(ActiveDivergenceScenario {
            kind: status.kind,
            player_index: row.player_index,
            core_frame: row.core_frame,
            source_frame: row.source_frame,
            end_core_frame: row.core_frame,
            end_source_frame: row.source_frame,
            duration_frames: 1,
            realigned_within_lookahead: false,
            realign_core_frame: None,
            realign_source_frame: None,
            rollback_replay_deterministic: true,
            first_frame: row,
            max_abs_position_delta_milli: status.max_abs_position_delta_milli,
            max_abs_velocity_delta_milli: status.max_abs_velocity_delta_milli,
        });
        return;
    }

    if let Some(active_scenario) = active.as_mut() {
        active_scenario.realign_core_frame = Some(row.core_frame);
        active_scenario.realign_source_frame = Some(row.source_frame);
        active_scenario.realigned_within_lookahead =
            active_scenario.duration_frames <= config.lookahead_frames;
        finish_active_divergence(active, scenarios);
    }
}

fn start_active_divergence_scenario(
    status: SlippiCoreDivergenceStatus,
    row: SlippiCoreTraceRow,
    frames: &[SlippiFrame],
    frame_index: usize,
    before_snapshot: &WorldRollbackSnapshot,
    after_step_checksum: u64,
    config: SlippiCoreDivergenceScanConfig,
) -> ActiveDivergenceScenario {
    let (realigned, realign_core_frame, realign_source_frame, rollback_replay_deterministic) =
        rollback_replay_realignment(
            frames,
            frame_index,
            before_snapshot,
            after_step_checksum,
            row.player_index,
            config,
        );
    ActiveDivergenceScenario {
        kind: status.kind,
        player_index: row.player_index,
        core_frame: row.core_frame,
        source_frame: row.source_frame,
        end_core_frame: row.core_frame,
        end_source_frame: row.source_frame,
        duration_frames: 1,
        realigned_within_lookahead: realigned,
        realign_core_frame,
        realign_source_frame,
        rollback_replay_deterministic,
        first_frame: row,
        max_abs_position_delta_milli: status.max_abs_position_delta_milli,
        max_abs_velocity_delta_milli: status.max_abs_velocity_delta_milli,
    }
}

fn finish_active_divergence(
    active: &mut Option<ActiveDivergenceScenario>,
    scenarios: &mut Vec<SlippiCoreDivergenceScenario>,
) {
    let Some(active) = active.take() else {
        return;
    };
    scenarios.push(SlippiCoreDivergenceScenario {
        scenario_index: scenarios.len() + 1,
        kind: active.kind,
        player_index: active.player_index,
        core_frame: active.core_frame,
        source_frame: active.source_frame,
        end_core_frame: active.end_core_frame,
        end_source_frame: active.end_source_frame,
        duration_frames: active.duration_frames,
        realigned_within_lookahead: active.realigned_within_lookahead,
        realign_core_frame: active.realign_core_frame,
        realign_source_frame: active.realign_source_frame,
        rollback_replay_deterministic: active.rollback_replay_deterministic,
        first_frame: active.first_frame,
        max_abs_position_delta_milli: active.max_abs_position_delta_milli,
        max_abs_velocity_delta_milli: active.max_abs_velocity_delta_milli,
        cascades_from_source_frame: None,
    });
}

fn mark_mixed_phase_cascades(scenarios: &mut [SlippiCoreDivergenceScenario]) {
    let mut cascade_roots: [Option<(i32, Option<i32>)>; PLAYER_COUNT] = [None; PLAYER_COUNT];
    for scenario in scenarios {
        let root = &mut cascade_roots[scenario.player_index];
        if let Some((_, Some(realign_source_frame))) = root {
            if scenario.source_frame >= *realign_source_frame {
                *root = None;
            }
        }

        if let Some((root_source_frame, _)) = *root {
            scenario.cascades_from_source_frame = Some(root_source_frame);
            continue;
        }

        if scenario.kind == SlippiCoreDivergenceKind::MixedPhaseWitness {
            *root = Some((scenario.source_frame, scenario.realign_source_frame));
        }
    }
}

fn divergence_kind_rank(kind: SlippiCoreDivergenceKind) -> u8 {
    match kind {
        SlippiCoreDivergenceKind::UnsupportedState => 0,
        SlippiCoreDivergenceKind::MixedPhaseWitness => 1,
        SlippiCoreDivergenceKind::StateMismatch => 2,
        SlippiCoreDivergenceKind::PositionDrift => 3,
        SlippiCoreDivergenceKind::VelocityDrift => 4,
    }
}

fn is_first_slippi_divergence_candidate(kind: SlippiCoreDivergenceKind) -> bool {
    match kind {
        SlippiCoreDivergenceKind::UnsupportedState
        | SlippiCoreDivergenceKind::StateMismatch
        | SlippiCoreDivergenceKind::PositionDrift
        | SlippiCoreDivergenceKind::VelocityDrift => true,
        SlippiCoreDivergenceKind::MixedPhaseWitness => false,
    }
}

fn rollback_replay_realignment(
    frames: &[SlippiFrame],
    frame_index: usize,
    before_snapshot: &WorldRollbackSnapshot,
    after_step_checksum: u64,
    player_index: usize,
    config: SlippiCoreDivergenceScanConfig,
) -> (bool, Option<Frame>, Option<i32>, bool) {
    let mut world = World::for_slippi_battlefield_singles_match_start();
    world.restore_rollback_snapshot(before_snapshot);
    let mut rollback_replay_deterministic = true;
    let end_index = frame_index
        .saturating_add(config.lookahead_frames)
        .min(frames.len().saturating_sub(1));

    for (offset, replay_index) in (frame_index..=end_index).enumerate() {
        let frame = &frames[replay_index];
        let (inputs, raw_inputs) = slippi_frame_inputs(frame);
        let core_frame = Frame(replay_index as u32);
        step_match_start_replay_world(&mut world, core_frame, &inputs);
        if offset == 0 {
            rollback_replay_deterministic = world.checksum() == after_step_checksum;
        }
        let Some(post) = frame
            .players
            .get(&player_index.to_string())
            .and_then(|player_frame| player_frame.post.as_ref())
        else {
            continue;
        };
        let snapshot = world.snapshot();
        let (_, status) = trace_row_and_divergence_status(
            core_frame,
            frame.frame,
            player_index,
            raw_inputs[player_index],
            post,
            snapshot.players[player_index],
            world.stage(),
            config.position_tolerance_milli,
            config.velocity_tolerance_milli,
        );
        if offset > 0 && status.is_none() {
            return (
                true,
                Some(core_frame),
                Some(frame.frame),
                rollback_replay_deterministic,
            );
        }
    }

    (false, None, None, rollback_replay_deterministic)
}

fn max_abs_position_delta(row: &SlippiCoreTraceRow) -> i32 {
    (row.actual_position.x - row.expected_position.x)
        .abs()
        .max((row.actual_position.y - row.expected_position.y).abs())
}

fn max_abs_velocity_delta_with_expected(
    row: &SlippiCoreTraceRow,
    expected_horizontal_velocity_x: i32,
    expected_velocity_y: i32,
) -> i32 {
    (row.actual_velocity_x - expected_horizontal_velocity_x)
        .abs()
        .max((row.actual_velocity_y - expected_velocity_y).abs())
}

fn empty_comparison(export: &SlippiExport, mode: SlippiCoreComparisonMode) -> SlippiCoreComparison {
    SlippiCoreComparison {
        mode,
        source_replay_path: export
            .source
            .as_ref()
            .and_then(|source| source.replay_path.clone()),
        ucf_players: export.ucf_players(),
        ucf_dashback_amendment_frames: [0; PLAYER_COUNT],
        frames_compared: 0,
        player_frames_compared: [0; PLAYER_COUNT],
        unsupported_state_count: 0,
        unsupported_states: Vec::new(),
        state_mismatch_count: 0,
        max_abs_ground_velocity_diff: [0; PLAYER_COUNT],
        first_position_drift: None,
        first_position_drift_by_player: [None; PLAYER_COUNT],
        first_state_mismatch: None,
        first_divergence: None,
    }
}

fn record_first_classified_divergence(
    comparison: &mut SlippiCoreComparison,
    core_frame: Frame,
    source_frame: i32,
    player_index: usize,
    raw_input: SlippiRustPlayerInput,
    post: &SlippiPostFrame,
    actual_player: PlayerRenderSnapshot,
    stage: StageProfile,
) {
    if comparison.first_divergence.is_some() {
        return;
    }

    let (_, status) = trace_row_and_divergence_status(
        core_frame,
        source_frame,
        player_index,
        raw_input,
        post,
        actual_player,
        stage,
        SIGNIFICANT_POSITION_DRIFT_MILLI,
        1,
    );
    let Some(status) = status else {
        return;
    };
    if !is_first_slippi_divergence_candidate(status.kind) {
        return;
    }

    comparison.first_divergence = Some(SlippiCoreFirstDivergence {
        kind: status.kind,
        player_index,
        core_frame,
        source_frame,
    });
}

#[allow(clippy::too_many_arguments)]
fn record_first_position_drift(
    comparison: &mut SlippiCoreComparison,
    frame: Frame,
    source_frame: i32,
    player_index: usize,
    expected_slippi_state_id: u16,
    expected_action_state_id: MeleeActionStateId,
    actual_action_state_id: Option<MeleeActionStateId>,
    expected_motion_state: Option<MotionState>,
    actual_motion_state: MotionState,
    states_match: bool,
    expected_position: Vec2,
    actual_position: Vec2,
    expected_ground_velocity_x: i32,
    expected_air_velocity_x: i32,
    expected_velocity_y: i32,
    actual_velocity_x: i32,
    actual_velocity_y: i32,
) {
    if !states_match {
        return;
    }

    let delta_x = actual_position.x - expected_position.x;
    let delta_y = actual_position.y - expected_position.y;
    let max_abs_delta = delta_x.abs().max(delta_y.abs());
    if max_abs_delta < SIGNIFICANT_POSITION_DRIFT_MILLI {
        return;
    }

    let drift = SlippiCorePositionDrift {
        frame,
        source_frame,
        player_index,
        expected_slippi_state_id,
        expected_action_state_id,
        actual_action_state_id,
        expected_motion_state,
        actual_motion_state,
        expected_position,
        actual_position,
        expected_ground_velocity_x,
        expected_air_velocity_x,
        expected_velocity_y,
        actual_velocity_x,
        actual_velocity_y,
    };
    comparison.first_position_drift.get_or_insert(drift);
    comparison.first_position_drift_by_player[player_index].get_or_insert(drift);
}

fn controller_fix_label(is_ucf: bool) -> &'static str {
    if is_ucf {
        "UCF"
    } else {
        "vanilla/unknown"
    }
}

fn player_state_from_slippi_pre(
    pre: &SlippiPreFrame,
    previous_post: Option<&SlippiPostFrame>,
    unsupported_states: &mut HashMap<u16, usize>,
) -> Option<PlayerState> {
    let action_state_id = pre.action_state_id.unwrap_or(14);
    let Some(motion_state) = slippi_action_state_to_motion(action_state_id) else {
        *unsupported_states.entry(action_state_id).or_default() += 1;
        return None;
    };
    let position = pre.position.unwrap_or_default();
    let source_position = SourceVec2 {
        x: slippi_units_to_source_f32(Some(position[0])),
        y: slippi_units_to_source_f32(Some(position[1])),
    };
    let facing = pre.facing.unwrap_or(1.0);
    let mut player = PlayerState::new(
        source_units_to_milli(source_position.x),
        source_units_to_milli(source_position.y),
        if facing < 0.0 { -1 } else { 1 },
    );
    player.source_position = source_position;
    player.set_motion_state_alias(motion_state);
    player.grounded = !slippi_motion_state_uses_air_velocity(motion_state);
    if matches!(
        motion_state,
        MotionState::Entry | MotionState::EntryStart | MotionState::EntryEnd
    ) {
        player.entry_timer = 1;
    }
    if let Some(post) = previous_post {
        if let Some(speeds) = &post.self_induced_speeds {
            let ground_x = slippi_units_to_source_f32(speeds.ground_x);
            let air_x = slippi_units_to_source_f32(speeds.air_x);
            let vertical = slippi_units_to_source_f32(speeds.y);
            let horizontal = if player.grounded { ground_x } else { air_x };
            player.source_self_velocity_x = horizontal;
            player.source_self_velocity_y = vertical;
            player.velocity = Vec2 {
                x: source_units_to_milli(horizontal),
                y: source_units_to_milli(vertical),
            };
            player.ground_velocity_x = ground_x;
        }
        player.motion_frame = rounded_u8(post.action_state_counter);
    }
    seed_ucf_dashback_turn_for_diagnostic(pre, &mut player);
    Some(player)
}

fn seed_ucf_dashback_turn_for_diagnostic(pre: &SlippiPreFrame, player: &mut PlayerState) {
    if player.motion_state != MotionState::Turn {
        return;
    }
    let input = effective_slippi_player_input(pre);
    if !input.ucf_dashback_amendment || input.stick_x == 0 {
        return;
    }

    player.motion_frame = 0;
    player.turn_facing_after = if input.stick_x < 0 { -1 } else { 1 };
    player.turn_has_turned = false;
    player.turn_just_turned = false;
    player.turn_frames_to_turn = player
        .profile
        .standing_turn_direction_change_frames
        .saturating_sub(1);
    player.turn_dash_after_direction = player.facing;
}

pub fn slippi_core_report_path(replay_path: impl AsRef<Path>) -> PathBuf {
    let stem = replay_path
        .as_ref()
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("slippi-replay");
    PathBuf::from("debug")
        .join("slippi")
        .join(format!("{stem}.core.report.md"))
}

pub fn write_slippi_core_report(
    path: impl AsRef<Path>,
    comparison: &SlippiCoreComparison,
) -> Result<(), SlippiCoreDiagnosticError> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, comparison.report_markdown())?;
    Ok(())
}

pub fn write_slippi_core_trace_report(
    path: impl AsRef<Path>,
    trace: &SlippiCoreTrace,
) -> Result<(), SlippiCoreDiagnosticError> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, trace.report_markdown())?;
    Ok(())
}

fn slippi_units_to_core_milli(value: Option<f64>) -> i32 {
    source_units_to_milli(slippi_units_to_source_f32(value))
}

fn slippi_units_to_source_f32(value: Option<f64>) -> f32 {
    value.unwrap_or_default() as f32
}

fn slippi_position_to_core_milli(position: [f64; 2]) -> Vec2 {
    Vec2 {
        x: slippi_units_to_core_milli(Some(position[0])),
        y: slippi_units_to_core_milli(Some(position[1])),
    }
}

fn slippi_post_position_to_core_milli(
    post: &SlippiPostFrame,
    stage: StageProfile,
    actual_floor_surface: Option<u8>,
    actual_y: i32,
) -> Vec2 {
    let Some(position) = post.position else {
        return Vec2::default();
    };
    let mut expected = slippi_position_to_core_milli(position);
    if let Some(surface_y) = actual_floor_surface.and_then(|index| stage_surface_y(stage, index)) {
        if (actual_y - expected.y - surface_y).abs() < 100 {
            expected.y += surface_y;
        }
    }
    expected
}

fn stage_surface_y(stage: StageProfile, surface_index: u8) -> Option<i32> {
    stage
        .collision_surfaces()
        .get(usize::from(surface_index))
        .map(|surface| surface.y)
}

fn diagnostic_core_frame(slippi_frame: i32) -> Frame {
    Frame(slippi_frame.max(0) as u32)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SlippiExpectedActionState {
    action_state_id: MeleeActionStateId,
    motion_state: Option<MotionState>,
}

fn slippi_action_state_to_expected(action_state_id: u16) -> Option<SlippiExpectedActionState> {
    if let Some(motion_state) = slippi_action_state_to_motion(action_state_id) {
        return Some(SlippiExpectedActionState {
            action_state_id: MeleeActionStateId::new(action_state_id),
            motion_state: Some(motion_state),
        });
    }
    if slippi_source_only_action_state_is_known(action_state_id) {
        return Some(SlippiExpectedActionState {
            action_state_id: MeleeActionStateId::new(action_state_id),
            motion_state: None,
        });
    }
    None
}

fn slippi_source_only_action_state_is_known(action_state_id: u16) -> bool {
    CANONICAL_SOURCE_ONLY_ACTION_BINDINGS
        .iter()
        .any(|binding| binding.action_state_id.get() == action_state_id)
}

fn slippi_action_identity_for_expected_state(
    expected_state: Option<SlippiExpectedActionState>,
    slippi_action_state_id: u16,
) -> SlippiActionIdentity {
    if let Some(state) = expected_state {
        if let Some(motion_state) = state.motion_state {
            if melee_action_state_id_for_motion_state(motion_state) == state.action_state_id {
                return slippi_action_identity_for_motion_state(motion_state);
            }
        }
        return slippi_action_identity_for_action_state_id(state.action_state_id);
    }
    let action_state_id = MeleeActionStateId::new(slippi_action_state_id);
    slippi_action_identity_for_action_state_id(action_state_id)
}

fn slippi_action_identity_for_player(player: &PlayerRenderSnapshot) -> SlippiActionIdentity {
    if let Some(action_state_id) = player.melee_action_state_id {
        if melee_action_state_id_for_motion_state(player.motion_state) == action_state_id {
            let identity = slippi_action_identity_for_motion_state(player.motion_state);
            if identity.source_action_table_index.is_some() {
                return identity;
            }
        }
        let identity = slippi_action_identity_for_action_state_id(action_state_id);
        if identity.source_action_table_index.is_some() {
            return identity;
        }
    }
    slippi_action_identity_for_motion_state(player.motion_state)
}

fn slippi_action_identity_for_action_state_id(
    action_state_id: MeleeActionStateId,
) -> SlippiActionIdentity {
    if let Some(binding) = crate::source_frame_data::ACTION_BINDINGS
        .iter()
        .find(|binding| {
            binding.melee_motion_state_id == Some(MeleeMotionStateId::new(action_state_id.get()))
        })
    {
        return SlippiActionIdentity {
            melee_motion_state_id: binding.melee_motion_state_id,
            source_action_table_index: Some(binding.source_action_table_index),
            source_action_key: Some(binding.source_action_key),
        };
    }

    if let Some(motion_state) = slippi_action_state_to_motion(action_state_id.get()) {
        if melee_action_state_id_for_motion_state(motion_state) == action_state_id {
            let identity = slippi_action_identity_for_motion_state(motion_state);
            if identity.source_action_table_index.is_some() {
                return identity;
            }
        }
    }

    if let Some(binding) = CANONICAL_SOURCE_ONLY_ACTION_BINDINGS
        .iter()
        .find(|binding| binding.action_state_id == action_state_id)
    {
        return SlippiActionIdentity {
            melee_motion_state_id: None,
            source_action_table_index: Some(SourceActionTableIndex::new(
                binding.source_action_table_id,
            )),
            source_action_key: Some(binding.source_action_key.as_str()),
        };
    }

    crate::source_frame_data::ACTION_BINDINGS
        .iter()
        .find(|binding| binding.source_action_table_index.get() == action_state_id.get())
        .map(|binding| SlippiActionIdentity {
            melee_motion_state_id: binding.melee_motion_state_id,
            source_action_table_index: Some(binding.source_action_table_index),
            source_action_key: Some(binding.source_action_key),
        })
        .unwrap_or_else(SlippiActionIdentity::missing)
}

fn slippi_action_identity_for_motion_state(motion_state: MotionState) -> SlippiActionIdentity {
    if let Some(binding) = source_binding_for_motion_state(motion_state) {
        return SlippiActionIdentity {
            melee_motion_state_id: binding.melee_motion_state_id,
            source_action_table_index: Some(binding.source_action_table_index),
            source_action_key: Some(binding.source_action_key.as_str()),
        };
    }

    crate::source_frame_data::ACTION_BINDINGS
        .iter()
        .find(|binding| binding.motion_state == Some(motion_state))
        .map(|binding| SlippiActionIdentity {
            melee_motion_state_id: binding.melee_motion_state_id,
            source_action_table_index: Some(binding.source_action_table_index),
            source_action_key: Some(binding.source_action_key),
        })
        .unwrap_or_else(SlippiActionIdentity::missing)
}

fn slippi_expected_state_matches(
    expected: SlippiExpectedActionState,
    actual_motion_state: MotionState,
    actual_action_state_id: Option<MeleeActionStateId>,
) -> bool {
    actual_action_state_id == Some(expected.action_state_id)
        || expected
            .motion_state
            .is_some_and(|motion_state| actual_motion_state == motion_state)
}

fn expected_velocity_grounded(
    post: &SlippiPostFrame,
    expected: Option<SlippiExpectedActionState>,
    actual_grounded: bool,
) -> bool {
    if let Some(expected) = expected {
        if let Some(motion_state) = expected.motion_state {
            return slippi_motion_state_uses_ground_velocity(motion_state);
        }
    }
    post.airborne
        .map(|airborne| !airborne)
        .unwrap_or(actual_grounded)
}

fn expected_base_horizontal_velocity_x(
    expected_ground_velocity_x: i32,
    expected_air_velocity_x: i32,
    expected_grounded: bool,
) -> i32 {
    if expected_grounded {
        expected_ground_velocity_x
    } else {
        expected_air_velocity_x
    }
}

fn expected_composed_horizontal_velocity_x(
    expected: Option<SlippiExpectedActionState>,
    expected_base_horizontal_velocity_x: i32,
    expected_attack_velocity_x: i32,
) -> i32 {
    if expected.is_some_and(slippi_expected_state_uses_attack_velocity)
        || expected.is_some_and(|expected| {
            expected.motion_state.is_none() && expected_attack_velocity_x != 0
        })
    {
        expected_base_horizontal_velocity_x + expected_attack_velocity_x
    } else {
        expected_base_horizontal_velocity_x
    }
}

fn expected_composed_velocity_y(
    expected: Option<SlippiExpectedActionState>,
    expected_velocity_y: i32,
    expected_attack_velocity_y: i32,
) -> i32 {
    if expected.is_some_and(slippi_expected_state_uses_attack_velocity) {
        expected_velocity_y + expected_attack_velocity_y
    } else {
        expected_velocity_y
    }
}

fn slippi_expected_state_uses_attack_velocity(expected: SlippiExpectedActionState) -> bool {
    is_source_damage_action_state_id(expected.action_state_id)
}

fn slippi_motion_state_uses_ground_velocity(motion_state: MotionState) -> bool {
    !slippi_motion_state_uses_air_velocity(motion_state)
}

fn slippi_motion_state_uses_air_velocity(motion_state: MotionState) -> bool {
    matches!(
        motion_state,
        MotionState::Entry
            | MotionState::DeadDown
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
            | MotionState::RebirthWait
            | MotionState::EntryStart
            | MotionState::EntryEnd
            | MotionState::Pass
            | MotionState::Fall
            | MotionState::FallF
            | MotionState::FallB
            | MotionState::FallAerial
            | MotionState::FallAerialF
            | MotionState::FallAerialB
            | MotionState::JumpF
            | MotionState::JumpB
            | MotionState::JumpAerialF
            | MotionState::JumpAerialB
            | MotionState::AttackAirN
            | MotionState::AttackAirF
            | MotionState::AttackAirB
            | MotionState::AttackAirHi
            | MotionState::AttackAirLw
            | MotionState::EscapeAir
            | MotionState::FallSpecial
            | MotionState::FallSpecialF
            | MotionState::FallSpecialB
            | MotionState::DamageFall
            | MotionState::SpecialHi
            | MotionState::SpecialAirHi
    )
}

fn expected_state_label(
    expected_motion_state: Option<MotionState>,
    action_state_id: u16,
) -> String {
    expected_motion_state
        .map(|state| format!("{state:?} ({action_state_id})"))
        .unwrap_or_else(|| {
            format!(
                "{} ({action_state_id})",
                slippi_action_state_name(action_state_id)
            )
        })
}

fn slippi_action_state_to_motion(action_state_id: u16) -> Option<MotionState> {
    match action_state_id {
        0 => Some(MotionState::DeadDown),
        1 => Some(MotionState::DeadLeft),
        2 => Some(MotionState::DeadRight),
        3 => Some(MotionState::DeadUp),
        4 => Some(MotionState::DeadUpStar),
        5 => Some(MotionState::DeadUpStarIce),
        6 => Some(MotionState::DeadUpFall),
        7 => Some(MotionState::DeadUpFallHitCamera),
        8 => Some(MotionState::DeadUpFallHitCameraFlat),
        9 => Some(MotionState::DeadUpFallIce),
        10 => Some(MotionState::DeadUpFallHitCameraIce),
        11 => Some(MotionState::Sleep),
        12 => Some(MotionState::Rebirth),
        13 => Some(MotionState::RebirthWait),
        14 => Some(MotionState::Wait),
        15 => Some(MotionState::WalkSlow),
        16 => Some(MotionState::WalkMiddle),
        17 => Some(MotionState::WalkFast),
        18 => Some(MotionState::Turn),
        19 => Some(MotionState::TurnRun),
        20 => Some(MotionState::Dash),
        21 => Some(MotionState::Run),
        22 => Some(MotionState::RunDirect),
        23 => Some(MotionState::RunBrake),
        24 => Some(MotionState::KneeBend),
        25 => Some(MotionState::JumpF),
        26 => Some(MotionState::JumpB),
        27 => Some(MotionState::JumpAerialF),
        28 => Some(MotionState::JumpAerialB),
        29 => Some(MotionState::Fall),
        30 => Some(MotionState::FallF),
        31 => Some(MotionState::FallB),
        32 => Some(MotionState::FallAerial),
        33 => Some(MotionState::FallAerialF),
        34 => Some(MotionState::FallAerialB),
        35 => Some(MotionState::FallSpecial),
        36 => Some(MotionState::FallSpecialF),
        37 => Some(MotionState::FallSpecialB),
        38 => Some(MotionState::DamageFall),
        39 => Some(MotionState::Squat),
        40 => Some(MotionState::SquatWait),
        41 => Some(MotionState::SquatRv),
        42 => Some(MotionState::Landing),
        43 => Some(MotionState::LandingFallSpecial),
        44 => Some(MotionState::Attack1),
        50 => Some(MotionState::AttackDash),
        53 => Some(MotionState::AttackS3),
        56 => Some(MotionState::AttackHi3),
        57 => Some(MotionState::AttackLw3),
        60 => Some(MotionState::AttackS4),
        63 => Some(MotionState::AttackHi4),
        64 => Some(MotionState::AttackLw4),
        65 => Some(MotionState::AttackAirN),
        66 => Some(MotionState::AttackAirF),
        67 => Some(MotionState::AttackAirB),
        68 => Some(MotionState::AttackAirHi),
        69 => Some(MotionState::AttackAirLw),
        70 => Some(MotionState::LandingAirN),
        71 => Some(MotionState::LandingAirF),
        72 => Some(MotionState::LandingAirB),
        73 => Some(MotionState::LandingAirHi),
        74 => Some(MotionState::LandingAirLw),
        212 => Some(MotionState::Catch),
        214 => Some(MotionState::CatchDash),
        178 => Some(MotionState::GuardOn),
        179 => Some(MotionState::Guard),
        180 => Some(MotionState::GuardOff),
        181 => Some(MotionState::GuardSetOff),
        182 => Some(MotionState::GuardReflect),
        233 => Some(MotionState::EscapeF),
        234 => Some(MotionState::EscapeB),
        235 => Some(MotionState::EscapeN),
        236 => Some(MotionState::EscapeAir),
        244 => Some(MotionState::Pass),
        252 => Some(MotionState::CliffCatch),
        253 => Some(MotionState::CliffWait),
        254 => Some(MotionState::CliffClimbSlow),
        255 => Some(MotionState::CliffClimbQuick),
        256 => Some(MotionState::CliffAttackSlow),
        257 => Some(MotionState::CliffAttackQuick),
        258 => Some(MotionState::CliffEscapeSlow),
        259 => Some(MotionState::CliffEscapeQuick),
        260 => Some(MotionState::CliffJumpSlow1),
        261 => Some(MotionState::CliffJumpSlow2),
        262 => Some(MotionState::CliffJumpQuick1),
        263 => Some(MotionState::CliffJumpQuick2),
        322 => Some(MotionState::Entry),
        323 => Some(MotionState::EntryStart),
        324 => Some(MotionState::EntryEnd),
        347 => Some(MotionState::SpecialN),
        348 => Some(MotionState::SpecialAirN),
        349 => Some(MotionState::SpecialSStart),
        350 => Some(MotionState::SpecialS),
        351 => Some(MotionState::SpecialAirSStart),
        352 => Some(MotionState::SpecialAirS),
        353 => Some(MotionState::SpecialHi),
        354 => Some(MotionState::SpecialAirHi),
        357 => Some(MotionState::SpecialLw),
        359 => Some(MotionState::SpecialAirLw),
        _ => None,
    }
}

fn slippi_action_state_name(action_state_id: u16) -> &'static str {
    match action_state_id {
        0 => "DeadDown",
        1 => "DeadLeft",
        2 => "DeadRight",
        3 => "DeadUp",
        4 => "DeadUpStar",
        5 => "DeadUpStarIce",
        6 => "DeadUpFall",
        7 => "DeadUpFallHitCamera",
        8 => "DeadUpFallHitCameraFlat",
        9 => "DeadUpFallIce",
        10 => "DeadUpFallHitCameraIce",
        11 => "Sleep",
        12 => "Rebirth",
        13 => "RebirthWait",
        14 => "Wait",
        15 => "WalkSlow",
        16 => "WalkMiddle",
        17 => "WalkFast",
        18 => "Turn",
        19 => "TurnRun",
        20 => "Dash",
        21 => "Run",
        22 => "RunDirect",
        23 => "RunBrake",
        24 => "KneeBend",
        25 => "JumpF",
        26 => "JumpB",
        27 => "JumpAerialF",
        28 => "JumpAerialB",
        29 => "Fall",
        30 => "FallF",
        31 => "FallB",
        32 => "FallAerial",
        33 => "FallAerialF",
        34 => "FallAerialB",
        35 => "FallSpecial",
        36 => "FallSpecialF",
        37 => "FallSpecialB",
        38 => "DamageFall",
        39 => "Squat",
        40 => "SquatWait",
        41 => "SquatRv",
        42 => "Landing",
        43 => "LandingFallSpecial",
        44 => "Attack11",
        45 => "Attack12",
        46 => "Attack13",
        47 => "Attack100Start",
        48 => "Attack100Loop",
        49 => "Attack100End",
        50 => "AttackDash",
        53 => "AttackS3S",
        56 => "AttackHi3",
        57 => "AttackLw3",
        60 => "AttackS4S",
        63 => "AttackHi4",
        64 => "AttackLw4",
        65 => "AttackAirN",
        66 => "AttackAirF",
        67 => "AttackAirB",
        68 => "AttackAirHi",
        69 => "AttackAirLw",
        70 => "LandingAirN",
        71 => "LandingAirF",
        72 => "LandingAirB",
        73 => "LandingAirHi",
        74 => "LandingAirLw",
        212 => "Catch",
        214 => "CatchDash",
        178 => "GuardOn",
        179 => "Guard",
        180 => "GuardOff",
        181 => "GuardSetOff",
        182 => "GuardReflect",
        233 => "EscapeF",
        234 => "EscapeB",
        235 => "EscapeN",
        236 => "EscapeAir",
        244 => "Pass",
        252 => "CliffCatch",
        253 => "CliffWait",
        254 => "CliffClimbSlow",
        255 => "CliffClimbQuick",
        256 => "CliffAttackSlow",
        257 => "CliffAttackQuick",
        258 => "CliffEscapeSlow",
        259 => "CliffEscapeQuick",
        260 => "CliffJumpSlow1",
        261 => "CliffJumpSlow2",
        262 => "CliffJumpQuick1",
        263 => "CliffJumpQuick2",
        322 => "Entry",
        323 => "EntryStart",
        324 => "EntryEnd",
        347 => "SpecialN",
        348 => "SpecialAirN",
        349 => "SpecialSStart",
        350 => "SpecialS",
        351 => "SpecialAirSStart",
        352 => "SpecialAirS",
        353 => "SpecialHi",
        354 => "SpecialAirHi",
        357 => "SpecialLw",
        359 => "SpecialAirLw",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SourceHitElementFilter;
    use crate::{source_collision_frame_from_frame, source_collision_step_from_frame, RenderFrame};
    use mole_core::collision::{
        capsules_intersect_3d, capsules_intersect_3d_with_hurt_matrix, Capsule3,
        SourceCollisionFrame, Vec3,
    };

    #[test]
    fn slippi_action_state_table_covers_existing_rust_motion_aliases() {
        for (action_state_id, motion_state) in [
            (39, MotionState::Squat),
            (40, MotionState::SquatWait),
            (41, MotionState::SquatRv),
            (44, MotionState::Attack1),
            (50, MotionState::AttackDash),
            (65, MotionState::AttackAirN),
            (68, MotionState::AttackAirHi),
            (212, MotionState::Catch),
            (214, MotionState::CatchDash),
            (347, MotionState::SpecialN),
            (348, MotionState::SpecialAirN),
            (353, MotionState::SpecialHi),
            (354, MotionState::SpecialAirHi),
            (357, MotionState::SpecialLw),
            (359, MotionState::SpecialAirLw),
            (38, MotionState::DamageFall),
        ] {
            assert_eq!(
                slippi_action_state_to_motion(action_state_id),
                Some(motion_state),
                "Slippi action state {action_state_id} should map to the existing Rust alias"
            );
        }
    }

    #[test]
    fn slippi_action_state_table_knows_jab_followups_without_motion_aliasing() {
        for (action_state_id, name) in [
            (45, "Attack12"),
            (46, "Attack13"),
            (47, "Attack100Start"),
            (48, "Attack100Loop"),
            (49, "Attack100End"),
        ] {
            assert_eq!(slippi_action_state_name(action_state_id), name);
            assert_eq!(
                slippi_action_state_to_motion(action_state_id),
                None,
                "{name} stays source-only instead of becoming a Rust MotionState alias"
            );
            assert_eq!(
                slippi_action_state_to_expected(action_state_id),
                Some(SlippiExpectedActionState {
                    action_state_id: MeleeActionStateId::new(action_state_id),
                    motion_state: None,
                }),
                "{name} should still be supported by canonical action-state comparison"
            );
        }
    }

    #[test]
    fn slippi_action_state_table_maps_decomp_cliff_options() {
        for (action_state_id, motion_state, name, source_action_key) in [
            (
                254,
                MotionState::CliffClimbSlow,
                "CliffClimbSlow",
                "CliffClimbSlow",
            ),
            (
                255,
                MotionState::CliffClimbQuick,
                "CliffClimbQuick",
                "CliffClimbQuick",
            ),
            (
                256,
                MotionState::CliffAttackSlow,
                "CliffAttackSlow",
                "CliffAttackSlow",
            ),
            (
                257,
                MotionState::CliffAttackQuick,
                "CliffAttackQuick",
                "CliffAttackQuick",
            ),
            (
                258,
                MotionState::CliffEscapeSlow,
                "CliffEscapeSlow",
                "CliffEscapeSlow",
            ),
            (
                259,
                MotionState::CliffEscapeQuick,
                "CliffEscapeQuick",
                "CliffEscapeQuick",
            ),
            (
                260,
                MotionState::CliffJumpSlow1,
                "CliffJumpSlow1",
                "CliffJumpSlow1",
            ),
            (
                261,
                MotionState::CliffJumpSlow2,
                "CliffJumpSlow2",
                "CliffJumpSlow2",
            ),
            (
                262,
                MotionState::CliffJumpQuick1,
                "CliffJumpQuick1",
                "CliffJumpQuick1",
            ),
            (
                263,
                MotionState::CliffJumpQuick2,
                "CliffJumpQuick2",
                "CliffJumpQuick2",
            ),
        ] {
            assert_eq!(slippi_action_state_name(action_state_id), name);
            assert_eq!(
                slippi_action_state_to_motion(action_state_id),
                Some(motion_state),
                "{name} should be a supported decomp common motion state"
            );
            assert_eq!(
                melee_action_state_id_for_motion_state(motion_state),
                MeleeActionStateId::new(action_state_id)
            );
            let identity = slippi_action_identity_for_action_state_id(MeleeActionStateId::new(
                action_state_id,
            ));
            assert_eq!(identity.source_action_key, Some(source_action_key));
        }
    }

    #[test]
    fn slippi_action_identity_separates_motion_state_ids_from_source_table_indices() {
        let escape_air = slippi_action_identity_for_action_state_id(MeleeActionStateId::new(236));
        assert_eq!(
            escape_air.melee_motion_state_id,
            Some(MeleeMotionStateId::new(236))
        );
        assert_eq!(
            escape_air.source_action_table_index,
            Some(SourceActionTableIndex::new(44))
        );
        assert_eq!(escape_air.source_action_key, Some("EscapeAir"));

        let throw_f = slippi_action_identity_for_action_state_id(MeleeActionStateId::new(219));
        assert_eq!(throw_f.melee_motion_state_id, None);
        assert_eq!(
            throw_f.source_action_table_index,
            Some(SourceActionTableIndex::new(247))
        );
        assert_eq!(throw_f.source_action_key, Some("ThrowF"));

        let damage_fall = slippi_action_identity_for_action_state_id(MeleeActionStateId::new(38));
        assert_eq!(
            damage_fall.melee_motion_state_id,
            Some(MeleeMotionStateId::new(38))
        );
        assert_eq!(
            damage_fall.source_action_table_index,
            Some(SourceActionTableIndex::new(1))
        );
        assert_eq!(damage_fall.source_action_key, Some("DamageFall"));
    }

    #[test]
    fn slippi_velocity_classifier_uses_air_speed_for_aerial_attacks() {
        for motion_state in [
            MotionState::AttackAirN,
            MotionState::AttackAirF,
            MotionState::AttackAirB,
            MotionState::AttackAirHi,
            MotionState::AttackAirLw,
            MotionState::DamageFall,
        ] {
            assert!(
                !slippi_motion_state_uses_ground_velocity(motion_state),
                "{motion_state:?} should compare against Slippi air_x, not ground_x"
            );
        }
    }

    #[test]
    fn slippi_velocity_classifier_uses_air_speed_for_captain_special_hi() {
        for motion_state in [MotionState::SpecialHi, MotionState::SpecialAirHi] {
            assert!(
                !slippi_motion_state_uses_ground_velocity(motion_state),
                "{motion_state:?} should compare against Slippi air_x; ftCa_SpecialHi_Phys drives fp->self_vel and leaves ground_x dormant"
            );
        }
    }

    #[test]
    fn slippi_effective_input_preserves_compact_ucf_cardinal_stick() {
        let pre = SlippiPreFrame {
            action_state_id: None,
            position: None,
            facing: None,
            main_stick: Some([-0.9875, 0.0]),
            c_stick: Some([0.0, 0.0]),
            trigger: None,
            physical_l_trigger: None,
            physical_r_trigger: None,
            rust_player_input: Some(SlippiRustPlayerInput {
                stick_x: -127,
                stick_y: 0,
                c_stick_x: 0,
                c_stick_y: 0,
                left_trigger: 0,
                right_trigger: 0,
                physical_button_bits: 0,
                ucf_dashback_amendment: true,
                ucf_shield_drop_amendment: true,
            }),
        };

        let input = effective_slippi_player_input(&pre);

        assert_eq!(input.stick_x, -127);
        assert!(input.ucf_dashback_amendment);
        assert!(input.ucf_shield_drop_amendment);
    }

    #[test]
    fn slippi_effective_input_deadzone_cleans_compact_replay_noise() {
        let pre = SlippiPreFrame {
            action_state_id: None,
            position: None,
            facing: None,
            main_stick: Some([0.0, 0.0]),
            c_stick: Some([0.0, 0.0]),
            trigger: None,
            physical_l_trigger: None,
            physical_r_trigger: None,
            rust_player_input: Some(SlippiRustPlayerInput {
                stick_x: -32,
                stick_y: 5,
                c_stick_x: 0,
                c_stick_y: 0,
                left_trigger: 0,
                right_trigger: 0,
                physical_button_bits: 0,
                ucf_dashback_amendment: false,
                ucf_shield_drop_amendment: false,
            }),
        };

        let input = effective_slippi_player_input(&pre);

        assert_eq!((input.stick_x, input.stick_y), (0, 0));
    }

    #[test]
    fn divergence_velocity_tolerance_allows_equal_milli_rounding_delta() {
        let mut actual_player = World::for_two_players().snapshot().players[0];
        actual_player.position = Vec2 { x: 0, y: 0 };
        actual_player.velocity = Vec2 { x: 0, y: 0 };
        actual_player.grounded = true;
        actual_player.motion_state = MotionState::Wait;
        actual_player.melee_action_state_id = Some(MeleeActionStateId::new(14));
        actual_player.ground_velocity_x = 0.0;

        let post = SlippiPostFrame {
            action_state_id: 14,
            action_state_counter: Some(0.0),
            position: Some([0.0, 0.0]),
            facing: None,
            airborne: Some(false),
            self_induced_speeds: Some(SlippiSelfInducedSpeeds {
                ground_x: Some(0.001),
                air_x: Some(0.0),
                y: Some(0.0),
                attack_x: Some(0.0),
                attack_y: Some(0.0),
            }),
        };

        let (row, status) = trace_row_and_divergence_status(
            Frame(0),
            0,
            0,
            SlippiRustPlayerInput::default(),
            &post,
            actual_player,
            StageProfile::battlefield(),
            SIGNIFICANT_POSITION_DRIFT_MILLI,
            1,
        );

        assert_eq!(row.expected_ground_velocity_x, 1);
        assert_eq!(row.actual_velocity_x, 0);
        assert!(status.is_none());
    }

    #[test]
    fn escape_air_soft_platform_commit_is_mixed_phase_witness() {
        let mut actual_player = World::for_two_players().snapshot().players[1];
        actual_player.position = Vec2 {
            x: 32_781,
            y: 27_200,
        };
        actual_player.source_position = SourceVec2 {
            x: 32.781_364,
            y: 27.200_1,
        };
        actual_player.velocity = Vec2 {
            x: -1_827,
            y: -2_108,
        };
        actual_player.grounded = true;
        actual_player.motion_state = MotionState::LandingFallSpecial;
        actual_player.melee_action_state_id = Some(MeleeActionStateId::new(43));
        actual_player.ground_velocity_x = -1.827_256;
        actual_player.source_coll_floor_surface_index = Some(2);
        actual_player.source_coll_floor_line_index = Some(4);
        actual_player.source_coll_env_flags = 0x1_8000;

        let post: SlippiPostFrame = serde_json::from_str(
            r#"{
                "action_state_id": 236,
                "position": [32.781368, 23.111729],
                "self_induced_speeds": {
                    "ground_x": 0.0,
                    "air_x": -1.827,
                    "y": -2.108,
                    "attack_x": 0.0,
                    "attack_y": 0.0
                }
            }"#,
        )
        .unwrap();

        let (_, status) = trace_row_and_divergence_status(
            Frame(142),
            142,
            1,
            SlippiRustPlayerInput::default(),
            &post,
            actual_player,
            StageProfile::battlefield(),
            SIGNIFICANT_POSITION_DRIFT_MILLI,
            1,
        );

        assert_eq!(
            status.map(|status| status.kind),
            Some(SlippiCoreDivergenceKind::MixedPhaseWitness)
        );
    }

    #[test]
    fn mixed_phase_witness_is_not_first_slippi_divergence_candidate() {
        assert!(!is_first_slippi_divergence_candidate(
            SlippiCoreDivergenceKind::MixedPhaseWitness
        ));
        assert!(is_first_slippi_divergence_candidate(
            SlippiCoreDivergenceKind::StateMismatch
        ));
        assert!(is_first_slippi_divergence_candidate(
            SlippiCoreDivergenceKind::PositionDrift
        ));
        assert!(is_first_slippi_divergence_candidate(
            SlippiCoreDivergenceKind::VelocityDrift
        ));
        assert!(is_first_slippi_divergence_candidate(
            SlippiCoreDivergenceKind::UnsupportedState
        ));
    }

    #[test]
    fn source_damage_velocity_uses_slippi_attack_speed_channels() {
        let mut actual_player = World::for_two_players().snapshot().players[0];
        actual_player.position = Vec2 {
            x: 16_350,
            y: 1_056,
        };
        actual_player.velocity = Vec2 { x: 652, y: 1_056 };
        actual_player.grounded = false;
        actual_player.motion_state = MotionState::Wait;
        actual_player.melee_action_state_id = Some(MeleeActionStateId::new(79));
        actual_player.source_self_velocity_x = 0.0;
        actual_player.source_self_velocity_y = -0.13;
        actual_player.source_knockback_velocity_x = 0.65203;
        actual_player.source_knockback_velocity_y = 1.186329;

        let post: SlippiPostFrame = serde_json::from_str(
            r#"{
                "action_state_id": 79,
                "action_state_counter": 2,
                "position": [16.350126, 1.056429],
                "self_induced_speeds": {
                    "ground_x": 0,
                    "air_x": 0,
                    "y": -0.13,
                    "attack_x": 0.65203,
                    "attack_y": 1.186329
                }
            }"#,
        )
        .unwrap();

        let (row, status) = trace_row_and_divergence_status(
            Frame(0),
            2257,
            1,
            SlippiRustPlayerInput::default(),
            &post,
            actual_player,
            StageProfile::battlefield(),
            SIGNIFICANT_POSITION_DRIFT_MILLI,
            1,
        );

        assert_eq!(row.expected_air_velocity_x, 0);
        assert_eq!(row.expected_velocity_y, -130);
        assert!(status.is_none());
    }

    #[test]
    fn visual_replay_inputs_sort_export_frames_and_preserve_ucf_bits() {
        let export = r#"{
            "schema_version": 1,
            "frames": [
                {
                    "frame": 5,
                    "players": {
                        "0": {
                            "pre": {
                                "rust_player_input": {
                                    "stick_x": 122,
                                    "stick_y": 0,
                                    "physical_button_bits": 1024,
                                    "ucf_dashback_amendment": true,
                                    "ucf_shield_drop_amendment": true
                                }
                            }
                        }
                    }
                },
                {
                    "frame": 4,
                    "players": {
                        "1": {
                            "pre": {
                                "rust_player_input": {
                                    "stick_x": -64,
                                    "stick_y": 8,
                                    "physical_button_bits": 512
                                }
                            }
                        }
                    }
                }
            ]
        }"#;

        let frames = slippi_visual_replay_inputs_from_match_start(export, None).unwrap();

        assert_eq!(frames.len(), 2);
        assert_eq!((frames[0].core_frame.0, frames[0].source_frame), (0, 4));
        assert_eq!((frames[1].core_frame.0, frames[1].source_frame), (1, 5));
        assert_eq!(frames[0].inputs[1].stick_x(), -64);
        assert!(frames[0].inputs[1].special());
        assert_eq!(frames[1].inputs[0].stick_x(), 122);
        assert!(frames[1].inputs[0].jump_primary());
        assert!(frames[1].inputs[0].ucf_dashback_amendment());
        assert!(frames[1].inputs[0].ucf_shield_drop_amendment());
    }

    #[test]
    fn visual_replay_world_uses_match_settings_costumes() {
        let export = r#"{
            "schema_version": 1,
            "settings": {
                "players": {
                    "0": { "character_color": 2 },
                    "1": { "character_color": 5 }
                }
            },
            "frames": []
        }"#;

        let world = slippi_match_start_world_from_export(export).unwrap();

        assert_eq!(world.players()[0].costume_index, 2);
        assert_eq!(world.players()[1].costume_index, 5);
        assert_eq!(
            world,
            world_for_slippi_export(&serde_json::from_str(export).unwrap())
        );
    }

    #[test]
    fn visual_replay_divergence_helper_compares_live_snapshot_to_post_frame() {
        let export = r#"{
            "frames": [
                {
                    "frame": 0,
                    "players": {
                        "0": {
                            "pre": {
                                "rust_player_input": {
                                    "stick_x": 0,
                                    "stick_y": 0,
                                    "physical_button_bits": 0
                                }
                            },
                            "post": {
                                "action_state_id": 322,
                                "position": [10.0, 0.0],
                                "facing": 1.0,
                                "self_induced_speeds": {
                                    "ground_x": 0.0,
                                    "air_x": 0.0,
                                    "y": 0.0
                                }
                            }
                        }
                    }
                }
            ]
        }"#;

        let frames = slippi_visual_replay_inputs_from_match_start(export, None).unwrap();
        let world = World::for_slippi_battlefield_singles_match_start();

        let divergence = slippi_visual_replay_divergence_for_frame(
            &frames[0],
            world.snapshot(),
            SlippiCoreDivergenceScanConfig::default(),
        )
        .expect("Wait at spawn should position-drift from source x=10");

        assert_eq!(divergence.kind, SlippiCoreDivergenceKind::PositionDrift);
        assert_eq!(divergence.player_index, 0);
        assert_eq!(divergence.source_frame, 0);
        assert_eq!(divergence.row.expected_position.x, 10_000);
        assert_eq!(divergence.row.actual_motion_state, MotionState::Entry);
    }

    #[test]
    fn visual_replay_divergence_gate_ignores_transient_realigning_drift() {
        let mut gate = SlippiVisualReplayDivergenceGate::new(3);
        let divergence = test_visual_replay_divergence(Frame(10), 100);

        assert!(gate.observe(Some(divergence.clone())).is_none());
        assert!(gate.observe(Some(divergence.clone())).is_none());
        assert!(gate.observe(Some(divergence)).is_none());
        assert!(
            gate.observe(None).is_none(),
            "a three-frame drift that realigns before lookahead expires should not stop visual replay"
        );
    }

    #[test]
    fn visual_replay_divergence_gate_reports_first_persistent_divergence() {
        let mut gate = SlippiVisualReplayDivergenceGate::new(3);
        let first = test_visual_replay_divergence(Frame(10), 100);

        assert!(gate.observe(Some(first.clone())).is_none());
        assert!(gate
            .observe(Some(test_visual_replay_divergence(Frame(11), 101)))
            .is_none());
        assert!(gate
            .observe(Some(test_visual_replay_divergence(Frame(12), 102)))
            .is_none());
        let reported = gate
            .observe(Some(test_visual_replay_divergence(Frame(13), 103)))
            .expect("persistent divergence should report after lookahead expires");

        assert_eq!(reported.core_frame, first.core_frame);
        assert_eq!(reported.source_frame, first.source_frame);
    }

    #[test]
    fn visual_replay_stops_on_mixed_phase_witness_rows() {
        assert!(
            visual_replay_should_stop_on_divergence_kind(
                SlippiCoreDivergenceKind::MixedPhaseWitness
            ),
            "mixed-phase is diagnostic metadata, not permission for live replay to continue past an observed disagreement"
        );
        assert!(visual_replay_should_stop_on_divergence_kind(
            SlippiCoreDivergenceKind::VelocityDrift
        ));
    }

    #[test]
    fn composed_velocity_channel_row_is_mixed_phase_witness() {
        let mut row = test_visual_replay_divergence(Frame(5254), 5131).row;
        row.expected_action_state_id = MeleeActionStateId::new(263);
        row.actual_action_state_id = Some(MeleeActionStateId::new(263));
        row.expected_motion_state = Some(MotionState::CliffJumpQuick2);
        row.actual_motion_state = MotionState::CliffJumpQuick2;
        row.actual_motion_frame = 1;
        row.expected_position = Vec2 { x: -67_900, y: 589 };
        row.actual_position = row.expected_position;
        row.expected_air_velocity_x = 1_000;
        row.actual_velocity_x = 1_000;
        row.expected_velocity_y = 3_300;
        row.actual_velocity_y = 3_300;
        row.expected_composed_velocity_x = 0;
        row.expected_composed_velocity_y = 3_300;

        assert!(is_same_state_composed_velocity_channel_mixed_phase_witness(
            &row
        ));
    }

    #[test]
    fn visual_replay_gate_reports_mixed_phase_witness_immediately() {
        let mut gate = SlippiVisualReplayDivergenceGate::new(0);
        let mut mixed_root = test_visual_replay_divergence(Frame(2748), 2625);
        mixed_root.kind = SlippiCoreDivergenceKind::MixedPhaseWitness;
        let reported = gate
            .observe(Some(mixed_root))
            .expect("live replay must stop on the first observed disagreement");
        assert_eq!(reported.source_frame, 2625);
        assert_eq!(reported.kind, SlippiCoreDivergenceKind::MixedPhaseWitness);
    }

    #[test]
    fn visual_replay_gate_ignores_proven_same_state_floor_commit_telemetry() {
        let mut gate = SlippiVisualReplayDivergenceGate::new(0);
        let mut witness = test_visual_replay_divergence(Frame(2416), 2293);
        witness.kind = SlippiCoreDivergenceKind::MixedPhaseWitness;
        witness.row.expected_action_state_id = MeleeActionStateId::new(42);
        witness.row.actual_action_state_id = Some(MeleeActionStateId::new(42));
        witness.row.expected_motion_state = Some(MotionState::Landing);
        witness.row.actual_motion_state = MotionState::Landing;
        witness.row.actual_grounded = true;
        witness.row.expected_position = Vec2 { x: 43_403, y: 0 };
        witness.row.actual_position = witness.row.expected_position;
        witness.row.expected_velocity_y = -2_080;
        witness.row.expected_attack_velocity_y = 646;
        witness.row.expected_composed_velocity_y = -2_080;
        witness.row.actual_velocity_y = -1_434;
        witness.row.actual_source_self_velocity_y = -2.079_999_9;
        witness.row.actual_source_knockback_velocity_y = 0.645_720_5;
        witness.row.actual_source_floor_line = Some(1);
        witness.row.actual_source_coll_prev_env_flags = 0;
        witness.row.actual_source_coll_env_flags = 0x0001_8000;

        assert!(
            gate.observe(Some(witness)).is_none(),
            "live replay must compare Melee self_vel and x8c_kb_vel to their matching Rust source lanes instead of treating Rust's composed projection as the same field"
        );
    }

    #[test]
    fn landing_floor_commit_velocity_row_is_mixed_phase_witness() {
        let mut row = test_visual_replay_divergence(Frame(2416), 2293).row;
        row.expected_action_state_id = MeleeActionStateId::new(42);
        row.actual_action_state_id = Some(MeleeActionStateId::new(42));
        row.expected_motion_state = Some(MotionState::Landing);
        row.actual_motion_state = MotionState::Landing;
        row.actual_grounded = true;
        row.expected_velocity_y = -2080;
        row.expected_attack_velocity_y = 646;
        row.expected_composed_velocity_y = -2080;
        row.actual_velocity_y = -1434;
        row.actual_source_self_velocity_y = -2.0799999;
        row.actual_source_knockback_velocity_y = 0.6457205;
        row.actual_source_floor_line = Some(1);
        row.actual_source_coll_prev_env_flags = 0;
        row.actual_source_coll_env_flags = 0x0001_8000;

        assert!(
            is_same_state_floor_commit_velocity_mixed_phase_witness(&row),
            "source frame 2293 has post-floor Landing identity and floor flags, while the Slippi velocity fields still expose the pre-floor vertical self velocity plus stale attack_y"
        );
    }

    #[test]
    fn source_only_passive_floor_commit_velocity_row_is_mixed_phase_witness() {
        let mut row = test_visual_replay_divergence(Frame(2475), 2352).row;
        row.expected_action_state_id = MeleeActionStateId::new(201);
        row.actual_action_state_id = Some(MeleeActionStateId::new(201));
        row.expected_motion_state = None;
        row.actual_motion_state = MotionState::GuardOn;
        row.actual_grounded = true;
        row.expected_velocity_y = -2900;
        row.expected_composed_velocity_y = -2900;
        row.expected_attack_velocity_y = 0;
        row.actual_velocity_y = 0;
        row.actual_source_self_velocity_y = -2.9000001;
        row.actual_source_knockback_velocity_y = 1.1195498;
        row.actual_source_floor_line = Some(1);
        row.actual_source_coll_prev_env_flags = 0;
        row.actual_source_coll_env_flags = 0x0001_8000;

        assert!(
            is_same_state_floor_commit_velocity_mixed_phase_witness(&row),
            "source frame 2352 has matching PassiveStandB identity and post-floor collision flags, while Slippi still exposes the pre-floor vertical self velocity"
        );
    }

    #[test]
    fn passive_stand_horizontal_expected_velocity_keeps_ground_knockback_like_fighter_proc_update()
    {
        let expected = Some(SlippiExpectedActionState {
            action_state_id: MeleeActionStateId::new(201),
            motion_state: None,
        });

        assert_eq!(
            expected_composed_horizontal_velocity_x(expected, -181, 84),
            -97,
            "Fighter_procUpdate applies x8c_kb_vel on every state while it is nonzero; grounded PassiveStandB keeps the floor-tangent knockback component in the exported horizontal velocity"
        );
    }

    #[test]
    fn common_landing_expected_velocity_ignores_residual_slippi_attack_x_field() {
        let expected = Some(SlippiExpectedActionState {
            action_state_id: MeleeActionStateId::new(42),
            motion_state: Some(MotionState::Landing),
        });

        assert_eq!(
            expected_composed_horizontal_velocity_x(expected, 0, 853),
            0,
            "source frame 2294 has matching Landing state/position and Slippi still exposes residual ground knockback x, but the compared common-state velocity field is the grounded self velocity"
        );
    }

    #[test]
    fn mixed_phase_witness_marks_later_same_player_scenarios_as_cascade() {
        let row = test_visual_replay_divergence(Frame(10), 100).row;
        let mut scenarios = vec![
            test_core_divergence_scenario(
                SlippiCoreDivergenceKind::MixedPhaseWitness,
                1,
                Frame(265),
                142,
                false,
                row.clone(),
            ),
            test_core_divergence_scenario(
                SlippiCoreDivergenceKind::StateMismatch,
                1,
                Frame(266),
                143,
                false,
                row.clone(),
            ),
            test_core_divergence_scenario(
                SlippiCoreDivergenceKind::StateMismatch,
                0,
                Frame(266),
                143,
                false,
                row,
            ),
        ];

        mark_mixed_phase_cascades(&mut scenarios);

        assert_eq!(scenarios[0].cascades_from_source_frame, None);
        assert_eq!(scenarios[1].cascades_from_source_frame, Some(142));
        assert_eq!(scenarios[2].cascades_from_source_frame, None);
    }

    #[test]
    fn seeded_trace_uses_slippi_pre_frame_state_instead_of_sequential_history() {
        let export = r#"{
            "frames": [
                {
                    "frame": 0,
                    "players": {
                        "0": {
                            "pre": {
                                "position": [0.0, 0.0],
                                "facing": 1.0,
                                "action_state_id": 29,
                                "action_state_frame": 0.0,
                                "rust": { "stick_x": 0, "stick_y": 0, "physical_button_bits": 0 }
                            },
                            "post": {
                                "action_state_id": 29,
                                "position": [200.0, 0.0],
                                "facing": 1.0,
                                "self_induced_speeds": { "ground_x": 0.0, "air_x": 0.0, "y": 0.0 }
                            }
                        }
                    }
                },
                {
                    "frame": 1,
                    "players": {
                        "0": {
                            "pre": {
                                "position": [10.0, 20.0],
                                "facing": 1.0,
                                "action_state_id": 29,
                                "action_state_frame": 0.0,
                                "rust": { "stick_x": 0, "stick_y": 0, "physical_button_bits": 0 }
                            },
                            "post": {
                                "action_state_id": 29,
                                "position": [10.0, 20.0],
                                "facing": 1.0,
                                "self_induced_speeds": { "ground_x": 0.0, "air_x": 0.0, "y": 0.0 }
                            }
                        }
                    }
                }
            ]
        }"#;

        let trace = trace_slippi_export_seeded_pre_frame_with_core(
            export,
            SlippiCoreTraceConfig {
                player_index: 0,
                source_frame_start: 1,
                source_frame_end: 1,
                max_frames: None,
            },
        )
        .expect("seeded trace should parse");

        assert_eq!(trace.rows.len(), 1);
        assert_eq!(trace.rows[0].source_frame, 1);
        assert_eq!(trace.rows[0].actual_source_position.x, 10.0);
        assert!(
            trace.rows[0].actual_source_position.y > 19.0,
            "seeded trace should step from the Slippi pre-frame y=20.0 position, not from match-start history"
        );
    }

    #[test]
    fn seeded_scan_realigns_from_next_slippi_pre_frame_instead_of_cascading() {
        let export = r#"{
            "frames": [
                {
                    "frame": 0,
                    "players": {
                        "0": {
                            "pre": {
                                "position": [0.0, 0.0],
                                "facing": 1.0,
                                "action_state_id": 14,
                                "rust": { "stick_x": 0, "stick_y": 0, "physical_button_bits": 0 }
                            },
                            "post": {
                                "action_state_id": 14,
                                "position": [100.0, 0.0],
                                "facing": 1.0,
                                "self_induced_speeds": { "ground_x": 0.0, "air_x": 0.0, "y": 0.0 }
                            }
                        }
                    }
                },
                {
                    "frame": 1,
                    "players": {
                        "0": {
                            "pre": {
                                "position": [10.0, 0.0],
                                "facing": 1.0,
                                "action_state_id": 14,
                                "rust": { "stick_x": 0, "stick_y": 0, "physical_button_bits": 0 }
                            },
                            "post": {
                                "action_state_id": 14,
                                "position": [10.0, 0.0],
                                "facing": 1.0,
                                "self_induced_speeds": { "ground_x": 0.0, "air_x": 0.0, "y": 0.0 }
                            }
                        }
                    }
                }
            ]
        }"#;

        let scan = scan_slippi_export_seeded_pre_frame_with_core(
            export,
            SlippiCoreDivergenceScanConfig {
                compare_players: [true, false],
                max_frames: None,
                lookahead_frames: 3,
                max_scenarios: None,
                position_tolerance_milli: 100,
                velocity_tolerance_milli: 1,
            },
        )
        .expect("seeded scan should parse");

        assert_eq!(scan.scenarios.len(), 1);
        assert_eq!(
            scan.scenarios[0].kind,
            SlippiCoreDivergenceKind::PositionDrift
        );
        assert_eq!(scan.scenarios[0].source_frame, 0);
        assert_eq!(scan.scenarios[0].realign_source_frame, Some(1));
        assert_eq!(scan.scenarios[0].cascades_from_source_frame, None);
    }

    #[test]
    #[ignore = "local diagnostic requires an untracked full Slippi export"]
    fn diagnose_source_2454_attack_air_f_damage_confirm() {
        let input_export_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("debug/slippi/Game_20260530T214929.full.inputs.json");
        let text = fs::read_to_string(input_export_path)
            .expect("full Slippi input export must exist for local parity diagnosis");
        let mut export: SlippiExport =
            serde_json::from_str(&text).expect("full Slippi input export must parse");
        export.frames.sort_by_key(|frame| frame.frame);
        let mut world = World::for_slippi_battlefield_singles_match_start();

        for (core_frame_index, frame) in export.frames.into_iter().enumerate() {
            let mut inputs = [PlayerInput::neutral(); PLAYER_COUNT];
            for (player_index, input_slot) in inputs.iter_mut().enumerate() {
                if let Some(pre) = frame
                    .players
                    .get(&player_index.to_string())
                    .and_then(|player_frame| player_frame.pre.as_ref())
                {
                    *input_slot = effective_slippi_player_input(pre).to_player_input();
                }
            }

            let core_frame = Frame(core_frame_index as u32);
            mole_core::step_world_with_source_runtime_data(
                &mut world,
                core_frame,
                &inputs,
                crate::runtime_source_pose_metadata_for_player,
                crate::runtime_source_action_total_frames_for_action_state_id,
            );

            if (2448..=2454).contains(&frame.frame) {
                let render_frame = RenderFrame::from_world(&world);
                let p2_selection = crate::render_source_hitbox_selection(&render_frame, 1);
                let p2_direct_hit_count = crate::runtime_source_frame_capsules_ref(
                    p2_selection.source_action_key,
                    p2_selection.source_frame,
                )
                .map(|capsules| capsules.hit_capsules.len())
                .unwrap_or(0);
                let p2_filtered_hit_count = crate::runtime_source_frame_capsules_ref(
                    p2_selection.source_action_key,
                    p2_selection.source_frame,
                )
                .map(|capsules| {
                    capsules
                        .hit_capsules
                        .iter()
                        .filter(|capsule| SourceHitElementFilter::NonCatch.accepts(**capsule))
                        .filter(|capsule| {
                            !capsule.hitbox_flags.skip_if_thrown_hitbox_owner_absent()
                                || render_frame.player_source_thrown_hitbox_owner_indexes[1]
                                    .is_some()
                        })
                        .count()
                })
                .unwrap_or(0);
                let collision_frame = source_collision_frame_from_frame(&render_frame);
                let step = source_collision_step_from_frame(&mut world, &render_frame);
                let nearest = nearest_hit_hurt_gap_for_debug(&collision_frame, 1, 0);
                eprintln!(
                    "core={} source={} p2_anim={} p2_state={:?} p2_key={:?} p2_pose_id={:?} p2_pose_key={:?} p2_render_selection={:?} p2_owner={:?} p2_direct_hits={} p2_filtered_hits={} hits={} p2_hits={} hurts={} nearest_p2_p1={:?} geom={:?} raw={:?} logged={:?} confirms={:?} stages={:?} results={:?}",
                    core_frame.0,
                    frame.frame,
                    world.players()[1].source_motion_anim_frame,
                    world.players()[1].motion_state,
                    world.players()[1].source_action_key,
                    render_frame.player_source_pose_action_state_ids[1],
                    render_frame.player_source_pose_action_keys[1],
                    p2_selection,
                    render_frame.player_source_thrown_hitbox_owner_indexes[1],
                    p2_direct_hit_count,
                    p2_filtered_hit_count,
                    collision_frame.hits.len(),
                    collision_frame
                        .hits
                        .iter()
                        .filter(|hit| hit.owner_index == 1)
                        .count(),
                    collision_frame.hurts.len(),
                    nearest,
                    step.geometry_confirms,
                    step.raw_confirms,
                    step.logged_confirms,
                    step.confirms,
                    step.stages,
                    step.results
                );
            }

            if frame.frame == 2454 {
                eprintln!("p1={:?} p2={:?}", world.players()[0], world.players()[1]);
                return;
            }

            let source_step = crate::apply_source_collisions_for_world(&mut world);
            world.apply_source_damage_results_with_action_total_frames(
                &source_step.results,
                crate::runtime_source_action_total_frames_for_action_state_id,
            );
            world.commit_staged_source_damage();
        }

        panic!("source frame 2454 was not present in the replay export");
    }

    #[test]
    #[ignore = "local diagnostic requires an untracked Slippi export"]
    fn diagnose_source_2662_attack_air_hi_guard_damage_confirm() {
        let input_export_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("debug/slippi/Game_20260530T214929.inputs.json");
        let text = fs::read_to_string(input_export_path)
            .expect("Slippi input export must exist for local parity diagnosis");
        let mut export: SlippiExport =
            serde_json::from_str(&text).expect("Slippi input export must parse");
        export.frames.sort_by_key(|frame| frame.frame);
        let mut world = World::for_slippi_battlefield_singles_match_start();

        for (core_frame_index, frame) in export.frames.into_iter().enumerate() {
            let (inputs, _) = slippi_frame_inputs(&frame);
            let core_frame = Frame(core_frame_index as u32);
            crate::step_world_with_source_runtime_data(
                &mut world,
                core_frame,
                &inputs,
                crate::runtime_source_pose_metadata_for_player,
                crate::runtime_source_action_total_frames_for_action_state_id,
            );
            crate::update_source_shield_hit_positions(&mut world);

            if (2661..=2663).contains(&frame.frame) {
                let render_frame = RenderFrame::from_world(&world);
                let collision_frame = source_collision_frame_from_frame(&render_frame);
                let p1_selection = crate::render_source_hitbox_selection(&render_frame, 0);
                let p2_selection = crate::render_source_hitbox_selection(&render_frame, 1);
                let nearest = nearest_hit_hurt_gap_for_debug(&collision_frame, 0, 1);
                let step = source_collision_step_from_frame(&mut world, &render_frame);
                eprintln!(
                    "core={} source={} p1_state={:?} p1_action={:?} p1_motion_frame={} p1_anim={} p1_selection={:?} p1_pos={:?} p2_state={:?} p2_action={:?} p2_motion_frame={} p2_anim={} p2_selection={:?} p2_pos={:?} p2_shield_active={} p2_shield_hit_active={} p2_shield_pos={:?} hits={} hurts={} nearest_p1_p2={:?} geom={:?} raw={:?} logged={:?} confirms={:?} stages={:?} results={:?}",
                    core_frame.0,
                    frame.frame,
                    world.players()[0].motion_state,
                    world.players()[0].melee_action_state_id,
                    world.players()[0].motion_frame,
                    world.players()[0].source_motion_anim_frame,
                    p1_selection,
                    world.players()[0].source_position,
                    world.players()[1].motion_state,
                    world.players()[1].melee_action_state_id,
                    world.players()[1].motion_frame,
                    world.players()[1].source_motion_anim_frame,
                    p2_selection,
                    world.players()[1].source_position,
                    world.players()[1].source_shield_collision_active,
                    world.players()[1].source_shield_hit_active,
                    world.players()[1].source_shield_hit_position,
                    collision_frame.hits.len(),
                    collision_frame.hurts.len(),
                    nearest,
                    step.geometry_confirms,
                    step.raw_confirms,
                    step.logged_confirms,
                    step.confirms,
                    step.stages,
                    step.results
                );
                world.apply_source_damage_results_with_action_total_frames(
                    &step.results,
                    crate::runtime_source_action_total_frames_for_action_state_id,
                );
                world.commit_staged_source_damage();
                continue;
            }

            let source_step = crate::apply_source_collisions_for_world(&mut world);
            world.apply_source_damage_results_with_action_total_frames(
                &source_step.results,
                crate::runtime_source_action_total_frames_for_action_state_id,
            );
            world.commit_staged_source_damage();

            if frame.frame > 2663 {
                return;
            }
        }

        panic!("source frame 2662 was not present in the replay export");
    }

    fn nearest_hit_hurt_gap_for_debug(
        frame: &SourceCollisionFrame,
        attacker_index: usize,
        victim_index: usize,
    ) -> Option<(
        u64,
        u64,
        Capsule3,
        Capsule3,
        f32,
        f32,
        bool,
        bool,
        Option<bool>,
    )> {
        let mut nearest: Option<(
            u64,
            u64,
            Capsule3,
            Capsule3,
            f32,
            f32,
            bool,
            bool,
            Option<bool>,
        )> = None;
        for hit in frame
            .hits
            .iter()
            .filter(|hit| hit.owner_index == attacker_index)
        {
            for hurt in frame
                .hurts
                .iter()
                .filter(|hurt| hurt.owner_index == victim_index)
            {
                let distance = segment_distance_for_debug(hit.capsule, hurt.capsule);
                let allowed = hit.capsule.radius + hurt.capsule.radius;
                let intersects = capsules_intersect_3d(&hit.capsule, &hurt.capsule);
                let matrix_intersects = capsules_intersect_3d_with_hurt_matrix(
                    &hit.capsule,
                    &hurt.capsule,
                    hurt.hurt_matrix,
                );
                let previous_matrix_intersects = hurt.previous_capsule.map(|previous| {
                    capsules_intersect_3d_with_hurt_matrix(
                        &hit.capsule,
                        &previous,
                        hurt.hurt_matrix,
                    )
                });
                let candidate = (
                    hit.capsule_id,
                    hurt.capsule_id,
                    hit.capsule,
                    hurt.capsule,
                    distance,
                    allowed,
                    intersects,
                    matrix_intersects,
                    previous_matrix_intersects,
                );
                if nearest
                    .map(|(_, _, _, _, best_distance, best_allowed, _, _, _)| {
                        distance - allowed < best_distance - best_allowed
                    })
                    .unwrap_or(true)
                {
                    nearest = Some(candidate);
                }
            }
        }
        nearest
    }

    fn segment_distance_for_debug(a: Capsule3, b: Capsule3) -> f32 {
        segment_distance_sq_for_debug(a.a, a.b, b.a, b.b).sqrt()
    }

    fn segment_distance_sq_for_debug(p1: Vec3, q1: Vec3, p2: Vec3, q2: Vec3) -> f32 {
        let d1 = sub3_for_debug(q1, p1);
        let d2 = sub3_for_debug(q2, p2);
        let r = sub3_for_debug(p1, p2);
        let a = dot3_for_debug(d1, d1);
        let e = dot3_for_debug(d2, d2);
        let f = dot3_for_debug(d2, r);
        let mut s;
        let mut t;

        if a <= f32::EPSILON && e <= f32::EPSILON {
            return dot3_for_debug(r, r);
        }
        if a <= f32::EPSILON {
            s = 0.0;
            t = (f / e).clamp(0.0, 1.0);
        } else {
            let c = dot3_for_debug(d1, r);
            if e <= f32::EPSILON {
                t = 0.0;
                s = (-c / a).clamp(0.0, 1.0);
            } else {
                let b = dot3_for_debug(d1, d2);
                let denom = a * e - b * b;
                if denom.abs() > f32::EPSILON {
                    s = ((b * f - c * e) / denom).clamp(0.0, 1.0);
                } else {
                    s = 0.0;
                }
                t = (b * s + f) / e;
                if t < 0.0 {
                    t = 0.0;
                    s = (-c / a).clamp(0.0, 1.0);
                } else if t > 1.0 {
                    t = 1.0;
                    s = ((b - c) / a).clamp(0.0, 1.0);
                }
            }
        }

        let c1 = add3_for_debug(p1, mul3_for_debug(d1, s));
        let c2 = add3_for_debug(p2, mul3_for_debug(d2, t));
        dot3_for_debug(sub3_for_debug(c1, c2), sub3_for_debug(c1, c2))
    }

    fn dot3_for_debug(a: Vec3, b: Vec3) -> f32 {
        a.x * b.x + a.y * b.y + a.z * b.z
    }

    fn sub3_for_debug(a: Vec3, b: Vec3) -> Vec3 {
        Vec3::new(a.x - b.x, a.y - b.y, a.z - b.z)
    }

    fn add3_for_debug(a: Vec3, b: Vec3) -> Vec3 {
        Vec3::new(a.x + b.x, a.y + b.y, a.z + b.z)
    }

    fn mul3_for_debug(v: Vec3, scalar: f32) -> Vec3 {
        Vec3::new(v.x * scalar, v.y * scalar, v.z * scalar)
    }

    fn test_core_divergence_scenario(
        kind: SlippiCoreDivergenceKind,
        player_index: usize,
        core_frame: Frame,
        source_frame: i32,
        realigned_within_lookahead: bool,
        first_frame: SlippiCoreTraceRow,
    ) -> SlippiCoreDivergenceScenario {
        SlippiCoreDivergenceScenario {
            scenario_index: 0,
            kind,
            player_index,
            core_frame,
            source_frame,
            end_core_frame: core_frame,
            end_source_frame: source_frame,
            duration_frames: 1,
            realigned_within_lookahead,
            realign_core_frame: None,
            realign_source_frame: None,
            rollback_replay_deterministic: false,
            first_frame,
            max_abs_position_delta_milli: 0,
            max_abs_velocity_delta_milli: 0,
            cascades_from_source_frame: None,
        }
    }

    fn test_visual_replay_divergence(
        core_frame: Frame,
        source_frame: i32,
    ) -> SlippiVisualReplayDivergence {
        SlippiVisualReplayDivergence {
            kind: SlippiCoreDivergenceKind::PositionDrift,
            player_index: 1,
            core_frame,
            source_frame,
            max_abs_position_delta_milli: SIGNIFICANT_POSITION_DRIFT_MILLI,
            max_abs_velocity_delta_milli: 0,
            row: SlippiCoreTraceRow {
                core_frame,
                source_frame,
                player_index: 1,
                input_stick_x: 0,
                input_stick_y: 0,
                input_button_bits: 0,
                input_left_trigger: 0,
                input_right_trigger: 0,
                input_ucf_dashback_amendment: false,
                input_ucf_shield_drop_amendment: false,
                actual_input_jump_pressed: false,
                actual_input_normal_jump_pressed: false,
                actual_input_shield_held: false,
                actual_input_shield_pressed: false,
                expected_slippi_state_id: 29,
                expected_action_state_id: MeleeActionStateId::new(29),
                actual_action_state_id: Some(MeleeActionStateId::new(29)),
                expected_action_identity: slippi_action_identity_for_action_state_id(
                    MeleeActionStateId::new(29),
                ),
                actual_action_identity: slippi_action_identity_for_action_state_id(
                    MeleeActionStateId::new(29),
                ),
                expected_motion_state: Some(MotionState::Fall),
                actual_motion_state: MotionState::Fall,
                actual_grounded: false,
                actual_motion_frame: 0,
                actual_motion_anim_frame_milli: 0,
                actual_source_fall_anim_blend: 0.0,
                actual_source_fall_anim_pose: MotionState::Fall,
                expected_facing: 1,
                actual_facing: 1,
                expected_position: Vec2 { x: 500, y: 0 },
                actual_position: Vec2 { x: 0, y: 0 },
                expected_source_position: SourceVec2::default(),
                actual_source_position: SourceVec2::default(),
                expected_ground_velocity_x: 0,
                expected_air_velocity_x: 0,
                expected_velocity_y: 0,
                expected_attack_velocity_x: 0,
                expected_attack_velocity_y: 0,
                expected_composed_velocity_x: 0,
                expected_composed_velocity_y: 0,
                expected_ground_velocity_x_source: 0.0,
                expected_air_velocity_x_source: 0.0,
                expected_velocity_y_source: 0.0,
                expected_attack_velocity_x_source: 0.0,
                expected_attack_velocity_y_source: 0.0,
                actual_velocity_x: 0,
                actual_velocity_y: 0,
                actual_source_self_velocity_x: 0.0,
                actual_source_self_velocity_y: 0.0,
                actual_source_knockback_velocity_x: 0.0,
                actual_source_knockback_velocity_y: 0.0,
                actual_source_ground_knockback_velocity: 0.0,
                actual_player_nudge_x: 0.0,
                actual_player_nudge_z: 0.0,
                actual_ground_velocity_x: 0.0,
                actual_ground_accel_x: 0.0,
                actual_ground_accel_x2: 0.0,
                actual_dash_entry_velocity_delta: 0.0,
                actual_dash_x0: 0.0,
                actual_source_coll_last_pos: SourceVec2::default(),
                actual_source_coll_cur_pos: SourceVec2::default(),
                actual_source_coll_prev_pos: SourceVec2::default(),
                actual_source_coll_ecb: SourceCollEcbSnapshot::default(),
                actual_source_coll_prev_ecb: SourceCollEcbSnapshot::default(),
                actual_source_coll_desired_ecb: SourceCollEcbSnapshot::default(),
                actual_ecb_bottom_lock_timer: 0,
                actual_source_coll_x130_locked: false,
                actual_floor_skip_surface: None,
                actual_source_floor_skip_line: None,
                actual_source_floor_surface: None,
                actual_source_floor_line: None,
                actual_source_coll_env_flags: 0,
                actual_source_coll_prev_env_flags: 0,
            },
        }
    }
}

#[derive(Debug, Deserialize)]
struct SlippiExport {
    source: Option<SlippiSource>,
    settings: Option<SlippiSettings>,
    frames: Vec<SlippiFrame>,
}

fn world_for_slippi_export(export: &SlippiExport) -> World {
    let mut world = World::for_slippi_battlefield_singles_match_start();
    if let Some(settings) = &export.settings {
        for player_index in 0..PLAYER_COUNT {
            let costume_index = settings
                .players
                .get(&player_index.to_string())
                .and_then(|player| player.character_color)
                .unwrap_or(0);
            world.set_player_costume_index(player_index, costume_index);
        }
    }
    world
}

impl SlippiExport {
    fn ucf_players(&self) -> [bool; PLAYER_COUNT] {
        let mut players = [false; PLAYER_COUNT];
        let Some(settings) = &self.settings else {
            return players;
        };
        for (player_index, is_ucf) in players.iter_mut().enumerate() {
            *is_ucf = settings
                .players
                .get(&player_index.to_string())
                .and_then(|player| player.controller_fix.as_deref())
                .is_some_and(|controller_fix| controller_fix.eq_ignore_ascii_case("ucf"));
        }
        players
    }
}

#[derive(Debug, Deserialize)]
struct SlippiSource {
    replay_path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SlippiSettings {
    #[serde(default)]
    players: HashMap<String, SlippiPlayerSettings>,
}

#[derive(Debug, Deserialize)]
struct SlippiPlayerSettings {
    controller_fix: Option<String>,
    character_color: Option<u8>,
}

#[derive(Debug, Deserialize)]
struct SlippiFrame {
    frame: i32,
    #[serde(default)]
    players: HashMap<String, SlippiPlayerFrame>,
}

#[derive(Debug, Deserialize)]
struct SlippiPlayerFrame {
    pre: Option<SlippiPreFrame>,
    post: Option<SlippiPostFrame>,
}

#[derive(Debug, Deserialize)]
struct SlippiPreFrame {
    action_state_id: Option<u16>,
    position: Option<[f64; 2]>,
    facing: Option<f64>,
    main_stick: Option<[f64; 2]>,
    c_stick: Option<[f64; 2]>,
    trigger: Option<f64>,
    physical_l_trigger: Option<f64>,
    physical_r_trigger: Option<f64>,
    rust_player_input: Option<SlippiRustPlayerInput>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub struct SlippiRustPlayerInput {
    #[serde(default)]
    pub stick_x: i8,
    #[serde(default)]
    pub stick_y: i8,
    #[serde(default)]
    pub c_stick_x: i8,
    #[serde(default)]
    pub c_stick_y: i8,
    #[serde(default)]
    pub left_trigger: u8,
    #[serde(default)]
    pub right_trigger: u8,
    #[serde(default)]
    pub physical_button_bits: u32,
    #[serde(default)]
    pub ucf_dashback_amendment: bool,
    #[serde(default)]
    pub ucf_shield_drop_amendment: bool,
}

impl SlippiRustPlayerInput {
    fn to_player_input(&self) -> PlayerInput {
        let bits = self.physical_button_bits;
        PlayerInput::neutral()
            .with_left_stick(self.stick_x, self.stick_y)
            .with_c_stick(self.c_stick_x, self.c_stick_y)
            .with_left_trigger_analog(self.left_trigger)
            .with_right_trigger_analog(self.right_trigger)
            .with_attack(bits & HSD_A != 0)
            .with_special(bits & HSD_B != 0)
            .with_jump_primary(bits & HSD_X != 0)
            .with_jump_secondary(bits & HSD_Y != 0)
            .with_grab(bits & HSD_Z != 0)
            .with_left_trigger_digital(bits & HSD_L != 0)
            .with_right_trigger_digital(bits & HSD_R != 0)
            .with_start(bits & HSD_START != 0)
            .with_dpad_up(bits & HSD_DPAD_UP != 0)
            .with_dpad_down(bits & HSD_DPAD_DOWN != 0)
            .with_dpad_left(bits & HSD_DPAD_LEFT != 0)
            .with_dpad_right(bits & HSD_DPAD_RIGHT != 0)
            .with_ucf_dashback_amendment(self.ucf_dashback_amendment)
            .with_ucf_shield_drop_amendment(self.ucf_shield_drop_amendment)
    }
}

fn effective_slippi_player_input(pre: &SlippiPreFrame) -> SlippiRustPlayerInput {
    let mut input = pre.rust_player_input.unwrap_or_default();
    if pre.rust_player_input.is_none() {
        if let Some([x, y]) = pre.main_stick {
            input.stick_x = slippi_stick_to_core_i8(x);
            input.stick_y = slippi_stick_to_core_i8(y);
        }
        if let Some([x, y]) = pre.c_stick {
            input.c_stick_x = slippi_stick_to_core_i8(x);
            input.c_stick_y = slippi_stick_to_core_i8(y);
        }
    }
    let input_config = MeleeCommonData::PROVISIONAL.input_config();
    input.stick_x = clean_replay_axis(input.stick_x, input_config.main_stick_deadzone_x);
    input.stick_y = clean_replay_axis(input.stick_y, input_config.main_stick_deadzone_y);
    input.c_stick_x = clean_replay_axis(input.c_stick_x, input_config.c_stick_deadzone_x);
    input.c_stick_y = clean_replay_axis(input.c_stick_y, input_config.c_stick_deadzone_y);
    if pre.physical_l_trigger.is_some() || pre.trigger.is_some() {
        input.left_trigger = slippi_trigger_to_core_u8(pre.physical_l_trigger.or(pre.trigger));
    }
    if pre.physical_r_trigger.is_some() || pre.trigger.is_some() {
        input.right_trigger = slippi_trigger_to_core_u8(pre.physical_r_trigger.or(pre.trigger));
    }
    input
}

fn clean_replay_axis(value: i8, deadzone: i8) -> i8 {
    if (value as i16).abs() <= threshold_abs_i8(deadzone) {
        0
    } else {
        value
    }
}

fn threshold_abs_i8(value: i8) -> i16 {
    if value == i8::MIN {
        128
    } else {
        value.abs() as i16
    }
}

fn slippi_stick_to_core_i8(value: f64) -> i8 {
    if !value.is_finite() {
        return 0;
    }
    (value * 127.0).round().clamp(-127.0, 127.0) as i8
}

fn slippi_trigger_to_core_u8(value: Option<f64>) -> u8 {
    let Some(value) = value else {
        return 0;
    };
    if !value.is_finite() {
        return 0;
    }
    (value * 255.0).round().clamp(0.0, 255.0) as u8
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SlippiPostFrame {
    pub action_state_id: u16,
    pub action_state_counter: Option<f64>,
    pub position: Option<[f64; 2]>,
    pub facing: Option<f64>,
    pub airborne: Option<bool>,
    pub self_induced_speeds: Option<SlippiSelfInducedSpeeds>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SlippiSelfInducedSpeeds {
    pub ground_x: Option<f64>,
    pub air_x: Option<f64>,
    pub y: Option<f64>,
    pub attack_x: Option<f64>,
    pub attack_y: Option<f64>,
}

fn rounded_u8(value: Option<f64>) -> u8 {
    value.unwrap_or_default().round().clamp(0.0, u8::MAX as f64) as u8
}

fn slippi_facing_to_i8(value: Option<f64>) -> i8 {
    if value.unwrap_or(1.0) < 0.0 {
        -1
    } else {
        1
    }
}
