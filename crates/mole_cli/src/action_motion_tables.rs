use serde_json::{json, Value};
use std::{collections::BTreeSet, fs, path::Path};

use crate::{read_json, SCHEMA_VERSION};

pub(crate) const ACTION_MOTION_TABLES_OUTPUT: &str = "docs/state_graphs/action_motion_tables.json";
pub(crate) const ACTION_MOTION_TABLES_GENERATOR: &str =
    "crates/mole_cli/src/action_motion_tables.rs";
pub(crate) const ACTION_MOTION_TABLES_COMMAND: &str =
    "cargo run -p mole_cli -- generated write-action-motion-tables --write --json";
pub(crate) const ACTION_MOTION_TABLES_INPUTS: &[&str] = &[
    "resources/melee/frame_data/dolphin_mole/source_manifest.json",
    "docs/state_graphs/parity_reports/falcon_ecb_coverage.json",
];
pub(crate) const ACTION_MOTION_TABLES_OUTPUTS: &[&str] = &[ACTION_MOTION_TABLES_OUTPUT];

pub(crate) fn write_action_motion_tables_report(root: &Path, write: bool) -> Value {
    let artifact = action_motion_tables_json(root);
    let mut errors = artifact
        .get("errors")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if write && errors.is_empty() {
        let path = root.join(ACTION_MOTION_TABLES_OUTPUT);
        if let Some(parent) = path.parent() {
            if let Err(error) = fs::create_dir_all(parent) {
                errors.push(json!(format!(
                    "failed to create {}: {error}",
                    parent.display()
                )));
            }
        }
        if errors.is_empty() {
            let text = serde_json::to_string_pretty(&artifact)
                .expect("action motion tables artifact serializes");
            if let Err(error) = fs::write(&path, format!("{text}\n")) {
                errors.push(json!(format!(
                    "failed to write {}: {error}",
                    path.display()
                )));
            }
        }
    }
    json!({
        "schema_version": SCHEMA_VERSION,
        "command": "generated write-action-motion-tables",
        "mutated": write && errors.is_empty(),
        "output": ACTION_MOTION_TABLES_OUTPUT,
        "artifact": artifact,
        "errors": errors,
    })
}

fn action_motion_tables_json(root: &Path) -> Value {
    let manifest_path = root.join("resources/melee/frame_data/dolphin_mole/source_manifest.json");
    let mut errors = Vec::new();
    let manifest = match read_json(manifest_path.clone()) {
        Ok(manifest) => manifest,
        Err(error) => {
            errors.push(format!("{}: {error}", manifest_path.display()));
            Value::Null
        }
    };
    let gap_states = manifest
        .get("rust_parity_gaps")
        .and_then(Value::as_array)
        .map(|gaps| {
            gaps.iter()
                .filter_map(|gap| gap.get("state").and_then(Value::as_str))
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    let mut entries = source_manifest_actions(&manifest)
        .into_iter()
        .map(|action| action_motion_entry(action, &gap_states))
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        left.get("state")
            .and_then(Value::as_str)
            .unwrap_or("")
            .cmp(right.get("state").and_then(Value::as_str).unwrap_or(""))
    });
    let aligned_count = entries
        .iter()
        .filter(|entry| entry.get("completeness_class").and_then(Value::as_str) == Some("aligned"))
        .count();
    let absent_count = entries
        .iter()
        .filter(|entry| entry.get("completeness_class").and_then(Value::as_str) == Some("absent"))
        .count();
    json!({
        "schema_version": SCHEMA_VERSION,
        "artifact_kind": "action_motion_tables",
        "source_manifest_path": "resources/melee/frame_data/dolphin_mole/source_manifest.json",
        "imported_action_count": entries.len(),
        "aligned_action_count": aligned_count,
        "absent_action_count": absent_count,
        "entries": entries,
        "errors": errors,
    })
}

fn source_manifest_actions(manifest: &Value) -> Vec<&Value> {
    let Some(actions) = manifest.get("actions") else {
        return Vec::new();
    };
    if let Some(array) = actions.as_array() {
        return array.iter().collect();
    }
    actions
        .as_object()
        .map(|object| object.values().collect())
        .unwrap_or_default()
}

fn action_motion_entry(action: &Value, gap_states: &BTreeSet<&str>) -> Value {
    let state = action.get("state").and_then(Value::as_str).unwrap_or("");
    let runtime_binding = action
        .get("runtime_motion_state")
        .cloned()
        .unwrap_or(Value::Null);
    let completeness_class = if runtime_binding.is_string() {
        "aligned"
    } else if gap_states.contains(state) {
        "absent"
    } else {
        "unclassified"
    };
    json!({
        "state": state,
        "source_action_key": action.get("source_action_key").cloned().unwrap_or(Value::Null),
        "source_action_name": action.get("source_action_name").cloned().unwrap_or(Value::Null),
        "runtime_binding": runtime_binding,
        "completeness_class": completeness_class,
    })
}
