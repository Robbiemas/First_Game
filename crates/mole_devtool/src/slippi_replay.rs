use crate::{LedgerTabTemplate, LedgerTabTemplateRow};
use mole_runtime::{trace_slippi_export_from_match_start_with_core, SlippiCoreTraceConfig};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlippiReplayExportRange {
    pub first_frame: i32,
    pub last_frame: i32,
    pub frame_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SlippiInputSource {
    replay_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SlippiInputFile {
    export: SlippiReplayExportRange,
    source: SlippiInputSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlippiRuntimeReplayLaunchPlan {
    pub input_export_path: PathBuf,
    pub replay_path: Option<PathBuf>,
    pub divergence_log_path: PathBuf,
    pub frames_to_run: u32,
}

impl SlippiRuntimeReplayLaunchPlan {
    pub fn from_input_export(
        path: impl AsRef<Path>,
        divergence_log_path: impl AsRef<Path>,
    ) -> Result<Self, String> {
        let path = path.as_ref();
        let text = fs::read_to_string(path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        let input_file: SlippiInputFile = serde_json::from_str(&text)
            .map_err(|error| format!("failed to parse replay input export: {error}"))?;
        let frames_to_run = (input_file.export.frame_count as u32).max(1);
        Ok(Self {
            input_export_path: path.to_path_buf(),
            replay_path: None,
            divergence_log_path: divergence_log_path.as_ref().to_path_buf(),
            frames_to_run,
        })
    }

    pub fn from_replay_path(path: impl AsRef<Path>, divergence_log_path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        Self {
            input_export_path: path.clone(),
            replay_path: Some(path),
            divergence_log_path: divergence_log_path.as_ref().to_path_buf(),
            frames_to_run: u32::MAX,
        }
    }

    pub fn runtime_args(&self) -> Vec<String> {
        let mut args = vec!["--sdl".to_string()];
        if let Some(replay_path) = &self.replay_path {
            args.extend([
                "--visual-slippi-replay".to_string(),
                replay_path.display().to_string(),
            ]);
        } else {
            args.extend([
                "--visual-slippi-inputs".to_string(),
                self.input_export_path.display().to_string(),
            ]);
        }
        args.extend([
            "--frames".to_string(),
            self.frames_to_run.to_string(),
            "--slippi-divergence-log".to_string(),
            self.divergence_log_path.display().to_string(),
            "--hold-final-frame".to_string(),
        ]);
        args
    }

    pub fn cargo_args(&self) -> Vec<String> {
        let mut args = vec![
            "run".to_string(),
            "--release".to_string(),
            "-p".to_string(),
            "mole_runtime".to_string(),
            "--features".to_string(),
            "sdl wup".to_string(),
            "--".to_string(),
        ];
        args.extend(self.runtime_args());
        args
    }
}

impl SlippiReplaySurface {
    pub fn load(root: impl AsRef<Path>) -> Result<Self, String> {
        let root = root.as_ref();
        let replay_options = SlippiReplayLoadOptions {
            focus_end: DEFAULT_SOURCE_FRAME_END,
            focus_start: DEFAULT_SOURCE_FRAME_START,
            input_export_path: root.join("replays").join("Game_20260530T214929.slp"),
            max_frames: Some(DEFAULT_MAX_FRAMES),
            player_number: DEFAULT_PLAYER_INDEX + 1,
        };

        if replay_options.input_export_path.exists() {
            return Self::load_replay_metadata_only(replay_options);
        }

        let options = SlippiReplayLoadOptions {
            focus_end: DEFAULT_SOURCE_FRAME_END,
            focus_start: DEFAULT_SOURCE_FRAME_START,
            input_export_path: root
                .join("debug")
                .join("slippi")
                .join("Game_20260530T214929.full.inputs.json"),
            max_frames: Some(DEFAULT_MAX_FRAMES),
            player_number: DEFAULT_PLAYER_INDEX + 1,
        };
        if options.input_export_path.exists() {
            Self::load_metadata_only(options)
        } else {
            Self::empty_missing_artifact(options)
        }
    }

    pub fn load_metadata_only(options: SlippiReplayLoadOptions) -> Result<Self, String> {
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

        Ok(Self {
            focus_end: options.focus_end,
            focus_player_number: options.player_number,
            focus_start: options.focus_start,
            input_export_path: options.input_export_path.display().to_string(),
            replay_path: input_file.source.replay_path,
            rows: Vec::new(),
            trace_report_text:
                "Replay metadata loaded. Press Refresh to compute the comparison trace.".to_string(),
        })
    }

    pub fn load_replay_metadata_only(options: SlippiReplayLoadOptions) -> Result<Self, String> {
        validate_load_options(&options)?;

        Ok(Self {
            focus_end: options.focus_end,
            focus_player_number: options.player_number,
            focus_start: options.focus_start,
            input_export_path: options.input_export_path.display().to_string(),
            replay_path: options.input_export_path.display().to_string(),
            rows: Vec::new(),
            trace_report_text:
                "Replay source loaded. Press Refresh to compute the comparison trace.".to_string(),
        })
    }

    fn empty_missing_artifact(options: SlippiReplayLoadOptions) -> Result<Self, String> {
        validate_load_options(&options)?;

        Ok(Self {
            focus_end: options.focus_end,
            focus_player_number: options.player_number,
            focus_start: options.focus_start,
            input_export_path: options.input_export_path.display().to_string(),
            replay_path: "No local Slippi replay source loaded".to_string(),
            rows: Vec::new(),
            trace_report_text:
                "No local Slippi replay source found. Put the replay at replays/Game_20260530T214929.slp. .inputs.json is supported only for explicit diagnostics."
                    .to_string(),
        })
    }

    pub fn load_with_options(options: SlippiReplayLoadOptions) -> Result<Self, String> {
        validate_load_options(&options)?;

        let text = load_slippi_source_text(&options.input_export_path)?;
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

    pub fn export_range_from_path(
        path: impl AsRef<Path>,
    ) -> Result<SlippiReplayExportRange, String> {
        let path = path.as_ref();
        let text = fs::read_to_string(path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        let input_file: SlippiInputFile = serde_json::from_str(&text)
            .map_err(|error| format!("failed to parse replay input export: {error}"))?;
        Ok(input_file.export)
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

fn validate_load_options(options: &SlippiReplayLoadOptions) -> Result<(), String> {
    if options.player_number == 0 {
        return Err("player numbers are 1-based and must be greater than zero".to_string());
    }
    if options.focus_end < options.focus_start {
        return Err(format!(
            "invalid replay trace window: {}..{}",
            options.focus_start, options.focus_end
        ));
    }
    Ok(())
}

fn load_slippi_source_text(path: &Path) -> Result<String, String> {
    if is_slippi_replay_path(path) {
        export_slippi_replay_to_json(path)
    } else {
        fs::read_to_string(path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))
    }
}

fn export_slippi_replay_to_json(replay_path: &Path) -> Result<String, String> {
    let root = workspace_root()?;
    let output = Command::new("node")
        .current_dir(&root)
        .arg(root.join("tools").join("slippi_replay_to_inputs.cjs"))
        .arg("--replay")
        .arg(replay_path)
        .arg("--include-negative-frames")
        .arg("--all-frames")
        .arg("--stdout")
        .output()
        .map_err(|error| format!("failed to run Slippi replay parser with node: {error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() {
        return Err(format!(
            "Slippi replay parser failed with status {}: {}{}",
            output.status,
            stderr.trim(),
            if stdout.trim().is_empty() {
                String::new()
            } else {
                format!("\nstdout: {}", stdout.trim())
            }
        ));
    }
    Ok(stdout)
}

pub(crate) fn is_slippi_replay_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("slp"))
}

fn workspace_root() -> Result<PathBuf, String> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .map(|path| path.to_path_buf())
        .ok_or_else(|| "failed to resolve workspace root from CARGO_MANIFEST_DIR".to_string())
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
    use std::path::PathBuf;

    #[test]
    fn slippi_replay_default_load_is_metadata_only_for_fast_startup() {
        let root = temp_test_dir("slippi_replay_default_load_is_metadata_only_for_fast_startup");
        let surface = SlippiReplaySurface::load(&root).unwrap();
        let template = LedgerTabTemplate::from(&surface);

        assert_eq!(surface.focus_player_number, 2);
        assert_eq!(surface.focus_start, 760);
        assert_eq!(surface.focus_end, 768);
        assert_eq!(template.title, "Slippi Replay");
        assert_eq!(template.headers.len(), 13);
        assert!(template.rows.is_empty());
        assert!(surface
            .trace_report_text
            .contains("No local Slippi replay source found"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn slippi_replay_default_load_uses_local_replay_as_source_when_available() {
        let root =
            temp_test_dir("slippi_replay_default_load_uses_local_replay_as_source_when_available");
        let replay_path = root.join("replays").join("Game_20260530T214929.slp");
        fs::create_dir_all(replay_path.parent().expect("replay parent")).unwrap();
        fs::write(&replay_path, []).unwrap();

        let surface = SlippiReplaySurface::load(&root).unwrap();

        assert_eq!(surface.input_export_path, replay_path.display().to_string());
        assert_eq!(surface.replay_path, replay_path.display().to_string());
        assert!(surface.rows.is_empty());
        assert!(surface
            .trace_report_text
            .contains("Press Refresh to compute the comparison trace"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn slippi_replay_loads_explicit_artifact_player_and_window() {
        let root = temp_test_dir("slippi_replay_loads_explicit_artifact_player_and_window");
        let path = root.join("fixture.inputs.json");
        fs::create_dir_all(&root).unwrap();
        fs::write(&path, slippi_trace_fixture()).unwrap();
        let surface = SlippiReplaySurface::load_with_options(SlippiReplayLoadOptions {
            input_export_path: path.clone(),
            player_number: 1,
            focus_start: -17,
            focus_end: -16,
            max_frames: Some(3),
        })
        .unwrap();

        assert_eq!(surface.focus_player_number, 1);
        assert_eq!(surface.focus_start, -17);
        assert_eq!(surface.focus_end, -16);
        assert_eq!(surface.rows.len(), 2);
        assert!(surface
            .rows
            .iter()
            .all(|row| (-17..=-16).contains(&row.source_frame)));
        let _ = fs::remove_file(path);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn slippi_replay_runtime_launch_plan_runs_full_export_and_logs_live_divergence() {
        let root = temp_test_dir(
            "slippi_replay_runtime_launch_plan_runs_full_export_and_logs_live_divergence",
        );
        let path = root.join("fixture.full.inputs.json");
        fs::create_dir_all(&root).unwrap();
        fs::write(&path, slippi_metadata_fixture()).unwrap();

        let log_path = root.join("debug/slippi/runtime-divergence.latest.json");
        let plan = SlippiRuntimeReplayLaunchPlan::from_input_export(&path, &log_path).unwrap();

        assert_eq!(plan.frames_to_run, 5313);
        assert!(plan.cargo_args().windows(2).any(|pair| {
            pair == [
                "--visual-slippi-inputs".to_string(),
                path.display().to_string(),
            ]
        }));
        assert!(plan
            .cargo_args()
            .windows(2)
            .any(|pair| pair == ["--frames".to_string(), "5313".to_string()]));
        assert!(plan.cargo_args().windows(2).any(|pair| {
            pair == [
                "--slippi-divergence-log".to_string(),
                log_path.display().to_string(),
            ]
        }));
        assert!(plan
            .cargo_args()
            .contains(&"--hold-final-frame".to_string()));
        let _ = fs::remove_file(path);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn slippi_replay_runtime_launch_plan_can_use_replay_source_without_export_artifact() {
        let root = temp_test_dir(
            "slippi_replay_runtime_launch_plan_can_use_replay_source_without_export_artifact",
        );
        let replay_path = root.join("replays").join("fixture.slp");
        let log_path = root.join("debug/slippi/runtime-divergence.latest.json");

        let plan = SlippiRuntimeReplayLaunchPlan::from_replay_path(&replay_path, &log_path);

        assert_eq!(plan.frames_to_run, u32::MAX);
        assert!(plan.cargo_args().windows(2).any(|pair| {
            pair == [
                "--visual-slippi-replay".to_string(),
                replay_path.display().to_string(),
            ]
        }));
        assert!(!plan
            .cargo_args()
            .contains(&"--visual-slippi-inputs".to_string()));
        assert!(plan.cargo_args().windows(2).any(|pair| {
            pair == [
                "--slippi-divergence-log".to_string(),
                log_path.display().to_string(),
            ]
        }));
        let _ = fs::remove_dir_all(root);
    }

    fn temp_test_dir(name: &str) -> PathBuf {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "mole_devtool_{name}_{}_{}",
            std::process::id(),
            suffix
        ))
    }

    fn slippi_metadata_fixture() -> &'static str {
        r#"{
          "export": {"first_frame": -123, "last_frame": 5189, "frame_count": 5313},
          "source": {"replay_path": "fixture.slp"},
          "frames": []
        }"#
    }

    fn slippi_trace_fixture() -> &'static str {
        r#"{
          "export": {"first_frame": -18, "last_frame": -16, "frame_count": 3},
          "source": {"replay_path": "fixture.slp"},
          "settings": {"players": {}},
          "frames": [
            {"frame": -18, "players": {"0": {
              "pre": {
                "rust_player_input": {
                  "stick_x": 0, "stick_y": 0, "c_stick_x": 0, "c_stick_y": 0,
                  "left_trigger": 0, "right_trigger": 0, "physical_button_bits": 0,
                  "ucf_dashback_amendment": false
                }
              },
              "post": {
                "action_state_id": 14,
                "position": [0.0, 0.0],
                "self_induced_speeds": {"ground_x": 0.0, "air_x": 0.0, "y": 0.0}
              }
            }}},
            {"frame": -17, "players": {"0": {
              "pre": {
                "rust_player_input": {
                  "stick_x": -125, "stick_y": 0, "c_stick_x": 0, "c_stick_y": 0,
                  "left_trigger": 0, "right_trigger": 0, "physical_button_bits": 2048,
                  "ucf_dashback_amendment": false
                }
              },
              "post": {
                "action_state_id": 24,
                "position": [32.2, 27.2],
                "self_induced_speeds": {"ground_x": -2.14, "air_x": -2.14, "y": 0.0}
              }
            }}},
            {"frame": -16, "players": {"0": {
              "pre": {
                "rust_player_input": {
                  "stick_x": -125, "stick_y": 0, "c_stick_x": 0, "c_stick_y": 0,
                  "left_trigger": 0, "right_trigger": 0, "physical_button_bits": 2048,
                  "ucf_dashback_amendment": false
                }
              },
              "post": {
                "action_state_id": 24,
                "position": [30.22, 27.2],
                "self_induced_speeds": {"ground_x": -1.98, "air_x": -1.98, "y": 0.0}
              }
            }}}
          ]
        }"#
    }
}
