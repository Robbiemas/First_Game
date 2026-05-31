use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use mole_core::{
    step_world, Frame, MeleeInputTimers, MotionState, PlayerInput, PlayerState, Vec2, World,
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
    pub player_index: usize,
    pub expected_slippi_state_id: u16,
    pub expected_motion_state: MotionState,
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
    pub first_state_mismatch: Option<SlippiCoreMismatch>,
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
                "- Max abs ground velocity diff: P1 {}, P2 {}",
                self.max_abs_ground_velocity_diff[0], self.max_abs_ground_velocity_diff[1]
            ),
            String::new(),
        ]);

        if let Some(mismatch) = self.first_state_mismatch {
            lines.extend([
                "## First State Mismatch".to_string(),
                String::new(),
                format!("- Frame: {}", mismatch.frame.0),
                format!("- Player: {}", mismatch.player_index + 1),
                format!(
                    "- Melee expected: {:?} ({})",
                    mismatch.expected_motion_state, mismatch.expected_slippi_state_id
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
                    "- Ground velocity X: Melee {} vs Rust {}",
                    mismatch.expected_ground_velocity_x, mismatch.actual_velocity_x
                ),
                format!(
                    "- Air velocity X: Melee {} vs Rust {}",
                    mismatch.expected_air_velocity_x, mismatch.actual_velocity_x
                ),
                format!(
                    "- Velocity Y: Melee {} vs Rust {}",
                    mismatch.expected_velocity_y, mismatch.actual_velocity_y
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
        let mut comparable_players = [false; PLAYER_COUNT];
        for player_index in 0..PLAYER_COUNT {
            if let Some(player_frame) = frame.players.get(&player_index.to_string()) {
                if let Some(pre) = &player_frame.pre {
                    if let Some(input) = &pre.rust_player_input {
                        if input.ucf_dashback_amendment {
                            comparison.ucf_dashback_amendment_frames[player_index] += 1;
                        }
                        inputs[player_index] = input.to_player_input();
                    }
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
            let Some(expected_motion_state) = slippi_action_state_to_motion(post.action_state_id)
            else {
                comparison.unsupported_state_count += 1;
                *unsupported_states.entry(post.action_state_id).or_default() += 1;
                continue;
            };

            comparison.player_frames_compared[player_index] += 1;
            let actual = snapshot.players[player_index].motion_state;
            let expected_position = post
                .position
                .map(slippi_position_to_core_milli)
                .unwrap_or_default();
            let actual_position = snapshot.players[player_index].position;
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
            let actual_velocity = snapshot.players[player_index].velocity.x;
            let actual_velocity_y = snapshot.players[player_index].velocity.y;
            let velocity_diff = (expected_ground_velocity - actual_velocity).abs();
            comparison.max_abs_ground_velocity_diff[player_index] =
                comparison.max_abs_ground_velocity_diff[player_index].max(velocity_diff);

            if actual != expected_motion_state {
                comparison.state_mismatch_count += 1;
                comparison
                    .first_state_mismatch
                    .get_or_insert(SlippiCoreMismatch {
                        frame: diagnostic_core_frame(frame.frame),
                        player_index,
                        expected_slippi_state_id: post.action_state_id,
                        expected_motion_state,
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
    let mut world = World::for_slippi_battlefield_singles_match_start();
    export.frames.sort_by_key(|frame| frame.frame);

    for (core_frame_index, frame) in export.frames.into_iter().take(frame_limit).enumerate() {
        let mut inputs = [PlayerInput::neutral(); PLAYER_COUNT];
        for (player_index, input_slot) in inputs.iter_mut().enumerate() {
            if let Some(input) = frame
                .players
                .get(&player_index.to_string())
                .and_then(|player_frame| player_frame.pre.as_ref())
                .and_then(|pre| pre.rust_player_input.as_ref())
            {
                if input.ucf_dashback_amendment {
                    comparison.ucf_dashback_amendment_frames[player_index] += 1;
                }
                *input_slot = input.to_player_input();
            }
        }

        let core_frame = Frame(core_frame_index as u32);
        step_world(&mut world, core_frame, &inputs);
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
            let Some(expected_motion_state) = slippi_action_state_to_motion(post.action_state_id)
            else {
                comparison.unsupported_state_count += 1;
                *unsupported_states.entry(post.action_state_id).or_default() += 1;
                continue;
            };

            comparison.player_frames_compared[player_index] += 1;
            let actual = snapshot.players[player_index].motion_state;
            let expected_position = post
                .position
                .map(slippi_position_to_core_milli)
                .unwrap_or_default();
            let actual_position = snapshot.players[player_index].position;
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
            let actual_velocity = snapshot.players[player_index].velocity.x;
            let actual_velocity_y = snapshot.players[player_index].velocity.y;
            let velocity_diff = (expected_ground_velocity - actual_velocity).abs();
            comparison.max_abs_ground_velocity_diff[player_index] =
                comparison.max_abs_ground_velocity_diff[player_index].max(velocity_diff);

            if actual != expected_motion_state {
                comparison.state_mismatch_count += 1;
                comparison
                    .first_state_mismatch
                    .get_or_insert(SlippiCoreMismatch {
                        frame: core_frame,
                        player_index,
                        expected_slippi_state_id: post.action_state_id,
                        expected_motion_state,
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
        first_state_mismatch: None,
    }
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
    let facing = pre.facing.unwrap_or(1.0);
    let mut player = PlayerState::new(
        slippi_units_to_core_milli(Some(position[0])),
        slippi_units_to_core_milli(Some(position[1])),
        if facing < 0.0 { -1 } else { 1 },
    );
    player.motion_state = motion_state;
    player.grounded = !matches!(
        motion_state,
        MotionState::Entry
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
            | MotionState::EscapeAir
            | MotionState::FallSpecial
            | MotionState::FallSpecialF
            | MotionState::FallSpecialB
    );
    if matches!(
        motion_state,
        MotionState::Entry | MotionState::EntryStart | MotionState::EntryEnd
    ) {
        player.entry_timer = 1;
    }
    if let Some(post) = previous_post {
        if let Some(speeds) = &post.self_induced_speeds {
            let ground_x = slippi_units_to_core_milli(speeds.ground_x);
            let air_x = slippi_units_to_core_milli(speeds.air_x);
            let vertical = slippi_units_to_core_milli(speeds.y);
            let horizontal = if player.grounded { ground_x } else { air_x };
            player.velocity = Vec2 {
                x: horizontal,
                y: vertical,
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
    let Some(input) = pre.rust_player_input.as_ref() else {
        return;
    };
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

fn slippi_units_to_core_milli(value: Option<f64>) -> i32 {
    (value.unwrap_or_default() * 1000.0).round() as i32
}

fn slippi_position_to_core_milli(position: [f64; 2]) -> Vec2 {
    Vec2 {
        x: slippi_units_to_core_milli(Some(position[0])),
        y: slippi_units_to_core_milli(Some(position[1])),
    }
}

fn diagnostic_core_frame(slippi_frame: i32) -> Frame {
    Frame(slippi_frame.max(0) as u32)
}

fn slippi_action_state_to_motion(action_state_id: u16) -> Option<MotionState> {
    match action_state_id {
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
        42 => Some(MotionState::Landing),
        43 => Some(MotionState::LandingFallSpecial),
        70 => Some(MotionState::LandingAirN),
        71 => Some(MotionState::LandingAirF),
        72 => Some(MotionState::LandingAirB),
        73 => Some(MotionState::LandingAirHi),
        74 => Some(MotionState::LandingAirLw),
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
        322 => Some(MotionState::Entry),
        323 => Some(MotionState::EntryStart),
        324 => Some(MotionState::EntryEnd),
        349 => Some(MotionState::SpecialSStart),
        350 => Some(MotionState::SpecialS),
        351 => Some(MotionState::SpecialAirSStart),
        352 => Some(MotionState::SpecialAirS),
        _ => None,
    }
}

fn slippi_action_state_name(action_state_id: u16) -> &'static str {
    match action_state_id {
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
        42 => "Landing",
        43 => "LandingFallSpecial",
        70 => "LandingAirN",
        71 => "LandingAirF",
        72 => "LandingAirB",
        73 => "LandingAirHi",
        74 => "LandingAirLw",
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
        322 => "Entry",
        323 => "EntryStart",
        324 => "EntryEnd",
        349 => "SpecialSStart",
        350 => "SpecialS",
        351 => "SpecialAirSStart",
        352 => "SpecialAirS",
        _ => "Unknown",
    }
}

#[derive(Debug, Deserialize)]
struct SlippiExport {
    source: Option<SlippiSource>,
    settings: Option<SlippiSettings>,
    frames: Vec<SlippiFrame>,
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
    rust_player_input: Option<SlippiRustPlayerInput>,
}

#[derive(Debug, Deserialize)]
struct SlippiRustPlayerInput {
    #[serde(default)]
    stick_x: i8,
    #[serde(default)]
    stick_y: i8,
    #[serde(default)]
    c_stick_x: i8,
    #[serde(default)]
    c_stick_y: i8,
    #[serde(default)]
    left_trigger: u8,
    #[serde(default)]
    right_trigger: u8,
    #[serde(default)]
    physical_button_bits: u32,
    #[serde(default)]
    ucf_dashback_amendment: bool,
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
    }
}

#[derive(Debug, Clone, Deserialize)]
struct SlippiPostFrame {
    action_state_id: u16,
    action_state_counter: Option<f64>,
    position: Option<[f64; 2]>,
    self_induced_speeds: Option<SlippiSelfInducedSpeeds>,
}

#[derive(Debug, Clone, Deserialize)]
struct SlippiSelfInducedSpeeds {
    ground_x: Option<f64>,
    air_x: Option<f64>,
    y: Option<f64>,
}

fn rounded_u8(value: Option<f64>) -> u8 {
    value.unwrap_or_default().round().clamp(0.0, u8::MAX as f64) as u8
}
