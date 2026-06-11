pub mod app;
pub mod ecb_coverage;
pub mod input_trace;
pub mod move_keyframes;
pub mod parity_ledger;
pub mod slippi_replay;
pub mod state_graphs;
pub mod template;
pub mod theme;
pub mod ui;

pub use app::{AppSection, ParityLedgerApp, ThemeMode};
pub use ecb_coverage::EcbCoverageSurface;
pub use input_trace::InputTraceSurface;
pub use move_keyframes::{
    MoveKeyframe, MoveKeyframeBodyPoint, MoveKeyframeEndpoint, MoveKeyframeHandle,
    MoveKeyframeHandleKind, MoveKeyframeJobjJoint, MoveKeyframeJobjSource, MoveKeyframeJobjTree,
    MoveKeyframeVec3, MoveKeyframesEditorSurface, MoveKeyframesSurface,
};
pub use parity_ledger::{ParityLedgerSurface, ParityLedgerSurfaceRow, ParityLedgerSurfaceTab};
pub use slippi_replay::SlippiReplaySurface;
pub use state_graphs::StateGraphsSurface;
pub use template::{LedgerTabTemplate, LedgerTabTemplateRow};

use mole_ledger::{
    DevtoolSurfaceRecord, LedgerMap, LedgerSurfaceState, LedgerTabKind, LedgerTabRecord,
    LedgerTabStatus,
};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParityLedgerRegistryView {
    pub tab_count: usize,
    pub active_tab_count: usize,
    pub planned_tab_count: usize,
    pub dual_surface: bool,
    pub devtool_surface_count: usize,
    pub active_devtool_surface_count: usize,
    pub planned_devtool_surface_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParityLedgerTabView {
    pub id: String,
    pub label: String,
    pub kind: LedgerTabKind,
    pub status: LedgerTabStatus,
    pub cli_surface: LedgerSurfaceState,
    pub gui_surface: LedgerSurfaceState,
    pub summary: String,
    pub source_artifacts: Vec<String>,
    pub outputs: Vec<String>,
    pub surface_alignment_ok: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevtoolSurfaceView {
    pub id: String,
    pub label: String,
    pub status: LedgerTabStatus,
    pub cli_surface: LedgerSurfaceState,
    pub gui_surface: LedgerSurfaceState,
    pub summary: String,
    pub cli_commands: Vec<String>,
    pub gui_section: String,
    pub source_artifacts: Vec<String>,
    pub outputs: Vec<String>,
    pub surface_alignment_ok: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParityLedgerViewModel {
    pub schema_version: u8,
    pub surface: String,
    pub registry: ParityLedgerRegistryView,
    pub tabs: Vec<ParityLedgerTabView>,
    pub devtool_surfaces: Vec<DevtoolSurfaceView>,
    pub all_tabs_dual_surface: bool,
    pub all_devtool_surfaces_dual_surface: bool,
}

impl ParityLedgerViewModel {
    pub fn from_ledger_map(ledger_map: &LedgerMap) -> Self {
        let registry = ParityLedgerRegistryView {
            tab_count: ledger_map.registry.tab_count,
            active_tab_count: ledger_map.registry.active_tab_count,
            planned_tab_count: ledger_map.registry.planned_tab_count,
            dual_surface: ledger_map.registry.dual_surface,
            devtool_surface_count: ledger_map.registry.devtool_surface_count,
            active_devtool_surface_count: ledger_map.registry.active_devtool_surface_count,
            planned_devtool_surface_count: ledger_map.registry.planned_devtool_surface_count,
        };
        let tabs = ledger_map
            .tabs
            .iter()
            .map(ParityLedgerTabView::from)
            .collect::<Vec<_>>();
        let devtool_surfaces = ledger_map
            .devtool_surfaces
            .iter()
            .map(DevtoolSurfaceView::from)
            .collect::<Vec<_>>();
        let all_tabs_dual_surface = tabs.iter().all(|tab| tab.surface_alignment_ok);
        let all_devtool_surfaces_dual_surface = devtool_surfaces
            .iter()
            .all(|surface| surface.surface_alignment_ok);
        Self {
            schema_version: ledger_map.schema_version,
            surface: "parity_ledger".to_string(),
            registry,
            tabs,
            devtool_surfaces,
            all_tabs_dual_surface,
            all_devtool_surfaces_dual_surface,
        }
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let text = fs::read_to_string(path.as_ref())
            .map_err(|error| format!("failed to read {}: {error}", path.as_ref().display()))?;
        let ledger_map: LedgerMap = serde_json::from_str(&text)
            .map_err(|error| format!("failed to parse ledger map: {error}"))?;
        Ok(Self::from_ledger_map(&ledger_map))
    }
}

impl From<&LedgerTabRecord> for ParityLedgerTabView {
    fn from(tab: &LedgerTabRecord) -> Self {
        Self {
            id: tab.id.clone(),
            label: tab.label.clone(),
            kind: tab.kind,
            status: tab.status,
            cli_surface: tab.cli_surface,
            gui_surface: tab.gui_surface,
            summary: tab.summary.clone(),
            source_artifacts: tab.source_artifacts.clone(),
            outputs: tab.outputs.clone(),
            surface_alignment_ok: tab.cli_surface == tab.gui_surface,
        }
    }
}

impl From<&DevtoolSurfaceRecord> for DevtoolSurfaceView {
    fn from(surface: &DevtoolSurfaceRecord) -> Self {
        Self {
            id: surface.id.clone(),
            label: surface.label.clone(),
            status: surface.status,
            cli_surface: surface.cli_surface,
            gui_surface: surface.gui_surface,
            summary: surface.summary.clone(),
            cli_commands: surface.cli_commands.clone(),
            gui_section: surface.gui_section.clone(),
            source_artifacts: surface.source_artifacts.clone(),
            outputs: surface.outputs.clone(),
            surface_alignment_ok: surface.cli_surface == surface.gui_surface,
        }
    }
}

pub fn load_parity_ledger_view_model(
    path: impl AsRef<Path>,
) -> Result<ParityLedgerViewModel, String> {
    ParityLedgerViewModel::load(path)
}

pub fn parity_ledger_view_model_from_ledger_map(ledger_map: &LedgerMap) -> ParityLedgerViewModel {
    ParityLedgerViewModel::from_ledger_map(ledger_map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mole_ledger::LedgerRegistry;

    #[test]
    fn view_model_from_ledger_map_preserves_dual_surface_tabs() {
        let view = ParityLedgerViewModel::from_ledger_map(&LedgerMap::from_registry(
            &LedgerRegistry::roadmap(),
        ));

        assert_eq!(view.schema_version, 1);
        assert_eq!(view.registry.tab_count, 10);
        assert_eq!(view.tabs.len(), 10);
        assert_eq!(view.devtool_surfaces.len(), 6);
        assert!(view.all_tabs_dual_surface);
        assert!(view.all_devtool_surfaces_dual_surface);
        assert_eq!(view.tabs[0].id, "global_values");
        assert_eq!(view.tabs[4].id, "stage_values");
        assert_eq!(view.devtool_surfaces[3].id, "input_trace");
        assert!(view.tabs.iter().all(|tab| tab.surface_alignment_ok));
        assert!(view
            .devtool_surfaces
            .iter()
            .all(|surface| surface.surface_alignment_ok));
    }
}
