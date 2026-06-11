use crate::ecb_coverage::EcbCoverageSurface;
use crate::input_trace::{InputTraceLoadOptions, InputTraceSurface};
use crate::move_keyframes::{
    MoveKeyframeHandleKind, MoveKeyframesEditorSurface, MoveKeyframesSurface,
};
use crate::parity_ledger::ParityLedgerSurface;
use crate::slippi_replay::{SlippiReplayLoadOptions, SlippiReplaySurface};
use crate::state_graphs::StateGraphsSurface;
use crate::ui;
use crate::ParityLedgerViewModel;
use eframe::egui;
use mole_ledger::LedgerMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppSection {
    StateGraphs,
    ParityLedger,
    EcbCoverage,
    InputTrace,
    SlippiReplay,
    MoveKeyframes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
}

#[derive(Debug, Clone)]
pub struct ParityLedgerApp {
    pub(crate) view_model: ParityLedgerViewModel,
    pub(crate) state_graphs: StateGraphsSurface,
    pub(crate) parity_ledger: ParityLedgerSurface,
    pub(crate) ecb_coverage: EcbCoverageSurface,
    pub(crate) input_trace: InputTraceSurface,
    pub(crate) input_trace_path: String,
    pub(crate) input_trace_player_number: usize,
    pub(crate) input_trace_start: i32,
    pub(crate) input_trace_end: i32,
    pub(crate) input_trace_status: Option<String>,
    pub(crate) slippi_replay: SlippiReplaySurface,
    pub(crate) slippi_replay_path: String,
    pub(crate) slippi_replay_player_number: usize,
    pub(crate) slippi_replay_start: i32,
    pub(crate) slippi_replay_end: i32,
    pub(crate) slippi_replay_max_frames: usize,
    pub(crate) slippi_replay_status: Option<String>,
    pub(crate) move_keyframes: MoveKeyframesSurface,
    pub(crate) move_keyframes_editor: MoveKeyframesEditorSurface,
    pub(crate) move_keyframes_active_handle: Option<MoveKeyframeHandleKind>,
    pub(crate) move_keyframes_active_drag_delta: egui::Vec2,
    pub(crate) selected_section: AppSection,
    pub(crate) selected_state_graph_row: usize,
    pub(crate) selected_ledger_tab: usize,
    pub(crate) selected_ledger_row: usize,
    pub(crate) selected_ecb_row: usize,
    pub(crate) selected_input_trace_row: usize,
    pub(crate) selected_slippi_replay_row: usize,
    pub(crate) selected_move_keyframe_row: usize,
    pub(crate) theme: ThemeMode,
    workspace_root: PathBuf,
}

