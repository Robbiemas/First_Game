use crate::{LedgerTabTemplate, LedgerTabTemplateRow};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

const DEFAULT_PLAYER_NUMBER: usize = 2;
const DEFAULT_WINDOW_START: i32 = 760;
const DEFAULT_WINDOW_END: i32 = 768;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputTraceSurface {
    pub export: InputTraceExport,
    pub focus_end: i32,
    pub focus_player_number: usize,
    pub focus_start: i32,
    pub frames: Vec<InputTraceRow>,
    pub input_export_path: String,
    pub metadata: InputTraceMetadata,
    pub raw_export_text: String,
    pub replay_path: String,
    pub settings: InputTraceSettings,
    pub source_parser: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputTraceLoadOptions {
    pub focus_end: i32,
    pub focus_start: i32,
    pub path: PathBuf,
    pub player_number: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputTraceExport {
    pub frame_count: usize,
    pub first_frame: i32,
    pub included_negative_frames: bool,
    pub last_frame: i32,
    pub requested_frame_limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputTraceMetadata {
    pub last_frame: i32,
    pub played_on: String,
    pub start_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputTraceSettings {
    pub is_pal: bool,
    pub is_teams: bool,
    pub slp_version: String,
    pub stage_id: usize,
    pub starting_timer_seconds: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputTraceRow {
    pub action_state_id: usize,
    pub button_bits: u64,
    pub c_stick: [f64; 2],
    pub detail: String,
    pub facing: i32,
    pub frame: i32,
    pub main_stick: [f64; 2],
    pub position: [f64; 2],
    pub raw_joystick: [i32; 2],
    pub rust_stick: [i32; 2],
    pub status: String,
    pub trigger: f64,
    pub ucf_dashback_amendment: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct InputTraceFile {
    export: InputTraceFileExport,
    frames: Vec<InputTraceFileFrame>,
    metadata: InputTraceMetadata,
    settings: InputTraceSettings,
    source: InputTraceSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct InputTraceFileExport {
    first_frame: i32,
    included_negative_frames: bool,
    last_frame: i32,
    frame_count: usize,
    requested_frame_limit: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct InputTraceFileFrame {
    frame: i32,
    players: BTreeMap<String, InputTraceFilePlayer>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct InputTraceFilePlayer {
    pre: InputTracePre,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct InputTracePre {
    action_state_id: usize,
    position: [f64; 2],
    facing: i32,
    main_stick: [f64; 2],
    c_stick: [f64; 2],
    trigger: f64,
    physical_l_trigger: f64,
    physical_r_trigger: f64,
    raw_joystick_x: i32,
    raw_joystick_y: i32,
    raw_c_stick_x: i32,
    raw_c_stick_y: i32,
    rust_player_input: InputTraceRustPlayerInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct InputTraceRustPlayerInput {
    stick_x: i32,
    stick_y: i32,
    c_stick_x: i32,
    c_stick_y: i32,
    left_trigger: i32,
    right_trigger: i32,
    physical_button_bits: u64,
    processed_button_bits: u64,
    ucf_dashback_amendment: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct InputTraceSource {
    parser: String,
    replay_path: String,
    parser_note: String,
}

impl InputTraceSurface {
    pub fn placeholder(root: impl AsRef<Path>) -> Result<Self, String> {
        let options = InputTraceLoadOptions {
            focus_end: DEFAULT_WINDOW_END,
            focus_start: DEFAULT_WINDOW_START,
            path: root
                .as_ref()
                .join("debug/slippi/Game_20260530T214929.inputs.json"),
            player_number: DEFAULT_PLAYER_NUMBER,
        };
        let mut surface = Self::empty_missing_artifact(options)?;
        surface.replay_path =
            "Diagnostic input trace not loaded. Press Refresh to read the selected export."
                .to_string();
        surface.source_parser = "lazy".to_string();
        Ok(surface)
    }

    pub fn is_placeholder(&self) -> bool {
        self.source_parser == "lazy"
    }

    pub fn load(root: impl AsRef<Path>) -> Result<Self, String> {
        let options = InputTraceLoadOptions {
            focus_end: DEFAULT_WINDOW_END,
            focus_start: DEFAULT_WINDOW_START,
            path: root
                .as_ref()
                .join("debug/slippi/Game_20260530T214929.inputs.json"),
            player_number: DEFAULT_PLAYER_NUMBER,
        };
        if options.path.exists() {
            Self::load_with_options(options)
        } else {
            Self::empty_missing_artifact(options)
        }
    }

    pub fn load_with_options(options: InputTraceLoadOptions) -> Result<Self, String> {
        if options.player_number == 0 {
            return Err("player numbers are 1-based and must be greater than zero".to_string());
        }
        if options.focus_end < options.focus_start {
            return Err(format!(
                "invalid trace window: {}..{}",
                options.focus_start, options.focus_end
            ));
        }

        let text = fs::read_to_string(&options.path)
            .map_err(|error| format!("failed to read {}: {error}", options.path.display()))?;
        let file: InputTraceFile = serde_json::from_str(&text)
            .map_err(|error| format!("failed to parse input trace: {error}"))?;
        let player_key = (options.player_number - 1).to_string();

        let frames = file
            .frames
            .into_iter()
            .filter_map(|frame| {
                if frame.frame < options.focus_start || frame.frame > options.focus_end {
                    return None;
                }
                let player = frame.players.get(&player_key)?;
                Some(InputTraceRow::from((frame.frame, player)))
            })
            .collect::<Vec<_>>();

        Ok(Self {
            export: InputTraceExport {
                frame_count: file.export.frame_count,
                first_frame: file.export.first_frame,
                included_negative_frames: file.export.included_negative_frames,
                last_frame: file.export.last_frame,
                requested_frame_limit: file.export.requested_frame_limit,
            },
            focus_end: options.focus_end,
            focus_player_number: options.player_number,
            focus_start: options.focus_start,
            frames,
            input_export_path: options.path.display().to_string(),
            metadata: file.metadata,
            raw_export_text: text,
            replay_path: file.source.replay_path,
            settings: file.settings,
            source_parser: file.source.parser,
        })
    }

    fn empty_missing_artifact(options: InputTraceLoadOptions) -> Result<Self, String> {
        if options.player_number == 0 {
            return Err("player numbers are 1-based and must be greater than zero".to_string());
        }
        if options.focus_end < options.focus_start {
            return Err(format!(
                "invalid trace window: {}..{}",
                options.focus_start, options.focus_end
            ));
        }

        Ok(Self {
            export: InputTraceExport {
                frame_count: 0,
                first_frame: 0,
                included_negative_frames: false,
                last_frame: 0,
                requested_frame_limit: 0,
            },
            focus_end: options.focus_end,
            focus_player_number: options.player_number,
            focus_start: options.focus_start,
            frames: Vec::new(),
            input_export_path: options.path.display().to_string(),
            metadata: InputTraceMetadata {
                last_frame: 0,
                played_on: String::new(),
                start_at: String::new(),
            },
            raw_export_text: String::new(),
            replay_path: "No local debug input export loaded".to_string(),
            settings: InputTraceSettings {
                is_pal: false,
                is_teams: false,
                slp_version: String::new(),
                stage_id: 0,
                starting_timer_seconds: 0,
            },
            source_parser: String::new(),
        })
    }

    pub fn summary(&self) -> String {
        format!(
            "Replay: {} | Player {} | Window: {}..{} | Export frames: {} | Stage id: {}",
            self.replay_path,
            self.focus_player_number,
            self.focus_start,
            self.focus_end,
            self.export.frame_count,
            self.settings.stage_id
        )
    }
}

impl From<(i32, &InputTraceFilePlayer)> for InputTraceRow {
    fn from((frame, player): (i32, &InputTraceFilePlayer)) -> Self {
        let pre = &player.pre;
        let rust = &pre.rust_player_input;
        let status = if rust.ucf_dashback_amendment {
            "derived"
        } else {
            "match"
        }
        .to_string();
        Self {
            action_state_id: pre.action_state_id,
            button_bits: rust.processed_button_bits,
            c_stick: pre.c_stick,
            detail: format!(
                "frame: {}\naction_state_id: {}\nposition: ({:.3}, {:.3})\nfacing: {}\nmain_stick: ({}, {})\nc_stick: ({}, {})\ntrigger: {}\nraw_joystick: ({}, {})\nraw_c_stick: ({}, {})\nrust_stick: ({}, {})\nleft_trigger: {}\nright_trigger: {}\nphysical_button_bits: {}\nprocessed_button_bits: {}\nucf_dashback_amendment: {}",
                frame,
                pre.action_state_id,
                pre.position[0],
                pre.position[1],
                pre.facing,
                pre.main_stick[0],
                pre.main_stick[1],
                pre.c_stick[0],
                pre.c_stick[1],
                pre.trigger,
                pre.raw_joystick_x,
                pre.raw_joystick_y,
                pre.raw_c_stick_x,
                pre.raw_c_stick_y,
                rust.stick_x,
                rust.stick_y,
                rust.left_trigger,
                rust.right_trigger,
                rust.physical_button_bits,
                rust.processed_button_bits,
                rust.ucf_dashback_amendment
            ),
            facing: pre.facing,
            frame,
            main_stick: pre.main_stick,
            position: pre.position,
            raw_joystick: [pre.raw_joystick_x, pre.raw_joystick_y],
            rust_stick: [rust.stick_x, rust.stick_y],
            status,
            trigger: pre.trigger,
            ucf_dashback_amendment: rust.ucf_dashback_amendment,
        }
    }
}

impl From<&InputTraceSurface> for LedgerTabTemplate {
    fn from(surface: &InputTraceSurface) -> Self {
        Self {
            title: "Input Trace".to_string(),
            summary: surface.summary(),
            headers: vec![
                "Frame".to_string(),
                "Action State".to_string(),
                "Facing".to_string(),
                "Position".to_string(),
                "Main Stick".to_string(),
                "C Stick".to_string(),
                "Trigger".to_string(),
                "Raw Joystick".to_string(),
                "Rust Stick".to_string(),
                "Buttons".to_string(),
            ],
            rows: surface
                .frames
                .iter()
                .map(LedgerTabTemplateRow::from)
                .collect(),
        }
    }
}

impl From<&InputTraceRow> for LedgerTabTemplateRow {
    fn from(row: &InputTraceRow) -> Self {
        Self {
            cells: vec![
                row.frame.to_string(),
                row.action_state_id.to_string(),
                row.facing.to_string(),
                format!("{:.3}, {:.3}", row.position[0], row.position[1]),
                format!("{:.4}, {:.4}", row.main_stick[0], row.main_stick[1]),
                format!("{:.4}, {:.4}", row.c_stick[0], row.c_stick[1]),
                row.trigger.to_string(),
                format!("{}, {}", row.raw_joystick[0], row.raw_joystick[1]),
                format!("{}, {}", row.rust_stick[0], row.rust_stick[1]),
                row.button_bits.to_string(),
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
    fn input_trace_loads_the_current_debug_export_window() {
        let surface = InputTraceSurface::load(workspace_root()).unwrap();
        let template = LedgerTabTemplate::from(&surface);

        assert_eq!(surface.focus_player_number, 2);
        assert_eq!(surface.focus_start, 760);
        assert_eq!(surface.focus_end, 768);
        assert_eq!(template.title, "Input Trace");
        assert_eq!(template.headers.len(), 10);
        assert_eq!(template.rows.len(), surface.frames.len());
    }

    #[test]
    fn input_trace_loads_explicit_artifact_player_and_window() {
        let root = temp_test_dir("input_trace_loads_explicit_artifact_player_and_window");
        let path = root.join("fixture.inputs.json");
        fs::create_dir_all(&root).unwrap();
        fs::write(&path, input_trace_fixture()).unwrap();
        let surface = InputTraceSurface::load_with_options(InputTraceLoadOptions {
            path: path.clone(),
            player_number: 1,
            focus_start: 760,
            focus_end: 762,
        })
        .unwrap();

        assert_eq!(surface.focus_player_number, 1);
        assert_eq!(surface.focus_start, 760);
        assert_eq!(surface.focus_end, 762);
        assert_eq!(surface.frames.len(), 3);
        assert!(surface
            .frames
            .iter()
            .all(|row| (760..=762).contains(&row.frame)));
        let _ = fs::remove_file(path);
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

    fn input_trace_fixture() -> &'static str {
        r#"{
          "export": {
            "first_frame": 760,
            "last_frame": 762,
            "frame_count": 3,
            "included_negative_frames": false,
            "requested_frame_limit": 3
          },
          "metadata": {
            "last_frame": 762,
            "played_on": "2026-06-21",
            "start_at": "fixture"
          },
          "settings": {
            "is_pal": false,
            "is_teams": false,
            "slp_version": "3.16.0",
            "stage_id": 31,
            "starting_timer_seconds": 480
          },
          "source": {
            "parser": "fixture",
            "replay_path": "fixture.slp",
            "parser_note": "unit test"
          },
          "frames": [
            {"frame": 760, "players": {"0": {"pre": {"action_state_id": 14, "position": [0.0, 0.0], "facing": 1, "main_stick": [0.0, 0.0], "c_stick": [0.0, 0.0], "trigger": 0.0, "physical_l_trigger": 0.0, "physical_r_trigger": 0.0, "raw_joystick_x": 0, "raw_joystick_y": 0, "raw_c_stick_x": 0, "raw_c_stick_y": 0, "rust_player_input": {"stick_x": 0, "stick_y": 0, "c_stick_x": 0, "c_stick_y": 0, "left_trigger": 0, "right_trigger": 0, "physical_button_bits": 0, "processed_button_bits": 0, "ucf_dashback_amendment": false}}}}},
            {"frame": 761, "players": {"0": {"pre": {"action_state_id": 14, "position": [1.0, 0.0], "facing": 1, "main_stick": [0.0, 0.0], "c_stick": [0.0, 0.0], "trigger": 0.0, "physical_l_trigger": 0.0, "physical_r_trigger": 0.0, "raw_joystick_x": 0, "raw_joystick_y": 0, "raw_c_stick_x": 0, "raw_c_stick_y": 0, "rust_player_input": {"stick_x": 0, "stick_y": 0, "c_stick_x": 0, "c_stick_y": 0, "left_trigger": 0, "right_trigger": 0, "physical_button_bits": 0, "processed_button_bits": 0, "ucf_dashback_amendment": false}}}}},
            {"frame": 762, "players": {"0": {"pre": {"action_state_id": 14, "position": [2.0, 0.0], "facing": 1, "main_stick": [0.0, 0.0], "c_stick": [0.0, 0.0], "trigger": 0.0, "physical_l_trigger": 0.0, "physical_r_trigger": 0.0, "raw_joystick_x": 0, "raw_joystick_y": 0, "raw_c_stick_x": 0, "raw_c_stick_y": 0, "rust_player_input": {"stick_x": 0, "stick_y": 0, "c_stick_x": 0, "c_stick_y": 0, "left_trigger": 0, "right_trigger": 0, "physical_button_bits": 0, "processed_button_bits": 0, "ucf_dashback_amendment": false}}}}}
          ]
        }"#
    }
}
