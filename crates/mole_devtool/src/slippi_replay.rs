use crate::{LedgerTabTemplate, LedgerTabTemplateRow};
use mole_runtime::{trace_slippi_export_from_match_start_with_core, SlippiCoreTraceConfig};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

const DEFAULT_PLAYER_INDEX: usize = 1;
const DEFAULT_SOURCE_FRAME_START: i32 = 760;
const DEFAULT_SOURCE_FRAME_END: i32 = 768;
const DEFAULT_MAX_FRAMES: usize = 9;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlippiReplaySurface {
    pub focus_end: i32,
    pub focus_player_number: usize,
    pub focus_start: i32,
    pub input_export_path: String,
    pub replay_path: String,
    pub rows: Vec<SlippiReplayRow>,
    pub trace_report_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlippiReplayLoadOptions {
    pub focus_end: i32,
    pub focus_start: i32,
    pub input_export_path: PathBuf,
    pub max_frames: Option<usize>,
    pub player_number: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlippiReplayRow {
    pub actual_action_state_id: Option<usize>,
    pub actual_motion_state: String,
    pub actual_position: [f64; 2],
    pub actual_velocity_x: f64,
    pub actual_velocity_y: f64,
    pub core_frame: i32,
    pub detail: String,
    pub expected_action_state_id: usize,
    pub expected_ground_velocity_x: f64,
    pub expected_motion_state: String,
    pub expected_position: [f64; 2],
    pub expected_velocity_y: f64,
    pub ground_velocity_x_delta: f64,
    pub input_button_bits: u32,
    pub input_left_trigger: f64,
    pub input_right_trigger: f64,
    pub input_stick: [i32; 2],
    pub position_delta: [f64; 2],
    pub source_frame: i32,
    pub status: String,
    pub ucf_dashback_amendment: bool,
    pub velocity_y_delta: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SlippiInputSource {
    replay_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SlippiInputFile {
    source: SlippiInputSource,
}

impl SlippiReplaySurface {
    pub fn load(root: impl AsRef<Path>) -> Result<Self, String> {
        Self::load_with_options(SlippiReplayLoadOptions {
            focus_end: DEFAULT_SOURCE_FRAME_END,
            focus_start: DEFAULT_SOURCE_FRAME_START,
            input_export_path: root
                .as_ref()
                .join("debug/slippi/Game_20260530T214929.inputs.json"),
            max_frames: Some(DEFAULT_MAX_FRAMES),
            player_number: DEFAULT_PLAYER_INDEX + 1,
        })
    }

    pub fn load_with_options(options: SlippiReplayLoadOptions) -> Result<Self, String> {
        if options.player_number == 0 {
            return Err("player numbers are 1-based and must be greater than zero".to_string());
        }
        if options.focus_end < options.focus_start {
            return Err(format!(
                "invalid replay trace window: {}..{}",
                options.focus_start, options.focus_end
            ));
        }

        let text = fs::read_to_string(&options.input_export_path).map_err(|error| {
            format!(
                "failed to read {}: {error}",
                options.input_export_path.display()
            )
        })?;
        let input_file: SlippiInputFile = serde_json::from_str(&text)
            .map_err(|error| format!("failed to parse replay input export: {error}"))?;
        let trace = trace_slippi_export_from_match_start_with_core(
            &text,
            SlippiCoreTraceConfig {
                player_index: options.player_number - 1,
                source_frame_start: options.focus_start,
                source_frame_end: options.focus_end,
                max_frames: options.max_frames,
            },
        )
        .map_err(|error| error.to_string())?;

        let rows = trace
            .rows
            .iter()
            .map(SlippiReplayRow::from)
            .collect::<Vec<_>>();
        let trace_report_text = rows
            .iter()
            .map(|row| row.detail.as_str())
            .collect::<Vec<_>>()
            .join("\n\n---\n\n");

        Ok(Self {
            focus_end: options.focus_end,
            focus_player_number: options.player_number,
            focus_start: options.focus_start,
            input_export_path: options.input_export_path.display().to_string(),
            replay_path: input_file.source.replay_path,
            rows,
            trace_report_text,
        })
    }

    pub fn summary(&self) -> String {
        let first_diff = self
            .rows
            .iter()
            .find(|row| row.status == "diff")
            .map(|row| format!(" | First diff source frame: {}", row.source_frame))
            .unwrap_or_default();
        format!(
            "Replay: {} | Player {} | Window: {}..{} | Rows: {}{}",
            self.replay_path,
            self.focus_player_number,
            self.focus_start,
            self.focus_end,
            self.rows.len(),
            first_diff
        )
    }

    pub fn first_diff_index(&self) -> Option<usize> {
        self.rows.iter().position(|row| row.status == "diff")
    }
}

impl From<&mole_runtime::SlippiCoreTraceRow> for SlippiReplayRow {
    fn from(row: &mole_runtime::SlippiCoreTraceRow) -> Self {
        let expected_motion_state = row
            .expected_motion_state
            .map(|state| format!("{state:?}"))
            .unwrap_or_else(|| "-".to_string());
        let actual_motion_state = format!("{:?}", row.actual_motion_state);
        let expected_position = [
            row.expected_position.x as f64,
            row.expected_position.y as f64,
        ];
        let actual_position = [row.actual_position.x as f64, row.actual_position.y as f64];
        let position_delta = [
            actual_position[0] - expected_position[0],
            actual_position[1] - expected_position[1],
        ];
        let expected_ground_velocity_x = row.expected_ground_velocity_x as f64;
        let actual_velocity_x = row.actual_velocity_x as f64;
        let expected_velocity_y = row.expected_velocity_y as f64;
        let actual_velocity_y = row.actual_velocity_y as f64;
        let ground_velocity_x_delta = actual_velocity_x - expected_ground_velocity_x;
        let velocity_y_delta = actual_velocity_y - expected_velocity_y;
        let status = if position_delta == [0.0, 0.0]
            && ground_velocity_x_delta == 0.0
            && velocity_y_delta == 0.0
        {
            "match"
        } else {
            "diff"
        }
        .to_string();
        Self {
            actual_action_state_id: row.actual_action_state_id.map(|state| usize::from(state.get())),
            actual_motion_state: actual_motion_state.clone(),
            actual_position,
            actual_velocity_x,
            actual_velocity_y,
            core_frame: row.core_frame.0 as i32,
            detail: format!(
                "core_frame: {}\nsource_frame: {}\nplayer: {}\ninput_stick: ({}, {})\ninput_button_bits: {}\nleft_trigger: {}\nright_trigger: {}\nucf_dashback_amendment: {}\nexpected_slippi_state_id: {}\nexpected_action_state_id: {}\nactual_action_state_id: {}\nexpected_motion_state: {}\nactual_motion_state: {}\nactual_motion_frame: {}\nexpected_position: ({:.3}, {:.3})\nactual_position: ({:.3}, {:.3})\nposition_delta: ({:.3}, {:.3})\nexpected_ground_velocity_x: {:.6}\nactual_velocity_x: {:.6}\nground_velocity_x_delta: {:.6}\nexpected_velocity_y: {:.6}\nactual_velocity_y: {:.6}\nvelocity_y_delta: {:.6}",
                row.core_frame.0,
                row.source_frame,
                row.player_index + 1,
                row.input_stick_x,
                row.input_stick_y,
                row.input_button_bits,
                row.input_left_trigger,
                row.input_right_trigger,
                row.input_ucf_dashback_amendment,
                row.expected_slippi_state_id,
                row.expected_action_state_id.get(),
                row.actual_action_state_id.map(|state| state.get()).unwrap_or(0),
                expected_motion_state,
                actual_motion_state,
                row.actual_motion_frame,
                expected_position[0],
                expected_position[1],
                actual_position[0],
                actual_position[1],
                position_delta[0],
                position_delta[1],
                expected_ground_velocity_x,
                actual_velocity_x,
                ground_velocity_x_delta,
                expected_velocity_y,
                actual_velocity_y,
                velocity_y_delta
            ),
            expected_action_state_id: usize::from(row.expected_action_state_id.get()),
            expected_ground_velocity_x,
            expected_motion_state,
            expected_position,
            expected_velocity_y,
            ground_velocity_x_delta,
            input_button_bits: row.input_button_bits,
            input_left_trigger: row.input_left_trigger as f64,
            input_right_trigger: row.input_right_trigger as f64,
            input_stick: [i32::from(row.input_stick_x), i32::from(row.input_stick_y)],
            position_delta,
            source_frame: row.source_frame,
            status,
            ucf_dashback_amendment: row.input_ucf_dashback_amendment,
            velocity_y_delta,
        }
    }
}

impl From<&SlippiReplaySurface> for LedgerTabTemplate {
    fn from(surface: &SlippiReplaySurface) -> Self {
        Self {
            title: "Slippi Replay".to_string(),
            summary: surface.summary(),
            headers: vec![
                "Core Frame".to_string(),
                "Source Frame".to_string(),
                "Expected State".to_string(),
                "Actual State".to_string(),
                "Expected Pos".to_string(),
                "Actual Pos".to_string(),
                "ΔPos".to_string(),
                "Exp GVelX".to_string(),
                "Act GVelX".to_string(),
                "ΔGVelX".to_string(),
                "Exp VelY".to_string(),
                "Act VelY".to_string(),
                "ΔVelY".to_string(),
            ],
            rows: surface
                .rows
                .iter()
                .map(LedgerTabTemplateRow::from)
                .collect(),
        }
    }
}

impl From<&SlippiReplayRow> for LedgerTabTemplateRow {
    fn from(row: &SlippiReplayRow) -> Self {
        Self {
            cells: vec![
                row.core_frame.to_string(),
                row.source_frame.to_string(),
                row.expected_motion_state.clone(),
                row.actual_motion_state.clone(),
                format!(
                    "{:.3}, {:.3}",
                    row.expected_position[0], row.expected_position[1]
                ),
                format!(
                    "{:.3}, {:.3}",
                    row.actual_position[0], row.actual_position[1]
                ),
                format!("{:.3}, {:.3}", row.position_delta[0], row.position_delta[1]),
                format!("{:.6}", row.expected_ground_velocity_x),
                format!("{:.6}", row.actual_velocity_x),
                format!("{:.6}", row.ground_velocity_x_delta),
                format!("{:.6}", row.expected_velocity_y),
                format!("{:.6}", row.actual_velocity_y),
                format!("{:.6}", row.velocity_y_delta),
            ],
            detail: row.detail.clone(),
            status: Some(row.status.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn workspace_root() -> PathBuf {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        manifest_dir
            .parent()
            .and_then(Path::parent)
            .expect("workspace root")
            .to_path_buf()
    }

    #[test]
    fn slippi_replay_loads_the_current_parity_window_trace() {
        let surface = SlippiReplaySurface::load(workspace_root()).unwrap();
        let template = LedgerTabTemplate::from(&surface);

        assert_eq!(surface.focus_player_number, 2);
        assert_eq!(surface.focus_start, 760);
        assert_eq!(surface.focus_end, 768);
        assert_eq!(template.title, "Slippi Replay");
        assert_eq!(template.headers.len(), 13);
        assert_eq!(template.rows.len(), 9);
        assert!(surface.trace_report_text.contains("source_frame: 760"));
        assert_eq!(
            surface.first_diff_index(),
            surface.rows.iter().position(|row| row.status == "diff")
        );
    }

    #[test]
    fn slippi_replay_loads_explicit_artifact_player_and_window() {
        let root = workspace_root();
        let path = root.join("debug/slippi/Game_20260530T214929.inputs.json");
        let surface = SlippiReplaySurface::load_with_options(SlippiReplayLoadOptions {
            input_export_path: path,
            player_number: 1,
            focus_start: 760,
            focus_end: 762,
            max_frames: Some(3),
        })
        .unwrap();

        assert_eq!(surface.focus_player_number, 1);
        assert_eq!(surface.focus_start, 760);
        assert_eq!(surface.focus_end, 762);
        assert_eq!(surface.rows.len(), 3);
        assert!(surface
            .rows
            .iter()
            .all(|row| (760..=762).contains(&row.source_frame)));
    }
}
