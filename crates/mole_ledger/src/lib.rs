use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LedgerTabKind {
    Global,
    Character,
    Physics,
    Combat,
    Stage,
    Motion,
    Collision,
    Entity,
    Interaction,
    SpecialState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LedgerTabStatus {
    Active,
    Planned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LedgerSurfaceState {
    Active,
    Planned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct LedgerAccess {
    pub cli: LedgerSurfaceState,
    pub gui: LedgerSurfaceState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LedgerTabSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: LedgerTabKind,
    pub status: LedgerTabStatus,
    pub access: LedgerAccess,
    pub summary: &'static str,
    pub source_artifacts: &'static [&'static str],
    pub outputs: &'static [&'static str],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DevtoolSurfaceSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub status: LedgerTabStatus,
    pub access: LedgerAccess,
    pub summary: &'static str,
    pub cli_commands: &'static [&'static str],
    pub gui_section: &'static str,
    pub source_artifacts: &'static [&'static str],
    pub outputs: &'static [&'static str],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LedgerRegistry {
    pub tabs: Vec<LedgerTabSpec>,
    pub devtool_surfaces: Vec<DevtoolSurfaceSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerTabRecord {
    pub id: String,
    pub label: String,
    pub kind: LedgerTabKind,
    pub status: LedgerTabStatus,
    pub cli_surface: LedgerSurfaceState,
    pub gui_surface: LedgerSurfaceState,
    pub summary: String,
    pub source_artifacts: Vec<String>,
    pub outputs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerRegistryRecord {
    pub tab_count: usize,
    pub active_tab_count: usize,
    pub planned_tab_count: usize,
    pub dual_surface: bool,
    pub devtool_surface_count: usize,
    pub active_devtool_surface_count: usize,
    pub planned_devtool_surface_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerMap {
    pub schema_version: u8,
    pub registry: LedgerRegistryRecord,
    pub tabs: Vec<LedgerTabRecord>,
    #[serde(default)]
    pub devtool_surfaces: Vec<DevtoolSurfaceRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevtoolSurfaceRecord {
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
}

impl LedgerRegistry {
    pub fn current() -> Self {
        Self {
            tabs: roadmap_tabs()
                .into_iter()
                .filter(|tab| tab.status == LedgerTabStatus::Active)
                .collect(),
            devtool_surfaces: devtool_surfaces()
                .into_iter()
                .filter(|surface| surface.status == LedgerTabStatus::Active)
                .collect(),
        }
    }

    pub fn roadmap() -> Self {
        Self {
            tabs: roadmap_tabs(),
            devtool_surfaces: devtool_surfaces(),
        }
    }

    pub fn is_dual_surface(&self) -> bool {
        self.tabs.iter().all(|tab| tab.access.cli == tab.access.gui)
            && self
                .devtool_surfaces
                .iter()
                .all(|surface| surface.access.cli == surface.access.gui)
    }
}

impl LedgerMap {
    pub fn from_registry(registry: &LedgerRegistry) -> Self {
        let tabs = registry
            .tabs
            .iter()
            .map(LedgerTabRecord::from)
            .collect::<Vec<_>>();
        let active_tab_count = registry
            .tabs
            .iter()
            .filter(|tab| tab.status == LedgerTabStatus::Active)
            .count();
        let planned_tab_count = registry
            .tabs
            .iter()
            .filter(|tab| tab.status == LedgerTabStatus::Planned)
            .count();
        let devtool_surfaces = registry
            .devtool_surfaces
            .iter()
            .map(DevtoolSurfaceRecord::from)
            .collect::<Vec<_>>();
        let active_devtool_surface_count = registry
            .devtool_surfaces
            .iter()
            .filter(|surface| surface.status == LedgerTabStatus::Active)
            .count();
        let planned_devtool_surface_count = registry
            .devtool_surfaces
            .iter()
            .filter(|surface| surface.status == LedgerTabStatus::Planned)
            .count();
        Self {
            schema_version: 1,
            registry: LedgerRegistryRecord {
                tab_count: registry.tabs.len(),
                active_tab_count,
                planned_tab_count,
                dual_surface: registry.is_dual_surface(),
                devtool_surface_count: registry.devtool_surfaces.len(),
                active_devtool_surface_count,
                planned_devtool_surface_count,
            },
            tabs,
            devtool_surfaces,
        }
    }

    pub fn from_json_str(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let text = fs::read_to_string(path.as_ref())
            .map_err(|error| format!("failed to read {}: {error}", path.as_ref().display()))?;
        Self::from_json_str(&text).map_err(|error| error.to_string())
    }

    pub fn is_dual_surface(&self) -> bool {
        self.registry.dual_surface
            && self
                .tabs
                .iter()
                .all(|tab| tab.cli_surface == tab.gui_surface)
            && self
                .devtool_surfaces
                .iter()
                .all(|surface| surface.cli_surface == surface.gui_surface)
    }
}

impl From<&LedgerTabSpec> for LedgerTabRecord {
    fn from(tab: &LedgerTabSpec) -> Self {
        Self {
            id: tab.id.to_string(),
            label: tab.label.to_string(),
            kind: tab.kind,
            status: tab.status,
            cli_surface: tab.access.cli,
            gui_surface: tab.access.gui,
            summary: tab.summary.to_string(),
            source_artifacts: tab
                .source_artifacts
                .iter()
                .map(|value| value.to_string())
                .collect(),
            outputs: tab.outputs.iter().map(|value| value.to_string()).collect(),
        }
    }
}

impl From<&DevtoolSurfaceSpec> for DevtoolSurfaceRecord {
    fn from(surface: &DevtoolSurfaceSpec) -> Self {
        Self {
            id: surface.id.to_string(),
            label: surface.label.to_string(),
            status: surface.status,
            cli_surface: surface.access.cli,
            gui_surface: surface.access.gui,
            summary: surface.summary.to_string(),
            cli_commands: surface
                .cli_commands
                .iter()
                .map(|value| value.to_string())
                .collect(),
            gui_section: surface.gui_section.to_string(),
            source_artifacts: surface
                .source_artifacts
                .iter()
                .map(|value| value.to_string())
                .collect(),
            outputs: surface
                .outputs
                .iter()
                .map(|value| value.to_string())
                .collect(),
        }
    }
}

pub fn registry_json() -> serde_json::Value {
    serde_json::to_value(LedgerRegistry::roadmap()).expect("ledger registry is serializable")
}

fn devtool_surfaces() -> Vec<DevtoolSurfaceSpec> {
    vec![
        DevtoolSurfaceSpec {
            id: "state_graphs",
            label: "State Graphs",
            status: LedgerTabStatus::Active,
            access: LedgerAccess {
                cli: LedgerSurfaceState::Active,
                gui: LedgerSurfaceState::Active,
            },
            summary: "State graph parity inspection, missing-entry triage, and layout work.",
            cli_commands: &[
                "graph missing",
                "graph next",
                "graph completeness",
                "graph inspect",
                "graph layout",
                "graph layout save",
                "generated write-action-motion-tables",
            ],
            gui_section: "StateGraphs",
            source_artifacts: &[
                "docs/state_graphs/melee_reference_graph.json",
                "docs/state_graphs/mole_current_graph.json",
                "docs/state_graphs/action_motion_tables.json",
                "config/state_graph_layout.json",
            ],
            outputs: &[
                "docs/state_graphs/mole_current_graph.json",
                "docs/state_graphs/action_motion_tables.json",
            ],
        },
        DevtoolSurfaceSpec {
            id: "parity_ledger",
            label: "Parity Ledger",
            status: LedgerTabStatus::Active,
            access: LedgerAccess {
                cli: LedgerSurfaceState::Active,
                gui: LedgerSurfaceState::Active,
            },
            summary: "Subsystem value sheets and the shared dual-surface registry.",
            cli_commands: &[
                "parity",
                "parity snapshot",
                "generated write-value-sheets",
                "generated write-ledger-map",
            ],
            gui_section: "ParityLedger",
            source_artifacts: &[
                "resources/melee/extracted/plco_common_data.json",
                "resources/melee/extracted/captain_falcon_profile.json",
                "resources/melee/extracted/stages/battlefield_stage.json",
            ],
            outputs: &[
                "docs/state_graphs/parity_ledger_map.json",
                "docs/state_graphs/value_sheets/global_common_values.json",
                "docs/state_graphs/value_sheets/captain_falcon_values.json",
                "docs/state_graphs/value_sheets/physics_engine_values.json",
                "docs/state_graphs/value_sheets/global_combat_values.json",
                "docs/state_graphs/value_sheets/captain_falcon_combat_values.json",
                "docs/state_graphs/value_sheets/battlefield_stage_values.json",
            ],
        },
        DevtoolSurfaceSpec {
            id: "ecb_coverage",
            label: "ECB Coverage",
            status: LedgerTabStatus::Active,
            access: LedgerAccess {
                cli: LedgerSurfaceState::Active,
                gui: LedgerSurfaceState::Active,
            },
            summary: "Motion-state ECB coverage and missing sampled mapping inspection.",
            cli_commands: &["parity snapshot", "generated check"],
            gui_section: "EcbCoverage",
            source_artifacts: &["docs/state_graphs/parity_reports/falcon_ecb_coverage.json"],
            outputs: &["docs/state_graphs/parity_reports/falcon_ecb_coverage.json"],
        },
        DevtoolSurfaceSpec {
            id: "input_trace",
            label: "Input Trace",
            status: LedgerTabStatus::Active,
            access: LedgerAccess {
                cli: LedgerSurfaceState::Active,
                gui: LedgerSurfaceState::Active,
            },
            summary: "Diagnostic Slippi input export inspection with raw and Rust-normalized inputs.",
            cli_commands: &["replay artifacts", "replay trace"],
            gui_section: "InputTrace",
            source_artifacts: &["debug/slippi/*.inputs.json (diagnostic export)"],
            outputs: &[],
        },
        DevtoolSurfaceSpec {
            id: "slippi_replay",
            label: "Slippi Replay",
            status: LedgerTabStatus::Active,
            access: LedgerAccess {
                cli: LedgerSurfaceState::Active,
                gui: LedgerSurfaceState::Active,
            },
            summary: "Direct .slp replay comparison against Rust runtime state, with exported JSON retained for explicit diagnostics.",
            cli_commands: &["replay artifacts", "replay trace", "replay check"],
            gui_section: "SlippiReplay",
            source_artifacts: &["replays/*.slp", "debug/slippi/*.inputs.json (diagnostic export)"],
            outputs: &["debug/slippi/*.core.report.md"],
        },
        DevtoolSurfaceSpec {
            id: "move_keyframes",
            label: "Move Keyframes",
            status: LedgerTabStatus::Active,
            access: LedgerAccess {
                cli: LedgerSurfaceState::Active,
                gui: LedgerSurfaceState::Active,
            },
            summary: "Frame-data character/state browsing, compact manifest inspection, runtime preview, and export readiness.",
            cli_commands: &["frame-data show", "frame-data sample", "frame-data export-runtime"],
            gui_section: "MoveKeyframes",
            source_artifacts: &[
                "resources/melee/frame_data/dolphin_mole/*.json",
                "resources/melee/extracted/captain_falcon_costume_skeleton.json",
            ],
            outputs: &[
                "resources/melee/frame_data/dolphin_mole/*.json",
                "crates/mole_runtime/src/generated/source_frame_data.rs",
            ],
        },
    ]
}

fn roadmap_tabs() -> Vec<LedgerTabSpec> {
    vec![
        LedgerTabSpec {
            id: "global_values",
            label: "Global Values",
            kind: LedgerTabKind::Global,
            status: LedgerTabStatus::Active,
            access: LedgerAccess {
                cli: LedgerSurfaceState::Active,
                gui: LedgerSurfaceState::Active,
            },
            summary: "Shared extracted values from the common data source.",
            source_artifacts: &["resources/melee/extracted/plco_common_data.json"],
            outputs: &["docs/state_graphs/value_sheets/global_common_values.json"],
        },
        LedgerTabSpec {
            id: "character_values",
            label: "Character Values",
            kind: LedgerTabKind::Character,
            status: LedgerTabStatus::Active,
            access: LedgerAccess {
                cli: LedgerSurfaceState::Active,
                gui: LedgerSurfaceState::Active,
            },
            summary: "Per-character profile values for the current playable template.",
            source_artifacts: &["resources/melee/extracted/captain_falcon_profile.json"],
            outputs: &["docs/state_graphs/value_sheets/captain_falcon_values.json"],
        },
        LedgerTabSpec {
            id: "physics_engine_values",
            label: "Physics Engine Values",
            kind: LedgerTabKind::Physics,
            status: LedgerTabStatus::Active,
            access: LedgerAccess {
                cli: LedgerSurfaceState::Active,
                gui: LedgerSurfaceState::Active,
            },
            summary: "Locomotion, gravity, jump, air, and movement state math.",
            source_artifacts: &[
                "resources/melee/extracted/plco_common_data.json",
                "resources/melee/extracted/captain_falcon_profile.json",
            ],
            outputs: &["docs/state_graphs/value_sheets/physics_engine_values.json"],
        },
        LedgerTabSpec {
            id: "global_combat_values",
            label: "Global Combat Values",
            kind: LedgerTabKind::Combat,
            status: LedgerTabStatus::Active,
            access: LedgerAccess {
                cli: LedgerSurfaceState::Active,
                gui: LedgerSurfaceState::Active,
            },
            summary: "Global knockback, hitlag, landing, and other combat response math.",
            source_artifacts: &["resources/melee/extracted/plco_common_data.json"],
            outputs: &["docs/state_graphs/value_sheets/global_combat_values.json"],
        },
        LedgerTabSpec {
            id: "captain_falcon_combat_values",
            label: "Captain Falcon Combat Values",
            kind: LedgerTabKind::Combat,
            status: LedgerTabStatus::Active,
            access: LedgerAccess {
                cli: LedgerSurfaceState::Active,
                gui: LedgerSurfaceState::Active,
            },
            summary: "Captain Falcon combat-specific character attributes currently used by global combat math.",
            source_artifacts: &["resources/melee/extracted/captain_falcon_profile.json"],
            outputs: &["docs/state_graphs/value_sheets/captain_falcon_combat_values.json"],
        },
        LedgerTabSpec {
            id: "stage_values",
            label: "Stage Values",
            kind: LedgerTabKind::Stage,
            status: LedgerTabStatus::Active,
            access: LedgerAccess {
                cli: LedgerSurfaceState::Active,
                gui: LedgerSurfaceState::Active,
            },
            summary: "Stage geometry, blast zones, spawn points, and friction surfaces.",
            source_artifacts: &["resources/melee/extracted/stages/battlefield_stage.json"],
            outputs: &["docs/state_graphs/value_sheets/battlefield_stage_values.json"],
        },
        LedgerTabSpec {
            id: "action_motion_tables",
            label: "Action / Motion Tables",
            kind: LedgerTabKind::Motion,
            status: LedgerTabStatus::Active,
            access: LedgerAccess {
                cli: LedgerSurfaceState::Active,
                gui: LedgerSurfaceState::Active,
            },
            summary: "Motion-state timing, callbacks, and transition windows.",
            source_artifacts: &[
                "resources/melee/frame_data/dolphin_mole/source_manifest.json",
                "docs/state_graphs/parity_reports/falcon_ecb_coverage.json",
            ],
            outputs: &["docs/state_graphs/action_motion_tables.json"],
        },
        LedgerTabSpec {
            id: "collision_volumes",
            label: "Collision Volumes",
            kind: LedgerTabKind::Collision,
            status: LedgerTabStatus::Planned,
            access: LedgerAccess {
                cli: LedgerSurfaceState::Planned,
                gui: LedgerSurfaceState::Planned,
            },
            summary: "Hitboxes, hurtboxes, ECB, body volumes, and other gameplay geometry.",
            source_artifacts: &[],
            outputs: &[],
        },
        LedgerTabSpec {
            id: "spawned_entities_weapons",
            label: "Spawned Entities / Weapons",
            kind: LedgerTabKind::Entity,
            status: LedgerTabStatus::Planned,
            access: LedgerAccess {
                cli: LedgerSurfaceState::Planned,
                gui: LedgerSurfaceState::Planned,
            },
            summary: "Gameplay objects spawned by moves or throws, not item-mode content.",
            source_artifacts: &[],
            outputs: &[],
        },
        LedgerTabSpec {
            id: "shield_grab_tech_ledge",
            label: "Shield / Grab / Tech / Ledge",
            kind: LedgerTabKind::Interaction,
            status: LedgerTabStatus::Planned,
            access: LedgerAccess {
                cli: LedgerSurfaceState::Planned,
                gui: LedgerSurfaceState::Planned,
            },
            summary: "Defense and interaction subsystems that do not fit cleanly elsewhere.",
            source_artifacts: &[],
            outputs: &[],
        },
        LedgerTabSpec {
            id: "character_special_state_values",
            label: "Character Special State Values",
            kind: LedgerTabKind::SpecialState,
            status: LedgerTabStatus::Planned,
            access: LedgerAccess {
                cli: LedgerSurfaceState::Planned,
                gui: LedgerSurfaceState::Planned,
            },
            summary: "Character-only resources and special mechanics that need a separate surface.",
            source_artifacts: &[],
            outputs: &[],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roadmap_includes_active_and_planned_subsystem_tabs() {
        let registry = LedgerRegistry::roadmap();
        let ids = registry.tabs.iter().map(|tab| tab.id).collect::<Vec<_>>();

        assert_eq!(
            ids,
            vec![
                "global_values",
                "character_values",
                "physics_engine_values",
                "global_combat_values",
                "captain_falcon_combat_values",
                "stage_values",
                "action_motion_tables",
                "collision_volumes",
                "spawned_entities_weapons",
                "shield_grab_tech_ledge",
                "character_special_state_values",
            ]
        );
        assert_eq!(
            registry
                .tabs
                .iter()
                .filter(|tab| tab.status == LedgerTabStatus::Active)
                .count(),
            7
        );
        assert_eq!(
            registry
                .tabs
                .iter()
                .filter(|tab| tab.status == LedgerTabStatus::Planned)
                .count(),
            4
        );
        assert_eq!(
            registry
                .devtool_surfaces
                .iter()
                .map(|surface| surface.id)
                .collect::<Vec<_>>(),
            vec![
                "state_graphs",
                "parity_ledger",
                "ecb_coverage",
                "input_trace",
                "slippi_replay",
                "move_keyframes",
            ]
        );
        assert!(registry.is_dual_surface());
    }

    #[test]
    fn ledger_map_round_trips_as_an_owned_consumer_shape() {
        let ledger_map = LedgerMap::from_registry(&LedgerRegistry::roadmap());
        let json = serde_json::to_string_pretty(&ledger_map).unwrap();
        let parsed: LedgerMap = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.registry.tab_count, 11);
        assert_eq!(parsed.registry.devtool_surface_count, 6);
        assert_eq!(parsed.tabs.len(), 11);
        assert_eq!(parsed.devtool_surfaces.len(), 6);
        assert!(parsed.is_dual_surface());
        assert_eq!(parsed.tabs[0].id, "global_values");
        assert_eq!(parsed.tabs[0].cli_surface, LedgerSurfaceState::Active);
        assert_eq!(parsed.tabs[6].status, LedgerTabStatus::Active);
        assert_eq!(parsed.devtool_surfaces[3].id, "input_trace");
        assert!(parsed.devtool_surfaces[3]
            .cli_commands
            .contains(&"replay artifacts".to_string()));
    }

    #[test]
    fn current_registry_keeps_only_the_active_tab_surface() {
        let registry = LedgerRegistry::current();
        let ids = registry.tabs.iter().map(|tab| tab.id).collect::<Vec<_>>();

        assert_eq!(
            ids,
            vec![
                "global_values",
                "character_values",
                "physics_engine_values",
                "global_combat_values",
                "captain_falcon_combat_values",
                "stage_values",
                "action_motion_tables",
            ]
        );
        assert_eq!(registry.devtool_surfaces.len(), 6);
        assert!(registry.is_dual_surface());
    }
}
