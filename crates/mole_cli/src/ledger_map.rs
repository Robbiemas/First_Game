use mole_ledger::{LedgerRegistry, LedgerSurfaceState, LedgerTabKind, LedgerTabStatus};
use serde_json::{json, Value};
use std::{fs, path::Path};

use crate::SCHEMA_VERSION;

pub(crate) const LEDGER_MAP_GENERATOR: &str = "crates/mole_cli/src/ledger_map.rs";
pub(crate) const LEDGER_MAP_COMMAND: &str =
    "cargo run -p mole_cli -- generated write-ledger-map --write --json";
pub(crate) const LEDGER_MAP_INPUTS: &[&str] = &["crates/mole_ledger/src/lib.rs"];
pub(crate) const LEDGER_MAP_OUTPUTS: &[&str] = &["docs/state_graphs/parity_ledger_map.json"];

pub(crate) fn write_ledger_map_report(root: &Path, write: bool) -> Value {
    let registry = LedgerRegistry::roadmap();
    let ledger_map = ledger_map_json();

    if write {
        if let Err(error) = write_outputs(root, &ledger_map) {
            return json!({
                "schema_version": SCHEMA_VERSION,
                "command": "generated write-ledger-map",
                "project_root": root.display().to_string(),
                "mutated": false,
                "ok": false,
                "error": error,
            });
        }
    }

    let tabs = registry
        .tabs
        .iter()
        .map(|tab| {
            json!({
                "id": tab.id,
                "label": tab.label,
                "kind": ledger_tab_kind_label(tab.kind),
                "status": ledger_tab_status_label(tab.status),
                "cli_surface": ledger_surface_state_label(tab.access.cli),
                "gui_surface": ledger_surface_state_label(tab.access.gui),
                "summary": tab.summary,
                "source_artifacts": tab.source_artifacts,
                "outputs": tab.outputs,
            })
        })
        .collect::<Vec<_>>();
    let devtool_surfaces = registry
        .devtool_surfaces
        .iter()
        .map(|surface| {
            json!({
                "id": surface.id,
                "label": surface.label,
                "status": ledger_tab_status_label(surface.status),
                "cli_surface": ledger_surface_state_label(surface.access.cli),
                "gui_surface": ledger_surface_state_label(surface.access.gui),
                "summary": surface.summary,
                "cli_commands": surface.cli_commands,
                "gui_section": surface.gui_section,
                "source_artifacts": surface.source_artifacts,
                "outputs": surface.outputs,
            })
        })
        .collect::<Vec<_>>();

    json!({
        "schema_version": SCHEMA_VERSION,
        "command": "generated write-ledger-map",
        "project_root": root.display().to_string(),
        "mutated": write,
        "ok": true,
        "write": write,
        "generator": LEDGER_MAP_GENERATOR,
        "recommended_command": LEDGER_MAP_COMMAND,
        "inputs": LEDGER_MAP_INPUTS,
        "outputs": LEDGER_MAP_OUTPUTS,
        "written_paths": if write {
            LEDGER_MAP_OUTPUTS.iter().map(|path| path.to_string()).collect::<Vec<_>>()
        } else {
            Vec::<String>::new()
        },
        "pending_paths": if write {
            Vec::<String>::new()
        } else {
            LEDGER_MAP_OUTPUTS
                .iter()
                .map(|path| path.to_string())
                .collect::<Vec<_>>()
        },
        "registry": {
            "tab_count": registry.tabs.len(),
            "active_tab_count": registry.tabs.iter().filter(|tab| tab.status == LedgerTabStatus::Active).count(),
            "planned_tab_count": registry.tabs.iter().filter(|tab| tab.status == LedgerTabStatus::Planned).count(),
            "devtool_surface_count": registry.devtool_surfaces.len(),
            "active_devtool_surface_count": registry.devtool_surfaces.iter().filter(|surface| surface.status == LedgerTabStatus::Active).count(),
            "planned_devtool_surface_count": registry.devtool_surfaces.iter().filter(|surface| surface.status == LedgerTabStatus::Planned).count(),
            "dual_surface": registry.is_dual_surface(),
        },
        "tabs": tabs,
        "devtool_surfaces": devtool_surfaces,
    })
}

fn write_outputs(root: &Path, ledger_map: &Value) -> Result<(), String> {
    let path = root.join(LEDGER_MAP_OUTPUTS[0]);
    let Some(parent) = path.parent() else {
        return Err(format!("missing parent directory for {}", path.display()));
    };
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let mut text = serde_json::to_string_pretty(ledger_map)
        .map_err(|error| format!("failed to serialize {}: {error}", LEDGER_MAP_OUTPUTS[0]))?;
    text.push('\n');
    fs::write(&path, text)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    Ok(())
}

pub(crate) fn ledger_map_json() -> Value {
    serde_json::to_value(mole_ledger::LedgerMap::from_registry(
        &LedgerRegistry::roadmap(),
    ))
    .expect("ledger map is serializable")
}

fn ledger_tab_kind_label(kind: LedgerTabKind) -> &'static str {
    match kind {
        LedgerTabKind::Global => "global",
        LedgerTabKind::Character => "character",
        LedgerTabKind::Physics => "physics",
        LedgerTabKind::Combat => "combat",
        LedgerTabKind::Stage => "stage",
        LedgerTabKind::Motion => "motion",
        LedgerTabKind::Collision => "collision",
        LedgerTabKind::Entity => "entity",
        LedgerTabKind::Interaction => "interaction",
        LedgerTabKind::SpecialState => "special_state",
    }
}

fn ledger_tab_status_label(status: LedgerTabStatus) -> &'static str {
    match status {
        LedgerTabStatus::Active => "active",
        LedgerTabStatus::Planned => "planned",
    }
}

fn ledger_surface_state_label(state: LedgerSurfaceState) -> &'static str {
    match state {
        LedgerSurfaceState::Active => "active",
        LedgerSurfaceState::Planned => "planned",
    }
}