impl ParityLedgerApp {
    pub fn from_view_model(
        view_model: ParityLedgerViewModel,
        state_graphs: StateGraphsSurface,
        parity_ledger: ParityLedgerSurface,
        ecb_coverage: EcbCoverageSurface,
        input_trace: InputTraceSurface,
        slippi_replay: SlippiReplaySurface,
        move_keyframes: MoveKeyframesSurface,
        move_keyframes_editor: MoveKeyframesEditorSurface,
    ) -> Self {
        let workspace_root = workspace_root().unwrap_or_default();
        Self::from_view_model_with_root(
            view_model,
            state_graphs,
            parity_ledger,
            ecb_coverage,
            input_trace,
            slippi_replay,
            move_keyframes,
            move_keyframes_editor,
            workspace_root,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn from_view_model_with_root(
        view_model: ParityLedgerViewModel,
        state_graphs: StateGraphsSurface,
        parity_ledger: ParityLedgerSurface,
        ecb_coverage: EcbCoverageSurface,
        input_trace: InputTraceSurface,
        slippi_replay: SlippiReplaySurface,
        move_keyframes: MoveKeyframesSurface,
        move_keyframes_editor: MoveKeyframesEditorSurface,
        workspace_root: PathBuf,
    ) -> Self {
        let input_trace_path = input_trace.input_export_path.clone();
        let input_trace_player_number = input_trace.focus_player_number;
        let input_trace_start = input_trace.focus_start;
        let input_trace_end = input_trace.focus_end;
        let slippi_replay_path = slippi_replay.input_export_path.clone();
        let slippi_replay_player_number = slippi_replay.focus_player_number;
        let slippi_replay_start = slippi_replay.focus_start;
        let slippi_replay_end = slippi_replay.focus_end;
        let slippi_replay_max_frames = slippi_replay.rows.len().max(1);
        Self {
            view_model,
            state_graphs,
            parity_ledger,
            ecb_coverage,
            input_trace,
            input_trace_path,
            input_trace_player_number,
            input_trace_start,
            input_trace_end,
            input_trace_status: None,
            slippi_replay,
            slippi_replay_path,
            slippi_replay_player_number,
            slippi_replay_start,
            slippi_replay_end,
            slippi_replay_max_frames,
            slippi_replay_status: None,
            move_keyframes,
            move_keyframes_editor,
            move_keyframes_active_handle: None,
            move_keyframes_active_drag_delta: egui::Vec2::ZERO,
            selected_section: AppSection::ParityLedger,
            selected_state_graph_row: 0,
            selected_ledger_tab: 0,
            selected_ledger_row: 0,
            selected_ecb_row: 0,
            selected_input_trace_row: 0,
            selected_slippi_replay_row: 0,
            selected_move_keyframe_row: 0,
            theme: ThemeMode::Light,
            workspace_root,
        }
    }

    pub fn from_ledger_map(
        ledger_map: &LedgerMap,
        state_graphs: StateGraphsSurface,
        parity_ledger: ParityLedgerSurface,
        ecb_coverage: EcbCoverageSurface,
        input_trace: InputTraceSurface,
        slippi_replay: SlippiReplaySurface,
        move_keyframes: MoveKeyframesSurface,
    ) -> Result<Self, String> {
        let root = workspace_root()?;
        let move_keyframes_editor =
            MoveKeyframesEditorSurface::from_surface_with_workspace(move_keyframes.clone(), &root)?;
        Ok(Self::from_view_model_with_root(
            ParityLedgerViewModel::from_ledger_map(ledger_map),
            state_graphs,
            parity_ledger,
            ecb_coverage,
            input_trace,
            slippi_replay,
            move_keyframes,
            move_keyframes_editor,
            root,
        ))
    }

    pub fn load(root: impl AsRef<Path>) -> Result<Self, String> {
        let root = root.as_ref();
        let view_model =
            ParityLedgerViewModel::load(root.join("docs/state_graphs/parity_ledger_map.json"))?;
        let state_graphs = StateGraphsSurface::load(root)?;
        let parity_ledger = ParityLedgerSurface::load(root)?;
        let ecb_coverage = EcbCoverageSurface::load(root)?;
        let input_trace = InputTraceSurface::load(root)?;
        let slippi_replay = SlippiReplaySurface::load(root)?;
        let move_keyframes = MoveKeyframesSurface::load(root)?;
        let move_keyframes_editor =
            MoveKeyframesEditorSurface::from_surface_with_workspace(move_keyframes.clone(), root)?;
        Ok(Self::from_view_model_with_root(
            view_model,
            state_graphs,
            parity_ledger,
            ecb_coverage,
            input_trace,
            slippi_replay,
            move_keyframes,
            move_keyframes_editor,
            root.to_path_buf(),
        ))
    }

    pub fn load_workspace_root() -> Result<Self, String> {
        let root = workspace_root()?;
        Self::load(root)
    }

    pub fn title(&self) -> &'static str {
        "Parity Ledger"
    }

    pub fn section_titles(&self) -> [&'static str; 6] {
        [
            "State Graphs",
            "Parity Ledger",
            "ECB Coverage",
            "Input Trace",
            "Slippi Replay",
            "Move Keyframes",
        ]
    }

    pub fn registered_devtool_surface_labels(&self) -> Vec<&str> {
        self.view_model
            .devtool_surfaces
            .iter()
            .filter(|surface| surface.gui_surface == mole_ledger::LedgerSurfaceState::Active)
            .map(|surface| surface.label.as_str())
            .collect()
    }

    pub fn ledger_tab_count(&self) -> usize {
        self.parity_ledger.tabs.len()
    }

    pub fn state_graph_missing_count(&self) -> usize {
        self.state_graphs.nodes.len() + self.state_graphs.edges.len()
    }

    pub fn ecb_coverage_motion_state_count(&self) -> usize {
        self.ecb_coverage.mapped_motion_states.len()
    }

    pub fn input_trace_row_count(&self) -> usize {
        self.input_trace.frames.len()
    }

    pub fn slippi_replay_row_count(&self) -> usize {
        self.slippi_replay.rows.len()
    }

    pub fn reload_input_trace(&mut self) -> Result<(), String> {
        let surface = InputTraceSurface::load_with_options(InputTraceLoadOptions {
            focus_end: self.input_trace_end,
            focus_start: self.input_trace_start,
            path: self.resolve_workspace_path(&self.input_trace_path),
            player_number: self.input_trace_player_number,
        })?;
        self.input_trace = surface;
        self.selected_input_trace_row = 0;
        self.input_trace_status = Some("Input trace refreshed.".to_string());
        Ok(())
    }

    pub fn reload_slippi_replay(&mut self) -> Result<(), String> {
        let surface = SlippiReplaySurface::load_with_options(SlippiReplayLoadOptions {
            focus_end: self.slippi_replay_end,
            focus_start: self.slippi_replay_start,
            input_export_path: self.resolve_workspace_path(&self.slippi_replay_path),
            max_frames: Some(self.slippi_replay_max_frames.max(1)),
            player_number: self.slippi_replay_player_number,
        })?;
        self.slippi_replay = surface;
        self.selected_slippi_replay_row = 0;
        self.slippi_replay_status = Some("Slippi replay trace refreshed.".to_string());
        Ok(())
    }

    pub fn select_first_slippi_replay_diff(&mut self) -> Option<usize> {
        let index = self.slippi_replay.first_diff_index()?;
        self.selected_slippi_replay_row = index;
        Some(index)
    }

    pub fn move_keyframe_count(&self) -> usize {
        self.move_keyframes.keyframes.len()
    }

    pub fn ledger_tab_titles(&self) -> Vec<&str> {
        self.parity_ledger
            .tabs
            .iter()
            .map(|tab| tab.label.as_str())
            .collect()
    }

    pub fn selected_ledger_tab_id(&self) -> Option<&str> {
        self.parity_ledger
            .tabs
            .get(self.selected_ledger_tab)
            .map(|tab| tab.id.as_str())
    }

    pub fn selected_ledger_tab_label(&self) -> Option<&str> {
        self.parity_ledger
            .tabs
            .get(self.selected_ledger_tab)
            .map(|tab| tab.label.as_str())
    }

    pub fn select_ledger_tab(&mut self, index: usize) {
        if index < self.parity_ledger.tabs.len() {
            self.selected_ledger_tab = index;
            self.selected_ledger_row = 0;
        }
    }

    pub fn select_section(&mut self, section: AppSection) {
        self.selected_section = section;
    }

    pub fn theme(&self) -> ThemeMode {
        self.theme
    }

    pub fn set_theme(&mut self, theme: ThemeMode) {
        self.theme = theme;
    }

    fn resolve_workspace_path(&self, path: &str) -> PathBuf {
        let path = PathBuf::from(path);
        if path.is_absolute() {
            path
        } else {
            self.workspace_root.join(path)
        }
    }
}

impl eframe::App for ParityLedgerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui::render_app(ui, self);
    }
}

fn workspace_root() -> Result<PathBuf, String> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .map(|path| path.to_path_buf())
        .ok_or_else(|| "failed to resolve workspace root from CARGO_MANIFEST_DIR".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mole_ledger::LedgerRegistry;

    #[test]
    fn app_state_exposes_outer_sections_and_nested_ledger_tabs() {
        let ledger_map = LedgerMap::from_registry(&LedgerRegistry::roadmap());
        let state_graphs = StateGraphsSurface::load(workspace_root().unwrap()).unwrap();
        let parity_ledger = ParityLedgerSurface::load(workspace_root().unwrap()).unwrap();
        let ecb_coverage = EcbCoverageSurface::load(workspace_root().unwrap()).unwrap();
        let input_trace = InputTraceSurface::load(workspace_root().unwrap()).unwrap();
        let slippi_replay = SlippiReplaySurface::load(workspace_root().unwrap()).unwrap();
        let move_keyframes = MoveKeyframesSurface::load(workspace_root().unwrap()).unwrap();
        let app = ParityLedgerApp::from_ledger_map(
            &ledger_map,
            state_graphs,
            parity_ledger,
            ecb_coverage,
            input_trace,
            slippi_replay,
            move_keyframes,
        )
        .unwrap();

        assert_eq!(app.title(), "Parity Ledger");
        assert_eq!(app.section_titles()[1], "Parity Ledger");
        assert_eq!(
            app.registered_devtool_surface_labels(),
            app.section_titles().to_vec()
        );
        assert_eq!(
            app.state_graph_missing_count(),
            app.state_graphs.missing_node_count + app.state_graphs.missing_edge_count
        );
        assert_eq!(app.ledger_tab_count(), 5);
        assert_eq!(app.ecb_coverage_motion_state_count(), 72);
        assert_eq!(app.input_trace_row_count(), 9);
        assert_eq!(app.slippi_replay_row_count(), 9);
        assert_eq!(app.move_keyframe_count(), 45);
        assert_eq!(app.selected_ledger_tab_id(), Some("global_values"));
        assert_eq!(app.selected_ledger_tab_label(), Some("Global Values"));
        assert!(app
            .ledger_tab_titles()
            .iter()
            .any(|title| *title == "Battlefield Stage Values"));
        assert_eq!(app.theme(), ThemeMode::Light);
        assert_eq!(
            app.move_keyframes_editor
                .pose_tree()
                .map(|tree| tree.root.as_str()),
            Some("PlyCaptain5K_Share_joint")
        );
    }

    #[test]
    fn app_reload_controls_drive_trace_and_replay_surfaces() {
        let ledger_map = LedgerMap::from_registry(&LedgerRegistry::roadmap());
        let root = workspace_root().unwrap();
        let state_graphs = StateGraphsSurface::load(&root).unwrap();
        let parity_ledger = ParityLedgerSurface::load(&root).unwrap();
        let ecb_coverage = EcbCoverageSurface::load(&root).unwrap();
        let input_trace = InputTraceSurface::load(&root).unwrap();
        let slippi_replay = SlippiReplaySurface::load(&root).unwrap();
        let move_keyframes = MoveKeyframesSurface::load(&root).unwrap();
        let mut app = ParityLedgerApp::from_ledger_map(
            &ledger_map,
            state_graphs,
            parity_ledger,
            ecb_coverage,
            input_trace,
            slippi_replay,
            move_keyframes,
        )
        .unwrap();

        app.input_trace_player_number = 1;
        app.input_trace_start = 760;
        app.input_trace_end = 762;
        app.reload_input_trace().unwrap();
        assert_eq!(app.input_trace.focus_player_number, 1);
        assert_eq!(app.input_trace.frames.len(), 3);

        app.slippi_replay_player_number = 1;
        app.slippi_replay_start = 760;
        app.slippi_replay_end = 762;
        app.slippi_replay_max_frames = 3;
        app.reload_slippi_replay().unwrap();
        assert_eq!(app.slippi_replay.focus_player_number, 1);
        assert_eq!(app.slippi_replay.rows.len(), 3);
    }
}
